# Hexaris

Hexaris is a native Rust workstation for local executable inspection and static analysis. It includes a desktop application, a headless CLI, and a reusable Rust engine.

The current development release supports executable metadata, x86 and x86-64 disassembly, control-flow graphs, direct cross-references, an SSA register projection, limited structured pseudocode for Windows x64 leaf functions, byte search, and portable projects with address comments and function names. See the [feature matrix](docs/FEATURE_MATRIX.md) for verified workflows and coverage gaps.

## Build and run

Install Rust and the native linker for your platform. On Windows, use the MSVC toolchain and a Windows SDK. On Linux, the desktop uses system OpenGL, X11 or Wayland, and an XDG desktop portal for file dialogs.

```sh
cargo build --workspace --release --locked
cargo run --locked -p hexaris-desktop -- path/to/program
```

The desktop accepts a file path, drag and drop, or **Open**. Use **File → Open raw image** to choose an architecture and base address for unstructured x86 bytes. Select an instruction to see its file offset, address comment, and discovered references in the inspector. Apply comments and names before saving a project.

Open more files to work across tabs. Each tab keeps its selection, history and search state. **Overview** shows file identity and click-to-hex entropy. **Calls** shows incoming and outgoing direct call sites. Press **Ctrl+D** to toggle a bookmark at the current address. Use **Themes** to switch palettes. Graphite is the default, with Slate, Ash Light, Foundry and High Contrast also available. The menu previews canvas, panel, accent and text colors. Hexaris remembers the chosen theme.

```sh
cargo run --locked -p hexaris-cli -- inspect path/to/program
cargo run --locked -p hexaris-cli -- triage path/to/first path/to/second
cargo run --locked -p hexaris-cli -- analyze path/to/program
cargo run --locked -p hexaris-cli -- references path/to/program 0x140001000
cargo run --locked -p hexaris-cli -- disassemble path/to/program 0x140001000 --count 32
cargo run --locked -p hexaris-cli -- decompile path/to/program 0x140001000
cargo run --locked -p hexaris-cli -- find path/to/program "48 8B ?? ??"
cargo run --locked -p hexaris-cli -- project create path/to/program sample.hxproj
```

Choose a virtual address appropriate to the input. Addresses accept decimal or `0x`-prefixed hexadecimal. Run `hexaris-cli --help` for the full command list. A project embeds the original bytes and local annotations and can be reopened without the source file. [Project format](docs/PROJECT_FORMAT.md)

**File → Export byte patch** and the CLI `patch` command can write a separate binary copy with a fixed-length replacement verified against the expected original bytes. The offset is a file offset. Reopen the copy to analyze it.

Hexaris can merge names, comments and bookmarks from a Ghidra program XML export. [Import instructions](docs/GHIDRA_XML.md) · [Commercial workflow coverage](docs/PRO_FEATURES.md)

Windows x86-64 is the current validation platform. The source targets Linux and macOS through the same Rust desktop framework. The runtime performs local analysis and does not launch another analysis engine. [Development environment](docs/DEVELOPMENT_ENVIRONMENT.md)

On Windows, run `scripts/package-windows.ps1` from a developer shell to create a portable ZIP with both executables and license notices under `dist/`. Extract the archive and open `hexaris.exe` or invoke `hexaris-cli.exe` from a terminal. To uninstall the portable build, remove the extracted directory. Project files live wherever you save them.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the build gates and how to add a reproducible analysis fixture. Hexaris is distributed under the [MIT license](LICENSE). [Third-party notices](THIRD_PARTY_NOTICES.md)
