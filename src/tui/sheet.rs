//! The Sheet screen: a year's paychecks as columns, fields as rows.

use crate::calc::{self, Sheet};
use crate::money::Cents;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;
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
}

impl SheetView {
    pub(super) fn new(year: i32, selected: usize) -> Self {
        Self {
            year,
            selected,
            scroll: 0,
        }
    }
}

/// Draw `sheet`: fixed label and YTD columns, and as many paycheck columns
/// between them as fit, scrolled so the selected one shows.
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
    if view.selected < view.scroll {
        view.scroll = view.selected;
    }
    if view.selected >= view.scroll + visible {
        view.scroll = view.selected + 1 - visible;
    }
    let len = sheet.columns.len();
    let start = view.scroll.min(len);
    let end = (start + visible).min(len);

    let row = |label: &str, cells: Vec<String>, ytd: String| {
        let mut text = format!("{label:<label_w$}");
        for cell in cells {
            text.push_str(&format!(" {cell:>COL$}"));
        }
        text.push_str(&format!(" │ {ytd:>COL$}"));
        Line::from(text)
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
    let mut lines = vec![row(&sheet.year.to_string(), header, "YTD".to_string())];
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
    if len == 0 {
        lines.push(Line::default());
        lines.push(Line::from(format!(
            "No paychecks in {}. Press a to add one.",
            sheet.year
        )));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{self, Field, Paycheck};
    use crate::tui::test_support::{day, draw, paycheck};

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
            .map(|(i, &(m, d))| paycheck(i as i64 + 1, day(2026, m, d), fields, PAY))
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
                "2026", "01-02", "◀ 01-16 ▶", "YTD"
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
    fn a_year_with_no_paychecks_shows_the_labels_and_a_hint() {
        let fields = fields();
        let mut view = SheetView::new(2027, 0);
        let lines = drawn(80, &mut view, &fields, &checks(&fields, &[(1, 2)]));
        assert_eq!(lines[0], format!("{:<LABEL$} │ {:>10}", "2027", "YTD"));
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
            format!("{:<LABEL$} {:>10} │ {:>10}", "2026", "◀ 01-30 ▶", "YTD")
        );
        view.selected = 0;
        let lines = drawn(41, &mut view, &fields, &checks);
        assert_eq!(
            lines[0],
            format!("{:<LABEL$} {:>10} │ {:>10}", "2026", "◀ 01-02 ▶", "YTD")
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
