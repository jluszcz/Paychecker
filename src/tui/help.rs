//! What every key does. The footers are joined from these same tables, so a
//! footer cannot drift from the panel that explains it.

use super::centered;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph};

#[derive(Copy, Clone, Debug)]
pub(super) struct Entry {
    pub(super) key: &'static str,
    /// The footer word; `None` keeps the key out of the footer but in the panel.
    pub(super) word: Option<&'static str>,
    pub(super) detail: &'static str,
}

const fn entry(key: &'static str, word: Option<&'static str>, detail: &'static str) -> Entry {
    Entry { key, word, detail }
}

pub(super) const GLOBAL: &[Entry] = &[
    entry("1", Some("sheet"), "Show the Sheet"),
    entry("2", Some("fields"), "Show the Fields list"),
    entry("?", Some("help"), "Open this panel (F1 too)"),
    entry("q", Some("quit"), "Quit"),
];

pub(super) const SHEET: &[Entry] = &[
    entry(
        "←/→",
        Some("paycheck"),
        "Select the previous or next paycheck",
    ),
    entry("Home/End", None, "Select the first or last paycheck"),
    entry("[ ]", Some("year"), "Show the previous or next year"),
    entry(
        "a",
        Some("add"),
        "Add a paycheck, prefilled from the latest one",
    ),
    entry("e", Some("edit"), "Edit the selected paycheck"),
    entry(
        "d",
        Some("delete"),
        "Delete the selected paycheck ('y' confirms)",
    ),
];

pub(super) const FIELDS: &[Entry] = &[
    entry("↑/↓", None, "Select a field"),
    entry(
        "Shift-↑/↓",
        Some("move"),
        "Move the selected field up or down",
    ),
    entry("a", Some("add"), "Add a field at the end"),
    entry(
        "e",
        Some("edit"),
        "Rename the selected field or change its kind",
    ),
    entry(
        "x",
        Some("archive"),
        "Archive or unarchive the selected field",
    ),
    entry(
        "d",
        Some("delete"),
        "Delete the selected field if no paycheck uses it ('y' confirms)",
    ),
];

pub(super) const PAYCHECK_FORM: &[Entry] = &[
    entry("Tab", Some("next"), "Next field; Shift-Tab goes back"),
    entry(
        "←/→",
        None,
        "Date: a day, or a week with Shift. Amount: move the caret",
    ),
    entry("[ ]", None, "Date: a month"),
    entry("Ctrl+U", None, "Clear to the start of the field"),
    entry("Enter", Some("save"), "Save the paycheck"),
    entry("Esc", Some("cancel"), "Close without saving"),
];

pub(super) const FIELD_FORM: &[Entry] = &[
    entry("Tab", Some("next"), "Switch between Name and Kind"),
    entry("←/→", None, "Kind: switch between Income and Deduction"),
    entry("Enter", Some("save"), "Save the field"),
    entry("Esc", Some("cancel"), "Close without saving"),
];

pub(super) const CONFIRM: &[Entry] = &[entry(
    "y",
    Some("confirm"),
    "Confirm; any other key cancels",
)];

pub(super) const HELP: &[Entry] = &[entry(
    "Esc",
    Some("close"),
    "Close this panel (? and F1 too)",
)];

pub(super) fn footer(tables: &[&[Entry]]) -> String {
    tables
        .iter()
        .flat_map(|table| table.iter())
        .filter_map(|e| e.word.map(|w| format!("{} {w}", e.key)))
        .collect::<Vec<_>>()
        .join("  ")
}

pub(super) fn render(frame: &mut Frame, area: Rect, topics: &[(&str, &[Entry])]) {
    let key_w = topics
        .iter()
        .flat_map(|(_, entries)| entries.iter())
        .map(|e| e.key.chars().count())
        .max()
        .unwrap_or(0);
    let mut lines: Vec<Line> = Vec::new();
    for (title, entries) in topics {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        lines.push(Line::styled(
            title.to_string(),
            Style::new().add_modifier(Modifier::BOLD),
        ));
        for e in *entries {
            lines.push(Line::from(format!("  {:<key_w$}  {}", e.key, e.detail)));
        }
    }
    let width = lines.iter().map(Line::width).max().unwrap_or(0) as u16 + 2;
    let popup = centered(area, width, lines.len() as u16 + 2);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" Help ")),
        popup,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::test_support::draw;

    const ALL: &[&[Entry]] = &[
        GLOBAL,
        SHEET,
        FIELDS,
        PAYCHECK_FORM,
        FIELD_FORM,
        CONFIRM,
        HELP,
    ];

    #[test]
    fn the_footer_joins_the_keys_that_have_a_word() {
        assert_eq!(
            footer(&[SHEET, GLOBAL]),
            "←/→ paycheck  [ ] year  a add  e edit  d delete  1 sheet  2 fields  ? help  q quit"
        );
    }

    #[test]
    fn no_table_names_a_key_twice() {
        for table in ALL {
            let mut keys: Vec<_> = table.iter().map(|e| e.key).collect();
            keys.sort_unstable();
            keys.dedup();
            assert_eq!(keys.len(), table.len());
        }
    }

    #[test]
    fn the_same_action_uses_the_same_key_on_both_screens() {
        for (key, word) in [("a", "add"), ("e", "edit"), ("d", "delete")] {
            for table in [SHEET, FIELDS] {
                let entry = table.iter().find(|e| e.key == key).unwrap();
                assert_eq!(entry.word, Some(word));
            }
        }
    }

    #[test]
    fn the_panel_lists_each_topic_with_its_keys() {
        let text = draw(70, 24, |f| {
            let area = f.area();
            render(f, area, &[("Sheet", SHEET), ("Everywhere", GLOBAL)])
        });
        assert!(text.contains("Help"));
        assert!(text.contains("Sheet"));
        assert!(text.contains("Show the previous or next year"));
        assert!(text.contains("Everywhere"));
    }
}
