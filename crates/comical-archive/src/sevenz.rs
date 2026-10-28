//! 7z books.

use std::io::{self, Read};
use std::path::{Path, PathBuf};

use comical_core::PageEntry;
use sevenz_rust::{Error as SevenZError, Password, SevenZReader};

use crate::{
    ArchiveError, ArchiveFormat, Candidate, ComicArchive, OpenOptions, check_declared_size,
    check_index, is_image_name, page_count_error, select_pages,
};

pub(crate) fn open(
    path: &Path,
    options: &OpenOptions<'_>,
) -> Result<Box<dyn ComicArchive>, ArchiveError> {
    let password = options.password.unwrap_or("");
    let reader = SevenZReader::open(path, Password::from(password))
        .map_err(|error| map_open_error(path, error))?;
    let archive = reader.archive();

    // sevenz-rust walks decoded folders in order and then files without a
    // stream. Record that exact callback order so a page can be located later.
    let order = callback_order(archive);
    let mut ordinal = vec![None; archive.files.len()];
    for (position, file_index) in order.iter().copied().enumerate() {
        if let Some(slot) = ordinal.get_mut(file_index) {
            *slot = Some(position);
        }
    }

    let mut candidates = Vec::new();
    for (file_index, entry) in archive.files.iter().enumerate() {
        if entry.is_directory() || !is_image_name(&entry.name) {
            continue;
        }
        let Some(position) = ordinal.get(file_index).copied().flatten() else {
            continue;
        };
        candidates.push(Candidate {
            name: entry.name.clone(),
            size: entry.size,
            source: position,
        });
    }

    let (pages, sources) = select_pages(candidates);
    if let Some(error) = page_count_error(&pages) {
        return Err(error);
    }

    Ok(Box::new(SevenZBook {
        path: path.to_path_buf(),
        pages,
        sources,
        password: options.password.map(str::to_owned),
    }))
}

fn callback_order(archive: &sevenz_rust::Archive) -> Vec<usize> {
    let mut order = Vec::new();
    for folder_index in 0..archive.folders.len() {
        let start = archive.stream_map.folder_first_file_index[folder_index];
        let count = archive.folders[folder_index].num_unpack_sub_streams;
        for file_index in start..start.saturating_add(count) {
            if let Some(file) = archive.files.get(file_index)
                && file.has_stream
                && file.size > 0
            {
                order.push(file_index);
            }
        }
    }
    for (file_index, _) in archive.files.iter().enumerate() {
        let in_folder = archive
            .stream_map
            .file_folder_index
            .get(file_index)
            .copied()
            .flatten()
            .is_some();
        if !in_folder {
            order.push(file_index);
        }
    }
    order
}

struct SevenZBook {
    path: PathBuf,
    pages: Vec<PageEntry>,
    sources: Vec<usize>,
    password: Option<String>,
}

impl ComicArchive for SevenZBook {
    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::SevenZ
    }

    fn pages(&self) -> &[PageEntry] {
        &self.pages
    }

    fn read_page(&mut self, index: usize, limit: u64) -> Result<Vec<u8>, ArchiveError> {
        check_index(index, &self.pages)?;
        let page = &self.pages[index];
        check_declared_size(&page.name, page.size, limit)?;

        let password = self.password.as_deref().unwrap_or("");
        let mut reader =
            SevenZReader::open(&self.path, Password::from(password)).map_err(map_read_error)?;

        let target = self.sources[index];
        let expected = page.name.clone();
        let mut seen = 0usize;
        let mut found = None;
        let mut overflow = false;
        let mut overflow_size = 0u64;

        reader
            .for_each_entries(|entry, source| {
                // Returning false stops the current folder, not the whole
                // traversal. Skip every later folder once the page is found.
                if found.is_some() || overflow {
                    return Ok(false);
                }
                if seen == target {
                    if entry.name != expected {
                        return Err(SevenZError::other("7z entry order changed while reading"));
                    }
                    let mut bytes = Vec::new();
                    source
                        .take(limit.saturating_add(1))
                        .read_to_end(&mut bytes)
                        .map_err(SevenZError::io)?;
                    if bytes.len() as u64 > limit {
                        overflow = true;
                        overflow_size = bytes.len() as u64;
                    } else {
                        found = Some(bytes);
                    }
                    return Ok(false);
                }
                // Solid folders require prior entries to be fully consumed;
                // otherwise later CRC checks fail (sevenz-rust issue #31).
                io::copy(source, &mut io::sink()).map_err(SevenZError::io)?;
                seen += 1;
                Ok(true)
            })
            .map_err(map_read_error)?;

        if overflow {
            return Err(ArchiveError::EntryTooLarge {
                name: page.name.clone(),
                size: overflow_size,
                limit,
            });
        }
        found.ok_or_else(|| {
            ArchiveError::corrupt(format!(
                "7z entry for page {} could not be located",
                page.name
            ))
        })
    }
}

fn map_open_error(path: &Path, error: SevenZError) -> ArchiveError {
    match error {
        SevenZError::PasswordRequired => ArchiveError::PasswordRequired,
        SevenZError::MaybeBadPassword(_) => ArchiveError::BadPassword,
        SevenZError::Unsupported(message) => ArchiveError::unsupported(message.into_owned()),
        SevenZError::UnsupportedCompressionMethod(method) => {
            ArchiveError::unsupported(format!("7z compression method {method}"))
        }
        SevenZError::Io(_, _) => ArchiveError::Open {
            path: path.to_path_buf(),
            source: Box::new(error),
        },
        _ => ArchiveError::corrupt(error.to_string()),
    }
}

fn map_read_error(error: SevenZError) -> ArchiveError {
    match error {
        SevenZError::PasswordRequired => ArchiveError::PasswordRequired,
        SevenZError::MaybeBadPassword(_) => ArchiveError::BadPassword,
        SevenZError::Unsupported(message) => ArchiveError::unsupported(message.into_owned()),
        SevenZError::UnsupportedCompressionMethod(method) => {
            ArchiveError::unsupported(format!("7z compression method {method}"))
        }
        SevenZError::Io(source, context) => {
            if context.is_empty() {
                ArchiveError::Io(source)
            } else {
                ArchiveError::Io(io::Error::new(
                    source.kind(),
                    format!("{context}: {source}"),
                ))
            }
        }
        other => ArchiveError::corrupt(other.to_string()),
    }
}
