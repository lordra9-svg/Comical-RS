//! Verifies a finished release directory before it is published.
//!
//! Packaging mistakes are expensive because they surface only after a release
//! is public: a tarball with the wrong member, a checksum file that omits an
//! artifact, or a Windows executable that quietly lost its icon, manifest or
//! DX12 backend. This command reads the directory the release run produced and
//! checks the artifacts themselves, without running them.
//!
//! It cannot replace a run on each target platform; it catches the failures
//! that the files alone can prove.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::msi;
use crate::release;

/// Machine type for x86-64 in a PE header.
const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;
/// `IMAGE_SUBSYSTEM_WINDOWS_GUI`.
const IMAGE_SUBSYSTEM_WINDOWS_GUI: u16 = 2;
/// Imports the Windows executable must keep: the OS, the window layer and the
/// DX12 shader compiler that only arrives with wgpu's `dx12` backend.
const REQUIRED_IMPORTS: [&str; 3] = ["KERNEL32.dll", "USER32.dll", "d3dcompiler_47.dll"];
/// The two paths inside the zipped macOS bundle that packaging depends on.
const MACOS_PLIST: &str = "Comical.app/Contents/Info.plist";
const MACOS_EXECUTABLE: &str = "Comical.app/Contents/MacOS/comical";
/// Present only when the bundle was signed; reported, not required, because a
/// local run on a machine without a Developer ID certificate is legitimate.
const MACOS_SIGNATURE: &str = "Comical.app/Contents/_CodeSignature/CodeResources";
/// Installer strings that prove the associations and the shortcut survived.
/// The registry component is the one a careless edit to the WiX source drops.
const REQUIRED_MSI_STRINGS: [&str; 10] = [
    "comical.exe",
    "Comical.CBZ",
    "Comical.CBR",
    "Comical.CB7",
    ".cbz",
    ".cbr",
    ".cb7",
    "[#ComicalEXE],0",
    "\"[#ComicalEXE]\" \"%1\"",
    "StartMenuComical",
];

