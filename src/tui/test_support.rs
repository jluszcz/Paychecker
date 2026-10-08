//! Helpers shared by the `tui` tests.

use super::app::App;
use crate::db::{self, Field, FieldId, Paycheck, PaycheckId};
use crate::money::Cents;
use chrono::NaiveDate;
use jluszcz_finance_utils::tui::app::App as _;
pub(super) use jluszcz_finance_utils::tui::testing::{
    buffer_text, ctrl, draw, draw_buffer, key, shift,
};
use ratatui::crossterm::event::KeyCode;

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

/// The rows inside the screen's border, with the border's sides removed and
/// trailing spaces trimmed. The title and the footer are not among them.
pub(super) fn inside(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    lines[1..lines.len().saturating_sub(2)]
        .iter()
        .map(|l| {
            let l = l.strip_prefix('│').unwrap_or(l);
            let l = l.strip_suffix('│').unwrap_or(l);
            l.trim_end().to_string()
        })
        .collect()
}

pub(super) fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, KeyCode::Char(c));
    }
}
