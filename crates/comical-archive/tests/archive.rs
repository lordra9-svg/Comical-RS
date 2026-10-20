//! Behavior tests for the archive slice.

use std::fs;
use std::path::Path;

use comical_archive::{
    ArchiveError, ComicArchive, OpenOptions, format_for, open, open_with_options,
};
use tempfile::TempDir;
use xtask::{
    FixtureEntry, ZipEncryption, ZipMethod, write_7z, write_corrupt, write_rar, write_zip,
};

const PAGE_A: &[u8] = b"\x89PNG\r\n\x1a\npage-a";
const PAGE_B: &[u8] = b"\xff\xd8\xff\xe0page-b";
const PAGE_C: &[u8] = b"\x89PNG\r\n\x1a\npage-c";
const PAGE_D: &[u8] = b"GIF89a page-d";

fn names(archive: &dyn ComicArchive) -> Vec<&str> {
    archive
        .pages()
        .iter()
        .map(|page| page.name.as_str())
        .collect()
}

fn open_path(path: &Path) -> Box<dyn ComicArchive> {
    open(path).unwrap_or_else(|error| panic!("open {}: {error}", path.display()))
}

#[test]
fn zip_store_filters_and_sorts_pages() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("book.cbz");
    let entries = [
        FixtureEntry::new("page10.png", PAGE_A),
        FixtureEntry::new("page2.PNG", PAGE_B),
        FixtureEntry::new("sub/Page1.jpg", PAGE_C),
        FixtureEntry::new("cover.jpeg", PAGE_D),
        FixtureEntry::new("anim.GIF", PAGE_A),
        FixtureEntry::new("notes.txt", b"not a page"),
        FixtureEntry::new("fake.png.txt", b"still not a page"),
    ];
    write_zip(&path, &entries, ZipMethod::Store, ZipEncryption::None, None).unwrap();

    let archive = open_path(&path);
    assert_eq!(
        names(archive.as_ref()),
        vec![
            "anim.GIF",
            "cover.jpeg",
            "page10.png",
            "page2.PNG",
            "sub/Page1.jpg"
        ]
    );
    let sizes: Vec<u64> = archive.pages().iter().map(|page| page.size).collect();
    assert_eq!(
        sizes,
        vec![
            PAGE_A.len() as u64,
            PAGE_D.len() as u64,
            PAGE_A.len() as u64,
            PAGE_B.len() as u64,
            PAGE_C.len() as u64
        ]
    );
    assert_eq!(format_for(&path), Some(comical_archive::ArchiveFormat::Zip));
}

#[test]
fn zip_deflate_extracts_exact_bytes() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("book.zip");
    let entries = [
        FixtureEntry::new("a.png", PAGE_A),
        FixtureEntry::new("b.jpg", PAGE_B),
        FixtureEntry::new("c.gif", PAGE_C),
    ];
    write_zip(
        &path,
        &entries,
        ZipMethod::Deflate,
        ZipEncryption::None,
        None,
    )
    .unwrap();

    let mut archive = open_path(&path);
    assert_eq!(archive.read_page(0, 1024).unwrap(), PAGE_A);
    assert_eq!(archive.read_page(1, 1024).unwrap(), PAGE_B);
    assert_eq!(archive.read_page(2, 1024).unwrap(), PAGE_C);
}

#[test]
fn zip_aes256_password_is_supported_and_wrong_password_is_rejected() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("secret.cbz");
    let entries = [FixtureEntry::new("secret.png", PAGE_A)];
    write_zip(
        &path,
        &entries,
        ZipMethod::Deflate,
        ZipEncryption::Aes256,
        Some("open-sesame"),
    )
    .unwrap();

    let mut locked = open_path(&path);
    assert!(matches!(
        locked.read_page(0, 1024),
        Err(ArchiveError::PasswordRequired)
    ));

    let mut wrong =
        open_with_options(&path, OpenOptions::new().with_password(Some("wrong"))).unwrap();
    assert!(matches!(
        wrong.read_page(0, 1024),
        Err(ArchiveError::BadPassword)
    ));

    let mut archive =
        open_with_options(&path, OpenOptions::new().with_password(Some("open-sesame"))).unwrap();
    assert_eq!(archive.read_page(0, 1024).unwrap(), PAGE_A);
}

#[test]
fn rar_lists_and_extracts() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("book.cbr");
    let entries = [
        FixtureEntry::new("b.png", PAGE_B),
        FixtureEntry::new("A.jpg", PAGE_A),
        FixtureEntry::new("sub/c.JPEG", PAGE_C),
        FixtureEntry::new("readme.txt", b"not a page"),
    ];
    write_rar(&path, &entries, None).unwrap();

    let mut archive = open_path(&path);
    assert_eq!(
        names(archive.as_ref()),
        vec!["A.jpg", "b.png", "sub/c.JPEG"]
    );
    assert_eq!(archive.read_page(0, 1024).unwrap(), PAGE_A);
    assert_eq!(archive.read_page(1, 1024).unwrap(), PAGE_B);
    assert_eq!(archive.read_page(2, 1024).unwrap(), PAGE_C);
}

#[test]
fn rar_password_is_supported_and_wrong_password_is_rejected() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("secret.cbr");
    let entries = [FixtureEntry::new("secret.png", PAGE_A)];
    write_rar(&path, &entries, Some("open-sesame")).unwrap();

    let mut locked = open_path(&path);
    assert!(matches!(
        locked.read_page(0, 1024),
        Err(ArchiveError::PasswordRequired)
    ));

    let mut wrong =
        open_with_options(&path, OpenOptions::new().with_password(Some("wrong"))).unwrap();
    assert!(matches!(
        wrong.read_page(0, 1024),
        Err(ArchiveError::BadPassword)
    ));

    let mut archive =
        open_with_options(&path, OpenOptions::new().with_password(Some("open-sesame"))).unwrap();
    assert_eq!(archive.read_page(0, 1024).unwrap(), PAGE_A);
}

