//! Synthetic archive fixtures for tests and manual acceptance.
//!
//! Everything here is generated at run time from tiny byte fixtures. No comic
//! images or third-party archive files are stored in the repository.

use std::error::Error;
use std::fs::File;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use sevenz_rust::{SevenZArchiveEntry, SevenZWriter};
use zip::write::SimpleFileOptions;
use zip::{AesMode, CompressionMethod, ZipWriter};

/// One file to place in a generated archive.
#[derive(Debug, Clone, Copy)]
pub struct FixtureEntry<'a> {
    /// Full entry name.
    pub name: &'a str,
    /// Exact payload bytes.
    pub data: &'a [u8],
}

impl<'a> FixtureEntry<'a> {
    /// Creates an entry.
    pub const fn new(name: &'a str, data: &'a [u8]) -> Self {
        Self { name, data }
    }
}

/// ZIP compression for generated fixtures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipMethod {
    /// No compression.
    Store,
    /// Deflate.
    Deflate,
}

/// ZIP encryption for generated fixtures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipEncryption {
    /// Plain archive.
    None,
    /// AES-256.
    Aes256,
}

/// Writes a ZIP archive.
pub fn write_zip(
    path: &Path,
    entries: &[FixtureEntry<'_>],
    method: ZipMethod,
    encryption: ZipEncryption,
    password: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let file = File::create(path)?;
    let mut writer = ZipWriter::new(file);
    let compression = match method {
        ZipMethod::Store => CompressionMethod::Stored,
        ZipMethod::Deflate => CompressionMethod::Deflated,
    };

    for entry in entries {
        let base = SimpleFileOptions::default().compression_method(compression);
        let options = match (encryption, password) {
            (ZipEncryption::Aes256, Some(password)) => {
                base.with_aes_encryption(AesMode::Aes256, password)
            }
            _ => base,
        };
        writer.start_file(entry.name, options)?;
        writer.write_all(entry.data)?;
    }
    writer.finish()?;
    Ok(())
}

/// Writes a RAR 4.0 archive with the writer's default compression.
pub fn write_rar(
    path: &Path,
    entries: &[FixtureEntry<'_>],
    password: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let mut builder = rars::Builder::new(rars::ArchiveVersion::Rar40)
        .password(password.map(|value| value.as_bytes().to_vec()));
    for entry in entries {
        builder.add_bytes(
            entry.name.as_bytes().to_vec(),
            entry.data.to_vec(),
            None,
            None,
        )?;
    }
    builder.write_to_path(path, None)?;
    Ok(())
}

/// Writes a 7z archive with LZMA2 content.
///
/// Encrypted 7z writing is not exposed: `sevenz-rust` 0.6.1's `aes256` feature
/// does not compile without its `compress` encoder feature, so readers keep
/// compressor features disabled and 7z encryption is deferred.
pub fn write_7z(path: &Path, entries: &[FixtureEntry<'_>]) -> Result<(), Box<dyn Error>> {
    let mut writer = SevenZWriter::create(path)?;
    for entry in entries {
        let mut archive_entry = SevenZArchiveEntry::new();
        archive_entry.name = entry.name.to_owned();
        writer.push_archive_entry(archive_entry, Some(Cursor::new(entry.data)))?;
    }
    writer.finish()?;
    Ok(())
}

/// Writes arbitrary bytes, for corrupt-archive tests.
pub fn write_corrupt(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Paths produced by [`write_sample_set`].
#[derive(Debug, Clone)]
pub struct SampleSet {
    /// Deflate ZIP sample.
    pub zip: PathBuf,
    /// Stored RAR sample.
    pub rar: PathBuf,
    /// LZMA2 7z sample.
    pub sevenz: PathBuf,
}

/// The entries used by the sample set.
pub const SAMPLE_ENTRIES: &[FixtureEntry<'static>] = &[
    FixtureEntry::new("001.png", b"\x89PNG\r\n\x1a\nsample page one"),
    FixtureEntry::new("002.jpg", b"\xff\xd8\xff\xe0sample page two"),
    FixtureEntry::new("sub/003.JPEG", b"\xff\xd8\xff\xe0sample page three"),
    FixtureEntry::new("notes.txt", b"not a page"),
];

/// Writes one sample archive per format into `directory`.
pub fn write_sample_set(directory: &Path) -> Result<SampleSet, Box<dyn Error>> {
    std::fs::create_dir_all(directory)?;
    let zip = directory.join("sample.cbz");
    let rar = directory.join("sample.cbr");
    let sevenz = directory.join("sample.cb7");

    write_zip(
        &zip,
        SAMPLE_ENTRIES,
        ZipMethod::Deflate,
        ZipEncryption::None,
        None,
    )?;
    write_rar(&rar, SAMPLE_ENTRIES, None)?;
    write_7z(&sevenz, SAMPLE_ENTRIES)?;

    Ok(SampleSet { zip, rar, sevenz })
}
