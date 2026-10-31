//! The local gate: formatting, clippy and tests.
//!
//! Mirrors `.github/workflows/ci.yml`. Both must run the same checks.

use std::error::Error;

use crate::tool;

/// Runs format check, clippy and the test suite in order.
pub fn run() -> Result<(), Box<dyn Error>> {
    println!("==> Checking formatting...");
    tool::cargo(&["fmt", "--all", "--", "--check"])?;

    println!("==> Running clippy...");
    tool::cargo(&["clippy", "--all-targets", "--", "-D", "warnings"])?;

    println!("==> Running tests...");
    tool::cargo(&["test", "--workspace"])?;

    println!("==> All checks passed.");
    Ok(())
}
