//! Toolbar panel with navigation and rotation buttons.
//!
//! The toolbar sits at the top of the window and provides ten buttons:
//! rotate left CCW, rotate left CW, first, previous turn, previous page,
//! next page, next turn, last, rotate right CCW, rotate right CW.
//! All buttons are disabled until a book is open.

use eframe::egui::{self, Context};

use crate::App;

/// Actions that the toolbar may request after drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarAction {
    RotateLeftCcw,
    RotateLeftCw,
    First,
    PrevTurn,
    PrevPage,
    NextPage,
    NextTurn,
    Last,
    RotateRightCcw,
    RotateRightCw,
}

/// Draw the toolbar if it is visible and not in fullscreen mode.
///
/// Returns an action to apply, if any button was clicked.
pub fn draw(ctx: &Context, app: &App) -> Option<ToolbarAction> {
    if app.fullscreen() || !app.settings().toolbar_visible {
        return None;
    }

    let enabled = app.has_book();
    let mut action = None;

    egui::TopBottomPanel::top("toolbar")
        .exact_height(28.0)
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;

                if toolbar_button(ui, "↺L", "Rotate left page counter-clockwise", enabled) {
                    action = Some(ToolbarAction::RotateLeftCcw);
                }
                if toolbar_button(ui, "↻L", "Rotate left page clockwise", enabled) {
                    action = Some(ToolbarAction::RotateLeftCw);
                }

                ui.separator();

                if toolbar_button(ui, "⏮", "First page", enabled) {
                    action = Some(ToolbarAction::First);
                }
                if toolbar_button(ui, "◁", "Previous turn", enabled) {
                    action = Some(ToolbarAction::PrevTurn);
                }
                if toolbar_button(ui, "◀", "Previous page", enabled) {
                    action = Some(ToolbarAction::PrevPage);
                }
                if toolbar_button(ui, "▶", "Next page", enabled) {
                    action = Some(ToolbarAction::NextPage);
                }
                if toolbar_button(ui, "▷", "Next turn", enabled) {
                    action = Some(ToolbarAction::NextTurn);
                }
                if toolbar_button(ui, "⏭", "Last page", enabled) {
                    action = Some(ToolbarAction::Last);
                }

                ui.separator();

                if toolbar_button(ui, "↺R", "Rotate right page counter-clockwise", enabled) {
                    action = Some(ToolbarAction::RotateRightCcw);
                }
                if toolbar_button(ui, "↻R", "Rotate right page clockwise", enabled) {
                    action = Some(ToolbarAction::RotateRightCw);
                }
            });
        });

    action
}

fn toolbar_button(ui: &mut egui::Ui, label: &str, tooltip: &str, enabled: bool) -> bool {
    ui.add_enabled(enabled, egui::Button::new(label).small())
        .on_hover_text(tooltip)
        .clicked()
}

/// Apply a toolbar action to the app state.
pub fn apply(app: &mut App, action: ToolbarAction) {
    match action {
        ToolbarAction::RotateLeftCcw => app.rotate_left_page_ccw(),
        ToolbarAction::RotateLeftCw => app.rotate_left_page_cw(),
        ToolbarAction::First => app.first(),
        ToolbarAction::PrevTurn => app.prev_turn(),
        ToolbarAction::PrevPage => app.prev_page(),
        ToolbarAction::NextPage => app.next_page(),
        ToolbarAction::NextTurn => app.next_turn(),
        ToolbarAction::Last => app.last(),
        ToolbarAction::RotateRightCcw => app.rotate_right_page_ccw(),
        ToolbarAction::RotateRightCw => app.rotate_right_page_cw(),
    }
}

/// Returns true if the toolbar buttons should be enabled (a book is open).
#[allow(dead_code)]
pub fn buttons_enabled(app: &App) -> bool {
    app.has_book()
}

#[cfg(test)]
mod tests {
    use super::*;
    use comical_core::Rotation;

    /// A valid 4x3 RGB PNG for test fixtures.
    const PAGE_PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x03, 0x08, 0x02, 0x00, 0x00, 0x00, 0x3b,
        0x96, 0x39, 0x91, 0x00, 0x00, 0x00, 0x30, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x60,
        0x60, 0x38, 0xa1, 0xc1, 0x70, 0x22, 0x80, 0xe1, 0x44, 0x05, 0xc3, 0x09, 0x06, 0x2e, 0x9b,
        0x13, 0x46, 0x36, 0x27, 0xa2, 0x6c, 0x4e, 0x34, 0xd9, 0x9c, 0x60, 0x10, 0xa9, 0x38, 0x61,
        0x53, 0x71, 0x22, 0xa5, 0xe2, 0x44, 0x4f, 0xc5, 0x09, 0x00, 0x04, 0x4a, 0x0f, 0x79, 0xd3,
        0x19, 0xe5, 0xf2, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    fn make_test_book(count: usize) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tmpdir");
        for i in 0..count {
            let path = dir.path().join(format!("page_{i:03}.png"));
            std::fs::write(&path, PAGE_PNG).expect("write");
        }
        dir
    }

    #[test]
    fn buttons_disabled_when_no_book() {
        let app = App::new();
        assert!(!buttons_enabled(&app));
    }

    #[test]
    fn apply_rotate_left_cycles_rotation() {
        let dir = make_test_book(4);
        let mut app = App::new();
        app.open_book(dir.path().to_str().unwrap()).expect("open");
        assert_eq!(app.page_rotation(0), Rotation::North);
        apply(&mut app, ToolbarAction::RotateLeftCw);
        assert_eq!(app.page_rotation(0), Rotation::East);
        apply(&mut app, ToolbarAction::RotateLeftCcw);
        assert_eq!(app.page_rotation(0), Rotation::North);
    }

    #[test]
    fn apply_next_page_advances() {
        let dir = make_test_book(4);
        let mut app = App::new();
        app.open_book(dir.path().to_str().unwrap()).expect("open");
        assert_eq!(app.current_page(), Some(0));
        apply(&mut app, ToolbarAction::NextPage);
        assert_eq!(app.current_page(), Some(1));
    }

    #[test]
    fn apply_first_navigates_to_first_page() {
        let dir = make_test_book(6);
        let mut app = App::new();
        app.open_book(dir.path().to_str().unwrap()).expect("open");
        apply(&mut app, ToolbarAction::NextTurn);
        apply(&mut app, ToolbarAction::NextTurn);
        assert_ne!(app.current_page(), Some(0));
        apply(&mut app, ToolbarAction::First);
        assert_eq!(app.current_page(), Some(0));
    }
}