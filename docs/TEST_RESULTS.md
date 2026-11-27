# Test results

On Windows x64, formatting, workspace checks, strict Clippy, the 12 test cases, and the release build passed using the locked dependency set. [Development environment](DEVELOPMENT_ENVIRONMENT.md)

Regression cases cover raw mapping and overflow, direct call discovery, reachability, overlaps, truncated decode, RIP-relative references, CFG joins, SSA backedges, expression width, wildcard bytes, ASCII boundaries, and portable project integrity. The bounded decompiler has cases for a recovered unsigned branch and rejection of loops and unknown memory.

The independently compiled PE fixture verified format, architecture, instruction and block counts, the direct call, linear decode, an annotation round trip, and guarded patch export. Recompiling Hexaris's C-like output and comparing it with the original function produced identical results across 250,008 inputs, including the unsigned branch boundary and wrapping cases. [Reproduction commands](../tests/fixtures/README.md)

Linux and macOS native runtime, accessibility, formal differential corpus coverage, fuzzing, and benchmark gates remain open.
