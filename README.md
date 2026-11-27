<p align="center">
  <img src="assets/branding/hexaris.svg" width="96" height="96" alt="Hexaris logo">
</p>

<h1 align="center">Hexaris</h1>

<p align="center">
  <strong>A native Rust workbench for inspecting executables.</strong><br>
  Follow machine code, browse binary metadata, and save your findings in a portable project.<br>
  Run the same analysis from the desktop or the command line.
</p>

<p align="center">
  <a href="https://github.com/Prestondevs/hexaris/releases/latest"><strong>Download for Windows x64</strong></a> ·
  <a href="#features">Features</a> ·
  <a href="#command-line">Command line</a> ·
  <a href="#build-from-source">Build from source</a> ·
  <a href="LICENSE">MIT license</a>
</p>

<p align="center">
  <img src="docs/images/workbench.png" width="100%" alt="Hexaris in Graphite with multiple binaries open, showing x86-64 disassembly, function navigation, and an address inspector">
  <br>
  <sub>The Graphite workbench on a sample executable, with its function list and address inspector open.</sub>
</p>

## Features

<table>
  <tr>
    <td width="50%" valign="top">
      <a href="docs/images/control-flow.png"><img src="docs/images/control-flow.png" width="100%" alt="A control-flow graph showing the two branches of the score fixture and their common return block"></a>
      <h3>Follow control flow</h3>
      Explore x86 and x86-64 functions in disassembly or as basic blocks. Select a block to open its first instruction in the listing.
    </td>
    <td width="50%" valign="top">
      <a href="docs/images/pseudo-c.png"><img src="docs/images/pseudo-c.png" width="100%" alt="Hexaris displaying C-like pseudocode for an unsigned conditional and two arithmetic return paths"></a>
      <h3>Read lifted code</h3>
      Inspect 32-bit register SSA and incoming values at block boundaries. Open Pseudo-C to see recovered expressions and conditionals for supported Windows x64 leaf functions.
    </td>
  </tr>
  <tr>
    <td width="50%" valign="top">
      <a href="docs/images/triage.png"><img src="docs/images/triage.png" width="100%" alt="Hexaris in Ash Light showing the CLI binary’s executable metadata, SHA-256 checksum, and entropy chart"></a>
      <h3>Inspect the file</h3>
      Find sections, symbols, imports and exports. Check the file’s SHA-256 or click the entropy chart to open the corresponding byte offset in Hex.
    </td>
    <td width="50%" valign="top">
      <a href="docs/images/calls.png"><img src="docs/images/calls.png" width="100%" alt="The Calls view listing direct callers, callees, and their selectable call-site addresses"></a>
      <h3>Trace calls and references</h3>
      Browse direct callers and callees, jump to a call site, and check the inspector for incoming and outgoing references to the selected address.
    </td>
  </tr>
</table>

Screenshots show Hexaris analyzing its own CLI and the [score fixture](tests/fixtures/score.c) and [workbench fixture](tests/fixtures/workbench.c).

| Workflow | What Hexaris provides |
| --- | --- |
| Load an image | Inspect PE, ELF, thin Mach-O, and COFF metadata. Set an architecture and base address when opening raw x86 bytes. |
| Search and navigate | Look for wildcard byte patterns or printable ASCII strings. Open multiple binaries and keep separate selections, views, and history in each tab. |
| Save an investigation | Add comments, rename functions, and save bookmarks. A `.hxproj` embeds the binary and its annotations. [Project format](docs/PROJECT_FORMAT.md) |
| Move annotations between tools | Merge function names, comments, and bookmarks from Ghidra program XML. Existing Hexaris annotations keep their values. [Import guide](docs/GHIDRA_XML.md) |
| Export a patch | Verify the original bytes at a file offset and write a separate copy with a fixed-length replacement. |
| Change the appearance | Preview and switch among Graphite, Slate, Ash Light, Foundry, and High Contrast in **Themes**. The selected palette is saved locally. |

## Downloads

Download the Windows x64 ZIP, extract it, and run `hexaris.exe`. The archive includes `hexaris-cli.exe`, documentation, and license notices. The release page also provides a SHA-256 checksum. [GitHub Releases](https://github.com/Prestondevs/hexaris/releases/latest)

Use **Open**, drag in a file, or pass a path to the desktop executable. Use **File → Open raw image** for unstructured bytes. Press **Ctrl+D** to toggle a bookmark and **Ctrl+S** to save the current project.

Windows x64 is the validated release platform. The source also targets Linux and macOS. See the [feature matrix](docs/FEATURE_MATRIX.md) for tested coverage, including the current limits of decompilation and architecture support.

## Command line

The CLI runs offline and returns JSON for inspection and analysis commands. These PowerShell examples use the executable from the Windows ZIP.

```powershell
.\hexaris-cli.exe inspect sample.exe
.\hexaris-cli.exe triage sample.exe other.exe
.\hexaris-cli.exe analyze sample.exe
.\hexaris-cli.exe find sample.exe "48 8B ?? ??"
.\hexaris-cli.exe project create sample.exe sample.hxproj
```

Address-based commands accept decimal or `0x`-prefixed hexadecimal addresses. Run `hexaris-cli --help` to see the full command list.

## Build from source

Install the Rust toolchain from `rust-toolchain.toml` and the platform linker. Windows builds use MSVC and a Windows SDK. On Linux, the desktop uses system OpenGL, X11 or Wayland, and an XDG desktop portal for file dialogs.

```sh
cargo build --workspace --release --locked
cargo run --locked -p hexaris-desktop -- path/to/program
cargo test --workspace --locked
```

Run `scripts/package-windows.ps1` in a Windows developer shell to build the portable ZIP locally. The desktop and CLI use the same Rust engine. [Architecture](docs/ARCHITECTURE.md) · [Development environment](docs/DEVELOPMENT_ENVIRONMENT.md)

## Contributing

See the [contributor guide](CONTRIBUTING.md) for build checks and fixture instructions. Hexaris is distributed under the [MIT license](LICENSE). [Third-party notices](THIRD_PARTY_NOTICES.md)
