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
use zip::write::SimpleFileOptions as ZipFileOptions;
use zip::{CompressionMethod, ZipWriter as ZapWriter};

use crate::tool;

const PACKAGE: &str = "comical-app";
const BINARY: &str = "comical";
const WINDOWS_MSVC: &str = "x86_64-pc-windows-msvc";
const WINDOWS_GNU: &str = "x86_64-pc-windows-gnu";
const MINGW_CC: &str = "x86_64-w64-mingw32-gcc";
/// macOS targets, combined into one universal bundle when both are installed.
const MACOS_TARGETS: [&str; 2] = ["aarch64-apple-darwin", "x86_64-apple-darwin"];
/// Fragment holding the associations Finder reads for a double-clicked
/// `.cbz`, `.cbr` or `.cb7`.
const DOCUMENT_TYPES: &str = "crates/comical-app/assets/macos-document-types.plist";

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
        windows_cross(&root, &dist, &version)?;
    } else {
        println!("[windows] SKIP: not on Windows and no mingw-w64 cross toolchain on PATH");
    }

    if host == "macos" {
        macos(&root, &dist, &version)?;
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
    println!(
        "  1. Validate the directory: cargo run -p xtask -- verify {}",
        dist.display()
    );
    println!("  2. Test each binary on its target platform");
    println!("  3. Sign Windows/macOS binaries if applicable");
    println!("  4. Create GitHub release with changelog and SHA256SUMS.txt");
    println!("  5. Upload artifacts to release");
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
    msi(dist, version)?;
    Ok(())
}

/// Cross-compiles the Windows executable from a non-Windows host.
fn windows_cross(root: &Path, dist: &Path, version: &str) -> Result<(), Box<dyn Error>> {
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
    msi(dist, version)?;
    Ok(())
}

/// Builds the MSI from the Windows executable already in `dist`.
///
/// Prefers `wixl` from msitools, which reads the same WiX v3 source on Linux,
/// macOS and Windows; falls back to `cargo-wix` on Windows. Both are told the
/// binary path and version of this run, so the MSI never packages a stale file.
fn msi(dist: &Path, version: &str) -> Result<(), Box<dyn Error>> {
    let exe = dist.join("comical-windows-x86_64.exe");
    if !exe.is_file() {
        return Err(format!("windows-msi: {} does not exist", exe.display()).into());
    }
    let source = wix_source()?;
    let output = dist.join(format!("comical-{version}-x86_64.msi"));

    if tool::command_exists("wixl") {
        println!("[windows-msi] wixl -> {}", output.display());
        let args = wixl_args(&source, &exe, &output, version);
        let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
        // Run from the wix directory: WixUI_Minimal reads License.rtf from it.
        tool::run("wixl", &borrowed, source.parent())?;
    } else if tool::command_exists("cargo-wix") {
        println!("[windows-msi] cargo wix -> {}", output.display());
        let args = wix_args(&exe, &output);
        let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
        tool::cargo(&borrowed)?;
    } else {
        println!("[windows-msi] SKIP: install wixl (msitools) or cargo-wix to build it");
        return Ok(());
    }

    if !output.is_file() {
        return Err(format!("windows-msi: no MSI was written to {}", output.display()).into());
    }
    Ok(())
}

/// The WiX source lives next to the crate that defines the binary.
fn wix_source() -> Result<PathBuf, Box<dyn Error>> {
    let source = workspace_root()?.join("crates/comical-app/wix/main.wxs");
    if !source.is_file() {
        return Err(format!("windows-msi: {} does not exist", source.display()).into());
    }
    Ok(source)
}

