# HTML Report on Close Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When `pc` quits, write every year's Sheet as one self-contained, offline-readable HTML page into a configured directory. `pc report [--dir]` writes the same page on demand.

**Architecture:**
- `config` reads an optional TOML file.
- `report::Snapshot` reads fields and paychecks once and builds a `calc::Sheet` per year.
- `report::html::page` renders them as one page with CSS-radio year tabs.
- `report::write` minifies the page and renames it into place atomically.
- `report::write_if_enabled` is the quit path's gate: it skips when the run changed nothing and today's page is already there.
- It is MisterManager's `src/report/`, copied in and trimmed.

**Tech Stack:** Rust 2024, rusqlite, chrono, clap. New: `serde`, `toml`, `minify-html`.

**Spec:** `docs/superpowers/specs/2026-09-29-html-report-design.md`

## Global Constraints

- **No real data.** Every money literal is invented; no real employer or person's name anywhere, including commit messages (AGENTS.md).
- `Cents(i64)` is the only money type; figures render through `Cents`' `Display` (`4,000.00`).
- `calc` stays pure — nothing in this plan edits `src/calc.rs`.
- Test names are full sentences. Unit tests live in `mod tests` at the bottom of the file under test. Database tests use `db::open_in_memory()` unless the test is about a file.
- Before every commit: `cargo fmt` and `cargo clippy --all-targets -- -D warnings` pass.
- Comments carry the non-obvious *why*. None of them say "changed to…", "previously…", or "copied from MisterManager".
- Commit messages follow the repo's style (`feat(report): …`) and **carry no Co-Authored-By or AI attribution trailer**.
- The page carries no `<script>` and no URL; `minify_html` is named only in `src/report/mod.rs`; `serde`/`toml` only in `src/config.rs`.
- File name on disk: `Paychecks.html`. Config path: `$XDG_CONFIG_HOME/paychecker/config.toml`, else `~/.config/paychecker/config.toml`.
- Dependency versions (MisterManager's): `minify-html = "0.18"`, `serde = { version = "1.0", features = ["derive"] }`, `toml = "1.1"`.

## Review Focus

1. **A field name containing `<`, `&` or `"`** should appear literally, not break the page. Pinned in Task 3 (`a_field_name_carrying_markup_is_escaped`).
2. **A `[report] dir` that does not exist yet**, such as a fresh sync folder, should be created, not fail. Pinned in Task 4 (`writing_lands_the_page_under_its_name_and_leaves_no_temporary_file` writes into a missing nested directory).
3. **A target that cannot be replaced** should return an error and leave no `.tmp` litter in the synced folder. Pinned in Task 4 (`a_page_that_cannot_be_renamed_into_place_leaves_no_temporary_file`).
4. **A malformed config file** should fail before the TUI opens and name the file, not after a session's work. Pinned in Task 1 (`a_config_file_that_does_not_parse_is_an_error_naming_its_path`), with the load ordered before `tui::run` in Task 5.
5. **A brand-new database file** has its seed inserts counted as writes, so its first quit writes a page. A reopened, untouched file counts none. Pinned in Task 2.

---

### Task 1: Config file

**Files:**
- Modify: `Cargo.toml` (dependencies)
- Create: `src/config.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Produces:
  - `config::Config { pub report: Option<Report> }` (Default)
  - `config::Report` with a private `dir: String`, and `pub fn dir(&self) -> anyhow::Result<PathBuf>`
  - `config::default_path() -> Result<PathBuf>`
  - `config::load(&Path) -> Result<Config>`

- [ ] **Step 1: Add the dependencies**

In `Cargo.toml` `[dependencies]`, keep alphabetical order:

```toml
[dependencies]
anyhow = "1.0"
chrono = { version = "0.4", default-features = false, features = ["std", "clock"] }
clap = { version = "4.6", features = ["derive"] }
minify-html = "0.18"
ratatui = "0.30"
rusqlite = { version = "0.40", features = ["bundled", "chrono"] }
serde = { version = "1.0", features = ["derive"] }
thiserror = "2.0"
toml = "1.1"
```

- [ ] **Step 2: Write `src/config.rs` with its tests and a stub `load`**

```rust
//! The configuration file, and the only place `serde` and `toml` are named.
//!
//! An absent file, or one with no `[report]` section, means the report is off:
//! a clean checkout and an unconfigured machine both do nothing. A file that is
//! present but does not parse is an error instead. `dir` has no default, so the
//! typo that would otherwise switch the report off silently (`directory =`) is
//! a missing field. Keys nothing reads are ignored.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Deserialize, PartialEq)]
pub struct Config {
    pub report: Option<Report>,
}

/// Where the HTML report is written on quit.
#[derive(Debug, Deserialize, PartialEq)]
pub struct Report {
    dir: String,
}

impl Report {
    /// The directory, with a leading `~/` expanded against `$HOME`. TOML does
    /// not expand it, and a `~` anywhere else is an ordinary character in a
    /// directory name.
    pub fn dir(&self) -> Result<PathBuf> {
        match self.dir.strip_prefix("~/") {
            Some(rest) => {
                let home = std::env::var_os("HOME").context("HOME is not set")?;
                Ok(PathBuf::from(home).join(rest))
            }
            None => Ok(PathBuf::from(&self.dir)),
        }
    }
}

/// `$XDG_CONFIG_HOME/paychecker/config.toml`, or `~/.config` when it is unset
/// or empty.
pub fn default_path() -> Result<PathBuf> {
    let dir = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => {
            let home = std::env::var_os("HOME").context("HOME is not set")?;
            PathBuf::from(home).join(".config")
        }
    };
    Ok(dir.join("paychecker").join("config.toml"))
}

