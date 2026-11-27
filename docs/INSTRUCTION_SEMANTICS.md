# Instruction semantics

`iced-x86` decodes and formats x86 and x86-64 instructions. Register lifting is an independent pass over the decoded operands.

| Supported operation | Projected effect |
| --- | --- |
| `MOV r32, r32/imm32` | Assign the source value to the destination |
| `ADD`, `SUB`, `AND`, `OR`, `XOR` with 32-bit register destination and supported register or immediate source | Assign the result modulo 2³², extending sign-extended immediates before arithmetic |
| `XOR r32, r32` with both operands equal | Assign zero |
| `NOP` | Preserve projected values |
| Direct near `Jcc` and `JMP` | Preserve projected register values across observed edges |
| Operandless near `RET` | Display the projected EAX value at the return |

Writes to ESP and other unmodeled instructions invalidate the projected register state. Flags and full-width aliases remain outside this pass. The regression corpus covers overlapping decodes, truncated instructions, RIP-relative addressing, branches, SSA joins, backedges, and wrapping immediates.
