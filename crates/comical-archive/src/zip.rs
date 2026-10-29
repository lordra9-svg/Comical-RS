//! ZIP books.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use comical_core::PageEntry;
use zip::result::ZipError;
use zip::{ZipArchive, ZipReadOptions};

use crate::{
    ArchiveError, ArchiveFormat, Candidate, ComicArchive, OpenOptions, check_declared_size,
    check_index, is_image_name, page_count_error, select_pages,
};

pub(crate) fn open(
    path: &Path,
    options: &OpenOptions<'_>,
) -> Result<Box<dyn ComicArchive>, ArchiveError> {
    let file = File::open(path).map_err(|source| ArchiveError::Open {
        path: path.to_path_buf(),
        source: Box::new(source),
    })?;
    let archive = ZipArchive::new(file).map_err(|error| map_zip_open_error(path, error))?;

    let mut candidates = Vec::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index_data(index)
            .map_err(|error| map_zip_open_error(path, error))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry
            .name()
            .map_err(|error| ArchiveError::corrupt(error.to_string()))?
            .into_owned();
        if !is_image_name(&name) {
            continue;
        }
        candidates.push(Candidate {
            name,
            size: entry.size(),
            source: index,
        });
    }

    let (pages, sources) = select_pages(candidates, options.natural_sort);
    if let Some(error) = page_count_error(&pages) {
        return Err(error);
    }

    Ok(Box::new(ZipBook {
        archive,
        pages,
        sources,
        password: options.password.map(str::to_owned),
    }))
}

struct ZipBook {
    archive: ZipArchive<File>,
    pages: Vec<PageEntry>,
    sources: Vec<usize>,
    password: Option<String>,
}

impl ComicArchive for ZipBook {
    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::Zip
    }

    fn pages(&self) -> &[PageEntry] {
        &self.pages
    }

    fn read_page(&mut self, index: usize, limit: u64) -> Result<Vec<u8>, ArchiveError> {
        check_index(index, &self.pages)?;
        let page = &self.pages[index];
        check_declared_size(&page.name, page.size, limit)?;

        let archive_index = self.sources[index];
        let options = ZipReadOptions::new()
            .password(self.password.as_deref().map(|password| password.as_bytes()));
        let mut entry = self
            .archive
            .by_index_with_options(archive_index, options)
            .map_err(map_zip_error)?;

        let mut bytes = Vec::new();
        entry
            .by_ref()
            .take(limit.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > limit {
            return Err(ArchiveError::EntryTooLarge {
                name: page.name.clone(),
                size: bytes.len() as u64,
                limit,
            });
        }
        Ok(bytes)
    }
}

fn map_zip_open_error(path: &Path, error: ZipError) -> ArchiveError {
    match error {
        ZipError::Io(source) => ArchiveError::Open {
            path: path.to_path_buf(),
            source: Box::new(source),
        },
        other => map_zip_error(other),
    }
}

fn map_zip_error(error: ZipError) -> ArchiveError {
    match error {
        ZipError::InvalidPassword => ArchiveError::BadPassword,
        ZipError::UnsupportedArchive(message) if message == ZipError::PASSWORD_REQUIRED => {
            ArchiveError::PasswordRequired
        }
        ZipError::UnsupportedArchive(message) => ArchiveError::unsupported(message.to_string()),
        ZipError::CompressionMethodNotSupported(method) => {
            ArchiveError::unsupported(format!("ZIP compression method {method}"))
        }
        ZipError::Io(error) => ArchiveError::Io(error),
        other => ArchiveError::corrupt(other.to_string()),
    }
}
