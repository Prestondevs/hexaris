# Implementation status

The Windows desktop and CLI share local binary analysis, address mapping, disassembly, function and CFG discovery, direct references, byte and ASCII searches, register SSA, and portable projects with annotations. A constrained Windows x64 symbolic evaluator recovers unsigned 32-bit return expressions for supported leaf functions.

The original comparison fixture was compiled independently and analyzed with Ghidra through REA. The public inventory links to documented reference workflows. [Research workflow](REA_INVESTIGATIONS.md)

General decompilation, instruction coverage beyond the current projection, wider architecture support, debugger backends, types, editing, plugins, workspace configuration, broader tests, and platform packaging are tracked as remaining work. [Feature matrix](FEATURE_MATRIX.md)
