//! `App`: which screen is showing, the status line, and where each key goes.

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::test_support::{STUB, app_with, ctrl, day, key, press, screen};
    use ratatui::crossterm::event::KeyCode;

    /// One paycheck in 2025 and two in 2026.
    fn standard() -> App {
        app_with(
            &[
                (day(2025, 12, 19), STUB),
                (day(2026, 1, 2), STUB),
                (day(2026, 1, 16), STUB),
            ],
            day(2026, 1, 20),
        )
    }

    #[test]
    fn the_sheet_opens_on_the_latest_paychecks_year_with_it_selected() {
        let app = standard();
        assert_eq!(
            (app.screen, app.sheet.year, app.sheet.selected),
            (Screen::Sheet, 2026, 1)
        );
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
        assert_eq!(
            text.lines().last().unwrap(),
            help::footer(&[help::SHEET, help::GLOBAL])
        );
        press(&mut app, KeyCode::Char('2'));
        let text = screen(&mut app, 120, 30);
        assert_eq!(
            text.lines().last().unwrap(),
            help::footer(&[help::FIELDS, help::GLOBAL])
        );
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