pub fn load(path: &Path) -> Result<Config> {
    let _ = path;
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes `body` to a temp file named for the test, since the tests run in
    /// one process at once and a shared name would have them reading each
    /// other's files.
    fn fixture(label: &str, body: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "paychecker_config_{label}_{}.toml",
            std::process::id()
        ));
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn a_report_section_naming_only_a_dir_is_a_complete_configuration() {
        let path = fixture("report", "[report]\ndir = \"/tmp/reports\"\n");
        let report = load(&path).unwrap().report.unwrap();
        assert_eq!(report.dir().unwrap(), PathBuf::from("/tmp/reports"));
    }

    #[test]
    fn a_misspelled_dir_is_an_error_rather_than_a_silently_disabled_report() {
        let path = fixture("typo", "[report]\ndirectory = \"/tmp/reports\"\n");
        assert!(load(&path).is_err());
    }

    #[test]
    fn a_config_file_with_no_report_section_leaves_reports_off() {
        let path = fixture("no_report", "[other]\nkey = 1\n");
        assert_eq!(load(&path).unwrap(), Config::default());
    }

    #[test]
    fn an_absent_config_file_leaves_reports_off() {
        let path = std::env::temp_dir().join("paychecker_config_absent_nowhere.toml");
        assert_eq!(load(&path).unwrap(), Config::default());
    }

    #[test]
    fn a_config_file_that_does_not_parse_is_an_error_naming_its_path() {
        let path = fixture("broken", "[report\n");
        let err = load(&path).unwrap_err();
        assert!(
            format!("{err:#}").contains(&path.display().to_string()),
            "{err:#}"
        );
    }

    #[test]
    fn a_leading_tilde_in_the_report_dir_expands_against_home() {
        let path = fixture("tilde", "[report]\ndir = \"~/reports\"\n");
        let report = load(&path).unwrap().report.unwrap();
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        assert_eq!(report.dir().unwrap(), home.join("reports"));
    }

    #[test]
    fn a_tilde_in_the_middle_of_the_report_dir_is_left_alone() {
        let path = fixture("mid_tilde", "[report]\ndir = \"/tmp/a~b\"\n");
        let report = load(&path).unwrap().report.unwrap();
        assert_eq!(report.dir().unwrap(), PathBuf::from("/tmp/a~b"));
    }
}
```

In `src/lib.rs`, add the module in alphabetical order:

```rust
pub mod calc;
pub mod config;
pub mod db;
pub mod money;
pub mod tui;
```

- [ ] **Step 3: Run the tests to see them fail**

Run: `cargo test config::`
Expected: all 7 tests panic with `not yet implemented`.

- [ ] **Step 4: Implement `load`**

```rust
pub fn load(path: &Path) -> Result<Config> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test config::`
Expected: 7 passed.

- [ ] **Step 6: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add Cargo.toml Cargo.lock src/config.rs src/lib.rs
git commit -m "feat(config): read an optional config file with a [report] section"
```

---

### Task 2: `Db::wrote_rows`

**Files:**
- Modify: `src/db/mod.rs` (the `impl Db` block, and `mod tests`)

**Interfaces:**
- Produces: `Db::wrote_rows(&self) -> bool`

- [ ] **Step 1: Write the failing tests**

Append to `mod tests` in `src/db/mod.rs`. `scratch_dir` already exists there.

```rust
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
```

If `NaiveDate` or `Cents` is not already in scope in `db/mod.rs`'s tests, add `use chrono::NaiveDate;` and `use crate::money::Cents;` inside `mod tests`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test db::tests`
Expected: compile error, because there is no method `wrote_rows`.

- [ ] **Step 3: Implement**

Add to `impl Db` in `src/db/mod.rs`, after `transaction`:

