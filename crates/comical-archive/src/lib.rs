//! Read-only archive access for the comic viewer.
//!
//! Opening a book produces a page list in book order. Pages are image entries
//! sorted by the byte value of their full names. Reading a page returns its
//! bytes and never writes to the archive or to disk.
//!
//! ZIP, RAR and 7z are read through the `zip`, `rars` and `sevenz-rust`
//! crates. Directories are walked with the standard library.

mod directory;
mod rar;
mod sevenz;
mod zip;

use std::error::Error as StdError;
use std::path::{Path, PathBuf};

use comical_core::PageEntry;
use thiserror::Error;

/// Default upper bound for one decoded entry.
///
/// The bound is checked from archive metadata before allocation and enforced
/// again while reading. Callers may pass a smaller bound to [`ComicArchive::read_page`].
pub const DEFAULT_MAX_ENTRY_BYTES: u64 = 512 * 1024 * 1024;

/// Archive container that backs a book.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    /// ZIP (`.zip`, `.cbz`).
    Zip,
    /// RAR (`.rar`, `.cbr`).
    Rar,
    /// 7z (`.7z`, `.cb7`).
    SevenZ,
    /// A directory walked recursively.
    Directory,
}

/// Options used when opening a book.
#[derive(Debug, Clone)]
pub struct OpenOptions<'a> {
    /// Password for encrypted entries or headers. Never persisted.
    pub password: Option<&'a str>,
    /// Upper bound for a single decoded entry, in bytes.
    pub max_entry_bytes: u64,
}

impl Default for OpenOptions<'_> {
    fn default() -> Self {
        Self {
            password: None,
            max_entry_bytes: DEFAULT_MAX_ENTRY_BYTES,
        }
    }
}

impl<'a> OpenOptions<'a> {
    /// Creates options with the default entry cap and no password.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the password used for encrypted archives.
    pub fn with_password(mut self, password: Option<&'a str>) -> Self {
        self.password = password;
        self
    }
}

/// A read-only book backed by an archive or a directory.
pub trait ComicArchive: Send {
    /// Container format.
    fn format(&self) -> ArchiveFormat;

    /// Page entries in book order.
    fn pages(&self) -> &[PageEntry];

    /// Reads one page by book-order index.
    ///
    /// `limit` caps the decoded size. The implementation must reject an entry
    /// whose declared size exceeds the cap before allocating a full buffer.
    fn read_page(&mut self, index: usize, limit: u64) -> Result<Vec<u8>, ArchiveError>;
}

/// Archive errors with user-facing messages.
#[derive(Debug, Error)]
pub enum ArchiveError {
    /// The extension does not name a supported archive.
    #[error("Cannot open {path}")]
    CannotOpen {
        /// Input path.
        path: PathBuf,
    },

    /// The archive could not be opened or parsed.
    #[error("cannot open {path}: {source}")]
    Open {
        /// Input path.
        path: PathBuf,
        /// Underlying cause.
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },

    /// The archive has no image entries.
    #[error("This archive does not contain any pages.")]
    NoPages,

    /// The requested book order index does not exist.
    #[error("page index {index} is out of range; this book has {count} pages")]
    IndexOutOfRange {
        /// Requested index.
        index: usize,
        /// Number of pages.
        count: usize,
    },

    /// One entry exceeds the configured cap.
    #[error("page {name} is {size} bytes, above the {limit}-byte limit")]
    EntryTooLarge {
        /// Entry name.
        name: String,
        /// Declared or observed size.
        size: u64,
        /// Configured cap.
        limit: u64,
    },

    /// The archive is damaged or is not the format its extension claims.
    #[error("archive is corrupt or unreadable: {0}")]
    Corrupt(String),

    /// A password is needed to list or read the archive.
    #[error("archive is encrypted; pass --password")]
    PasswordRequired,

    /// The supplied password did not work.
    #[error("wrong password or corrupt encrypted data")]
    BadPassword,

    /// The archive uses a feature this build does not support.
    #[error("archive uses an unsupported feature: {0}")]
    Unsupported(String),

    /// Filesystem failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl ArchiveError {
    pub(crate) fn corrupt(message: impl Into<String>) -> Self {
        Self::Corrupt(message.into())
    }

    pub(crate) fn unsupported(message: impl Into<String>) -> Self {
        Self::Unsupported(message.into())
    }
}

/// Opens a book using default limits and no password.
pub fn open(path: impl AsRef<Path>) -> Result<Box<dyn ComicArchive>, ArchiveError> {
    open_with_options(path, OpenOptions::default())
}

