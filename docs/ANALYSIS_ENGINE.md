# Analysis engine

The loader recognizes PE, ELF, thin Mach-O, COFF, and explicit raw x86 images. It records architecture, byte order, sections, segments, defined symbols, and named import and export information when exposed by the format parser. Relocatable objects expose metadata and disk bytes without assigning a linked virtual address. Raw x86 images require an explicit architecture and base address.

File and virtual address mapping uses validated, file-backed ranges. Sections take priority over segments. Zero-filled memory and file padding outside a mapped region do not have a file mapping. An unmapped or non-executable address cannot start a decode.

Automatic x86 analysis seeds from the entry point, defined text symbols, and executable named exports. PE x64 primary unwind records add bounded roots for functions without names, including functions unreachable from the entry. The exception directory inventory covers at most 65,536 complete records and skips chained fragments and unsupported unwind versions. The entry and its direct calls take priority when the analysis budget is limited.

Per-function traversal follows direct branches and fallthrough and stops at returns, unresolved transfers, known function boundaries, or invalid instructions. It records direct calls, branches, and RIP-relative operand references. Indexed lookups provide incoming and outgoing references and call sites. Indirect targets, tail calls and jump tables still require analysis support. [Microsoft x64 unwind specification](https://learn.microsoft.com/en-us/cpp/build/exception-handling-x64)

Linear disassembly decodes sequentially from an explicit executable virtual address. It stops at a region boundary, an invalid instruction, or a requested count and does not follow control transfers.

Automatic analysis is bounded to 4,096 functions and 100,000 instructions total. Functions and individual sequential reads are bounded to 8,192 instructions, and source files are bounded to 256 MiB.