```rust
    /// Whether this connection has inserted, updated or deleted a row. It
    /// counts this run only, so a change made by another process leaves it
    /// `false`.
    pub fn wrote_rows(&self) -> bool {
        self.conn.total_changes() > 0
    }
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test db::tests`
Expected: all pass. If `reopening_…` fails, the migration is writing a row on every open; stop and report it rather than weakening the test.

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add src/db/mod.rs
git commit -m "feat(db): report whether this run wrote any rows"
```

---

### Task 3: Snapshot and the page

**Files:**
- Create: `src/report/mod.rs` (`Snapshot` only in this task)
- Create: `src/report/html.rs`
- Create: `src/report/fixture.rs` (test-only helpers)
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes:
  - `Db::fields() -> Result<Vec<Field>>`
  - `Db::paychecks() -> Result<Vec<Paycheck>>`
  - `calc::sheet(year: i32, &[Field], &[Paycheck]) -> Sheet`
  - `calc::show(Option<Percent>) -> String`
  - `Sheet { year, columns: Vec<Column{date}>, rows: Vec<AmountRow{name, cells: Vec<Option<Cents>>, ytd}>, net: Vec<Cents>, net_ytd, percent_rows: Vec<PercentRow{label, cells, ytd}> }`
  - test-only `Db::field_id(&str) -> FieldId`
- Produces:
  - `report::Snapshot { pub sheets: Vec<Sheet>, pub today: NaiveDate, pub generated_at: DateTime<Local> }`
  - `Snapshot::load(&Db, NaiveDate, DateTime<Local>) -> Result<Snapshot>`
  - `report::html::page(&Snapshot) -> String`
  - `pub(super) fn html::tab_id(year: i32) -> String`, which returns `"y2026"` for 2026
  - test-only in `report::fixture`:
    - `day(y, m, d) -> NaiveDate`
    - `with_checks(&[NaiveDate]) -> Db`
    - `snapshot(&Db, today: NaiveDate) -> Snapshot`

- [ ] **Step 1: Write the fixture**

`src/report/fixture.rs`:

```rust
//! Helpers shared by the report's tests. Every figure is invented; see
//! `AGENTS.md`.

use super::Snapshot;
use crate::db::{self, Db};
use crate::money::Cents;
use chrono::{Local, NaiveDate, TimeZone};

pub(super) fn day(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

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
    let generated_at = Local.with_ymd_and_hms(2026, 1, 16, 9, 30, 0).single().unwrap();
    Snapshot::load(db, today, generated_at).unwrap()
}
```

- [ ] **Step 2: Write `src/report/mod.rs` with `Snapshot` and its tests**

```rust
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
```

In `src/lib.rs`:

```rust
pub mod calc;
pub mod config;
pub mod db;
pub mod money;
pub mod report;
pub mod tui;
```

- [ ] **Step 3: Write `src/report/html.rs` with its tests and a stub `page`**

```rust
//! The report as one page.
//!
//! Self-contained by requirement: inline CSS, no script, no font, no request
//! of any kind. A phone opening this out of a sync folder may be offline, and
//! anything fetched would render half-drawn or not at all. That is also why
//! the year tabs are radio buttons and sibling selectors: a click handler
//! would be the one thing on the page that could fail to arrive.

use super::Snapshot;
use crate::calc::{self, Sheet};
use chrono::Datelike;

pub fn page(snapshot: &Snapshot) -> String {
    let _ = snapshot;
    todo!()
}

#[cfg(test)]
mod tests {
    use super::super::fixture::{day, snapshot, with_checks};
    use super::*;
    use crate::db::Kind;

    #[test]
    fn the_page_makes_no_external_request_and_carries_no_script() {
        let db = with_checks(&[day(2026, 1, 2)]);
        let page = page(&snapshot(&db, day(2026, 1, 2)));
        assert!(!page.contains("http"), "the page reaches out of itself");
        assert!(!page.contains("<script"), "the page carries script");
    }

    #[test]
    fn the_page_has_one_tab_per_year_newest_first() {
        let db = with_checks(&[day(2025, 12, 19), day(2026, 1, 2)]);
        let page = page(&snapshot(&db, day(2026, 1, 2)));
        let newer = page.find("<label for=\"y2026\">2026</label>").expect("no 2026 tab");
        let older = page.find("<label for=\"y2025\">2025</label>").expect("no 2025 tab");
        assert!(newer < older, "the years are not newest first");
        for year in [2025, 2026] {
            assert!(page.contains(&format!("id=\"y{year}-panel\"")), "no {year} panel");
            assert!(
                page.contains(&format!("#y{year}:checked~#y{year}-panel{{display:block}}")),
                "no {year} switch"
            );
        }
    }

    #[test]
    fn the_page_opens_on_the_year_today_falls_in() {
        let db = with_checks(&[day(2025, 12, 19), day(2026, 1, 2)]);
        let page = page(&snapshot(&db, day(2025, 12, 31)));
        assert!(page.contains("id=\"y2025\" checked"), "{page}");
        assert!(!page.contains("id=\"y2026\" checked"), "{page}");
    }

