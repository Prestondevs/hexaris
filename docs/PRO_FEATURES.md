# Commercial and Ultimate workflow coverage

Vector 35 uses the names **Commercial** and **Ultimate** for its current paid tiers. This inventory describes public workflows and the equivalent behavior available in Hexaris. [Edition comparison](https://binary.ninja/purchase/)

| Workflow | Hexaris coverage | Remaining work |
| --- | --- | --- |
| Headless processing and batch triage | Standalone CLI, ordered per-input batch reports, structured analysis and reference output, and a shared Rust API | Bulk analysis presets, configurable worker limits, and external workflow automation |
| Triage and entropy navigation | File identity, named imports and exports, format data, entropy windows, and click-to-hex navigation | Large-corpus comparisons and richer per-region scoring |
| Local project management | Multiple binary and project tabs, separate navigation and search state, bookmark-aware portable projects, active-tab save and close | Persisted multi-file workspace manifests, project folders, and user-managed file links |
| Ghidra and IDB interoperability | Ghidra program XML import for flat-address function names, comments, and bookmarks | Types, data variables, memory maps, database containers, and IDB |
| Function discovery and call navigation | PE x64 primary unwind ranges and executable named exports as roots, indexed direct calls and reference navigation | Secondary unwind chunks, indirect calls, jump tables, and module resolution |
| Instruction semantics and decompilation | x86/x64 decoding, projected register SSA, and limited structured Windows x64 pseudocode | General decompilation, more ISAs and loader formats, flags, memory, and type recovery |
| Debugging | Static code and project inspection | Native and remote debugger backends, breakpoints, register views, and time travel |
| Types, signatures, and extensions | Public Rust engine API | Type-library and signature formats, plugin loading, extension management, and custom analysis workflows |
| Binary similarity and firmware tooling | File hashing and raw x86 inspection | Function similarity, firmware-specific loaders and analysis workflows |
| Team workflows | Shareable analysis files | Remote project state, synchronization, authentication, roles, and audit history |

The [feature matrix](FEATURE_MATRIX.md) records test evidence for individual implementations. The public reference describes richer functionality than the current Hexaris release. [Product features](https://binary.ninja/features/) · [Headless workflow](https://docs.binary.ninja/dev/batch.html) · [Ghidra import](https://docs.binary.ninja/guide/migration/ghidra/ghidraimport.html)
