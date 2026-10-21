use thiserror::Error;

/// Errors surfaced by the viewer, mapped from the underlying libraries.
#[derive(Debug, Error)]
pub enum AppError {
    /// The archive could not be opened.
    #[error(transparent)]
    OpenBook(comical_archive::ArchiveError),
    /// A page image could not be decoded.
    #[error(transparent)]
    Decode(comical_image::ImageError),
    /// Spread construction failed.
    #[error(transparent)]
    Spread(comical_core::SpreadError),
    /// Filesystem failure outside an archive implementation.
    #[error("io error: {0}")]
    Io(std::io::Error),
    /// One page failed while the rest of the book stays usable.
    #[error("page {page} could not be shown: {message}")]
    Page {
        /// Zero-based page index.
        page: usize,
        /// User-facing reason for the failure.
        message: String,
    },
    /// An operation needed an open book.
    #[error("no book is open")]
    NoBook,
}

impl From<comical_archive::ArchiveError> for AppError {
    fn from(error: comical_archive::ArchiveError) -> Self {
        Self::OpenBook(error)
    }
}

impl From<comical_image::ImageError> for AppError {
    fn from(error: comical_image::ImageError) -> Self {
        Self::Decode(error)
    }
}

impl From<comical_core::SpreadError> for AppError {
    fn from(error: comical_core::SpreadError) -> Self {
        Self::Spread(error)
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
