//! Shared, dependency-free domain types.
//!
//! This crate contains book-order page descriptions, the spread state machine
//! and zoom geometry. It performs no file I/O and has no UI, image or archive
//! dependencies.

mod spread;
mod zoom;

pub use spread::{PageOrientation, PagePart, PageRef, Spread, SpreadError, SpreadState, ViewMode};
pub use zoom::{
    MAX_CUSTOM_PERCENT, MIN_CUSTOM_PERCENT, Size, ZoomMode, ZoomOptions, ZoomResult, compute_zoom,
    compute_zoom_with_neighbors, is_oversized,
};

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
