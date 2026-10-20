//! Recursive directory books.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use comical_core::PageEntry;

use crate::{
    ArchiveError, ArchiveFormat, Candidate, ComicArchive, check_declared_size, check_index,
    is_image_name, page_count_error, select_pages,
};

pub(crate) fn open(root: &Path) -> Result<Box<dyn ComicArchive>, ArchiveError> {
    let mut candidates = Vec::new();
    walk(root, Path::new(""), &mut candidates)?;
    let (pages, sources) = select_pages(candidates);
    if let Some(error) = page_count_error(&pages) {
        return Err(error);
    }
    Ok(Box::new(DirectoryBook {
        root: root.to_path_buf(),
        pages,
        sources,
    }))
}

fn walk(
    root: &Path,
    relative: &Path,
    candidates: &mut Vec<Candidate<PathBuf>>,
) -> Result<(), ArchiveError> {
    let directory = root.join(relative);
    let mut entries = fs::read_dir(&directory)?.collect::<Result<Vec<_>, _>>()?;
    // Deterministic traversal keeps duplicate handling predictable across
    // platforms. The final book sort is independent of this order.
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let file_type = entry.file_type()?;
        let child = relative.join(entry.file_name());
        if file_type.is_dir() {
            walk(root, &child, candidates)?;
        } else if file_type.is_file() {
            let name = to_slash(&child);
            if is_image_name(&name) {
                let size = entry.metadata()?.len();
                candidates.push(Candidate {
                    name,
                    size,
                    source: child,
                });
            }
        }
    }
    Ok(())
}

fn to_slash(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

struct DirectoryBook {
    root: PathBuf,
    pages: Vec<PageEntry>,
    sources: Vec<PathBuf>,
}

impl ComicArchive for DirectoryBook {
    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::Directory
    }

    fn pages(&self) -> &[PageEntry] {
        &self.pages
    }

    fn read_page(&mut self, index: usize, limit: u64) -> Result<Vec<u8>, ArchiveError> {
        check_index(index, &self.pages)?;
        let page = &self.pages[index];
        check_declared_size(&page.name, page.size, limit)?;

        let path = self.root.join(&self.sources[index]);
        let file = File::open(path)?;
        read_bounded(file, &page.name, limit)
    }
}

fn read_bounded<R: Read>(reader: R, name: &str, limit: u64) -> Result<Vec<u8>, ArchiveError> {
    let mut bytes = Vec::new();
    reader
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(ArchiveError::EntryTooLarge {
            name: name.to_owned(),
            size: bytes.len() as u64,
            limit,
        });
    }
    Ok(bytes)
}