    #[test]
    fn the_page_opens_on_the_newest_year_when_this_year_has_no_paychecks() {
        let db = with_checks(&[day(2025, 12, 19), day(2026, 1, 2)]);
        let page = page(&snapshot(&db, day(2027, 3, 1)));
        assert!(page.contains("id=\"y2026\" checked"), "{page}");
    }

    #[test]
    fn a_field_name_carrying_markup_is_escaped() {
        let db = with_checks(&[day(2026, 1, 2)]);
        db.update_field(db.field_id("Salary"), "<b>Pay & \"Co\"</b>", Kind::Income)
            .unwrap();
        let page = page(&snapshot(&db, day(2026, 1, 2)));
        assert!(page.contains("&lt;b&gt;Pay &amp; &quot;Co&quot;&lt;/b&gt;"), "{page}");
        assert!(!page.contains("<b>Pay"), "the name reached the page as markup");
    }

    #[test]
    fn an_empty_database_draws_the_no_paychecks_panel() {
        let page = page(&snapshot(&with_checks(&[]), day(2026, 1, 2)));
        assert!(page.contains("No paychecks yet."), "{page}");
        assert!(!page.contains("<nav>"), "an empty page drew a tab bar");
    }

    #[test]
    fn a_years_grid_carries_every_paycheck_date_its_ytd_and_its_net() {
        let db = with_checks(&[day(2026, 1, 2), day(2026, 1, 16)]);
        let page = page(&snapshot(&db, day(2026, 1, 16)));
        assert!(page.contains("<th class=\"n d\">01-02</th><th class=\"n d\">01-16</th><th class=\"n\">YTD</th>"), "{page}");
        assert!(page.contains("<tr><th>Salary</th><td class=\"n\">4,000.00</td><td class=\"n\">4,000.00</td><td class=\"n\">8,000.00</td></tr>"), "{page}");
        assert!(page.contains("<tr class=\"net\"><th>Net</th><td class=\"n\">3,400.00</td><td class=\"n\">3,400.00</td><td class=\"n\">6,800.00</td></tr>"), "{page}");
        assert!(page.contains("<tr><th>Federal Tax</th><td class=\"n\">15.00%</td><td class=\"n\">15.00%</td><td class=\"n\">15.00%</td></tr>"), "{page}");
        assert!(page.contains("<tr><th>Net Pay</th><td class=\"n\">85.00%</td><td class=\"n\">85.00%</td><td class=\"n\">85.00%</td></tr>"), "{page}");
    }

    #[test]
    fn an_amount_a_paycheck_does_not_have_is_an_empty_cell() {
        let db = with_checks(&[day(2026, 1, 2), day(2026, 1, 16)]);
        let page = page(&snapshot(&db, day(2026, 1, 16)));
        assert!(page.contains("<tr><th>Medicare</th><td class=\"n\"></td><td class=\"n\"></td><td class=\"n\">0.00</td></tr>"), "{page}");
    }

    #[test]
    fn the_footer_says_when_the_page_was_written() {
        let page = page(&snapshot(&with_checks(&[]), day(2026, 1, 16)));
        assert!(page.contains("Written 2026-01-16 09:30"), "{page}");
    }
}
```

- [ ] **Step 4: Run the tests to see them fail**

Run: `cargo test report::`
Expected: the two `Snapshot` tests pass; every `html` test panics with `not yet implemented`.

- [ ] **Step 5: Implement the page**

Replace the stub `page` in `src/report/html.rs` with everything below, keeping the `use` lines and the tests:

```rust
/// Every interpolation of user-typed text goes through here: a field named
/// with an angle bracket would otherwise truncate the page at its own row.
/// Escapes `&`, `<`, `>` and `"`, never `'`, because no attribute on this
/// page is single-quoted.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

/// A year's radio id. The `y` is there because a CSS id selector cannot
/// start with a digit, and `#2026` would match nothing.
pub(super) fn tab_id(year: i32) -> String {
    format!("y{year}")
}

/// Today's year when it has paychecks, and otherwise the newest year: a page
/// opened in the first days of January should not open on an empty tab.
fn opening_year(snapshot: &Snapshot) -> Option<i32> {
    let this_year = snapshot.today.year();
    let years = || snapshot.sheets.iter().map(|s| s.year);
    years().find(|&y| y == this_year).or_else(|| years().next())
}

/// The tab bar, and the radios that drive it. Every radio sits ahead of the
/// nav and every panel, because `:checked ~` only looks forward.
fn tab_bar(snapshot: &Snapshot) -> String {
    let open = opening_year(snapshot);
    let inputs: String = snapshot
        .sheets
        .iter()
        .map(|s| {
            let checked = if Some(s.year) == open { " checked" } else { "" };
            format!(
                "<input class=\"tab\" type=\"radio\" name=\"tab\" id=\"{}\"{checked}>",
                tab_id(s.year)
            )
        })
        .collect();
    let labels: String = snapshot
        .sheets
        .iter()
        .map(|s| format!("<label for=\"{}\">{}</label>", tab_id(s.year), s.year))
        .collect();
    format!("{inputs}<nav>{labels}</nav>")
}

