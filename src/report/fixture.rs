//! Helpers shared by the report's tests. Every figure is invented; see
//! `AGENTS.md`.

use super::Snapshot;
use crate::db::{self, Db};
use crate::money::Cents;
use chrono::{Local, NaiveDate, TimeZone};

pub(super) use jluszcz_finance_utils::testing::day;

/// An in-memory database with one paycheck on each of `dates`: a 4,000.00
/// salary less 600.00 federal tax.
pub(super) fn with_checks(dates: &[NaiveDate]) -> Db {
    let db = db::open_in_memory().unwrap();
    let (salary, federal) = (db.field_id("Salary"), db.field_id("Federal Tax"));
    for &date in dates {
        db.insert_paycheck(date, &[(salary, Cents(400_000)), (federal, Cents(60_000))])
            .unwrap();
    }
    db
}

pub(super) fn snapshot(db: &Db, today: NaiveDate) -> Snapshot {
    let generated_at = Local
        .with_ymd_and_hms(2026, 1, 16, 9, 30, 0)
        .single()
        .unwrap();
    Snapshot::load(db, today, generated_at).unwrap()
}
