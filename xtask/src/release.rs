//! Builds release artifacts for every platform this host can produce.
//!
//! Linux always produces a bare binary, a tarball and (when `cargo-deb` is
//! installed) a `.deb`. Windows produces an `.exe` on a Windows host, or by
//! cross-compiling with mingw-w64 on Linux; a `.msi` needs `cargo-wix`. macOS
//! produces an `.app` with `cargo-bundle`. Every artifact is listed in
//! `SHA256SUMS.txt`.

use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::write::GzEncoder;
use sha2::{Digest, Sha256};

use crate::tool;

const PACKAGE: &str = "comical-app";
const BINARY: &str = "comical";
const WINDOWS_MSVC: &str = "x86_64-pc-windows-msvc";
const WINDOWS_GNU: &str = "x86_64-pc-windows-gnu";
const MINGW_CC: &str = "x86_64-w64-mingw32-gcc";

/// Builds every artifact this host supports into `target/release-dist`.
pub fn run(version: Option<String>) -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let version = match version {
        Some(version) => version,
        None => workspace_version(&root)?,
    };
    if !is_version(&version) {
        return Err(format!("release: '{version}' is not a MAJOR.MINOR.PATCH version").into());
    }

    let dist = root.join("target/release-dist").join(&version);
    fs::create_dir_all(&dist)?;
    println!("Building Comical v{version} into {}...", dist.display());

    let host = std::env::consts::OS;
    if host == "linux" {
        linux(&root, &dist, &version)?;
    } else {
        println!("[linux] SKIP: this host is {host}, not linux");
    }

    if host == "windows" {
        windows(&root, &dist, &version)?;
    } else if target_installed(WINDOWS_GNU) && tool::command_exists(MINGW_CC) {
        windows_cross(&root, &dist)?;
    } else {
        println!("[windows] SKIP: not on Windows and no mingw-w64 cross toolchain on PATH");
    }

    if host == "macos" {
        macos(&root, &dist)?;
    } else {
        println!("[macos] SKIP: not on macOS");
    }

    checksums(&dist)?;

    println!();
    println!("Release artifacts in {}:", dist.display());
    for (name, bytes) in listing(&dist)? {
        println!("  {name:<40} {:>8} bytes", bytes);
    }
    println!();
    println!("Next steps:");
    println!("  1. Test each binary on its target platform");
    println!("  2. Sign Windows/macOS binaries if applicable");
    println!("  3. Create GitHub release with changelog and SHA256SUMS.txt");
    println!("  4. Upload artifacts to release");
    Ok(())
}

fn linux(root: &Path, dist: &Path, version: &str) -> Result<(), Box<dyn Error>> {
    println!("[linux-x86_64] cargo build --release");
    tool::cargo(&["build", "--release", "-p", PACKAGE, "--bin", BINARY])?;
    let built = root.join("target/release").join(BINARY);
    let binary = dist.join("comical-linux-x86_64");
    fs::copy(&built, &binary)?;

    println!("[linux-tar] comical-{version}-linux-x86_64.tar.gz");
    write_tarball(
        &binary,
        BINARY,
        &dist.join(format!("comical-{version}-linux-x86_64.tar.gz")),
    )?;

    if tool::command_exists("cargo-deb") {
        println!("[linux-deb] cargo deb");
        let output = dist.join(format!("comical_{version}-1_amd64.deb"));
        tool::cargo(&[
            "deb",
            "-p",
            PACKAGE,
            "--no-build",
            "--output",
            &output.to_string_lossy(),
        ])?;
    } else {
        println!("[linux-deb] SKIP: cargo-deb not installed (cargo install cargo-deb)");
    }
    Ok(())
}

fn windows(root: &Path, dist: &Path, version: &str) -> Result<(), Box<dyn Error>> {
    println!("[windows] cargo build --release --target {WINDOWS_MSVC}");
    tool::cargo(&[
        "build",
        "--release",
        "-p",
        PACKAGE,
        "--bin",
        BINARY,
        "--target",
        WINDOWS_MSVC,
    ])?;
    fs::copy(
        root.join("target")
            .join(WINDOWS_MSVC)
            .join("release/comical.exe"),
        dist.join("comical-windows-x86_64.exe"),
    )?;

    if tool::command_exists("cargo-wix") {
        println!("[windows-msi] cargo wix");
        let output = dist.join(format!("comical-{version}-x86_64.msi"));
        tool::cargo(&[
            "wix",
            "-p",
            PACKAGE,
            "--no-build",
            "--nocapture",
            "--output",
            &output.to_string_lossy(),
        ])?;
    } else {
        println!("[windows-msi] SKIP: cargo-wix not installed (cargo install cargo-wix)");
    }
    Ok(())
}

