//! Rebuilds the real-writer corpus in `tests/corpus`.
//!
//! The fixtures must keep coming from Info-ZIP and 7-Zip so the corpus proves
//! interoperability with other writers. This command only orchestrates them:
//! it writes the page stubs, runs the two archivers, truncates copies and
//! rewrites `manifest.json`.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::ser::{SerializeMap, Serializer};
use sha2::{Digest, Sha256};

use crate::tool;

const PASSWORD: &str = "corpus-secret";

/// Source page stubs: name, header bytes and the filler that pads the file.
///
/// The sizes and contents are what the committed fixtures were built from.
const PAGES: &[(&str, &[u8], u8, usize)] = &[
    ("a.png", b"\x89PNG\r\n\x1a\ninfozip-page-a", b'A', 4000),
    ("b.jpg", b"\xff\xd8\xff\xe0infozip-page-b", b'B', 4000),
    ("c.gif", b"GIF89ainfozip-page-c", b'C', 4000),
    (
        "uni-日本語.png",
        b"\x89PNG\r\n\x1a\nunicode-page",
        b'U',
        2000,
    ),
    (
        "deep/path/here/nested.png",
        b"\x89PNG\r\n\x1a\nnested-page",
        b'N',
        2000,
    ),
];

/// Page order used by the manifest's `pages` object.
const PAGE_ORDER: &[&str] = &[
    "a.png",
    "b.jpg",
    "c.gif",
    "uni-日本語.png",
    "deep/path/here/nested.png",
];

/// Expected outcome of one fixture.
#[derive(Debug, Serialize)]
struct Case {
    fixture: String,
    writer: &'static str,
    password: bool,
    pages: Vec<&'static str>,
    expect: &'static str,
}

#[derive(Debug, Serialize)]
struct PageInfo {
    size: usize,
    sha256: String,
}

/// Serializes pages as a map in insertion order, matching the committed file.
#[derive(Debug)]
struct Pages(Vec<(String, PageInfo)>);

impl Serialize for Pages {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (name, info) in &self.0 {
            map.serialize_entry(name, info)?;
        }
        map.end()
    }
}

#[derive(Debug, Serialize)]
struct Manifest {
    password: &'static str,
    pages: Pages,
    cases: Vec<Case>,
}

/// Regenerates every fixture and the manifest, then prints the test command.
pub fn run() -> Result<(), Box<dyn Error>> {
    if !tool::command_exists("zip") {
        return Err("zip (Info-ZIP) is required".into());
    }
    // Upstream 7-Zip ships `7zz`; most distributions also install a `7z` alias.
    let seven = ["7z", "7zz"]
        .into_iter()
        .find(|name| tool::command_exists(name))
        .ok_or("7z (7-Zip) is required")?;

    let root = workspace_root()?;
    let pages_dir = root.join("tests/corpus/pages");
    let fixtures = root.join("tests/corpus/fixtures");
    fs::create_dir_all(pages_dir.join("deep/path/here"))?;
    fs::create_dir_all(&fixtures)?;

    for &(name, ..) in PAGES {
        let bytes = page_bytes(name).ok_or("regen-corpus: unknown page stub")?;
        fs::write(pages_dir.join(name), bytes)?;
    }

    for entry in fs::read_dir(&fixtures)? {
        let path = entry?.path();
        if path.is_file() {
            fs::remove_file(path)?;
        }
    }

    zip(
        &pages_dir,
        &fixtures,
        "infozip-store.cbz",
        "-0",
        &THREE,
        None,
    )?;
    zip(
        &pages_dir,
        &fixtures,
        "infozip-deflate.cbz",
        "-9",
        &THREE,
        None,
    )?;
    zip(
        &pages_dir,
        &fixtures,
        "infozip-zipcrypto.cbz",
        "-9",
        &TWO,
        Some(PASSWORD),
    )?;
    zip(
        &pages_dir,
        &fixtures,
        "infozip-unicode-deep.cbz",
        "-9",
        &["uni-日本語.png", "deep/path/here/nested.png"],
        None,
    )?;

    seven_zip(
        seven,
        &pages_dir,
        &fixtures,
        "sevenzip-zip-deflate.cbz",
        &["-tzip", "-mx=9"],
        &THREE,
        None,
    )?;
    seven_zip(
        seven,
        &pages_dir,
        &fixtures,
        "sevenzip-lzma2.cb7",
        &["-t7z", "-m0=LZMA2", "-ms=off", "-mx=5"],
        &THREE,
        None,
    )?;
    seven_zip(
        seven,
        &pages_dir,
        &fixtures,
        "sevenzip-lzma.cb7",
        &["-t7z", "-m0=LZMA", "-ms=off", "-mx=5"],
        &THREE,
        None,
    )?;
    seven_zip(
        seven,
        &pages_dir,
        &fixtures,
        "sevenzip-solid.cb7",
        &["-t7z", "-m0=LZMA2", "-ms=on", "-mx=5"],
        &THREE,
        None,
    )?;
    seven_zip(
        seven,
        &pages_dir,
        &fixtures,
        "sevenzip-aes.cb7",
        &["-t7z", "-m0=LZMA2", "-ms=off", "-mx=5", "-mhe=off"],
        &TWO,
        Some(PASSWORD),
    )?;
    seven_zip(
        seven,
        &pages_dir,
        &fixtures,
        "sevenzip-aes-headers.cb7",
        &["-t7z", "-m0=LZMA2", "-ms=off", "-mx=5", "-mhe=on"],
        &TWO,
        Some(PASSWORD),
    )?;

    truncate(
        &fixtures.join("infozip-deflate.cbz"),
        &fixtures.join("truncated-zip.cbz"),
    )?;
    truncate(
        &fixtures.join("sevenzip-lzma2.cb7"),
        &fixtures.join("truncated-7z.cb7"),
    )?;

    let manifest = manifest(&pages_dir)?;
    fs::write(
        root.join("tests/corpus/manifest.json"),
        serde_json::to_string_pretty(&manifest)? + "\n",
    )?;

    println!("regenerated {} fixtures", fs::read_dir(&fixtures)?.count());
    println!("Run: cargo test -p comical-archive --test corpus");
    Ok(())
}