/// Command line for `wixl`. The UI fragment comes from wixl-data, and the two
/// variables are what the `.wxs` requires.
fn wixl_args(source: &Path, exe: &Path, output: &Path, version: &str) -> Vec<String> {
    let mut args = vec![
        "--arch".to_owned(),
        "x64".to_owned(),
        "--ext".to_owned(),
        "ui".to_owned(),
    ];
    // Set when wixl-data was unpacked somewhere other than its compiled-in
    // default, as happens with a portable install.
    for (variable, flag) in [("WIXL_EXTDIR", "--extdir"), ("WIXL_WXIDIR", "--wxidir")] {
        if let Ok(directory) = std::env::var(variable)
            && !directory.is_empty()
        {
            args.push(flag.to_owned());
            args.push(directory);
        }
    }
    args.push("-D".to_owned());
    args.push(format!("Version={version}"));
    args.push("-D".to_owned());
    args.push(format!("BinaryPath={}", exe.display()));
    args.push("-o".to_owned());
    args.push(output.to_string_lossy().into_owned());
    args.push(
        source
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| source.to_string_lossy().into_owned()),
    );
    args
}

/// Command line for `cargo wix`, including the `candle` argument that points it
/// at the binary this run built.
fn wix_args(exe: &Path, output: &Path) -> Vec<String> {
    vec![
        "wix".to_owned(),
        "-p".to_owned(),
        PACKAGE.to_owned(),
        "--no-build".to_owned(),
        "--nocapture".to_owned(),
        "--output".to_owned(),
        output.to_string_lossy().into_owned(),
        // Everything after `--` is forwarded to candle.
        "--".to_owned(),
        format!("-dBinaryPath={}", exe.display()),
    ]
}

fn macos(root: &Path, dist: &Path, version: &str) -> Result<(), Box<dyn Error>> {
    if !tool::command_exists("cargo-bundle") {
        println!("[macos] SKIP: cargo-bundle not installed (cargo install cargo-bundle)");
        return Ok(());
    }
    // Both targets when rustup has them: cargo-bundle combines them with `lipo`,
    // so one download runs natively on Apple silicon and on Intel.
    let universal = MACOS_TARGETS.iter().all(|target| target_installed(target));
    let mut args = vec!["bundle", "--release", "-p", PACKAGE];
    if universal {
        for target in MACOS_TARGETS {
            args.push("--target");
            args.push(target);
        }
    }
    println!(
        "[macos] cargo bundle --release{}",
        if universal { " (universal)" } else { "" }
    );
    tool::cargo(&args)?;

    let app = bundle_app_path(root, universal);
    add_document_types(root, &app)?;
    copy_directory(&app, &dist.join("Comical.app"))?;

    // A bundle is a directory and a release asset has to be a file, so the
    // published artifact is the zipped bundle.
    let arch = if universal {
        "universal"
    } else {
        std::env::consts::ARCH
    };
    let archive = dist.join(format!("comical-{version}-macos-{arch}.zip"));
    println!(
        "[macos-zip] {}",
        archive.file_name().unwrap_or_default().to_string_lossy()
    );
    write_app_zip(&app, &archive)?;
    Ok(())
}

/// Where cargo-bundle writes `Comical.app`.
///
/// Two `--target` flags make cargo-bundle build both slices and combine them
/// under `target/universal`; a single target stays under `target/release`.
fn bundle_app_path(root: &Path, universal: bool) -> PathBuf {
    if universal {
        root.join("target/universal/release/bundle/osx/Comical.app")
    } else {
        root.join("target/release/bundle/osx/Comical.app")
    }
}

/// Adds the comic associations to the bundle's `Info.plist`.
///
/// cargo-bundle is configured to append the fragment itself, but the bundle it
/// built in CI carried no `CFBundleDocumentTypes` and only the release check
/// noticed. Merging here is deterministic and unit tested, and the association
/// is the difference between a reader and a reader that opens its own files.
fn add_document_types(root: &Path, app: &Path) -> Result<(), Box<dyn Error>> {
    let plist_path = app.join("Contents/Info.plist");
    let plist = fs::read_to_string(&plist_path)
        .map_err(|error| format!("macos: {}: {error}", plist_path.display()))?;
    if plist.contains("CFBundleDocumentTypes") {
        return Ok(());
    }
    let fragment = fs::read_to_string(root.join(DOCUMENT_TYPES))?;
    let merged =
        insert_into_plist(&plist, &fragment).ok_or("macos: Info.plist has no top-level </dict>")?;
    fs::write(&plist_path, merged)?;
    println!("[macos-plist] CFBundleDocumentTypes");
    Ok(())
}

