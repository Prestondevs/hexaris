# Development checkpoint

The repository contains an independent engine, desktop and CLI. Windows x64 currently has the strongest test evidence. The analyzed fixture is `tests/fixtures/score.c`, with build and comparison instructions next to it. [Research workflow](REA_INVESTIGATIONS.md)

Before making changes, run the commands in [the contributor guide](../CONTRIBUTING.md). Restore research prerequisites from [the development environment](DEVELOPMENT_ENVIRONMENT.md) if working on another machine. Keep local reference binaries and evidence out of the repository.

Version 0.2.1 updates the desktop artwork, screenshots, and Windows launch instructions. Version 0.2.0 added stripped ELF segment fallback coverage and PE unwind-table seeding. The next engine task is to reconcile chained unwind ranges and tail calls, then expand typed instruction semantics and non-leaf decompilation on independently compiled fixtures. The [paid workflow inventory](PRO_FEATURES.md) also identifies the remaining debugger, type library, plugin, and workspace work. Update the feature matrix and test report with results.

The current source tree adds function context export, assistant response review, read-only local MCP, and heuristic build comparison. All share the Rust engine. The [user guide](USER_GUIDE.md) shows the workflows and reproduces them with the score fixture.

Disassembly and Linear use formatter tokens and theme colors. The control-flow canvas fits all decoded blocks in the selected function and supports pan, zoom, focus, and branch selection. The patch editor queues fixed-length replacements and opens the exported copy in a separate tab. Pseudo-C has a register-level control-flow view for decoded functions with calls, loops, or other unsupported structured effects. [Editing walkthrough](USER_GUIDE.md#export-a-byte-patch) · [Decompilation scope](DECOMPILER.md)

The desktop keeps the selected code representation during function navigation and shares address highlights between the graph, disassembly, and address-linked Pseudo-C. It has in-view text search, operand base controls, a value converter, argument names in version 3 projects, and branch patch menus. Static testing of a local training executable is tracked in the test report without publishing the input or its analysis export. [User guide](USER_GUIDE.md)

The README banner and app icon render the same supplied Braille dot artwork on transparent canvases. The app uses the square SVG for the welcome screen and desktop header, and the Windows MSVC build embeds multiple sizes in the executable. [Asset manifest](../assets/ASSET_MANIFEST.toml)
