//! The report as one page.
//!
//! Self-contained by requirement: inline CSS, no font, no request of any
//! kind. A phone opening this out of a sync folder may be offline, and
//! anything fetched would render half-drawn or not at all. The only script is
//! the shared shell's, inline, which switches the year tabs; the panels are
//! plain tables, so a reader whose script never runs still gets every figure
//! from the open year.

use super::Snapshot;
use crate::calc::{self, Sheet};
use crate::money::Cents;
use chrono::Datelike;
use jluszcz_finance_utils::report::escape;
use jluszcz_finance_utils::report::html::{self as shared, App, NavTab};

/// A year's tab id. The `y` keeps the id a valid CSS identifier, which
/// cannot start with a digit.
pub(super) fn tab_id(year: i32) -> String {
    format!("y{year}")
}

/// Today's year when it has paychecks, and otherwise the newest year: a page
/// opened in the first days of January should not open on an empty tab.
fn opening_year(snapshot: &Snapshot) -> Option<i32> {
    let this_year = snapshot.today.year();
    let years = || snapshot.sheets.iter().map(|s| s.year);
    years().find(|&y| y == this_year).or_else(|| years().next())
}

/// One tab per year, newest first, as the sheets are. An empty snapshot has
/// the one `Paychecks` tab, whose panel says there is nothing yet.
fn tabs(snapshot: &Snapshot) -> Vec<NavTab> {
    if snapshot.sheets.is_empty() {
        return vec![NavTab::new("paychecks", "Paychecks")];
    }
    snapshot
        .sheets
        .iter()
        .map(|s| NavTab::new(tab_id(s.year), s.year.to_string()))
        .collect()
}

/// An amount with its cents in a span of their own, which a phone's rules
/// hide: on a narrow screen a column of whole dollars is a column more in view.
/// Hiding the span truncates rather than rounds, so a phone shows the dollars
/// the figure actually contains.
fn money(cents: Cents) -> String {
    let text = cents.to_string();
    let (whole, fraction) = text.split_at(text.len() - 3);
    format!("{whole}<span class=\"c\">{fraction}</span>")
}

/// One row of the grid: a label, a cell per paycheck, and the YTD cell. Only
/// the label is escaped, because the cells are figures this crate formatted.
fn grid_row(
    class: &str,
    label: &str,
    cells: impl IntoIterator<Item = String>,
    ytd: String,
) -> String {
    let cells: String = cells
        .into_iter()
        .map(|c| format!("<td class=\"n\">{c}</td>"))
        .collect();
    let class = if class.is_empty() {
        String::new()
    } else {
        format!(" class=\"{class}\"")
    };
    format!(
        "<tr{class}><th>{}</th>{cells}<td class=\"n ytd\">{ytd}</td></tr>",
        escape(label)
    )
}

/// A year's Sheet with its paychecks newest first, the reverse of the Sheet
/// screen: this page is read on a phone to check the latest pay. Then the
/// amounts, a rule, `Net`, a gap, and the percentages, as the screen has them.
fn grid(sheet: &Sheet) -> String {
    let dates: String = sheet
        .columns
        .iter()
        .rev()
        .map(|c| format!("<th class=\"n d\">{}</th>", c.date.format("%m-%d")))
        .collect();
    let mut body = String::new();
    for r in &sheet.rows {
        let cells = r
            .cells
            .iter()
            .rev()
            .map(|c| c.map_or_else(String::new, money));
        body.push_str(&grid_row("", &r.name, cells, money(r.ytd)));
    }
    let net = sheet.net.iter().rev().map(|&c| money(c));
    body.push_str(&grid_row("net", "Net", net, money(sheet.net_ytd)));
    body.push_str(&format!(
        "<tr class=\"gap\"><td colspan=\"{}\"></td></tr>",
        sheet.columns.len() + 2
    ));
    for r in &sheet.percent_rows {
        let cells = r.cells.iter().rev().map(|p| calc::show(*p));
        body.push_str(&grid_row("", &r.label, cells, calc::show(r.ytd)));
    }
    format!(
        "<table><thead><tr><th></th>{dates}<th class=\"n ytd\">YTD</th></tr></thead>\
         <tbody>{body}</tbody></table>"
    )
}

