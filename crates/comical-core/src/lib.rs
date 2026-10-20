//! Shared, dependency-free domain types.
//!
//! M1 only needs one type: the description of a page inside a book. Image
//! decoding, spread construction and settings land in later milestones.

/// One image entry in a book, in book order.
///
/// `name` is the full entry path inside an archive, or the path relative to
/// the opened directory. Names keep duplicates; identity is the page index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageEntry {
    /// Full entry name, using `/` as the separator.
    pub name: String,
    /// Uncompressed size in bytes.
    pub size: u64,
}

impl PageEntry {
    /// Creates a page description.
    pub fn new(name: impl Into<String>, size: u64) -> Self {
        Self {
            name: name.into(),
            size,
        }
    }
}
