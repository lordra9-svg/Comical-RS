#!/usr/bin/env bash
# Build release binaries and packages for all platforms.
# Usage: ./scripts/release.sh [version]
# If no version is given, reads the workspace version from Cargo.toml.

set -euo pipefail

# The version lives in [workspace.package]; every crate inherits it through
# `version.workspace = true`, so a crate manifest has no literal version.
VERSION="${1:-$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')}"
if [[ ! "${VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "release: could not determine the version (read '${VERSION}' from Cargo.toml)" >&2
    exit 1
fi
TARGET_DIR="target/release-dist/${VERSION}"

echo "Building Comical v${VERSION}..."
mkdir -p "${TARGET_DIR}"

# Linux x86_64
echo "[linux-x86_64] cargo build --release"
cargo build --release -p comical-app --bin comical
cp target/release/comical "${TARGET_DIR}/comical-linux-x86_64"

# Portable tarball for the Linux binary.
echo "[linux-tar] comical-${VERSION}-linux-x86_64.tar.gz"
tar -czf "${TARGET_DIR}/comical-${VERSION}-linux-x86_64.tar.gz" -C target/release comical

# Linux .deb (requires cargo-deb)
if command -v cargo-deb &>/dev/null; then
    echo "[linux-deb] cargo deb"
    cargo deb -p comical-app --no-build \
        --output "${TARGET_DIR}/comical_${VERSION}-1_amd64.deb"
else
    echo "[linux-deb] SKIP: cargo-deb not installed (cargo install cargo-deb)"
fi

# Windows .exe (cross-compile or native)
if [[ "$(uname)" == "MINGW"* ]] || [[ "$(uname)" == "CYGWIN"* ]] || [[ "${CARGO_TARGET_X86_64_PC_WINDOWS_MSVC:-}" == "1" ]]; then
    echo "[windows] cargo build --release --target x86_64-pc-windows-msvc"
    cargo build --release -p comical-app --bin comical --target x86_64-pc-windows-msvc
    cp target/x86_64-pc-windows-msvc/release/comical.exe "${TARGET_DIR}/comical-windows-x86_64.exe"

    # Windows MSI (requires cargo-wix)
    if command -v cargo-wix &>/dev/null; then
        echo "[windows-msi] cargo wix"
        cargo wix -p comical-app --no-build --nocapture --output "${TARGET_DIR}/comical-${VERSION}-x86_64.msi"
    else
        echo "[windows-msi] SKIP: cargo-wix not installed (cargo install cargo-wix)"
    fi
else
    echo "[windows] SKIP: not on Windows and cross-compile not configured"
fi

# macOS .app bundle (requires cargo-bundle)
if [[ "$(uname)" == "Darwin" ]]; then
    if command -v cargo-bundle &>/dev/null; then
        echo "[macos] cargo bundle --release"
        cargo bundle --release -p comical-app
        cp -r target/release/bundle/osx/Comical.app "${TARGET_DIR}/Comical.app"
    else
        echo "[macos] SKIP: cargo-bundle not installed (cargo install cargo-bundle)"
    fi
else
    echo "[macos] SKIP: not on macOS"
fi

# Checksums for every artifact in the release directory.
if command -v sha256sum &>/dev/null; then
    echo "[checksums] SHA256SUMS.txt"
    (
        cd "${TARGET_DIR}"
        find . -maxdepth 1 -type f ! -name SHA256SUMS.txt -print0 \
            | sort -z \
            | xargs -0 sha256sum > SHA256SUMS.txt
    )
fi

echo ""
echo "Release artifacts in ${TARGET_DIR}:"
ls -lh "${TARGET_DIR}/"
echo ""
echo "Next steps:"
echo "  1. Test each binary on its target platform"
echo "  2. Sign Windows/macOS binaries if applicable"
echo "  3. Create GitHub release with changelog and SHA256SUMS.txt"
echo "  4. Upload artifacts to release"
