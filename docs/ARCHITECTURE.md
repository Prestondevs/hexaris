# Architecture

`hexaris-engine` owns the image model, address mapping, analysis, register lifting, pattern and string scans, and project serialization. The CLI and desktop both call this public Rust API. `object` parses executable metadata and `iced-x86` decodes instructions. Function discovery, control-flow construction, lifting, and persistence are implemented in the engine.

`hexaris-cli` writes JSON for metadata, disassembly, analysis, string scans, and matches. It writes diagnostics to stderr and returns a failure status on an invalid request. Its `project` subcommands operate on the same project format as the desktop.

`hexaris-desktop` uses eframe with an OpenGL backend and native file dialogs. File parsing and search execute on worker threads. A successful open replaces the current session, and a failed open leaves the previous session available. The project and disassembly keep one shared immutable backing buffer. The UI virtualizes the disassembly, hex, symbol, import, and result lists.

No development tool or proprietary component is linked into the runtime. [Analysis engine](ANALYSIS_ENGINE.md)
