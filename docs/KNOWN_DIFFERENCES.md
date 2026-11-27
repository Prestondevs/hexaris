# Known differences

The reference combines code, padding, and typed data in its linear view. Hexaris has separate bounded linear decoding and a file-offset hex view. The graph layout has a 200-block display limit and focuses on the selected function. Type and data views remain open.

The reference's recovered intermediate languages model memory and flags and emit typed decompilation across many architectures. Hexaris currently models low 32-bit register state and a small class of Windows x64 leaf functions. Project files embed original bytes and annotations and recompute the analysis graph at open.

The desktop currently has one active image, resizable side panels, and a fixed dark palette. Native debugging, plugin hosting, and multi-project workspaces remain open. [Feature matrix](FEATURE_MATRIX.md)

Byte patch export replaces a fixed-length range in a new file after checking its expected bytes. In-place editing, assembly, undo and automatic reanalysis of edits remain open.
