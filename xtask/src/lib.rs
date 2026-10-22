//! Synthetic archive fixtures for tests and manual acceptance.
//!
//! Everything here is generated at run time from tiny byte fixtures. No comic
//! images or third-party archive files are stored in the repository.

use std::error::Error;
use std::fs::File;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use sevenz_rust::{AesEncoderOptions, Password, SevenZArchiveEntry, SevenZMethod, SevenZWriter};
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

/// Writes a stored single-entry ZIP whose name is raw CP437 with no UTF-8 flag.
pub fn write_cp437_zip(path: &Path, name: &[u8], data: &[u8]) -> Result<(), Box<dyn Error>> {
    let name_len = u16::try_from(name.len())?;
    let data_len = u32::try_from(data.len())?;
    let crc = crc32(data);

    let mut archive = Vec::new();
    push_u32(&mut archive, 0x0403_4b50);
    push_u16(&mut archive, 20);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u32(&mut archive, crc);
    push_u32(&mut archive, data_len);
    push_u32(&mut archive, data_len);
    push_u16(&mut archive, name_len);
    push_u16(&mut archive, 0);
    archive.extend_from_slice(name);
    archive.extend_from_slice(data);

    let central_offset = u32::try_from(archive.len())?;
    push_u32(&mut archive, 0x0201_4b50);
    push_u16(&mut archive, 20);
    push_u16(&mut archive, 20);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u32(&mut archive, crc);
    push_u32(&mut archive, data_len);
    push_u32(&mut archive, data_len);
    push_u16(&mut archive, name_len);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u32(&mut archive, 0);
    push_u32(&mut archive, 0);
    archive.extend_from_slice(name);

    let central_size = u32::try_from(archive.len())? - central_offset;
    push_u32(&mut archive, 0x0605_4b50);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 1);
    push_u16(&mut archive, 1);
    push_u32(&mut archive, central_size);
    push_u32(&mut archive, central_offset);
    push_u16(&mut archive, 0);

    std::fs::write(path, archive)?;
    Ok(())
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// RAR format version for generated fixtures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RarVersion {
    /// RAR 1.4.
    Rar14,
    /// RAR 1.5.
    Rar15,
    /// RAR 2.0.
    Rar20,
    /// RAR 2.9.
    Rar29,
    /// RAR 3.0.
    Rar30,
    /// RAR 4.0.
    Rar40,
    /// RAR 5.0.
    Rar50,
    /// RAR 7.0.
    Rar70,
}

impl RarVersion {
    const fn archive_version(self) -> rars::ArchiveVersion {
        match self {
            Self::Rar14 => rars::ArchiveVersion::Rar14,
            Self::Rar15 => rars::ArchiveVersion::Rar15,
            Self::Rar20 => rars::ArchiveVersion::Rar20,
            Self::Rar29 => rars::ArchiveVersion::Rar29,
            Self::Rar30 => rars::ArchiveVersion::Rar30,
            Self::Rar40 => rars::ArchiveVersion::Rar40,
            Self::Rar50 => rars::ArchiveVersion::Rar50,
            Self::Rar70 => rars::ArchiveVersion::Rar70,
        }
    }
}

/// Options for generated RAR fixtures.
#[derive(Debug, Clone, Copy)]
pub struct RarFixtureOptions<'a> {
    /// RAR format version.
    pub version: RarVersion,
    /// Password used for data encryption, or for headers when requested.
    pub password: Option<&'a str>,
    /// Compress members against each other.
    pub solid: bool,
    /// Encrypt archive headers as well as member data.
    pub encrypted_headers: bool,
}

impl Default for RarFixtureOptions<'_> {
    fn default() -> Self {
        Self {
            version: RarVersion::Rar40,
            password: None,
            solid: false,
            encrypted_headers: false,
        }
    }
}

/// Writes a RAR archive with explicit format and encryption options.
pub fn write_rar_with_options(
    path: &Path,
    entries: &[FixtureEntry<'_>],
    options: RarFixtureOptions<'_>,
) -> Result<(), Box<dyn Error>> {
    let builder = builder_for(entries, options)?;
    builder.write_to_path(path, None)?;
    Ok(())
}

/// Writes a RAR 4.0 archive with the writer's default compression.
pub fn write_rar(
    path: &Path,
    entries: &[FixtureEntry<'_>],
    password: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    write_rar_with_options(
        path,
        entries,
        RarFixtureOptions {
            password,
            ..RarFixtureOptions::default()
        },
    )
}

fn builder_for(
    entries: &[FixtureEntry<'_>],
    options: RarFixtureOptions<'_>,
) -> Result<rars::Builder, rars::Error> {
    let mut builder = rars::Builder::new(options.version.archive_version())
        .password(options.password.map(|value| value.as_bytes().to_vec()))
        .solid(options.solid)
        .header_encryption(options.encrypted_headers);
    for entry in entries {
        builder.add_bytes(
            entry.name.as_bytes().to_vec(),
            entry.data.to_vec(),
            None,
            None,
        )?;
    }
    Ok(builder)
}

/// Writes a multi-volume RAR set and returns its parts in order.
///
/// The reader only needs the first part to detect a split member. All parts
/// are written so the fixture is a valid volume set.
pub fn write_rar_volume_set(
    directory: &Path,
    stem: &str,
    entries: &[FixtureEntry<'_>],
    options: RarFixtureOptions<'_>,
    volume_size: usize,
) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    std::fs::create_dir_all(directory)?;
    let builder = builder_for(entries, options)?.volume_size(Some(volume_size));
    let volumes = builder.build_volumes(None)?;
    let mut paths = Vec::with_capacity(volumes.len());
    for (index, bytes) in volumes.iter().enumerate() {
        let path = directory.join(format!("{stem}.part{}.rar", index + 1));
        std::fs::write(&path, bytes)?;
        paths.push(path);
    }
    Ok(paths)
}

/// Writes a 7z archive with LZMA2 content.
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

/// Writes a 7z archive with AES-256 content and header encryption.
pub fn write_7z_encrypted(
    path: &Path,
    entries: &[FixtureEntry<'_>],
    password: &str,
) -> Result<(), Box<dyn Error>> {
    let mut writer = SevenZWriter::create(path)?;
    writer.set_content_methods(vec![
        AesEncoderOptions::new(Password::from(password)).into(),
        SevenZMethod::LZMA2.into(),
    ]);
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
