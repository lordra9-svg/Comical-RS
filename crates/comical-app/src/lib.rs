//! comical-app — eframe-based comic reader application.
//!
//! Opens ZIP, RAR and 7z archives and directories, draws one- and two-page
//! spreads of decoded page images on a black canvas, and provides fit and
//! custom zoom modes, drag-to-pan, F11 fullscreen, and keyboard navigation.
//!
//! # Usage
//!
//! ```no_run
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     comical_app::run(None)?;
//!     Ok(())
//! }
//! ```

mod app;
mod browser;
mod cache;
pub mod diagnostics;
mod error;
mod nav;
mod settings;
mod toolbar;
mod view;

pub use app::App;
pub use error::AppError;
pub use settings::Settings;

use eframe::egui;

/// Run the comical application.
///
/// If `path` is provided, the book at that path is opened on startup.
/// Otherwise the app shows the "Open a book" dialog.
///
/// An error opening the startup path is shown inside the window instead of
/// aborting the process, so the user can pick another book.
pub fn run(path: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new();
    if let Some(path) = path
        && let Err(error) = app.open_book(path)
    {
        diagnostics::record(&format!("startup: could not open {path}: {error}"));
        app.show_error(error.to_string());
    }

    let s = app.settings();
    let options = eframe::NativeOptions {
        // The crate is built without the glow backend, so name the renderer
        // instead of relying on a default that no longer exists.
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([s.window_width as f32, s.window_height as f32])
            .with_position([s.window_x as f32, s.window_y as f32])
            .with_title("comical"),
        ..Default::default()
    };

    eframe::run_native(
        "comical",
        options,
        Box::new(move |cc| {
            // Record which backend actually came up. On Windows this is the
            // difference between DX12, Vulkan and the GLES fallback.
            if let Some(render_state) = &cc.wgpu_render_state {
                let adapter = render_state.adapter.get_info();
                diagnostics::record(&format!(
                    "startup: backend={:?} device={:?} adapter={} driver={}",
                    adapter.backend, adapter.device_type, adapter.name, adapter.driver
                ));
            }
            Ok(Box::new(app) as Box<dyn eframe::App>)
        }),
    )?;
    Ok(())
}
