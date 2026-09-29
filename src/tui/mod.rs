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
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use ratatui::layout::Rect;
use std::time::Duration;

const TICK: Duration = Duration::from_millis(250);

/// Runs the screens until the user quits, then hands the database back so
/// the quit path can read what this run wrote.
pub fn run(db: Db, today: NaiveDate) -> Result<Db> {
    let mut app = App::new(db, today)?;
    // `try_init` enables raw mode, enters the alternate screen, and installs a
    // panic hook that restores the terminal before unwinding.
    let mut terminal = ratatui::try_init()?;
    let result = event_loop(&mut terminal, &mut app);
    ratatui::try_restore()?;
    result?;
    Ok(app.into_db())
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
