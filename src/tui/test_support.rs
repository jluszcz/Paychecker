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
