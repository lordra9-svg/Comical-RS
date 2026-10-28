# Real-writer archive corpus

Interoperability fixtures produced by Info-ZIP `zip` and 7-Zip `7z`, not by the
in-tree `xtask` writers. Payloads are synthetic page stubs (no copyrighted
comics). Copyrighted samples must stay outside this repository.

## Layout

```text
pages/       source page stubs used when regenerating fixtures
fixtures/    committed archives from Info-ZIP and 7-Zip
manifest.json expected page order, sizes and SHA-256 digests
```

## Coverage

| Fixture | Writer | Notes |
|---|---|---|
| `infozip-store.cbz` | Info-ZIP | stored ZIP |
| `infozip-deflate.cbz` | Info-ZIP | deflated ZIP |
| `infozip-zipcrypto.cbz` | Info-ZIP | ZipCrypto, password `corpus-secret` |
| `infozip-unicode-deep.cbz` | Info-ZIP | Unicode name and deep path |
| `sevenzip-zip-deflate.cbz` | 7-Zip | ZIP container via 7-Zip |
| `sevenzip-lzma2.cb7` | 7-Zip | non-solid LZMA2 |
| `sevenzip-lzma.cb7` | 7-Zip | non-solid LZMA |
| `sevenzip-solid.cb7` | 7-Zip | solid LZMA2 |
| `sevenzip-aes.cb7` | 7-Zip | AES-256 data, password `corpus-secret` |
| `sevenzip-aes-headers.cb7` | 7-Zip | AES-256 headers, password `corpus-secret` |
| `truncated-zip.cbz` | truncated | must fail to open |
| `truncated-7z.cb7` | truncated | must fail to open |

RAR samples from WinRAR / rarcli are not included yet (no RAR writer on the
build host). Add them the same way when available.

## Tests

```text
cargo test -p comical-archive --test corpus
```

## Regenerate

Requires Info-ZIP `zip` and 7-Zip `7z` on `PATH`:

```text
./scripts/regen-corpus.sh
```

Then re-run the corpus test. Do not regenerate from the Rust `xtask` writers;
those already have their own synthetic coverage.
