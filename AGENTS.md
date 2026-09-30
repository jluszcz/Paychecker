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
ratatui, rusqlite). Pieces such as `TextBuffer`/`edit_key` and the migration pattern are copied
in and trimmed rather than shared through a common crate.

`src/report/` writes the Sheet as an HTML page on quit (see README). The page carries **no
script** and is read offline on a phone. So every control is CSS (the year tabs are radios and
`:checked ~` rules generated from the same list as the markup). The file is renamed onto its name,
never written to it. It is minified in `report::write`, not in `html::page`, whose readable
output is what the tests assert against. `minify_html` is named only in `src/report/mod.rs`, and
`serde`/`toml` only in `src/config.rs` and `src/backup/state.rs`.

`src/backup/` copies the database to S3 (see README), ported from MisterManager's. `aws_config`,
`aws_sdk_s3`, `aws_smithy_types` and `tokio` are named only in `s3.rs`, whose runtime lives for one
upload. `db::snapshot` makes the copy so `rusqlite` stays in `src/db/`. The invariants:

- The IAM user in `paychecker.tf` may only `PutObject`, and only with `If-None-Match: *`, which
  `s3::upload` sends. The key is long-lived and unattended, so the policy bounds it: it can add a
  backup but never replace one. Restores use the owner's own identity.
- The bucket is the application's own and its name is composed from the account and region, which
  is what keeps it out of the repository and lets the lifecycle rules cover the whole bucket.
- No key prefix: `backup::key_for` and the IAM policy's `<bucket arn>/*` would otherwise have to
  spell it identically, with `AccessDenied` as the only sign they drifted.
- The schedule reads `Utc::now()`, never `--today`, and the scheduled check runs only on the
  default database. `pc backup` is exempt from the second rule.
- The state file is advisory: unreadable means a warning and one redundant upload. It is written
  only after a successful upload, and the temp snapshot is removed on both paths.
- `interval_days` is clamped to ten years before it reaches `TimeDelta::days`, which panics
  outside chrono's calendar.

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