/// Verifies `directory`, or the current version's release directory.
pub fn run(directory: Option<String>) -> Result<(), Box<dyn Error>> {
    let directory = match directory {
        Some(path) => PathBuf::from(path),
        None => {
            let root = release::workspace_root()?;
            let version = release::workspace_version(&root)?;
            root.join("target/release-dist").join(version)
        }
    };
    if !directory.is_dir() {
        return Err(format!("verify: {} is not a directory", directory.display()).into());
    }

    println!("Verifying {}", directory.display());
    let mut report = Report::default();
    report.check(check_checksums(&directory));

    let linux_binary = directory.join("comical-linux-x86_64");
    let mut entries = fs::read_dir(&directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();

    for path in entries.iter().filter(|path| path.is_file()) {
        let name = file_name(path);
        if name == "SHA256SUMS.txt" {
            continue;
        }
        let size = fs::metadata(path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        if size == 0 {
            report.check(Err(format!("{name} is empty")));
            continue;
        }
        if name.ends_with(".tar.gz") {
            let reference = linux_binary.is_file().then_some(linux_binary.as_path());
            report.check(check_tarball(path, "comical", reference));
        } else if name.ends_with(".exe") {
            report.check(check_windows_exe(path));
        } else if name.ends_with(".deb") {
            report.check(check_deb(path));
        } else if name.ends_with(".msi") {
            report.check(check_msi(path));
        } else if name.ends_with(".zip") {
            report.check(check_macos_zip(path));
        } else {
            println!("  ok      {name} ({size} bytes)");
        }
    }

    report.finish()
}

#[derive(Default)]
struct Report {
    failures: Vec<String>,
}

impl Report {
    /// Records one check. `Ok` carries the summary to print on success.
    fn check(&mut self, result: Result<String, String>) {
        match result {
            Ok(summary) => println!("  ok      {summary}"),
            Err(message) => {
                println!("  FAILED  {message}");
                self.failures.push(message);
            }
        }
    }

    fn finish(self) -> Result<(), Box<dyn Error>> {
        if self.failures.is_empty() {
            println!("All artifacts verified.");
            Ok(())
        } else {
            Err(format!(
                "verify: {} check(s) failed:\n  {}",
                self.failures.len(),
                self.failures.join("\n  ")
            )
            .into())
        }
    }
}

/// Checks every file in `SHA256SUMS.txt` and that nothing else is present.
fn check_checksums(directory: &Path) -> Result<String, String> {
    let text = fs::read_to_string(directory.join("SHA256SUMS.txt"))
        .map_err(|error| format!("SHA256SUMS.txt: {error}"))?;

    let mut listed = BTreeSet::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (digest, name) = line
            .split_once("  ")
            .ok_or_else(|| format!("SHA256SUMS.txt line {}: not '<hash>  <name>'", index + 1))?;
        let name = name.strip_prefix("./").unwrap_or(name);
        listed.insert(name.to_owned());
        let path = directory.join(name);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            // Reported by the coverage check below, which names the problem.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("{name}: {error}")),
        };
        let actual = sha256(&bytes);
        if actual != digest {
            return Err(format!(
                "{name}: checksum is {actual}, manifest says {digest}"
            ));
        }
    }

    if listed.is_empty() {
        return Err("SHA256SUMS.txt lists no files".to_owned());
    }

    let present: BTreeSet<String> = fs::read_dir(directory)
        .map_err(|error| format!("{directory:?}: {error}"))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name != "SHA256SUMS.txt")
        .collect();

    let missing: Vec<&String> = listed.difference(&present).collect();
    if !missing.is_empty() {
        return Err(format!("listed but not on disk: {missing:?}"));
    }
    let extra: Vec<&String> = present.difference(&listed).collect();
    if !extra.is_empty() {
        return Err(format!("not listed in SHA256SUMS.txt: {extra:?}"));
    }

    Ok(format!("SHA256SUMS.txt covers {} artifacts", listed.len()))
}

/// Checks the Linux tarball: one member, executable mode, neutral ownership and
/// bytes identical to the loose binary when that binary is present.
fn check_tarball(path: &Path, member: &str, reference: Option<&Path>) -> Result<String, String> {
    let name = file_name(path);
    let file = fs::File::open(path).map_err(|error| format!("{name}: {error}"))?;
    let mut archive = tar::Archive::new(GzDecoder::new(file));
    let mut entries = archive
        .entries()
        .map_err(|error| format!("{name}: {error}"))?;
    let mut entry = entries
        .next()
        .ok_or_else(|| format!("{name}: archive is empty"))?
        .map_err(|error| format!("{name}: {error}"))?;

    let found = entry
        .path()
        .map_err(|error| format!("{name}: {error}"))?
        .to_string_lossy()
        .into_owned();
    if found != member {
        return Err(format!("{name}: member is {found:?}, expected {member:?}"));
    }

    let mode = entry.header().mode().unwrap_or(0);
    if mode & 0o777 != 0o755 {
        return Err(format!("{name}: member mode is {mode:o}, expected 755"));
    }
    let uid = entry.header().uid().unwrap_or(u64::MAX);
    let gid = entry.header().gid().unwrap_or(u64::MAX);
    if uid != 0 || gid != 0 {
        return Err(format!("{name}: member owner is {uid}/{gid}, expected 0/0"));
    }

    let mut bytes = Vec::new();
    entry
        .read_to_end(&mut bytes)
        .map_err(|error| format!("{name}: {error}"))?;
    if bytes.is_empty() {
        return Err(format!("{name}: member is empty"));
    }
    if entries.next().is_some() {
        return Err(format!("{name}: holds more than one member"));
    }

    if let Some(reference) = reference {
        let loose = fs::read(reference).map_err(|error| format!("{name}: {error}"))?;
        if sha256(&loose) != sha256(&bytes) {
            return Err(format!(
                "{name}: member differs from {}",
                file_name(reference)
            ));
        }
    }

    Ok(format!(
        "{name}: {member}, {} bytes, mode 755, owner 0/0",
        bytes.len()
    ))
}

