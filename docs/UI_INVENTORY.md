# Desktop interface

| Surface | Interaction |
| --- | --- |
| Welcome | Open a binary or project, open as raw x86, or drop a file. |
| Header | Open files, save, save as, Ghidra XML import, guarded byte-patch export, byte search, inspector toggle and theme previews. |
| Document tabs | Switch binaries and projects while preserving per-tab view state. Close with `Ctrl+W` or the tab button. |
| Image browser | Filter functions, select addresses, and inspect regions, symbols, imports, exports and saved bookmarks. |
| Overview | File format, architecture, entry, SHA-256 and file entropy. Click the chart to see the corresponding position in Hex. |
| Workspace | Jump to a virtual address, analyze a function, or move through instruction navigation history. |
| Disassembly | Inspect instructions, select an address, copy its address or instruction text. |
| Linear | Decode sequentially from an explicit address and advance within its executable region. |
| Control flow | Scroll the block diagram and select a block to return to its disassembly. |
| Calls | Inspect direct callers and callees and navigate to function or call-site addresses. |
| Register IR | Inspect projected SSA statements and incoming values. |
| Pseudo-C | Inspect qualified Windows x64 leaf-function pseudocode and copy it. |
| Hex | Browse bytes by file offset with an ASCII column. |
| Search | Find wildcard byte patterns and printable ASCII runs with file-offset results. |
| Inspector | Edit project annotations and bookmarks, and navigate indexed references. |

The desktop uses resizable side panels and stores the selected appearance locally. See [known differences](KNOWN_DIFFERENCES.md) for the scope of comparison.
