//! Keyboard input mapping for the viewer.

use comical_core::{ViewMode, ZoomMode};
use eframe::egui::{Context, Key, ViewportCommand};

use crate::App;

/// Snapshot of the keys handled this frame.
struct Pressed {
    next_turn: bool,
    prev_turn: bool,
    next_page: bool,
    prev_page: bool,
    first: bool,
    last: bool,
    one_page: bool,
    two_page: bool,
    cycle_zoom: bool,
    zoom_width: bool,
    zoom_height: bool,
    zoom_full: bool,
    zoom_custom: bool,
    zoom_in: bool,
    zoom_out: bool,
    zoom_reset: bool,
    fullscreen: bool,
    toggle_direction: bool,
    rotate_left_ccw: bool,
    rotate_left_cw: bool,
    rotate_right_ccw: bool,
    rotate_right_cw: bool,
}

/// Handle global keyboard navigation, zoom and fullscreen.
///
/// Returns `true` if the input was consumed (the app handled it).
pub fn handle_input(ctx: &Context, app: &mut App) -> bool {
    let pressed = ctx.input(|input| {
        let shift = input.modifiers.shift;
        let ctrl = input.modifiers.ctrl;
        Pressed {
            next_turn: input.key_pressed(Key::ArrowRight)
                || input.key_pressed(Key::PageDown)
                || input.key_pressed(Key::J),
            prev_turn: input.key_pressed(Key::ArrowLeft)
                || input.key_pressed(Key::PageUp)
                || input.key_pressed(Key::K),
            next_page: input.key_pressed(Key::ArrowDown) || input.key_pressed(Key::Space),
            prev_page: input.key_pressed(Key::ArrowUp) || input.key_pressed(Key::Backspace),
            first: input.key_pressed(Key::Home),
            last: input.key_pressed(Key::End),
            one_page: input.key_pressed(Key::Num1),
            two_page: input.key_pressed(Key::Num2),
            cycle_zoom: input.key_pressed(Key::F),
            zoom_width: input.key_pressed(Key::W),
            zoom_height: input.key_pressed(Key::H),
            zoom_full: input.key_pressed(Key::O),
            zoom_custom: input.key_pressed(Key::C),
            zoom_in: input.key_pressed(Key::Plus) || input.key_pressed(Key::Equals),
            zoom_out: input.key_pressed(Key::Minus),
            zoom_reset: input.key_pressed(Key::Num0),
            fullscreen: input.key_pressed(Key::F11),
            toggle_direction: input.key_pressed(Key::D),
            // Rotation: [ or Shift+Left for left page CCW, ] or Shift+Right for left page CW
            rotate_left_ccw: input.key_pressed(Key::OpenBracket) || (shift && input.key_pressed(Key::ArrowLeft)),
            rotate_left_cw: input.key_pressed(Key::CloseBracket) || (shift && input.key_pressed(Key::ArrowRight)),
            // Right page rotation: Ctrl+[ / Ctrl+]
            rotate_right_ccw: ctrl && input.key_pressed(Key::OpenBracket),
            rotate_right_cw: ctrl && input.key_pressed(Key::CloseBracket),
        }
    });

    let mut consumed = false;

    if pressed.fullscreen {
        app.toggle_fullscreen();
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(app.fullscreen()));
        consumed = true;
    }

    // Rotation takes priority over navigation when modifiers are held.
    if pressed.rotate_right_ccw {
        app.rotate_right_page_ccw();
        consumed = true;
    } else if pressed.rotate_right_cw {
        app.rotate_right_page_cw();
        consumed = true;
    } else if pressed.rotate_left_ccw {
        app.rotate_left_page_ccw();
        consumed = true;
    } else if pressed.rotate_left_cw {
        app.rotate_left_page_cw();
        consumed = true;
    }

    // Spread and page navigation. One action per frame, in priority order.
    if !consumed && pressed.next_turn {
        app.next_turn();
        consumed = true;
    } else if !consumed && pressed.prev_turn {
        app.prev_turn();
        consumed = true;
    } else if !consumed && pressed.next_page {
        app.next_page();
        consumed = true;
    } else if !consumed && pressed.prev_page {
        app.prev_page();
        consumed = true;
    } else if !consumed && pressed.first {
        app.first();
        consumed = true;
    } else if !consumed && pressed.last {
        app.last();
        consumed = true;
    }

    // View mode: 1 = one page, 2 = two pages.
    if pressed.one_page {
        app.set_view_mode(ViewMode::OnePage);
        consumed = true;
    } else if pressed.two_page {
        app.set_view_mode(ViewMode::TwoPage);
        consumed = true;
    }

    // Zoom mode: F cycles, the letter keys select directly.
    if pressed.cycle_zoom {
        app.cycle_zoom();
        consumed = true;
    } else if pressed.zoom_width {
        app.set_zoom_mode(ZoomMode::Width);
        consumed = true;
    } else if pressed.zoom_height {
        app.set_zoom_mode(ZoomMode::Height);
        consumed = true;
    } else if pressed.zoom_full {
        app.set_zoom_mode(ZoomMode::Full);
        consumed = true;
    } else if pressed.zoom_custom {
        app.set_zoom_mode(ZoomMode::Custom);
        consumed = true;
    }

    if pressed.zoom_in {
        app.adjust_custom_percent(10);
        consumed = true;
    } else if pressed.zoom_out {
        app.adjust_custom_percent(-10);
        consumed = true;
    } else if pressed.zoom_reset {
        app.set_custom_percent(100);
        consumed = true;
    }

    if pressed.toggle_direction {
        app.toggle_direction();
        consumed = true;
    }

    consumed
}