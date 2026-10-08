//! What every key does. The footers are joined from these same tables, so a
//! footer cannot drift from the panel that explains it.

pub(super) use jluszcz_finance_utils::tui::help::Entry;
use jluszcz_finance_utils::tui::help::footer_items;
pub(super) use jluszcz_finance_utils::tui::help::render_panel as render;

/// `Some(word)` puts the key in the footer; `None` keeps it in the panel only.
const fn entry(key: &'static str, word: Option<&'static str>, detail: &'static str) -> Entry {
    match word {
        Some(word) => Entry::own(key, word, detail),
        None => Entry::hidden(key, detail),
    }
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
    entry(
        "⇧←→",
        Some("page"),
        "Jump a screen of paychecks back or ahead; PgUp/PgDn do the same",
    ),
    entry("Home/End", None, "Select the first or last paycheck"),
    entry("↑/↓", None, "Scroll the rows when they do not fit"),
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
    entry("⇧↑↓", Some("move"), "Move the selected field up or down"),
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
    entry("Tab", Some("next"), "Next field; ⇧Tab goes back"),
    entry(
        "←/→",
        None,
        "Date: a day, or a week with ⇧. Amount: move the caret",
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
    footer_items(tables).join("  ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::test_support::draw;
    use jluszcz_finance_utils::tui::help::{Label, duplicate_keys};

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
            "←/→ paycheck  ⇧←→ page  [ ] year  a add  e edit  d delete  1 sheet  2 fields  ? help  q quit"
        );
    }

    #[test]
    fn no_table_names_a_key_twice() {
        for table in ALL {
            assert!(
                duplicate_keys(table).is_empty(),
                "{:?}",
                duplicate_keys(table)
            );
        }
    }

    #[test]
    fn the_same_action_uses_the_same_key_on_both_screens() {
        for (key, word) in [("a", "add"), ("e", "edit"), ("d", "delete")] {
            for table in [SHEET, FIELDS] {
                let entry = table.iter().find(|e| e.key == key).unwrap();
                assert_eq!(entry.label, Label::Own(word));
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
