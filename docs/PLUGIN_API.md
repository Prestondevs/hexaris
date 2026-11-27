# Extension API status

`hexaris-engine` exposes the current typed image, disassembly and control-flow models, projected register SSA, decompilation and project operations to Rust clients. These are early library APIs. The CLI writes JSON for headless consumers.

Plugin discovery, isolation, version negotiation and UI extension APIs remain open. Existing library linkage should be described as a Rust API until a plugin contract is implemented and tested.
