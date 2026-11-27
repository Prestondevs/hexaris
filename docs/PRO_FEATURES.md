# Binary Ninja and IDA Pro workflow coverage

Vector 35 uses the names **Commercial** and **Ultimate** for its current paid tiers. This inventory describes public workflows and the equivalent behavior available in Hexaris. [Edition comparison](https://binary.ninja/purchase/)

| Workflow | Hexaris coverage | Remaining work |
| --- | --- | --- |
| Headless processing and batch triage | Standalone CLI, ordered per-input batch reports, structured analysis and reference output, and a shared Rust API | Bulk analysis presets, configurable worker limits, and external workflow automation |
| Triage and entropy navigation | File identity, named imports and exports, format data, entropy windows, and click-to-hex navigation | Large-corpus comparisons and richer per-region scoring |
| Local project management | Multiple binary and project tabs, separate navigation and search state, bookmark-aware portable projects, active-tab save and close | Persisted multi-file workspace manifests, project folders, and user-managed file links |
| Ghidra and IDB interoperability | Ghidra program XML import for flat-address function names, comments, and bookmarks | Types, data variables, memory maps, database containers, and IDB |
| Function discovery and call navigation | PE x64 primary unwind ranges and executable named exports as roots, indexed direct calls and reference navigation | Secondary unwind chunks, indirect calls, jump tables, and module resolution |
| Instruction semantics and decompilation | x86/x64 decoding, projected register SSA, limited structured Windows and System V x86-64 Pseudo-C, and address-linked code for calls, loops and string references | Structured non-leaf output, general local-variable and type recovery, flags, memory, and additional ISAs |
| Interactive code navigation | Per-tab code view preference, graph edge and string navigation, cross-view address highlights, operand bases, and in-view search | Independently docked and synchronized code panes, richer graph editing, indirect target resolution |
| Binary patching | Fixed-length patch queue for mapped offsets and short and near x86 conditional branches | Assembly input, interactive patch undo history, debugger-backed editing |
| Debugging | Static code and project inspection | Native and remote debugger backends, breakpoints, register views, and time travel |
| Types, signatures, and extensions | Public Rust engine API and recovered argument names | Type libraries, library recognition signatures, a shared function metadata service, runtime plugin loading, extension management, and scripting SDK |
| Binary similarity and firmware tooling | File hashing and raw x86 inspection | Function similarity, firmware-specific loaders and analysis workflows |
| Team workflows | Shareable analysis files | Remote project state, synchronization, authentication, roles, and audit history |

The [feature matrix](FEATURE_MATRIX.md) records test evidence for individual implementations. IDA Pro documents broad processor and decompiler coverage, interactive retyping, local and remote debugging, FLIRT signatures, Lumina metadata, and automation. Binary Ninja documents synchronized panes, type libraries and archives, multiple intermediate languages, and debugger backends. [IDA Pro](https://hex-rays.com/ida-pro) · [Binary Ninja User Guide](https://docs.binary.ninja/guide/index.html) · [Binary Ninja Debugger](https://docs.binary.ninja/guide/debugger/index.html)
