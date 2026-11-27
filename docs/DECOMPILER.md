# Pseudocode coverage

Hexaris emits C-like register pseudocode for a single complete basic block containing supported 32-bit register operations and an operandless near return. It treats live-in registers as explicit values and shows the low 32 bits of EAX at return. This is a register projection and makes no claim about source-level signatures, return types, side effects, calling conventions, or wider system state.

The CLI exposes this field through `analyze --address ADDRESS --lift`. In the desktop, expand **Register pseudocode (EAX return)** in the register IR view when present. It is absent for unsupported or multi-block functions.

The **Pseudo-C** view and the CLI `decompile` command support a bounded structured projection for Windows and System V x86-64 leaf functions. It evaluates supported 32-bit register operations and Windows ABI home slots, follows direct CFG edges, and emits supported signed and unsigned comparisons. The pass requires a fully decoded acyclic function, supported effects on each path, and a known EAX at each return. It uses inferred register arguments and models a 32-bit result. Calls, indirect flow, arbitrary memory, other flag effects, loops, stack-pointer changes, and nonvolatile writes are outside this projection.

The **Control-flow** view and the CLI’s `control_flow` field provide an address-linked, register-level listing for decoded x86 and x86-64 functions, including ones with loops and calls. Conditional and direct branches point to local block labels or to an address outside the listing. The listing marks unresolved instruction effects and stops after 1,024 instructions. General type, variable, and side-effect recovery remain open. [Feature matrix](FEATURE_MATRIX.md)