/// The grid's metrics, and the one color the shell has no token for. The
/// shell owns the base tokens, the body, the tab bar and the footer.
///
/// A year is up to ~26 paycheck columns, far wider than a phone, so the panel
/// scrolls sideways and the label column is sticky with an opaque background
/// so the figures slide under it. `border-collapse:separate` because a sticky
/// cell's borders do not travel with it under `collapse`. `n` and `d` never
/// wrap: a comma or a hyphen is a break opportunity a narrow column would
/// take, drawing `4,` over `000.00`.
///
/// YTD is pinned to the right edge the same way, so a reader scrolling back
/// through the year never loses the total.
///
/// Every other paycheck column is shaded: a year is ~26 look-alike columns
/// of figures, and the band keeps the eye on one down the page. The label is
/// the first cell of every row and YTD the last, so `even` minus the last is
/// exactly the 1st, 3rd, 5th... paycheck; the gap row's one cell is neither.
///
/// A phone gets one media query that buys columns: a smaller face, half the
/// padding, cents hidden, and a label column fixed at `6rem` that wraps
/// its longer names. The fixed width is what lets the columns snap: a swipe
/// settles each paycheck against the label column's edge, `6rem` plus its
/// `0.25rem` padding either side, rather than half under it.
const STYLE: &str = "\
    :root{--band:#f2f3f5}\
    @media (prefers-color-scheme: dark){:root{--band:#1e2126}}\
    table{border-collapse:separate;border-spacing:0;font-size:0.82rem}\
    td,th{padding:0.25rem 0.5rem;border-bottom:1px solid var(--rule);text-align:left}\
    th{font-weight:600}\
    .n{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}\
    .d{white-space:nowrap}\
    tr>:nth-child(even):not(:last-child){background:var(--band)}\
    .ytd{position:sticky;right:0;background:var(--bg);border-left:1px solid var(--rule)}\
    tr>th:first-child{position:sticky;left:0;background:var(--bg);white-space:nowrap}\
    tr.net th,tr.net td{border-top:2px solid var(--fg)}\
    tr.gap td{border-bottom:none;padding:0.5rem}\
    section[role=tabpanel]{overflow-x:auto}\
    @media (max-width: 480px){\
    table{font-size:0.75rem}\
    td,th{padding:0.2rem 0.25rem}\
    .c{display:none}\
    section[role=tabpanel]{scroll-snap-type:x mandatory;scroll-padding-left:6.5rem}\
    th.d{scroll-snap-align:start}\
    tr>th:first-child{white-space:normal;width:6rem;min-width:6rem;max-width:6rem}\
    }";

const STAMP_FORMAT: &str = "%Y-%m-%d %H:%M";

pub fn page(snapshot: &Snapshot) -> String {
    let panels = if snapshot.sheets.is_empty() {
        vec!["<p>No paychecks yet.</p>".to_string()]
    } else {
        snapshot.sheets.iter().map(grid).collect()
    };
    let open = opening_year(snapshot)
        .and_then(|year| snapshot.sheets.iter().position(|s| s.year == year))
        .unwrap_or(0);
    shared::app_page(&App {
        title: "Paychecks",
        favicon: None,
        style: STYLE,
        tabs: &tabs(snapshot),
        open,
        panels: &panels,
        data: None,
        script: None,
        stamp: &format!("Written {}", snapshot.generated_at.format(STAMP_FORMAT)),
    })
}

#[cfg(test)]
mod tests {
    use super::super::fixture::{day, snapshot, with_checks};
    use super::*;
    use crate::db::Kind;
    use crate::money::Cents;

    #[test]
    fn the_page_makes_no_external_request_and_loads_no_script() {
        let db = with_checks(&[day(2026, 1, 2)]);
        let page = page(&snapshot(&db, day(2026, 1, 2)));
        assert!(!page.contains("http"), "the page reaches out of itself");
        assert!(!page.contains("<script src"), "the page loads script");
        assert!(
            !page.contains("<link rel=\"stylesheet"),
            "the page loads CSS"
        );
    }

    #[test]
    fn the_page_has_one_tab_per_year_newest_first() {
        let db = with_checks(&[day(2025, 12, 19), day(2026, 1, 2)]);
        let page = page(&snapshot(&db, day(2026, 1, 2)));
        let newer = page.find("data-t=\"y2026\"").expect("no 2026 tab");
        let older = page.find("data-t=\"y2025\"").expect("no 2025 tab");
        assert!(newer < older, "the years are not newest first");
        assert!(page.contains("<span class=\"l\">2026</span>"), "{page}");
        for year in [2025, 2026] {
            assert!(
                page.contains(&format!("id=\"y{year}-panel\"")),
                "no {year} panel"
            );
        }
    }

