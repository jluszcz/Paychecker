//! The report as one page.
//!
//! Self-contained by requirement: inline CSS, no script, no font, no request
//! of any kind. A phone opening this out of a sync folder may be offline, and
//! anything fetched would render half-drawn or not at all. That is also why
//! the year tabs are radio buttons and sibling selectors: a click handler
//! would be the one thing on the page that could fail to arrive.

use super::Snapshot;
use crate::calc::{self, Sheet};
use chrono::Datelike;

/// Every interpolation of user-typed text goes through here: a field named
/// with an angle bracket would otherwise truncate the page at its own row.
/// Escapes `&`, `<`, `>` and `"`, never `'`, because no attribute on this
/// page is single-quoted.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

/// A year's radio id. The `y` is there because a CSS id selector cannot
/// start with a digit, and `#2026` would match nothing.
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

/// The tab bar, and the radios that drive it. Every radio sits ahead of the
/// nav and every panel, because `:checked ~` only looks forward.
fn tab_bar(snapshot: &Snapshot) -> String {
    let open = opening_year(snapshot);
    let inputs: String = snapshot
        .sheets
        .iter()
        .map(|s| {
            let checked = if Some(s.year) == open { " checked" } else { "" };
            format!(
                "<input class=\"tab\" type=\"radio\" name=\"tab\" id=\"{}\"{checked}>",
                tab_id(s.year)
            )
        })
        .collect();
    let labels: String = snapshot
        .sheets
        .iter()
        .map(|s| format!("<label for=\"{}\">{}</label>", tab_id(s.year), s.year))
        .collect();
    format!("{inputs}<nav>{labels}</nav>")
}

/// Which panel shows, which label is lit, and where the focus ring goes: one
/// rule set per year, generated from the same list as the markup.
fn tab_rules(snapshot: &Snapshot) -> String {
    snapshot
        .sheets
        .iter()
        .map(|s| {
            let id = tab_id(s.year);
            format!(
                "#{id}:checked~nav label[for={id}]\
                 {{color:inherit;border-bottom-color:currentColor}}\
                 #{id}:focus-visible~nav label[for={id}]\
                 {{outline:2px solid currentColor;outline-offset:-2px}}\
                 #{id}:checked~#{id}-panel{{display:block}}"
            )
        })
        .collect()
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
        "<tr{class}><th>{}</th>{cells}<td class=\"n\">{ytd}</td></tr>",
        escape(label)
    )
}

/// A year's Sheet, laid out as `tui::sheet` draws it: paychecks across, the
/// amounts, a rule, `Net`, a gap, then the percentages.
fn grid(sheet: &Sheet) -> String {
    let dates: String = sheet
        .columns
        .iter()
        .map(|c| format!("<th class=\"n d\">{}</th>", c.date.format("%m-%d")))
        .collect();
    let mut body = String::new();
    for r in &sheet.rows {
        let cells = r
            .cells
            .iter()
            .map(|c| c.map_or_else(String::new, |c| c.to_string()));
        body.push_str(&grid_row("", &r.name, cells, r.ytd.to_string()));
    }
    let net = sheet.net.iter().map(ToString::to_string);
    body.push_str(&grid_row("net", "Net", net, sheet.net_ytd.to_string()));
    body.push_str(&format!(
        "<tr class=\"gap\"><td colspan=\"{}\"></td></tr>",
        sheet.columns.len() + 2
    ));
    for r in &sheet.percent_rows {
        let cells = r.cells.iter().map(|p| calc::show(*p));
        body.push_str(&grid_row("", &r.label, cells, calc::show(r.ytd)));
    }
    format!(
        "<table><thead><tr><th></th>{dates}<th class=\"n\">YTD</th></tr></thead>\
         <tbody>{body}</tbody></table>"
    )
}

/// A system font stack, tabular figures, the tab switch, and one media query
/// for dark mode.
///
/// A year is up to ~26 paycheck columns, far wider than a phone, so the panel
/// scrolls sideways and the label column is sticky with an opaque background
/// so the figures slide under it. `border-collapse:separate` because a sticky
/// cell's borders do not travel with it under `collapse`. `n` and `d` never
/// wrap: a comma or a hyphen is a break opportunity a narrow column would
/// take, drawing `4,` over `000.00`.
///
/// The radios are moved off the page rather than `display:none`d, which would
/// take them out of the focus order.
const STYLE: &str = "\
    :root{color-scheme:light dark;--bg:#ffffff;--fg:#1a1a1a;--rule:#dddddd;--muted:#666666}\
    @media (prefers-color-scheme: dark){\
    :root{--bg:#121212;--fg:#eeeeee;--rule:#333333;--muted:#999999}}\
    body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;\
    margin:0;padding:1rem;background:var(--bg);color:var(--fg)}\
    table{border-collapse:separate;border-spacing:0;font-size:0.82rem}\
    td,th{padding:0.25rem 0.5rem;border-bottom:1px solid var(--rule);text-align:left}\
    th{font-weight:600}\
    .n{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}\
    .d{white-space:nowrap}\
    tr>th:first-child{position:sticky;left:0;background:var(--bg);white-space:nowrap}\
    tr.net th,tr.net td{border-top:2px solid var(--fg)}\
    tr.gap td{border-bottom:none;padding:0.5rem}\
    input.tab{position:absolute;opacity:0;width:0;height:0}\
    nav{display:flex;flex-wrap:wrap;border-bottom:1px solid var(--rule);margin-bottom:0.8rem}\
    nav label{padding:0.5rem 0.7rem;margin-bottom:-1px;cursor:pointer;font-weight:600;\
    color:var(--muted);border-bottom:2px solid transparent}\
    section.panel{display:none;overflow-x:auto}\
    footer{border-top:1px solid var(--rule);margin-top:1.5rem;padding-top:0.6rem}\
    p.stamp{color:var(--muted);margin:0;font-size:0.85rem}";

