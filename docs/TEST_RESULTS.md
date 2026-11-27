# Test results

On Windows x64, formatting, workspace checks, strict Clippy, all 30 test cases, and the release build passed using the locked dependency set. [Development environment](DEVELOPMENT_ENVIRONMENT.md)

Regression cases cover raw mapping and overflow, direct call discovery, reachability, overlaps, truncated decode, RIP-relative references, CFG joins, SSA backedges, expression width, wildcard bytes, ASCII boundaries, and portable project integrity. The bounded decompiler has cases for a recovered unsigned branch and rejection of loops and unknown memory.

The independently compiled PE fixture verified format, architecture, instruction and block counts, the direct call, linear decode, an annotation round trip, and guarded patch export. Reanalysis found the inverted `jae` condition at the original `jb` location, with the same jump target and unchanged source hash. Recompiling Hexaris's C-like output and comparing it with the original function produced identical results across 250,008 inputs, including the unsigned branch boundary and wrapping cases. [Reproduction commands](../tests/fixtures/README.md)

The workstation smoke test verifies a PE function retained with an unwind record and no incoming calls, an executable named export, indexed call sites, ordered batch results with an intervening missing file, SHA-256 against PowerShell's file hash, and bookmark persistence. It also previews and imports an actual Ghidra 12.2 program XML export, decodes a literal ampersand in comments, and preserves a bookmark already present in the Hexaris project. Automated tests exercise stripped ELF segment fallback, zero-fill mapping, entropy boundaries, malformed XML and the five palette contrast combinations. [Smoke test](../scripts/verify-workstation.ps1)

In the Windows desktop, two fixtures opened as separate tabs, a repeated open focused the existing tab, and switching tabs restored the prior view. The Themes menu displayed five sets of swatches, switching back to Graphite refreshed the workbench, and Ash Light survived a restart. Following an indexed call opened the correct instruction. Clicking the entropy chart selected the exact file offset in Hex, including offsets without a virtual address.

The assistant workflow tests cover exact 64-bit address strings, context pagination, unavailable Pseudo-C, image identity and base checks, and accepting one annotation without applying the others. A live stdio session against the score fixture verified MCP negotiation, tool discovery, function evidence, and references. Comparing a fixture rebuilt with a changed return constant identified the changed function and its first differing instruction. The [user guide](USER_GUIDE.md) includes Windows captures of the context, review, and comparison windows.

The desktop graph layout includes all 241 blocks in the large fixture. Engine tests verify short and near branch inversion, patch queue overlap rejection, and a register-level listing that retains a call and a loop backedge. Windows captures show the syntax colors, fit view, branch patch preview, and both Pseudo-C views. [User guide](USER_GUIDE.md)

The workbench update adds tests for forced branch outcomes, mapped ASCII and ASCII-range UTF-16LE references, operand number bases, address-range dragging, and version 3 argument name persistence. The native desktop smoke test used an authored string fixture to check graph previews, sticky views, in-view search, and address highlights in Pseudo-C. Windows Explorer extracted the new icon from the release executable.

Read-only static analysis of the local Bob’s Gambling PE discovered 71 functions, 252 basic blocks, and 134 direct branch references. Hexaris emitted address-linked Pseudo-C for all 71 discovered functions and structured output for four. The file contained printable strings and the analysis resolved zero direct text references. The sample hash stayed the same.

For 0.2.3, the extracted Windows x64 package reported the new CLI version and opened the score fixture in the desktop app. Windows Explorer extracted the embedded Braille eye icon with transparent corners. The ZIP includes the matching transparent README banner and the dependency license notices.

Linux and macOS native runtime, accessibility, formal differential corpus coverage, fuzzing, and benchmark gates remain open.
