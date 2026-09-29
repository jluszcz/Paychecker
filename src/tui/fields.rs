//! The Fields screen: every field in `position` order.

use crate::db::Field;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

#[derive(Debug, Default)]
pub(super) struct FieldsView {
    pub(super) selected: usize,
}

/// Archived fields are dimmed here and nowhere else: this is the one screen
/// whose subject is whether a field is in use.
pub(super) fn render(frame: &mut Frame, area: Rect, view: &FieldsView, fields: &[Field]) {
    let name_w = fields
        .iter()
        .map(|f| f.name.chars().count())
        .max()
        .unwrap_or(0);
    let lines: Vec<Line> = fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let marker = if i == view.selected { "›" } else { " " };
            let tag = if f.archived { "  archived" } else { "" };
            let text = format!("{marker} {:<name_w$}  {:<9}{tag}", f.name, f.kind.label());
            let mut style = Style::new();
            if f.archived {
                style = style.add_modifier(Modifier::DIM);
            }
            if i == view.selected {
                style = style.add_modifier(Modifier::REVERSED);
            }
            Line::styled(text, style)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::tui::test_support::{buffer_text, draw_buffer};

    #[test]
    fn each_field_shows_its_name_and_kind_with_the_selection_marked() {
        let fields = db::open_in_memory().unwrap().fields().unwrap();
        let view = FieldsView { selected: 1 };
        let text = buffer_text(&draw_buffer(60, 12, |f| {
            let area = f.area();
            render(f, area, &view, &fields)
        }));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], format!("  {:<15}  Income", "Salary"));
        assert_eq!(lines[1], format!("› {:<15}  Deduction", "Federal Tax"));
    }

    #[test]
    fn archived_fields_are_tagged_and_dimmed_here() {
        let db = db::open_in_memory().unwrap();
        db.set_archived(db.field_id("HSA"), true).unwrap();
        let fields = db.fields().unwrap();
        let view = FieldsView::default();
        let buffer = draw_buffer(60, 12, |f| {
            let area = f.area();
            render(f, area, &view, &fields)
        });
        let text = buffer_text(&buffer);
        let row = text.lines().position(|l| l.contains("HSA")).unwrap();
        assert!(
            text.lines()
                .nth(row)
                .unwrap()
                .ends_with("Deduction  archived")
        );
        assert!(buffer[(2, row as u16)].modifier.contains(Modifier::DIM));
        assert!(!buffer[(2, 1)].modifier.contains(Modifier::DIM));
    }
}
