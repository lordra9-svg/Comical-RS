//! RAR books.

use std::io;
use std::path::Path;

use comical_core::PageEntry;
use rars::{ArchiveReadOptions, ArchiveReader, ErrorKind};

use crate::{
    ArchiveError, ArchiveFormat, Candidate, ComicArchive, OpenOptions, check_declared_size,
    check_index, is_image_name, page_count_error, select_pages,
};

pub(crate) fn open(
    path: &Path,
    options: &OpenOptions<'_>,
) -> Result<Box<dyn ComicArchive>, ArchiveError> {
    let password = options.password.map(str::as_bytes);
    let read_options = ArchiveReadOptions::with_optional_password(password);
    let archive = ArchiveReader::read_path_with_options(path, read_options)
        .map_err(|error| map_open_error(path, error))?;
    if archive_is_volume(&archive) {
        return Err(ArchiveError::Unsupported("multi-volume RAR".to_owned()));
    }

    let mut candidates = Vec::new();
    for (index, member) in archive.member_refs().enumerate() {
        let owned = member.to_owned();
        let meta = &owned.meta;
        // Multi-volume archives are out of scope. Skipping a split part would
        // silently produce an incomplete page list, so reject the book.
        if meta.is_split_before || meta.is_split_after {
            return Err(ArchiveError::Unsupported("multi-volume RAR".to_owned()));
        }
        if member.is_directory() || member.is_redirection() {
            continue;
        }

        let decoded = owned.decoded_name(None).map_err(map_read_error)?;
        let name = normalize_name(&decoded);
        if !is_image_name(&name) {
            continue;
        }
        candidates.push(Candidate {
            name,
            size: meta.unpacked_size,
            source: index,
        });
    }

    let (pages, sources) = select_pages(candidates, options.natural_sort);
    if let Some(error) = page_count_error(&pages) {
        return Err(error);
    }

    Ok(Box::new(RarBook {
        archive,
        pages,
        sources,
        password: options.password.map(str::to_owned),
    }))
}

fn archive_is_volume(archive: &rars::Archive) -> bool {
    match archive {
        rars::Archive::Rar13(archive) => archive.main.is_volume(),
        rars::Archive::Rar15To40(archive) => archive.main.is_volume(),
        rars::Archive::Rar50Plus(archive) => archive.main.is_volume(),
        _ => false,
    }
}

fn normalize_name(bytes: &[u8]) -> String {
    let name = String::from_utf8_lossy(bytes);
    // Legacy RAR stores path separators as backslashes. Book names use `/`.
    name.replace('\\', "/")
}

struct RarBook {
    archive: rars::Archive,
    pages: Vec<PageEntry>,
    sources: Vec<usize>,
    password: Option<String>,
}

impl ComicArchive for RarBook {
    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::Rar
    }

    fn pages(&self) -> &[PageEntry] {
        &self.pages
    }

    fn read_page(&mut self, index: usize, limit: u64) -> Result<Vec<u8>, ArchiveError> {
        check_index(index, &self.pages)?;
        let page = &self.pages[index];
        check_declared_size(&page.name, page.size, limit)?;

        let password = self.password.as_deref().map(str::as_bytes);
        let options = ArchiveReadOptions::with_optional_password(password)
            .with_max_member_output_bytes(limit);
        let bytes = self
            .archive
            .read_member_at_with_options(self.sources[index], options)
            .map_err(map_read_error)?
            .ok_or_else(|| {
                ArchiveError::corrupt(format!(
                    "RAR member for page {} has no readable payload",
                    page.name
                ))
            })?;
        Ok(bytes)
    }
}

fn map_open_error(path: &Path, error: rars::Error) -> ArchiveError {
    match error.kind() {
        ErrorKind::PasswordRequired => ArchiveError::PasswordRequired,
        ErrorKind::BadPassword => ArchiveError::BadPassword,
        ErrorKind::UnsupportedFeature => ArchiveError::unsupported(error.to_string()),
        ErrorKind::Io => ArchiveError::Open {
            path: path.to_path_buf(),
            source: Box::new(error),
        },
        _ => ArchiveError::corrupt(error.to_string()),
    }
}

fn map_read_error(error: rars::Error) -> ArchiveError {
    match error.kind() {
        ErrorKind::PasswordRequired => ArchiveError::PasswordRequired,
        ErrorKind::BadPassword => ArchiveError::BadPassword,
        ErrorKind::UnsupportedFeature => ArchiveError::unsupported(error.to_string()),
        ErrorKind::Io => ArchiveError::Io(io::Error::other(error.to_string())),
        _ => ArchiveError::corrupt(error.to_string()),
    }
}
