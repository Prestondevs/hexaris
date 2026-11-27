# Feature matrix

The [machine-readable matrix](feature_matrix.json) records public reference behavior, implementation status, test coverage and remaining gaps. A verified workflow describes the tested scope. It does not imply parity across all binaries or platforms.

| Area | Verified or implemented scope | Remaining gaps |
| --- | --- | --- |
| Loaders | Windows x64 PE, explicit raw mappings, and ELF x64 segment fallback with thin Mach-O and COFF metadata paths | Broader fixture coverage, universal binaries, archives, firmware, debug formats |
| Disassembly and CFG | Syntax-highlighted x86/x86-64 listings, fit and focus controls for every decoded block in one function, PE unwind and export roots, indexed direct calls and references | Other ISAs, indirect calls, chained unwind ranges, jump tables, richer data references |
| SSA and decompilation | Projected 32-bit register SSA, structured Windows and System V x86-64 leaf pseudocode, register-level listing with branches and calls | Full alias, flags and memory semantics, type recovery, structured non-leaf recovery |
| Desktop | Multiple tabs with independent view state, five persistent themes with swatches, overview and entropy, function browser, exact byte selection, search, inspector | Detachable docking, custom palette editing, persisted multi-file workspaces, call and data-flow graphs |
| Projects and CLI | Embedded images, bookmark persistence, batch triage and JSON, import of Ghidra XML annotations | Incremental analysis state, IDB or full Ghidra project import, extension automation |
| Assistant workflows | Paged Markdown and JSON function context, clickable evidence, individually accepted annotations, and a read-only local stdio MCP server | Built-in model integration, richer exported IR, and a broader MCP client test matrix |
| Build comparison | Desktop and CLI comparison by relative address, unique function name, or identical decoded bytes, with links and the first differing instruction | Semantic equivalence, indirect targets, and undiscovered code |
| Editing, debugger and extensions | Preview and queue byte replacements, invert supported conditional branches, export and reanalyze a separate copy, reusable Rust engine API | Assembly input, live editing, native debugger, plug-in loading and SDK, type editing |
| Platforms and distribution | Windows x64 analysis and portable build workflow | Verified Linux and macOS runtime and packaging, Windows installer |

The public reference inventory links to documented workflows. [Commercial and Ultimate gaps](PRO_FEATURES.md) · [Reference inventory](REFERENCE_INVENTORY.md) · [Known differences](KNOWN_DIFFERENCES.md)
