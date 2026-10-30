#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Attempt to decode arbitrary bytes as an image.
    // The image pipeline must not panic on malformed input.
    let _ = comical_image::decode(data, false);
    let _ = comical_image::probe_size(data, false);
    let _ = comical_image::exif_rotation(data);
});