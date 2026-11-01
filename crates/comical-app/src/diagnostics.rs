//! Startup diagnostics for the GUI binary.
//!
//! Release builds on Windows are linked as GUI programs, so they have no
//! console. A failure before the window exists used to end the process with
//! exit code 1 and nothing visible anywhere. Every panic and every fatal
//! startup error is appended to `comical.log` in the platform data directory,
//! and a fatal startup error also opens a message box naming that file.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Log file name inside the platform data directory.
const LOG_FILE: &str = "comical.log";

/// Installs a panic hook that writes the panic to the log file.
///
/// The previous hook still runs, so a debug build keeps its stderr output.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|location| format!("{}:{}", location.file(), location.line()))
            .unwrap_or_else(|| "unknown location".to_owned());
        let payload = describe_payload(info.payload());
        let backtrace = std::backtrace::Backtrace::force_capture();
        record(&format!("panic at {location}: {payload}\n{backtrace}"));
        previous(info);
    }));
}

/// Writes a fatal startup error to the log and tells the user about it.
///
/// Returns the message for stderr, which a terminal shows on every platform
/// and a Windows GUI build discards.
pub fn report_fatal(error: &str) -> String {
    let path = record(&format!("fatal: {error}"));
    let message = match &path {
        Some(path) => format!(
            "Comical could not start.\n\n{error}\n\nDetails were written to:\n{}",
            path.display()
        ),
        None => format!("Comical could not start.\n\n{error}"),
    };
    show_dialog(&message);
    message
}

/// Appends one timestamped entry to the log file, returning the file used.
///
/// Logging never fails the caller: an unwritable directory returns `None`.
pub fn record(entry: &str) -> Option<PathBuf> {
    let path = log_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok()?;
    }
    append(&path, entry).ok()?;
    Some(path)
}

/// Returns the log file path, or `None` if there is no data directory.
pub fn log_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "comical").map(|dirs| dirs.data_dir().join(LOG_FILE))
}

fn append(path: &Path, entry: &str) -> std::io::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "[{}] {}", timestamp(), entry)
}

/// Whole seconds since the Unix epoch; good enough to order log entries.
fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// Renders a panic payload, which is usually a string but need not be.
fn describe_payload(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "panic payload was not a string".to_owned()
    }
}

/// Shows a blocking error box. Only Windows needs it: this is the platform
/// where the release binary has no console to print to.
#[cfg(windows)]
fn show_dialog(message: &str) {
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("Comical")
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

#[cfg(not(windows))]
fn show_dialog(_message: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_writes_one_timestamped_line_per_entry() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("nested/comical.log");
        fs::create_dir_all(path.parent().expect("parent")).expect("create dir");

        append(&path, "first").expect("first write");
        append(&path, "second").expect("second write");

        let text = fs::read_to_string(&path).expect("read log");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with('['), "{}", lines[0]);
        assert!(lines[0].ends_with("] first"), "{}", lines[0]);
        assert!(lines[1].ends_with("] second"), "{}", lines[1]);
    }

    #[test]
    fn append_keeps_earlier_entries() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("comical.log");
        append(&path, "before crash").expect("write");
        append(&path, "fatal: no window").expect("write");
        let text = fs::read_to_string(&path).expect("read log");
        assert!(text.contains("before crash"));
        assert!(text.contains("fatal: no window"));
    }

    #[test]
    fn timestamp_orders_entries_after_2020() {
        assert!(timestamp() > 1_577_836_800);
    }

    #[test]
    fn describe_payload_reads_str_and_string_payloads() {
        let borrowed: &(dyn std::any::Any + Send) = &"borrowed message";
        assert_eq!(describe_payload(borrowed), "borrowed message");

        let owned: &(dyn std::any::Any + Send) = &"owned message".to_owned();
        assert_eq!(describe_payload(owned), "owned message");
    }

    #[test]
    fn describe_payload_handles_non_string_payloads() {
        let number: &(dyn std::any::Any + Send) = &42u8;
        assert_eq!(describe_payload(number), "panic payload was not a string");
    }
}
