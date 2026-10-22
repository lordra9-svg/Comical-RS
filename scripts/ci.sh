#!/usr/bin/env bash
set -euo pipefail

echo "==> Checking formatting..."
cargo fmt --all -- --check

echo "==> Running clippy..."
cargo clippy --all-targets -- -D warnings

echo "==> Running tests..."
cargo test

echo "==> All checks passed."