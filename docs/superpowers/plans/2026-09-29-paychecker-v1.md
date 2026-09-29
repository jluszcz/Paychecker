# Paychecker v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `pc` terminal app: record paychecks in SQLite and show a year of them as a spreadsheet-style Sheet with YTD, net, and percentages, plus a Fields screen to maintain the rows.

**Architecture:** One crate, `paychecker`, with a binary `pc`. `money` owns `Cents`; `db` owns SQLite (schema, migrations, seed, queries) and the plain `Field`/`Paycheck`/`Kind` values; `calc` is pure arithmetic over those values; `tui` is ratatui screens driven by `App::on_key`. Pieces are copied from the sibling project `../MisterManager` and trimmed.

**Tech Stack:** Rust 2024, ratatui 0.30 (crossterm backend), rusqlite 0.40 (`bundled`, `chrono`), chrono 0.4, clap 4 (derive), anyhow, thiserror.

**Spec:** `docs/superpowers/specs/2026-09-29-paychecker-design.md` — read it before starting any task.

## Global Constraints

- **No real data.** Nothing committed may carry a real figure, a real employer, or a name that identifies a real person — source, tests, fixtures, docs, commit messages, PR text. Every money literal in this plan is invented; keep it that way.
- `Cents(i64)` is the only money type. Parse and display go through `money`.
- `calc` is pure: no `rusqlite`, no `ratatui` imports.
- Keys: the same action uses the same key on every screen that offers it; `Ctrl`+letter is always text editing; `Esc` backs out of the innermost thing; footers are built from the help tables.
- Test names are full sentences. Unit tests live in `mod tests` at the bottom of the file under test. Database tests run against in-memory SQLite.
- Every task ends with `cargo fmt`, `cargo test`, and `cargo clippy --all-targets -- -D warnings` all clean.
- Comments describe the code as it is (no "previously…", "changed to…"); they carry the non-obvious *why*.
- Work on a feature branch, never `main`. Commit with the `jluszcz:commit` skill; the message given in each task is the one to use.

## Review Focus

1. **Editing a paycheck without changing its date** must save — the duplicate-date check has to exclude the paycheck itself. Pinned in Task 4 and Task 12.
2. **Renaming a field to its own name, or changing only its kind,** must save — the duplicate-name check has to exclude the field itself. Pinned in Task 3.
3. **A terminal too narrow for more than one paycheck column** still draws the Sheet and keeps the selected column visible (and a terminal narrower than the fixed columns does not panic). Pinned in Task 8.
4. **Deleting the last paycheck in a year** leaves an empty year where `e`/`d` do nothing, the hint shows, and nothing panics on an out-of-range selection. Pinned in Task 12.
5. **`M/D` typed in late December for an early-January paycheck** resolves to next year, not eleven months back. Pinned in Task 7.

---

## File Structure

| File | Responsibility |
|---|---|
| `Cargo.toml` | Dependencies |
| `src/lib.rs` | `pub mod money; pub mod db; pub mod calc; pub mod tui;` |
| `src/bin/pc.rs` | CLI (`--db`, `--today`), open the database, run the TUI |
| `src/money.rs` | `Cents`, parse, display |
| `src/db/mod.rs` | `Db`, `open`, `open_in_memory`, `default_path`, `Kind`, `Field`, `Paycheck` |
| `src/db/schema.sql` | Frozen version-1 schema |
| `src/db/seed.sql` | Default fields inserted into a new database |
| `src/db/migration.rs` | `PRAGMA user_version` runner and the (empty) `MIGRATIONS` chain |
| `src/db/field.rs` | Field queries: list, insert, update, archive, move, delete guard |
| `src/db/paycheck.rs` | Paycheck queries: list, insert, update, delete, unique date |
| `src/calc.rs` | `Percent`, `Totals`, row order and visibility, `Sheet` |
| `src/tui/mod.rs` | `run`, event loop, `centered` |
| `src/tui/text.rs` | `TextBuffer`, `edit_key`, `is_bare` (copied from MisterManager) |
| `src/tui/form.rs` | Date parsing and stepping, `PaycheckForm`, `FieldForm`, their rendering |
| `src/tui/sheet.rs` | `SheetView` and Sheet rendering |
| `src/tui/fields.rs` | `FieldsView` and Fields rendering |
| `src/tui/help.rs` | Key tables, footer join, help panel |
| `src/tui/app.rs` | `App`: screens, modal, status line, key dispatch |
| `src/tui/test_support.rs` | `#[cfg(test)]` helpers shared by the `tui` tests |

---

### Task 0: Branch

- [ ] **Step 1: Create the feature branch**

```bash
git switch -c feat/v1
```

---

### Task 1: Dependencies and `money`

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/lib.rs`
- Create: `src/money.rs`

**Interfaces:**
- Produces: `pub struct Cents(pub i64)` with `Cents::ZERO`, `Default`, `Add`, `Sub`, `Neg`, `AddAssign`, `Sum`, `Display` (`-1,234.56`), `FromStr<Err = ParseMoneyError>`.

- [ ] **Step 1: Add dependencies**

Replace the empty `[dependencies]` table in `Cargo.toml`:

```toml
[dependencies]
anyhow = "1.0"
chrono = { version = "0.4", default-features = false, features = ["std", "clock"] }
clap = { version = "4.6", features = ["derive"] }
ratatui = "0.30"
rusqlite = { version = "0.40", features = ["bundled", "chrono"] }
thiserror = "2.0"
```

- [ ] **Step 2: Write the failing tests**

`src/lib.rs`:

```rust
//! Paychecker: record paychecks and see where each one goes.

pub mod money;
```

`src/money.rs` (tests only for now, with an empty type so it compiles to a failure):

```rust
//! `Cents`, the crate's only money type.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_groups_thousands_and_keeps_two_decimals() {
        assert_eq!(Cents(12_345_678).to_string(), "123,456.78");
        assert_eq!(Cents(100_000).to_string(), "1,000.00");
        assert_eq!(Cents(5).to_string(), "0.05");
        assert_eq!(Cents(0).to_string(), "0.00");
    }

    #[test]
    fn display_puts_the_sign_before_the_digits() {
        assert_eq!(Cents(-123_456).to_string(), "-1,234.56");
        assert_eq!(Cents(-50).to_string(), "-0.50");
    }

    #[test]
    fn parsing_strips_dollar_signs_commas_underscores_and_spaces() {
        assert_eq!("$1,234.56".parse::<Cents>().unwrap(), Cents(123_456));
        assert_eq!(" 1_000 ".parse::<Cents>().unwrap(), Cents(100_000));
        assert_eq!("$ 12".parse::<Cents>().unwrap(), Cents(1_200));
    }

    #[test]
    fn parsing_accepts_a_leading_minus_and_one_or_two_decimals() {
        assert_eq!("-12.5".parse::<Cents>().unwrap(), Cents(-1_250));
        assert_eq!(".05".parse::<Cents>().unwrap(), Cents(5));
        assert_eq!("7.".parse::<Cents>().unwrap(), Cents(700));
    }

    #[test]
    fn parsing_refuses_text_that_is_not_an_amount() {
        for bad in ["", "-", ".", "1.234", "1.2.3", "abc", "12a", "--1", "1-"] {
            assert!(bad.parse::<Cents>().is_err(), "{bad:?} parsed");
        }
    }

    #[test]
    fn parsing_refuses_an_amount_too_large_for_cents() {
        assert!("999999999999999999".parse::<Cents>().is_err());
    }

    #[test]
    fn cents_sum_and_subtract() {
        let total: Cents = [Cents(100), Cents(250)].into_iter().sum();
        assert_eq!(total, Cents(350));
        assert_eq!(total - Cents(400), Cents(-50));
        assert_eq!(-Cents(5), Cents(-5));
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test money::`
Expected: compile error, `cannot find type Cents`.

- [ ] **Step 4: Implement `Cents`**

Above the tests in `src/money.rs` (this is MisterManager's `src/money.rs` trimmed to what Paychecker uses):

```rust
use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Neg, Sub};
use std::str::FromStr;

/// A monetary amount in integer cents. The only money type in the crate.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Cents(pub i64);

impl Cents {
    pub const ZERO: Cents = Cents(0);
}

/// A whole-dollar figure with thousands separators.
fn grouped(dollars: u64) -> String {
    let digits = dollars.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

impl Add for Cents {
    type Output = Cents;
    fn add(self, rhs: Cents) -> Cents {
        Cents(self.0 + rhs.0)
    }
}

impl Sub for Cents {
    type Output = Cents;
    fn sub(self, rhs: Cents) -> Cents {
        Cents(self.0 - rhs.0)
    }
}

impl Neg for Cents {
    type Output = Cents;
    fn neg(self) -> Cents {
        Cents(-self.0)
    }
}

impl AddAssign for Cents {
    fn add_assign(&mut self, rhs: Cents) {
        self.0 += rhs.0;
    }
}

impl Sum for Cents {
    fn sum<I: Iterator<Item = Cents>>(iter: I) -> Cents {
        Cents(iter.map(|c| c.0).sum())
    }
}

impl fmt::Display for Cents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let abs = self.0.unsigned_abs();
        let sign = if self.0 < 0 { "-" } else { "" };
        write!(f, "{sign}{}.{:02}", grouped(abs / 100), abs % 100)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("not a monetary amount: {0:?}")]
pub struct ParseMoneyError(String);

impl FromStr for Cents {
    type Err = ParseMoneyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || ParseMoneyError(s.to_string());
        let cleaned: String = s
            .chars()
            .filter(|c| !matches!(c, '$' | ',' | '_' | ' '))
            .collect();
        let (negative, body) = match cleaned.strip_prefix('-') {
            Some(rest) => (true, rest.to_string()),
            None => (false, cleaned),
        };
        let (whole, frac) = body.split_once('.').unwrap_or((body.as_str(), ""));
        if frac.len() > 2 || (whole.is_empty() && frac.is_empty()) {
            return Err(err());
        }
        if !whole.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
            return Err(err());
        }
        let whole: i64 = if whole.is_empty() {
            0
        } else {
            whole.parse().map_err(|_| err())?
        };
        let frac: i64 = match frac.len() {
            0 => 0,
            1 => frac.parse::<i64>().map_err(|_| err())? * 10,
            _ => frac.parse().map_err(|_| err())?,
        };
        let value = whole
            .checked_mul(100)
            .and_then(|v| v.checked_add(frac))
            .ok_or_else(err)?;
        Ok(Cents(if negative { -value } else { value }))
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test money:: && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: 7 passed; clippy and fmt clean.

- [ ] **Step 6: Commit**

Message: `feat: add Cents money type with parse and display`

---

### Task 2: Database open, schema, migrations, seed

**Files:**
- Modify: `src/lib.rs` (add `pub mod db;`)
- Create: `src/db/mod.rs`, `src/db/schema.sql`, `src/db/seed.sql`, `src/db/migration.rs`

**Interfaces:**
- Consumes: `money::Cents`.
- Produces:
  - `pub type FieldId = i64; pub type PaycheckId = i64;`
  - `pub enum Kind { Income, Deduction }` with `as_str(self) -> &'static str` (`"income"`/`"deduction"`), `label(self) -> &'static str` (`"Income"`/`"Deduction"`), `other(self) -> Kind`, `from_sql_text(&str) -> Option<Kind>`.
  - `pub struct Field { pub id: FieldId, pub name: String, pub kind: Kind, pub position: i64, pub archived: bool }`
  - `pub struct Paycheck { pub id: PaycheckId, pub date: NaiveDate, pub amounts: BTreeMap<FieldId, Cents> }`
  - `pub struct Db` (private `conn`), `pub fn open(&Path) -> Result<Db>`, `pub fn open_in_memory() -> Result<Db>`, `pub fn default_path() -> Result<PathBuf>`.
  - `Db::transaction<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T>` (private to `db`).

- [ ] **Step 1: Write the schema and seed**

`src/db/schema.sql` — the spec's schema verbatim:

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

`src/db/seed.sql`:

```sql
INSERT INTO field (name, kind, position) VALUES
    ('Salary', 'income', 0),
    ('Federal Tax', 'deduction', 1),
    ('Social Security', 'deduction', 2),
    ('Medicare', 'deduction', 3),
    ('State Tax', 'deduction', 4),
    ('Family Leave', 'deduction', 5),
    ('Medical Leave', 'deduction', 6),
    ('HSA', 'deduction', 7),
    ('401K (Trad)', 'deduction', 8),
    ('401K (Roth)', 'deduction', 9);
```

- [ ] **Step 2: Write the migration runner's failing tests**

`src/db/migration.rs`:

```rust
//! The chain of schema edits, and the runner that applies it.
//!
//! `schema.sql` is frozen at version 1 and is never edited again. Every change
//! since is an arm in [`MIGRATIONS`], and a fresh database takes the baseline,
//! the seed, and then the whole chain -- the same SQL, in the same order, that
//! an existing database takes the tail of. Every test builds its database
//! through `db::open_in_memory`, so the chain is replayed on every `cargo test`.

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "CREATE TABLE t (a INTEGER);";

    fn version(conn: &Connection) -> i64 {
        conn.query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn a_fresh_database_takes_the_baseline_the_seed_and_every_arm() {
        let conn = Connection::open_in_memory().unwrap();
        let chain = [Migration {
            version: 2,
            sql: "ALTER TABLE t ADD COLUMN b INTEGER;",
        }];
        apply(&conn, BASE, "INSERT INTO t (a) VALUES (7);", &chain).unwrap();
        assert_eq!(version(&conn), 2);
        let a: i64 = conn.query_row("SELECT a FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(a, 7);
        conn.execute("UPDATE t SET b = 1", []).unwrap();
    }

    #[test]
    fn a_database_takes_only_the_arms_above_its_version() {
        let conn = Connection::open_in_memory().unwrap();
        apply(&conn, BASE, "", &[]).unwrap();
        assert_eq!(version(&conn), 1);
        let chain = [Migration {
            version: 2,
            sql: "ALTER TABLE t ADD COLUMN b INTEGER;",
        }];
        apply(&conn, BASE, "", &chain).unwrap();
        assert_eq!(version(&conn), 2);
    }

    #[test]
    fn a_failing_arm_leaves_the_database_at_the_version_it_came_in_at() {
        let conn = Connection::open_in_memory().unwrap();
        apply(&conn, BASE, "", &[]).unwrap();
        let chain = [
            Migration {
                version: 2,
                sql: "ALTER TABLE t ADD COLUMN b INTEGER;",
            },
            Migration {
                version: 3,
                sql: "THIS IS NOT SQL;",
            },
        ];
        assert!(apply(&conn, BASE, "", &chain).is_err());
        assert_eq!(version(&conn), 1);
        assert!(conn.execute("UPDATE t SET b = 1", []).is_err());
    }

    #[test]
    fn a_database_newer_than_the_build_is_refused() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 5).unwrap();
        let err = apply(&conn, BASE, "", &[]).unwrap_err();
        assert!(err.to_string().contains("newer than this build"), "{err}");
    }

    #[test]
    fn every_arm_declares_the_version_its_position_gives_it() {
        for (index, arm) in MIGRATIONS.iter().enumerate() {
            assert_eq!(arm.version, index as i64 + 2);
        }
    }
}
```

- [ ] **Step 3: Write `db/mod.rs` with its failing tests**

`src/lib.rs` gains `pub mod db;`. `src/db/mod.rs`:

```rust
//! SQLite storage: opening and migrating the database, and the values it holds.

mod migration;

use crate::money::Cents;
use anyhow::{Context, Result};
use chrono::NaiveDate;
use rusqlite::Connection;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub type FieldId = i64;
pub type PaycheckId = i64;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Income,
    Deduction,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Income => "income",
            Kind::Deduction => "deduction",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Income => "Income",
            Kind::Deduction => "Deduction",
        }
    }

    pub fn other(self) -> Kind {
        match self {
            Kind::Income => Kind::Deduction,
            Kind::Deduction => Kind::Income,
        }
    }

    pub fn from_sql_text(text: &str) -> Option<Kind> {
        match text {
            "income" => Some(Kind::Income),
            "deduction" => Some(Kind::Deduction),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub id: FieldId,
    pub name: String,
    pub kind: Kind,
    pub position: i64,
    pub archived: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paycheck {
    pub id: PaycheckId,
    pub date: NaiveDate,
    pub amounts: BTreeMap<FieldId, Cents>,
}

pub struct Db {
    conn: Connection,
}

impl Db {
    /// Run `f` inside one transaction, committing only if it returns `Ok`.
    fn transaction<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let tx = self.conn.unchecked_transaction()?;
        let value = f(&tx)?;
        tx.commit()?;
        Ok(value)
    }
}

/// `~/.local/share/paychecker/paychecks.db`.
pub fn default_path() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".local/share/paychecker/paychecks.db"))
}

/// Open (creating if needed) the database at `path`, creating its parent
/// directory if missing, and bring it up to this build's schema.
pub fn open(path: &Path) -> Result<Db> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let conn = Connection::open(path)
        .with_context(|| format!("opening database at {}", path.display()))?;
    prepare(&conn).with_context(|| format!("preparing database at {}", path.display()))?;
    Ok(Db { conn })
}

pub fn open_in_memory() -> Result<Db> {
    let conn = Connection::open_in_memory()?;
    prepare(&conn)?;
    Ok(Db { conn })
}

fn prepare(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    migration::run(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("paychecker-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_new_database_is_at_schema_version_one() {
        let db = open_in_memory().unwrap();
        let version: i64 = db
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn a_new_database_enforces_foreign_keys() {
        let db = open_in_memory().unwrap();
        let on: bool = db
            .conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert!(on);
    }

    #[test]
    fn opening_a_file_creates_its_parent_directory() {
        let dir = scratch_dir("parent");
        let path = dir.join("nested").join("paychecks.db");
        open(&path).unwrap();
        assert!(path.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_written_by_a_newer_build_refuses_to_open() {
        let dir = scratch_dir("newer");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("paychecks.db");
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", 99)
            .unwrap();
        let err = open(&path).err().expect("opened a newer database");
        assert!(format!("{err:#}").contains("newer than this build"), "{err:#}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn kinds_round_trip_through_their_sql_text() {
        for kind in [Kind::Income, Kind::Deduction] {
            assert_eq!(Kind::from_sql_text(kind.as_str()), Some(kind));
        }
        assert_eq!(Kind::from_sql_text("bonus"), None);
    }
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cargo test db::`
Expected: compile errors — `migration::run`, `apply`, `Migration`, `MIGRATIONS` not found.

- [ ] **Step 5: Implement the runner**

