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
use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, Local, NaiveDate};
use minify_html::Cfg;
use std::path::{Path, PathBuf};

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

/// Written beside the report and renamed onto it: a partial file under the
/// real name is what this exists to prevent. The pid is in the name because
/// a `pc report --dir` run can overlap an open app's quit in the same
/// directory.
fn temp_name() -> String {
    format!(".{FILE_NAME}.{}.tmp", std::process::id())
}

/// The page as it goes to the disk. `html::page` stays readable because its
/// tests assert against it; this is the one place between it and the disk.
/// `minify_css` because the whole layout is one inline `<style>`; no JS
/// minifier because the page carries no script.
fn minify(page: &str) -> Vec<u8> {
    let cfg = Cfg {
        minify_css: true,
        ..Cfg::new()
    };
    minify_html::minify(page.as_bytes(), &cfg)
}

/// A size for a person to read in a one-line message: whole KiB, rounded up
/// so a small page never reads as `0 KiB`, and whole MiB from one up.
pub fn human_bytes(bytes: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    if bytes >= MIB {
        format!("{} MiB", bytes / MIB)
    } else {
        format!("{} KiB", bytes.div_ceil(1024))
    }
}

/// A page that reached the disk: where it landed, and how big it is.
#[derive(Debug)]
pub struct Written {
    pub path: PathBuf,
    pub bytes: u64,
}

#[derive(Debug)]
pub enum Outcome {
    /// No `[report]` section.
    Disabled,
    /// A `--db` or `--today` run; see [`write_if_enabled`].
    Skipped,
    /// Today's page is already there, and this run wrote no rows.
    Unchanged,
    Written(Written),
}

/// Whether the page is owed a rewrite: this run changed something, or the
/// page on disk was not written today.
///
/// The directory is usually a synced one, so a rename that produces the same
/// bytes still costs an upload and a download on the phone.
///
/// Both halves are approximations. `wrote_rows` sees only this run's
/// connection. `last_written` is the page's mtime, which is the day it was
/// *written*, not the day it quotes, so a `--today` run or a session held
/// across midnight can leave a stale page standing until the next run that
/// writes a row.
pub fn is_due(last_written: Option<NaiveDate>, today: NaiveDate, wrote_rows: bool) -> bool {
    wrote_rows || last_written != Some(today)
}

/// The local day `path` was last written, or `None` for every reason it
/// cannot be read. All of those mean the page is due.
fn written_on(path: &Path) -> Option<NaiveDate> {
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    Some(DateTime::<Local>::from(modified).date_naive())
}

/// The two steps that can fail with a temporary file on disk, so `write` has
/// one error path to clean up after.
fn write_then_rename(temp: &Path, path: &Path, page: &[u8]) -> Result<()> {
    std::fs::write(temp, page).with_context(|| format!("writing {}", temp.display()))?;
    // The rename replaces the file rather than rewriting it, so a page the
    // owner narrowed to themselves would otherwise reopen at the umask default.
    if let Some(existing) = std::fs::metadata(path).ok().filter(|m| m.is_file()) {
        std::fs::set_permissions(temp, existing.permissions())
            .with_context(|| format!("setting permissions on {}", temp.display()))?;
    }
    std::fs::rename(temp, path).with_context(|| format!("renaming onto {}", path.display()))
}

/// Write the report into `dir`, whatever the config says. `pc report` calls
/// this directly; the quit path reaches it through `write_if_enabled`.
pub fn write(db: &Db, dir: &Path, today: NaiveDate) -> Result<Written> {
    let snapshot = Snapshot::load(db, today, Local::now())?;
    let page = minify(&html::page(&snapshot));

    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    // Same directory, so the rename stays atomic rather than crossing a
    // filesystem, and a sync client never sees the page half-written.
    let temp = dir.join(temp_name());
    let path = dir.join(FILE_NAME);
    write_then_rename(&temp, &path, &page).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })?;

    Ok(Written {
        path,
        bytes: page.len() as u64,
    })
}

/// Write the report on quit, if the config asks for one and it is due.
///
/// `scratch` is a run given `--db` or `--today`. The configured directory
/// holds the page for the real database on the real day, so such a run skips
/// before anything is read; `pc report --dir` is how it writes one.
pub fn write_if_enabled(db: &Db, cfg: &Config, today: NaiveDate, scratch: bool) -> Result<Outcome> {
    let Some(report) = cfg.report.as_ref() else {
        return Ok(Outcome::Disabled);
    };
    if scratch {
        return Ok(Outcome::Skipped);
    }
    let dir = report.dir()?;
    if !is_due(written_on(&dir.join(FILE_NAME)), today, db.wrote_rows()) {
        return Ok(Outcome::Unchanged);
    }
    Ok(Outcome::Written(write(db, &dir, today)?))
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

    #[test]
    fn a_page_is_due_unless_it_was_written_today_by_a_run_that_changed_nothing() {
        let today = day(2026, 1, 16);
        let yesterday = day(2026, 1, 15);
        assert!(!is_due(Some(today), today, false));
        assert!(is_due(Some(today), today, true));
        assert!(is_due(Some(yesterday), today, false));
        assert!(is_due(None, today, false));
    }

    #[test]
    fn writing_lands_the_page_under_its_name_and_leaves_no_temporary_file() {
        let root = scratch("lands");
        let dir = root.join("not").join("yet");
        let written = write(&with_checks(&[day(2026, 1, 2)]), &dir, day(2026, 1, 2)).unwrap();
        assert_eq!(written.path, dir.join(FILE_NAME));
        assert_eq!(
            written.bytes,
            std::fs::metadata(&written.path).unwrap().len()
        );
        assert!(is_the_page(
            &std::fs::read_to_string(&written.path).unwrap()
        ));
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, [FILE_NAME]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// A non-empty directory squatting on the name: the rename fails, and the
    /// temporary file must not be left behind in a synced folder.
    #[test]
    fn a_page_that_cannot_be_renamed_into_place_leaves_no_temporary_file() {
        let dir = scratch("blocked");
        std::fs::create_dir_all(dir.join(FILE_NAME).join("occupied")).unwrap();
        assert!(write(&with_checks(&[]), &dir, day(2026, 1, 2)).is_err());
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, [FILE_NAME]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rewriting_the_page_keeps_the_permissions_it_was_given() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("perms");
        let db = with_checks(&[day(2026, 1, 2)]);
        let written = write(&db, &dir, day(2026, 1, 2)).unwrap();
        std::fs::set_permissions(&written.path, std::fs::Permissions::from_mode(0o600)).unwrap();
        write(&db, &dir, day(2026, 1, 2)).unwrap();
        let mode = std::fs::metadata(&written.path)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        std::fs::remove_dir_all(&dir).unwrap();
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

    #[test]
    fn no_report_section_writes_nothing() {
        let outcome = write_if_enabled(
            &with_checks(&[]),
            &Config::default(),
            day(2026, 1, 2),
            false,
        )
        .unwrap();
        assert!(matches!(outcome, Outcome::Disabled), "{outcome:?}");
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

    #[test]
    fn a_size_under_a_mebibyte_is_whole_kibibytes_rounded_up() {
        assert_eq!(human_bytes(2048), "2 KiB");
        assert_eq!(human_bytes(1025), "2 KiB");
    }

    #[test]
    fn a_size_of_a_mebibyte_or_more_is_whole_mebibytes() {
        assert_eq!(human_bytes(4 * 1024 * 1024), "4 MiB");
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
