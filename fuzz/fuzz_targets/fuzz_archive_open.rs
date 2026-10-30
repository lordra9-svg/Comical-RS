#![no_main]

use libfuzzer_sys::fuzz_target;
use std::io::Write;

fuzz_target!(|data: &[u8]| {
    // Write fuzz input to a temp file since the archive API is path-based.
    // The archive layer must not panic on malformed input.
    let dir = std::env::temp_dir();
    let path = dir.join("comical_fuzz_archive");
    if let Ok(mut f) = std::fs::File::create(&path) {
        let _ = f.write_all(data);
        drop(f);
        let _ = comical_archive::open(&path);
        let _ = std::fs::remove_file(&path);
    }
});