# Desktop interface

| Surface | Interaction |
| --- | --- |
| Welcome | Open a binary or project, open as raw x86, or drop a file. |
| Header | Open, save, save as, guarded byte-patch export, byte search, and inspector toggle. |
| Image browser | Filter function names, select function address, inspect regions, symbols, imports and exports. |
| Workspace | Jump to a virtual address, analyze a function, or move through instruction navigation history. |
| Disassembly | Inspect instructions, select an address, copy its address or instruction text. |
| Linear | Decode sequentially from an explicit address and advance within its executable region. |
| Control flow | Scroll the block diagram and select a block to return to its disassembly. |
| Register IR | Inspect projected SSA statements and incoming values. |
| Pseudo-C | Inspect qualified Windows x64 leaf-function pseudocode and copy it. |
| Hex | Browse bytes by file offset with an ASCII column. |
| Search | Find wildcard byte patterns and printable ASCII runs with file-offset results. |
| Inspector | Edit project annotations and navigate discovered references. |

The desktop uses one active project, fixed resizable side panels, and a dark palette. See [known differences](KNOWN_DIFFERENCES.md) for the scope of comparison.