/// Checks the macOS artifact: the zipped bundle holds an `Info.plist` naming
/// the executable and the comic associations, and that executable kept a mode
/// that can run.
fn check_macos_zip(path: &Path) -> Result<String, String> {
    let name = file_name(path);
    let file = fs::File::open(path).map_err(|error| format!("{name}: {error}"))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("{name}: not a zip archive: {error}"))?;

    let mut plist = None;
    let mut mode = None;
    let mut signed = false;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("{name}: {error}"))?;
        let member = entry
            .name()
            .map_err(|error| format!("{name}: {error}"))?
            .into_owned();
        // A release asset is unpacked by users, so a member that escapes the
        // bundle directory is a reason to stop, not to tidy up.
        if member.starts_with('/') || member.split('/').any(|part| part == "..") {
            return Err(format!("{name}: unsafe member {member:?}"));
        }
        if member == MACOS_PLIST {
            let mut text = String::new();
            entry
                .read_to_string(&mut text)
                .map_err(|error| format!("{name}: {member}: {error}"))?;
            plist = Some(text);
        } else if member == MACOS_EXECUTABLE {
            mode = Some(entry.unix_mode().unwrap_or(0));
        } else if member == MACOS_SIGNATURE {
            signed = true;
        }
    }

    let plist = plist.ok_or_else(|| format!("{name}: no {MACOS_PLIST}"))?;
    let mode = mode.ok_or_else(|| format!("{name}: no {MACOS_EXECUTABLE}"))?;
    if mode & 0o111 == 0 {
        return Err(format!(
            "{name}: {MACOS_EXECUTABLE} mode is {mode:o}, so the app cannot launch"
        ));
    }
    if !plist.contains("<string>comical</string>") {
        return Err(format!(
            "{name}: Info.plist does not name comical as CFBundleExecutable"
        ));
    }
    for value in [
        "CFBundleIdentifier",
        "CFBundleName",
        "CFBundleShortVersionString",
    ] {
        if !plist.contains(value) {
            return Err(format!("{name}: Info.plist omits {value}"));
        }
    }
    for extension in ["cbz", "cbr", "cb7"] {
        if !plist.contains(extension) {
            return Err(format!(
                "{name}: Info.plist omits the {extension} association"
            ));
        }
    }

    Ok(format!(
        "{name}: Comical.app, mode {mode:o}, document types and version present, {}",
        if signed { "signed" } else { "unsigned" }
    ))
}

/// Checks the Windows executable's PE headers, resources and imports.
fn check_windows_exe(path: &Path) -> Result<String, String> {
    let name = file_name(path);
    let bytes = fs::read(path).map_err(|error| format!("{name}: {error}"))?;
    let info = pe_info(&bytes).map_err(|error| format!("{name}: {error}"))?;

    if info.machine != IMAGE_FILE_MACHINE_AMD64 {
        return Err(format!(
            "{name}: machine 0x{:04x}, expected x86-64",
            info.machine
        ));
    }
    if info.subsystem != IMAGE_SUBSYSTEM_WINDOWS_GUI {
        return Err(format!(
            "{name}: subsystem {}, expected {IMAGE_SUBSYSTEM_WINDOWS_GUI} (Windows GUI)",
            info.subsystem
        ));
    }
    if !info.has_resources {
        return Err(format!(
            "{name}: no .rsrc section, so no icon, version info or manifest"
        ));
    }
    for required in REQUIRED_IMPORTS {
        if !info
            .imports
            .iter()
            .any(|import| import.eq_ignore_ascii_case(required))
        {
            return Err(format!("{name}: does not import {required}"));
        }
    }

    Ok(format!(
        "{name}: x86-64 Windows GUI, {} imports, resources present",
        info.imports.len()
    ))
}