    #[test]
    fn the_page_opens_on_the_year_today_falls_in() {
        let db = with_checks(&[day(2025, 12, 19), day(2026, 1, 2)]);
        let page = page(&snapshot(&db, day(2025, 12, 31)));
        assert!(
            page.contains("id=\"y2025-panel\" role=\"tabpanel\">"),
            "{page}"
        );
        assert!(
            page.contains("id=\"y2026-panel\" role=\"tabpanel\" hidden>"),
            "{page}"
        );
        assert!(
            page.contains("data-t=\"y2025\" aria-selected=\"true\""),
            "{page}"
        );
    }

    #[test]
    fn the_page_opens_on_the_newest_year_when_this_year_has_no_paychecks() {
        let db = with_checks(&[day(2025, 12, 19), day(2026, 1, 2)]);
        let page = page(&snapshot(&db, day(2027, 3, 1)));
        assert!(
            page.contains("id=\"y2026-panel\" role=\"tabpanel\">"),
            "{page}"
        );
        assert!(
            page.contains("id=\"y2025-panel\" role=\"tabpanel\" hidden>"),
            "{page}"
        );
    }

    #[test]
    fn a_field_name_carrying_markup_is_escaped() {
        let db = with_checks(&[day(2026, 1, 2)]);
        db.update_field(db.field_id("Salary"), "<b>Pay & \"Co\"</b>", Kind::Income)
            .unwrap();
        let page = page(&snapshot(&db, day(2026, 1, 2)));
        assert!(
            page.contains("&lt;b&gt;Pay &amp; &quot;Co&quot;&lt;/b&gt;"),
            "{page}"
        );
        assert!(
            !page.contains("<b>Pay"),
            "the name reached the page as markup"
        );
    }

    #[test]
    fn an_empty_database_draws_the_no_paychecks_panel() {
        let page = page(&snapshot(&with_checks(&[]), day(2026, 1, 2)));
        assert!(page.contains("No paychecks yet."), "{page}");
        assert!(
            !page.contains("data-t=\"y"),
            "an empty page drew a year tab"
        );
    }

    #[test]
    fn a_years_grid_carries_every_paycheck_date_its_ytd_and_its_net() {
        let db = with_checks(&[day(2026, 1, 2), day(2026, 1, 16)]);
        let page = page(&snapshot(&db, day(2026, 1, 16)));
        assert!(
            page.contains(
                "<th class=\"n d\">01-16</th><th class=\"n d\">01-02</th><th class=\"n ytd\">YTD</th>"
            ),
            "{page}"
        );
        assert!(page.contains("<tr><th>Salary</th><td class=\"n\">4,000<span class=\"c\">.00</span></td><td class=\"n\">4,000<span class=\"c\">.00</span></td><td class=\"n ytd\">8,000<span class=\"c\">.00</span></td></tr>"), "{page}");
        assert!(page.contains("<tr class=\"net\"><th>Net</th><td class=\"n\">3,400<span class=\"c\">.00</span></td><td class=\"n\">3,400<span class=\"c\">.00</span></td><td class=\"n ytd\">6,800<span class=\"c\">.00</span></td></tr>"), "{page}");
        assert!(page.contains("<tr><th>Federal Tax</th><td class=\"n\">15.00%</td><td class=\"n\">15.00%</td><td class=\"n ytd\">15.00%</td></tr>"), "{page}");
        assert!(page.contains("<tr><th>Net Pay</th><td class=\"n\">85.00%</td><td class=\"n\">85.00%</td><td class=\"n ytd\">85.00%</td></tr>"), "{page}");
    }

    #[test]
    fn an_amount_a_paycheck_does_not_have_is_an_empty_cell() {
        let db = with_checks(&[day(2026, 1, 2), day(2026, 1, 16)]);
        let page = page(&snapshot(&db, day(2026, 1, 16)));
        assert!(page.contains("<tr><th>Medicare</th><td class=\"n\"></td><td class=\"n\"></td><td class=\"n ytd\">0<span class=\"c\">.00</span></td></tr>"), "{page}");
    }