const THREE: [&str; 3] = ["a.png", "b.jpg", "c.gif"];
const TWO: [&str; 2] = ["a.png", "b.jpg"];

/// Runs Info-ZIP over `members`, writing `fixtures/name`.
fn zip(
    pages: &Path,
    fixtures: &Path,
    name: &str,
    method: &str,
    members: &[&str],
    password: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let destination = fixtures.join(name);
    let destination = destination.to_string_lossy().into_owned();
    let protected;
    let mut args = vec![method, "-X"];
    if let Some(password) = password {
        protected = format!("-P{password}");
        args.push(&protected);
    }
    args.push(&destination);
    args.extend_from_slice(members);
    tool::run("zip", &args, Some(pages))
}

/// Runs 7-Zip over `members`, writing `fixtures/name`.
fn seven_zip(
    program: &str,
    pages: &Path,
    fixtures: &Path,
    name: &str,
    switches: &[&str],
    members: &[&str],
    password: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let destination = fixtures.join(name);
    let destination = destination.to_string_lossy().into_owned();
    let protected;
    let mut args = vec!["a"];
    args.extend_from_slice(switches);
    if let Some(password) = password {
        protected = format!("-p{password}", password = password);
        args.push(&protected);
    }
    args.push("-bso0");
    args.push("-bsp0");
    args.push(&destination);
    args.extend_from_slice(members);
    tool::run(program, &args, Some(pages))
}

/// Bytes of a page stub: its header followed by the filler byte.
fn page_bytes(name: &str) -> Option<Vec<u8>> {
    let (_, header, filler, padding) = PAGES.iter().find(|(stub, ..)| *stub == name)?;
    let mut bytes = header.to_vec();
    bytes.extend(std::iter::repeat_n(*filler, *padding));
    Some(bytes)
}

/// How much of a fixture to keep: a quarter, but at least 32 bytes.
fn truncated_len(len: usize) -> usize {
    (len / 4).max(32).min(len)
}

/// Copies the first quarter of `source`, but at least 32 bytes, to `destination`.
fn truncate(source: &Path, destination: &Path) -> Result<(), Box<dyn Error>> {
    let bytes = fs::read(source)?;
    fs::write(destination, &bytes[..truncated_len(bytes.len())])?;
    Ok(())
}

