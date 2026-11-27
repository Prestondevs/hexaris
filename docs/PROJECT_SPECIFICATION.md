# Project specification

Hexaris provides independent, offline binary analysis through a native desktop workstation, a headless CLI, and a public Rust engine. Target platforms are Windows, Linux, and macOS. Windows x86-64 is the first validation target.

Primary workflows are loading and mapping a binary, identifying and navigating machine code, interpreting control and data flow, analyzing higher-level semantics, and saving reusable projects. Later workflows include debugging authorized local processes, reversible binary edits, plug-in extensions, and wider format and ISA support. Each feature needs documented behavior, an implementation, and evidence. [Feature matrix](FEATURE_MATRIX.md) · [Architecture](ARCHITECTURE.md)
