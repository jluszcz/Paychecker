//! `App`: which screen is showing, the status line, and where each key goes.

use super::fields::{self, FieldsView};
use super::form::{self, FieldForm, Outcome, PaycheckForm};
use super::help::{self, Entry};
use super::sheet::{self, SheetView};
use super::text::is_bare;
use crate::calc;
use crate::db::{Db, Field, FieldId, Paycheck, PaycheckId};
use anyhow::Result;
use chrono::{Datelike, NaiveDate};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Paragraph};
use std::time::{Duration, Instant};

pub(super) const STATUS_TTL: Duration = Duration::from_secs(4);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum Screen {
    Sheet,
    Fields,
}

pub(super) enum Modal {
    Paycheck(PaycheckForm),
    /// Waiting for `y`; the question is on the status line.
    DeletePaycheck(PaycheckId),
    Field(FieldForm),
    DeleteField(FieldId),
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
    pub(super) modal: Option<Modal>,
    pub(super) status: Option<Status>,
    /// Whether the key being handled set the status line. Closing a modal
    /// clears a status message the closing key did not set.
    status_set: bool,
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
            modal: None,
            status: None,
            status_set: false,
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
        let expires = self.modal.is_none().then(|| Instant::now() + STATUS_TTL);
        self.status = Some(Status {
            text,
            error,
            expires,
        });
        self.status_set = true;
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
        if let Some(modal) = self.modal.take() {
            return self.modal_key(modal, key);
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
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::Left if shift => self.sheet.page(false, last),
            KeyCode::Right if shift => self.sheet.page(true, last),
            KeyCode::Left => self.sheet.selected = self.sheet.selected.saturating_sub(1),
            KeyCode::Right => self.sheet.selected = (self.sheet.selected + 1).min(last),
            KeyCode::Up => self.sheet.row_scroll = self.sheet.row_scroll.saturating_sub(1),
            KeyCode::Down => self.sheet.row_scroll += 1,
            KeyCode::Home => self.sheet.selected = 0,
            KeyCode::End => self.sheet.selected = last,
            KeyCode::Char('[') => self.show_year(self.sheet.year - 1),
            KeyCode::Char(']') => self.show_year(self.sheet.year + 1),
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
            _ => {}
        }
        Ok(())
    }

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
                    let name = self
                        .fields
                        .iter()
                        .find(|f| f.id == id)
                        .map(|f| f.name.clone());
                    self.db.delete_field(id)?;
                    self.reload()?;
                    let last = self.fields.len().saturating_sub(1);
                    self.fields_view.selected = self.fields_view.selected.min(last);
                    if let Some(name) = name {
                        self.info(format!("Deleted {name}"));
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
        let block = Block::bordered().title(self.title());
        let inner = block.inner(body);
        frame.render_widget(block, body);
        match self.screen {
            Screen::Sheet => {
                let data = calc::sheet(self.sheet.year, &self.fields, &self.paychecks);
                sheet::render(frame, inner, &mut self.sheet, &data);
            }
            Screen::Fields => fields::render(frame, inner, &self.fields_view, &self.fields),
        }
        match &self.modal {
            Some(Modal::Paycheck(form)) => form::render_paycheck(frame, body, form),
            Some(Modal::Field(form)) => form::render_field(frame, body, form),
            _ => {}
        }
        if self.help {
            help::render(frame, body, &self.help_topics());
        }
        frame.render_widget(self.footer(), footer);
    }

    /// The year the Sheet shows. Fields belong to no year, so their border
    /// is bare.
    fn title(&self) -> String {
        match self.screen {
            Screen::Sheet => format!(" {} ", self.sheet.year),
            Screen::Fields => String::new(),
        }
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
        match (&self.modal, self.screen) {
            (Some(Modal::Paycheck(_)), _) => vec![help::PAYCHECK_FORM],
            (Some(Modal::Field(_)), _) => vec![help::FIELD_FORM],
            (Some(Modal::DeletePaycheck(_) | Modal::DeleteField(_)), _) => vec![help::CONFIRM],
            (None, Screen::Sheet) => vec![help::SHEET, help::GLOBAL],
            (None, Screen::Fields) => vec![help::FIELDS, help::GLOBAL],
        }
    }

    /// The open form's keys first, then the screen's, then the global keys.
    fn help_topics(&self) -> Vec<(&'static str, &'static [Entry])> {
        let mut topics = Vec::new();
        match &self.modal {
            Some(Modal::Paycheck(_)) => topics.push(("Paycheck form", help::PAYCHECK_FORM)),
            Some(Modal::Field(_)) => topics.push(("Field form", help::FIELD_FORM)),
            _ => {}
        }
        topics.push(match self.screen {
            Screen::Sheet => ("Sheet", help::SHEET),
            Screen::Fields => ("Fields", help::FIELDS),
        });
        topics.push(("Everywhere", help::GLOBAL));
        topics
    }
}