#[test]
fn sevenz_lists_and_extracts() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("book.cb7");
    let entries = [
        FixtureEntry::new("z.png", PAGE_C),
        FixtureEntry::new("a.png", PAGE_A),
        FixtureEntry::new("nested/m.jpg", PAGE_B),
        FixtureEntry::new("note.txt", b"not a page"),
    ];
    write_7z(&path, &entries).unwrap();

    let mut archive = open_path(&path);
    assert_eq!(
        names(archive.as_ref()),
        vec!["a.png", "nested/m.jpg", "z.png"]
    );
    assert_eq!(archive.read_page(0, 1024).unwrap(), PAGE_A);
    assert_eq!(archive.read_page(1, 1024).unwrap(), PAGE_B);
    assert_eq!(archive.read_page(2, 1024).unwrap(), PAGE_C);
}

#[test]
fn directory_is_walked_recursively_and_sorted() {
    let directory = TempDir::new().unwrap();
    let root = directory.path();
    fs::create_dir_all(root.join("chapter/two")).unwrap();
    fs::write(root.join("z.PNG"), PAGE_A).unwrap();
    fs::write(root.join("chapter/a.jpg"), PAGE_B).unwrap();
    fs::write(root.join("chapter/two/b.JPEG"), PAGE_C).unwrap();
    fs::write(root.join("chapter/skip.txt"), b"not a page").unwrap();

    let mut archive = open_path(root);
    assert_eq!(
        names(archive.as_ref()),
        vec!["chapter/a.jpg", "chapter/two/b.JPEG", "z.PNG"]
    );
    assert_eq!(archive.read_page(0, 1024).unwrap(), PAGE_B);
    assert_eq!(archive.read_page(1, 1024).unwrap(), PAGE_C);
    assert_eq!(archive.read_page(2, 1024).unwrap(), PAGE_A);
}

#[test]
fn out_of_range_index_is_rejected() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("book.cbz");
    write_zip(
        &path,
        &[FixtureEntry::new("only.png", PAGE_A)],
        ZipMethod::Store,
        ZipEncryption::None,
        None,
    )
    .unwrap();

    let mut archive = open_path(&path);
    match archive.read_page(1, 1024) {
        Err(ArchiveError::IndexOutOfRange { index, count }) => {
            assert_eq!(index, 1);
            assert_eq!(count, 1);
        }
        other => panic!("expected out-of-range error, got {other:?}"),
    }
}

#[test]
fn entry_size_cap_is_checked_before_allocation() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("book.cbz");
    write_zip(
        &path,
        &[FixtureEntry::new("large.png", PAGE_A)],
        ZipMethod::Store,
        ZipEncryption::None,
        None,
    )
    .unwrap();

    let mut archive = open_path(&path);
    match archive.read_page(0, 4) {
        Err(ArchiveError::EntryTooLarge { name, size, limit }) => {
            assert_eq!(name, "large.png");
            assert_eq!(size, PAGE_A.len() as u64);
            assert_eq!(limit, 4);
        }
        other => panic!("expected size cap error, got {other:?}"),
    }
}

#[test]
fn every_reader_enforces_the_size_cap() {
    let directory = TempDir::new().unwrap();
    let rar = directory.path().join("book.cbr");
    let sevenz = directory.path().join("book.cb7");
    let folder = directory.path().join("folder");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("large.png"), PAGE_A).unwrap();

    write_rar(&rar, &[FixtureEntry::new("large.png", PAGE_A)], None).unwrap();
    write_7z(&sevenz, &[FixtureEntry::new("large.png", PAGE_A)]).unwrap();

    for path in [&rar, &sevenz, &folder] {
        let mut archive = open_path(path);
        assert!(
            matches!(
                archive.read_page(0, 4),
                Err(ArchiveError::EntryTooLarge { .. })
            ),
            "cap not enforced for {}",
            path.display()
        );
    }
}

#[test]
fn corrupt_archives_report_clear_errors() {
    let directory = TempDir::new().unwrap();
    for (extension, bytes) in [
        ("cbz", b"not a zip archive".as_slice()),
        ("cbr", b"not a rar archive".as_slice()),
        ("cb7", b"not a 7z archive".as_slice()),
    ] {
        let path = directory.path().join(format!("broken.{extension}"));
        write_corrupt(&path, bytes).unwrap();
        match open(&path) {
            Err(ArchiveError::Corrupt(_)) | Err(ArchiveError::Open { .. }) => {}
            other => panic!(
                "expected corrupt-archive error for {extension}, got {}",
                match other {
                    Ok(_) => "a successfully opened archive".to_owned(),
                    Err(error) => error.to_string(),
                }
            ),
        }
    }
}

#[test]
fn archives_without_images_are_errors() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("text.cbz");
    write_zip(
        &path,
        &[FixtureEntry::new("readme.txt", b"no pages here")],
        ZipMethod::Store,
        ZipEncryption::None,
        None,
    )
    .unwrap();
    assert!(matches!(open(&path), Err(ArchiveError::NoPages)));

    let empty = TempDir::new().unwrap();
    assert!(matches!(open(empty.path()), Err(ArchiveError::NoPages)));
}

#[test]
fn unsupported_extension_is_rejected() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("book.rar.txt");
    fs::write(&path, b"whatever").unwrap();
    match open(&path) {
        Err(ArchiveError::CannotOpen { .. }) => {}
        other => panic!(
            "expected unsupported extension error, got {}",
            match other {
                Ok(_) => "a successfully opened archive".to_owned(),
                Err(error) => error.to_string(),
            }
        ),
    }
}
