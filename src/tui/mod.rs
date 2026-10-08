//! The terminal UI.

mod app;
mod fields;
mod form;
mod help;
mod sheet;
mod text;

#[cfg(test)]
mod test_support;

use crate::db::Db;
use anyhow::Result;
use app::App;
use chrono::NaiveDate;
pub(super) use jluszcz_finance_utils::tui::centered;

/// Runs the screens until the user quits, then hands the database back so
/// the quit path can read what this run wrote.
pub fn run(db: Db, today: NaiveDate) -> Result<Db> {
    Ok(jluszcz_finance_utils::tui::app::run(App::new(db, today)?)?.into_db())
}