fn manifest(pages_dir: &Path) -> Result<Manifest, Box<dyn Error>> {
    let mut pages = Vec::new();
    for name in PAGE_ORDER {
        let bytes = fs::read(pages_dir.join(name))?;
        pages.push((
            (*name).to_owned(),
            PageInfo {
                size: bytes.len(),
                sha256: sha256(&bytes),
            },
        ));
    }

    let cases = vec![
        case("infozip-store.cbz", "infozip", false, &THREE, "ok"),
        case("infozip-deflate.cbz", "infozip", false, &THREE, "ok"),
        case("sevenzip-zip-deflate.cbz", "sevenzip", false, &THREE, "ok"),
        case("sevenzip-lzma2.cb7", "sevenzip", false, &THREE, "ok"),
        case("sevenzip-lzma.cb7", "sevenzip", false, &THREE, "ok"),
        case("sevenzip-solid.cb7", "sevenzip", false, &THREE, "ok"),
        case(
            "infozip-unicode-deep.cbz",
            "infozip",
            false,
            &["deep/path/here/nested.png", "uni-日本語.png"],
            "ok",
        ),
        case("infozip-zipcrypto.cbz", "infozip", true, &TWO, "ok"),
        case("sevenzip-aes.cb7", "sevenzip", true, &TWO, "ok"),
        case("sevenzip-aes-headers.cb7", "sevenzip", true, &TWO, "ok"),
        case("truncated-zip.cbz", "truncated", false, &[], "open_error"),
        case("truncated-7z.cb7", "truncated", false, &[], "open_error"),
    ];

    Ok(Manifest {
        password: PASSWORD,
        pages: Pages(pages),
        cases,
    })
}

fn case(
    fixture: &str,
    writer: &'static str,
    password: bool,
    pages: &[&'static str],
    expect: &'static str,
) -> Case {
    Case {
        fixture: fixture.to_owned(),
        writer,
        password,
        pages: pages.to_vec(),
        expect,
    }
}

fn sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "regen-corpus: xtask has no workspace parent".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The manifest records digests of the committed page stubs. If a stub
    /// changes without the manifest, the corpus test would still pass against
    /// the old pages while regeneration produces different bytes.
    #[test]
    fn stub_bytes_match_the_committed_manifest() {
        let text = fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/corpus/manifest.json"),
        )
        .expect("read the committed manifest");
        let value: serde_json::Value = serde_json::from_str(&text).expect("parse manifest");
        let pages = value["pages"].as_object().expect("pages object");

        assert_eq!(pages.len(), PAGES.len(), "stub count");
        for &(name, ..) in PAGES {
            let bytes = page_bytes(name).expect("page stub");
            let recorded = pages
                .get(name)
                .unwrap_or_else(|| panic!("{name} in manifest"));
            assert_eq!(
                recorded["size"].as_u64().expect("size"),
                bytes.len() as u64,
                "{name} size"
            );
            assert_eq!(
                recorded["sha256"].as_str().expect("sha256"),
                sha256(&bytes),
                "{name} hash"
            );
        }
    }

    #[test]
    fn page_order_covers_every_stub_once() {
        let mut listed: Vec<&str> = PAGES.iter().map(|(name, ..)| *name).collect();
        let mut ordered = PAGE_ORDER.to_vec();
        listed.sort_unstable();
        ordered.sort_unstable();
        assert_eq!(listed, ordered);
    }

    #[test]
    fn unknown_stub_names_are_rejected() {
        assert!(page_bytes("missing.png").is_none());
    }

    #[test]
    fn truncation_keeps_a_quarter_with_a_32_byte_floor() {
        assert_eq!(truncated_len(4096), 1024);
        assert_eq!(truncated_len(100), 32);
        assert_eq!(truncated_len(10), 10);
        assert_eq!(truncated_len(0), 0);
    }

    /// Guards the manifest key order, page hashes and the case list at once:
    /// regeneration must reproduce the committed file exactly.
    #[test]
    fn manifest_reproduces_the_committed_file() {
        let root = workspace_root().expect("workspace root");
        // Checkouts made before .gitattributes pinned this file, or with a
        // core.autocrlf setting of their own, may hand us CRLF. Line endings are
        // not part of the manifest, so normalize them and still compare the
        // bytes (formatting, key order, hashes) exactly.
        let expected = fs::read_to_string(root.join("tests/corpus/manifest.json"))
            .expect("committed file")
            .replace("\r\n", "\n");
        let regenerated = manifest(&root.join("tests/corpus/pages")).expect("manifest");
        let actual = serde_json::to_string_pretty(&regenerated).expect("json") + "\n";
        assert_eq!(actual, expected);
    }

    #[test]
    fn pages_serialize_in_insertion_order() {
        let pages = Pages(vec![
            (
                "b.png".to_owned(),
                PageInfo {
                    size: 2,
                    sha256: "x".to_owned(),
                },
            ),
            (
                "a.png".to_owned(),
                PageInfo {
                    size: 1,
                    sha256: "y".to_owned(),
                },
            ),
        ]);
        let text = serde_json::to_string(&pages).expect("json");
        let second = text.find("b.png").expect("b.png");
        let first = text.find("a.png").expect("a.png");
        assert!(second < first, "insertion order lost: {text}");
    }
}
