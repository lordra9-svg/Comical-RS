//! Process helpers shared by the `xtask` commands.

use std::error::Error;
use std::path::Path;
use std::process::Command;

/// Returns whether `program` is an executable file on `PATH`.
pub fn command_exists(program: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|directory| {
        directory.join(program).is_file()
            || (cfg!(windows) && directory.join(format!("{program}.exe")).is_file())
    })
}

/// Runs `program` with `args`, inheriting stdio, and fails on a non-zero exit.
///
/// `directory` sets the working directory when given.
pub fn run(program: &str, args: &[&str], directory: Option<&Path>) -> Result<(), Box<dyn Error>> {
    run_named(program, program, args, directory)
}

/// Runs `program`, printing `display` instead of the full path.
pub fn run_named(
    display: &str,
    program: &str,
    args: &[&str],
    directory: Option<&Path>,
) -> Result<(), Box<dyn Error>> {
    println!("    $ {display} {}", args.join(" "));
    let mut command = Command::new(program);
    command.args(args);
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{display} {} failed ({status})", args.join(" ")).into())
    }
}

/// Runs `program` without echoing the argument list.
///
/// Only for a command whose arguments carry a credential: the other helpers log
/// their command line, and a notary password must not reach a job log.
pub fn run_silent(
    program: &str,
    args: &[&str],
    directory: Option<&Path>,
) -> Result<(), Box<dyn Error>> {
    let mut command = Command::new(program);
    command.args(args);
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} failed ({status})").into())
    }
}

/// Runs `cargo` with `args`, preferring the cargo that started this task.
pub fn cargo(args: &[&str]) -> Result<(), Box<dyn Error>> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    run_named("cargo", &cargo, args, None)
}