Insert above the tests in `src/db/migration.rs`:

```rust
use anyhow::Result;
use rusqlite::Connection;

/// The frozen baseline. A schema change is an arm in [`MIGRATIONS`], never an
/// edit here: editing the baseline would give a fresh database a schema no
/// existing one can reach.
const SCHEMA: &str = include_str!("schema.sql");

/// The default fields, inserted once, into a database being created. Written
/// against the version-1 schema, so it runs before any arm.
const SEED: &str = include_str!("seed.sql");

/// One edit to the schema, and the version it leaves a database at.
// The chain is empty, so outside the tests nothing constructs one.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct Migration {
    pub version: i64,
    /// Run as a batch, so several statements separated by `;` are fine.
    pub sql: &'static str,
}

/// Every change to the schema since version 1, in order. The head version is
/// one plus the length, so appending an arm is the whole change.
pub(super) const MIGRATIONS: &[Migration] = &[];

const fn head(chain: &[Migration]) -> i64 {
    1 + chain.len() as i64
}

pub(super) fn run(conn: &Connection) -> Result<()> {
    apply(conn, SCHEMA, SEED, MIGRATIONS)
}

/// Bring `conn` from whatever version it is at to the head of `chain`, in one
/// transaction. `PRAGMA user_version` is transactional, so a failure partway
/// leaves the database at the version it came in at. A version above the head
/// was written by a later build and is refused rather than half-understood.
fn apply(conn: &Connection, schema: &str, seed: &str, chain: &[Migration]) -> Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let head = head(chain);
    if current == head {
        return Ok(());
    }
    anyhow::ensure!(
        current < head,
        "database is at schema version {current}, newer than this build ({head})"
    );
    let tx = conn.unchecked_transaction()?;
    if current == 0 {
        tx.execute_batch(schema)?;
        tx.execute_batch(seed)?;
    }
    for arm in chain.iter().filter(|arm| arm.version > current) {
        tx.execute_batch(arm.sql)?;
    }
    tx.pragma_update(None, "user_version", head)?;
    tx.commit()?;
    Ok(())
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test db:: && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: all pass, clean.

- [ ] **Step 7: Commit**

Message: `feat: add SQLite storage with versioned migrations and seed`

---

### Task 3: Field queries

**Files:**
- Create: `src/db/field.rs`
- Modify: `src/db/mod.rs` (add `mod field;` and a test helper)

**Interfaces:**
- Consumes: `Db`, `Field`, `FieldId`, `Kind`, `Db::transaction`.
- Produces (all `pub fn` on `impl Db`):
  - `fields(&self) -> Result<Vec<Field>>` — every field, archived included, in `position` order.
  - `insert_field(&self, name: &str, kind: Kind) -> Result<FieldId>` — trimmed name, appended at the end.
  - `update_field(&self, id: FieldId, name: &str, kind: Kind) -> Result<()>`
  - `set_archived(&self, id: FieldId, archived: bool) -> Result<()>`
  - `move_field(&self, id: FieldId, up: bool) -> Result<()>` — swap with neighbor, renumber `0..n` densely; no-op at either end.
  - `ensure_deletable(&self, id: FieldId) -> Result<()>` — `Err("{name} is used by a paycheck; archive it with x instead")`.
  - `delete_field(&self, id: FieldId) -> Result<()>`
  - `#[cfg(test)] pub(crate) fn field_id(&self, name: &str) -> FieldId` — panics if absent.
- Error messages (used verbatim by later tests): `"a field needs a name"`, `"a field named \"{name}\" already exists"`.

- [ ] **Step 1: Write the failing tests**

In `src/db/mod.rs` add `mod field;` next to `mod migration;` and, inside `impl Db`:

```rust
    #[cfg(test)]
    pub(crate) fn field_id(&self, name: &str) -> FieldId {
        self.fields()
            .unwrap()
            .into_iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("no field named {name:?}"))
            .id
    }
```

`src/db/field.rs`:

```rust
//! Field queries.

#[cfg(test)]
mod tests {
    use crate::db::{Kind, open_in_memory};

    fn names(db: &crate::db::Db) -> Vec<String> {
        db.fields().unwrap().into_iter().map(|f| f.name).collect()
    }

    #[test]
    fn a_new_database_is_seeded_with_the_default_fields_in_order() {
        let db = open_in_memory().unwrap();
        let fields = db.fields().unwrap();
        assert_eq!(
            names(&db),
            [
                "Salary",
                "Federal Tax",
                "Social Security",
                "Medicare",
                "State Tax",
                "Family Leave",
                "Medical Leave",
                "HSA",
                "401K (Trad)",
                "401K (Roth)",
            ]
        );
        assert_eq!(fields[0].kind, Kind::Income);
        assert!(fields[1..].iter().all(|f| f.kind == Kind::Deduction));
        assert!(fields.iter().all(|f| !f.archived));
    }

    #[test]
    fn a_new_field_goes_at_the_end_with_its_name_trimmed() {
        let db = open_in_memory().unwrap();
        let id = db.insert_field("  Bonus ", Kind::Income).unwrap();
        let last = db.fields().unwrap().pop().unwrap();
        assert_eq!((last.id, last.name.as_str(), last.kind), (id, "Bonus", Kind::Income));
        assert_eq!(last.position, 10);
    }

    #[test]
    fn a_blank_name_is_refused() {
        let db = open_in_memory().unwrap();
        let err = db.insert_field("   ", Kind::Deduction).unwrap_err();
        assert_eq!(err.to_string(), "a field needs a name");
    }

    #[test]
    fn a_duplicate_name_is_refused() {
        let db = open_in_memory().unwrap();
        let err = db.insert_field("Medicare ", Kind::Deduction).unwrap_err();
        assert_eq!(err.to_string(), "a field named \"Medicare\" already exists");
        let hsa = db.field_id("HSA");
        assert!(db.update_field(hsa, "Medicare", Kind::Deduction).is_err());
    }

    #[test]
    fn a_field_keeps_its_own_name_when_only_its_kind_changes() {
        let db = open_in_memory().unwrap();
        let hsa = db.field_id("HSA");
        db.update_field(hsa, "HSA", Kind::Income).unwrap();
        let field = db.fields().unwrap().into_iter().find(|f| f.id == hsa).unwrap();
        assert_eq!(field.kind, Kind::Income);
    }

    #[test]
    fn renaming_a_field_keeps_its_id_and_position() {
        let db = open_in_memory().unwrap();
        let hsa = db.field_id("HSA");
        db.update_field(hsa, "Health Savings", Kind::Deduction).unwrap();
        assert_eq!(db.field_id("Health Savings"), hsa);
        assert_eq!(names(&db)[7], "Health Savings");
    }

    #[test]
    fn archiving_and_unarchiving_a_field() {
        let db = open_in_memory().unwrap();
        let hsa = db.field_id("HSA");
        db.set_archived(hsa, true).unwrap();
        assert!(db.fields().unwrap()[7].archived);
        db.set_archived(hsa, false).unwrap();
        assert!(!db.fields().unwrap()[7].archived);
    }

    #[test]
    fn moving_a_field_swaps_it_with_its_neighbor_and_renumbers_densely() {
        let db = open_in_memory().unwrap();
        db.delete_field(db.field_id("Social Security")).unwrap();
        db.move_field(db.field_id("Medicare"), true).unwrap();
        let fields = db.fields().unwrap();
        assert_eq!(fields[1].name, "Medicare");
        assert_eq!(fields[2].name, "Federal Tax");
        let positions: Vec<i64> = fields.iter().map(|f| f.position).collect();
        assert_eq!(positions, (0..9).collect::<Vec<_>>());
    }

    #[test]
    fn moving_past_either_end_changes_nothing() {
        let db = open_in_memory().unwrap();
        let before = names(&db);
        db.move_field(db.field_id("Salary"), true).unwrap();
        db.move_field(db.field_id("401K (Roth)"), false).unwrap();
        assert_eq!(names(&db), before);
    }

    #[test]
    fn an_unused_field_can_be_deleted() {
        let db = open_in_memory().unwrap();
        db.delete_field(db.field_id("HSA")).unwrap();
        assert!(!names(&db).contains(&"HSA".to_string()));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test db::field`
Expected: compile errors — `fields`, `insert_field`, etc. not found.

- [ ] **Step 3: Implement**

Above the tests in `src/db/field.rs`:

```rust
use super::{Db, Field, FieldId, Kind};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, Row, params};

impl Db {
    pub fn fields(&self) -> Result<Vec<Field>> {
        let fields = self
            .conn
            .prepare("SELECT id, name, kind, position, archived FROM field ORDER BY position")?
            .query_map([], field_from_row)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(fields)
    }

    pub fn insert_field(&self, name: &str, kind: Kind) -> Result<FieldId> {
        let name = checked_name(&self.conn, name, None)?;
        self.conn.execute(
            "INSERT INTO field (name, kind, position)
             VALUES (?1, ?2, (SELECT COALESCE(MAX(position), -1) + 1 FROM field))",
            params![name, kind.as_str()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_field(&self, id: FieldId, name: &str, kind: Kind) -> Result<()> {
        let name = checked_name(&self.conn, name, Some(id))?;
        self.conn.execute(
            "UPDATE field SET name = ?1, kind = ?2 WHERE id = ?3",
            params![name, kind.as_str(), id],
        )?;
        Ok(())
    }

    pub fn set_archived(&self, id: FieldId, archived: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE field SET archived = ?1 WHERE id = ?2",
            params![archived, id],
        )?;
        Ok(())
    }

    /// Swap `id` with the field above (`up`) or below it, then renumber every
    /// position `0..n` so gaps left by deletions close.
    pub fn move_field(&self, id: FieldId, up: bool) -> Result<()> {
        self.transaction(|conn| {
            let mut ids: Vec<FieldId> = conn
                .prepare("SELECT id FROM field ORDER BY position")?
                .query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            let at = ids.iter().position(|&i| i == id).context("no such field")?;
            let to = if up {
                at.checked_sub(1)
            } else {
                (at + 1 < ids.len()).then_some(at + 1)
            };
            if let Some(to) = to {
                ids.swap(at, to);
            }
            for (position, id) in ids.iter().enumerate() {
                conn.execute(
                    "UPDATE field SET position = ?1 WHERE id = ?2",
                    params![position as i64, id],
                )?;
            }
            Ok(())
        })
    }

    /// Refuse to delete a field any paycheck has an amount for. The schema
    /// refuses too (`amount.field_id` has no cascade); this is the refusal
    /// with a sentence in it.
    pub fn ensure_deletable(&self, id: FieldId) -> Result<()> {
        let (name, used): (String, bool) = self.conn.query_row(
            "SELECT name, EXISTS(SELECT 1 FROM amount WHERE field_id = field.id)
             FROM field WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        ensure!(!used, "{name} is used by a paycheck; archive it with x instead");
        Ok(())
    }

    pub fn delete_field(&self, id: FieldId) -> Result<()> {
        self.ensure_deletable(id)?;
        self.conn.execute("DELETE FROM field WHERE id = ?1", [id])?;
        Ok(())
    }
}

/// `name` trimmed, or an error if it is blank or another field has it.
fn checked_name(conn: &Connection, name: &str, except: Option<FieldId>) -> Result<String> {
    let name = name.trim();
    ensure!(!name.is_empty(), "a field needs a name");
    let taken: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM field WHERE name = ?1 AND id IS NOT ?2)",
        params![name, except],
        |r| r.get(0),
    )?;
    ensure!(!taken, "a field named {name:?} already exists");
    Ok(name.to_string())
}

fn field_from_row(row: &Row) -> rusqlite::Result<Field> {
    let kind: String = row.get(2)?;
    let kind = Kind::from_sql_text(&kind).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Text,
            format!("unknown field kind {kind:?}").into(),
        )
    })?;
    Ok(Field {
        id: row.get(0)?,
        name: row.get(1)?,
        kind,
        position: row.get(3)?,
        archived: row.get(4)?,
    })
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test db:: && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: all pass, clean.

- [ ] **Step 5: Commit**

Message: `feat: add field queries with dense reordering and delete guard`

---

### Task 4: Paycheck queries

**Files:**
- Create: `src/db/paycheck.rs`
- Modify: `src/db/mod.rs` (add `mod paycheck;`)

**Interfaces:**
- Consumes: `Db`, `Paycheck`, `PaycheckId`, `FieldId`, `Cents`, `Db::transaction`, `Db::field_id` (tests).
- Produces (on `impl Db`):
  - `paychecks(&self) -> Result<Vec<Paycheck>>` — every paycheck, oldest first, each with all its amounts.
  - `insert_paycheck(&self, date: NaiveDate, amounts: &[(FieldId, Cents)]) -> Result<PaycheckId>`
  - `update_paycheck(&self, id: PaycheckId, date: NaiveDate, amounts: &[(FieldId, Cents)]) -> Result<()>` — replaces all amounts.
  - `delete_paycheck(&self, id: PaycheckId) -> Result<()>`
- Error message: `"a paycheck dated {date} already exists"` (ISO date).

- [ ] **Step 1: Write the failing tests**

`src/db/paycheck.rs`:

```rust
//! Paycheck queries.

#[cfg(test)]
mod tests {
    use crate::db::{Db, FieldId, open_in_memory};
    use crate::money::Cents;
    use chrono::NaiveDate;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn stub(db: &Db) -> Vec<(FieldId, Cents)> {
        vec![
            (db.field_id("Salary"), Cents(400_000)),
            (db.field_id("Federal Tax"), Cents(60_000)),
        ]
    }

    fn amount_rows(db: &Db) -> i64 {
        db.conn
            .query_row("SELECT COUNT(*) FROM amount", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn a_saved_paycheck_comes_back_with_its_date_and_every_amount() {
        let db = open_in_memory().unwrap();
        let id = db.insert_paycheck(day(2026, 1, 16), &stub(&db)).unwrap();
        let checks = db.paychecks().unwrap();
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].id, id);
        assert_eq!(checks[0].date, day(2026, 1, 16));
        assert_eq!(checks[0].amounts[&db.field_id("Salary")], Cents(400_000));
        assert_eq!(checks[0].amounts[&db.field_id("Federal Tax")], Cents(60_000));
        assert_eq!(checks[0].amounts.len(), 2);
    }

    #[test]
    fn paychecks_come_back_oldest_first() {
        let db = open_in_memory().unwrap();
        db.insert_paycheck(day(2026, 1, 16), &[]).unwrap();
        db.insert_paycheck(day(2025, 12, 19), &[]).unwrap();
        db.insert_paycheck(day(2026, 1, 2), &[]).unwrap();
        let dates: Vec<_> = db.paychecks().unwrap().iter().map(|p| p.date).collect();
        assert_eq!(dates, [day(2025, 12, 19), day(2026, 1, 2), day(2026, 1, 16)]);
    }

    #[test]
    fn a_second_paycheck_on_the_same_date_is_refused() {
        let db = open_in_memory().unwrap();
        db.insert_paycheck(day(2026, 1, 16), &[]).unwrap();
        let err = db.insert_paycheck(day(2026, 1, 16), &[]).unwrap_err();
        assert_eq!(err.to_string(), "a paycheck dated 2026-01-16 already exists");
        let other = db.insert_paycheck(day(2026, 1, 30), &[]).unwrap();
        assert!(db.update_paycheck(other, day(2026, 1, 16), &[]).is_err());
    }

    #[test]
    fn editing_a_paycheck_without_changing_its_date_is_allowed() {
        let db = open_in_memory().unwrap();
        let id = db.insert_paycheck(day(2026, 1, 16), &stub(&db)).unwrap();
        db.update_paycheck(id, day(2026, 1, 16), &stub(&db)).unwrap();
    }

    #[test]
    fn editing_a_paycheck_replaces_its_date_and_amounts() {
        let db = open_in_memory().unwrap();
        let id = db.insert_paycheck(day(2026, 1, 16), &stub(&db)).unwrap();
        let hsa = db.field_id("HSA");
        db.update_paycheck(id, day(2026, 1, 17), &[(hsa, Cents(10_000))])
            .unwrap();
        let check = &db.paychecks().unwrap()[0];
        assert_eq!(check.date, day(2026, 1, 17));
        assert_eq!(check.amounts.len(), 1);
        assert_eq!(check.amounts[&hsa], Cents(10_000));
    }

    #[test]
    fn a_failed_edit_changes_nothing() {
        let db = open_in_memory().unwrap();
        let id = db.insert_paycheck(day(2026, 1, 16), &stub(&db)).unwrap();
        assert!(db.update_paycheck(id, day(2026, 1, 16), &[(9_999, Cents(1))]).is_err());
        assert_eq!(db.paychecks().unwrap()[0].amounts.len(), 2);
    }

    #[test]
    fn deleting_a_paycheck_deletes_its_amounts() {
        let db = open_in_memory().unwrap();
        let id = db.insert_paycheck(day(2026, 1, 16), &stub(&db)).unwrap();
        db.delete_paycheck(id).unwrap();
        assert!(db.paychecks().unwrap().is_empty());
        assert_eq!(amount_rows(&db), 0);
    }

