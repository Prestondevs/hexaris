use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
    rc::Rc,
};

use iced_x86::{ConditionCode, Mnemonic, OpKind, Register};
use serde::Serialize;

use crate::{
    Architecture, ControlFlowCode, EdgeKind, Function, Image, Instruction, decompile_listing,
};

const MAX_BLOCK_VISITS: usize = 64;
const MAX_EXPRESSION_SIZE: usize = 64;
const WINDOWS_ARGUMENTS: [Register; 4] =
    [Register::ECX, Register::EDX, Register::R8D, Register::R9D];
const SYSV_ARGUMENTS: [Register; 6] = [
    Register::EDI,
    Register::ESI,
    Register::EDX,
    Register::ECX,
    Register::R8D,
    Register::R9D,
];

#[derive(Debug, Serialize)]
pub struct Decompilation {
    pub address: u64,
    pub code: Option<String>,
    pub unavailable: Option<String>,
    pub control_flow: Option<ControlFlowCode>,
}

/// Recover a structured leaf-function result and a register-level control-flow listing.
pub fn decompile(image: &Image, function: &Function) -> Decompilation {
    let result = match (image.info.architecture, image.info.format) {
        (Architecture::X86_64, "PE") => structured(function, true),
        (Architecture::X86_64, "ELF" | "Mach-O") => structured(function, false),
        _ => Err("Structured pseudocode requires a Windows or System V x86-64 ABI".into()),
    };
    let (code, unavailable) = match result {
        Ok(code) => (Some(code), None),
        Err(reason) => (None, Some(reason)),
    };
    Decompilation {
        address: function.address,
        code,
        unavailable,
        control_flow: decompile_listing::render(image, function),
    }
}

fn structured(function: &Function, windows: bool) -> Result<String, String> {
    if !function.issues.is_empty()
        || function.instructions.len() > 128
        || function.blocks.len() > 32
    {
        return Err("Structured pseudocode needs a small, fully decoded function".into());
    }
    let arguments: &[Register] = if windows {
        &WINDOWS_ARGUMENTS
    } else {
        &SYSV_ARGUMENTS
    };
    let state = State {
        windows,
        registers: arguments
            .iter()
            .enumerate()
            .map(|(index, &register)| (register, Value::argument(index)))
            .collect(),
        ..Default::default()
    };
    let mut traversal = Traversal {
        function,
        remaining: MAX_BLOCK_VISITS,
        active: BTreeSet::new(),
        arguments: 0,
    };
    let body = traversal.block(function.address, state)?;
    let arguments = (1..=traversal.arguments)
        .map(|number| format!("uint32_t arg{number}"))
        .collect::<Vec<_>>()
        .join(", ");
    let parameters = if arguments.is_empty() {
        "void"
    } else {
        &arguments
    };
    let mut code = format!("uint32_t sub_{:X}({parameters}) {{\n", function.address);
    body.write(&mut code, 1);
    code.push('}');
    Ok(code)
}

#[derive(Clone)]
struct Value {
    expression: Rc<Expression>,
    size: usize,
}

enum Expression {
    Argument(usize),
    Constant(u32),
    Binary(&'static str, Value, Value),
}

impl Value {
    fn argument(number: usize) -> Self {
        Self {
            expression: Rc::new(Expression::Argument(number)),
            size: 1,
        }
    }

    fn constant(value: u32) -> Self {
        Self {
            expression: Rc::new(Expression::Constant(value)),
            size: 1,
        }
    }

    fn binary(operator: &'static str, left: Self, right: Self) -> Option<Self> {
        let size = left.size + right.size + 1;
        (size <= MAX_EXPRESSION_SIZE).then(|| Self {
            expression: Rc::new(Expression::Binary(operator, left, right)),
            size,
        })
    }

