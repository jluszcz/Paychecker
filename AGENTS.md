# AGENTS.md

This file provides guidance to AI coding agents when working with code in this repository.

## Commands

```bash
cargo build
cargo test
cargo fmt                                  # pre-commit hook runs `cargo fmt --check`
cargo clippy --all-targets -- -D warnings  # CI treats warnings as errors
cargo run --bin pc -- --db /tmp/scratch.db --today 2026-01-16
```

## Design

Paychecker borrows its stack and conventions from the sibling project MisterManager (Rust,
ratatui, rusqlite). What the two share lives in `jluszcz_finance_utils` (`../finance-utils`):
money, config paths and sections, the report's minified atomic write, the S3 backup, and TUI text
editing. The migration pattern is still copied in and trimmed.

`src/report/` writes the Sheet as an HTML page on quit (see README). The page carries **no
script** and is read offline on a phone. So every control is CSS (the year tabs are radios and
`:checked ~` rules generated from the same list as the markup). The file is renamed onto its name,
never written to it. `html::page` stays readable, because its tests assert against it;
`finance_utils::report::write` minifies on the way to the disk. `serde` is named only in
`src/config.rs`.

Backups go through `finance_utils::backup` (its AGENTS.md holds the invariants). `lib.rs`'s `BACKUP`
names the app, and `db::snapshot` is the snapshot, which keeps `rusqlite` in `src/db/`. What stays
here: `paychecker.tf`'s IAM policy must allow `PutObject` only with `If-None-Match` present, and the
policy's `<bucket arn>/*` must match the crate's un-prefixed keys. The scheduled check runs only on
the default database, and never after `pc backup`.

## No real data in the repository

The repository is public; the owner's pay is not. **Nothing committed here may carry a real
figure, a real employer, or a name that identifies a real person** — not in source, tests,
fixtures, docs, `README.md`, commit messages, or PR text. Every money literal is invented, and a
fixture copied from a real paystub leaks even when it is only a test input.

## Conventions

- `Cents(i64)` is the only money type. Parse and display go through `money`.
- `calc` is pure: it takes fields and paychecks as plain values and knows nothing about SQLite or
  ratatui.
- Keys: the same action uses the same key on every screen that offers it, `Ctrl`+letter is always
  text editing, `Esc` backs out of the innermost thing, and footers are built from the help tables.

## Testing conventions

Test names are full sentences describing the scenario
(`percent_is_a_dash_when_income_is_zero`, not `test_percent_2`). Unit tests live in `mod tests` at
the bottom of the file under test, and database tests run against in-memory SQLite.