/// Checks the Debian package is an `ar` archive holding the expected members.
fn check_deb(path: &Path) -> Result<String, String> {
    let name = file_name(path);
    let bytes = fs::read(path).map_err(|error| format!("{name}: {error}"))?;
    if !bytes.starts_with(b"!<arch>\n") {
        return Err(format!("{name}: not an ar archive"));
    }
    for member in ["debian-binary", "control.tar", "data.tar"] {
        if !bytes
            .windows(member.len())
            .any(|window| window == member.as_bytes())
        {
            return Err(format!("{name}: missing {member}"));
        }
    }
    Ok(format!("{name}: {} bytes, ar members present", bytes.len()))
}

/// Checks the MSI holds the installer this project ships: the executable of
/// this run, the `.cbz`/`.cbr`/`.cb7` associations and the Start Menu
/// shortcut, at the version in the artifact's file name.
fn check_msi(path: &Path) -> Result<String, String> {
    let name = file_name(path);
    let bytes = fs::read(path).map_err(|error| format!("{name}: {error}"))?;
    let package = msi::read(&bytes).map_err(|error| format!("{name}: {error}"))?;

    if !package
        .streams
        .iter()
        .any(|stream| stream == "\u{5}SummaryInformation")
    {
        return Err(format!("{name}: no SummaryInformation stream"));
    }

    // The table streams are uncompressed, so a dropped component shows up as a
    // missing string even though the stream names are obfuscated.
    for required in REQUIRED_MSI_STRINGS {
        if !holds(&package.tables, required.as_bytes()) {
            return Err(format!(
                "{name}: installer tables omit {required:?}, so that component was dropped"
            ));
        }
    }

    let version = msi_version(&name)
        .ok_or_else(|| format!("{name}: expected comical-<version>-x86_64.msi"))?;
    if !holds(&package.tables, version.as_bytes()) {
        return Err(format!(
            "{name}: installer does not carry product version {version}"
        ));
    }

    // The payload must be the executable this same run produced, not one left
    // over from an earlier build.
    let shipped = path
        .parent()
        .map(|directory| directory.join("comical-windows-x86_64.exe"))
        .filter(|executable| executable.is_file());
    if let Some(executable) = shipped {
        let expected = fs::metadata(&executable)
            .map_err(|error| format!("{name}: {error}"))?
            .len();
        if !package
            .payload
            .iter()
            .any(|file| u64::from(file.size) == expected)
        {
            return Err(format!(
                "{name}: embedded payload is not the {expected}-byte executable beside it"
            ));
        }
    }

    let payload: Vec<String> = package
        .payload
        .iter()
        .map(|file| format!("{} ({} bytes)", file.name, file.size))
        .collect();
    Ok(format!(
        "{name}: {} bytes, {} streams, payload {}, version {version}, associations and shortcut present",
        bytes.len(),
        package.streams.len(),
        payload.join(" + ")
    ))
}