/// Opens a book with explicit limits and an optional password.
pub fn open_with_options(
    path: impl AsRef<Path>,
    options: OpenOptions<'_>,
) -> Result<Box<dyn ComicArchive>, ArchiveError> {
    let path = path.as_ref();

    if path.is_dir() {
        return directory::open(path);
    }

    match format_for(path) {
        Some(ArchiveFormat::Zip) => zip::open(path, &options),
        Some(ArchiveFormat::Rar) => rar::open(path, &options),
        Some(ArchiveFormat::SevenZ) => sevenz::open(path, &options),
        Some(ArchiveFormat::Directory) => directory::open(path),
        None => Err(ArchiveError::CannotOpen {
            path: path.to_path_buf(),
        }),
    }
}

/// Returns the container format implied by a path's extension.
pub fn format_for(path: &Path) -> Option<ArchiveFormat> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "zip" | "cbz" => Some(ArchiveFormat::Zip),
        "rar" | "cbr" => Some(ArchiveFormat::Rar),
        "7z" | "cb7" => Some(ArchiveFormat::SevenZ),
        _ => None,
    }
}

pub(crate) fn is_image_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".jpg", ".jpeg", ".gif", ".png"]
        .iter()
        .any(|suffix| lower.ends_with(suffix))
}

/// A page candidate before book-order sorting.
pub(crate) struct Candidate<S> {
    pub(crate) name: String,
    pub(crate) size: u64,
    pub(crate) source: S,
}

/// Sorts candidates by the byte value of their full names and splits them into
/// the public page list and the reader-specific source handles.
///
/// When every candidate shares a single root directory prefix (e.g. all entries
/// are inside `Comic/`), that prefix is stripped so the book opens as if the
/// wrapper folder did not exist. This implements "one-level recursive" opening
/// for archives that bundle their pages inside a single top-level directory.
pub(crate) fn select_pages<S>(candidates: Vec<Candidate<S>>) -> (Vec<PageEntry>, Vec<S>) {
    let mut candidates = candidates;
    strip_common_root(&mut candidates);
    // Byte order, not natural or locale order. The sort is stable, so entries
    // with identical names keep their archive order.
    candidates.sort_by(|left, right| left.name.as_bytes().cmp(right.name.as_bytes()));

    let mut pages = Vec::with_capacity(candidates.len());
    let mut sources = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        pages.push(PageEntry::new(candidate.name, candidate.size));
        sources.push(candidate.source);
    }
    (pages, sources)
}

/// If every candidate name starts with the same `dir/` prefix, remove it.
///
/// Only strips when *all* entries share the prefix, so mixed layouts (some
/// files at root, some in subdirs) are left untouched. A bare filename without
/// `/` never counts as a directory prefix.
fn strip_common_root<S>(candidates: &mut [Candidate<S>]) {
    if candidates.is_empty() {
        return;
    }
    let first_slash = match candidates[0].name.find('/') {
        Some(pos) => pos,
        None => return,
    };
    let prefix = candidates[0].name[..first_slash + 1].to_owned();
    if !candidates.iter().all(|c| c.name.starts_with(&prefix)) {
        return;
    }
    let prefix_len = prefix.len();
    for candidate in candidates.iter_mut() {
        candidate.name = candidate.name[prefix_len..].to_owned();
    }
}

pub(crate) fn page_count_error(pages: &[PageEntry]) -> Option<ArchiveError> {
    if pages.is_empty() {
        Some(ArchiveError::NoPages)
    } else {
        None
    }
}

pub(crate) fn check_index(index: usize, pages: &[PageEntry]) -> Result<(), ArchiveError> {
    if index >= pages.len() {
        return Err(ArchiveError::IndexOutOfRange {
            index,
            count: pages.len(),
        });
    }
    Ok(())
}

pub(crate) fn check_declared_size(name: &str, size: u64, limit: u64) -> Result<(), ArchiveError> {
    if size > limit {
        return Err(ArchiveError::EntryTooLarge {
            name: name.to_owned(),
            size,
            limit,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_selection_sorts_by_bytes_and_keeps_duplicates_stable() {
        let candidates = vec![
            Candidate {
                name: "page2.png".to_owned(),
                size: 2,
                source: 1,
            },
            Candidate {
                name: "page10.png".to_owned(),
                size: 10,
                source: 2,
            },
            Candidate {
                name: "page2.png".to_owned(),
                size: 20,
                source: 3,
            },
            Candidate {
                name: "Page3.PNG".to_owned(),
                size: 3,
                source: 4,
            },
        ];
        let (pages, sources) = select_pages(candidates);
        let names: Vec<&str> = pages.iter().map(|page| page.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["Page3.PNG", "page10.png", "page2.png", "page2.png"]
        );
        assert_eq!(sources, vec![4, 2, 1, 3]);
        assert_eq!(pages[2].size, 2);
        assert_eq!(pages[3].size, 20);
    }

    #[test]
    fn suffix_filter_is_case_insensitive_and_exact() {
        for name in ["a.jpg", "a.JPEG", "a.Gif", "a.PNG"] {
            assert!(is_image_name(name), "{name}");
        }
        for name in ["a.png.txt", "a.webp", "png", "a.jpe"] {
            assert!(!is_image_name(name), "{name}");
        }
    }
}
