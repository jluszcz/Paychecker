//! The Sheet screen: a year's paychecks as columns, fields as rows.

use crate::calc::{self, Sheet};
use crate::money::Cents;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
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
    /// How many paycheck columns the last draw fit: one page.
    visible: usize,
    /// The first row drawn below the pinned header. `render` clamps it, so
    /// callers may step it past the end.
    pub(super) row_scroll: usize,
}

impl SheetView {
    pub(super) fn new(year: i32, selected: usize) -> Self {
        Self {
            year,
            selected,
            scroll: 0,
            visible: 1,
            row_scroll: 0,
        }
    }

    /// Move the selection and the drawn columns together by a page, so the
    /// selected paycheck keeps its place on screen. `last` is the last index.
    pub(super) fn page(&mut self, forward: bool, last: usize) {
        if forward {
            self.selected = (self.selected + self.visible).min(last);
            self.scroll += self.visible;
        } else {
            self.selected = self.selected.saturating_sub(self.visible);
            self.scroll = self.scroll.saturating_sub(self.visible);
        }
    }
}

/// Draw `sheet`: fixed label and YTD columns, and as many paycheck columns
/// between them as fit, scrolled so the selected one shows. The header stays
/// on the top line; the rows below it scroll when they do not fit. Column
/// headings and row labels are bold; the year is on the border around it.
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
    view.visible = visible;
    let len = sheet.columns.len();
    view.scroll = view.scroll.min(len.saturating_sub(visible));
    if view.selected < view.scroll {
        view.scroll = view.selected;
    }
    if view.selected >= view.scroll + visible {
        view.scroll = view.selected + 1 - visible;
    }
    let start = view.scroll.min(len);
    let end = (start + visible).min(len);

    let bold = Style::new().add_modifier(Modifier::BOLD);
    let row = |label: &str, cells: Vec<String>, ytd: String| {
        let mut text = String::new();
        for cell in cells {
            text.push_str(&format!(" {cell:>COL$}"));
        }
        text.push_str(&format!(" │ {ytd:>COL$}"));
        Line::from(vec![
            Span::styled(format!("{label:<label_w$}"), bold),
            Span::raw(text),
        ])
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
    let header = row("", header, "YTD".to_string()).style(bold);
    let mut lines = Vec::new();
    if len == 0 {
        lines.push(Line::from(format!(
            "No paychecks in {}. Press a to add one.",
            sheet.year
        )));
        lines.push(Line::default());
    }
    for r in &sheet.rows {
        lines.push(row(&r.name, amounts(&r.cells), r.ytd.to_string()));
    }
    let bar = label_w + (end - start) * (COL + 1) + 1;
    lines.push(Line::from(format!(
        "{}┼{}",
        "─".repeat(bar),
        "─".repeat(COL + 1)
    )));
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
    let body_rows = usize::from(area.height).saturating_sub(1);
    view.row_scroll = view.row_scroll.min(lines.len().saturating_sub(body_rows));
    let mut shown = vec![header];
    shown.extend(lines.into_iter().skip(view.row_scroll));
    frame.render_widget(Paragraph::new(shown), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{self, Field, Paycheck, PaycheckId};
    use crate::tui::test_support::{day, draw, draw_buffer, paycheck};

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
            .map(|(i, &(m, d))| paycheck(PaycheckId(i as i64 + 1), day(2026, m, d), fields, PAY))
            .collect()
    }

    fn drawn(
        width: u16,
        view: &mut SheetView,
        fields: &[Field],
        checks: &[Paycheck],
    ) -> Vec<String> {
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
            format!(
                "{:<LABEL$} {:>10} {:>10} │ {:>10}",
                "", "01-02", "◀ 01-16 ▶", "YTD"
            )
        );
    }

    #[test]
    fn amount_rows_show_each_paycheck_and_the_ytd_total() {
        let fields = fields();
        let mut view = SheetView::new(2026, 1);
        let lines = drawn(80, &mut view, &fields, &checks(&fields, &[(1, 2), (1, 16)]));
        assert_eq!(
            lines[1],
            format!(
                "{:<LABEL$} {:>10} {:>10} │ {:>10}",
                "Salary", "4,000.00", "4,000.00", "8,000.00"
            )
        );
        assert_eq!(
            line(&lines, "Social Security"),
            format!(
                "{:<LABEL$} {:>10} {:>10} │ {:>10}",
                "Social Security", "", "", "0.00"
            )
        );
    }

    #[test]
    fn a_rule_then_net_follow_the_amounts() {
        let fields = fields();
        let mut view = SheetView::new(2026, 1);
        let lines = drawn(80, &mut view, &fields, &checks(&fields, &[(1, 2), (1, 16)]));
        assert_eq!(
            lines[11],
            format!("{}┼{}", "─".repeat(LABEL + 2 * 11 + 1), "─".repeat(11))
        );
        assert_eq!(
            lines[12],
            format!(
                "{:<LABEL$} {:>10} {:>10} │ {:>10}",
                "Net", "3,400.00", "3,400.00", "6,800.00"
            )
        );
        assert_eq!(lines[13], "");
    }

    #[test]
    fn the_percent_block_lists_deductions_then_net_pay() {
        let fields = fields();
        let mut view = SheetView::new(2026, 1);
        let lines = drawn(80, &mut view, &fields, &checks(&fields, &[(1, 2), (1, 16)]));
        assert_eq!(
            lines[14],
            format!(
                "{:<LABEL$} {:>10} {:>10} │ {:>10}",
                "Federal Tax", "15.00%", "15.00%", "15.00%"
            )
        );
        assert_eq!(
            lines[23],
            format!(
                "{:<LABEL$} {:>10} {:>10} │ {:>10}",
                "Net Pay", "85.00%", "85.00%", "85.00%"
            )
        );
    }

    #[test]
    fn column_headings_and_row_labels_are_bold_and_amounts_are_not() {
        let fields = fields();
        let mut view = SheetView::new(2026, 0);
        let sheet = calc::sheet(2026, &fields, &checks(&fields, &[(1, 2)]));
        let buffer = draw_buffer(80, 30, |frame| {
            let area = frame.area();
            render(frame, area, &mut view, &sheet)
        });
        let bold = |x: usize, y: u16| buffer[(x as u16, y)].modifier.contains(Modifier::BOLD);
        let ytd = LABEL + 11 + 3 + 7;
        assert!(bold(LABEL + 6, 0), "the date heading");
        assert!(bold(ytd, 0), "the YTD heading");
        assert!(bold(0, 1), "the Salary label");
        assert!(!bold(LABEL + 6, 1), "a Salary amount");
        assert!(!bold(ytd, 1), "Salary's YTD");
    }

    #[test]
    fn a_year_with_no_paychecks_shows_the_labels_and_a_hint() {
        let fields = fields();
        let mut view = SheetView::new(2027, 0);
        let lines = drawn(80, &mut view, &fields, &checks(&fields, &[(1, 2)]));
        assert_eq!(lines[0], format!("{:<LABEL$} │ {:>10}", "", "YTD"));
        assert_eq!(
            line(&lines, "Net Pay"),
            format!("{:<LABEL$} │ {:>10}", "Net Pay", "—")
        );
        assert!(lines.contains(&"No paychecks in 2027. Press a to add one.".to_string()));
    }

    #[test]
    fn a_narrow_terminal_scrolls_to_keep_the_selected_paycheck_visible() {
        let fields = fields();
        let checks = checks(&fields, &[(1, 2), (1, 16), (1, 30)]);
        let mut view = SheetView::new(2026, 2);
        let lines = drawn(41, &mut view, &fields, &checks);
        assert_eq!(
            lines[0],
            format!("{:<LABEL$} {:>10} │ {:>10}", "", "◀ 01-30 ▶", "YTD")
        );
        view.selected = 0;
        let lines = drawn(41, &mut view, &fields, &checks);
        assert_eq!(
            lines[0],
            format!("{:<LABEL$} {:>10} │ {:>10}", "", "◀ 01-02 ▶", "YTD")
        );
    }

    #[test]
    fn paging_past_the_end_keeps_a_full_screen_of_columns() {
        let fields = fields();
        let checks = checks(&fields, &[(1, 2), (1, 16), (1, 30)]);
        let mut view = SheetView::new(2026, 0);
        // At 52 wide, two paycheck columns fit.
        drawn(52, &mut view, &fields, &checks);
        view.page(true, 2);
        assert_eq!(view.selected, 2);
        let lines = drawn(52, &mut view, &fields, &checks);
        assert_eq!(
            lines[0],
            format!(
                "{:<LABEL$} {:>10} {:>10} │ {:>10}",
                "", "01-16", "◀ 01-30 ▶", "YTD"
            )
        );
    }

    #[test]
    fn a_terminal_narrower_than_the_fixed_columns_still_draws() {
        let fields = fields();
        let mut view = SheetView::new(2026, 0);
        let lines = drawn(12, &mut view, &fields, &checks(&fields, &[(1, 2)]));
        assert!(lines[1].starts_with("Salary"));
    }
}
