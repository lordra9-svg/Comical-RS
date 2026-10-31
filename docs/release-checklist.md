# Release Checklist

This document covers the manual and external steps required before publishing
a Comical-RS release. Automated packaging is handled by `scripts/release.sh`;
this checklist covers everything that cannot be automated.

## Pre-release verification

- [ ] All milestones M0–M12 pass on Windows, macOS and Linux
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --all-targets -- -D warnings` passes with zero warnings
- [ ] `cargo test --all` passes on all three platforms
- [ ] Manual acceptance testing completed on each platform:
  - [ ] Open CBZ, CBR and 7z archives (including password-protected)
  - [ ] One-page, two-page and webtoon view modes render correctly
  - [ ] EXIF orientation applied when setting is enabled
  - [ ] Per-page rotation works via keyboard shortcuts
  - [ ] RTL reading mode swaps pages correctly
  - [ ] Thumbnail browser highlights current page
  - [ ] Settings persist across restart
  - [ ] Memory usage stays within cache budget during rapid page turns
  - [ ] No crashes or hangs on malformed/corrupt archives
- [ ] CHANGELOG.md updated with all changes since last release
- [ ] Version bumped in workspace Cargo.toml

## Binary signing

- [ ] **Windows**: Sign `.exe` and `.msi` with a code signing certificate
  - Obtain certificate from a trusted CA (DigiCert, Sectigo, etc.)
  - Sign with `signtool sign /f cert.pfx /p PASSWORD /tr http://timestamp.digicert.com /td sha256 /fd sha256 comical.exe`
  - Verify signature: `signtool verify /pa comical.exe`
- [ ] **macOS**: Sign `.app` bundle and notarize with Apple
  - `codesign --force --deep --sign "Developer ID Application: ..." Comical.app`
  - Create DMG: `hdiutil create -volname Comical -srcfolder Comical.app -ov Comical.dmg`
  - Notarize: `xcrun notarytool submit Comical.dmg --apple-id ... --team-id ... --password ...`
  - Staple: `xcrun stapler staple Comical.app`
- [ ] **Linux**: GPG-sign `.deb` package if distributing via apt repository
  - `dpkg-sig --sign builder comical_*.deb`

## File association verification

- [ ] **Windows**: Install MSI on clean Windows VM; verify .cbz/.cbr/.cb7 open with Comical
- [ ] **macOS**: Install .app on clean macOS; verify double-click opens comic files
- [ ] **Linux**: Install .deb on clean Ubuntu/Debian; verify `xdg-open comic.cbz` launches Comical

## GitHub release

- [ ] Create tag: `git tag -a v0.1.0 -m "Release v0.1.0"`
- [ ] Push tag: `git push origin v0.1.0`
- [ ] Create GitHub release with:
  - [ ] Release title and description from CHANGELOG
  - [ ] Upload all platform binaries from `target/release-dist/`
  - [ ] Attach SHA-256 checksums file
  - [ ] Link to full changelog
- [ ] Verify all download links work

## Legal and compliance

- [ ] Independent clean-room reviewer named and review completed
  - Reviewer must not have contributed code to the project
  - Review covers: no copied GPL/LGPL code, no reverse-engineered proprietary codecs
  - Review documented in CLEANROOM.md or separate review report
- [ ] Legal review completed
  - License compatibility audit for all dependencies
  - THIRD_PARTY_LICENSES.md verified accurate and complete
  - Trademark search for product name "Comical"
  - Privacy policy if app collects any user data (currently it does not)
- [ ] PROVENANCE.md reviewed for accuracy

## Post-release

- [ ] Announce release (blog, social media, forums as appropriate)
- [ ] Monitor issue tracker for regression reports
- [ ] Update roadmap status table
- [ ] Begin next milestone planning

## Known blockers for first public release

These items are documented as known gaps and must be resolved before v1.0:

1. **Webtoon renderer**: Data model exists but continuous vertical scroll is not
   implemented. Users selecting Webtoon mode get single-page behavior.
2. **RAR corpus**: No WinRAR/rarcli samples in test corpus. RAR compatibility
   is unverified against real-world archives.
3. **Clean-room reviewer**: No independent reviewer has been named or engaged.
4. **Legal review**: Not started. Required before public distribution.
5. **Binary signing certificates**: Not obtained. Unsigned binaries trigger
   OS security warnings on Windows and macOS.
6. **App icons**: Placeholder only. Need designed icons for all platforms.