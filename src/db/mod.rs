//! SQLite storage: opening and migrating the database, and the values it holds.

mod field;
mod migration;
mod paycheck;

use crate::money::Cents;
use anyhow::{Context, Result};
use chrono::NaiveDate;
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
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".local/share/paychecker/paychecks.db"))
}

/// Open (creating if needed) the database at `path`, creating its parent
/// directory if missing, and bring it up to this build's schema.
pub fn open(path: &Path) -> Result<Db> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let conn = Connection::open(path)
        .with_context(|| format!("opening database at {}", path.display()))?;
    prepare(&conn).with_context(|| format!("preparing database at {}", path.display()))?;
    Ok(Db { conn })
}

pub fn open_in_memory() -> Result<Db> {
    let conn = Connection::open_in_memory()?;
    prepare(&conn)?;
    Ok(Db { conn })
}

fn prepare(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    migration::run(conn)
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
    fn opening_a_file_creates_its_parent_directory() {
        let dir = scratch_dir("parent");
        let path = dir.join("nested").join("paychecks.db");
        open(&path).unwrap();
        assert!(path.exists());
        std::fs::remove_dir_all(&dir).unwrap();
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
}
