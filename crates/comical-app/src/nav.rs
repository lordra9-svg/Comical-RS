//! Keyboard and pointer input mapping for the viewer.

use comical_core::{ViewMode, ZoomMode};
use eframe::egui::{Context, Key, ViewportCommand};

use crate::App;

/// Scroll step for arrow keys in canvas points.
const ARROW_SCROLL_STEP: f32 = 10.0;

/// Snapshot of the keys handled this frame.
struct Pressed {
    scroll_up: bool,
    scroll_down: bool,
    scroll_left: bool,
    scroll_right: bool,
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
    toggle_toolbar: bool,
    toggle_browser: bool,
}

/// Mouse wheel actions captured this frame.
struct Wheel {
    /// Vertical scroll delta (positive = scroll down).
    scroll_y: f32,
    /// Ctrl+wheel zoom delta (positive = zoom in).
    zoom_delta: f32,
}

/// Handle global keyboard navigation, zoom, scrolling and fullscreen.
///
/// Returns `true` if the input was consumed (the app handled it).
pub fn handle_input(ctx: &Context, app: &mut App) -> bool {
    let (pressed, wheel) = ctx.input(|input| {
        let shift = input.modifiers.shift;
        let ctrl = input.modifiers.ctrl;
        let pressed = Pressed {
            scroll_up: input.key_pressed(Key::ArrowUp),
            scroll_down: input.key_pressed(Key::ArrowDown),
            scroll_left: input.key_pressed(Key::ArrowLeft),
            scroll_right: input.key_pressed(Key::ArrowRight),
            next_turn: input.key_pressed(Key::PageDown) || input.key_pressed(Key::J),
            prev_turn: input.key_pressed(Key::PageUp) || input.key_pressed(Key::K),
            next_page: input.key_pressed(Key::Space),
            prev_page: input.key_pressed(Key::Backspace),
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
            rotate_left_ccw: input.key_pressed(Key::OpenBracket)
                || (shift && input.key_pressed(Key::ArrowLeft)),
            rotate_left_cw: input.key_pressed(Key::CloseBracket)
                || (shift && input.key_pressed(Key::ArrowRight)),
            // Right page rotation: Ctrl+[ / Ctrl+]
            rotate_right_ccw: ctrl && input.key_pressed(Key::OpenBracket),
            rotate_right_cw: ctrl && input.key_pressed(Key::CloseBracket),
            toggle_toolbar: input.key_pressed(Key::T),
            toggle_browser: input.key_pressed(Key::B),
        };
        let mut scroll_y = 0.0_f32;
        let mut zoom_delta = 0.0_f32;
        for event in &input.events {
            if let eframe::egui::Event::MouseWheel {
                delta, modifiers, ..
            } = event
            {
                if modifiers.ctrl {
                    zoom_delta += delta.y;
                } else {
                    scroll_y += delta.y;
                }
            }
        }
        (
            pressed,
            Wheel {
                scroll_y,
                zoom_delta,
            },
        )
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

    // Arrow keys scroll the viewport.
    if !consumed && pressed.scroll_up {
        app.scroll((0.0, ARROW_SCROLL_STEP));
        consumed = true;
    } else if !consumed && pressed.scroll_down {
        app.scroll((0.0, -ARROW_SCROLL_STEP));
        consumed = true;
    } else if !consumed && pressed.scroll_left {
        app.scroll((ARROW_SCROLL_STEP, 0.0));
        consumed = true;
    } else if !consumed && pressed.scroll_right {
        app.scroll((-ARROW_SCROLL_STEP, 0.0));
        consumed = true;
    }

    // Mouse wheel: vertical scrolls, Ctrl+wheel zooms.
    if wheel.zoom_delta != 0.0 {
        let steps = (wheel.zoom_delta / 10.0).round() as i32;
        if steps != 0 {
            app.adjust_custom_percent(steps * 10);
            consumed = true;
        }
    } else if wheel.scroll_y != 0.0 {
        app.scroll((0.0, wheel.scroll_y));
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

    if pressed.toggle_toolbar {
        app.toggle_toolbar();
        consumed = true;
    }

    if pressed.toggle_browser {
        app.toggle_browser();
        consumed = true;
    }

    consumed
}
