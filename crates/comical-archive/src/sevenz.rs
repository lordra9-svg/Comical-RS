//! 7z books.

use std::borrow::Cow;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use comical_core::PageEntry;
use sevenz_rust2::{ArchiveReader, Error as SevenZError, Password};

use crate::{
    ArchiveError, ArchiveFormat, Candidate, ComicArchive, OpenOptions, check_declared_size,
    check_index, is_image_name, page_count_error, select_pages,
};

pub(crate) fn open(
    path: &Path,
    options: &OpenOptions<'_>,
) -> Result<Box<dyn ComicArchive>, ArchiveError> {
    let password = options.password.unwrap_or("");
    let reader = ArchiveReader::open(path, Password::from(password))
        .map_err(|error| map_open_error(path, error))?;
    let archive = reader.archive();

    // sevenz-rust2 decodes block by block and then yields the files that have
    // no block of their own. Record that exact callback order so a page can be
    // located later. The order must include entries that are later skipped
    // (directories, zero-byte files), because the reader still yields them to
    // the callback.
    let order = callback_order(archive);
    let mut ordinal = vec![None; archive.files.len()];
    for (position, file_index) in order.iter().copied().enumerate() {
        if let Some(slot) = ordinal.get_mut(file_index) {
            *slot = Some(position);
        }
    }

    let mut candidates = Vec::new();
    let mut original_names = std::collections::HashMap::new();
    for (file_index, entry) in archive.files.iter().enumerate() {
        if entry.is_directory()
            || !entry.has_stream
            || entry.size == 0
            || !is_image_name(&entry.name)
        {
            continue;
        }
        let Some(position) = ordinal.get(file_index).copied().flatten() else {
            continue;
        };
        original_names.insert(position, entry.name.clone());
        candidates.push(Candidate {
            name: entry.name.clone(),
            size: entry.size,
            source: position,
        });
    }

    let (pages, sources) = select_pages(candidates, options.natural_sort);
    if let Some(error) = page_count_error(&pages) {
        return Err(error);
    }
    let source_names = sources
        .iter()
        .map(|position| original_names.get(position).cloned().unwrap_or_default())
        .collect();

    Ok(Box::new(SevenZBook {
        path: path.to_path_buf(),
        pages,
        sources,
        source_names,
        password: options.password.map(str::to_owned),
    }))
}

/// Reproduces the order in which `for_each_entries` hands entries to its
/// callback.
///
/// The reader decodes one block at a time, in block order, and then yields
/// every file that is not part of a block. A file without a stream of its own
/// is still yielded, so the position has to count it even when the page is
/// skipped later.
fn callback_order(archive: &sevenz_rust2::Archive) -> Vec<usize> {
    let mut per_block = vec![Vec::new(); archive.blocks.len()];
    let mut without_block = Vec::new();
    for (file_index, block_index) in archive.stream_map.file_block_index.iter().enumerate() {
        match *block_index {
            Some(block) if block < per_block.len() => per_block[block].push(file_index),
            _ => without_block.push(file_index),
        }
    }
    let mut order = Vec::with_capacity(archive.files.len());
    for entries in per_block {
        order.extend(entries);
    }
    order.extend(without_block);
    order
}

struct SevenZBook {
    path: PathBuf,
    pages: Vec<PageEntry>,
    sources: Vec<usize>,
    /// Original archive names in `sources` order.
    ///
    /// `select_pages` may strip a common wrapper directory from the public
    /// page names, so the on-disk name cannot be reconstructed from `pages`.
    source_names: Vec<String>,
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
            ArchiveReader::open(&self.path, Password::from(password)).map_err(map_read_error)?;

        let target = self.sources[index];
        let expected = self
            .source_names
            .get(index)
            .cloned()
            .unwrap_or_else(|| page.name.clone());
        let mut seen = 0usize;
        let mut found = None;
        let mut overflow = false;
        let mut overflow_size = 0u64;

        reader
            .for_each_entries(|entry, source| {
                // Returning false stops the current block, not the whole
                // traversal. Skip every later block once the page is found.
                if found.is_some() || overflow {
                    return Ok(false);
                }
                if seen == target {
                    if entry.name != expected {
                        return Err(SevenZError::Other(Cow::Borrowed(
                            "7z entry order changed while reading",
                        )));
                    }
                    let mut bytes = Vec::new();
                    source
                        .take(limit.saturating_add(1))
                        .read_to_end(&mut bytes)
                        .map_err(no_context)?;
                    if bytes.len() as u64 > limit {
                        overflow = true;
                        overflow_size = bytes.len() as u64;
                    } else {
                        found = Some(bytes);
                    }
                    return Ok(false);
                }
                // Solid blocks require prior entries to be fully consumed;
                // otherwise later CRC checks fail (sevenz-rust issue #31).
                io::copy(source, &mut io::sink()).map_err(no_context)?;
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

/// Wraps a callback I/O failure the way the crate expects.
fn no_context(error: io::Error) -> SevenZError {
    SevenZError::Io(error, Cow::Borrowed(""))
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
