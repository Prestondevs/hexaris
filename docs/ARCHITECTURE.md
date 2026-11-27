# Architecture

`hexaris-engine` owns the image model, address mapping, analysis, register lifting, pattern and string scans, and project serialization. The CLI and desktop both call this public Rust API. `object` parses executable metadata and `iced-x86` decodes instructions. Function discovery, control-flow construction, lifting, and persistence are implemented in the engine.

`hexaris-cli` writes JSON for metadata, disassembly, analysis, string scans, and matches. It writes diagnostics to stderr and returns a failure status on an invalid request. Its `project` subcommands operate on the same project format as the desktop.

Function context and build comparison are engine data structures shared by both clients. Context exports use hexadecimal address strings and a versioned image identity. The CLI's local MCP server opens one user-selected image at startup and answers read-only requests over stdio. The desktop imports assistant findings and proposals for a matching image, then applies individual accepted annotations through the existing project API. [User guide](USER_GUIDE.md#work-with-an-assistant)

`hexaris-desktop` uses eframe with an OpenGL backend, native file dialogs, and persisted appearance settings. File parsing, triage and byte search execute on worker threads. Opening an image adds a tab or focuses its existing tab. Each tab has its own project, selection, history and search state. A failed open leaves the other tabs available. The project and disassembly keep one shared immutable backing buffer. The UI virtualizes the disassembly, hex, symbol, import, call, and result lists.

No development tool or proprietary component is linked into the runtime. [Analysis engine](ANALYSIS_ENGINE.md)
