//! Generate synthetic archive fixtures.

use std::error::Error;
use std::path::PathBuf;

use xtask::{
    FixtureEntry, RarFixtureOptions, RarVersion, SAMPLE_ENTRIES, ZipEncryption, ZipMethod,
    write_7z_encrypted, write_corrupt, write_cp437_zip, write_rar, write_rar_volume_set,
    write_rar_with_options, write_sample_set, write_zip,
};

fn main() -> Result<(), Box<dyn Error>> {
    let directory = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/fixtures"));

    let samples = write_sample_set(&directory)?;
    println!("{}", samples.zip.display());
    println!("{}", samples.rar.display());
    println!("{}", samples.sevenz.display());

    let store = directory.join("store.cbz");
    write_zip(
        &store,
        &[
            FixtureEntry::new("page10.png", b"page ten"),
            FixtureEntry::new("page2.png", b"page two"),
            FixtureEntry::new("UPPER.JPG", b"upper"),
            FixtureEntry::new("ignore.txt", b"ignore"),
        ],
        ZipMethod::Store,
        ZipEncryption::None,
        None,
    )?;
    println!("{}", store.display());

    let encrypted = directory.join("encrypted.cbz");
    write_zip(
        &encrypted,
        &[
            FixtureEntry::new("secret.png", b"secret page one"),
            FixtureEntry::new("secret2.jpg", b"secret page two"),
        ],
        ZipMethod::Deflate,
        ZipEncryption::Aes256,
        Some("open-sesame"),
    )?;
    println!("{}", encrypted.display());

    let encrypted_rar = directory.join("encrypted.cbr");
    write_rar(
        &encrypted_rar,
        &[
            FixtureEntry::new("secret.png", b"secret page one"),
            FixtureEntry::new("secret2.jpg", b"secret page two"),
        ],
        Some("open-sesame"),
    )?;
    println!("{}", encrypted_rar.display());

    for (version, file_name) in [
        (RarVersion::Rar50, "sample-rar5.cbr"),
        (RarVersion::Rar70, "sample-rar7.cbr"),
    ] {
        let path = directory.join(file_name);
        write_rar_with_options(
            &path,
            SAMPLE_ENTRIES,
            RarFixtureOptions {
                version,
                ..RarFixtureOptions::default()
            },
        )?;
        println!("{}", path.display());
    }

    let solid = directory.join("solid-rar5.cbr");
    write_rar_with_options(
        &solid,
        SAMPLE_ENTRIES,
        RarFixtureOptions {
            version: RarVersion::Rar50,
            solid: true,
            ..RarFixtureOptions::default()
        },
    )?;
    println!("{}", solid.display());

    let encrypted_rar5 = directory.join("encrypted-rar5.cbr");
    write_rar_with_options(
        &encrypted_rar5,
        SAMPLE_ENTRIES,
        RarFixtureOptions {
            version: RarVersion::Rar50,
            password: Some("open-sesame"),
            ..RarFixtureOptions::default()
        },
    )?;
    println!("{}", encrypted_rar5.display());

    let encrypted_headers = directory.join("encrypted-headers-rar5.cbr");
    write_rar_with_options(
        &encrypted_headers,
        SAMPLE_ENTRIES,
        RarFixtureOptions {
            version: RarVersion::Rar50,
            password: Some("open-sesame"),
            encrypted_headers: true,
            ..RarFixtureOptions::default()
        },
    )?;
    println!("{}", encrypted_headers.display());

    let encrypted_7z = directory.join("encrypted.cb7");
    write_7z_encrypted(&encrypted_7z, SAMPLE_ENTRIES, "open-sesame")?;
    println!("{}", encrypted_7z.display());

    let cp437 = directory.join("legacy-name.cbz");
    write_cp437_zip(&cp437, b"caf\x82.png", b"\x89PNG\r\n\x1a\nlegacy name")?;
    println!("{}", cp437.display());

    let mut split_a = vec![0u8; 2048];
    let mut split_b = vec![0u8; 2048];
    let mut state = 0x1234_5678u32;
    for byte in split_a.iter_mut().chain(split_b.iter_mut()) {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        *byte = (state >> 24) as u8;
    }
    let parts = write_rar_volume_set(
        &directory.join("multivolume"),
        "split-rar5",
        &[
            FixtureEntry::new("one.png", &split_a),
            FixtureEntry::new("two.jpg", &split_b),
        ],
        RarFixtureOptions {
            version: RarVersion::Rar50,
            ..RarFixtureOptions::default()
        },
        1024,
    )?;
    println!("{}", parts[0].display());

    let corrupt = directory.join("corrupt.cbz");
    write_corrupt(&corrupt, b"this is not a zip archive")?;
    println!("{}", corrupt.display());

    Ok(())
}