    fn visit_arguments(&self, count: &mut usize) {
        match self.expression.as_ref() {
            Expression::Argument(index) => *count = (*count).max(index + 1),
            Expression::Constant(_) => {}
            Expression::Binary(_, left, right) => {
                left.visit_arguments(count);
                right.visit_arguments(count);
            }
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.expression.as_ref() {
            Expression::Argument(index) => write!(f, "arg{}", index + 1),
            Expression::Constant(value) => write!(f, "{value}u"),
            Expression::Binary(operator, left, right) => write!(f, "({left} {operator} {right})"),
        }
    }
}

enum Statement {
    Return(Value),
    If {
        left: Value,
        operator: &'static str,
        right: Value,
        signed: bool,
        taken: Box<Self>,
        fallthrough: Box<Self>,
    },
}

impl Statement {
    fn write(&self, output: &mut String, level: usize) {
        let indent = "    ".repeat(level);
        match self {
            Self::Return(value) => {
                let _ = writeln!(output, "{indent}return {value};");
            }
            Self::If {
                left,
                operator,
                right,
                signed,
                taken,
                fallthrough,
            } => {
                let cast = if *signed { "(int32_t)" } else { "" };
                let _ = writeln!(
                    output,
                    "{indent}if ({cast}{left} {operator} {cast}{right}) {{"
                );
                taken.write(output, level + 1);
                let _ = writeln!(output, "{indent}}} else {{");
                fallthrough.write(output, level + 1);
                let _ = writeln!(output, "{indent}}}");
            }
        }
    }
}

#[derive(Clone, Default)]
struct State {
    windows: bool,
    registers: BTreeMap<Register, Value>,
    stack_slots: BTreeMap<u64, Value>,
    comparison: Option<Comparison>,
}

#[derive(Clone)]
struct Comparison {
    left: Value,
    right: Value,
    from_test: bool,
}

impl State {
    fn operand(&self, decoded: &iced_x86::Instruction, index: u32) -> Option<Value> {
        match decoded.op_kind(index) {
            OpKind::Register => self.registers.get(&decoded.op_register(index)).cloned(),
            OpKind::Memory if self.windows => self.stack_slots.get(&home_slot(decoded)?).cloned(),
            OpKind::Immediate32 | OpKind::Immediate8to32 => {
                Some(Value::constant(decoded.immediate(index) as u32))
            }
            _ => None,
        }
    }

