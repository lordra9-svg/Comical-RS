# Third-party licenses

This repository is MIT. These dependencies keep their own licenses.

| Crate | License | Use |
|---|---|---|
| `zip` | MIT | ZIP reader |
| `rars` | Apache-2.0 | RAR reader |
| `sevenz-rust` | Apache-2.0 | 7z reader |
| `image` | MIT OR Apache-2.0 | JPEG, PNG, GIF decode |
| `eframe` | MIT OR Apache-2.0 | Windowing and UI |
| `egui` | MIT OR Apache-2.0 | UI |
| `clap` | MIT OR Apache-2.0 | CLI |
| `serde` | MIT OR Apache-2.0 | Settings |
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

The Apache-2.0 components remain under Apache-2.0. Keep their notices and
license text. Before release, generate the final dependency list with
`cargo-about` or `cargo-deny` and vendor the full license texts.

Do not add GPL, AGPL, LGPL or unRAR dependencies.