    #[test]
    fn a_field_a_paycheck_uses_cannot_be_deleted() {
        let db = open_in_memory().unwrap();
        db.insert_paycheck(day(2026, 1, 16), &stub(&db)).unwrap();
        let err = db.delete_field(db.field_id("Salary")).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Salary is used by a paycheck; archive it with x instead"
        );
        assert_eq!(db.fields().unwrap()[0].name, "Salary");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test db::paycheck`
Expected: compile errors — `insert_paycheck`, `paychecks`, etc. not found.

- [ ] **Step 3: Implement**

Add `mod paycheck;` to `src/db/mod.rs`. Above the tests in `src/db/paycheck.rs`:

```rust
use super::{Db, FieldId, Paycheck, PaycheckId};
use crate::money::Cents;
use anyhow::{Result, ensure};
use chrono::NaiveDate;
use rusqlite::{Connection, params};
use std::collections::{BTreeMap, HashMap};

impl Db {
    pub fn paychecks(&self) -> Result<Vec<Paycheck>> {
        let mut checks: Vec<Paycheck> = self
            .conn
            .prepare("SELECT id, date FROM paycheck ORDER BY date")?
            .query_map([], |r| {
                Ok(Paycheck {
                    id: r.get(0)?,
                    date: r.get(1)?,
                    amounts: BTreeMap::new(),
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let index: HashMap<PaycheckId, usize> =
            checks.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
        let mut stmt = self
            .conn
            .prepare("SELECT paycheck_id, field_id, cents FROM amount")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let paycheck_id: PaycheckId = row.get(0)?;
            if let Some(&i) = index.get(&paycheck_id) {
                checks[i].amounts.insert(row.get(1)?, Cents(row.get(2)?));
            }
        }
        Ok(checks)
    }

    pub fn insert_paycheck(
        &self,
        date: NaiveDate,
        amounts: &[(FieldId, Cents)],
    ) -> Result<PaycheckId> {
        self.transaction(|conn| {
            ensure_date_free(conn, date, None)?;
            conn.execute("INSERT INTO paycheck (date) VALUES (?1)", [date])?;
            let id = conn.last_insert_rowid();
            write_amounts(conn, id, amounts)?;
            Ok(id)
        })
    }

    pub fn update_paycheck(
        &self,
        id: PaycheckId,
        date: NaiveDate,
        amounts: &[(FieldId, Cents)],
    ) -> Result<()> {
        self.transaction(|conn| {
            ensure_date_free(conn, date, Some(id))?;
            conn.execute(
                "UPDATE paycheck SET date = ?1 WHERE id = ?2",
                params![date, id],
            )?;
            conn.execute("DELETE FROM amount WHERE paycheck_id = ?1", [id])?;
            write_amounts(conn, id, amounts)
        })
    }

    pub fn delete_paycheck(&self, id: PaycheckId) -> Result<()> {
        self.conn
            .execute("DELETE FROM paycheck WHERE id = ?1", [id])?;
        Ok(())
    }
}

/// Refuse `date` if a paycheck other than `except` already has it.
fn ensure_date_free(conn: &Connection, date: NaiveDate, except: Option<PaycheckId>) -> Result<()> {
    let taken: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM paycheck WHERE date = ?1 AND id IS NOT ?2)",
        params![date, except],
        |r| r.get(0),
    )?;
    ensure!(!taken, "a paycheck dated {date} already exists");
    Ok(())
}

fn write_amounts(conn: &Connection, id: PaycheckId, amounts: &[(FieldId, Cents)]) -> Result<()> {
    let mut stmt =
        conn.prepare("INSERT INTO amount (paycheck_id, field_id, cents) VALUES (?1, ?2, ?3)")?;
    for (field_id, cents) in amounts {
        stmt.execute(params![id, field_id, cents.0])?;
    }
    Ok(())
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test db:: && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: all pass, clean. (`a_failed_edit_changes_nothing` passes because field `9999` violates the foreign key and the transaction rolls back.)

- [ ] **Step 5: Commit**

Message: `feat: add paycheck queries with unique dates`

---

### Task 5: `calc`

**Files:**
- Create: `src/calc.rs`
- Modify: `src/lib.rs` (add `pub mod calc;`)
- Modify: `docs/superpowers/specs/2026-09-29-paychecker-design.md` (Semantics)

**Interfaces:**
- Consumes: `db::{Field, FieldId, Kind, Paycheck, PaycheckId}`, `money::Cents`. No `rusqlite`, no `ratatui`.
- Produces:
  - `pub struct Percent(pub i64)` — hundredths of a percent; `Display` → `27.38%`.
  - `pub fn percent(part: Cents, income: Cents) -> Option<Percent>` — `None` when income is 0; rounds half away from zero.
  - `pub fn show(percent: Option<Percent>) -> String` — `—` for `None`.
  - `pub struct Totals { pub income: Cents, pub net: Cents }` (`Copy`, `Default`, `PartialEq`, `Debug`).
  - `pub fn totals(amounts: impl IntoIterator<Item = (Kind, Cents)>) -> Totals`
  - `pub fn sort_rows(fields: &mut [&Field])` — income first, then deductions, each by `position`.
  - `pub fn visible_fields<'a>(fields: &'a [Field], paychecks: &[&Paycheck]) -> Vec<&'a Field>` — active, or used by one of `paychecks`; sorted by `sort_rows`.
  - `pub struct Column { pub id: PaycheckId, pub date: NaiveDate }`
  - `pub struct AmountRow { pub name: String, pub cells: Vec<Option<Cents>>, pub ytd: Cents }`
  - `pub struct PercentRow { pub label: String, pub cells: Vec<Option<Percent>>, pub ytd: Option<Percent> }`
  - `pub struct Sheet { pub year: i32, pub columns: Vec<Column>, pub rows: Vec<AmountRow>, pub net: Vec<Cents>, pub net_ytd: Cents, pub percent_rows: Vec<PercentRow> }`
  - `pub fn sheet(year: i32, fields: &[Field], paychecks: &[Paycheck]) -> Sheet`

- [ ] **Step 1: Write the failing tests**

`src/lib.rs` gains `pub mod calc;`. `src/calc.rs`:

```rust
//! The Sheet's arithmetic: per-paycheck and YTD totals, net, percentages, and
//! which fields get a row. Pure: it takes fields and paychecks as plain values.

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn field(id: FieldId, name: &str, kind: Kind, position: i64, archived: bool) -> Field {
        Field {
            id,
            name: name.to_string(),
            kind,
            position,
            archived,
        }
    }

    fn check(id: PaycheckId, (y, m, d): (i32, u32, u32), amounts: &[(FieldId, i64)]) -> Paycheck {
        Paycheck {
            id,
            date: NaiveDate::from_ymd_opt(y, m, d).unwrap(),
            amounts: amounts.iter().map(|&(f, c)| (f, Cents(c))).collect::<BTreeMap<_, _>>(),
        }
    }

    /// Salary (income), Tax, Retirement (deductions); Bonus (income) sorts
    /// after Tax by position but before it by kind.
    fn fields() -> Vec<Field> {
        vec![
            field(1, "Salary", Kind::Income, 0, false),
            field(2, "Tax", Kind::Deduction, 1, false),
            field(3, "Bonus", Kind::Income, 2, false),
            field(4, "Retirement", Kind::Deduction, 3, false),
        ]
    }

    #[test]
    fn net_is_income_minus_deductions() {
        let t = totals([
            (Kind::Income, Cents(400_000)),
            (Kind::Deduction, Cents(60_000)),
            (Kind::Income, Cents(10_000)),
            (Kind::Deduction, Cents(-500)),
        ]);
        assert_eq!(t, Totals { income: Cents(410_000), net: Cents(350_500) });
    }

    #[test]
    fn percent_shows_two_decimals() {
        assert_eq!(show(percent(Cents(60_000), Cents(400_000))), "15.00%");
        assert_eq!(show(percent(Cents(109_520), Cents(400_000))), "27.38%");
    }

    #[test]
    fn percent_rounds_half_away_from_zero() {
        assert_eq!(percent(Cents(1), Cents(3)), Some(Percent(3333)));
        assert_eq!(percent(Cents(2), Cents(3)), Some(Percent(6667)));
        assert_eq!(percent(Cents(-2), Cents(3)), Some(Percent(-6667)));
        assert_eq!(show(percent(Cents(-2), Cents(3))), "-66.67%");
    }

    #[test]
    fn percent_is_a_dash_when_income_is_zero() {
        assert_eq!(percent(Cents(100), Cents(0)), None);
        assert_eq!(show(None), "—");
    }

    #[test]
    fn rows_put_income_before_deductions_each_in_position_order() {
        let fields = fields();
        let names: Vec<_> = visible_fields(&fields, &[]).iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["Salary", "Bonus", "Tax", "Retirement"]);
    }

    #[test]
    fn an_archived_field_appears_only_in_years_a_paycheck_used_it() {
        let mut fields = fields();
        fields[3].archived = true;
        let checks = [check(1, (2025, 12, 19), &[(1, 100), (4, 10)]), check(2, (2026, 1, 2), &[(1, 100)])];
        let names = |year: i32| -> Vec<String> {
            sheet(year, &fields, &checks).rows.into_iter().map(|r| r.name).collect()
        };
        assert!(names(2025).contains(&"Retirement".to_string()));
        assert!(!names(2026).contains(&"Retirement".to_string()));
    }

    #[test]
    fn the_sheet_holds_only_the_years_paychecks_oldest_first() {
        let checks = [
            check(1, (2026, 1, 16), &[(1, 100)]),
            check(2, (2025, 12, 19), &[(1, 100)]),
            check(3, (2026, 1, 2), &[(1, 100)]),
        ];
        let s = sheet(2026, &fields(), &checks);
        let ids: Vec<_> = s.columns.iter().map(|c| c.id).collect();
        assert_eq!(ids, [3, 1]);
    }

    #[test]
    fn a_field_a_paycheck_lacks_is_a_blank_cell_and_zero_in_the_percent_block() {
        let checks = [check(1, (2026, 1, 2), &[(1, 400_000)])];
        let s = sheet(2026, &fields(), &checks);
        let tax = s.rows.iter().find(|r| r.name == "Tax").unwrap();
        assert_eq!(tax.cells, [None]);
        assert_eq!(tax.ytd, Cents::ZERO);
        let tax_pct = s.percent_rows.iter().find(|r| r.label == "Tax").unwrap();
        assert_eq!(tax_pct.cells, [Some(Percent(0))]);
    }

    #[test]
    fn the_sheet_totals_each_column_and_the_year() {
        let checks = [
            check(1, (2026, 1, 2), &[(1, 400_000), (2, 60_000), (4, 20_000)]),
            check(2, (2026, 1, 16), &[(1, 400_000), (3, 50_000), (2, 70_000), (4, 20_000)]),
        ];
        let s = sheet(2026, &fields(), &checks);
        assert_eq!(s.net, [Cents(320_000), Cents(360_000)]);
        assert_eq!(s.net_ytd, Cents(680_000));
        let salary = &s.rows[0];
        assert_eq!(salary.cells, [Some(Cents(400_000)), Some(Cents(400_000))]);
        assert_eq!(salary.ytd, Cents(800_000));
    }

    #[test]
    fn the_percent_block_lists_deductions_then_net_pay() {
        let checks = [check(1, (2026, 1, 2), &[(1, 400_000), (2, 60_000), (4, 20_000)])];
        let s = sheet(2026, &fields(), &checks);
        let labels: Vec<_> = s.percent_rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, ["Tax", "Retirement", "Net Pay"]);
        assert_eq!(s.percent_rows[2].cells, [Some(Percent(8000))]);
    }

    #[test]
    fn the_ytd_percent_is_the_ratio_of_the_sums_not_an_average() {
        // 10% and 30% average to 20%, but 1,000 of 4,000 is 25%.
        let checks = [
            check(1, (2026, 1, 2), &[(1, 100_000), (2, 10_000)]),
            check(2, (2026, 1, 16), &[(1, 300_000), (2, 90_000)]),
        ];
        let s = sheet(2026, &fields(), &checks);
        assert_eq!(s.percent_rows[0].ytd, Some(Percent(2500)));
    }

    #[test]
    fn a_year_with_no_paychecks_has_rows_but_no_columns_and_dashes_for_percentages() {
        let s = sheet(2027, &fields(), &[]);
        assert!(s.columns.is_empty());
        assert_eq!(s.rows.len(), 4);
        assert_eq!(s.net_ytd, Cents::ZERO);
        assert_eq!(s.percent_rows.last().unwrap().ytd, None);
    }

    #[test]
    fn changing_a_fields_kind_changes_past_net() {
        let mut fields = fields();
        let checks = [check(1, (2026, 1, 2), &[(1, 400_000), (3, 50_000)])];
        assert_eq!(sheet(2026, &fields, &checks).net, [Cents(450_000)]);
        fields[2].kind = Kind::Deduction;
        assert_eq!(sheet(2026, &fields, &checks).net, [Cents(350_000)]);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test calc::`
Expected: compile errors — `totals`, `percent`, `sheet` not found.

- [ ] **Step 3: Implement**

Above the tests in `src/calc.rs`:

```rust
use crate::db::{Field, FieldId, Kind, Paycheck, PaycheckId};
use crate::money::Cents;
use chrono::{Datelike, NaiveDate};
use std::fmt;

/// A percentage in hundredths of a percent: `Percent(2738)` is `27.38%`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Percent(pub i64);

impl fmt::Display for Percent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let abs = self.0.unsigned_abs();
        let sign = if self.0 < 0 { "-" } else { "" };
        write!(f, "{sign}{}.{:02}%", abs / 100, abs % 100)
    }
}

/// `part` as a percentage of `income`, rounded half away from zero, or `None`
/// when there is no income to divide by. `i128` because `part * 10_000`
/// overflows `i64` for amounts a `Cents` can hold.
pub fn percent(part: Cents, income: Cents) -> Option<Percent> {
    if income.0 == 0 {
        return None;
    }
    let num = i128::from(part.0) * 10_000;
    let den = i128::from(income.0);
    let (q, r) = (num / den, num % den);
    let q = if r.abs() * 2 >= den.abs() {
        q + num.signum() * den.signum()
    } else {
        q
    };
    Some(Percent(q as i64))
}

pub fn show(percent: Option<Percent>) -> String {
    percent.map_or_else(|| "—".to_string(), |p| p.to_string())
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub income: Cents,
    pub net: Cents,
}

pub fn totals(amounts: impl IntoIterator<Item = (Kind, Cents)>) -> Totals {
    let mut t = Totals::default();
    for (kind, cents) in amounts {
        match kind {
            Kind::Income => {
                t.income += cents;
                t.net += cents;
            }
            Kind::Deduction => t.net = t.net - cents,
        }
    }
    t
}

/// Sheet row order, which is also the paycheck form's Tab order.
pub fn sort_rows(fields: &mut [&Field]) {
    fields.sort_by_key(|f| (f.kind == Kind::Deduction, f.position));
}

/// The fields that get a row: every active field, plus any archived field one
/// of `paychecks` has an amount for.
pub fn visible_fields<'a>(fields: &'a [Field], paychecks: &[&Paycheck]) -> Vec<&'a Field> {
    let mut shown: Vec<&Field> = fields
        .iter()
        .filter(|f| !f.archived || paychecks.iter().any(|p| p.amounts.contains_key(&f.id)))
        .collect();
    sort_rows(&mut shown);
    shown
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    pub id: PaycheckId,
    pub date: NaiveDate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AmountRow {
    pub name: String,
    /// `None` where the paycheck has no amount for this field.
    pub cells: Vec<Option<Cents>>,
    pub ytd: Cents,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PercentRow {
    pub label: String,
    pub cells: Vec<Option<Percent>>,
    pub ytd: Option<Percent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sheet {
    pub year: i32,
    pub columns: Vec<Column>,
    pub rows: Vec<AmountRow>,
    pub net: Vec<Cents>,
    pub net_ytd: Cents,
    /// Every visible deduction, then `Net Pay`.
    pub percent_rows: Vec<PercentRow>,
}

/// Lay out `year`: one column per paycheck dated in it, oldest first.
pub fn sheet(year: i32, fields: &[Field], paychecks: &[Paycheck]) -> Sheet {
    let mut checks: Vec<&Paycheck> = paychecks.iter().filter(|p| p.date.year() == year).collect();
    checks.sort_by_key(|p| p.date);
    let kind_of = |id: FieldId| fields.iter().find(|f| f.id == id).map(|f| f.kind);
    let column_totals: Vec<Totals> = checks
        .iter()
        .map(|p| totals(p.amounts.iter().filter_map(|(&id, &c)| Some((kind_of(id)?, c)))))
        .collect();
    let ytd = Totals {
        income: column_totals.iter().map(|t| t.income).sum(),
        net: column_totals.iter().map(|t| t.net).sum(),
    };
    let shown = visible_fields(fields, &checks);
    let rows: Vec<AmountRow> = shown
        .iter()
        .map(|f| {
            let cells: Vec<Option<Cents>> =
                checks.iter().map(|p| p.amounts.get(&f.id).copied()).collect();
            AmountRow {
                name: f.name.clone(),
                ytd: cells.iter().flatten().copied().sum(),
                cells,
            }
        })
        .collect();
    let mut percent_rows: Vec<PercentRow> = shown
        .iter()
        .zip(&rows)
        .filter(|(f, _)| f.kind == Kind::Deduction)
        .map(|(_, row)| PercentRow {
            label: row.name.clone(),
            cells: row
                .cells
                .iter()
                .zip(&column_totals)
                .map(|(c, t)| percent(c.unwrap_or_default(), t.income))
                .collect(),
            ytd: percent(row.ytd, ytd.income),
        })
        .collect();
    percent_rows.push(PercentRow {
        label: "Net Pay".to_string(),
        cells: column_totals.iter().map(|t| percent(t.net, t.income)).collect(),
        ytd: percent(ytd.net, ytd.income),
    });
    Sheet {
        year,
        columns: checks.iter().map(|p| Column { id: p.id, date: p.date }).collect(),
        rows,
        net: column_totals.iter().map(|t| t.net).collect(),
        net_ytd: ytd.net,
        percent_rows,
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test calc:: && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: all pass, clean.

- [ ] **Step 5: Record the missing-amount rule in the spec**

In the spec's **Semantics** list, after the **Row visibility** bullet, add:

```markdown
- **Missing amounts.** A paycheck with no amount for a visible field shows a blank cell there, and
  counts it as 0 in its percentages and in YTD.
```

- [ ] **Step 6: Commit**

Message: `feat: add pure Sheet calculations for totals, net, and percentages`

---

### Task 6: `tui::text`

**Files:**
- Create: `src/tui/mod.rs`, `src/tui/text.rs` (copied from `../MisterManager/src/tui/text.rs`)
- Modify: `src/lib.rs` (add `pub mod tui;`)

**Interfaces:**
- Produces (all `pub(super)`, i.e. visible throughout `tui`): `TextBuffer` (`value`, `caret`, `len`, `set`, `clear`, `insert`, `backspace`, `delete`, `step(isize)`, `start`, `end`, `From<impl Into<String>>`, `Default`), `enum Edit { Ignored, Moved, Changed }`, `fn edit_key(&mut TextBuffer, KeyEvent) -> Edit`, `fn is_bare(KeyEvent) -> bool`.

- [ ] **Step 1: Copy the module and its tests**

```bash
mkdir -p src/tui
cp ../MisterManager/src/tui/text.rs src/tui/text.rs
```

`src/lib.rs` gains `pub mod tui;`. `src/tui/mod.rs`:

```rust
//! The terminal UI.

// Nothing outside the tests calls into these modules until `App` is wired up
// to `run`.
#![allow(dead_code)]

mod text;
```

- [ ] **Step 2: Trim MisterManager's references out of the copy**

Replace the module doc (lines 1–10) with:

```rust
//! A line of text being typed, and the keys that edit it.
//!
//! One buffer serves every text box in the app, so the caret, the editing,
//! and the keys that drive them are the same in each.
```

Replace the `edit_key` doc comment's paragraph beginning "The same shape as [`scroll_key`]…" and the two link definitions `[`scroll_key`]: …` / `[`search_key`]: …` with:

```rust
/// Written once here so `Ctrl`+`W` deletes a word in every box in the app.
```

Replace the `is_bare` doc comment's second paragraph ("The app's own keys are read through this too -- see `App::dispatch` -- …") with:

```rust
/// The app's own keys are read through this too, so a modifier nothing binds
/// does not fall through to the bare letter's meaning: `Ctrl`+`Q` does not quit.
```

In the tests, delete any sentence describing MisterManager's history (e.g. "-- which is what `Ctrl`+`C` used to do"). Grep the file for `MisterManager`, `ledger`, `search`, `SearchBox`, `form::Field`, `used to`, and remove or reword each hit so it describes this app.

- [ ] **Step 3: Run the copied tests**

Run: `cargo test tui::text && cargo clippy --all-targets -- -D warnings && cargo fmt --check && RUSTDOCFLAGS=-Dwarnings cargo doc --no-deps --document-private-items`
Expected: all copied tests pass; clippy and fmt clean; no rustdoc broken-link warnings.

- [ ] **Step 4: Commit**

Message: `feat: copy TextBuffer and edit_key from MisterManager`

---

### Task 7: `tui::form` — dates, paycheck form, field form

**Files:**
- Create: `src/tui/form.rs`, `src/tui/test_support.rs`
- Modify: `src/tui/mod.rs` (add `mod form;` and `#[cfg(test)] mod test_support;`)

**Interfaces:**
- Consumes: `TextBuffer`, `edit_key`, `is_bare`; `calc::{sort_rows, totals, Totals}`; `db::{Field, FieldId, Kind, Paycheck, PaycheckId}`; `Cents`.
- Produces (`pub(super)`):
  - `fn parse_date(raw: &str, today: NaiveDate) -> Result<NaiveDate>` — `YYYY-MM-DD` or `M/D`.
  - `fn iso(date: NaiveDate) -> String`
  - `enum Step { Days(i64), Months(i32) }`, `fn step_date(date: NaiveDate, step: Step) -> Option<NaiveDate>`
  - `enum Outcome { Continue, Submit, Cancel }`
  - `struct AmountInput { field_id: FieldId, name: String, kind: Kind, text: TextBuffer }`
  - `struct PaycheckForm { editing: Option<PaycheckId>, date: TextBuffer, amounts: Vec<AmountInput>, focus: usize /* 0 = date, n = amounts[n-1] */, today (private) }` with `add(fields, latest: Option<&Paycheck>, today)`, `edit(fields, paycheck: &Paycheck, today)`, `on_key(KeyEvent) -> Outcome`, `totals() -> Option<Totals>`, `parsed() -> Result<(NaiveDate, Vec<(FieldId, Cents)>)>`.
  - `enum FieldFocus { Name, Kind }`, `struct FieldForm { editing: Option<FieldId>, name: TextBuffer, kind: Kind, focus: FieldFocus }` with `add()`, `edit(&Field)`, `on_key(KeyEvent) -> Outcome`.
  - `test_support`: `day(y, m, d)`, `key(KeyCode)`, `shift(KeyCode)`, `ctrl(char)`, `const STUB: &[(&str, i64)]`, `paycheck(id, date, fields: &[Field], amounts: &[(&str, i64)]) -> Paycheck`.

- [ ] **Step 1: Write `test_support`**

`src/tui/test_support.rs`:

```rust
//! Helpers shared by the `tui` tests.

use crate::db::{Field, Paycheck, PaycheckId};
use crate::money::Cents;
use chrono::NaiveDate;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Invented amounts: a salary and three deductions.
pub(super) const STUB: &[(&str, i64)] = &[
    ("Salary", 400_000),
    ("Federal Tax", 60_000),
    ("Social Security", 24_800),
    ("Medicare", 5_800),
];

pub(super) fn day(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

pub(super) fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

pub(super) fn shift(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::SHIFT)
}

pub(super) fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

/// A paycheck whose amounts are named by field name.
pub(super) fn paycheck(
    id: PaycheckId,
    date: NaiveDate,
    fields: &[Field],
    amounts: &[(&str, i64)],
) -> Paycheck {
    let id_of = |name: &str| fields.iter().find(|f| f.name == name).unwrap().id;
    Paycheck {
        id,
        date,
        amounts: amounts.iter().map(|&(n, c)| (id_of(n), Cents(c))).collect(),
    }
}
```

`src/tui/mod.rs` gains `mod form;` and `#[cfg(test)] mod test_support;`.

- [ ] **Step 2: Write the failing tests**

`src/tui/form.rs`:

```rust
//! The modal forms: a paycheck's date and amounts, and a field's name and kind.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::tui::test_support::{STUB, ctrl, day, key, paycheck, shift};

    fn today() -> NaiveDate {
        day(2026, 1, 20)
    }

    fn fields() -> Vec<Field> {
        db::open_in_memory().unwrap().fields().unwrap()
    }

    fn latest(fields: &[Field]) -> Paycheck {
        paycheck(1, day(2026, 1, 16), fields, STUB)
    }

    fn type_into(form: &mut PaycheckForm, text: &str) {
        for c in text.chars() {
            form.on_key(key(KeyCode::Char(c)));
        }
    }

    fn amount<'a>(form: &'a PaycheckForm, name: &str) -> &'a str {
        form.amounts.iter().find(|a| a.name == name).unwrap().text.value()
    }

    #[test]
    fn an_iso_date_parses() {
        assert_eq!(parse_date(" 2026-01-16 ", today()).unwrap(), day(2026, 1, 16));
    }

    #[test]
    fn m_d_shorthand_takes_this_year_when_the_month_is_not_behind() {
        assert_eq!(parse_date("1/30", today()).unwrap(), day(2026, 1, 30));
        assert_eq!(parse_date("1/2", today()).unwrap(), day(2026, 1, 2));
        assert_eq!(parse_date("3/1", today()).unwrap(), day(2026, 3, 1));
    }

    #[test]
    fn m_d_typed_in_late_december_for_january_is_next_year() {
        assert_eq!(parse_date("1/2", day(2026, 12, 30)).unwrap(), day(2027, 1, 2));
    }

    #[test]
    fn text_that_is_not_a_date_is_refused() {
        for bad in ["", "soon", "13/1", "2/30", "2026-02-30", "2026/01/16"] {
            assert!(parse_date(bad, today()).is_err(), "{bad:?} parsed");
        }
    }

    #[test]
    fn stepping_a_month_clamps_the_day() {
        assert_eq!(step_date(day(2026, 1, 31), Step::Months(1)), Some(day(2026, 2, 28)));
        assert_eq!(step_date(day(2026, 3, 31), Step::Months(-1)), Some(day(2026, 2, 28)));
    }

    #[test]
    fn adding_prefills_the_date_fourteen_days_after_the_latest_paycheck() {
        let fields = fields();
        let form = PaycheckForm::add(&fields, Some(&latest(&fields)), today());
        assert_eq!(form.date.value(), "2026-01-30");
        assert_eq!(form.focus, 0);
        assert_eq!(form.editing, None);
    }

    #[test]
    fn adding_with_no_paychecks_uses_today_and_blank_amounts() {
        let form = PaycheckForm::add(&fields(), None, today());
        assert_eq!(form.date.value(), "2026-01-20");
        assert_eq!(form.amounts.len(), 10);
        assert!(form.amounts.iter().all(|a| a.text.value().is_empty()));
    }

    #[test]
    fn adding_prefills_active_fields_from_the_latest_paycheck_and_leaves_the_rest_blank() {
        let mut fields = fields();
        let latest = paycheck(1, day(2026, 1, 16), &fields, &[("Salary", 400_000), ("HSA", 10_000)]);
        fields.iter_mut().find(|f| f.name == "HSA").unwrap().archived = true;
        let form = PaycheckForm::add(&fields, Some(&latest), today());
        assert_eq!(amount(&form, "Salary"), "4,000.00");
        assert_eq!(amount(&form, "State Tax"), "");
        assert!(form.amounts.iter().all(|a| a.name != "HSA"));
        let salary = &form.amounts[0].text;
        assert_eq!(salary.caret(), salary.len());
    }

    #[test]
    fn editing_shows_an_archived_field_the_paycheck_used_and_active_fields_it_lacks() {
        let mut fields = fields();
        let check = paycheck(7, day(2026, 1, 16), &fields, &[("Salary", 400_000), ("HSA", 10_000)]);
        fields.iter_mut().find(|f| f.name == "HSA").unwrap().archived = true;
        let form = PaycheckForm::edit(&fields, &check, today());
        assert_eq!(form.editing, Some(7));
        assert_eq!(form.date.value(), "2026-01-16");
        assert_eq!(amount(&form, "HSA"), "100.00");
        assert_eq!(amount(&form, "Medicare"), "");
        assert_eq!(form.amounts[0].name, "Salary");
        assert_eq!(form.amounts.len(), 10);
    }

    #[test]
    fn tab_and_shift_tab_wrap_at_both_ends() {
        let mut form = PaycheckForm::add(&fields(), None, today());
        form.on_key(key(KeyCode::BackTab));
        assert_eq!(form.focus, 10);
        form.on_key(key(KeyCode::Tab));
        assert_eq!(form.focus, 0);
        form.on_key(key(KeyCode::Tab));
        assert_eq!(form.focus, 1);
    }

    #[test]
    fn arrows_step_the_date_a_day_and_shift_arrows_a_week() {
        let mut form = PaycheckForm::add(&fields(), None, today());
        form.on_key(key(KeyCode::Right));
        assert_eq!(form.date.value(), "2026-01-21");
        form.on_key(shift(KeyCode::Left));
        assert_eq!(form.date.value(), "2026-01-14");
    }

    #[test]
    fn brackets_step_the_date_a_month() {
        let mut form = PaycheckForm::add(&fields(), None, today());
        form.on_key(key(KeyCode::Char(']')));
        assert_eq!(form.date.value(), "2026-02-20");
        form.on_key(key(KeyCode::Char('[')));
        form.on_key(key(KeyCode::Char('[')));
        assert_eq!(form.date.value(), "2025-12-20");
    }

    #[test]
    fn stepping_does_nothing_while_the_date_is_blank_or_invalid() {
        let mut form = PaycheckForm::add(&fields(), None, today());
        form.on_key(ctrl('u'));
        form.on_key(key(KeyCode::Right));
        assert_eq!(form.date.value(), "");
        type_into(&mut form, "x");
        form.on_key(key(KeyCode::Char(']')));
        assert_eq!(form.date.value(), "x");
    }

    #[test]
    fn leaving_the_date_shows_it_in_iso_form() {
        let mut form = PaycheckForm::add(&fields(), None, today());
        form.on_key(ctrl('u'));
        type_into(&mut form, "2/13");
        assert_eq!(form.date.value(), "2/13");
        form.on_key(key(KeyCode::Tab));
        assert_eq!(form.date.value(), "2026-02-13");
    }

    #[test]
    fn leaving_an_invalid_date_keeps_the_raw_text() {
        let mut form = PaycheckForm::add(&fields(), None, today());
        form.on_key(ctrl('u'));
        type_into(&mut form, "2/30");
        form.on_key(key(KeyCode::Tab));
        assert_eq!(form.date.value(), "2/30");
    }

    #[test]
    fn arrows_move_the_caret_in_an_amount() {
        let fields = fields();
        let mut form = PaycheckForm::add(&fields, Some(&latest(&fields)), today());
        form.on_key(key(KeyCode::Tab));
        form.on_key(key(KeyCode::Left));
        form.on_key(key(KeyCode::Left));
        type_into(&mut form, "5");
        assert_eq!(amount(&form, "Salary"), "4,000.500");
    }

    #[test]
    fn a_blank_amount_saves_as_zero() {
        let form = PaycheckForm::add(&fields(), None, today());
        let (date, amounts) = form.parsed().unwrap();
        assert_eq!(date, today());
        assert_eq!(amounts.len(), 10);
        assert!(amounts.iter().all(|&(_, c)| c == Cents::ZERO));
    }

    #[test]
    fn a_bad_amount_is_reported_with_its_field_name() {
        let mut form = PaycheckForm::add(&fields(), None, today());
        form.on_key(key(KeyCode::Tab));
        type_into(&mut form, "abc");
        let err = form.parsed().unwrap_err();
        assert_eq!(format!("{err:#}"), "Salary: not a monetary amount: \"abc\"");
    }

    #[test]
    fn the_live_totals_follow_the_input_and_vanish_on_a_bad_amount() {
        let fields = fields();
        let mut form = PaycheckForm::add(&fields, Some(&latest(&fields)), today());
        assert_eq!(
            form.totals(),
            Some(Totals { income: Cents(400_000), net: Cents(309_400) })
        );
        form.on_key(key(KeyCode::Tab));
        type_into(&mut form, "x");
        assert_eq!(form.totals(), None);
    }

    #[test]
    fn escape_cancels_and_enter_submits() {
        let mut form = PaycheckForm::add(&fields(), None, today());
        assert_eq!(form.on_key(key(KeyCode::Char('1'))), Outcome::Continue);
        assert_eq!(form.on_key(key(KeyCode::Enter)), Outcome::Submit);
        assert_eq!(form.on_key(key(KeyCode::Esc)), Outcome::Cancel);
    }

    #[test]
    fn a_field_form_defaults_to_a_deduction_and_arrows_cycle_the_kind() {
        let mut form = FieldForm::add();
        assert_eq!((form.kind, form.focus), (Kind::Deduction, FieldFocus::Name));
        form.on_key(key(KeyCode::Tab));
        assert_eq!(form.focus, FieldFocus::Kind);
        form.on_key(key(KeyCode::Left));
        assert_eq!(form.kind, Kind::Income);
        form.on_key(key(KeyCode::Right));
        assert_eq!(form.kind, Kind::Deduction);
        form.on_key(key(KeyCode::BackTab));
        assert_eq!(form.focus, FieldFocus::Name);
    }

    #[test]
    fn a_field_form_types_into_the_name_and_arrows_move_its_caret() {
        let fields = fields();
        let mut form = FieldForm::edit(&fields[7]);
        assert_eq!((form.editing, form.name.value()), (Some(fields[7].id), "HSA"));
        form.on_key(key(KeyCode::Left));
        form.on_key(key(KeyCode::Char('x')));
        assert_eq!(form.name.value(), "HSxA");
        assert_eq!(form.on_key(key(KeyCode::Enter)), Outcome::Submit);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test tui::form`
Expected: compile errors — `parse_date`, `PaycheckForm`, etc. not found.

- [ ] **Step 4: Implement**

Above the tests in `src/tui/form.rs`:

```rust
use super::text::{TextBuffer, edit_key, is_bare};
use crate::calc::{self, Totals};
use crate::db::{Field, FieldId, Kind, Paycheck, PaycheckId};
use crate::money::Cents;
use anyhow::{Context, Result, anyhow};
use chrono::{Datelike, Months, NaiveDate, TimeDelta};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// `YYYY-MM-DD`, or MisterManager's `M/D` shorthand.
pub(super) fn parse_date(raw: &str, today: NaiveDate) -> Result<NaiveDate> {
    let raw = raw.trim();
    if raw.contains('/') {
        return parse_shorthand(raw, today);
    }
    NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .with_context(|| format!("not a YYYY-MM-DD or M/D date: {raw:?}"))
}

/// `M/D` -- a month and a day, taking the next year that month occurs in.
///
/// The year turns on the month alone: `1/2` typed on January 20th is this
/// January, a backdated entry, while `1/2` typed in December is next year's.
fn parse_shorthand(raw: &str, today: NaiveDate) -> Result<NaiveDate> {
    let malformed = || anyhow!("not a M/D date: {raw:?}");
    let (month, day) = raw.split_once('/').ok_or_else(malformed)?;
    let month: u32 = month.trim().parse().map_err(|_| malformed())?;
    let day: u32 = day.trim().parse().map_err(|_| malformed())?;
    let year = if month >= today.month() {
        today.year()
    } else {
        today.year() + 1
    };
    NaiveDate::from_ymd_opt(year, month, day).ok_or_else(|| anyhow!("no such date: {raw:?}"))
}

pub(super) fn iso(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum Step {
    Days(i64),
    /// A month's step clamps the day to the end of a shorter month.
    Months(i32),
}

pub(super) fn step_date(date: NaiveDate, step: Step) -> Option<NaiveDate> {
    match step {
        Step::Days(n) => date.checked_add_signed(TimeDelta::days(n)),
        Step::Months(n) if n >= 0 => date.checked_add_months(Months::new(n.unsigned_abs())),
        Step::Months(n) => date.checked_sub_months(Months::new(n.unsigned_abs())),
    }
}

/// The date keys: `←`/`→` a day, with `Shift` a week, and `[`/`]` a month.
fn date_step(key: KeyEvent) -> Option<Step> {
    if !is_bare(key) {
        return None;
    }
    let week = key.modifiers.contains(KeyModifiers::SHIFT);
    match key.code {
        KeyCode::Left => Some(Step::Days(if week { -7 } else { -1 })),
        KeyCode::Right => Some(Step::Days(if week { 7 } else { 1 })),
        KeyCode::Char('[') => Some(Step::Months(-1)),
        KeyCode::Char(']') => Some(Step::Months(1)),
        _ => None,
    }
}

/// A blank amount is zero.
fn parse_amount(raw: &str) -> Result<Cents> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(Cents::ZERO);
    }
    Ok(raw.parse::<Cents>()?)
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Continue,
    Submit,
    Cancel,
}

pub(super) struct AmountInput {
    pub(super) field_id: FieldId,
    pub(super) name: String,
    pub(super) kind: Kind,
    pub(super) text: TextBuffer,
}

impl AmountInput {
    fn new(field: &Field, cents: Option<Cents>) -> Self {
        Self {
            field_id: field.id,
            name: field.name.clone(),
            kind: field.kind,
            text: cents.map_or_else(TextBuffer::default, |c| TextBuffer::from(c.to_string())),
        }
    }
}

pub(super) struct PaycheckForm {
    pub(super) editing: Option<PaycheckId>,
    pub(super) date: TextBuffer,
    pub(super) amounts: Vec<AmountInput>,
    /// `0` is the date; `n` is `amounts[n - 1]`.
    pub(super) focus: usize,
    today: NaiveDate,
}

impl PaycheckForm {
    /// The active fields, prefilled from `latest`, dated two weeks after it.
    pub(super) fn add(fields: &[Field], latest: Option<&Paycheck>, today: NaiveDate) -> Self {
        let date = latest
            .and_then(|p| step_date(p.date, Step::Days(14)))
            .unwrap_or(today);
        let mut shown: Vec<&Field> = fields.iter().filter(|f| !f.archived).collect();
        calc::sort_rows(&mut shown);
        let amounts = shown
            .into_iter()
            .map(|f| AmountInput::new(f, latest.and_then(|p| p.amounts.get(&f.id).copied())))
            .collect();
        Self { editing: None, date: TextBuffer::from(iso(date)), amounts, focus: 0, today }
    }

    /// The paycheck's own fields, archived ones included, plus any active
    /// field it lacks.
    pub(super) fn edit(fields: &[Field], paycheck: &Paycheck, today: NaiveDate) -> Self {
        let mut shown: Vec<&Field> = fields
            .iter()
            .filter(|f| !f.archived || paycheck.amounts.contains_key(&f.id))
            .collect();
        calc::sort_rows(&mut shown);
        let amounts = shown
            .into_iter()
            .map(|f| AmountInput::new(f, paycheck.amounts.get(&f.id).copied()))
            .collect();
        Self {
            editing: Some(paycheck.id),
            date: TextBuffer::from(iso(paycheck.date)),
            amounts,
            focus: 0,
            today,
        }
    }

    pub(super) fn on_key(&mut self, key: KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Esc => return Outcome::Cancel,
            KeyCode::Enter => {
                self.normalize_date();
                return Outcome::Submit;
            }
            KeyCode::Tab => {
                self.move_focus(1);
                return Outcome::Continue;
            }
            KeyCode::BackTab => {
                self.move_focus(-1);
                return Outcome::Continue;
            }
            _ => {}
        }
        if self.focus == 0 {
            match date_step(key) {
                Some(step) => self.step(step),
                None => {
                    edit_key(&mut self.date, key);
                }
            }
        } else {
            let text = &mut self.amounts[self.focus - 1].text;
            match key.code {
                KeyCode::Left if is_bare(key) => text.step(-1),
                KeyCode::Right if is_bare(key) => text.step(1),
                _ => {
                    edit_key(text, key);
                }
            }
        }
        Outcome::Continue
    }

    /// Net and income over the typed amounts, or `None` while any fails to parse.
    pub(super) fn totals(&self) -> Option<Totals> {
        let amounts: Option<Vec<(Kind, Cents)>> = self
            .amounts
            .iter()
            .map(|a| parse_amount(a.text.value()).ok().map(|c| (a.kind, c)))
            .collect();
        amounts.map(calc::totals)
    }

    /// The date and one amount per field shown, ready to save.
    pub(super) fn parsed(&self) -> Result<(NaiveDate, Vec<(FieldId, Cents)>)> {
        let date = parse_date(self.date.value(), self.today)?;
        let amounts = self
            .amounts
            .iter()
            .map(|a| {
                let cents = parse_amount(a.text.value()).with_context(|| a.name.clone())?;
                Ok((a.field_id, cents))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((date, amounts))
    }

    fn step(&mut self, step: Step) {
        if let Ok(date) = parse_date(self.date.value(), self.today)
            && let Some(next) = step_date(date, step)
        {
            self.date.set(iso(next));
        }
    }

    fn move_focus(&mut self, by: isize) {
        if self.focus == 0 {
            self.normalize_date();
        }
        let stops = (self.amounts.len() + 1) as isize;
        self.focus = (self.focus as isize + by).rem_euclid(stops) as usize;
    }

    /// Show the date in ISO form once it parses; leave text that does not.
    fn normalize_date(&mut self) {
        if let Ok(date) = parse_date(self.date.value(), self.today) {
            self.date.set(iso(date));
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum FieldFocus {
    Name,
    Kind,
}

pub(super) struct FieldForm {
    pub(super) editing: Option<FieldId>,
    pub(super) name: TextBuffer,
    pub(super) kind: Kind,
    pub(super) focus: FieldFocus,
}

impl FieldForm {
    pub(super) fn add() -> Self {
        Self {
            editing: None,
            name: TextBuffer::default(),
            kind: Kind::Deduction,
            focus: FieldFocus::Name,
        }
    }

    pub(super) fn edit(field: &Field) -> Self {
        Self {
            editing: Some(field.id),
            name: TextBuffer::from(field.name.as_str()),
            kind: field.kind,
            focus: FieldFocus::Name,
        }
    }

    pub(super) fn on_key(&mut self, key: KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Esc => return Outcome::Cancel,
            KeyCode::Enter => return Outcome::Submit,
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = match self.focus {
                    FieldFocus::Name => FieldFocus::Kind,
                    FieldFocus::Kind => FieldFocus::Name,
                };
                return Outcome::Continue;
            }
            _ => {}
        }
        match self.focus {
            FieldFocus::Name => match key.code {
                KeyCode::Left if is_bare(key) => self.name.step(-1),
                KeyCode::Right if is_bare(key) => self.name.step(1),
                _ => {
                    edit_key(&mut self.name, key);
                }
            },
            FieldFocus::Kind => {
                if matches!(key.code, KeyCode::Left | KeyCode::Right) && is_bare(key) {
                    self.kind = self.kind.other();
                }
            }
        }
        Outcome::Continue
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test tui:: && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: all pass, clean.

- [ ] **Step 6: Commit**

Message: `feat: add paycheck and field forms with date parsing and stepping`

---

### Task 8: `tui::sheet` rendering

**Files:**
- Create: `src/tui/sheet.rs`
- Modify: `src/tui/mod.rs` (add `mod sheet;`), `src/tui/test_support.rs` (add drawing helpers)

**Interfaces:**
- Consumes: `calc::{self, Sheet}`, `Cents`.
- Produces:
  - `pub(super) struct SheetView { pub(super) year: i32, pub(super) selected: usize, scroll: usize }` with `pub(super) fn new(year: i32, selected: usize) -> Self`.
  - `pub(super) fn render(frame: &mut Frame, area: Rect, view: &mut SheetView, sheet: &Sheet)` — adjusts `view.scroll` to keep `view.selected` visible.
  - Empty-year hint text: `"No paychecks in {year}. Press a to add one."`
  - `test_support`: `draw_buffer(width, height, impl FnOnce(&mut Frame)) -> Buffer`, `draw(width, height, impl FnOnce(&mut Frame)) -> String`, `buffer_text(&Buffer) -> String` (lines right-trimmed, joined by `\n`).

- [ ] **Step 1: Add the drawing helpers**

Append to `src/tui/test_support.rs` (and extend its imports with `ratatui::{Frame, Terminal, backend::TestBackend, buffer::Buffer}`):

```rust
pub(super) fn draw_buffer(width: u16, height: u16, render: impl FnOnce(&mut Frame)) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(render).unwrap();
    terminal.backend().buffer().clone()
}

pub(super) fn draw(width: u16, height: u16, render: impl FnOnce(&mut Frame)) -> String {
    buffer_text(&draw_buffer(width, height, render))
}

/// The buffer as text, one line per row with trailing spaces trimmed.
pub(super) fn buffer_text(buffer: &Buffer) -> String {
    (0..buffer.area.height)
        .map(|y| {
            let line: String = (0..buffer.area.width).map(|x| buffer[(x, y)].symbol()).collect();
            line.trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
```

- [ ] **Step 2: Write the failing tests**

`src/tui/sheet.rs`:

```rust
//! The Sheet screen: a year's paychecks as columns, fields as rows.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{self, Field, Paycheck};
    use crate::tui::test_support::{day, draw, paycheck};

    const PAY: &[(&str, i64)] = &[("Salary", 400_000), ("Federal Tax", 60_000)];

    /// "Social Security" is the longest label, so labels are 15 + 2 wide.
    const LABEL: usize = 17;

    fn fields() -> Vec<Field> {
        db::open_in_memory().unwrap().fields().unwrap()
    }

    fn checks(fields: &[Field], dates: &[(u32, u32)]) -> Vec<Paycheck> {
        dates
            .iter()
            .enumerate()
            .map(|(i, &(m, d))| paycheck(i as i64 + 1, day(2026, m, d), fields, PAY))
            .collect()
    }

    fn drawn(width: u16, view: &mut SheetView, fields: &[Field], checks: &[Paycheck]) -> Vec<String> {
        let sheet = calc::sheet(view.year, fields, checks);
        draw(width, 30, |frame| {
            let area = frame.area();
            render(frame, area, view, &sheet)
        })
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn line<'a>(lines: &'a [String], label: &str) -> &'a str {
        lines.iter().find(|l| l.starts_with(label)).unwrap()
    }

    #[test]
    fn the_header_marks_the_selected_paycheck_and_ends_with_ytd() {
        let fields = fields();
        let mut view = SheetView::new(2026, 1);
        let lines = drawn(80, &mut view, &fields, &checks(&fields, &[(1, 2), (1, 16)]));
        assert_eq!(
            lines[0],
            format!("{:<LABEL$} {:>10} {:>10} │ {:>10}", "2026", "01-02", "◀ 01-16 ▶", "YTD")
        );
    }

    #[test]
    fn amount_rows_show_each_paycheck_and_the_ytd_total() {
        let fields = fields();
        let mut view = SheetView::new(2026, 1);
        let lines = drawn(80, &mut view, &fields, &checks(&fields, &[(1, 2), (1, 16)]));
        assert_eq!(lines[1], format!("{:<LABEL$} {:>10} {:>10} │ {:>10}", "Salary", "4,000.00", "4,000.00", "8,000.00"));
        assert_eq!(
            line(&lines, "Social Security"),
            format!("{:<LABEL$} {:>10} {:>10} │ {:>10}", "Social Security", "", "", "0.00")
        );
    }

    #[test]
    fn a_rule_then_net_follow_the_amounts() {
        let fields = fields();
        let mut view = SheetView::new(2026, 1);
        let lines = drawn(80, &mut view, &fields, &checks(&fields, &[(1, 2), (1, 16)]));
        assert_eq!(lines[11], format!("{}┼{}", "─".repeat(LABEL + 2 * 11 + 1), "─".repeat(11)));
        assert_eq!(lines[12], format!("{:<LABEL$} {:>10} {:>10} │ {:>10}", "Net", "3,400.00", "3,400.00", "6,800.00"));
        assert_eq!(lines[13], "");
    }

    #[test]
    fn the_percent_block_lists_deductions_then_net_pay() {
        let fields = fields();
        let mut view = SheetView::new(2026, 1);
        let lines = drawn(80, &mut view, &fields, &checks(&fields, &[(1, 2), (1, 16)]));
        assert_eq!(lines[14], format!("{:<LABEL$} {:>10} {:>10} │ {:>10}", "Federal Tax", "15.00%", "15.00%", "15.00%"));
        assert_eq!(lines[23], format!("{:<LABEL$} {:>10} {:>10} │ {:>10}", "Net Pay", "85.00%", "85.00%", "85.00%"));
    }

    #[test]
    fn a_year_with_no_paychecks_shows_the_labels_and_a_hint() {
        let fields = fields();
        let mut view = SheetView::new(2027, 0);
        let lines = drawn(80, &mut view, &fields, &checks(&fields, &[(1, 2)]));
        assert_eq!(lines[0], format!("{:<LABEL$} │ {:>10}", "2027", "YTD"));
        assert_eq!(line(&lines, "Net Pay"), format!("{:<LABEL$} │ {:>10}", "Net Pay", "—"));
        assert!(lines.contains(&"No paychecks in 2027. Press a to add one.".to_string()));
    }

    #[test]
    fn a_narrow_terminal_scrolls_to_keep_the_selected_paycheck_visible() {
        let fields = fields();
        let checks = checks(&fields, &[(1, 2), (1, 16), (1, 30)]);
        let mut view = SheetView::new(2026, 2);
        let lines = drawn(41, &mut view, &fields, &checks);
        assert_eq!(lines[0], format!("{:<LABEL$} {:>10} │ {:>10}", "2026", "◀ 01-30 ▶", "YTD"));
        view.selected = 0;
        let lines = drawn(41, &mut view, &fields, &checks);
        assert_eq!(lines[0], format!("{:<LABEL$} {:>10} │ {:>10}", "2026", "◀ 01-02 ▶", "YTD"));
    }

    #[test]
    fn a_terminal_narrower_than_the_fixed_columns_still_draws() {
        let fields = fields();
        let mut view = SheetView::new(2026, 0);
        let lines = drawn(12, &mut view, &fields, &checks(&fields, &[(1, 2)]));
        assert!(lines[1].starts_with("Salary"));
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test tui::sheet`
Expected: compile errors — `SheetView`, `render` not found.

- [ ] **Step 4: Implement**

`src/tui/mod.rs` gains `mod sheet;`. Above the tests in `src/tui/sheet.rs`:

```rust
use crate::calc::{self, Sheet};
use crate::money::Cents;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

/// A paycheck column's width: room for `-99,999.99` and for the selected
/// header's `◀ 01-30 ▶`.
const COL: usize = 10;

#[derive(Debug)]
pub(super) struct SheetView {
    pub(super) year: i32,
    /// Index into the year's paychecks, oldest first.
    pub(super) selected: usize,
    /// The first paycheck column drawn.
    scroll: usize,
}

impl SheetView {
    pub(super) fn new(year: i32, selected: usize) -> Self {
        Self { year, selected, scroll: 0 }
    }
}

/// Draw `sheet`: fixed label and YTD columns, and as many paycheck columns
/// between them as fit, scrolled so the selected one shows.
pub(super) fn render(frame: &mut Frame, area: Rect, view: &mut SheetView, sheet: &Sheet) {
    let label_w = sheet
        .rows
        .iter()
        .map(|r| r.name.as_str())
        .chain(sheet.percent_rows.iter().map(|r| r.label.as_str()))
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0)
        .max("Net Pay".len())
        + 2;
    let room = usize::from(area.width).saturating_sub(label_w + 3 + COL);
    let visible = (room / (COL + 1)).max(1);
    if view.selected < view.scroll {
        view.scroll = view.selected;
    }
    if view.selected >= view.scroll + visible {
        view.scroll = view.selected + 1 - visible;
    }
    let len = sheet.columns.len();
    let start = view.scroll.min(len);
    let end = (start + visible).min(len);

    let row = |label: &str, cells: Vec<String>, ytd: String| {
        let mut text = format!("{label:<label_w$}");
        for cell in cells {
            text.push_str(&format!(" {cell:>COL$}"));
        }
        text.push_str(&format!(" │ {ytd:>COL$}"));
        Line::from(text)
    };
    let amounts = |cells: &[Option<Cents>]| -> Vec<String> {
        cells[start..end]
            .iter()
            .map(|c| c.map_or_else(String::new, |c| c.to_string()))
            .collect()
    };

    let header = sheet.columns[start..end]
        .iter()
        .enumerate()
        .map(|(i, col)| {
            let date = col.date.format("%m-%d").to_string();
            if start + i == view.selected {
                format!("◀ {date} ▶")
            } else {
                date
            }
        })
        .collect();
    let mut lines = vec![row(&sheet.year.to_string(), header, "YTD".to_string())];
    for r in &sheet.rows {
        lines.push(row(&r.name, amounts(&r.cells), r.ytd.to_string()));
    }
    let bar = label_w + (end - start) * (COL + 1) + 1;
    lines.push(Line::from(format!("{}┼{}", "─".repeat(bar), "─".repeat(COL + 1))));
    lines.push(row(
        "Net",
        sheet.net[start..end].iter().map(Cents::to_string).collect(),
        sheet.net_ytd.to_string(),
    ));
    lines.push(Line::default());
    for r in &sheet.percent_rows {
        let cells = r.cells[start..end].iter().map(|p| calc::show(*p)).collect();
        lines.push(row(&r.label, cells, calc::show(r.ytd)));
    }
    if len == 0 {
        lines.push(Line::default());
        lines.push(Line::from(format!(
            "No paychecks in {}. Press a to add one.",
            sheet.year
        )));
    }
    frame.render_widget(Paragraph::new(lines), area);
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test tui:: && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: all pass, clean. If a line assertion is off by a space, fix `render`, not the test's `format!` layout — the test's layout is the spec's.

- [ ] **Step 6: Commit**

Message: `feat: render the Sheet with scrolling paycheck columns`

---

### Task 9: `tui::fields` rendering

**Files:**
- Create: `src/tui/fields.rs`
- Modify: `src/tui/mod.rs` (add `mod fields;`)

**Interfaces:**
- Consumes: `db::Field`, `Kind::label`.
- Produces: `#[derive(Debug, Default)] pub(super) struct FieldsView { pub(super) selected: usize }`, `pub(super) fn render(frame: &mut Frame, area: Rect, view: &FieldsView, fields: &[Field])`.
- Row format: `"{marker} {name:<w}  {kind:<9}{tag}"` where marker is `›` when selected else space, and tag is `"  archived"` for archived fields. Archived rows are `DIM`; the selected row is `REVERSED`.

- [ ] **Step 1: Write the failing tests**

`src/tui/fields.rs`:

```rust
//! The Fields screen: every field in `position` order.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::tui::test_support::{buffer_text, draw_buffer};

    #[test]
    fn each_field_shows_its_name_and_kind_with_the_selection_marked() {
        let fields = db::open_in_memory().unwrap().fields().unwrap();
        let view = FieldsView { selected: 1 };
        let text = buffer_text(&draw_buffer(60, 12, |f| { let area = f.area(); render(f, area, &view, &fields) }));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], format!("  {:<15}  Income", "Salary"));
        assert_eq!(lines[1], format!("› {:<15}  Deduction", "Federal Tax"));
    }

    #[test]
    fn archived_fields_are_tagged_and_dimmed_here() {
        let db = db::open_in_memory().unwrap();
        db.set_archived(db.field_id("HSA"), true).unwrap();
        let fields = db.fields().unwrap();
        let view = FieldsView::default();
        let buffer = draw_buffer(60, 12, |f| { let area = f.area(); render(f, area, &view, &fields) });
        let text = buffer_text(&buffer);
        let row = text.lines().position(|l| l.contains("HSA")).unwrap();
        assert!(text.lines().nth(row).unwrap().ends_with("Deduction  archived"));
        assert!(buffer[(2, row as u16)].modifier.contains(Modifier::DIM));
        assert!(!buffer[(2, 1)].modifier.contains(Modifier::DIM));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test tui::fields`
Expected: compile errors — `FieldsView`, `render` not found.

- [ ] **Step 3: Implement**

`src/tui/mod.rs` gains `mod fields;`. Above the tests:

```rust
use crate::db::Field;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

#[derive(Debug, Default)]
pub(super) struct FieldsView {
    pub(super) selected: usize,
}

/// Archived fields are dimmed here and nowhere else: this is the one screen
/// whose subject is whether a field is in use.
pub(super) fn render(frame: &mut Frame, area: Rect, view: &FieldsView, fields: &[Field]) {
    let name_w = fields.iter().map(|f| f.name.chars().count()).max().unwrap_or(0);
    let lines: Vec<Line> = fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let marker = if i == view.selected { "›" } else { " " };
            let tag = if f.archived { "  archived" } else { "" };
            let text = format!("{marker} {:<name_w$}  {:<9}{tag}", f.name, f.kind.label());
            let mut style = Style::new();
            if f.archived {
                style = style.add_modifier(Modifier::DIM);
            }
            if i == view.selected {
                style = style.add_modifier(Modifier::REVERSED);
            }
            Line::styled(text, style)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test tui:: && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: all pass, clean.

- [ ] **Step 5: Commit**

Message: `feat: render the Fields list with archived fields dimmed`

---

### Task 10: `tui::help`

**Files:**
- Create: `src/tui/help.rs`
- Modify: `src/tui/mod.rs` (add `mod help;` and `pub(super) fn centered`)

**Interfaces:**
- Produces:
  - `pub(super) struct Entry { pub(super) key: &'static str, pub(super) word: Option<&'static str>, pub(super) detail: &'static str }`
  - Tables (`pub(super) const … : &[Entry]`): `GLOBAL`, `SHEET`, `FIELDS`, `PAYCHECK_FORM`, `FIELD_FORM`, `CONFIRM`, `HELP`.
  - `pub(super) fn footer(tables: &[&[Entry]]) -> String` — `"{key} {word}"` for every entry with a word, joined by two spaces.
  - `pub(super) fn render(frame: &mut Frame, area: Rect, topics: &[(&str, &[Entry])])` — bordered panel titled ` Help `.
  - In `tui/mod.rs`: `pub(super) fn centered(area: Rect, width: u16, height: u16) -> Rect`.

- [ ] **Step 1: Write the failing tests**

`src/tui/help.rs`:

```rust
//! What every key does. The footers are joined from these same tables, so a
//! footer cannot drift from the panel that explains it.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::test_support::draw;

    const ALL: &[&[Entry]] = &[GLOBAL, SHEET, FIELDS, PAYCHECK_FORM, FIELD_FORM, CONFIRM, HELP];

    #[test]
    fn the_footer_joins_the_keys_that_have_a_word() {
        assert_eq!(
            footer(&[SHEET, GLOBAL]),
            "←/→ paycheck  [ ] year  a add  e edit  d delete  1 sheet  2 fields  ? help  q quit"
        );
    }

    #[test]
    fn no_table_names_a_key_twice() {
        for table in ALL {
            let mut keys: Vec<_> = table.iter().map(|e| e.key).collect();
            keys.sort_unstable();
            keys.dedup();
            assert_eq!(keys.len(), table.len());
        }
    }

    #[test]
    fn the_same_action_uses_the_same_key_on_both_screens() {
        for (key, word) in [("a", "add"), ("e", "edit"), ("d", "delete")] {
            for table in [SHEET, FIELDS] {
                let entry = table.iter().find(|e| e.key == key).unwrap();
                assert_eq!(entry.word, Some(word));
            }
        }
    }

    #[test]
    fn the_panel_lists_each_topic_with_its_keys() {
        let text = draw(70, 24, |f| { let area = f.area(); render(f, area, &[("Sheet", SHEET), ("Everywhere", GLOBAL)]) });
        assert!(text.contains("Help"));
        assert!(text.contains("Sheet"));
        assert!(text.contains("Show the previous or next year"));
        assert!(text.contains("Everywhere"));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test tui::help`
Expected: compile errors.

- [ ] **Step 3: Implement**

Add to `src/tui/mod.rs` (with `use ratatui::layout::Rect;`) and `mod help;`:

```rust
/// A `width` × `height` rectangle centered in `area`, shrunk to fit it.
pub(super) fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}
```

Above the tests in `src/tui/help.rs`:

```rust
use super::centered;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph};

#[derive(Copy, Clone, Debug)]
pub(super) struct Entry {
    pub(super) key: &'static str,
    /// The footer word; `None` keeps the key out of the footer but in the panel.
    pub(super) word: Option<&'static str>,
    pub(super) detail: &'static str,
}

const fn entry(key: &'static str, word: Option<&'static str>, detail: &'static str) -> Entry {
    Entry { key, word, detail }
}

pub(super) const GLOBAL: &[Entry] = &[
    entry("1", Some("sheet"), "Show the Sheet"),
    entry("2", Some("fields"), "Show the Fields list"),
    entry("?", Some("help"), "Open this panel (F1 too)"),
    entry("q", Some("quit"), "Quit"),
];

pub(super) const SHEET: &[Entry] = &[
    entry("←/→", Some("paycheck"), "Select the previous or next paycheck"),
    entry("Home/End", None, "Select the first or last paycheck"),
    entry("[ ]", Some("year"), "Show the previous or next year"),
    entry("a", Some("add"), "Add a paycheck, prefilled from the latest one"),
    entry("e", Some("edit"), "Edit the selected paycheck"),
    entry("d", Some("delete"), "Delete the selected paycheck ('y' confirms)"),
];

pub(super) const FIELDS: &[Entry] = &[
    entry("↑/↓", None, "Select a field"),
    entry("Shift-↑/↓", Some("move"), "Move the selected field up or down"),
    entry("a", Some("add"), "Add a field at the end"),
    entry("e", Some("edit"), "Rename the selected field or change its kind"),
    entry("x", Some("archive"), "Archive or unarchive the selected field"),
    entry("d", Some("delete"), "Delete the selected field if no paycheck uses it ('y' confirms)"),
];

pub(super) const PAYCHECK_FORM: &[Entry] = &[
    entry("Tab", Some("next"), "Next field; Shift-Tab goes back"),
    entry("←/→", None, "Date: a day, or a week with Shift. Amount: move the caret"),
    entry("[ ]", None, "Date: a month"),
    entry("Ctrl+U", None, "Clear to the start of the field"),
    entry("Enter", Some("save"), "Save the paycheck"),
    entry("Esc", Some("cancel"), "Close without saving"),
];

pub(super) const FIELD_FORM: &[Entry] = &[
    entry("Tab", Some("next"), "Switch between Name and Kind"),
    entry("←/→", None, "Kind: switch between Income and Deduction"),
    entry("Enter", Some("save"), "Save the field"),
    entry("Esc", Some("cancel"), "Close without saving"),
];

pub(super) const CONFIRM: &[Entry] = &[entry("y", Some("confirm"), "Confirm; any other key cancels")];

pub(super) const HELP: &[Entry] = &[entry("Esc", Some("close"), "Close this panel (? and F1 too)")];

pub(super) fn footer(tables: &[&[Entry]]) -> String {
    tables
        .iter()
        .flat_map(|table| table.iter())
        .filter_map(|e| e.word.map(|w| format!("{} {w}", e.key)))
        .collect::<Vec<_>>()
        .join("  ")
}

pub(super) fn render(frame: &mut Frame, area: Rect, topics: &[(&str, &[Entry])]) {
    let key_w = topics
        .iter()
        .flat_map(|(_, entries)| entries.iter())
        .map(|e| e.key.chars().count())
        .max()
        .unwrap_or(0);
    let mut lines: Vec<Line> = Vec::new();
    for (title, entries) in topics {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        lines.push(Line::styled(title.to_string(), Style::new().add_modifier(Modifier::BOLD)));
        for e in *entries {
            lines.push(Line::from(format!("  {:<key_w$}  {}", e.key, e.detail)));
        }
    }
    let width = lines.iter().map(Line::width).max().unwrap_or(0) as u16 + 2;
    let popup = centered(area, width, lines.len() as u16 + 2);
    frame.render_widget(Clear, popup);
    frame.render_widget(Paragraph::new(lines).block(Block::bordered().title(" Help ")), popup);
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test tui:: && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: all pass, clean.

- [ ] **Step 5: Commit**

Message: `feat: add key tables, footers, and the help panel`

---

### Task 11: `App` core, event loop, and CLI

**Files:**
- Create: `src/tui/app.rs`
- Modify: `src/tui/mod.rs` (`mod app;`, `run`, event loop), `src/tui/test_support.rs`, `src/bin/pc.rs`
- Modify: spec (Sheet section)

**Interfaces:**
- Consumes: `Db::{fields, paychecks}`, `calc::sheet`, `SheetView`, `FieldsView`, `sheet::render`, `fields::render`, `help::{self, Entry}`, `is_bare`.
- Produces:
  - `pub fn tui::run(db: Db, today: NaiveDate) -> Result<()>`
  - `pub(super) const STATUS_TTL: Duration` (4 s)
  - `pub(super) enum Screen { Sheet, Fields }`
  - `pub(super) struct Status { pub(super) text: String, pub(super) error: bool, expires: Option<Instant> }`
  - `pub(super) struct App` with `pub(super)` fields `screen`, `sheet: SheetView`, `fields_view: FieldsView`, `help: bool`, `status: Option<Status>`, `fields: Vec<Field>`, `paychecks: Vec<Paycheck>`; private `db`, `today`, `quit`.
  - Methods: `new(Db, NaiveDate) -> Result<App>`, `should_quit()`, `on_key(KeyEvent)`, `expire_status() -> bool`, `expire_status_at(Instant) -> bool`, `render(&mut self, &mut Frame)`, private `info(String)`, `error(String)`, `reload()`, `year_paychecks()`, `selected_paycheck()`, `select_last_in_year()`, `show_year(i32)`.
  - `test_support`: `press(&mut App, KeyCode)`, `app_with(&[(NaiveDate, &[(&str, i64)])], today) -> App`, `screen(&mut App, width, height) -> String`.

- [ ] **Step 1: Add the `App` test helpers**

Append to `src/tui/test_support.rs` (imports: `super::app::App`, `crate::db::{self, FieldId}`):

```rust
pub(super) fn press(app: &mut App, code: KeyCode) {
    app.on_key(key(code));
}

/// An app on a freshly seeded in-memory database holding `paychecks`.
pub(super) fn app_with(paychecks: &[(NaiveDate, &[(&str, i64)])], today: NaiveDate) -> App {
    let db = db::open_in_memory().unwrap();
    for (date, amounts) in paychecks {
        let amounts: Vec<(FieldId, Cents)> = amounts
            .iter()
            .map(|&(name, cents)| (db.field_id(name), Cents(cents)))
            .collect();
        db.insert_paycheck(*date, &amounts).unwrap();
    }
    App::new(db, today).unwrap()
}

pub(super) fn screen(app: &mut App, width: u16, height: u16) -> String {
    draw(width, height, |frame| app.render(frame))
}
```

- [ ] **Step 2: Write the failing tests**

`src/tui/app.rs`:

```rust
//! `App`: which screen is showing, the status line, and where each key goes.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::test_support::{STUB, app_with, ctrl, day, key, press, screen};
    use ratatui::crossterm::event::KeyCode;

    /// One paycheck in 2025 and two in 2026.
    fn standard() -> App {
        app_with(
            &[(day(2025, 12, 19), STUB), (day(2026, 1, 2), STUB), (day(2026, 1, 16), STUB)],
            day(2026, 1, 20),
        )
    }

    #[test]
    fn the_sheet_opens_on_the_latest_paychecks_year_with_it_selected() {
        let app = standard();
        assert_eq!((app.screen, app.sheet.year, app.sheet.selected), (Screen::Sheet, 2026, 1));
    }

    #[test]
    fn with_no_paychecks_the_sheet_opens_on_todays_year_with_a_hint() {
        let mut app = app_with(&[], day(2026, 3, 1));
        assert_eq!(app.sheet.year, 2026);
        assert!(screen(&mut app, 80, 30).contains("No paychecks in 2026. Press a to add one."));
    }

    #[test]
    fn the_sheet_draws_the_years_paychecks() {
        let mut app = standard();
        let text = screen(&mut app, 80, 30);
        assert!(text.contains("◀ 01-16 ▶"));
        assert!(text.contains("8,000.00"));
        assert!(!text.contains("12-19"));
    }

    #[test]
    fn brackets_step_the_year_and_select_its_latest_paycheck() {
        let mut app = standard();
        press(&mut app, KeyCode::Char('['));
        assert_eq!((app.sheet.year, app.sheet.selected), (2025, 0));
        press(&mut app, KeyCode::Char(']'));
        press(&mut app, KeyCode::Char(']'));
        assert_eq!((app.sheet.year, app.sheet.selected), (2027, 0));
    }

    #[test]
    fn arrows_home_and_end_move_the_selection_within_the_year() {
        let mut app = standard();
        press(&mut app, KeyCode::Left);
        press(&mut app, KeyCode::Left);
        assert_eq!(app.sheet.selected, 0);
        press(&mut app, KeyCode::End);
        assert_eq!(app.sheet.selected, 1);
        press(&mut app, KeyCode::Right);
        assert_eq!(app.sheet.selected, 1);
        press(&mut app, KeyCode::Home);
        assert_eq!(app.sheet.selected, 0);
    }

    #[test]
    fn one_and_two_switch_screens_and_q_quits() {
        let mut app = standard();
        press(&mut app, KeyCode::Char('2'));
        assert_eq!(app.screen, Screen::Fields);
        assert!(screen(&mut app, 80, 30).contains("Social Security"));
        press(&mut app, KeyCode::Char('1'));
        assert_eq!(app.screen, Screen::Sheet);
        press(&mut app, KeyCode::Char('q'));
        assert!(app.should_quit());
    }

    #[test]
    fn a_ctrl_letter_is_not_an_app_key() {
        let mut app = standard();
        app.on_key(ctrl('q'));
        assert!(!app.should_quit());
    }

    #[test]
    fn a_status_message_replaces_the_footer_and_clears_at_the_next_key() {
        let mut app = standard();
        app.info("Hello".to_string());
        assert!(screen(&mut app, 80, 30).ends_with("Hello"));
        press(&mut app, KeyCode::Right);
        assert!(app.status.is_none());
    }

    #[test]
    fn a_status_message_expires_after_four_seconds() {
        let mut app = standard();
        app.info("Hello".to_string());
        assert!(!app.expire_status_at(Instant::now()));
        assert!(app.expire_status_at(Instant::now() + STATUS_TTL));
        assert!(app.status.is_none());
    }

    #[test]
    fn help_opens_with_question_mark_or_f1_and_closes_with_esc() {
        let mut app = standard();
        press(&mut app, KeyCode::Char('?'));
        assert!(app.help);
        let text = screen(&mut app, 100, 40);
        assert!(text.contains("Select the previous or next paycheck"));
        assert!(text.contains("Everywhere"));
        press(&mut app, KeyCode::Char('2'));
        assert_eq!(app.screen, Screen::Sheet);
        press(&mut app, KeyCode::Esc);
        assert!(!app.help);
        app.on_key(key(KeyCode::F(1)));
        assert!(app.help);
        app.on_key(key(KeyCode::F(1)));
        assert!(!app.help);
    }

    #[test]
    fn the_footer_is_built_from_the_screens_help_table() {
        let mut app = standard();
        let text = screen(&mut app, 120, 30);
        assert_eq!(text.lines().last().unwrap(), help::footer(&[help::SHEET, help::GLOBAL]));
        press(&mut app, KeyCode::Char('2'));
        let text = screen(&mut app, 120, 30);
        assert_eq!(text.lines().last().unwrap(), help::footer(&[help::FIELDS, help::GLOBAL]));
    }

    #[test]
    fn arrows_move_the_fields_selection() {
        let mut app = standard();
        press(&mut app, KeyCode::Char('2'));
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Down);
        assert_eq!(app.fields_view.selected, 2);
        press(&mut app, KeyCode::Up);
        assert_eq!(app.fields_view.selected, 1);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test tui::app`
Expected: compile errors — `App`, `Screen` not found.

- [ ] **Step 4: Implement `App`**

`src/tui/mod.rs` gains `mod app;`. Above the tests in `src/tui/app.rs`:

```rust
use super::fields::{self, FieldsView};
use super::help::{self, Entry};
use super::sheet::{self, SheetView};
use super::text::is_bare;
use crate::calc;
use crate::db::{Db, Field, Paycheck};
use anyhow::Result;
use chrono::{Datelike, NaiveDate};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::Paragraph;
use std::time::{Duration, Instant};

pub(super) const STATUS_TTL: Duration = Duration::from_secs(4);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum Screen {
    Sheet,
    Fields,
}

#[derive(Debug)]
pub(super) struct Status {
    pub(super) text: String,
    pub(super) error: bool,
    expires: Option<Instant>,
}

pub(super) struct App {
    db: Db,
    today: NaiveDate,
    pub(super) screen: Screen,
    pub(super) sheet: SheetView,
    pub(super) fields_view: FieldsView,
    pub(super) help: bool,
    pub(super) status: Option<Status>,
    quit: bool,
    pub(super) fields: Vec<Field>,
    /// Every paycheck, oldest first.
    pub(super) paychecks: Vec<Paycheck>,
}

impl App {
    pub(super) fn new(db: Db, today: NaiveDate) -> Result<App> {
        let fields = db.fields()?;
        let paychecks = db.paychecks()?;
        let year = paychecks.last().map_or(today.year(), |p| p.date.year());
        let mut app = App {
            db,
            today,
            screen: Screen::Sheet,
            sheet: SheetView::new(year, 0),
            fields_view: FieldsView::default(),
            help: false,
            status: None,
            quit: false,
            fields,
            paychecks,
        };
        app.select_last_in_year();
        Ok(app)
    }

    pub(super) fn should_quit(&self) -> bool {
        self.quit
    }

    pub(super) fn on_key(&mut self, key: KeyEvent) {
        self.status = None;
        if let Err(e) = self.dispatch(key) {
            self.error(format!("{e:#}"));
        }
    }

    pub(super) fn expire_status(&mut self) -> bool {
        self.expire_status_at(Instant::now())
    }

    /// Drop a status message whose time is up, and say whether one went.
    pub(super) fn expire_status_at(&mut self, now: Instant) -> bool {
        let expired = self
            .status
            .as_ref()
            .and_then(|s| s.expires)
            .is_some_and(|at| now >= at);
        if expired {
            self.status = None;
        }
        expired
    }

    fn info(&mut self, text: String) {
        self.set_status(text, false);
    }

    fn error(&mut self, text: String) {
        self.set_status(text, true);
    }

    fn set_status(&mut self, text: String, error: bool) {
        self.status = Some(Status {
            text,
            error,
            expires: Some(Instant::now() + STATUS_TTL),
        });
    }

    fn dispatch(&mut self, key: KeyEvent) -> Result<()> {
        if self.help {
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('?') | KeyCode::F(1)) {
                self.help = false;
            }
            return Ok(());
        }
        if key.code == KeyCode::F(1) || (key.code == KeyCode::Char('?') && is_bare(key)) {
            self.help = true;
            return Ok(());
        }
        if !is_bare(key) {
            return Ok(());
        }
        match key.code {
            KeyCode::Char('1') => self.screen = Screen::Sheet,
            KeyCode::Char('2') => self.screen = Screen::Fields,
            KeyCode::Char('q') => self.quit = true,
            _ => match self.screen {
                Screen::Sheet => self.sheet_key(key)?,
                Screen::Fields => self.fields_key(key)?,
            },
        }
        Ok(())
    }

    fn sheet_key(&mut self, key: KeyEvent) -> Result<()> {
        let last = self.year_paychecks().len().saturating_sub(1);
        match key.code {
            KeyCode::Left => self.sheet.selected = self.sheet.selected.saturating_sub(1),
            KeyCode::Right => self.sheet.selected = (self.sheet.selected + 1).min(last),
            KeyCode::Home => self.sheet.selected = 0,
            KeyCode::End => self.sheet.selected = last,
            KeyCode::Char('[') => self.show_year(self.sheet.year - 1),
            KeyCode::Char(']') => self.show_year(self.sheet.year + 1),
            _ => {}
        }
        Ok(())
    }

    fn fields_key(&mut self, key: KeyEvent) -> Result<()> {
        let last = self.fields.len().saturating_sub(1);
        match key.code {
            KeyCode::Up => self.fields_view.selected = self.fields_view.selected.saturating_sub(1),
            KeyCode::Down => self.fields_view.selected = (self.fields_view.selected + 1).min(last),
            _ => {}
        }
        Ok(())
    }

    fn reload(&mut self) -> Result<()> {
        self.fields = self.db.fields()?;
        self.paychecks = self.db.paychecks()?;
        Ok(())
    }

    /// The shown year's paychecks, oldest first -- the Sheet's columns.
    fn year_paychecks(&self) -> Vec<&Paycheck> {
        self.paychecks
            .iter()
            .filter(|p| p.date.year() == self.sheet.year)
            .collect()
    }

    fn selected_paycheck(&self) -> Option<&Paycheck> {
        self.year_paychecks().get(self.sheet.selected).copied()
    }

    fn select_last_in_year(&mut self) {
        self.sheet.selected = self.year_paychecks().len().saturating_sub(1);
    }

    fn show_year(&mut self, year: i32) {
        self.sheet.year = year;
        self.select_last_in_year();
    }

    pub(super) fn render(&mut self, frame: &mut Frame) {
        let [body, footer] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());
        match self.screen {
            Screen::Sheet => {
                let data = calc::sheet(self.sheet.year, &self.fields, &self.paychecks);
                sheet::render(frame, body, &mut self.sheet, &data);
            }
            Screen::Fields => fields::render(frame, body, &self.fields_view, &self.fields),
        }
        if self.help {
            help::render(frame, body, &self.help_topics());
        }
        frame.render_widget(self.footer(), footer);
    }

    fn footer(&self) -> Paragraph<'static> {
        match &self.status {
            Some(s) if s.error => Paragraph::new(s.text.clone()).style(Style::new().fg(Color::Red)),
            Some(s) => Paragraph::new(s.text.clone()),
            None => Paragraph::new(help::footer(&self.footer_tables())),
        }
    }

    fn footer_tables(&self) -> Vec<&'static [Entry]> {
        if self.help {
            return vec![help::HELP];
        }
        match self.screen {
            Screen::Sheet => vec![help::SHEET, help::GLOBAL],
            Screen::Fields => vec![help::FIELDS, help::GLOBAL],
        }
    }

    fn help_topics(&self) -> Vec<(&'static str, &'static [Entry])> {
        vec![
            match self.screen {
                Screen::Sheet => ("Sheet", help::SHEET),
                Screen::Fields => ("Fields", help::FIELDS),
            },
            ("Everywhere", help::GLOBAL),
        ]
    }
}
```

`reload`, `selected_paycheck`, `db`, and `today` are unused until Task 12; the module-level `#![allow(dead_code)]` in `tui/mod.rs` covers them.

- [ ] **Step 5: Implement `run` and the event loop**

Add to `src/tui/mod.rs`:

```rust
use crate::db::Db;
use anyhow::Result;
use app::App;
use chrono::NaiveDate;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use std::time::Duration;

const TICK: Duration = Duration::from_millis(250);

pub fn run(db: Db, today: NaiveDate) -> Result<()> {
    let mut app = App::new(db, today)?;
    // `try_init` enables raw mode, enters the alternate screen, and installs a
    // panic hook that restores the terminal before unwinding.
    let mut terminal = ratatui::try_init()?;
    let result = event_loop(&mut terminal, &mut app);
    ratatui::try_restore()?;
    result
}

/// Draw only when something changed: a key press, a resize, or a status
/// message running out. The tick keeps firing so the last one is noticed.
fn event_loop(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    let mut dirty = true;
    while !app.should_quit() {
        dirty |= app.expire_status();
        if dirty {
            terminal.draw(|frame| app.render(frame))?;
            dirty = false;
        }
        if !event::poll(TICK)? {
            continue;
        }
        match event::read()? {
            Event::Key(key) if is_press(&key) => {
                app.on_key(key);
                dirty = true;
            }
            Event::Resize(..) => dirty = true,
            _ => {}
        }
    }
    Ok(())
}

/// Windows reports releases too; acting on both would run every key twice.
fn is_press(key: &KeyEvent) -> bool {
    key.kind == KeyEventKind::Press
}
```

- [ ] **Step 6: Implement the CLI**

`src/bin/pc.rs`:

```rust
use anyhow::Result;
use chrono::{Local, NaiveDate};
use clap::Parser;
use paychecker::{db, tui};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pc", about = "Paychecker: record paychecks and see where each one goes")]
struct Cli {
    /// Database file. Defaults to ~/.local/share/paychecker/paychecks.db
    #[arg(long)]
    db: Option<PathBuf>,
    /// Treat this date as today. Defaults to the local date.
    #[arg(long)]
    today: Option<NaiveDate>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let path = match cli.db {
        Some(path) => path,
        None => db::default_path()?,
    };
    let db = db::open(&path)?;
    let today = cli.today.unwrap_or_else(|| Local::now().date_naive());
    tui::run(db, today)
}
```

- [ ] **Step 7: Run the tests and a smoke run**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: all pass, clean.

Run: `cargo run --bin pc -- --db /tmp/pc-smoke.db --today 2026-01-16` — the Sheet shows the ten seeded labels and the empty-year hint; `2` shows Fields; `?` opens help; `q` quits and the shell is restored. Then `rm /tmp/pc-smoke.db*`.

Run: `cargo run --bin pc -- --db /tmp/pc-smoke.db --today nope` — clap rejects the date and exits non-zero.

- [ ] **Step 8: Record the initial selection in the spec**

In the spec's **Sheet** bullets, extend the first bullet so it reads:

```markdown
- It opens on the year of the latest paycheck, or the current year if there are none, with the
  year's latest paycheck selected. `[`/`]` step the year and select that year's latest paycheck.
```

- [ ] **Step 9: Commit**

Message: `feat: add App with screen switching, status line, help, and the pc binary`

---

### Task 12: Paycheck add, edit, and delete

**Files:**
- Modify: `src/tui/app.rs`, `src/tui/form.rs` (rendering), `src/tui/test_support.rs`
- Modify: spec (Sheet section)

**Interfaces:**
- Consumes: `PaycheckForm`, `Outcome`, `Db::{insert_paycheck, update_paycheck, delete_paycheck}`, `centered`, `calc::{percent, show}`.
- Produces:
  - `pub(super) enum Modal { Paycheck(PaycheckForm), DeletePaycheck(PaycheckId) }` (Task 13 adds variants).
  - `App.modal: Option<Modal>` (`pub(super)`), private `status_set: bool`.
  - `form::render_paycheck(frame, area, &PaycheckForm)`, `form::net_line(Option<Totals>) -> String` (`"Net 3,094.00  77.35%"` or `"Net —"`).
  - Status texts: `"Saved the paycheck dated {date}"`, `"Delete the paycheck dated {date}? y to confirm"`, `"Deleted the paycheck dated {date}"`.
  - `test_support::type_text(&mut App, &str)`.

- [ ] **Step 1: Add `type_text`**

Append to `src/tui/test_support.rs`:

```rust
pub(super) fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, KeyCode::Char(c));
    }
}
```

- [ ] **Step 2: Write the failing tests**

Add to `mod tests` in `src/tui/app.rs` (extend the `test_support` import with `type_text`):

```rust
    fn paycheck_form(app: &App) -> &PaycheckForm {
        match &app.modal {
            Some(Modal::Paycheck(form)) => form,
            _ => panic!("no paycheck form open"),
        }
    }

    #[test]
    fn adding_a_paycheck_prefills_the_date_and_the_latest_amounts() {
        let mut app = standard();
        press(&mut app, KeyCode::Char('a'));
        let form = paycheck_form(&app);
        assert_eq!(form.date.value(), "2026-01-30");
        assert_eq!(form.amounts[0].text.value(), "4,000.00");
        let text = screen(&mut app, 100, 30);
        assert!(text.contains("Add paycheck"));
        assert!(text.contains("Net 3,094.00  77.35%"));
        assert!(text.lines().last().unwrap().starts_with("Tab next"));
    }

    #[test]
    fn saving_an_added_paycheck_selects_it_and_confirms() {
        let mut app = standard();
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Enter);
        assert!(app.modal.is_none());
        assert_eq!(app.paychecks.len(), 4);
        assert_eq!((app.sheet.year, app.sheet.selected), (2026, 2));
        assert_eq!(app.status.as_ref().unwrap().text, "Saved the paycheck dated 2026-01-30");
        assert!(app.expire_status_at(Instant::now() + STATUS_TTL));
    }

    #[test]
    fn saving_a_paycheck_in_another_year_moves_the_sheet_there() {
        let mut app = app_with(&[(day(2026, 12, 25), STUB)], day(2026, 12, 26));
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Enter);
        assert_eq!((app.sheet.year, app.sheet.selected), (2027, 0));
    }

    #[test]
    fn a_bad_amount_keeps_the_form_open_with_the_error_until_it_closes() {
        let mut app = standard();
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Tab);
        app.on_key(ctrl('u'));
        type_text(&mut app, "abc");
        press(&mut app, KeyCode::Enter);
        assert!(app.modal.is_some());
        let status = app.status.as_ref().unwrap();
        assert!(status.error);
        assert!(status.text.starts_with("Salary: "), "{}", status.text);
        assert!(!app.expire_status_at(Instant::now() + STATUS_TTL * 10));
        press(&mut app, KeyCode::Tab);
        assert!(app.status.is_some());
        press(&mut app, KeyCode::Esc);
        assert!(app.modal.is_none());
        assert!(app.status.is_none());
        assert_eq!(app.paychecks.len(), 3);
    }

    #[test]
    fn a_duplicate_date_is_refused_on_the_status_line() {
        let mut app = standard();
        press(&mut app, KeyCode::Char('a'));
        app.on_key(ctrl('u'));
        type_text(&mut app, "1/16");
        press(&mut app, KeyCode::Enter);
        assert!(app.modal.is_some());
        assert_eq!(
            app.status.as_ref().unwrap().text,
            "a paycheck dated 2026-01-16 already exists"
        );
    }

    #[test]
    fn editing_the_selected_paycheck_saves_its_changes_under_the_same_date() {
        let mut app = standard();
        press(&mut app, KeyCode::Left);
        press(&mut app, KeyCode::Char('e'));
        assert_eq!(paycheck_form(&app).date.value(), "2026-01-02");
        press(&mut app, KeyCode::Tab);
        app.on_key(ctrl('u'));
        type_text(&mut app, "4100");
        press(&mut app, KeyCode::Enter);
        assert!(app.modal.is_none(), "{:?}", app.status);
        let salary = app.db.field_id("Salary");
        assert_eq!(app.paychecks[1].amounts[&salary], Cents(410_000));
        assert_eq!(app.sheet.selected, 0);
        assert!(!screen(&mut app, 100, 30).contains("Edit paycheck"));
    }

    #[test]
    fn deleting_a_paycheck_needs_y() {
        let mut app = standard();
        press(&mut app, KeyCode::Char('d'));
        assert_eq!(
            app.status.as_ref().unwrap().text,
            "Delete the paycheck dated 2026-01-16? y to confirm"
        );
        press(&mut app, KeyCode::Char('n'));
        assert!(app.modal.is_none());
        assert!(app.status.is_none());
        assert_eq!(app.paychecks.len(), 3);
        press(&mut app, KeyCode::Char('d'));
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(app.paychecks.len(), 2);
        assert_eq!(app.sheet.selected, 0);
        assert_eq!(app.status.as_ref().unwrap().text, "Deleted the paycheck dated 2026-01-16");
    }

    #[test]
    fn deleting_the_only_paycheck_in_a_year_leaves_an_empty_year() {
        let mut app = standard();
        press(&mut app, KeyCode::Char('['));
        press(&mut app, KeyCode::Char('d'));
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(app.sheet.year, 2025);
        press(&mut app, KeyCode::Char('e'));
        press(&mut app, KeyCode::Char('d'));
        assert!(app.modal.is_none());
        assert!(screen(&mut app, 80, 30).contains("No paychecks in 2025. Press a to add one."));
    }

    #[test]
    fn global_keys_are_typed_into_an_open_form() {
        let mut app = standard();
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Tab);
        type_text(&mut app, "12q");
        assert!(!app.should_quit());
        assert_eq!(paycheck_form(&app).amounts[0].text.value(), "4,000.0012q");
    }

    #[test]
    fn help_over_a_form_lists_the_forms_keys_first() {
        let mut app = standard();
        press(&mut app, KeyCode::Char('a'));
        app.on_key(key(KeyCode::F(1)));
        let text = screen(&mut app, 100, 40);
        let form_at = text.find("Paycheck form").unwrap();
        let sheet_at = text.find("Sheet").unwrap();
        assert!(form_at < sheet_at);
        press(&mut app, KeyCode::Esc);
        assert!(!app.help);
        assert!(app.modal.is_some());
    }
```

Add `use crate::money::Cents;` to the test imports.

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test tui::app`
Expected: compile errors — `Modal`, `app.modal` not found.

- [ ] **Step 4: Add form rendering**

Append to `src/tui/form.rs` (above the tests; add imports `super::centered`, `ratatui::{Frame, layout::Rect, text::Line, widgets::{Block, Clear, Paragraph}}`):

```rust
/// The live line under the amounts: `—` while any amount fails to parse.
pub(super) fn net_line(totals: Option<Totals>) -> String {
    match totals {
        Some(t) => format!("Net {}  {}", t.net, calc::show(calc::percent(t.net, t.income))),
        None => "Net —".to_string(),
    }
}

pub(super) fn render_paycheck(frame: &mut Frame, area: Rect, form: &PaycheckForm) {
    let label_w = form
        .amounts
        .iter()
        .map(|a| a.name.chars().count())
        .max()
        .unwrap_or(0)
        .max("Date".len());
    let rows = std::iter::once(("Date", &form.date))
        .chain(form.amounts.iter().map(|a| (a.name.as_str(), &a.text)));
    let mut lines: Vec<Line> = rows
        .enumerate()
        .map(|(i, (label, text))| {
            let marker = if i == form.focus { "›" } else { " " };
            Line::from(format!("{marker} {label:<label_w$}  {}", text.value()))
        })
        .collect();
    lines.push(Line::default());
    lines.push(Line::from(net_line(form.totals())));
    let title = if form.editing.is_some() { " Edit paycheck " } else { " Add paycheck " };
    let width = (label_w + 4 + 16).max(32) as u16 + 2;
    let popup = centered(area, width, lines.len() as u16 + 2);
    frame.render_widget(Clear, popup);
    frame.render_widget(Paragraph::new(lines).block(Block::bordered().title(title)), popup);
    let caret = match form.focus {
        0 => form.date.caret(),
        n => form.amounts[n - 1].text.caret(),
    };
    frame.set_cursor_position((
        popup.x + 1 + (label_w + 4 + caret) as u16,
        popup.y + 1 + form.focus as u16,
    ));
}
```

- [ ] **Step 5: Wire the modal into `App`**

In `src/tui/app.rs`:

Imports: add `super::form::{self, Outcome, PaycheckForm}` and `crate::db::PaycheckId`.

Add the enum:

```rust
pub(super) enum Modal {
    Paycheck(PaycheckForm),
    /// Waiting for `y`; the question is on the status line.
    DeletePaycheck(PaycheckId),
}
```

Add fields to `App` (and initialize them to `None` / `false` in `new`):

```rust
    pub(super) modal: Option<Modal>,
    /// Whether the key being handled set the status line. Closing a modal
    /// clears a status message the closing key did not set.
    status_set: bool,
```

Replace `on_key` and `set_status`:

```rust
    /// With no modal open, the status line lasts until the next key or
    /// `STATUS_TTL`. With one open, it lasts until the modal closes, so an
    /// error stays in view while the form is being fixed.
    pub(super) fn on_key(&mut self, key: KeyEvent) {
        let had_modal = self.modal.is_some();
        if !had_modal {
            self.status = None;
        }
        self.status_set = false;
        if let Err(e) = self.dispatch(key) {
            self.error(format!("{e:#}"));
        }
        if had_modal && self.modal.is_none() && !self.status_set {
            self.status = None;
        }
    }