fn holds(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// Reads the version out of `comical-1.2.3-x86_64.msi`.
fn msi_version(name: &str) -> Option<String> {
    let rest = name.strip_prefix("comical-")?;
    let version = rest.strip_suffix("-x86_64.msi")?;
    let numeric = !version.is_empty()
        && version
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.');
    numeric.then(|| version.to_owned())
}

/// The fields of a PE image that packaging cares about.
#[derive(Debug)]
struct PeInfo {
    machine: u16,
    subsystem: u16,
    has_resources: bool,
    imports: Vec<String>,
}

struct Section {
    name: String,
    virtual_address: u32,
    virtual_size: u32,
    raw_pointer: u32,
    raw_size: u32,
}

/// Reads the PE headers, section table and import table.
fn pe_info(bytes: &[u8]) -> Result<PeInfo, String> {
    if bytes.len() < 0x40 || !bytes.starts_with(b"MZ") {
        return Err("not a PE image: no MZ header".to_owned());
    }
    let pe = read_u32(bytes, 0x3c)? as usize;
    if bytes.get(pe..pe + 4) != Some(b"PE\0\0".as_slice()) {
        return Err("not a PE image: no PE signature".to_owned());
    }

    let machine = read_u16(bytes, pe + 4)?;
    let section_count = read_u16(bytes, pe + 6)? as usize;
    let optional_size = read_u16(bytes, pe + 20)? as usize;
    let optional = pe + 24;
    let magic = read_u16(bytes, optional)?;
    let subsystem = read_u16(bytes, optional + 68)?;
    let directories = match magic {
        0x20b => optional + 112,
        0x10b => optional + 96,
        other => return Err(format!("unknown optional header magic 0x{other:04x}")),
    };
    let import_rva = read_u32(bytes, directories + 8)?;

    let table = optional + optional_size;
    let mut sections = Vec::with_capacity(section_count);
    for index in 0..section_count {
        let base = table + index * 40;
        let name_bytes = bytes
            .get(base..base + 8)
            .ok_or_else(|| "truncated section table".to_owned())?;
        sections.push(Section {
            name: String::from_utf8_lossy(name_bytes)
                .trim_end_matches('\0')
                .to_owned(),
            virtual_size: read_u32(bytes, base + 8)?,
            virtual_address: read_u32(bytes, base + 12)?,
            raw_size: read_u32(bytes, base + 16)?,
            raw_pointer: read_u32(bytes, base + 20)?,
        });
    }
    let has_resources = sections.iter().any(|section| section.name == ".rsrc");

    let mut imports = Vec::new();
    if import_rva != 0 {
        let mut offset = map_rva(&sections, import_rva)?;
        // A descriptor is 20 bytes; the table ends with an all-zero entry.
        for _ in 0..512 {
            let name_rva = read_u32(bytes, offset + 12)?;
            if name_rva == 0 {
                break;
            }
            let name_offset = map_rva(&sections, name_rva)?;
            imports.push(read_c_string(bytes, name_offset)?);
            offset += 20;
        }
    }

    Ok(PeInfo {
        machine,
        subsystem,
        has_resources,
        imports,
    })
}

/// Translates a relative virtual address to a file offset.
fn map_rva(sections: &[Section], rva: u32) -> Result<usize, String> {
    for section in sections {
        let extent = section.virtual_size.max(section.raw_size);
        if rva >= section.virtual_address && rva < section.virtual_address + extent {
            let delta = rva - section.virtual_address;
            return Ok((section.raw_pointer + delta) as usize);
        }
    }
    Err(format!("rva 0x{rva:x} is outside every section"))
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let slice = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| format!("truncated header at 0x{offset:x}"))?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("truncated header at 0x{offset:x}"))?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_c_string(bytes: &[u8], offset: usize) -> Result<String, String> {
    let rest = bytes
        .get(offset..)
        .ok_or_else(|| format!("string at 0x{offset:x} is outside the file"))?;
    let end = rest
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| format!("string at 0x{offset:x} is not terminated"))?;
    Ok(String::from_utf8_lossy(&rest[..end]).into_owned())
}

