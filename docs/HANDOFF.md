# Development checkpoint

The repository contains an independent engine, desktop and CLI. Windows x64 currently has the strongest test evidence. The analyzed fixture is `tests/fixtures/score.c`, with build and comparison instructions next to it. [Research workflow](REA_INVESTIGATIONS.md)

Before making changes, run the commands in [the contributor guide](../CONTRIBUTING.md). Restore research prerequisites from [the development environment](DEVELOPMENT_ENVIRONMENT.md) if working on another machine. Keep local reference binaries and evidence out of the repository.

Version 0.2.1 updates the desktop artwork, screenshots, and Windows launch instructions. Version 0.2.0 added stripped ELF segment fallback coverage and PE unwind-table seeding. The next engine task is to reconcile chained unwind ranges and tail calls, then expand typed instruction semantics and non-leaf decompilation on independently compiled fixtures. The [paid workflow inventory](PRO_FEATURES.md) also identifies the remaining debugger, type library, plugin, and workspace work. Update the feature matrix and test report with results.