/// Cross-compiles the Windows executable from a non-Windows host.
fn windows_cross(root: &Path, dist: &Path) -> Result<(), Box<dyn Error>> {
    println!("[windows] cargo build --release --target {WINDOWS_GNU} (cross)");
    tool::cargo(&[
        "build",
        "--release",
        "-p",
        PACKAGE,
        "--bin",
        BINARY,
        "--target",
        WINDOWS_GNU,
    ])?;
    fs::copy(
        root.join("target")
            .join(WINDOWS_GNU)
            .join("release/comical.exe"),
        dist.join("comical-windows-x86_64.exe"),
    )?;
    Ok(())
}

fn macos(root: &Path, dist: &Path) -> Result<(), Box<dyn Error>> {
    if !tool::command_exists("cargo-bundle") {
        println!("[macos] SKIP: cargo-bundle not installed (cargo install cargo-bundle)");
        return Ok(());
    }
    println!("[macos] cargo bundle --release");
    tool::cargo(&["bundle", "--release", "-p", PACKAGE])?;
    copy_directory(
        &root.join("target/release/bundle/osx/Comical.app"),
        &dist.join("Comical.app"),
    )?;
    Ok(())
}

/// Writes a gzipped tar archive holding a single file.
fn write_tarball(source: &Path, name: &str, destination: &Path) -> Result<(), Box<dyn Error>> {
    let file = fs::File::create(destination)?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = tar::Builder::new(encoder);

    let mut header = tar::Header::new_gnu();
    header.set_metadata(&fs::metadata(source)?);
    header.set_path(name)?;
    header.set_mode(0o755);
    // Do not leak the building user's ids into the published archive.
    header.set_uid(0);
    header.set_gid(0);
    header.set_cksum();
    let mut data = fs::File::open(source)?;
    builder.append(&header, &mut data)?;

    builder.into_inner()?.finish()?;
    Ok(())
}

/// Writes `SHA256SUMS.txt` for every file in `dist`, in the `sha256sum` format.
fn checksums(dist: &Path) -> Result<(), Box<dyn Error>> {
    let mut rows = Vec::new();
    for (name, path) in files(dist)? {
        if name == "SHA256SUMS.txt" {
            continue;
        }
        rows.push((name, sha256(&path)?));
    }
    rows.sort();

    let mut text = String::new();
    for (name, digest) in rows {
        text.push_str(&format!("{digest}  ./{name}\n"));
    }
    fs::write(dist.join("SHA256SUMS.txt"), text)?;
    println!("[checksums] SHA256SUMS.txt");
    Ok(())
}

fn listing(dist: &Path) -> Result<Vec<(String, u64)>, Box<dyn Error>> {
    let mut rows = Vec::new();
    for (name, path) in files(dist)? {
        rows.push((name, fs::metadata(path)?.len()));
    }
    rows.sort();
    Ok(rows)
}

fn files(directory: &Path) -> Result<Vec<(String, PathBuf)>, Box<dyn Error>> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if !entry.path().is_file() {
            continue;
        }
        entries.push((
            entry.file_name().to_string_lossy().into_owned(),
            entry.path(),
        ));
    }
    Ok(entries)
}

fn sha256(path: &Path) -> Result<String, Box<dyn Error>> {
    let mut hasher = Sha256::new();
    let mut file = fs::File::open(path)?;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.path().is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn target_installed(target: &str) -> bool {
    let rustup = std::env::var("RUSTUP").unwrap_or_else(|_| "rustup".to_owned());
    let Ok(output) = std::process::Command::new(rustup)
        .args(["target", "list", "--installed"])
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|line| line.trim() == target)
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "release: xtask has no workspace parent".into())
}

/// Reads `[workspace.package] version`, the only place the version exists.
fn workspace_version(root: &Path) -> Result<String, Box<dyn Error>> {
    let manifest = fs::read_to_string(root.join("Cargo.toml"))?;
    let mut in_package = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[workspace.package]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(rest) = line.strip_prefix("version")
            && let Some(value) = rest.trim_start().strip_prefix('=')
        {
            return Ok(value.trim().trim_matches('"').to_owned());
        }
    }
    Err("release: could not determine the version from the workspace Cargo.toml".into())
}

fn is_version(value: &str) -> bool {
    let mut parts = value.split('.');
    let major = parts.next();
    let minor = parts.next();
    let patch = parts.next();
    if parts.next().is_some() {
        return false;
    }
    [major, minor, patch].into_iter().all(|part| {
        part.is_some_and(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    })
}
