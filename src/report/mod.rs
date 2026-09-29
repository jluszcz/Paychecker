//! The HTML report: every year's Sheet as one self-contained page a phone can
//! open offline, one tab per year.
//!
//! `html` writes the page readably and `write` minifies it on the way to the
//! disk, so what a test asserts against and what a phone downloads are the
//! same page in two forms.

pub mod html;

#[cfg(test)]
mod fixture;

use crate::calc::{self, Sheet};
use crate::db::Db;
use anyhow::Result;
use chrono::{DateTime, Datelike, Local, NaiveDate};

/// What the page is drawn from, read in one pass.
pub struct Snapshot {
    /// One per year with at least one paycheck, newest first.
    pub sheets: Vec<Sheet>,
    pub today: NaiveDate,
    pub generated_at: DateTime<Local>,
}

impl Snapshot {
    pub fn load(db: &Db, today: NaiveDate, generated_at: DateTime<Local>) -> Result<Snapshot> {
        let fields = db.fields()?;
        let paychecks = db.paychecks()?;
        let mut years: Vec<i32> = paychecks.iter().map(|p| p.date.year()).collect();
        years.sort_unstable_by(|a, b| b.cmp(a));
        years.dedup();
        let sheets = years
            .into_iter()
            .map(|year| calc::sheet(year, &fields, &paychecks))
            .collect();
        Ok(Snapshot {
            sheets,
            today,
            generated_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::{day, snapshot, with_checks};

    #[test]
    fn a_snapshot_has_one_sheet_per_year_with_paychecks_newest_first() {
        let db = with_checks(&[day(2025, 12, 19), day(2026, 1, 2), day(2026, 1, 16)]);
        let years: Vec<i32> = snapshot(&db, day(2026, 1, 16))
            .sheets
            .iter()
            .map(|s| s.year)
            .collect();
        assert_eq!(years, [2026, 2025]);
    }

    #[test]
    fn a_snapshot_of_an_empty_database_has_no_sheets() {
        let db = with_checks(&[]);
        assert!(snapshot(&db, day(2026, 1, 16)).sheets.is_empty());
    }
}
