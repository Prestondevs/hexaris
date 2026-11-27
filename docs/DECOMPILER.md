# Pseudocode coverage

Hexaris emits C-like register pseudocode for a single complete basic block containing supported 32-bit register operations and an operandless near return. It treats live-in registers as explicit values and shows the low 32 bits of EAX at return. This is a register projection and makes no claim about source-level signatures, return types, side effects, calling conventions, or wider system state.

The CLI exposes this field through `analyze --address ADDRESS --lift`. In the desktop, expand **Register pseudocode (EAX return)** in the register IR view when present. It is absent for unsupported or multi-block functions.

The **Pseudo-C** view and the CLI `decompile` command also support a bounded Windows x64 leaf-function projection. They symbolically evaluate supported 32-bit register operations and fixed ABI home slots, follow direct CFG edges, and emit unsigned conditionals from a known `CMP`. The pass requires a fully decoded acyclic function, supported instructions on every path, and a known EAX at each return. It refuses calls, indirect flow, arbitrary memory, unsupported flags, loops, stack-pointer changes and nonvolatile register writes. The output models the 32-bit result and uses inferred register arguments. Wider values, source types, full side effects, and general variable recovery remain open. [Feature matrix](FEATURE_MATRIX.md)