    fn set_status(&mut self, text: String, error: bool) {
        let expires = self.modal.is_none().then(|| Instant::now() + STATUS_TTL);
        self.status = Some(Status { text, error, expires });
        self.status_set = true;
    }
```

In `dispatch`, after the `F1`/`?` branch and before the `is_bare` check, add:

```rust
        if let Some(modal) = self.modal.take() {
            return self.modal_key(modal, key);
        }
```

Add to `sheet_key`'s match:

```rust
            KeyCode::Char('a') => {
                let form = PaycheckForm::add(&self.fields, self.paychecks.last(), self.today);
                self.modal = Some(Modal::Paycheck(form));
            }
            KeyCode::Char('e') => {
                if let Some(p) = self.selected_paycheck() {
                    let form = PaycheckForm::edit(&self.fields, p, self.today);
                    self.modal = Some(Modal::Paycheck(form));
                }
            }
            KeyCode::Char('d') => {
                if let Some(p) = self.selected_paycheck() {
                    let (id, date) = (p.id, p.date);
                    self.modal = Some(Modal::DeletePaycheck(id));
                    self.info(format!("Delete the paycheck dated {date}? y to confirm"));
                }
            }
```

Add:

```rust
    /// The modal has been taken out of `self.modal`; put it back to keep it open.
    fn modal_key(&mut self, modal: Modal, key: KeyEvent) -> Result<()> {
        match modal {
            Modal::Paycheck(mut form) => match form.on_key(key) {
                Outcome::Continue => self.modal = Some(Modal::Paycheck(form)),
                Outcome::Cancel => {}
                Outcome::Submit => {
                    if let Err(e) = self.save_paycheck(&form) {
                        self.modal = Some(Modal::Paycheck(form));
                        return Err(e);
                    }
                }
            },
            Modal::DeletePaycheck(id) => {
                if is_yes(key) {
                    let date = self.paychecks.iter().find(|p| p.id == id).map(|p| p.date);
                    self.db.delete_paycheck(id)?;
                    self.reload()?;
                    let last = self.year_paychecks().len().saturating_sub(1);
                    self.sheet.selected = self.sheet.selected.min(last);
                    if let Some(date) = date {
                        self.info(format!("Deleted the paycheck dated {date}"));
                    }
                }
            }
        }
        Ok(())
    }

