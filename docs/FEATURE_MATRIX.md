# Feature matrix

The [machine-readable matrix](feature_matrix.json) records public reference behavior, implementation status, test coverage and remaining gaps. A verified workflow describes the tested scope. It does not imply parity across all binaries or platforms.

| Area | Verified or implemented scope | Remaining gaps |
| --- | --- | --- |
| Loaders | Windows x64 PE, explicit raw mappings, and ELF x64 segment fallback with thin Mach-O and COFF metadata paths | Broader fixture coverage, universal binaries, archives, firmware, debug formats |
| Disassembly and CFG | x86/x86-64 decode, linear listing, PE unwind and export roots, indexed direct calls and references | Other ISAs, indirect calls, chained unwind ranges, jump tables, richer data references |
| SSA and decompilation | Projected 32-bit register SSA, constrained Windows x64 leaf pseudocode | Full alias, flags and memory semantics, ABI and type recovery, complex flow |
| Desktop | Multiple tabs with independent view state, five persistent themes with swatches, overview and entropy, function browser, hex, search, inspector | Detachable docking, custom palette editing, persisted multi-file workspaces, advanced graph views |
| Projects and CLI | Embedded images, bookmark persistence, batch triage and JSON, import of Ghidra XML annotations | Incremental analysis state, IDB or full Ghidra project import, extension automation |
| Editing, debugger and extensions | Guarded export of a fixed-length byte replacement, reusable Rust engine API | Live edit/reanalysis, native debugger, plug-in loading and SDK, type editing |
| Platforms and distribution | Windows x64 analysis and portable build workflow | Verified Linux and macOS runtime and packaging, Windows installer |

The public reference inventory links to documented workflows. [Commercial and Ultimate gaps](PRO_FEATURES.md) · [Reference inventory](REFERENCE_INVENTORY.md) · [Known differences](KNOWN_DIFFERENCES.md)
