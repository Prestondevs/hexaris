# Implementation status

The Windows desktop and CLI share local binary analysis, address mapping, disassembly, PE unwind and export seeding, CFG discovery, indexed direct references, triage, byte and ASCII searches, register SSA, and portable projects with annotations and bookmarks. A constrained Windows x64 symbolic evaluator recovers unsigned 32-bit return expressions for supported leaf functions.

The original comparison fixture was compiled independently and analyzed with Ghidra through REA. The public inventory links to documented reference workflows. [Research workflow](REA_INVESTIGATIONS.md)

The desktop has multiple document tabs, a file entropy overview, a direct call browser and five saved themes. The CLI has ordered batch triage. Both import names, comments and bookmarks from Ghidra program XML.

General decompilation, wider architecture support, debugger backends, types, live editing, plugins, persisted multi-file workspaces, broader tests, and Linux and macOS validation are tracked as remaining work. [Feature matrix](FEATURE_MATRIX.md) · [Commercial and Ultimate workflows](PRO_FEATURES.md)
