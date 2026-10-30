#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Drive SpreadState with arbitrary page counts and mode toggles.
    // Navigation must not panic on edge-case inputs.
    if data.len() < 2 {
        return;
    }
    let pages = (data[0] as usize).max(1).min(256);
    let orientations: Vec<comical_core::PageOrientation> = (0..pages)
        .map(|i| {
            if i % 3 == 0 {
                comical_core::PageOrientation::Landscape
            } else {
                comical_core::PageOrientation::Portrait
            }
        })
        .collect();
    let mut state = match comical_core::SpreadState::new(orientations, comical_core::ViewMode::OnePage) {
        Ok(s) => s,
        Err(_) => return,
    };
    for &byte in &data[1..] {
        match byte % 6 {
            0 => state.next_turn(),
            1 => state.prev_turn(),
            2 => state.next_page(),
            3 => state.prev_page(),
            4 => state.toggle_mode(),
            5 => state.go_to_page(byte as usize),
            _ => {}
        }
    }
});