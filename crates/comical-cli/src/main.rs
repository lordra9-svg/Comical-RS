//! Command line archive probe and extract.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use comical_archive::{DEFAULT_MAX_ENTRY_BYTES, OpenOptions, open_with_options};

#[derive(Debug, Parser)]
#[command(
    name = "comical-cli",
    version,
    about = "Probe and extract pages from comic archives"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List image pages in book order.
    Probe {
        /// ZIP, RAR or 7z archive, or a directory.
        book: PathBuf,
        /// Password for an encrypted archive.
        #[arg(long)]
        password: Option<String>,
    },
    /// Write one page to a file.
    Extract {
        /// ZIP, RAR or 7z archive, or a directory.
        book: PathBuf,
        /// Book-order page index, starting at 0.
        index: usize,
        /// Destination file.
        out: PathBuf,
        /// Password for an encrypted archive.
        #[arg(long)]
        password: Option<String>,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Probe { book, password } => {
            let archive = open(&book, password.as_deref())?;
            if archive.pages().is_empty() {
                return Err("This archive does not contain any pages.".to_owned());
            }
            for (index, page) in archive.pages().iter().enumerate() {
                println!("{index}\t{}\t{}", page.size, page.name);
            }
            Ok(())
        }
        Command::Extract {
            book,
            index,
            out,
            password,
        } => {
            let mut archive = open(&book, password.as_deref())?;
            if index >= archive.pages().len() {
                return Err(format!(
                    "page index {index} is out of range; this book has {} pages",
                    archive.pages().len()
                ));
            }
            let bytes = archive
                .read_page(index, DEFAULT_MAX_ENTRY_BYTES)
                .map_err(|error| error.to_string())?;
            std::fs::write(&out, bytes)
                .map_err(|error| format!("cannot write {}: {error}", out.display()))?;
            Ok(())
        }
    }
}

fn open(
    book: &std::path::Path,
    password: Option<&str>,
) -> Result<Box<dyn comical_archive::ComicArchive>, String> {
    let options = OpenOptions::new().with_password(password);
    open_with_options(book, options).map_err(|error| error.to_string())
}