    fn save_paycheck(&mut self, form: &PaycheckForm) -> Result<()> {
        let (date, amounts) = form.parsed()?;
        let id = match form.editing {
            None => self.db.insert_paycheck(date, &amounts)?,
            Some(id) => {
                self.db.update_paycheck(id, date, &amounts)?;
                id
            }
        };
        self.reload()?;
        self.sheet.year = date.year();
        self.sheet.selected = self
            .year_paychecks()
            .iter()
            .position(|p| p.id == id)
            .unwrap_or(0);
        self.info(format!("Saved the paycheck dated {date}"));
        Ok(())
    }
```

and, at module level:

```rust
fn is_yes(key: KeyEvent) -> bool {
    key.code == KeyCode::Char('y') && is_bare(key)
}
```

In `render`, between the screen and the help panel:

```rust
        if let Some(Modal::Paycheck(form)) = &self.modal {
            form::render_paycheck(frame, body, form);
        }
```

Replace `footer_tables` and `help_topics`:

```rust
    fn footer_tables(&self) -> Vec<&'static [Entry]> {
        if self.help {
            return vec![help::HELP];
        }
        match (&self.modal, self.screen) {
            (Some(Modal::Paycheck(_)), _) => vec![help::PAYCHECK_FORM],
            (Some(Modal::DeletePaycheck(_)), _) => vec![help::CONFIRM],
            (None, Screen::Sheet) => vec![help::SHEET, help::GLOBAL],
            (None, Screen::Fields) => vec![help::FIELDS, help::GLOBAL],
        }
    }

