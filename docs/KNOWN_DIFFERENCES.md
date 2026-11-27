# Known differences

The reference combines code, padding, and typed data in its linear view. Hexaris has separate bounded linear decoding and a file-offset hex view. The graph layout has a 200-block display limit and focuses on the selected function. Type and data views remain open.

The reference's recovered intermediate languages model memory and flags and emit typed decompilation across many architectures. Hexaris currently models low 32-bit register state and a small class of Windows x64 leaf functions. Project files embed original bytes and annotations and recompute the analysis graph at open.

The desktop can keep several binaries or projects open, with resizable side panels, per-tab navigation state and five named palettes. Native debugging, plugin hosting, detachable docking, custom palette editing and saving the entire multi-file workspace remain open. [Feature matrix](FEATURE_MATRIX.md)

Byte patch export replaces a fixed-length range in a new file after checking its expected bytes. In-place editing, assembly, undo and automatic reanalysis of edits remain open.
