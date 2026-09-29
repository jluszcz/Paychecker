# Paychecker — Design

A terminal app for recording paychecks and seeing where each one goes: per-paycheck and
year-to-date amounts, net pay, and each line as a percentage of income. It replaces a personal
spreadsheet and borrows its stack and conventions from the sibling project MisterManager
(Rust, ratatui, rusqlite), though it is much smaller.

## Goals

- Enter a paycheck quickly. The date and amounts are prefilled from the previous paycheck, so
  usually only the changed values need typing.
- See a year's paychecks laid out like the spreadsheet: fields as rows, paychecks as columns,
  a YTD total column, net pay, and percentages.
- Maintain the set and order of fields. Fields can be added, renamed, reordered, re-kinded, and
  archived without disturbing past paychecks.

## Non-goals (v1)

- Graphs or other analysis screens.
- Importing the existing spreadsheet. Data is entered by hand.
- Multiple concurrent paycheck streams. There is one stream, and job changes are modeled by
  archiving old fields and adding new ones.
- Sharing code with MisterManager through a common crate. The pieces needed are copied in and
  trimmed; extraction is deferred.

## Data

No real figures, employers, or names appear in any tracked file (source, fixtures, docs, commit
messages). Test fixtures use invented amounts.

### Storage

SQLite via rusqlite (`bundled`), opened with `foreign_keys=ON` and `journal_mode=WAL`. The default
path is `~/.local/share/paychecker/paychecks.db`; the parent directory is created if missing.
The schema is versioned with `PRAGMA user_version`, following MisterManager's migration pattern:
a frozen `schema.sql` is version 1, and later versions append to a `MIGRATIONS` list applied in
one transaction. A database newer than the build refuses to open.

### Schema

```sql
CREATE TABLE field (
    id       INTEGER PRIMARY KEY,
    name     TEXT    NOT NULL UNIQUE,
    kind     TEXT    NOT NULL CHECK (kind IN ('income', 'deduction')),
    position INTEGER NOT NULL,
    archived INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1))
);

CREATE TABLE paycheck (
    id   INTEGER PRIMARY KEY,
    date TEXT    NOT NULL UNIQUE  -- ISO YYYY-MM-DD
);

CREATE TABLE amount (
    paycheck_id INTEGER NOT NULL REFERENCES paycheck(id) ON DELETE CASCADE,
    field_id    INTEGER NOT NULL REFERENCES field(id),
    cents       INTEGER NOT NULL,
    PRIMARY KEY (paycheck_id, field_id)
);
```

`position` values are unique in practice and are renumbered densely when a field moves.
`amount.field_id` has no cascade: a field that any paycheck uses can't be deleted.

### Seed

When the database is created, it is seeded with these fields in this order: Salary (income),
Federal Tax, Social Security, Medicare, State Tax, Family Leave, Medical Leave, HSA,
401K (Trad), 401K (Roth) (all deductions).

### Semantics

- **Money** is `Cents(i64)`, displayed as `-1,234.56`. Amounts may be negative, for corrections.
- **Saving a paycheck** writes one `amount` row for every field shown in its form. A blank amount
  is saved as 0.
- **Net** = Σ income amounts − Σ deduction amounts.
- **Percent** for a field = field amount / Σ income. Net Pay % = Net / Σ income. When Σ income is 0,
  the percentage displays as `—`. Percentages show two decimals (`27.38%`).
- **YTD** for a year applies the same math to the sums over every paycheck dated in that
  calendar year: the YTD % is the ratio of the sums, not an average of per-paycheck percentages.
- **Row visibility** for a year: a field has a row if it is active, or if any paycheck in that
  year has an amount for it. Archived fields therefore appear only in years they applied to.
- **Missing amounts.** A paycheck with no amount for a visible field shows a blank cell there, and
  counts it as 0 in its percentages and in YTD.

## Architecture

One crate, `paychecker`, with a binary `pc`.

| Module       | Responsibility                                                                   |
|--------------|----------------------------------------------------------------------------------|
| `money`      | `Cents`: parse (`$`, `,`, `_`, spaces stripped; leading `-`; ≤ 2 decimals), display with grouping |
| `db`         | open/prepare, migrations, seed; `field` and `paycheck` queries                   |
| `calc`       | Pure functions over a year's fields and paychecks: per-column totals, net, percentages, YTD, and which rows are visible |
| `tui::text`  | `TextBuffer` and `edit_key`, copied from MisterManager                           |
| `tui::form`  | Form fields (text, date, amount, selector), Tab order, date stepping and parsing |
| `tui::sheet` | Sheet screen state and rendering                                                 |
| `tui::fields`| Fields screen state and rendering                                                |
| `tui::help`  | Help panel topics; footers are built from the same tables                        |
| `tui::app`   | `App`: screen switching, modal, status line, key dispatch                        |

`calc` knows nothing about SQLite or ratatui. It takes fields and paychecks as plain values,
which makes it the most heavily unit-tested unit.

### CLI

`pc [--db <path>] [--today <YYYY-MM-DD>]` runs the TUI. `--today` exists for tests and demos,
and defaults to the local date. There are no subcommands in v1.

### Event loop

As in MisterManager: `ratatui::try_init`, redraw only when dirty, poll every 250 ms, act only on
`KeyEventKind::Press`, and restore the terminal on exit.

## Screens and keys

Key conventions follow MisterManager's: the same action uses the same key on every screen that
offers it, `Ctrl`+letter is always text editing, `Esc` backs out of the innermost thing, and
footers come from the help tables.

**Global** (when no modal is open): `1` Sheet, `2` Fields, `q` quit, `?`/`F1` help.
Errors and confirmations replace the footer on a status line. With no modal open, the status
line expires after 4 s or at the next key press. With a modal open, it stays until the modal
closes.