    /// The open form's keys first, then the screen's, then the global keys.
    fn help_topics(&self) -> Vec<(&'static str, &'static [Entry])> {
        let mut topics = Vec::new();
        if let Some(Modal::Paycheck(_)) = &self.modal {
            topics.push(("Paycheck form", help::PAYCHECK_FORM));
        }
        topics.push(match self.screen {
            Screen::Sheet => ("Sheet", help::SHEET),
            Screen::Fields => ("Fields", help::FIELDS),
        });
        topics.push(("Everywhere", help::GLOBAL));
        topics
    }
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: all pass, clean.

- [ ] **Step 7: Record the confirmation prompt in the spec**

In the Sheet section, replace "`d` delete the selected one (`y` confirms, anything else cancels)." with:

```markdown
`d` delete the selected one: the status line asks, `y` confirms, and any other key cancels.
```

- [ ] **Step 8: Commit**

Message: `feat: add, edit, and delete paychecks from the Sheet`

---

### Task 13: Fields screen actions

**Files:**
- Modify: `src/tui/app.rs`, `src/tui/form.rs` (field form rendering), `src/tui/mod.rs` (remove the `dead_code` allow)

**Interfaces:**
- Consumes: `FieldForm`, `FieldFocus`, `Db::{insert_field, update_field, set_archived, move_field, ensure_deletable, delete_field}`.
- Produces:
  - `Modal::Field(FieldForm)`, `Modal::DeleteField(FieldId)`.
  - `form::render_field(frame, area, &FieldForm)`.
  - Status texts: `"Saved {name}"`, `"Archived {name}"`, `"Unarchived {name}"`, `"Delete {name}? y to confirm"`, `"Deleted {name}"`.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `src/tui/app.rs`:

```rust
    fn names(app: &App) -> Vec<&str> {
        app.fields.iter().map(|f| f.name.as_str()).collect()
    }

    #[test]
    fn a_new_field_goes_at_the_end_and_into_the_next_paycheck_form() {
        let mut app = standard();
        press(&mut app, KeyCode::Char('2'));
        press(&mut app, KeyCode::Char('a'));
        assert!(screen(&mut app, 80, 30).contains("Add field"));
        type_text(&mut app, "Bonus");
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Left);
        press(&mut app, KeyCode::Enter);
        assert!(app.modal.is_none(), "{:?}", app.status);
        let bonus = app.fields.last().unwrap();
        assert_eq!((bonus.name.as_str(), bonus.kind), ("Bonus", Kind::Income));
        assert_eq!(app.fields_view.selected, 10);
        assert_eq!(app.status.as_ref().unwrap().text, "Saved Bonus");
        press(&mut app, KeyCode::Char('1'));
        press(&mut app, KeyCode::Char('a'));
        assert_eq!(paycheck_form(&app).amounts[1].name, "Bonus");
    }

    #[test]
    fn a_duplicate_or_blank_name_keeps_the_field_form_open() {
        let mut app = standard();
        press(&mut app, KeyCode::Char('2'));
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.status.as_ref().unwrap().text, "a field needs a name");
        type_text(&mut app, "Medicare");
        press(&mut app, KeyCode::Enter);
        assert!(matches!(app.modal, Some(Modal::Field(_))));
        assert_eq!(
            app.status.as_ref().unwrap().text,
            "a field named \"Medicare\" already exists"
        );
    }

    #[test]
    fn renaming_a_field_keeps_its_past_amounts() {
        let mut app = standard();
        press(&mut app, KeyCode::Char('2'));
        press(&mut app, KeyCode::Char('e'));
        app.on_key(ctrl('u'));
        type_text(&mut app, "Base Pay");
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('1'));
        let text = screen(&mut app, 100, 30);
        let row = text.lines().find(|l| l.starts_with("Base Pay")).unwrap();
        assert!(row.ends_with("8,000.00"), "{row}");
    }

    #[test]
    fn shift_arrows_move_the_selected_field_and_keep_it_selected() {
        let mut app = standard();
        press(&mut app, KeyCode::Char('2'));
        press(&mut app, KeyCode::Down);
        app.on_key(shift(KeyCode::Down));
        assert_eq!(names(&app)[1..3], ["Social Security", "Federal Tax"]);
        assert_eq!(app.fields_view.selected, 2);
        app.on_key(shift(KeyCode::Up));
        assert_eq!(names(&app)[1], "Federal Tax");
        assert_eq!(app.fields_view.selected, 1);
    }

    #[test]
    fn x_archives_and_unarchives_the_selected_field() {
        let mut app = standard();
        press(&mut app, KeyCode::Char('2'));
        for _ in 0..7 {
            press(&mut app, KeyCode::Down);
        }
        press(&mut app, KeyCode::Char('x'));
        assert!(app.fields[7].archived);
        assert_eq!(app.status.as_ref().unwrap().text, "Archived HSA");
        assert!(screen(&mut app, 80, 30).contains("archived"));
        press(&mut app, KeyCode::Char('x'));
        assert!(!app.fields[7].archived);
        assert_eq!(app.status.as_ref().unwrap().text, "Unarchived HSA");
    }

    #[test]
    fn deleting_a_used_field_says_to_archive_it_instead() {
        let mut app = standard();
        press(&mut app, KeyCode::Char('2'));
        press(&mut app, KeyCode::Char('d'));
        assert!(app.modal.is_none());
        let status = app.status.as_ref().unwrap();
        assert!(status.error);
        assert_eq!(status.text, "Salary is used by a paycheck; archive it with x instead");
    }

    #[test]
    fn deleting_an_unused_field_needs_y() {
        let mut app = standard();
        press(&mut app, KeyCode::Char('2'));
        let last = app.fields.len() - 1;
        app.fields_view.selected = last;
        press(&mut app, KeyCode::Char('d'));
        assert_eq!(app.status.as_ref().unwrap().text, "Delete 401K (Roth)? y to confirm");
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(app.fields.len(), 9);
        assert_eq!(app.fields_view.selected, 8);
        assert_eq!(app.status.as_ref().unwrap().text, "Deleted 401K (Roth)");
    }

    #[test]
    fn an_archived_field_shows_only_in_years_a_paycheck_used_it() {
        let mut app = app_with(
            &[
                (day(2025, 12, 19), &[("Salary", 400_000), ("HSA", 10_000)]),
                (day(2026, 1, 2), &[("Salary", 400_000)]),
            ],
            day(2026, 1, 20),
        );
        press(&mut app, KeyCode::Char('2'));
        app.fields_view.selected = 7;
        press(&mut app, KeyCode::Char('x'));
        press(&mut app, KeyCode::Char('1'));
        assert!(!screen(&mut app, 100, 30).lines().any(|l| l.starts_with("HSA")));
        press(&mut app, KeyCode::Char('a'));
        assert!(paycheck_form(&app).amounts.iter().all(|a| a.name != "HSA"));
        press(&mut app, KeyCode::Esc);
        press(&mut app, KeyCode::Char('['));
        let text = screen(&mut app, 100, 30);
        let row = text.lines().find(|l| l.starts_with("HSA")).unwrap();
        assert!(row.contains("100.00"), "{row}");
        press(&mut app, KeyCode::Char('e'));
        assert!(paycheck_form(&app).amounts.iter().any(|a| a.name == "HSA"));
    }
```

Extend the test imports with `shift` and `crate::db::Kind`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test tui::app`
Expected: compile errors — `Modal::Field` not found.

- [ ] **Step 3: Add field form rendering**

Append to `src/tui/form.rs` above the tests:

```rust
pub(super) fn render_field(frame: &mut Frame, area: Rect, form: &FieldForm) {
    let marker = |focus| if form.focus == focus { "›" } else { " " };
    let lines = vec![
        Line::from(format!("{} Name  {}", marker(FieldFocus::Name), form.name.value())),
        Line::from(format!("{} Kind  ◀ {} ▶", marker(FieldFocus::Kind), form.kind.label())),
    ];
    let title = if form.editing.is_some() { " Edit field " } else { " Add field " };
    let popup = centered(area, 40, 4);
    frame.render_widget(Clear, popup);
    frame.render_widget(Paragraph::new(lines).block(Block::bordered().title(title)), popup);
    if form.focus == FieldFocus::Name {
        frame.set_cursor_position((popup.x + 1 + 8 + form.name.caret() as u16, popup.y + 1));
    }
}
```

- [ ] **Step 4: Wire the Fields actions into `App`**

Imports: add `super::form::FieldForm`, `crate::db::FieldId`, `ratatui::crossterm::event::KeyModifiers`.

Extend `Modal`:

```rust
    Field(FieldForm),
    DeleteField(FieldId),
```

Replace `fields_key`:

```rust
    fn fields_key(&mut self, key: KeyEvent) -> Result<()> {
        let last = self.fields.len().saturating_sub(1);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::Up if shift => self.move_field(true)?,
            KeyCode::Down if shift => self.move_field(false)?,
            KeyCode::Up => self.fields_view.selected = self.fields_view.selected.saturating_sub(1),
            KeyCode::Down => self.fields_view.selected = (self.fields_view.selected + 1).min(last),
            KeyCode::Char('a') => self.modal = Some(Modal::Field(FieldForm::add())),
            KeyCode::Char('e') => {
                if let Some(f) = self.selected_field() {
                    let form = FieldForm::edit(f);
                    self.modal = Some(Modal::Field(form));
                }
            }
            KeyCode::Char('x') => {
                if let Some(f) = self.selected_field() {
                    let (id, archived, name) = (f.id, f.archived, f.name.clone());
                    self.db.set_archived(id, !archived)?;
                    self.reload()?;
                    let verb = if archived { "Unarchived" } else { "Archived" };
                    self.info(format!("{verb} {name}"));
                }
            }
            KeyCode::Char('d') => {
                if let Some(f) = self.selected_field() {
                    let (id, name) = (f.id, f.name.clone());
                    self.db.ensure_deletable(id)?;
                    self.modal = Some(Modal::DeleteField(id));
                    self.info(format!("Delete {name}? y to confirm"));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn selected_field(&self) -> Option<&Field> {
        self.fields.get(self.fields_view.selected)
    }

    fn select_field(&mut self, id: FieldId) {
        if let Some(i) = self.fields.iter().position(|f| f.id == id) {
            self.fields_view.selected = i;
        }
    }

    fn move_field(&mut self, up: bool) -> Result<()> {
        if let Some(id) = self.selected_field().map(|f| f.id) {
            self.db.move_field(id, up)?;
            self.reload()?;
            self.select_field(id);
        }
        Ok(())
    }

    fn save_field(&mut self, form: &FieldForm) -> Result<()> {
        let id = match form.editing {
            None => self.db.insert_field(form.name.value(), form.kind)?,
            Some(id) => {
                self.db.update_field(id, form.name.value(), form.kind)?;
                id
            }
        };
        self.reload()?;
        self.select_field(id);
        let name = self.fields[self.fields_view.selected].name.clone();
        self.info(format!("Saved {name}"));
        Ok(())
    }
```

Add arms to `modal_key`:

```rust
            Modal::Field(mut form) => match form.on_key(key) {
                Outcome::Continue => self.modal = Some(Modal::Field(form)),
                Outcome::Cancel => {}
                Outcome::Submit => {
                    if let Err(e) = self.save_field(&form) {
                        self.modal = Some(Modal::Field(form));
                        return Err(e);
                    }
                }
            },
            Modal::DeleteField(id) => {
                if is_yes(key) {
                    let name = self.fields.iter().find(|f| f.id == id).map(|f| f.name.clone());
                    self.db.delete_field(id)?;
                    self.reload()?;
                    let last = self.fields.len().saturating_sub(1);
                    self.fields_view.selected = self.fields_view.selected.min(last);
                    if let Some(name) = name {
                        self.info(format!("Deleted {name}"));
                    }
                }
            }
```

In `render`, replace the `if let Some(Modal::Paycheck(form))` block with:

```rust
        match &self.modal {
            Some(Modal::Paycheck(form)) => form::render_paycheck(frame, body, form),
            Some(Modal::Field(form)) => form::render_field(frame, body, form),
            _ => {}
        }
```

In `footer_tables`, replace the two modal arms with:

```rust
            (Some(Modal::Paycheck(_)), _) => vec![help::PAYCHECK_FORM],
            (Some(Modal::Field(_)), _) => vec![help::FIELD_FORM],
            (Some(Modal::DeletePaycheck(_) | Modal::DeleteField(_)), _) => vec![help::CONFIRM],
```

In `help_topics`, replace the `if let` with:

```rust
        match &self.modal {
            Some(Modal::Paycheck(_)) => topics.push(("Paycheck form", help::PAYCHECK_FORM)),
            Some(Modal::Field(_)) => topics.push(("Field form", help::FIELD_FORM)),
            _ => {}
        }
```

- [ ] **Step 5: Remove the temporary `dead_code` allow**

Delete the comment and `#![allow(dead_code)]` from `src/tui/mod.rs`.

- [ ] **Step 6: Run the tests and clippy**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: all pass, clean. If clippy now reports dead code, the item is genuinely unused: delete it (or, for a copied `TextBuffer` method no screen calls, such as `clear`, delete it along with any test that exercised only it).

- [ ] **Step 7: Commit**

Message: `feat: add, edit, move, archive, and delete fields`

---

### Task 14: README and end-to-end check

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add a Usage section to `README.md`**

After the opening paragraph and before `## Development`:

````markdown
## Usage

```bash
cargo install --path .
pc                      # opens ~/.local/share/paychecker/paychecks.db
pc --db /tmp/demo.db    # a scratch database
```

`1` shows the Sheet (a year's paychecks, YTD, net, and percentages) and `2` the Fields list.
`a` adds, `e` edits, and `d` deletes on either screen; `?` lists every key.
````

- [ ] **Step 2: Full verification**

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: clean, all tests pass.

- [ ] **Step 3: Manual walkthrough on a scratch database**

Run: `cargo run --bin pc -- --db /tmp/pc-walk.db --today 2026-01-16` and check, with invented amounts only:
1. `a`: date is `2026-01-16`, amounts blank; type `4000` for Salary, `600` for Federal Tax; the Net line updates; `Enter` saves and the Sheet shows `◀ 01-16 ▶`.
2. `a` again: date `2026-01-30`, amounts prefilled; `Enter`. YTD Salary reads `8,000.00`; Federal Tax % reads `15.00%`.
3. `2`, select HSA, `x`: dimmed and tagged. `1`: no HSA row. `2`, `Shift-↓` on Salary: moves. `d` on Salary: status says to archive instead.
4. Resize the terminal narrow: the selected column stays visible.
5. `q`: the shell is restored.

Then `rm /tmp/pc-walk.db*`.

- [ ] **Step 4: Commit**

Message: `docs: add usage to README`

---

## Self-Review Notes

- **Spec coverage:** storage/WAL/foreign keys/default path/parent dir (T2), migrations and newer-DB refusal (T2), seed (T2/T3), semantics incl. negatives, blank = 0, net, percent, `—`, YTD ratio, visibility (T1, T5, T7), module layout (all), CLI (T11), event loop (T11), global keys and status line rules (T11, T12), Sheet layout/scroll/year stepping/hint/add-edit-delete (T8, T11, T12), paycheck form (T7, T12), Fields screen (T9, T13), help (T10–T13), error handling (T11–T13), testing layers (every task), repository/README (T14; hooks, CI, `.gitignore`, `AGENTS.md` already exist).
- **Type names** used across tasks: `Cents`, `Kind`, `Field`, `Paycheck`, `FieldId`, `PaycheckId`, `Db`, `Totals`, `Percent`, `Sheet`, `SheetView`, `FieldsView`, `PaycheckForm`, `AmountInput`, `FieldForm`, `FieldFocus`, `Outcome`, `Entry`, `Modal`, `Screen`, `Status`, `App`.
