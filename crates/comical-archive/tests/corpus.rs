//! Interoperability corpus: archives written by Info-ZIP and 7-Zip.
//!
//! Fixtures live in `tests/corpus/`. They are not copyrighted comic pages;
//! payloads are synthetic page stubs. See `tests/corpus/README.md`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use comical_archive::{ArchiveError, OpenOptions, open, open_with_options};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Deserialize)]
struct Manifest {
    password: String,
    pages: HashMap<String, PageMeta>,
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct PageMeta {
    size: u64,
    sha256: String,
}

#[derive(Debug, Deserialize)]
struct Case {
    fixture: String,
    password: bool,
    pages: Vec<String>,
    expect: String,
}

fn corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus")
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn load_manifest() -> Manifest {
    let path = corpus_root().join("manifest.json");
    let text = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("read {}: {error}", path.display());
    });
    serde_json::from_str(&text).expect("parse tests/corpus/manifest.json")
}

fn load_page_bytes(name: &str) -> Vec<u8> {
    let path = corpus_root().join("pages").join(name);
    fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn page_payloads_match_manifest_hashes() {
    let manifest = load_manifest();
    for (name, meta) in &manifest.pages {
        let bytes = load_page_bytes(name);
        assert_eq!(bytes.len() as u64, meta.size, "{name} size");
        assert_eq!(sha256_hex(&bytes), meta.sha256, "{name} sha256");
    }
}

#[test]
fn real_writer_corpus_lists_and_extracts_exact_bytes() {
    let manifest = load_manifest();
    let fixtures = corpus_root().join("fixtures");

    for case in &manifest.cases {
        let path = fixtures.join(&case.fixture);
        assert!(path.is_file(), "missing fixture {}", path.display());

        match case.expect.as_str() {
            "ok" => assert_case_ok(&path, case, &manifest),
            "open_error" => assert_case_open_error(&path),
            other => panic!("unknown expect {other} in {}", case.fixture),
        }
    }
}

fn assert_case_ok(path: &Path, case: &Case, manifest: &Manifest) {
    let mut archive = if case.password {
        open_with_options(
            path,
            OpenOptions::new().with_password(Some(manifest.password.as_str())),
        )
        .unwrap_or_else(|error| panic!("open {}: {error}", path.display()))
    } else {
        open(path).unwrap_or_else(|error| panic!("open {}: {error}", path.display()))
    };

    let names: Vec<&str> = archive
        .pages()
        .iter()
        .map(|page| page.name.as_str())
        .collect();
    assert_eq!(names, case.pages, "{}", case.fixture);

    for (index, name) in case.pages.iter().enumerate() {
        let expected = load_page_bytes(name);
        let meta = manifest
            .pages
            .get(name)
            .unwrap_or_else(|| panic!("manifest missing page {name}"));
        assert_eq!(expected.len() as u64, meta.size, "{name}");
        let got = archive
            .read_page(index, expected.len() as u64 + 1024)
            .unwrap_or_else(|error| panic!("read {}[{index}]: {error}", case.fixture));
        assert_eq!(got, expected, "{} page {name}", case.fixture);
        assert_eq!(
            sha256_hex(&got),
            meta.sha256,
            "{} page {name} hash",
            case.fixture
        );
    }
}

fn assert_case_open_error(path: &Path) {
    match open(path) {
        Ok(_) => panic!(
            "truncated corpus fixture should fail to open: {}",
            path.display()
        ),
        Err(
            ArchiveError::Corrupt(_)
            | ArchiveError::Open { .. }
            | ArchiveError::Unsupported(_)
            | ArchiveError::Io(_),
        ) => {}
        Err(other) => panic!("unexpected error for {}: {other}", path.display()),
    }
}
