//! The modal forms: a paycheck's date and amounts, and a field's name and kind.

use super::centered;
use super::text::{TextBuffer, edit_key, is_bare};
use crate::calc::{self, Totals};
use crate::db::{Field, FieldId, Kind, Paycheck, PaycheckId};
use crate::money::Cents;
use anyhow::{Context, Result};
use chrono::NaiveDate;
use jluszcz_finance_utils::tui::date::Step;
pub(super) use jluszcz_finance_utils::tui::date::iso;
pub(super) use jluszcz_finance_utils::tui::date::parse as parse_date;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph};

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
            .and_then(|p| Step::days(14).apply(p.date))
            .unwrap_or(today);
        let mut shown: Vec<&Field> = fields.iter().filter(|f| !f.archived).collect();
        calc::sort_rows(&mut shown);
        let amounts = shown
            .into_iter()
            .map(|f| AmountInput::new(f, latest.and_then(|p| p.amounts.get(&f.id).copied())))
            .collect();
        Self {
            editing: None,
            date: TextBuffer::from(iso(date)),
            amounts,
            focus: 0,
            today,
        }
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
            match Step::from_key(key) {
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
            && let Some(next) = step.apply(date)
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

/// The live line under the amounts: `—` while any amount fails to parse.
pub(super) fn net_line(totals: Option<Totals>) -> String {
    match totals {
        Some(t) => format!(
            "Net {}  {}",
            t.net,
            calc::show(calc::percent(t.net, t.income))
        ),
        None => "Net —".to_string(),
    }
}

/// The date and amounts scroll inside the popup when they do not all fit, so
/// the focused row and the Net line are always in view.
pub(super) fn render_paycheck(frame: &mut Frame, area: Rect, form: &PaycheckForm) {
    let label_w = form
        .amounts
        .iter()
        .map(|a| a.name.chars().count())
        .max()
        .unwrap_or(0)
        .max("Date".len());
    let rows: Vec<(&str, &TextBuffer)> = std::iter::once(("Date", &form.date))
        .chain(form.amounts.iter().map(|a| (a.name.as_str(), &a.text)))
        .collect();
    let title = if form.editing.is_some() {
        " Edit paycheck "
    } else {
        " Add paycheck "
    };
    let width = (label_w + 4 + 16).max(32) as u16 + 2;
    // Two borders, and the blank line and Net line under the rows.
    let popup = centered(area, width, rows.len() as u16 + 4);
    let room = usize::from(popup.height).saturating_sub(4).max(1);
    let offset = form.focus.saturating_sub(room - 1);
    let mut lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .skip(offset)
        .take(room)
        .map(|(i, (label, text))| {
            let marker = if i == form.focus { "›" } else { " " };
            Line::from(format!("{marker} {label:<label_w$}  {}", text.value()))
        })
        .collect();
    lines.push(Line::default());
    lines.push(Line::from(net_line(form.totals())));
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(title)),
        popup,
    );
    let caret = rows[form.focus].1.caret();
    let x = popup.x + 1 + (label_w + 4 + caret) as u16;
    frame.set_cursor_position((
        x.min(popup.right().saturating_sub(2)),
        popup.y + 1 + (form.focus - offset) as u16,
    ));
}

pub(super) fn render_field(frame: &mut Frame, area: Rect, form: &FieldForm) {
    let marker = |focus| if form.focus == focus { "›" } else { " " };
    let lines = vec![
        Line::from(format!(
            "{} Name  {}",
            marker(FieldFocus::Name),
            form.name.value()
        )),
        Line::from(format!(
            "{} Kind  ◀ {} ▶",
            marker(FieldFocus::Kind),
            form.kind.label()
        )),
    ];
    let title = if form.editing.is_some() {
        " Edit field "
    } else {
        " Add field "
    };
    let popup = centered(area, 40, 4);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(title)),
        popup,
    );
    if form.focus == FieldFocus::Name {
        frame.set_cursor_position((popup.x + 1 + 8 + form.name.caret() as u16, popup.y + 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::tui::test_support::{STUB, ctrl, day, key, paycheck, shift};
    use ratatui::crossterm::event::KeyModifiers;

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
        form.amounts
            .iter()
            .find(|a| a.name == name)
            .unwrap()
            .text
            .value()
    }

    #[test]
    fn an_iso_date_parses() {
        assert_eq!(
            parse_date(" 2026-01-16 ", today()).unwrap(),
            day(2026, 1, 16)
        );
    }

    #[test]
    fn text_that_is_not_a_date_is_refused() {
        for bad in ["", "soon", "13/1", "2/30", "2026-02-30", "2026/01/16"] {
            assert!(parse_date(bad, today()).is_err(), "{bad:?} parsed");
        }
    }

    #[test]
    fn stepping_a_month_clamps_the_day() {
        assert_eq!(
            Step::NEXT_MONTH.apply(day(2026, 1, 31)),
            Some(day(2026, 2, 28))
        );
        assert_eq!(
            Step::PREVIOUS_MONTH.apply(day(2026, 3, 31)),
            Some(day(2026, 2, 28))
        );
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
        let latest = paycheck(
            1,
            day(2026, 1, 16),
            &fields,
            &[("Salary", 400_000), ("HSA", 10_000)],
        );
        fields
            .iter_mut()
            .find(|f| f.name == "HSA")
            .unwrap()
            .archived = true;
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
        let check = paycheck(
            7,
            day(2026, 1, 16),
            &fields,
            &[("Salary", 400_000), ("HSA", 10_000)],
        );
        fields
            .iter_mut()
            .find(|f| f.name == "HSA")
            .unwrap()
            .archived = true;
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
    fn ctrl_with_an_arrow_on_the_date_steps_nothing() {
        let fields = fields();
        let mut form = PaycheckForm::add(&fields, None, today());
        let before = form.date.value().to_string();
        form.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL));
        assert_eq!(form.date.value(), before);
    }

    #[test]
    fn adding_after_a_paycheck_at_a_month_end_prefills_into_the_next_month() {
        let fields = fields();
        let latest = paycheck(1, day(2028, 2, 20), &fields, STUB);
        let form = PaycheckForm::add(&fields, Some(&latest), today());
        assert_eq!(form.date.value(), "2028-03-05");
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
            Some(Totals {
                income: Cents(400_000),
                net: Cents(309_400)
            })
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
        assert_eq!(
            (form.editing, form.name.value()),
            (Some(fields[7].id), "HSA")
        );
        form.on_key(key(KeyCode::Left));
        form.on_key(key(KeyCode::Char('x')));
        assert_eq!(form.name.value(), "HSxA");
        assert_eq!(form.on_key(key(KeyCode::Enter)), Outcome::Submit);
    }
}
