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
use crate::config::Config;
use crate::db::Db;
use anyhow::Result;
use chrono::{DateTime, Datelike, Local, NaiveDate};
use std::path::Path;

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

/// The one name the report is written under, so a phone bookmark keeps
/// working.
pub const FILE_NAME: &str = "Paychecks.html";

pub use jluszcz_finance_utils::report::{Outcome, Written};

/// Write the report into `dir`, whatever the config says. `pc report` calls
/// this directly; the quit path reaches it through `write_if_enabled`.
pub fn write(db: &Db, dir: &Path, today: NaiveDate) -> Result<Written> {
    let snapshot = Snapshot::load(db, today, Local::now())?;
    jluszcz_finance_utils::report::write(dir, FILE_NAME, &html::page(&snapshot))
}

/// Write the report on quit, if the config asks for one and it is due.
///
/// `scratch` is a run given `--db` or `--today`. The configured directory
/// holds the page for the real database on the real day, so such a run skips
/// before anything is read; `pc report --dir` is how it writes one.
pub fn write_if_enabled(db: &Db, cfg: &Config, today: NaiveDate, scratch: bool) -> Result<Outcome> {
    jluszcz_finance_utils::report::write_if_enabled(
        cfg.report.as_ref(),
        scratch,
        FILE_NAME,
        today,
        db.wrote_rows(),
        || Ok(html::page(&Snapshot::load(db, today, Local::now())?)),
    )
}

#[cfg(test)]
mod tests {
    use super::fixture::{day, snapshot, with_checks};
    use super::*;
    use crate::config::Config;
    use crate::db;
    use std::path::PathBuf;

    fn scratch(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("paychecker_report_{label}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// The minifier lowercases the doctype, so this reads the file rather
    /// than the case `html::page` spells it in.
    fn is_the_page(text: &str) -> bool {
        text.to_ascii_lowercase().starts_with("<!doctype html>")
    }

    /// A database file opened, closed and opened again, so that this
    /// connection has written nothing.
    fn reopened(dir: &Path) -> Db {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join("paychecks.db");
        drop(db::open(&path).unwrap());
        db::open(&path).unwrap()
    }

    /// A `--db` or `--today` run is a scratch session, and the configured
    /// directory holds the page for the real database: overwriting it would
    /// put scratch figures on the phone.
    #[test]
    fn a_scratch_run_leaves_the_configured_page_alone() {
        let dir = scratch("scratch_run");
        let db = with_checks(&[day(2026, 1, 2)]);
        let outcome =
            write_if_enabled(&db, &Config::reporting_to(&dir), day(2026, 1, 2), true).unwrap();
        assert!(matches!(outcome, Outcome::Skipped), "{outcome:?}");
        assert!(
            !dir.exists(),
            "a scratch run wrote into the configured directory"
        );
    }

    /// `today` is the real local date because the gate reads the page's mtime,
    /// which is the real day it was written.
    #[test]
    fn a_quit_that_changed_nothing_leaves_todays_page_alone() {
        let dir = scratch("unchanged");
        let db = reopened(&dir);
        let today = Local::now().date_naive();
        write(&db, &dir, today).unwrap();
        let outcome = write_if_enabled(&db, &Config::reporting_to(&dir), today, false).unwrap();
        assert!(matches!(outcome, Outcome::Unchanged), "{outcome:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_quit_that_wrote_rows_rewrites_the_page() {
        let dir = scratch("rewrite");
        let db = with_checks(&[day(2026, 1, 2)]);
        let today = Local::now().date_naive();
        write(&db, &dir, today).unwrap();
        let outcome = write_if_enabled(&db, &Config::reporting_to(&dir), today, false).unwrap();
        assert!(matches!(outcome, Outcome::Written(_)), "{outcome:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Every control is a radio and a sibling selector, so a minifier that
    /// dropped an id or reordered an input past its panel would leave a page
    /// that renders and then does nothing. Quotes come off before matching,
    /// so which attributes the minifier unquotes stays its business.
    #[test]
    fn minification_leaves_the_column_bands_intact() {
        let dir = scratch("bands");
        let written = write(&with_checks(&[day(2026, 1, 2)]), &dir, day(2026, 1, 2)).unwrap();
        let page = std::fs::read_to_string(&written.path).unwrap();
        assert!(
            page.contains(":nth-child(2n):not(:last-child){background:var(--band)}")
                || page.contains(":nth-child(even):not(:last-child){background:var(--band)}"),
            "the band rule did not survive: {page}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The phone's rules are all inside one media query, which a minifier is
    /// free to rewrite. It sorts properties, so each is matched on its own.
    #[test]
    fn minification_leaves_the_narrow_screen_rules_intact() {
        let dir = scratch("narrow");
        let written = write(&with_checks(&[day(2026, 1, 2)]), &dir, day(2026, 1, 2)).unwrap();
        let page = std::fs::read_to_string(&written.path).unwrap();
        let narrow = page
            .split("@media")
            .find(|block| block.contains("480px"))
            .unwrap_or_else(|| panic!("no narrow-screen rules: {page}"));
        for piece in [
            ".c{display:none}",
            "scroll-snap-type:x mandatory",
            "scroll-padding-left:6.5rem",
            "scroll-snap-align:start",
            "max-width:6rem",
        ] {
            assert!(narrow.contains(piece), "missing {piece}: {narrow}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn minification_leaves_every_tab_and_its_switch_intact() {
        let dir = scratch("switches");
        let db = with_checks(&[day(2025, 12, 19), day(2026, 1, 2)]);
        let written = write(&db, &dir, day(2026, 1, 2)).unwrap();
        let page = std::fs::read_to_string(&written.path).unwrap();
        assert!(is_the_page(&page), "the doctype did not survive");
        let unquoted = page.replace('"', "");
        for year in [2025, 2026] {
            let id = html::tab_id(year);
            assert!(
                unquoted.contains(&format!("for={id}>{year}</label>")),
                "no {year} label: {page}"
            );
            assert!(
                unquoted.contains(&format!("id={id}-panel")),
                "no {year} panel: {page}"
            );
            assert!(
                page.contains(&format!("#{id}:checked~#{id}-panel{{display:block}}")),
                "no {year} switch: {page}"
            );
        }
        // The minifier sorts attributes, so the radio is found by its id and
        // then asked whether it is the checked one.
        let radio = |id: &str| {
            unquoted
                .split('<')
                .find(|tag| tag.starts_with("input") && tag.contains(&format!("id={id} ")))
                .unwrap_or_else(|| panic!("no {id} radio: {page}"))
                .to_string()
        };
        assert!(
            radio("y2026").contains("checked"),
            "the page does not open on 2026: {page}"
        );
        assert!(
            !radio("y2025").contains("checked"),
            "2025 is checked too: {page}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

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
