use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use iced_x86::{Mnemonic, OpKind, Register};
use serde::Serialize;

use crate::Function;

#[derive(Debug, Serialize)]
pub struct LiftedFunction {
    pub blocks: Vec<LiftedBlock>,
    pub register_pseudocode: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LiftedBlock {
    pub address: u64,
    pub inputs: Vec<RegisterInput>,
    pub statements: Vec<LiftedStatement>,
}

#[derive(Debug, Serialize)]
pub struct RegisterInput {
    pub name: String,
    pub incoming: Vec<RegisterIncoming>,
}

#[derive(Debug, Serialize)]
pub struct RegisterIncoming {
    /// `None` identifies an external entry to the observed graph.
    pub predecessor: Option<u64>,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct LiftedStatement {
    pub address: u64,
    pub text: String,
    pub supported: bool,
}

/// Projects low 32-bit GPR values into SSA over the observed control-flow graph.
/// Flags, memory and stack effects are outside this model. Unmodeled
/// instructions conservatively invalidate the register state.
pub fn lift(function: &Function) -> LiftedFunction {
    let mut result = LiftedFunction {
        blocks: Vec::new(),
        register_pseudocode: None,
    };
    let mut states = BTreeMap::new();
    let mut registers = BTreeSet::new();
    let single = function.blocks.len() == 1;
    for block in &function.blocks {
        let prefix = if single {
            String::new()
        } else {
            format!("b{:x}_", block.address)
        };
        let mut state = RegisterState {
            prefix,
            ..Default::default()
        };
        let mut statements = Vec::new();
        let mut pseudocode = String::new();
        let mut has_return = false;
        let mut complete = single && function.issues.is_empty() && block.successors.is_empty();
        for instruction in &function.instructions
            [block.first_instruction..block.first_instruction + block.instruction_count]
        {
            let decoded = &instruction.decoded;
            let (text, supported) = if decoded.mnemonic() == Mnemonic::Nop {
                ("nop".into(), true)
            } else if decoded.is_jcc_short_or_near() || decoded.is_jmp_short_or_near() {
                (
                    format!("branch                  {}", instruction.text),
                    true,
                )
            } else if decoded.mnemonic() == Mnemonic::Ret && decoded.op_count() == 0 {
                let value = state.read(Register::EAX);
                let _ = writeln!(pseudocode, "    return {value};");
                has_return = true;
                (format!("return {value}"), true)
            } else if let Some((destination, expression)) = assignment(decoded, &mut state) {
                let name = state.define(destination);
                let _ = writeln!(pseudocode, "    uint32_t {name} = {expression};");
                (format!("{name} = {expression}"), true)
            } else {
                state.invalidate();
                complete = false;
                (
                    format!(
                        "opaque {}{}     {}",
                        state.prefix, state.barriers, instruction.text
                    ),
                    false,
                )
            };
            statements.push(LiftedStatement {
                address: instruction.address,
                text,
                supported,
            });
        }
        if complete && has_return {
            let parameters = state
                .inputs
                .iter()
                .map(|&register| format!("uint32_t {}", state.input_name(register)))
                .collect::<Vec<_>>()
                .join(", ");
            let parameters = if parameters.is_empty() {
                "void"
            } else {
                &parameters
            };
            result.register_pseudocode = Some(format!(
                "uint32_t sub_{:X}({parameters}) {{\n{pseudocode}}}",
                function.address
            ));
        }
        result.blocks.push(LiftedBlock {
            address: block.address,
            inputs: Vec::new(),
            statements,
        });
        registers.extend(state.used.iter().copied());
        states.insert(block.address, state);
    }
    let mut predecessors: BTreeMap<u64, BTreeSet<u64>> = BTreeMap::new();
    for block in &function.blocks {
        for edge in &block.successors {
            if states.contains_key(&edge.target) {
                predecessors
                    .entry(edge.target)
                    .or_default()
                    .insert(block.address);
            }
        }
    }
    for block in &mut result.blocks {
        let state = &states[&block.address];
        let parents = predecessors.get(&block.address);
        for &register in &registers {
            let mut incoming = Vec::new();
            if block.address == function.address || parents.is_none() {
                incoming.push(RegisterIncoming {
                    predecessor: None,
                    value: format!("{register:?}_entry").to_ascii_lowercase(),
                });
            }
            for &parent in parents.into_iter().flatten() {
                incoming.push(RegisterIncoming {
                    predecessor: Some(parent),
                    value: states[&parent].output_name(register),
                });
            }
            block.inputs.push(RegisterInput {
                name: state.input_name(register),
                incoming,
            });
        }
    }
    result
}

#[derive(Default)]
struct RegisterState {
    values: BTreeMap<Register, String>,
    prefix: String,
    inputs: BTreeSet<Register>,
    used: BTreeSet<Register>,
    next_id: usize,
    barriers: usize,
}

impl RegisterState {
    fn read(&mut self, register: Register) -> String {
        self.used.insert(register);
        if self.barriers == 0 && !self.values.contains_key(&register) {
            self.inputs.insert(register);
        }
        self.output_name(register)
    }

    fn input_name(&self, register: Register) -> String {
        format!("{}{register:?}_in", self.prefix).to_ascii_lowercase()
    }

    fn output_name(&self, register: Register) -> String {
        match self.values.get(&register) {
            Some(value) => value.clone(),
            None if self.barriers == 0 => self.input_name(register),
            None => format!("{}unknown_{}_{register:?}", self.prefix, self.barriers)
                .to_ascii_lowercase(),
        }
    }

    fn define(&mut self, register: Register) -> String {
        self.used.insert(register);
        self.next_id += 1;
        let name = format!("{}{register:?}_{}", self.prefix, self.next_id).to_ascii_lowercase();
        self.values.insert(register, name.clone());
        name
    }

    fn invalidate(&mut self) {
        self.values.clear();
        self.barriers += 1;
    }
}

fn assignment(
    decoded: &iced_x86::Instruction,
    state: &mut RegisterState,
) -> Option<(Register, String)> {
    if decoded.op_count() != 2 || decoded.op0_kind() != OpKind::Register {
        return None;
    }
    let destination = decoded.op0_register();
    if !destination.is_gpr32() || destination == Register::ESP {
        return None;
    }
    let operator = match decoded.mnemonic() {
        Mnemonic::Mov => None,
        Mnemonic::Add => Some("+"),
        Mnemonic::Sub => Some("-"),
        Mnemonic::And => Some("&"),
        Mnemonic::Or => Some("|"),
        Mnemonic::Xor
            if decoded.op1_kind() == OpKind::Register && destination == decoded.op1_register() =>
        {
            return Some((destination, "0u".into()));
        }
        Mnemonic::Xor => Some("^"),
        _ => return None,
    };
    let source = match decoded.op1_kind() {
        OpKind::Register if decoded.op1_register().is_gpr32() => state.read(decoded.op1_register()),
        OpKind::Immediate32 | OpKind::Immediate8to32 => {
            format!("0x{:X}u", decoded.immediate(1) as u32)
        }
        _ => return None,
    };
    let expression = match operator {
        Some(operator) => format!("{} {operator} {source}", state.read(destination)),
        None => source,
    };
    Some((destination, expression))
}