/// Puts `fragment` last inside the plist's top-level dictionary.
fn insert_into_plist(plist: &str, fragment: &str) -> Option<String> {
    let index = plist.rfind("</dict>")?;
    let mut merged = String::with_capacity(plist.len() + fragment.len());
    merged.push_str(&plist[..index]);
    merged.push_str(fragment);
    merged.push('\n');
    merged.push_str(&plist[index..]);
    Some(merged)
}

/// Zips a `.app` bundle with its paths relative to the bundle directory.
fn write_app_zip(app: &Path, destination: &Path) -> Result<(), Box<dyn Error>> {
    let root = app
        .file_name()
        .ok_or_else(|| format!("macos-zip: {} has no directory name", app.display()))?;
    let mut writer = ZapWriter::new(fs::File::create(destination)?);
    zip_directory(&mut writer, app, Path::new(root))?;
    writer.finish()?;
    Ok(())
}

/// Adds one bundle directory and everything under it.
///
/// Entries are sorted so two runs of the same build produce the same order,
/// and the executable bit is carried over: without it the app will not launch.
fn zip_directory(
    writer: &mut ZapWriter<fs::File>,
    directory: &Path,
    prefix: &Path,
) -> Result<(), Box<dyn Error>> {
    let mut entries = fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();

    for path in entries {
        let name = prefix.join(path.file_name().unwrap_or_default());
        let name = name.to_string_lossy().into_owned();
        if path.is_dir() {
            let child = name.clone();
            writer.add_directory(name, ZipFileOptions::default())?;
            zip_directory(writer, &path, Path::new(&child))?;
        } else {
            let permissions = if is_executable(&path) { 0o755 } else { 0o644 };
            let options = ZipFileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .unix_permissions(permissions);
            writer.start_file(name, options)?;
            let mut source = fs::File::open(&path)?;
            std::io::copy(&mut source, writer)?;
        }
    }
    Ok(())
}

/// Whether the file carries an executable bit. Only macOS builds run this, but
/// the module also compiles on Windows, where the mode does not exist.
fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
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

pub(crate) fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "release: xtask has no workspace parent".into())
}

