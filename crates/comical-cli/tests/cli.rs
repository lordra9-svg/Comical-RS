//! End-to-end tests for the command line binary.

use std::fs;
use std::process::{Command, Output};

use tempfile::TempDir;
use xtask::{
    FixtureEntry, ZipEncryption, ZipMethod, write_7z, write_corrupt, write_rar, write_zip,
};

const PAGE_A: &[u8] = b"\x89PNG\r\n\x1a\ncli-page-a";
const PAGE_B: &[u8] = b"\xff\xd8\xff\xe0cli-page-b";

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_comical-cli"))
}

fn run(arguments: &[&str]) -> Output {
    cli()
        .args(arguments)
        .output()
        .unwrap_or_else(|error| panic!("run comical-cli {arguments:?}: {error}"))
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn entry_lines(entries: &[(&str, &[u8])]) -> String {
    entries
        .iter()
        .enumerate()
        .map(|(index, (name, data))| format!("{index}\t{}\t{name}\n", data.len()))
        .collect()
}

#[test]
fn probe_and_extract_work_for_zip_rar_and_7z() {
    let directory = TempDir::new().unwrap();
    let entries = [("a.png", PAGE_A), ("b.jpg", PAGE_B)];

    let zip = directory.path().join("book.cbz");
    let rar = directory.path().join("book.cbr");
    let sevenz = directory.path().join("book.cb7");
    write_zip(
        &zip,
        &[
            FixtureEntry::new("a.png", PAGE_A),
            FixtureEntry::new("b.jpg", PAGE_B),
        ],
        ZipMethod::Store,
        ZipEncryption::None,
        None,
    )
    .unwrap();
    write_rar(
        &rar,
        &[
            FixtureEntry::new("a.png", PAGE_A),
            FixtureEntry::new("b.jpg", PAGE_B),
        ],
        None,
    )
    .unwrap();
    write_7z(
        &sevenz,
        &[
            FixtureEntry::new("a.png", PAGE_A),
            FixtureEntry::new("b.jpg", PAGE_B),
        ],
    )
    .unwrap();

    for path in [&zip, &rar, &sevenz] {
        let output = run(&["probe", path.to_str().unwrap()]);
        assert!(
            output.status.success(),
            "probe {} failed: {}",
            path.display(),
            stderr(&output)
        );
        assert_eq!(stdout(&output), entry_lines(&entries));

        let extracted = directory.path().join(format!(
            "{}-page.bin",
            path.extension().unwrap().to_string_lossy()
        ));
        let output = run(&[
            "extract",
            path.to_str().unwrap(),
            "1",
            extracted.to_str().unwrap(),
        ]);
        assert!(
            output.status.success(),
            "extract {} failed: {}",
            path.display(),
            stderr(&output)
        );
        assert_eq!(fs::read(extracted).unwrap(), PAGE_B);
    }
}

#[test]
fn probe_uses_byte_order_and_filters_suffixes() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("book.cbz");
    write_zip(
        &path,
        &[
            FixtureEntry::new("page10.png", PAGE_A),
            FixtureEntry::new("page2.PNG", PAGE_B),
            FixtureEntry::new("notes.txt", b"ignore"),
        ],
        ZipMethod::Deflate,
        ZipEncryption::None,
        None,
    )
    .unwrap();

    let output = run(&["probe", path.to_str().unwrap()]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        entry_lines(&[("page10.png", PAGE_A), ("page2.PNG", PAGE_B)])
    );
}

