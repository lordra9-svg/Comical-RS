//! Generate synthetic archive fixtures.

use std::error::Error;
use std::path::PathBuf;

use xtask::{
    FixtureEntry, ZipEncryption, ZipMethod, write_corrupt, write_rar, write_sample_set, write_zip,
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

    let corrupt = directory.join("corrupt.cbz");
    write_corrupt(&corrupt, b"this is not a zip archive")?;
    println!("{}", corrupt.display());

    Ok(())
}