/// Reads `[workspace.package] version`, the only place the version exists.
pub(crate) fn workspace_version(root: &Path) -> Result<String, Box<dyn Error>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_version_matches_the_packaged_crate() {
        let root = workspace_root().expect("workspace root");
        let version = workspace_version(&root).expect("version");
        assert_eq!(version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn version_validation_accepts_three_numeric_parts() {
        for good in ["0.2.0", "1.0.0", "10.20.30"] {
            assert!(is_version(good), "{good} should be accepted");
        }
        for bad in ["1.0", "1.0.0.0", "1.0.x", "v1.0.0", "", "1..0", " 1.0.0"] {
            assert!(!is_version(bad), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn tarball_preserves_content_and_normalizes_ownership() {
        let directory = tempfile::tempdir().expect("temp dir");
        let source = directory.path().join("comical");
        fs::write(&source, b"binary payload").expect("write source");
        let archive = directory.path().join("comical.tar.gz");

        write_tarball(&source, "comical", &archive).expect("write tarball");

        let file = fs::File::open(&archive).expect("open tarball");
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        let mut entries = archive.entries().expect("entries");
        let mut entry = entries.next().expect("one entry").expect("entry");

        assert_eq!(entry.path().expect("path").to_string_lossy(), "comical");
        assert_eq!(entry.header().mode().expect("mode"), 0o755);
        assert_eq!(entry.header().uid().expect("uid"), 0);
        assert_eq!(entry.header().gid().expect("gid"), 0);

        let mut content = Vec::new();
        entry.read_to_end(&mut content).expect("read member");
        assert_eq!(content, b"binary payload");
        assert!(entries.next().is_none(), "tarball must hold one member");
    }

    #[test]
    fn checksums_cover_every_artifact_in_sorted_order() {
        let directory = tempfile::tempdir().expect("dist dir");
        fs::write(directory.path().join("b.bin"), b"beta").expect("write b");
        fs::write(directory.path().join("a.bin"), b"alpha").expect("write a");

        checksums(directory.path()).expect("checksums");

        let text = fs::read_to_string(directory.path().join("SHA256SUMS.txt")).expect("read sums");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(lines[0].ends_with("  ./a.bin"), "{}", lines[0]);
        assert!(lines[1].ends_with("  ./b.bin"), "{}", lines[1]);
        assert_eq!(
            lines[0],
            format!(
                "{}\u{20}\u{20}./a.bin",
                sha256(&directory.path().join("a.bin")).expect("hash")
            )
        );
    }

    #[test]
    fn wix_arguments_point_candle_at_the_built_binary() {
        let exe = Path::new("/tmp/dist/comical-windows-x86_64.exe");
        let output = Path::new("/tmp/dist/comical-1.2.3-x86_64.msi");
        let args = wix_args(exe, output);

        assert_eq!(args.first().map(String::as_str), Some("wix"));
        let separator = args.iter().position(|arg| arg == "--").expect("-- gap");
        assert_eq!(
            args.last().map(String::as_str),
            Some("-dBinaryPath=/tmp/dist/comical-windows-x86_64.exe")
        );
        assert_eq!(separator + 1, args.len() - 1, "-d must follow --");
        assert!(args.contains(&output.to_string_lossy().into_owned()));
        assert!(args.contains(&"--no-build".to_owned()));
    }

    /// The MSI must not assume one toolchain's output path again.
    #[test]
    fn wix_source_is_a_variable_not_a_fixed_path() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/comical-app/wix/main.wxs");
        let wxs = fs::read_to_string(&path).expect("read main.wxs");
        assert!(wxs.contains("Source='$(var.BinaryPath)'"), "{wxs}");
        assert!(!wxs.contains("x86_64-pc-windows-msvc/release"), "{wxs}");
        // wixl (msitools) rejects WiX preprocessor instructions, and the build
        // tool now always supplies the variable, so no `<?define?>` is needed.
        assert!(!wxs.contains("<?define"), "{wxs}");
        assert!(!wxs.contains("<?ifndef"), "{wxs}");
    }

    /// A universal build lands under `target/universal`, so reading the app
    /// from `target/release` fails only when both targets are installed.
    #[test]
    fn bundle_path_follows_the_universal_build() {
        let root = Path::new("/w");
        assert_eq!(
            bundle_app_path(root, true),
            Path::new("/w/target/universal/release/bundle/osx/Comical.app")
        );
        assert_eq!(
            bundle_app_path(root, false),
            Path::new("/w/target/release/bundle/osx/Comical.app")
        );
    }

    #[test]
    fn document_types_land_inside_the_bundle_dictionary() {
        let plist = "<?xml version=\"1.0\"?>\n<plist version=\"1.0\">\n<dict>\n \
            <key>CFBundleName</key>\n <string>Comical</string>\n</dict>\n</plist>\n";
        let merged = insert_into_plist(plist, "<key>CFBundleDocumentTypes</key>").expect("dict");

        let key = merged.find("CFBundleDocumentTypes").expect("key");
        let close = merged.rfind("</dict>").expect("closing tag");
        assert!(key < close, "the association must be inside the dictionary");
        assert!(merged.ends_with("</plist>\n"), "{merged}");
        assert_eq!(
            merged.matches("</dict>").count(),
            plist.matches("</dict>").count(),
            "no dictionary may be opened or closed twice"
        );
    }

    #[test]
    fn a_plist_without_a_dictionary_is_reported() {
        assert!(insert_into_plist("<plist/>", "<key>x</key>").is_none());
    }

    #[test]
    fn app_zip_keeps_bundle_paths_and_the_executable_bit() {
        let directory = tempfile::tempdir().expect("dir");
        let app = directory.path().join("Comical.app");
        let contents = app.join("Contents");
        fs::create_dir_all(contents.join("MacOS")).expect("create bundle");
        fs::write(contents.join("Info.plist"), b"<plist/>").expect("write plist");
        let executable = contents.join("MacOS").join(BINARY);
        fs::write(&executable, b"binary").expect("write executable");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o755))
                .expect("make executable");
        }

        let archive = directory.path().join("app.zip");
        write_app_zip(&app, &archive).expect("zip the bundle");

        let mut zip =
            zip::ZipArchive::new(fs::File::open(&archive).expect("open zip")).expect("read zip");
        let mut names = Vec::new();
        for index in 0..zip.len() {
            let entry = zip.by_index(index).expect("entry");
            names.push(entry.name().expect("member name").into_owned());
        }
        assert!(
            names.contains(&"Comical.app/Contents/Info.plist".to_owned()),
            "{names:?}"
        );
        assert!(
            names.contains(&"Comical.app/Contents/MacOS/comical".to_owned()),
            "{names:?}"
        );

        let entry = zip
            .by_name(&format!("Comical.app/Contents/MacOS/{BINARY}"))
            .expect("executable entry");
        #[cfg(unix)]
        assert_eq!(entry.unix_mode().map(|mode| mode & 0o777), Some(0o755));
    }

    #[test]
    fn wixl_arguments_carry_arch_ext_and_both_variables() {
        let source = Path::new("/repo/crates/comical-app/wix/main.wxs");
        let exe = Path::new("/repo/dist/comical-windows-x86_64.exe");
        let output = Path::new("/repo/dist/comical-1.2.3-x86_64.msi");
        let args = wixl_args(source, exe, output, "1.2.3");

        assert!(args.contains(&"--arch".to_owned()));
        assert!(args.contains(&"x64".to_owned()));
        assert!(args.contains(&"--ext".to_owned()));
        assert!(args.contains(&"ui".to_owned()));
        assert!(args.contains(&"Version=1.2.3".to_owned()));
        assert!(args.contains(&"BinaryPath=/repo/dist/comical-windows-x86_64.exe".to_owned()));
        assert!(args.contains(&"/repo/dist/comical-1.2.3-x86_64.msi".to_owned()));
        // wixl is run from the source directory, so it gets the bare file name.
        assert_eq!(args.last().map(String::as_str), Some("main.wxs"));
    }

    #[test]
    fn wix_source_and_license_exist() {
        let source = wix_source().expect("wix source");
        let license = source.parent().expect("wix directory").join("License.rtf");
        assert!(license.is_file(), "WixUI_Minimal needs License.rtf");
    }

    #[test]
    fn msi_reports_a_missing_binary_instead_of_running_wix() {
        if !tool::command_exists("cargo-wix") {
            // The guard is only reachable where cargo-wix exists.
            return;
        }
        let directory = tempfile::tempdir().expect("dist dir");
        let error = msi(directory.path(), "1.2.3").expect_err("missing binary must fail");
        assert!(error.to_string().contains("does not exist"), "{error}");
    }

    #[test]
    fn checksums_ignore_their_own_output() {
        let directory = tempfile::tempdir().expect("dist dir");
        fs::write(directory.path().join("only.bin"), b"payload").expect("write");
        checksums(directory.path()).expect("first pass");
        checksums(directory.path()).expect("second pass");
        let text = fs::read_to_string(directory.path().join("SHA256SUMS.txt")).expect("read sums");
        assert_eq!(text.lines().count(), 1, "{text}");
    }
}
