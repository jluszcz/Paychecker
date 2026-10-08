//! SQLite storage: opening and migrating the database, and the values it holds.

mod field;
mod migration;
mod paycheck;

use crate::money::Cents;
use anyhow::Result;
use chrono::NaiveDate;
use jluszcz_finance_utils::sqlite;
pub use jluszcz_finance_utils::sqlite::snapshot;
use rusqlite::Connection;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub type FieldId = i64;
pub type PaycheckId = i64;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Income,
    Deduction,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Income => "income",
            Kind::Deduction => "deduction",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Income => "Income",
            Kind::Deduction => "Deduction",
        }
    }

    pub fn other(self) -> Kind {
        match self {
            Kind::Income => Kind::Deduction,
            Kind::Deduction => Kind::Income,
        }
    }

    pub fn from_sql_text(text: &str) -> Option<Kind> {
        match text {
            "income" => Some(Kind::Income),
            "deduction" => Some(Kind::Deduction),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub id: FieldId,
    pub name: String,
    pub kind: Kind,
    pub position: i64,
    pub archived: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paycheck {
    pub id: PaycheckId,
    pub date: NaiveDate,
    pub amounts: BTreeMap<FieldId, Cents>,
}

pub struct Db {
    conn: Connection,
}

impl Db {
    #[cfg(test)]
    pub(crate) fn field_id(&self, name: &str) -> FieldId {
        self.fields()
            .unwrap()
            .into_iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("no field named {name:?}"))
            .id
    }

    /// Run `f` inside one transaction, committing only if it returns `Ok`.
    fn transaction<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let tx = self.conn.unchecked_transaction()?;
        let value = f(&tx)?;
        tx.commit()?;
        Ok(value)
    }

    /// Whether this connection has inserted, updated or deleted a row. It
    /// counts this run only, so a change made by another process leaves it
    /// `false`.
    pub fn wrote_rows(&self) -> bool {
        self.conn.total_changes() > 0
    }
}

/// `~/.local/share/paychecker/paychecks.db`.
pub fn default_path() -> Result<PathBuf> {
    jluszcz_finance_utils::config::data_path(crate::config::APP, "paychecks.db")
}

/// Open (creating if needed) the database at `path`, creating its parent
/// directory if missing, and bring it up to this build's schema.
pub fn open(path: &Path) -> Result<Db> {
    Ok(Db {
        conn: sqlite::open(path, &migration::SCHEMA)?,
    })
}

pub fn open_in_memory() -> Result<Db> {
    Ok(Db {
        conn: sqlite::open_in_memory(&migration::SCHEMA)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("paychecker-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_new_database_is_at_schema_version_one() {
        let db = open_in_memory().unwrap();
        let version: i64 = db
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn a_new_database_enforces_foreign_keys() {
        let db = open_in_memory().unwrap();
        let on: bool = db
            .conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert!(on);
    }

    #[test]
    fn a_file_written_by_a_newer_build_refuses_to_open() {
        let dir = scratch_dir("newer");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("paychecks.db");
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", 99)
            .unwrap();
        let err = open(&path).err().expect("opened a newer database");
        assert!(
            format!("{err:#}").contains("newer than this build"),
            "{err:#}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn kinds_round_trip_through_their_sql_text() {
        for kind in [Kind::Income, Kind::Deduction] {
            assert_eq!(Kind::from_sql_text(kind.as_str()), Some(kind));
        }
        assert_eq!(Kind::from_sql_text("bonus"), None);
    }

    #[test]
    fn reopening_a_database_without_writing_reports_no_rows_written() {
        let dir = scratch_dir("wrote_none");
        let path = dir.join("paychecks.db");
        drop(open(&path).unwrap());
        assert!(!open(&path).unwrap().wrote_rows());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A brand-new file's seed fields are rows this run wrote, so its first
    /// quit writes a page rather than finding nothing changed.
    #[test]
    fn creating_a_database_counts_its_seed_fields_as_rows_written() {
        assert!(open_in_memory().unwrap().wrote_rows());
    }

    #[test]
    fn saving_a_paycheck_reports_rows_written() {
        let dir = scratch_dir("wrote_some");
        let path = dir.join("paychecks.db");
        drop(open(&path).unwrap());
        let db = open(&path).unwrap();
        let salary = db.field_id("Salary");
        db.insert_paycheck(
            NaiveDate::from_ymd_opt(2026, 1, 2).unwrap(),
            &[(salary, Cents(400_000))],
        )
        .unwrap();
        assert!(db.wrote_rows());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The scheduled backup runs while `pc`'s own connection is still open,
    /// and a WAL database's recent writes live in the `-wal` file until a
    /// checkpoint.
    #[test]
    fn a_snapshot_holds_writes_still_in_the_wal_of_an_open_database() {
        let dir = scratch_dir("snapshot");
        let path = dir.join("paychecks.db");
        let db = open(&path).unwrap();
        let salary = db.field_id("Salary");
        let date = NaiveDate::from_ymd_opt(2026, 1, 16).unwrap();
        db.insert_paycheck(date, &[(salary, Cents(400_000))])
            .unwrap();

        let copy = dir.join("copy.db");
        snapshot(&path, &copy).unwrap();

        let paychecks = open(&copy).unwrap().paychecks().unwrap();
        assert_eq!(paychecks.len(), 1);
        assert_eq!(paychecks[0].date, date);
        assert_eq!(paychecks[0].amounts.get(&salary), Some(&Cents(400_000)));
        drop(db);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
