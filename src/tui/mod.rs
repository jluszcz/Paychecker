//! The terminal UI.

// Nothing outside the tests calls into these modules until `App` is wired up
// to `run`.
#![allow(dead_code)]

mod fields;
mod form;
mod help;
mod sheet;
mod text;

#[cfg(test)]
mod test_support;

use ratatui::layout::Rect;

/// A `width` × `height` rectangle centered in `area`, shrunk to fit it.
pub(super) fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}
