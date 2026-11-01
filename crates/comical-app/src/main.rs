//! Minimal entry point: a first argument names a book to open on startup.

// Release builds are GUI apps. Without this the Windows binary links as a
// console program, so launching the viewer from Explorer also opens a console
// window behind it. Debug builds keep the console for panic and log output.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::process::ExitCode;

fn main() -> ExitCode {
    comical_app::diagnostics::install_panic_hook();

    let path = std::env::args().nth(1);
    match comical_app::run(path.as_deref()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // A GUI build has nowhere to print, so this also writes the log
            // and, on Windows, shows a dialog naming it.
            let message = comical_app::diagnostics::report_fatal(&error.to_string());
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