fn is_yes(key: KeyEvent) -> bool {
    key.code == KeyCode::Char('y') && is_bare(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Kind;
    use crate::money::Cents;
    use crate::tui::test_support::{
        STUB, app_with, ctrl, day, inside, key, press, screen, shift, type_text,
    };
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
    fn shift_arrows_jump_a_screen_of_paychecks() {
        let dates: Vec<NaiveDate> = (0..12)
            .map(|i| day(2026, 1, 2) + chrono::Days::new(14 * i))
            .collect();
        let checks: Vec<_> = dates.iter().map(|&d| (d, STUB)).collect();
        let mut app = app_with(&checks, day(2026, 6, 10));
        // At 80 wide, four paycheck columns fit.
        screen(&mut app, 80, 30);
        app.on_key(shift(KeyCode::Left));
        assert_eq!(app.sheet.selected, 7);
        let header = inside(&screen(&mut app, 80, 30))[0].clone();
        assert!(header.contains("02-27") && header.contains("◀ 04-10 ▶"));
        assert!(!header.contains("04-24"));
        app.on_key(shift(KeyCode::Left));
        app.on_key(shift(KeyCode::Left));
        assert_eq!(app.sheet.selected, 0);
        app.on_key(shift(KeyCode::Right));
        assert_eq!(app.sheet.selected, 4);
        app.on_key(shift(KeyCode::Right));
        app.on_key(shift(KeyCode::Right));
        assert_eq!(app.sheet.selected, 11);
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
    fn the_border_shows_the_sheets_year_and_is_bare_on_fields() {
        let mut app = standard();
        let text = screen(&mut app, 80, 30);
        assert!(text.starts_with("┌ 2026 ─"), "{text}");
        press(&mut app, KeyCode::Char('['));
        assert!(screen(&mut app, 80, 30).starts_with("┌ 2025 ─"));
        press(&mut app, KeyCode::Char('2'));
        assert!(screen(&mut app, 80, 30).starts_with("┌──"));
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
        assert_eq!(
            app.status.as_ref().unwrap().text,
            "Saved the paycheck dated 2026-01-30"
        );
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
        assert!(screen(&mut app, 100, 30).contains("Edit paycheck"));
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
        assert_eq!(
            app.status.as_ref().unwrap().text,
            "Deleted the paycheck dated 2026-01-16"
        );
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
        let text = inside(&screen(&mut app, 100, 40)).join("\n");
        let form_at = text.find("Paycheck form").unwrap();
        let sheet_at = text.find("Sheet").unwrap();
        assert!(form_at < sheet_at);
        press(&mut app, KeyCode::Esc);
        assert!(!app.help);
        assert!(app.modal.is_some());
    }

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
        let lines = inside(&screen(&mut app, 100, 30));
        let row = lines.iter().find(|l| l.starts_with("Base Pay")).unwrap();
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
        assert_eq!(
            status.text,
            "Salary is used by a paycheck; archive it with x instead"
        );
    }

    #[test]
    fn deleting_an_unused_field_needs_y() {
        let mut app = standard();
        press(&mut app, KeyCode::Char('2'));
        let last = app.fields.len() - 1;
        app.fields_view.selected = last;
        press(&mut app, KeyCode::Char('d'));
        assert_eq!(
            app.status.as_ref().unwrap().text,
            "Delete 401K (Roth)? y to confirm"
        );
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
        assert!(
            !inside(&screen(&mut app, 100, 30))
                .iter()
                .any(|l| l.starts_with("HSA"))
        );
        press(&mut app, KeyCode::Char('a'));
        assert!(paycheck_form(&app).amounts.iter().all(|a| a.name != "HSA"));
        press(&mut app, KeyCode::Esc);
        press(&mut app, KeyCode::Char('['));
        let lines = inside(&screen(&mut app, 100, 30));
        let row = lines.iter().find(|l| l.starts_with("HSA")).unwrap();
        assert!(row.contains("100.00"), "{row}");
        press(&mut app, KeyCode::Char('e'));
        assert!(paycheck_form(&app).amounts.iter().any(|a| a.name == "HSA"));
    }

    #[test]
    fn on_a_short_terminal_the_empty_year_hint_shows_under_the_header() {
        let mut app = app_with(&[], day(2026, 3, 1));
        let text = screen(&mut app, 80, 24);
        assert!(inside(&text)[0].ends_with("YTD"));
        assert!(text.contains("No paychecks in 2026. Press a to add one."));
    }

    #[test]
    fn a_sheet_taller_than_the_terminal_scrolls_with_the_header_pinned() {
        let mut app = standard();
        let lines = inside(&screen(&mut app, 80, 24));
        assert!(!lines.iter().any(|l| l.starts_with("Net Pay")));
        for _ in 0..5 {
            press(&mut app, KeyCode::Down);
        }
        let lines = inside(&screen(&mut app, 80, 24));
        assert!(lines[0].ends_with("YTD"));
        assert!(lines.iter().any(|l| l.starts_with("Net Pay")));
        assert!(!lines.iter().any(|l| l.starts_with("Salary")));
        for _ in 0..5 {
            press(&mut app, KeyCode::Up);
        }
        let lines = inside(&screen(&mut app, 80, 24));
        assert!(lines[1].starts_with("Salary"));
    }

    #[test]
    fn a_paycheck_form_taller_than_the_terminal_keeps_the_focused_field_and_net_in_view() {
        let mut app = standard();
        for i in 0..20 {
            app.db
                .insert_field(&format!("Extra {i:02}"), Kind::Deduction)
                .unwrap();
        }
        app.reload().unwrap();
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::BackTab);
        let text = screen(&mut app, 100, 24);
        assert!(text.contains("› Extra 19"), "{text}");
        assert!(text.contains("Net 3,094.00"), "{text}");
    }
}
