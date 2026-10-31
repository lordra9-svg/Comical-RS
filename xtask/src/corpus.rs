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

    for (name, header, filler, padding) in PAGES {
        let mut bytes = header.to_vec();
        bytes.extend(std::iter::repeat_n(*filler, *padding));
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

/// Copies the first quarter of `source`, but at least 32 bytes, to `destination`.
fn truncate(source: &Path, destination: &Path) -> Result<(), Box<dyn Error>> {
    let bytes = fs::read(source)?;
    let keep = (bytes.len() / 4).max(32).min(bytes.len());
    fs::write(destination, &bytes[..keep])?;
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