    fn update(&mut self, decoded: &iced_x86::Instruction) -> Option<()> {
        let mnemonic = decoded.mnemonic();
        if mnemonic == Mnemonic::Nop {
            return Some(());
        }
        if decoded.op_count() != 2 {
            return None;
        }
        let destination = decoded.op0_register();
        match mnemonic {
            Mnemonic::Cmp | Mnemonic::Test => {
                let left = self.operand(decoded, 0)?;
                let right = self.operand(decoded, 1)?;
                let from_test = mnemonic == Mnemonic::Test;
                let (left, right) = if from_test {
                    (Value::binary("&", left, right)?, Value::constant(0))
                } else {
                    (left, right)
                };
                self.comparison = Some(Comparison {
                    left,
                    right,
                    from_test,
                });
            }
            Mnemonic::Mov => {
                let value = self.operand(decoded, 1)?;
                match decoded.op0_kind() {
                    OpKind::Register if is_writable(destination, self.windows) => {
                        self.registers.insert(destination, value);
                    }
                    OpKind::Memory if self.windows => {
                        self.stack_slots.insert(home_slot(decoded)?, value);
                    }
                    _ => return None,
                }
            }
            _ if decoded.op0_kind() == OpKind::Register
                && is_writable(destination, self.windows) =>
            {
                let value = if mnemonic == Mnemonic::Xor
                    && decoded.op1_kind() == OpKind::Register
                    && decoded.op1_register() == destination
                {
                    Value::constant(0)
                } else {
                    let (operator, right) = match mnemonic {
                        Mnemonic::Add => ("+", self.operand(decoded, 1)?),
                        Mnemonic::Sub => ("-", self.operand(decoded, 1)?),
                        Mnemonic::And => ("&", self.operand(decoded, 1)?),
                        Mnemonic::Or => ("|", self.operand(decoded, 1)?),
                        Mnemonic::Xor => ("^", self.operand(decoded, 1)?),
                        Mnemonic::Shl | Mnemonic::Shr
                            if decoded.op1_kind() == OpKind::Immediate8 =>
                        {
                            let operator = if mnemonic == Mnemonic::Shl {
                                "<<"
                            } else {
                                ">>"
                            };
                            (
                                operator,
                                Value::constant(u32::from(decoded.immediate8() & 31)),
                            )
                        }
                        _ => return None,
                    };
                    Value::binary(operator, self.operand(decoded, 0)?, right)?
                };
                self.comparison = None;
                self.registers.insert(destination, value);
            }
            _ => return None,
        }
        Some(())
    }
}

fn is_writable(register: Register, windows: bool) -> bool {
    // Avoid hiding changes to callee-saved registers.
    matches!(
        register,
        Register::EAX
            | Register::ECX
            | Register::EDX
            | Register::R8D
            | Register::R9D
            | Register::R10D
            | Register::R11D
    ) || (!windows && matches!(register, Register::EDI | Register::ESI))
}

fn home_slot(decoded: &iced_x86::Instruction) -> Option<u64> {
    let offset = decoded.memory_displacement64();
    (decoded.memory_base() == Register::RSP
        && decoded.memory_index() == Register::None
        && decoded.segment_prefix() == Register::None
        && decoded.memory_size().size() == 4
        && (8..=36).contains(&offset)
        && offset.is_multiple_of(4))
    .then_some(offset)
}

struct Traversal<'a> {
    function: &'a Function,
    remaining: usize,
    active: BTreeSet<u64>,
    arguments: usize,
}

impl Traversal<'_> {
    fn block(&mut self, address: u64, state: State) -> Result<Statement, String> {
        if self.remaining == 0 || !self.active.insert(address) {
            return Err(
                "Structured pseudocode cannot represent this loop or path count yet".into(),
            );
        }
        self.remaining -= 1;
        let result = self.block_body(address, state);
        self.active.remove(&address);
        result
    }

    fn block_body(&mut self, address: u64, mut state: State) -> Result<Statement, String> {
        let block = self
            .function
            .blocks
            .iter()
            .find(|block| block.address == address)
            .ok_or_else(|| format!("Unresolved control flow to 0x{address:X}"))?;
        let instructions = &self.function.instructions
            [block.first_instruction..block.first_instruction + block.instruction_count];
        let (last, body) = instructions
            .split_last()
            .ok_or_else(|| "Empty basic block".to_owned())?;
        for instruction in body {
            state
                .update(&instruction.decoded)
                .ok_or_else(|| unsupported(instruction))?;
        }
        if last.decoded.mnemonic() == Mnemonic::Ret && last.decoded.op_count() == 0 {
            let value = state
                .registers
                .get(&Register::EAX)
                .ok_or_else(|| "Unknown EAX value at return".to_owned())?;
            value.visit_arguments(&mut self.arguments);
            return Ok(Statement::Return(value.clone()));
        }
        if last.decoded.is_jcc_short_or_near() {
            let (operator, signed) = match last.decoded.condition_code() {
                ConditionCode::b => ("<", false),
                ConditionCode::be => ("<=", false),
                ConditionCode::ae => (">=", false),
                ConditionCode::a => (">", false),
                ConditionCode::e => ("==", false),
                ConditionCode::ne => ("!=", false),
                ConditionCode::l => ("<", true),
                ConditionCode::le => ("<=", true),
                ConditionCode::ge => (">=", true),
                ConditionCode::g => (">", true),
                _ => return Err(unsupported(last)),
            };
            let Comparison {
                left,
                right,
                from_test,
            } = state.comparison.clone().ok_or_else(|| unsupported(last))?;
            if from_test && !signed && matches!(operator, "<" | "<=" | ">=" | ">") {
                return Err(unsupported(last));
            }
            left.visit_arguments(&mut self.arguments);
            right.visit_arguments(&mut self.arguments);
            let target = |kind| {
                block
                    .successors
                    .iter()
                    .find(|edge| edge.kind == kind)
                    .map(|edge| edge.target)
                    .ok_or_else(|| unsupported(last))
            };
            return Ok(Statement::If {
                left,
                operator,
                right,
                signed,
                taken: Box::new(self.block(target(EdgeKind::Taken)?, state.clone())?),
                fallthrough: Box::new(self.block(target(EdgeKind::Fallthrough)?, state)?),
            });
        }
        if !last.decoded.is_jmp_short_or_near() {
            state
                .update(&last.decoded)
                .ok_or_else(|| unsupported(last))?;
        }
        let [edge] = block.successors.as_slice() else {
            return Err(unsupported(last));
        };
        self.block(edge.target, state)
    }
}