/// Which panel shows, which label is lit, and where the focus ring goes: one
/// rule set per year, generated from the same list as the markup.
fn tab_rules(snapshot: &Snapshot) -> String {
    snapshot
        .sheets
        .iter()
        .map(|s| {
            let id = tab_id(s.year);
            format!(
                "#{id}:checked~nav label[for={id}]\
                 {{color:inherit;border-bottom-color:currentColor}}\
                 #{id}:focus-visible~nav label[for={id}]\
                 {{outline:2px solid currentColor;outline-offset:-2px}}\
                 #{id}:checked~#{id}-panel{{display:block}}"
            )
        })
        .collect()
}

/// One row of the grid: a label, a cell per paycheck, and the YTD cell. Only
/// the label is escaped, because the cells are figures this crate formatted.
fn grid_row(
    class: &str,
    label: &str,
    cells: impl IntoIterator<Item = String>,
    ytd: String,
) -> String {
    let cells: String = cells
        .into_iter()
        .map(|c| format!("<td class=\"n\">{c}</td>"))
        .collect();
    let class = if class.is_empty() {
        String::new()
    } else {
        format!(" class=\"{class}\"")
    };
    format!(
        "<tr{class}><th>{}</th>{cells}<td class=\"n\">{ytd}</td></tr>",
        escape(label)
    )
}

/// A year's Sheet, laid out as `tui::sheet` draws it: paychecks across, the
/// amounts, a rule, `Net`, a gap, then the percentages.
fn grid(sheet: &Sheet) -> String {
    let dates: String = sheet
        .columns
        .iter()
        .map(|c| format!("<th class=\"n d\">{}</th>", c.date.format("%m-%d")))
        .collect();
    let mut body = String::new();
    for r in &sheet.rows {
        let cells = r.cells.iter().map(|c| c.map_or_else(String::new, |c| c.to_string()));
        body.push_str(&grid_row("", &r.name, cells, r.ytd.to_string()));
    }
    let net = sheet.net.iter().map(ToString::to_string);
    body.push_str(&grid_row("net", "Net", net, sheet.net_ytd.to_string()));
    body.push_str(&format!(
        "<tr class=\"gap\"><td colspan=\"{}\"></td></tr>",
        sheet.columns.len() + 2
    ));
    for r in &sheet.percent_rows {
        let cells = r.cells.iter().map(|p| calc::show(*p));
        body.push_str(&grid_row("", &r.label, cells, calc::show(r.ytd)));
    }
    format!(
        "<table><thead><tr><th></th>{dates}<th class=\"n\">YTD</th></tr></thead>\
         <tbody>{body}</tbody></table>"
    )
}

/// A system font stack, tabular figures, the tab switch, and one media query
/// for dark mode.
///
/// A year is up to ~26 paycheck columns, far wider than a phone, so the panel
/// scrolls sideways and the label column is sticky with an opaque background
/// so the figures slide under it. `border-collapse:separate` because a sticky
/// cell's borders do not travel with it under `collapse`. `n` and `d` never
/// wrap: a comma or a hyphen is a break opportunity a narrow column would
/// take, drawing `4,` over `000.00`.
///
/// The radios are moved off the page rather than `display:none`d, which would
/// take them out of the focus order.
const STYLE: &str = "\
    :root{color-scheme:light dark;--bg:#ffffff;--fg:#1a1a1a;--rule:#dddddd;--muted:#666666}\
    @media (prefers-color-scheme: dark){\
    :root{--bg:#121212;--fg:#eeeeee;--rule:#333333;--muted:#999999}}\
    body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;\
    margin:0;padding:1rem;background:var(--bg);color:var(--fg)}\
    table{border-collapse:separate;border-spacing:0;font-size:0.82rem}\
    td,th{padding:0.25rem 0.5rem;border-bottom:1px solid var(--rule);text-align:left}\
    th{font-weight:600}\
    .n{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}\
    .d{white-space:nowrap}\
    tr>th:first-child{position:sticky;left:0;background:var(--bg);white-space:nowrap}\
    tr.net th,tr.net td{border-top:2px solid var(--fg)}\
    tr.gap td{border-bottom:none;padding:0.5rem}\
    input.tab{position:absolute;opacity:0;width:0;height:0}\
    nav{display:flex;flex-wrap:wrap;border-bottom:1px solid var(--rule);margin-bottom:0.8rem}\
    nav label{padding:0.5rem 0.7rem;margin-bottom:-1px;cursor:pointer;font-weight:600;\
    color:var(--muted);border-bottom:2px solid transparent}\
    section.panel{display:none;overflow-x:auto}\
    footer{border-top:1px solid var(--rule);margin-top:1.5rem;padding-top:0.6rem}\
    p.stamp{color:var(--muted);margin:0;font-size:0.85rem}";

