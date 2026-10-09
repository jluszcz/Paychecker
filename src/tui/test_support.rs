//! Helpers shared by the `tui` tests.

use super::app::App;
use crate::db::{self, Field, FieldId, Paycheck, PaycheckId};
use crate::money::Cents;
use chrono::NaiveDate;
pub(super) use jluszcz_finance_utils::testing::day;
pub(super) use jluszcz_finance_utils::tui::testing::{
    buffer_text, ctrl, draw, draw_buffer, inside, key, press, screen, shift, type_text,
};

/// Invented amounts: a salary and three deductions.
pub(super) const STUB: &[(&str, i64)] = &[
    ("Salary", 400_000),
    ("Federal Tax", 60_000),
    ("Social Security", 24_800),
    ("Medicare", 5_800),
];

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