### Sheet (`1`)

```
2026                01-02     01-16  ◀ 01-30 ▶     02-13 │       YTD
Salary          10,000.00 10,000.00 10,000.00 10,000.00 │ 40,000.00
Federal Tax      2,000.00  2,000.00  2,000.00  2,000.00 │  8,000.00
...
────────────────────────────────────────────────────────┼──────────
Net              6,500.00  6,500.00  6,500.00  6,500.00 │ 26,000.00

Federal Tax        20.00%    20.00%    20.00%    20.00% │    20.00%
...
Net Pay            65.00%    65.00%    65.00%    65.00% │    65.00%
```

- It opens on the year of the latest paycheck, or the current year if there are none, with the
  year's latest paycheck selected. `[`/`]` step the year and select that year's latest paycheck.
- `←`/`→` move the selected paycheck column, and `Home`/`End` jump to the first or last. The label
  and YTD columns are fixed. The paycheck columns scroll horizontally to keep the selection
  visible.
- Rows, top to bottom:
  1. Income fields, then deduction fields, each group in `position` order.
  2. A rule, then Net.
  3. A blank line, then the percentage block: every deduction field, then Net Pay.
- Archived fields that are visible render the same as active ones.
- A year with no paychecks shows the labels and a hint to press `a`.
- `a` add a paycheck, `e` edit the selected one, `d` delete the selected one (`y` confirms,
  anything else cancels).

### Paycheck form (modal, used for add and edit)

- **Date**, the first field.
  - Add prefills it with the latest paycheck's date + 14 days, or today if there are none.
  - Accepts `YYYY-MM-DD` or MisterManager's `M/D` shorthand.
  - `←`/`→` step a day and Shift+`←`/`→` a week. `[`/`]` step a month, clamping the day.
  - Stepping does nothing while the text is blank or invalid.
  - While focused it shows the raw text. Once focus leaves, it shows the ISO date.
- **Amounts**, one per field, labeled with the field name, in the same order as Sheet rows.
  - Add shows the active fields, prefilled with the latest paycheck's amounts for those fields.
    A field that paycheck lacks, or every field when there is no previous paycheck, starts blank.
  - Edit shows the paycheck's own fields, including archived ones, plus any active fields it
    lacks. They are filled with its amounts, and missing ones start blank.
  - Prefilled text puts the caret at the end. `Ctrl+U` clears the field.
- `Tab`/`Shift-Tab` move through date and amounts, wrapping at either end.
- Text editing is MisterManager's `edit_key`: `Ctrl+A/E` start and end, `Ctrl+B/F` left and
  right, `Ctrl+W` delete the previous word, `Ctrl+U/K` kill to start or end, `Ctrl+D`/`Delete`
  delete forward, `Backspace` delete back. `←`/`→` move the caret in an amount field. Other Ctrl
  and Alt combinations are ignored.
- A live **Net / Net %** line under the fields reflects the current input. It shows `—` while any
  amount fails to parse.
- `Enter` saves and `Esc` cancels.
  - Saving validates the date, every amount, and that no other paycheck has the date.
  - On error, the form stays open and the status line shows the message.
  - On success, the modal closes, the Sheet moves to the saved paycheck's year and selects it,
    and the status line confirms.

### Fields (`2`)

- A list in `position` order showing the name, the kind, and an `archived` tag. Archived fields
  are dimmed here, and only here.
- `↑`/`↓` select. `Shift-↑`/`Shift-↓` move the selected field up or down, which changes both Sheet
  row order (within its kind) and form Tab order.
- `a` adds a field with a modal form: **Name** (text) and **Kind** (a selector cycled with
  `←`/`→`, defaulting to Deduction). New fields go at the end.
- `e` edits the selected field's name or kind with the same form. Renaming keeps every past
  amount. Changing the kind changes how past paychecks' Net is computed.
- `x` archives or unarchives the selected field.
- `d` deletes the selected field, but only if no paycheck has an amount for it. Otherwise the
  status line says to archive it instead. It asks for `y` confirmation like paycheck deletion.
- Names must be non-empty after trimming and unique. Violations go to the status line.

### Help

`?`/`F1` opens a panel listing the current screen's keys (and the form's, when a modal is open).
`Esc`/`?`/`F1` close it.

## Error handling

- Database and I/O errors surface as `anyhow` errors.
  - At startup (open, migrate) they print and exit non-zero.
  - Inside the TUI, an operation's error goes to the status line and the app keeps running.
- Validation errors (bad date, bad amount, duplicate date, duplicate name, deleting a used field)
  go to the status line, and the state is not changed.

## Testing

- **Unit:** `money` parse and display; `calc` totals, net, percentages (including zero income),
  YTD, and row visibility with archived fields; date parsing and stepping; `TextBuffer` editing.
- **DB:** queries and migrations against in-memory SQLite, including the seed, dense
  renumbering on move, the delete guard for used fields, and the unique date constraint.
- **App:** a `test_support` module (as in MisterManager) builds an `App` on an in-memory database
  and drives `on_key`. Tests assert on state and on `TestBackend` renders: the add flow
  (prefilled date and amounts, Tab wrap, save), edit, delete confirmation, year switching, and
  archived-field visibility.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` stay clean.

## Repository

Mirrors MisterManager where it applies:
- Rust edition 2024.
- The same pre-commit hooks, minus Terraform.
- CI through `jluszcz/github-utils` `rust-ci.yml`.
- A sorted `.gitignore` covering `*.db*` and `target/`.
- `README.md` and `AGENTS.md`, which records the no-real-data rule and the key conventions.