const STAMP_FORMAT: &str = "%Y-%m-%d %H:%M";

pub fn page(snapshot: &Snapshot) -> String {
    let body = if snapshot.sheets.is_empty() {
        "<p>No paychecks yet.</p>".to_string()
    } else {
        let panels: String = snapshot
            .sheets
            .iter()
            .map(|s| {
                format!(
                    "<section class=\"panel\" id=\"{}-panel\">{}</section>",
                    tab_id(s.year),
                    grid(s)
                )
            })
            .collect();
        format!("{}{panels}", tab_bar(snapshot))
    };
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\"><head>\
         <meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>Paychecks</title><style>{STYLE}{}</style></head><body>\
         {body}\
         <footer><p class=\"stamp\">Written {}</p></footer>\
         </body></html>",
        tab_rules(snapshot),
        snapshot.generated_at.format(STAMP_FORMAT),
    )
}
```

- [ ] **Step 6: Run the tests to see them pass**

Run: `cargo test report::`
Expected: all 11 pass. If an exact-markup assertion fails, fix the renderer to emit that markup rather than loosening the assertion, unless the assertion contradicts the spec.

- [ ] **Step 7: Look at it**

Add this test temporarily, run it with `cargo test scratch_render -- --nocapture`, open the file it prints in a browser, then delete the test:

```rust
    #[test]
    fn scratch_render() {
        let db = with_checks(&[day(2025, 12, 19), day(2026, 1, 2), day(2026, 1, 16)]);
        let path = std::env::temp_dir().join("paychecks-preview.html");
        std::fs::write(&path, page(&snapshot(&db, day(2026, 1, 16)))).unwrap();
        println!("{}", path.display());
    }
```

Check that:
- the tabs switch
- the label column stays put while the grid scrolls sideways in a narrow window
- dark mode reads

**Remove the scratch test before committing.**

- [ ] **Step 8: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add src/lib.rs src/report/
git commit -m "feat(report): render every year's Sheet as one self-contained page"
```

---

### Task 4: Writing the page to disk, and the quit-path gate

**Files:**
- Modify: `src/report/mod.rs`
- Modify: `src/config.rs` (a test-only constructor)

**Interfaces:**
- Consumes:
  - `html::page`
  - `html::tab_id`
  - `Snapshot::load`
  - `config::Config` / `Report::dir()`
  - `Db::wrote_rows()`
  - `fixture::{day, with_checks}`
- Produces:
  - `#[cfg(test)] pub(crate) fn Config::reporting_to(dir: &Path) -> Config` in `src/config.rs`
  - `pub const FILE_NAME: &str = "Paychecks.html"`
  - `pub struct Written { pub path: PathBuf, pub bytes: u64 }`
  - `pub enum Outcome { Disabled, Unchanged, Written(Written) }`
  - `pub fn is_due(Option<NaiveDate>, NaiveDate, bool) -> bool`
  - `pub fn write(&Db, &Path, NaiveDate) -> Result<Written>`
  - `pub fn write_if_enabled(&Db, &Config, NaiveDate) -> Result<Outcome>`

- [ ] **Step 1: Write the failing tests**

`Report`'s `dir` is private, so add a test-only constructor to `src/config.rs`, after `impl Report`:

```rust
impl Config {
    #[cfg(test)]
    pub(crate) fn reporting_to(dir: &Path) -> Config {
        Config {
            report: Some(Report {
                dir: dir.display().to_string(),
            }),
        }
    }
}
```

Append inside `mod tests` in `src/report/mod.rs`, and extend its `use` lines:

```rust
    use super::*;
    use crate::config::Config;
    use crate::db;
    use std::path::PathBuf;

    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "paychecker_report_{label}_{}",
            std::process::id()
        ));
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
        assert_eq!(written.bytes, std::fs::metadata(&written.path).unwrap().len());
        assert!(is_the_page(&std::fs::read_to_string(&written.path).unwrap()));
        let names: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
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
        let names: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, [FILE_NAME]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn no_report_section_writes_nothing() {
        let outcome = write_if_enabled(&with_checks(&[]), &Config::default(), day(2026, 1, 2)).unwrap();
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
        let outcome = write_if_enabled(&db, &Config::reporting_to(&dir), today).unwrap();
        assert!(matches!(outcome, Outcome::Unchanged), "{outcome:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_quit_that_wrote_rows_rewrites_the_page() {
        let dir = scratch("rewrite");
        let db = with_checks(&[day(2026, 1, 2)]);
        let today = Local::now().date_naive();
        write(&db, &dir, today).unwrap();
        let outcome = write_if_enabled(&db, &Config::reporting_to(&dir), today).unwrap();
        assert!(matches!(outcome, Outcome::Written(_)), "{outcome:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Every control is a radio and a sibling selector, so a minifier that
    /// dropped an id or reordered an input past its panel would leave a page
    /// that renders and then does nothing. Quotes come off before matching,
    /// so which attributes the minifier unquotes stays its business.
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
            assert!(unquoted.contains(&format!("for={id}>{year}</label>")), "no {year} label: {page}");
            assert!(unquoted.contains(&format!("id={id}-panel")), "no {year} panel: {page}");
            assert!(
                page.contains(&format!("#{id}:checked~#{id}-panel{{display:block}}")),
                "no {year} switch: {page}"
            );
        }
        assert!(unquoted.contains("id=y2026 checked"), "the page does not open on 2026: {page}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
```

