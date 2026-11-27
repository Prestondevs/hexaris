# Development checkpoint

The repository contains an independent engine, desktop and CLI. Windows x64 currently has the strongest test evidence. The analyzed fixture is `tests/fixtures/score.c`, with build and comparison instructions next to it. [Research workflow](REA_INVESTIGATIONS.md)

Before making changes, run the commands in [the contributor guide](../CONTRIBUTING.md). Restore research prerequisites from [the development environment](DEVELOPMENT_ENVIRONMENT.md) if working on another machine. Keep local reference binaries and evidence out of the repository.

The next engine task is to add a linked ELF fixture and PE unwind-table seeding with explicit address mapping tests. Verify stripped executables and reconcile duplicate or overlapping function roots before expanding instruction semantics. Update the feature matrix and test report with results.