fn sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;

    fn write_checksums(directory: &Path, files: &[&str]) {
        let mut text = String::new();
        for name in files {
            let bytes = fs::read(directory.join(name)).expect("read artifact");
            text.push_str(&format!("{}  ./{name}\n", sha256(&bytes)));
        }
        fs::write(directory.join("SHA256SUMS.txt"), text).expect("write sums");
    }

    #[test]
    fn checksum_check_accepts_matching_files() {
        let directory = tempfile::tempdir().expect("dir");
        fs::write(directory.path().join("a.bin"), b"alpha").expect("write");
        fs::write(directory.path().join("b.bin"), b"beta").expect("write");
        write_checksums(directory.path(), &["a.bin", "b.bin"]);

        let summary = check_checksums(directory.path()).expect("checksums");
        assert!(summary.contains("2 artifacts"), "{summary}");
    }

    #[test]
    fn checksum_check_rejects_a_tampered_file() {
        let directory = tempfile::tempdir().expect("dir");
        fs::write(directory.path().join("a.bin"), b"alpha").expect("write");
        write_checksums(directory.path(), &["a.bin"]);
        fs::write(directory.path().join("a.bin"), b"tampered").expect("rewrite");

        let error = check_checksums(directory.path()).expect_err("must fail");
        assert!(error.contains("checksum is"), "{error}");
    }

    #[test]
    fn checksum_check_rejects_missing_and_unlisted_files() {
        let directory = tempfile::tempdir().expect("dir");
        fs::write(directory.path().join("a.bin"), b"alpha").expect("write");
        write_checksums(directory.path(), &["a.bin"]);

        fs::write(directory.path().join("stray.bin"), b"stray").expect("write");
        let error = check_checksums(directory.path()).expect_err("extra file");
        assert!(error.contains("not listed"), "{error}");

        fs::remove_file(directory.path().join("stray.bin")).expect("remove");
        fs::remove_file(directory.path().join("a.bin")).expect("remove");
        let error = check_checksums(directory.path()).expect_err("missing file");
        assert!(error.contains("listed but not on disk"), "{error}");
    }

    #[test]
    fn checksum_check_requires_the_manifest() {
        let directory = tempfile::tempdir().expect("dir");
        let error = check_checksums(directory.path()).expect_err("no manifest");
        assert!(error.contains("SHA256SUMS.txt"), "{error}");
    }

    fn write_tarball(directory: &Path, name: &str, mode: u32, uid: u64, payload: &[u8]) -> PathBuf {
        let path = directory.join(name);
        let file = fs::File::create(&path).expect("create tarball");
        let encoder = GzEncoder::new(file, Compression::fast());
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_size(payload.len() as u64);
        header.set_mode(mode);
        header.set_uid(uid);
        header.set_gid(0);
        header.set_cksum();
        builder
            .append_data(&mut header, "comical", payload)
            .expect("append");
        builder.into_inner().expect("finish").finish().expect("gz");
        path
    }

    #[test]
    fn tarball_check_accepts_the_release_layout() {
        let directory = tempfile::tempdir().expect("dir");
        let path = write_tarball(directory.path(), "comical.tar.gz", 0o755, 0, b"binary");

        let summary = check_tarball(&path, "comical", None).expect("tarball");
        assert!(summary.contains("6 bytes"), "{summary}");
    }

    #[test]
    fn tarball_check_rejects_wrong_mode_and_owner() {
        let directory = tempfile::tempdir().expect("dir");
        let read_only = write_tarball(directory.path(), "readonly.tar.gz", 0o644, 0, b"binary");
        let error = check_tarball(&read_only, "comical", None).expect_err("mode");
        assert!(error.contains("mode"), "{error}");

        let owned = write_tarball(directory.path(), "owned.tar.gz", 0o755, 1000, b"binary");
        let error = check_tarball(&owned, "comical", None).expect_err("owner");
        assert!(error.contains("owner"), "{error}");
    }

    #[test]
    fn tarball_check_compares_against_the_loose_binary() {
        let directory = tempfile::tempdir().expect("dir");
        let path = write_tarball(directory.path(), "comical.tar.gz", 0o755, 0, b"binary");
        let loose = directory.path().join("comical-linux-x86_64");
        fs::write(&loose, b"binary").expect("write loose");
        check_tarball(&path, "comical", Some(&loose)).expect("matching binary");

        fs::write(&loose, b"different").expect("rewrite loose");
        let error = check_tarball(&path, "comical", Some(&loose)).expect_err("mismatch");
        assert!(error.contains("differs"), "{error}");
    }

    /// A zipped bundle holding the two entries the macOS check reads.
    fn write_macos_zip(directory: &Path, mode: u32, plist: &str) -> PathBuf {
        use std::io::Write;

        use zip::ZipWriter;
        use zip::write::SimpleFileOptions;

        let path = directory.join("comical-1.2.3-macos-universal.zip");
        let file = fs::File::create(&path).expect("create zip");
        let mut writer = ZipWriter::new(file);
        writer
            .start_file(MACOS_PLIST, SimpleFileOptions::default())
            .expect("plist entry");
        writer.write_all(plist.as_bytes()).expect("write plist");
        writer
            .start_file(
                MACOS_EXECUTABLE,
                SimpleFileOptions::default().unix_permissions(mode),
            )
            .expect("executable entry");
        writer.write_all(b"binary").expect("write executable");
        writer.finish().expect("finish zip");
        path
    }

    /// The keys `cargo bundle` writes plus the associations added by the plist
    /// extension in `crates/comical-app/assets`.
    const BUNDLE_PLIST: &str = "<key>CFBundleIdentifier</key>\
<string>io.github.lordra9-svg.comical</string>\
<key>CFBundleName</key><string>Comical</string>\
<key>CFBundleExecutable</key><string>comical</string>\
<key>CFBundleShortVersionString</key><string>1.2.3</string>\
<key>CFBundleTypeExtensions</key>\
<array><string>cbz</string><string>cbr</string><string>cb7</string></array>";

    #[test]
    fn macos_check_accepts_the_zipped_bundle() {
        let directory = tempfile::tempdir().expect("dir");
        let path = write_macos_zip(directory.path(), 0o755, BUNDLE_PLIST);

        let summary = check_macos_zip(&path).expect("bundle");
        assert!(summary.contains("Comical.app"), "{summary}");
    }

    #[test]
    fn macos_check_rejects_an_unlaunchable_executable() {
        let directory = tempfile::tempdir().expect("dir");
        let path = write_macos_zip(directory.path(), 0o644, BUNDLE_PLIST);

        let error = check_macos_zip(&path).expect_err("mode");
        assert!(error.contains("cannot launch"), "{error}");
    }

    #[test]
    fn macos_check_requires_the_document_types() {
        let directory = tempfile::tempdir().expect("dir");
        let plist = BUNDLE_PLIST.replace("<string>cb7</string>", "");
        let path = write_macos_zip(directory.path(), 0o755, &plist);

        let error = check_macos_zip(&path).expect_err("missing association");
        assert!(error.contains("cb7"), "{error}");
    }

    /// A minimal PE image: x86-64, Windows GUI, a `.rsrc` section and one
    /// import descriptor naming KERNEL32.dll.
    fn synthetic_pe() -> Vec<u8> {
        let mut bytes = vec![0u8; 0x800];
        bytes[..2].copy_from_slice(b"MZ");
        let pe = 0x80usize;
        bytes[0x3c..0x40].copy_from_slice(&(pe as u32).to_le_bytes());
        bytes[pe..pe + 4].copy_from_slice(b"PE\0\0");

        let coff = pe + 4;
        bytes[coff..coff + 2].copy_from_slice(&IMAGE_FILE_MACHINE_AMD64.to_le_bytes());
        bytes[coff + 2..coff + 4].copy_from_slice(&2u16.to_le_bytes());
        bytes[coff + 16..coff + 18].copy_from_slice(&240u16.to_le_bytes());

        let optional = coff + 20;
        bytes[optional..optional + 2].copy_from_slice(&0x20bu16.to_le_bytes());
        bytes[optional + 68..optional + 70]
            .copy_from_slice(&IMAGE_SUBSYSTEM_WINDOWS_GUI.to_le_bytes());
        bytes[optional + 108..optional + 112].copy_from_slice(&16u32.to_le_bytes());
        // Data directory 1 (imports) starts at optional + 112.
        let import_directory = optional + 112 + 8;
        bytes[import_directory..import_directory + 4].copy_from_slice(&0x2000u32.to_le_bytes());

        let table = optional + 240;
        section(&mut bytes, table, ".text", 0x1000, 0x400);
        section(&mut bytes, table + 40, ".rsrc", 0x2000, 0x600);

        // Import descriptor at file offset 0x600 (rva 0x2000), name at 0x610.
        bytes[0x600 + 12..0x600 + 16].copy_from_slice(&0x2010u32.to_le_bytes());
        let import_name = b"KERNEL32.dll\0";
        bytes[0x610..0x610 + import_name.len()].copy_from_slice(import_name);
        bytes
    }

    fn section(bytes: &mut [u8], at: usize, name: &str, va: u32, raw: u32) {
        let mut padded = [0u8; 8];
        padded[..name.len()].copy_from_slice(name.as_bytes());
        bytes[at..at + 8].copy_from_slice(&padded);
        bytes[at + 8..at + 12].copy_from_slice(&0x200u32.to_le_bytes());
        bytes[at + 12..at + 16].copy_from_slice(&va.to_le_bytes());
        bytes[at + 16..at + 20].copy_from_slice(&0x200u32.to_le_bytes());
        bytes[at + 20..at + 24].copy_from_slice(&raw.to_le_bytes());
    }

    #[test]
    fn pe_reader_reports_headers_resources_and_imports() {
        let info = pe_info(&synthetic_pe()).expect("parse PE");
        assert_eq!(info.machine, IMAGE_FILE_MACHINE_AMD64);
        assert_eq!(info.subsystem, IMAGE_SUBSYSTEM_WINDOWS_GUI);
        assert!(info.has_resources);
        assert_eq!(info.imports, vec!["KERNEL32.dll".to_owned()]);
    }

    #[test]
    fn pe_reader_rejects_a_file_without_headers() {
        let error = pe_info(b"ELF or plain text").expect_err("must fail");
        assert!(error.contains("MZ"), "{error}");
    }

    #[test]
    fn windows_check_rejects_a_binary_missing_required_imports() {
        let directory = tempfile::tempdir().expect("dir");
        let path = directory.path().join("comical-windows-x86_64.exe");
        fs::write(&path, synthetic_pe()).expect("write exe");

        let error = check_windows_exe(&path).expect_err("missing imports");
        assert!(error.contains("USER32.dll"), "{error}");
    }

    #[test]
    fn msi_version_comes_from_the_file_name() {
        assert_eq!(
            msi_version("comical-1.2.3-x86_64.msi"),
            Some("1.2.3".to_owned())
        );
        assert_eq!(msi_version("comical-windows-x86_64.exe"), None);
        assert_eq!(msi_version("comical-1.2.3-x86_64.msi.exe"), None);
    }

    #[test]
    fn msi_check_rejects_a_file_that_is_not_an_installer() {
        let directory = tempfile::tempdir().expect("dir");
        let path = directory.path().join("comical-1.2.3-x86_64.msi");
        fs::write(&path, b"not an installer at all").expect("write msi");

        let error = check_msi(&path).expect_err("must fail");
        assert!(error.contains("comical-1.2.3-x86_64.msi"), "{error}");
    }

    #[test]
    fn windows_check_rejects_a_console_subsystem() {
        let mut bytes = synthetic_pe();
        let optional = 0x80 + 24;
        bytes[optional + 68..optional + 70].copy_from_slice(&3u16.to_le_bytes());
        let directory = tempfile::tempdir().expect("dir");
        let path = directory.path().join("comical-windows-x86_64.exe");
        fs::write(&path, bytes).expect("write exe");

        let error = check_windows_exe(&path).expect_err("console subsystem");
        assert!(error.contains("subsystem"), "{error}");
    }
}