Change the existing `use super::fixture::{day, snapshot, with_checks};` in this `mod tests` so that it stays alongside the new `use` lines.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test report::tests`
Expected: compile errors, because `is_due`, `write`, `FILE_NAME`, `Outcome` and `write_if_enabled` do not exist yet.

- [ ] **Step 3: Implement**

In `src/report/mod.rs`, extend the imports:

```rust
use crate::calc::{self, Sheet};
use crate::config::Config;
use crate::db::Db;
use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, Local, NaiveDate};
use minify_html::Cfg;
use std::path::{Path, PathBuf};
```

Add after `impl Snapshot`:

```rust
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
pub fn write_if_enabled(db: &Db, cfg: &Config, today: NaiveDate) -> Result<Outcome> {
    let Some(report) = cfg.report.as_ref() else {
        return Ok(Outcome::Disabled);
    };
    let dir = report.dir()?;
    if !is_due(written_on(&dir.join(FILE_NAME)), today, db.wrote_rows()) {
        return Ok(Outcome::Unchanged);
    }
    Ok(Outcome::Written(write(db, &dir, today)?))
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test report::`
Expected: all pass. If `minification_leaves_every_tab_and_its_switch_intact` fails, read the minified page in the failure message. Adjust the assertion only if the minifier respelled a selector equivalently, for example by reordering; if it broke the switch, fix the markup.

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add src/report/mod.rs src/config.rs
git commit -m "feat(report): write the page atomically, skipping a quit that changed nothing"
```

---

### Task 5: Wire the CLI and the quit path, and document it

**Files:**
- Modify: `src/tui/app.rs` (add `into_db`)
- Modify: `src/tui/mod.rs:24-32` (`run` returns the `Db`)
- Modify: `src/bin/pc.rs`
- Modify: `README.md`, `AGENTS.md`
- Modify: `docs/superpowers/specs/2026-09-29-html-report-design.md` (the `--config` flag)

**Interfaces:**
- Consumes:
  - `config::{default_path, load}`
  - `Report::dir`
  - `report::{write, write_if_enabled, Written}`
- Produces:
  - `tui::run(db: Db, today: NaiveDate) -> Result<Db>`
  - `pc [--db] [--today] [--config]`
  - `pc report [--dir DIR]`

- [ ] **Step 1: Hand the database back from the TUI**

In `src/tui/app.rs`, add to `impl App` after `should_quit`:

```rust
    pub(super) fn into_db(self) -> Db {
        self.db
    }
```

In `src/tui/mod.rs`, replace `run`:

```rust
/// Runs the screens until the user quits, then hands the database back so
/// the quit path can read what this run wrote.
pub fn run(db: Db, today: NaiveDate) -> Result<Db> {
    let mut app = App::new(db, today)?;
    // `try_init` enables raw mode, enters the alternate screen, and installs a
    // panic hook that restores the terminal before unwinding.
    let mut terminal = ratatui::try_init()?;
    let result = event_loop(&mut terminal, &mut app);
    ratatui::try_restore()?;
    result?;
    Ok(app.into_db())
}
```

- [ ] **Step 2: Rewrite `src/bin/pc.rs`**

```rust
use anyhow::{Context, Result};
use chrono::{Local, NaiveDate};
use clap::{Parser, Subcommand};
use paychecker::{config, db, report, tui};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "pc",
    about = "Paychecker: record paychecks and see where each one goes"
)]
struct Cli {
    /// Database file. Defaults to ~/.local/share/paychecker/paychecks.db
    #[arg(long, global = true)]
    db: Option<PathBuf>,
    /// Treat this date as today. Defaults to the local date.
    #[arg(long, global = true)]
    today: Option<NaiveDate>,
    /// Config file. Defaults to ~/.config/paychecker/config.toml
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Write the HTML report without opening the application.
    Report {
        /// Directory to write Paychecks.html into. Defaults to the config
        /// file's [report] dir, which this makes optional.
        #[arg(long)]
        dir: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config_path = match cli.config {
        Some(path) => path,
        None => config::default_path()?,
    };
    // Before the TUI opens: a config that does not parse should say so on a
    // terminal in its normal mode, not after a session's work.
    let cfg = config::load(&config_path)?;
    let path = match cli.db {
        Some(path) => path,
        None => db::default_path()?,
    };
    let db = db::open(&path)?;
    let today = cli.today.unwrap_or_else(|| Local::now().date_naive());

    match cli.command {
        None => {
            let db = tui::run(db, today)?;
            // The session's work is already saved, so a report that cannot be
            // written is a warning, not a failed run.
            if let Err(e) = report::write_if_enabled(&db, &cfg, today) {
                eprintln!("report: {e:#}");
            }
        }
        Some(Command::Report { dir }) => {
            // An unset [report] section means "not on every quit", which is
            // a different question from the one `pc report` asks.
            let dir = match dir {
                Some(dir) => dir,
                None => cfg
                    .report
                    .as_ref()
                    .with_context(|| {
                        format!(
                            "no --dir given, and no [report] section naming one in {}",
                            config_path.display()
                        )
                    })?
                    .dir()?,
            };
            let written = report::write(&db, &dir, today)?;
            println!("wrote {} ({} bytes)", written.path.display(), written.bytes);
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Build and exercise the subcommand**

Use a scratch directory: `$S`, the session scratchpad, or `mktemp -d`.

```bash
cargo build
S=$(mktemp -d)
./target/debug/pc --db "$S/t.db" --config "$S/none.toml" report
```
Expected: error `no --dir given, and no [report] section naming one in …/none.toml`, non-zero exit.

```bash
./target/debug/pc --db "$S/t.db" report --dir "$S/out"
ls -a "$S/out"
```
Expected: `wrote …/out/Paychecks.html (N bytes)`; the listing holds only `.`, `..` and `Paychecks.html`.

```bash
printf '[report\n' > "$S/bad.toml"
./target/debug/pc --db "$S/t.db" --config "$S/bad.toml"
```
Expected: exits immediately with `parsing …/bad.toml` and never opens the TUI.

- [ ] **Step 4: Update the docs**

In `README.md`, add to the `## Usage` code block, after the `pc --db` line:

```bash
pc report               # write the HTML report without opening the application
```

Then add a new section before `## Development`:

````markdown
## Report

Off until a config file switches it on:

```toml
# ~/.config/paychecker/config.toml
[report]
dir = "~/Dropbox/pay"   # required
```

When `pc` quits, it writes a self-contained HTML page of the Sheet to `<dir>/Paychecks.html`,
one tab per year. The page opens on the current year, or the newest year that has paychecks.
It carries no script and loads nothing, so it reads on a phone offline. Pointing `dir` at a
synced folder puts it there.

The write happens after the screen is torn down. A failure prints to stderr and does not fail
the run. The file is written beside its name and renamed into place, so a sync client never
uploads half a page.

A quit that changed nothing leaves the page alone if it was already written that day. That check
sees only this run's writes and the file's timestamp, so a `--today` run or a session held open
across midnight can leave a stale page until the next quit that changes something.

```bash
pc report                    # into the configured dir, never skipped
pc report --dir /tmp/export  # anywhere, config or no config
```
````

In `AGENTS.md`, add a paragraph at the end of `## Design`:

```markdown
`src/report/` writes the Sheet as an HTML page on quit (see README). The page carries **no
script** and is read offline on a phone. So every control is CSS (the year tabs are radios and
`:checked ~` rules generated from the same list as the markup). The file is renamed onto its name,
never written to it. It is minified in `report::write`, not in `html::page`, whose readable
output is what the tests assert against. `minify_html` is named only in `src/report/mod.rs`, and
`serde`/`toml` only in `src/config.rs`.
```

In the spec's `## CLI` section, change the `--db`/`--today` bullet to:

```markdown
- `--db`, `--today` and `--config` (the config file path, for testing and scratch runs) are global
  and apply to both.
```

- [ ] **Step 5: Full verification**

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```
Expected: all pass, no warnings.

- [ ] **Step 6: Commit**

```bash
git add src/tui/app.rs src/tui/mod.rs src/bin/pc.rs README.md AGENTS.md docs/superpowers/specs/2026-09-29-html-report-design.md
git commit -m "feat: write the HTML report on quit and add pc report"
```

- [ ] **Step 7: Manual check of the quit path (human)**

The TUI needs a terminal, so the owner runs this step:

```bash
printf '[report]\ndir = "%s/out"\n' "$S" > "$S/cfg.toml"
./target/debug/pc --db "$S/t.db" --config "$S/cfg.toml"   # add a paycheck, press q
ls -l "$S/out"; open "$S/out/Paychecks.html"
```
Expected: the page is written on quit and shows the new paycheck. Quitting again at once without changes leaves the mtime unchanged.
