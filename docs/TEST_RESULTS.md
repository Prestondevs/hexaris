# Test results

On Windows x64, formatting, workspace checks, strict Clippy, all 18 test cases, and the release build passed using the locked dependency set. [Development environment](DEVELOPMENT_ENVIRONMENT.md)

Regression cases cover raw mapping and overflow, direct call discovery, reachability, overlaps, truncated decode, RIP-relative references, CFG joins, SSA backedges, expression width, wildcard bytes, ASCII boundaries, and portable project integrity. The bounded decompiler has cases for a recovered unsigned branch and rejection of loops and unknown memory.

The independently compiled PE fixture verified format, architecture, instruction and block counts, the direct call, linear decode, an annotation round trip, and guarded patch export. Recompiling Hexaris's C-like output and comparing it with the original function produced identical results across 250,008 inputs, including the unsigned branch boundary and wrapping cases. [Reproduction commands](../tests/fixtures/README.md)

The workstation smoke test verifies a PE function retained with an unwind record and no incoming calls, an executable named export, indexed call sites, ordered batch results with an intervening missing file, SHA-256 against PowerShell's file hash, and bookmark persistence. It also previews and imports an actual Ghidra 12.2 program XML export, decodes a literal ampersand in comments, and preserves a bookmark already present in the Hexaris project. Automated tests exercise stripped ELF segment fallback, zero-fill mapping, entropy boundaries, malformed XML and the five palette contrast combinations. [Smoke test](../scripts/verify-workstation.ps1)

In the Windows desktop, two fixtures opened as separate tabs, a repeated open focused the existing tab, and switching tabs restored the prior view. The Themes menu displayed five sets of swatches, switching back to Graphite refreshed the workbench, and Ash Light survived a restart. Following an indexed call opened the correct instruction. Clicking the entropy chart selected the exact file offset in Hex, including offsets without a virtual address.

Linux and macOS native runtime, accessibility, formal differential corpus coverage, fuzzing, and benchmark gates remain open.
