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