    /// The report reads from the latest paycheck back, the opposite of the
    /// Sheet screen: on a phone the newest figures are the ones checked.
    #[test]
    fn the_newest_paycheck_is_the_leftmost_column_in_every_row() {
        let db = with_checks(&[day(2026, 1, 2)]);
        let (salary, federal) = (db.field_id("Salary"), db.field_id("Federal Tax"));
        db.insert_paycheck(
            day(2026, 1, 16),
            &[(salary, Cents(410_000)), (federal, Cents(60_000))],
        )
        .unwrap();
        let page = page(&snapshot(&db, day(2026, 1, 16)));
        assert!(
            page.contains("<th class=\"n d\">01-16</th><th class=\"n d\">01-02</th>"),
            "{page}"
        );
        assert!(
            page.contains(
                "<th>Salary</th><td class=\"n\">4,100<span class=\"c\">.00</span></td><td class=\"n\">4,000<span class=\"c\">.00</span></td>"
            ),
            "{page}"
        );
        assert!(
            page.contains("<th>Net</th><td class=\"n\">3,500<span class=\"c\">.00</span></td><td class=\"n\">3,400<span class=\"c\">.00</span></td>"),
            "{page}"
        );
        assert!(
            page.contains(
                "<th>Federal Tax</th><td class=\"n\">14.63%</td><td class=\"n\">15.00%</td>"
            ),
            "{page}"
        );
    }

    /// A year is wider than a phone, so YTD sticks to the panel's right edge
    /// the way the labels stick to its left, and stays in view while the
    /// paychecks scroll between them.
    #[test]
    fn the_ytd_column_is_pinned_to_the_right_edge() {
        let page = page(&snapshot(&with_checks(&[day(2026, 1, 2)]), day(2026, 1, 2)));
        assert!(
            page.contains(".ytd{position:sticky;right:0;background:var(--bg)"),
            "{page}"
        );
        assert!(page.contains("<th class=\"n ytd\">YTD</th>"), "{page}");
    }

    /// On a phone the cents are the first thing to go, so every amount
    /// carries them in a span the narrow-screen rules can hide. Percentages
    /// keep their decimals: there they are the figure, not noise.
    #[test]
    fn an_amounts_cents_are_split_off_so_a_phone_can_hide_them() {
        let page = page(&snapshot(&with_checks(&[day(2026, 1, 2)]), day(2026, 1, 2)));
        assert!(
            page.contains("<td class=\"n\">4,000<span class=\"c\">.00</span></td>"),
            "{page}"
        );
        assert!(page.contains("<td class=\"n\">15.00%</td>"), "{page}");
    }

    /// Everything a phone needs is in one media query: less padding, a
    /// smaller face, a fixed label column that wraps, cents hidden, and the
    /// columns snapping to the label column's edge.
    #[test]
    fn a_narrow_screen_tightens_the_grid_and_snaps_its_columns() {
        let page = page(&snapshot(&with_checks(&[day(2026, 1, 2)]), day(2026, 1, 2)));
        let narrow = page
            .split("@media (max-width: 480px){")
            .nth(1)
            .expect("no narrow-screen rules");
        for rule in [
            "table{font-size:0.75rem}",
            ".c{display:none}",
            "section[role=tabpanel]{scroll-snap-type:x mandatory;scroll-padding-left:6.5rem}",
            "th.d{scroll-snap-align:start}",
            "tr>th:first-child{white-space:normal;width:6rem;min-width:6rem;max-width:6rem}",
        ] {
            assert!(narrow.contains(rule), "missing {rule}: {narrow}");
        }
    }

    /// A year runs to ~26 look-alike columns of figures, so every other
    /// paycheck is shaded to keep the eye on one down the page. The label is
    /// the first cell and YTD the last, so neither is a paycheck.
    #[test]
    fn every_other_paycheck_column_is_shaded_but_not_ytd() {
        let page = page(&snapshot(&with_checks(&[day(2026, 1, 2)]), day(2026, 1, 2)));
        assert!(
            page.contains("tr>:nth-child(even):not(:last-child){background:var(--band)}"),
            "{page}"
        );
        assert!(page.contains("--band:#f2f3f5"), "no light band");
        assert!(page.contains("--band:#1e2126"), "no dark band");
    }

    #[test]
    fn the_footer_says_when_the_page_was_written() {
        let page = page(&snapshot(&with_checks(&[]), day(2026, 1, 16)));
        assert!(page.contains("Written 2026-01-16 09:30"), "{page}");
    }
}
