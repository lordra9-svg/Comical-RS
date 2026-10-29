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

use std::cmp::Ordering;
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
    /// Sort digit runs numerically (`page2` before `page10`).
    ///
    /// Off by default: the behavior spec defines book order as byte order, and
    /// the setting is opt-in.
    pub natural_sort: bool,
}

impl Default for OpenOptions<'_> {
    fn default() -> Self {
        Self {
            password: None,
            max_entry_bytes: DEFAULT_MAX_ENTRY_BYTES,
            natural_sort: false,
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

    /// Enables natural (numeric-aware) page ordering.
    pub fn with_natural_sort(mut self, natural_sort: bool) -> Self {
        self.natural_sort = natural_sort;
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

    /// Whether the archive needs a password to list or read.
    pub fn is_password_required(&self) -> bool {
        matches!(self, Self::PasswordRequired)
    }

    /// Whether the supplied password was rejected.
    pub fn is_bad_password(&self) -> bool {
        matches!(self, Self::BadPassword)
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
        return directory::open(path, &options);
    }

    match format_for(path) {
        Some(ArchiveFormat::Zip) => zip::open(path, &options),
        Some(ArchiveFormat::Rar) => rar::open(path, &options),
        Some(ArchiveFormat::SevenZ) => sevenz::open(path, &options),
        Some(ArchiveFormat::Directory) => directory::open(path, &options),
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
pub(crate) fn select_pages<S>(
    candidates: Vec<Candidate<S>>,
    natural_sort: bool,
) -> (Vec<PageEntry>, Vec<S>) {
    let mut candidates = candidates;
    strip_common_root(&mut candidates);
    if natural_sort {
        candidates.sort_by(|left, right| natural_cmp(&left.name, &right.name));
    } else {
        // Byte order, not natural or locale order. The sort is stable, so
        // entries with identical names keep their archive order.
        candidates.sort_by(|left, right| left.name.as_bytes().cmp(right.name.as_bytes()));
    }

    let mut pages = Vec::with_capacity(candidates.len());
    let mut sources = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        pages.push(PageEntry::new(candidate.name, candidate.size));
        sources.push(candidate.source);
    }
    (pages, sources)
}

/// Compares two entry names with runs of ASCII digits treated as numbers.
///
/// Everything outside a digit run compares by byte value, so natural order is
/// a strict refinement of the default book order: only names that differ in a
/// number can move. Numeric runs compare by value regardless of width, so
/// `page9` sorts before `page10`. Runs with equal value fall back to the run
/// length (`2` before `02`), which keeps the comparator total.
fn natural_cmp(left: &str, right: &str) -> Ordering {
    let left = left.as_bytes();
    let right = right.as_bytes();
    let (mut left_at, mut right_at) = (0usize, 0usize);

    while left_at < left.len() && right_at < right.len() {
        let (left_byte, right_byte) = (left[left_at], right[right_at]);
        if left_byte.is_ascii_digit() && right_byte.is_ascii_digit() {
            let left_end = digit_run_end(left, left_at);
            let right_end = digit_run_end(right, right_at);
            let left_digits = trim_leading_zeros(&left[left_at..left_end]);
            let right_digits = trim_leading_zeros(&right[right_at..right_end]);
            let order = left_digits
                .len()
                .cmp(&right_digits.len())
                .then_with(|| left_digits.cmp(right_digits))
                .then_with(|| (left_end - left_at).cmp(&(right_end - right_at)));
            if order != Ordering::Equal {
                return order;
            }
            left_at = left_end;
            right_at = right_end;
            continue;
        }
        if left_byte != right_byte {
            return left_byte.cmp(&right_byte);
        }
        left_at += 1;
        right_at += 1;
    }

    left.len().cmp(&right.len())
}

fn digit_run_end(bytes: &[u8], start: usize) -> usize {
    let mut end = start;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    end
}

fn trim_leading_zeros(digits: &[u8]) -> &[u8] {
    let first = digits
        .iter()
        .position(|byte| *byte != b'0')
        .unwrap_or(digits.len());
    &digits[first..]
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
        let (pages, sources) = select_pages(candidates, false);
        let names: Vec<&str> = pages.iter().map(|page| page.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["Page3.PNG", "page10.png", "page2.png", "page2.png"]
        );
        assert_eq!(sources, vec![4, 2, 1, 3]);
        assert_eq!(pages[2].size, 2);
        assert_eq!(pages[3].size, 20);
    }

    fn candidate(name: &str, source: usize) -> Candidate<usize> {
        Candidate {
            name: name.to_owned(),
            size: source as u64,
            source,
        }
    }

    #[test]
    fn natural_sort_orders_digit_runs_by_value() {
        let candidates = vec![
            candidate("page10.png", 1),
            candidate("page2.png", 2),
            candidate("page1.png", 3),
            candidate("page02.png", 4),
            candidate("page100.png", 5),
        ];
        let (pages, sources) = select_pages(candidates, true);
        let names: Vec<&str> = pages.iter().map(|page| page.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "page1.png",
                "page2.png",
                "page02.png",
                "page10.png",
                "page100.png"
            ]
        );
        assert_eq!(sources, vec![3, 2, 4, 1, 5]);
    }

    #[test]
    fn natural_sort_only_changes_digit_runs() {
        // Names without digits keep byte order, including case.
        let candidates = vec![
            candidate("b.png", 1),
            candidate("A.png", 2),
            candidate("a.png", 3),
        ];
        let (pages, _) = select_pages(candidates, true);
        let names: Vec<&str> = pages.iter().map(|page| page.name.as_str()).collect();
        assert_eq!(names, vec!["A.png", "a.png", "b.png"]);
    }

    #[test]
    fn natural_sort_handles_long_digit_runs_without_overflow() {
        let candidates = vec![
            candidate("99999999999999999999999.png", 1),
            candidate("100000000000000000000000.png", 2),
        ];
        let (pages, _) = select_pages(candidates, true);
        let names: Vec<&str> = pages.iter().map(|page| page.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "99999999999999999999999.png",
                "100000000000000000000000.png"
            ]
        );
    }

    #[test]
    fn natural_sort_is_a_total_order() {
        let names = ["a2b10", "a2b9", "a02b9", "a10", "a2", "a", "a0", "a00"];
        for left in names {
            for right in names {
                let forward = natural_cmp(left, right);
                let backward = natural_cmp(right, left);
                assert_eq!(forward, backward.reverse(), "{left} vs {right}");
                if left == right {
                    assert_eq!(forward, Ordering::Equal);
                }
            }
        }
    }

    #[test]
    fn password_errors_report_whether_a_retry_is_needed() {
        assert!(ArchiveError::PasswordRequired.is_password_required());
        assert!(!ArchiveError::PasswordRequired.is_bad_password());
        assert!(!ArchiveError::BadPassword.is_password_required());
        assert!(ArchiveError::BadPassword.is_bad_password());
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
