//! Helpers shared by the `tui` tests.

use crate::db::{Field, Paycheck, PaycheckId};
use crate::money::Cents;
use chrono::NaiveDate;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Frame, Terminal};

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
            let line: String = (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect();
            line.trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
