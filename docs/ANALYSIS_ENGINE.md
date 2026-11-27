# Analysis engine

The loader recognizes PE, ELF, thin Mach-O, COFF, and explicit raw x86 images. It records architecture, byte order, sections, segments, defined symbols, and named import and export information when exposed by the format parser. Relocatable objects expose metadata and disk bytes without assigning a linked virtual address. Raw x86 images require an explicit architecture and base address.

File and virtual address mapping uses validated, file-backed ranges. Sections take priority over segments. Zero-filled memory and file padding outside a mapped region do not have a file mapping. An unmapped or non-executable address cannot start a decode.

Automatic x86 analysis seeds from mapped entry points and defined text symbols, then follows direct calls to additional executable targets. Per-function traversal follows direct branches and fallthrough and stops at returns, unresolved transfers, known function boundaries, or invalid instructions. It partitions the discovered instructions into basic blocks and records direct calls, branches, and RIP-relative operand references. It reports overlaps and analysis budget exhaustion. Indirect targets, unwind metadata, tail calls and jump tables still require analysis support.

Linear disassembly decodes sequentially from an explicit executable virtual address. It stops at a region boundary, an invalid instruction, or a requested count and does not follow control transfers.

Automatic analysis is bounded to 4,096 functions and 100,000 instructions total. Functions and individual sequential reads are bounded to 8,192 instructions, and source files are bounded to 256 MiB.