fn unsupported(instruction: &Instruction) -> String {
    format!(
        "Unsupported effect at 0x{:X} ({})",
        instruction.address, instruction.text
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Architecture, LoadMode, analyze_function};

    #[test]
    fn recovers_an_unsigned_branch_through_a_stack_home_slot() {
        // mov [rsp+8],ecx; cmp [rsp+8],10; jb lower; mov eax,[rsp+8];
        // add eax,3; jmp exit; lower: mov eax,[rsp+8]; shl eax,1; exit: ret.
        let bytes = vec![
            0x89, 0x4c, 0x24, 0x08, 0x83, 0x7c, 0x24, 0x08, 0x0a, 0x72, 0x09, 0x8b, 0x44, 0x24,
            0x08, 0x83, 0xc0, 0x03, 0xeb, 0x06, 0x8b, 0x44, 0x24, 0x08, 0xd1, 0xe0, 0xc3,
        ];
        let image = Image::from_bytes(
            bytes,
            LoadMode::Raw {
                architecture: Architecture::X86_64,
                base: 0x1000,
            },
        )
        .unwrap();
        let function = analyze_function(&image, 0x1000).unwrap();
        let code = structured(&function, true).unwrap();
        assert_eq!(
            code,
            "uint32_t sub_1000(uint32_t arg1) {\n    if (arg1 < 10u) {\n        return (arg1 << 1u);\n    } else {\n        return (arg1 + 3u);\n    }\n}"
        );
    }

    #[test]
    fn withholds_code_for_cycles_and_external_memory() {
        let raw = |bytes| {
            Image::from_bytes(
                bytes,
                LoadMode::Raw {
                    architecture: Architecture::X86_64,
                    base: 0,
                },
            )
            .unwrap()
        };
        let looped = analyze_function(&raw(vec![0xeb, 0xfe]), 0).unwrap();
        assert!(structured(&looped, true).unwrap_err().contains("loop"));
        let out_of_scope = analyze_function(&raw(vec![0x8b, 0x01, 0xc3]), 0).unwrap();
        assert!(
            structured(&out_of_scope, true)
                .unwrap_err()
                .contains("Unsupported effect")
        );
    }

    #[test]
    fn recovers_a_signed_system_v_test() {
        // mov eax,edi; test edi,edi; jl negative; shr eax,1; negative: ret
        let image = Image::from_bytes(
            vec![0x89, 0xf8, 0x85, 0xff, 0x7c, 2, 0xd1, 0xe8, 0xc3],
            LoadMode::Raw {
                architecture: Architecture::X86_64,
                base: 0,
            },
        )
        .unwrap();
        let function = analyze_function(&image, 0).unwrap();
        let code = structured(&function, false).unwrap();
        assert!(code.contains("uint32_t arg1"));
        assert!(code.contains("(int32_t)(arg1 & arg1) < (int32_t)0u"));
        assert!(code.contains("return (arg1 >> 1u);"));
    }
}