const STAMP_FORMAT: &str = "%Y-%m-%d %H:%M";

pub fn page(snapshot: &Snapshot) -> String {
    let body = if snapshot.sheets.is_empty() {
        "<p>No paychecks yet.</p>".to_string()
    } else {
        let panels: String = snapshot
            .sheets
            .iter()
            .map(|s| {
                format!(
                    "<section class=\"panel\" id=\"{}-panel\">{}</section>",
                    tab_id(s.year),
                    grid(s)
                )
            })
            .collect();
        format!("{}{panels}", tab_bar(snapshot))
    };
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\"><head>\
         <meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>Paychecks</title><style>{STYLE}{}</style></head><body>\
         {body}\
         <footer><p class=\"stamp\">Written {}</p></footer>\
         </body></html>",
        tab_rules(snapshot),
        snapshot.generated_at.format(STAMP_FORMAT),
    )
}

#[cfg(test)]
mod tests {
    use super::super::fixture::{day, snapshot, with_checks};
    use super::*;
    use crate::db::Kind;

    #[test]
    fn the_page_makes_no_external_request_and_carries_no_script() {
        let db = with_checks(&[day(2026, 1, 2)]);
        let page = page(&snapshot(&db, day(2026, 1, 2)));
        assert!(!page.contains("http"), "the page reaches out of itself");
        assert!(!page.contains("<script"), "the page carries script");
    }

    #[test]
    fn the_page_has_one_tab_per_year_newest_first() {
        let db = with_checks(&[day(2025, 12, 19), day(2026, 1, 2)]);
        let page = page(&snapshot(&db, day(2026, 1, 2)));
        let newer = page
            .find("<label for=\"y2026\">2026</label>")
            .expect("no 2026 tab");
        let older = page
            .find("<label for=\"y2025\">2025</label>")
            .expect("no 2025 tab");
        assert!(newer < older, "the years are not newest first");
        for year in [2025, 2026] {
            assert!(
                page.contains(&format!("id=\"y{year}-panel\"")),
                "no {year} panel"
            );
            assert!(
                page.contains(&format!("#y{year}:checked~#y{year}-panel{{display:block}}")),
                "no {year} switch"
            );
        }
    }

    #[test]
    fn the_page_opens_on_the_year_today_falls_in() {
        let db = with_checks(&[day(2025, 12, 19), day(2026, 1, 2)]);
        let page = page(&snapshot(&db, day(2025, 12, 31)));
        assert!(page.contains("id=\"y2025\" checked"), "{page}");
        assert!(!page.contains("id=\"y2026\" checked"), "{page}");
    }

    #[test]
    fn the_page_opens_on_the_newest_year_when_this_year_has_no_paychecks() {
        let db = with_checks(&[day(2025, 12, 19), day(2026, 1, 2)]);
        let page = page(&snapshot(&db, day(2027, 3, 1)));
        assert!(page.contains("id=\"y2026\" checked"), "{page}");
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
        assert!(!page.contains("<nav>"), "an empty page drew a tab bar");
    }

    #[test]
    fn a_years_grid_carries_every_paycheck_date_its_ytd_and_its_net() {
        let db = with_checks(&[day(2026, 1, 2), day(2026, 1, 16)]);
        let page = page(&snapshot(&db, day(2026, 1, 16)));
        assert!(
            page.contains(
                "<th class=\"n d\">01-02</th><th class=\"n d\">01-16</th><th class=\"n\">YTD</th>"
            ),
            "{page}"
        );
        assert!(page.contains("<tr><th>Salary</th><td class=\"n\">4,000.00</td><td class=\"n\">4,000.00</td><td class=\"n\">8,000.00</td></tr>"), "{page}");
        assert!(page.contains("<tr class=\"net\"><th>Net</th><td class=\"n\">3,400.00</td><td class=\"n\">3,400.00</td><td class=\"n\">6,800.00</td></tr>"), "{page}");
        assert!(page.contains("<tr><th>Federal Tax</th><td class=\"n\">15.00%</td><td class=\"n\">15.00%</td><td class=\"n\">15.00%</td></tr>"), "{page}");
        assert!(page.contains("<tr><th>Net Pay</th><td class=\"n\">85.00%</td><td class=\"n\">85.00%</td><td class=\"n\">85.00%</td></tr>"), "{page}");
    }

    #[test]
    fn an_amount_a_paycheck_does_not_have_is_an_empty_cell() {
        let db = with_checks(&[day(2026, 1, 2), day(2026, 1, 16)]);
        let page = page(&snapshot(&db, day(2026, 1, 16)));
        assert!(page.contains("<tr><th>Medicare</th><td class=\"n\"></td><td class=\"n\"></td><td class=\"n\">0.00</td></tr>"), "{page}");
    }

    #[test]
    fn the_footer_says_when_the_page_was_written() {
        let page = page(&snapshot(&with_checks(&[]), day(2026, 1, 16)));
        assert!(page.contains("Written 2026-01-16 09:30"), "{page}");
    }
}
