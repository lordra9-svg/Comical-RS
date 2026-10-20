# Third-party licenses

This repository is MIT. These dependencies keep their own licenses.

| Crate | License | Use |
|---|---|---|
| `zip` | MIT | ZIP reader |
| `rars` | Apache-2.0 | RAR reader |
| `sevenz-rust` | Apache-2.0 | 7z reader |
| `lzma-rust` | Apache-2.0 | LZMA codec behind `sevenz-rust` |
| `zlib-rs` | Zlib | Deflate behind `zip` |
| `image` | MIT OR Apache-2.0 | JPEG, PNG, GIF decode |
| `byteorder-lite` | Unlicense OR MIT | Byte order helpers behind `image` |
| `bytemuck` | Zlib OR Apache-2.0 OR MIT | Safe casts behind `image` |
| `color_quant` | MIT | GIF palette quantization |
| `gif` | MIT OR Apache-2.0 | GIF decoder |
| `weezl` | MIT OR Apache-2.0 | LZW codec behind `gif` |
| `png` | MIT OR Apache-2.0 | PNG decoder |
| `fdeflate` | MIT OR Apache-2.0 | Fast deflate behind `png` |
| `flate2` | MIT OR Apache-2.0 | Deflate behind `png` |
| `miniz_oxide` | MIT OR Zlib OR Apache-2.0 | Deflate backend |
| `simd-adler32` | MIT | Adler-32 backend |
| `adler2` | 0BSD OR MIT OR Apache-2.0 | Adler-32 backend |
| `zune-jpeg` | MIT OR Apache-2.0 OR Zlib | JPEG decoder |
| `zune-core` | MIT OR Apache-2.0 OR Zlib | Shared types behind `zune-jpeg` |
| `moxcms` | BSD-3-Clause OR Apache-2.0 | Colour management behind `image` |
| `pxfm` | BSD-3-Clause OR Apache-2.0 | Maths behind `moxcms` |
| `bitflags` | MIT OR Apache-2.0 | Flag types |
| `crc32fast` | MIT OR Apache-2.0 | PNG checksums |
| `cfg-if` | MIT OR Apache-2.0 | Configuration helper |
| `num-traits` | MIT OR Apache-2.0 | Numeric traits |
| `autocfg` | Apache-2.0 OR MIT | Build-time configuration |
| `eframe` | MIT OR Apache-2.0 | Windowing and UI |
| `egui` | MIT OR Apache-2.0 | UI |
| `clap` | MIT OR Apache-2.0 | CLI |
| `tempfile` | MIT OR Apache-2.0 | Test-only temporary directories |
| `serde` | MIT OR Apache-2.0 | Settings |
| `serde_json` | MIT OR Apache-2.0 | Test vectors |
| `toml` | MIT OR Apache-2.0 | Settings |
| `thiserror` | MIT OR Apache-2.0 | Library errors |
| `anyhow` | MIT OR Apache-2.0 | App errors |
| `crossbeam-channel` | MIT OR Apache-2.0 | Worker channel |
| `tracing` | MIT | Logging |
| `tracing-subscriber` | MIT | Logging |
| `rfd` | MIT | Native file dialogs |
| `muda` | MIT OR Apache-2.0 | Native menus |
| `secrecy` | MIT OR Apache-2.0 | Password wrapper |
| `sha2` | MIT OR Apache-2.0 | Test hashes |

Some crates offer a choice of licenses. The project elects MIT or Apache-2.0,
never LGPL. `r-efi` is triple-licensed as MIT, Apache-2.0 or LGPL-2.1-or-later
and is used under MIT or Apache-2.0. `moxcms` and `pxfm` are BSD-3-Clause or
Apache-2.0 and are used under Apache-2.0.

The Apache-2.0 components remain under Apache-2.0. Keep their notices and
license text. Before release, generate the final dependency list with
`cargo-about` or `cargo-deny` and vendor the full license texts.

Do not add GPL, AGPL, LGPL-only or unRAR dependencies.
