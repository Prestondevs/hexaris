# Roadmap

1. Expand loader fixtures across PE32, stripped ELF, linked Mach-O and malformed inputs. Establish consistent section/segment mapping, then seed functions from valid unwind entries and exports.
2. Expand lifting to typed values, register aliases, flags, stack and memory, with data-flow validation. Add calling conventions and structured decompilation beyond small leaf functions.
3. Expand reference indexing, source-byte editing, reanalysis and project persistence. Develop navigation and workspace layouts against verified workflows.
4. Verify native debugger lifecycle on original executables before adding attach or other process controls. Define and test a plugin isolation and versioning contract.
5. Verify Linux and macOS desktop operation, produce platform packages, and complete license, corpus, accessibility, and performance validation.

Work is tracked in [the feature matrix](feature_matrix.json). [Checkpoint](HANDOFF.md)