#[test]
fn extract_rejects_out_of_range_index() {
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

    let out = directory.path().join("never.bin");
    let output = run(&[
        "extract",
        path.to_str().unwrap(),
        "7",
        out.to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("out of range"));
    assert!(
        !out.exists(),
        "out-of-range extraction must not write a file"
    );
}

#[test]
fn password_is_required_for_encrypted_zip_and_never_needed_for_listing() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("secret.cbz");
    write_zip(
        &path,
        &[FixtureEntry::new("secret.png", PAGE_A)],
        ZipMethod::Deflate,
        ZipEncryption::Aes256,
        Some("open-sesame"),
    )
    .unwrap();

    let output = run(&["probe", path.to_str().unwrap()]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), entry_lines(&[("secret.png", PAGE_A)]));

    let out = directory.path().join("secret.bin");
    let output = run(&[
        "extract",
        path.to_str().unwrap(),
        "0",
        out.to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("password"));

    let output = run(&[
        "extract",
        path.to_str().unwrap(),
        "0",
        out.to_str().unwrap(),
        "--password",
        "wrong",
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("wrong password"));

    let output = run(&[
        "extract",
        path.to_str().unwrap(),
        "0",
        out.to_str().unwrap(),
        "--password",
        "open-sesame",
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(fs::read(out).unwrap(), PAGE_A);
}

#[test]
fn directory_probe_uses_relative_slash_paths() {
    let directory = TempDir::new().unwrap();
    let root = directory.path().join("book");
    fs::create_dir_all(root.join("chapter")).unwrap();
    fs::write(root.join("z.PNG"), PAGE_A).unwrap();
    fs::write(root.join("chapter/a.jpg"), PAGE_B).unwrap();
    fs::write(root.join("chapter/notes.txt"), b"ignore").unwrap();

    let output = run(&["probe", root.to_str().unwrap()]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        entry_lines(&[("chapter/a.jpg", PAGE_B), ("z.PNG", PAGE_A)])
    );
}

#[test]
fn clear_errors_for_no_pages_unsupported_and_corrupt_inputs() {
    let directory = TempDir::new().unwrap();

    let empty = directory.path().join("empty.cbz");
    write_zip(
        &empty,
        &[FixtureEntry::new("notes.txt", b"ignore")],
        ZipMethod::Store,
        ZipEncryption::None,
        None,
    )
    .unwrap();
    let output = run(&["probe", empty.to_str().unwrap()]);
    assert!(!output.status.success());
    assert_eq!(
        stderr(&output).trim(),
        "This archive does not contain any pages."
    );

    let unsupported = directory.path().join("book.txt");
    write_corrupt(&unsupported, b"not an archive").unwrap();
    let output = run(&["probe", unsupported.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("Cannot open"));

    let corrupt = directory.path().join("broken.cbz");
    write_corrupt(&corrupt, b"this is not a zip").unwrap();
    let output = run(&["probe", corrupt.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(stderr(&output).to_lowercase().contains("corrupt"));
}

#[test]
fn extract_writes_bytes_for_a_deflated_entry() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("book.cbz");
    write_zip(
        &path,
        &[
            FixtureEntry::new("a.png", PAGE_A),
            FixtureEntry::new("b.jpg", PAGE_B),
        ],
        ZipMethod::Deflate,
        ZipEncryption::None,
        None,
    )
    .unwrap();

    let out = directory.path().join("page.bin");
    let output = run(&[
        "extract",
        path.to_str().unwrap(),
        "0",
        out.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(fs::read(out).unwrap(), PAGE_A);
}

#[test]
fn output_path_that_cannot_be_written_reports_an_error() {
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

    let missing_parent = directory.path().join("no-such-dir/out.bin");
    let output = run(&[
        "extract",
        path.to_str().unwrap(),
        "0",
        missing_parent.to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("cannot write"));
    assert!(!missing_parent.exists());
}

#[test]
fn probe_sorts_by_bytes_unless_natural_sort_is_requested() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("book.cbz");
    write_zip(
        &path,
        &[
            FixtureEntry::new("page10.png", PAGE_A),
            FixtureEntry::new("page2.png", PAGE_B),
            FixtureEntry::new("page1.png", PAGE_A),
        ],
        ZipMethod::Store,
        ZipEncryption::None,
        None,
    )
    .unwrap();
    let book = path.to_str().unwrap();

    let byte_order = run(&["probe", book]);
    assert!(byte_order.status.success(), "{}", stderr(&byte_order));
    assert_eq!(
        stdout(&byte_order),
        entry_lines(&[
            ("page1.png", PAGE_A),
            ("page10.png", PAGE_A),
            ("page2.png", PAGE_B)
        ])
    );

    let natural_order = run(&["probe", "--natural-sort", book]);
    assert!(natural_order.status.success(), "{}", stderr(&natural_order));
    assert_eq!(
        stdout(&natural_order),
        entry_lines(&[
            ("page1.png", PAGE_A),
            ("page2.png", PAGE_B),
            ("page10.png", PAGE_A)
        ])
    );
}
