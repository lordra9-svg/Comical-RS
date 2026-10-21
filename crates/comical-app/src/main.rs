//! Minimal entry point: a first argument names a book to open on startup.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1);
    comical_app::run(path.as_deref())
}
