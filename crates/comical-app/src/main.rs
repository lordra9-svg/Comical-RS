//! Minimal entry point: a first argument names a book to open on startup.

// Release builds are GUI apps. Without this the Windows binary links as a
// console program, so launching the viewer from Explorer also opens a console
// window behind it. Debug builds keep the console for panic and log output.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1);
    comical_app::run(path.as_deref())
}
