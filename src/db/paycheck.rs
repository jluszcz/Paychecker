//! Paycheck queries.

use super::{Db, FieldId, Paycheck, PaycheckId};
use crate::money::Cents;
use anyhow::{Result, ensure};
use chrono::NaiveDate;
use rusqlite::{Connection, params};
use std::collections::{BTreeMap, HashMap};

impl Db {
    pub fn paychecks(&self) -> Result<Vec<Paycheck>> {
        let mut checks: Vec<Paycheck> = self
            .conn
            .prepare("SELECT id, date FROM paycheck ORDER BY date")?
            .query_map([], |r| {
                Ok(Paycheck {
                    id: r.get(0)?,
                    date: r.get(1)?,
                    amounts: BTreeMap::new(),
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let index: HashMap<PaycheckId, usize> =
            checks.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
        let mut stmt = self
            .conn
            .prepare("SELECT paycheck_id, field_id, cents FROM amount")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let paycheck_id: PaycheckId = row.get(0)?;
            if let Some(&i) = index.get(&paycheck_id) {
                checks[i].amounts.insert(row.get(1)?, Cents(row.get(2)?));
            }
        }
        Ok(checks)
    }

    pub fn insert_paycheck(
        &self,
        date: NaiveDate,
        amounts: &[(FieldId, Cents)],
    ) -> Result<PaycheckId> {
        self.transaction(|conn| {
            ensure_date_free(conn, date, None)?;
            conn.execute("INSERT INTO paycheck (date) VALUES (?1)", [date])?;
            let id = PaycheckId(conn.last_insert_rowid());
            write_amounts(conn, id, amounts)?;
            Ok(id)
        })
    }

    pub fn update_paycheck(
        &self,
        id: PaycheckId,
        date: NaiveDate,
        amounts: &[(FieldId, Cents)],
    ) -> Result<()> {
        self.transaction(|conn| {
            ensure_date_free(conn, date, Some(id))?;
            conn.execute(
                "UPDATE paycheck SET date = ?1 WHERE id = ?2",
                params![date, id],
            )?;
            conn.execute("DELETE FROM amount WHERE paycheck_id = ?1", [id])?;
            write_amounts(conn, id, amounts)
        })
    }

    pub fn delete_paycheck(&self, id: PaycheckId) -> Result<()> {
        self.conn
            .execute("DELETE FROM paycheck WHERE id = ?1", [id])?;
        Ok(())
    }
}

/// Refuse `date` if a paycheck other than `except` already has it.
fn ensure_date_free(conn: &Connection, date: NaiveDate, except: Option<PaycheckId>) -> Result<()> {
    let taken: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM paycheck WHERE date = ?1 AND id IS NOT ?2)",
        params![date, except],
        |r| r.get(0),
    )?;
    ensure!(!taken, "a paycheck dated {date} already exists");
    Ok(())
}

fn write_amounts(conn: &Connection, id: PaycheckId, amounts: &[(FieldId, Cents)]) -> Result<()> {
    let mut stmt =
        conn.prepare("INSERT INTO amount (paycheck_id, field_id, cents) VALUES (?1, ?2, ?3)")?;
    for (field_id, cents) in amounts {
        stmt.execute(params![id, field_id, cents.0])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::db::{Db, FieldId, open_in_memory};
    use crate::money::Cents;
    use chrono::NaiveDate;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn stub(db: &Db) -> Vec<(FieldId, Cents)> {
        vec![
            (db.field_id("Salary"), Cents(400_000)),
            (db.field_id("Federal Tax"), Cents(60_000)),
        ]
    }

    fn amount_rows(db: &Db) -> i64 {
        db.conn
            .query_row("SELECT COUNT(*) FROM amount", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn a_saved_paycheck_comes_back_with_its_date_and_every_amount() {
        let db = open_in_memory().unwrap();
        let id = db.insert_paycheck(day(2026, 1, 16), &stub(&db)).unwrap();
        let checks = db.paychecks().unwrap();
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].id, id);
        assert_eq!(checks[0].date, day(2026, 1, 16));
        assert_eq!(checks[0].amounts[&db.field_id("Salary")], Cents(400_000));
        assert_eq!(
            checks[0].amounts[&db.field_id("Federal Tax")],
            Cents(60_000)
        );
        assert_eq!(checks[0].amounts.len(), 2);
    }

    #[test]
    fn paychecks_come_back_oldest_first() {
        let db = open_in_memory().unwrap();
        db.insert_paycheck(day(2026, 1, 16), &[]).unwrap();
        db.insert_paycheck(day(2025, 12, 19), &[]).unwrap();
        db.insert_paycheck(day(2026, 1, 2), &[]).unwrap();
        let dates: Vec<_> = db.paychecks().unwrap().iter().map(|p| p.date).collect();
        assert_eq!(
            dates,
            [day(2025, 12, 19), day(2026, 1, 2), day(2026, 1, 16)]
        );
    }

    #[test]
    fn a_second_paycheck_on_the_same_date_is_refused() {
        let db = open_in_memory().unwrap();
        db.insert_paycheck(day(2026, 1, 16), &[]).unwrap();
        let err = db.insert_paycheck(day(2026, 1, 16), &[]).unwrap_err();
        assert_eq!(
            err.to_string(),
            "a paycheck dated 2026-01-16 already exists"
        );
        let other = db.insert_paycheck(day(2026, 1, 30), &[]).unwrap();
        assert!(db.update_paycheck(other, day(2026, 1, 16), &[]).is_err());
    }

    #[test]
    fn editing_a_paycheck_without_changing_its_date_is_allowed() {
        let db = open_in_memory().unwrap();
        let id = db.insert_paycheck(day(2026, 1, 16), &stub(&db)).unwrap();
        db.update_paycheck(id, day(2026, 1, 16), &stub(&db))
            .unwrap();
    }

    #[test]
    fn editing_a_paycheck_replaces_its_date_and_amounts() {
        let db = open_in_memory().unwrap();
        let id = db.insert_paycheck(day(2026, 1, 16), &stub(&db)).unwrap();
        let hsa = db.field_id("HSA");
        db.update_paycheck(id, day(2026, 1, 17), &[(hsa, Cents(10_000))])
            .unwrap();
        let check = &db.paychecks().unwrap()[0];
        assert_eq!(check.date, day(2026, 1, 17));
        assert_eq!(check.amounts.len(), 1);
        assert_eq!(check.amounts[&hsa], Cents(10_000));
    }

    #[test]
    fn a_failed_edit_changes_nothing() {
        let db = open_in_memory().unwrap();
        let id = db.insert_paycheck(day(2026, 1, 16), &stub(&db)).unwrap();
        assert!(
            db.update_paycheck(id, day(2026, 1, 16), &[(FieldId(9_999), Cents(1))])
                .is_err()
        );
        assert_eq!(db.paychecks().unwrap()[0].amounts.len(), 2);
    }

    #[test]
    fn deleting_a_paycheck_deletes_its_amounts() {
        let db = open_in_memory().unwrap();
        let id = db.insert_paycheck(day(2026, 1, 16), &stub(&db)).unwrap();
        db.delete_paycheck(id).unwrap();
        assert!(db.paychecks().unwrap().is_empty());
        assert_eq!(amount_rows(&db), 0);
    }

    #[test]
    fn a_field_a_paycheck_uses_cannot_be_deleted() {
        let db = open_in_memory().unwrap();
        db.insert_paycheck(day(2026, 1, 16), &stub(&db)).unwrap();
        let err = db.delete_field(db.field_id("Salary")).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Salary is used by a paycheck; archive it with x instead"
        );
        assert_eq!(db.fields().unwrap()[0].name, "Salary");
    }
}
