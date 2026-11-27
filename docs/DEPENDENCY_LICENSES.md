# Dependencies and licensing

The project source uses MIT. Cargo.lock records the exact dependency graph. Rust and the MSVC/Windows SDK, Apple SDK, or Linux linker are build tools. The desktop uses the platform's windowing, file dialogs, and OpenGL system interfaces. No Java or Node runtime is required.

| Component | Role | Upstream license |
| --- | --- | --- |
| `object` | Loader metadata | Apache-2.0 OR MIT |
| `quick-xml` | Local Ghidra program XML parsing | MIT |
| `iced-x86` | Instruction decoder | MIT |
| `eframe` and `egui` | Native desktop workbench | MIT OR Apache-2.0 |
| `rfd` | System file dialogs | MIT |
| `resvg`, `usvg`, `tiny-skia` | Local SVG rendering | Apache-2.0 OR MIT for resvg and usvg. BSD-3-Clause for tiny-skia. |
| `serde`, `serde_json`, `sha2`, `tempfile`, `thiserror`, `clap` | Serialization, integrity, atomic replacement, errors and CLI | Consult the resolved package metadata and upstream license files. |

The full transitive dependency and asset licensing inventory is required for a release. See [third-party notices](../THIRD_PARTY_NOTICES.md).
