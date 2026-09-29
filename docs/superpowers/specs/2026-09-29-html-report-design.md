# HTML report on close — design

## Intent

When `pc` quits, write a self-contained HTML page of the Sheet into a configured directory,
so a synced folder carries it to a phone that reads it offline. The machinery is MisterManager's
`src/report/`, copied in and trimmed, the way `TextBuffer` and the migration pattern were.

Paychecker has no `--demo` mode, so the report has no demo skip.

## Config — `src/config.rs`

- Path: `$XDG_CONFIG_HOME/paychecker/config.toml`, or `~/.config/paychecker/config.toml` when
  that variable is unset or empty.
- `load(path) -> Result<Config>`: an absent file is `Config::default()`; a file that is present but
  does not parse is an error naming the path.
- `Config { report: Option<Report> }`, `Report { dir: String }`.
  - No `[report]` section means reports are off.
  - `dir` has no default, so a section with a misspelled key (`directory =`) is a
    missing-field error rather than a silently disabled report. Keys nothing reads are ignored,
    as in MisterManager.
  - `Report::dir() -> Result<PathBuf>` expands a *leading* `~/` against `$HOME`; a `~` elsewhere
    is left alone.

```toml
# ~/.config/paychecker/config.toml
[report]
dir = "~/Dropbox/pay"   # required
```

## CLI — `src/bin/pc.rs`

- `pc` with no subcommand opens the TUI, as today.
- `pc report [--dir DIR]` writes the page without opening the TUI and without the unchanged gate.
  `--dir` overrides the config and makes the `[report]` section optional; with neither, it is an
  error telling the user to add one or pass `--dir`. A failure is an error exit.
- After `tui::run` returns and the terminal is restored, `main` calls
  `report::write_if_enabled`. A failure prints `report: <error>` to stderr and the run still
  exits 0. A written page prints nothing. A run given `--db` or `--today` skips it
  (`Outcome::Skipped`): it is a scratch session, and the configured page is the real database's.
- `--db`, `--today` and `--config` (the config file path, for testing and scratch runs) are global
  and apply to both.

## Database — `Db::wrote_rows`

`pub fn wrote_rows(&self) -> bool { self.conn.total_changes() > 0 }`. It counts this
connection only, which is the documented blind spot of the gate below.

## `src/report/mod.rs`

- `Snapshot { sheets: Vec<calc::Sheet>, today: NaiveDate, generated_at: DateTime<Local> }`.
  `Snapshot::load(db, today, generated_at)` reads the fields and paychecks once and builds
  `calc::sheet(year, …)` for every year that has at least one paycheck, newest first.
- `pub const FILE_NAME: &str = "Paychecks.html";`
- `temp_name()`: `.Paychecks.html.<pid>.tmp`, in the same directory as the target.
- `minify(page) -> Vec<u8>`: `minify_html` with `minify_css: true`. `minify_html` is named only
  in this file.
- `write(db, dir, today) -> Result<Written { path, bytes }>`: this function loads the snapshot,
  renders it, minifies it, runs `create_dir_all`, writes the temp file and renames it onto the
  target. The temp file is removed if either step fails.
- `is_due(last_written: Option<NaiveDate>, today, wrote_rows) -> bool`:
  `wrote_rows || last_written != Some(today)`.
- `written_on(path) -> Option<NaiveDate>`: the local date of the file's mtime.
- `write_if_enabled(db, cfg, today, scratch) -> Result<Outcome>`, where
  `Outcome { Disabled, Skipped, Unchanged, Written(Written) }`:
  - `Disabled` when there is no `[report]` section.
  - `Unchanged` when `!is_due(...)`.
  - `Written` otherwise.

## `src/report/html.rs`

One file: there is one kind of tab, so the per-tab module split MisterManager needs does not pay
here.

- `page(&Snapshot) -> String`, written readably; tests assert against this, not the minified
  bytes.
- **Self-contained:** inline `<style>`, no `<script>`, no external URL of any kind. Light and dark
  through `prefers-color-scheme`. `<meta name="viewport">` for phones.
- **Tabs:** one per year in `Snapshot::sheets`. Each is a visually hidden radio (moved
  off-screen, not `display:none`) plus a `<label>`, placed ahead of the panels, with one
  generated `#yYYYY:checked ~ .panels #pYYYY { display:block }` rule per year built from the
  same list as the markup. The checked radio is `today`'s year when it has paychecks, and the
  newest year otherwise.
- **A year's panel is the Sheet grid, mirroring `tui::sheet`:**
  - a header row of the paycheck dates as `%m-%d` plus `YTD` (the year is the tab)
  - one row per `AmountRow`, then a rule, then `Net`
  - a blank spacer row, then one row per `PercentRow`
  - labels bold; the label column `position:sticky; left:0` with an opaque background
  - the panel carries `overflow-x:auto`, so the grid scrolls sideways inside it
  - cell classes `n` (figure) and `d` (date) with `white-space:nowrap`
  - an absent amount is an empty cell, as on the screen
  - money via `Cents`' `Display`; percents via `calc::show`
- **Escaping:** `escape()` (`&`, `<`, `>`, `"`) on every field name, since it is user text.
- **Empty database:** no tabs, and one panel reading "No paychecks yet."
- **Footer:** "Written YYYY-MM-DD HH:MM" from `generated_at`.

## Testing

All tests use in-memory SQLite and invented figures, and each name is a sentence.

- **`config`:**
  - a section naming only `dir` is complete
  - a misspelled `dir` is an error
  - no section leaves reports off
  - an absent file is the default
  - a leading `~` expands; a mid-path `~` does not
- **`db`:**
  - a fresh open reports no rows written
  - an insert reports one
- **`report`:**
  - `is_due` truth table
  - `write` lands `Paychecks.html`, leaves no temp file, and reports its byte count
  - `write_if_enabled` returns `Disabled` without a section
  - `write_if_enabled` returns `Unchanged` on a same-day run that wrote nothing, and `Written`
    when rows were written
  - minification leaves every year tab and its `:checked` rule intact
- **`report::html`:**
  - the page makes no external request and carries no script
  - one tab per year, newest first
  - opens on today's year, or on the newest when today's has none
  - a field name with `<` is escaped
  - an empty database draws the "No paychecks yet" panel
  - the grid carries every paycheck date and the YTD column

## Docs

- README: a `## Report` section covering the config, write-on-quit, the unchanged gate and its
  blind spots, and `pc report [--dir]`.
- AGENTS.md: a short `Report` note:
  - the page carries no script and is read offline
  - every control is CSS
  - the file is renamed onto its name, never written to it
  - the page is minified in `write`, not in `html`

## Dependencies

`serde` (derive), `toml`, `minify-html`, at the versions MisterManager pins.
