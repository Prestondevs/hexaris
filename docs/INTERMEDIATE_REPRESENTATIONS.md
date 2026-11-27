# Intermediate representations

The register view projects low 32-bit general-purpose register values into SSA over the discovered control-flow graph. An assignment creates a new value. Block inputs have incoming values keyed by predecessor addresses. The function entry has an external input and can also have a loop backedge. The form retains pass-through values and trivial φ nodes so the relationships are explicit.

Supported arithmetic uses unsigned 32-bit bit-vector semantics. Near direct jumps preserve projected register values. The model has no flag values, stack or heap memory, calling convention, segment state, or 64-bit general-purpose values. Opaque instructions create fresh unknown register values. The model describes the observed CFG and inherits gaps from indirect or truncated control flow.

See [instruction semantics](INSTRUCTION_SEMANTICS.md) and [pseudocode scope](DECOMPILER.md).
