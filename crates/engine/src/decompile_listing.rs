use std::collections::BTreeSet;

use iced_x86::{ConditionCode, FlowControl, Mnemonic, OpKind};
use serde::Serialize;

use crate::{EdgeKind, Function, Instruction};

const INSTRUCTION_LIMIT: usize = 1_024;

#[derive(Debug, Serialize)]
pub struct ControlFlowCode {
    pub lines: Vec<PseudoLine>,
    pub truncated: bool,
}

impl ControlFlowCode {
    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug, Serialize)]
pub struct PseudoLine {
    pub address: Option<u64>,
    pub text: String,
}

pub(crate) fn render(function: &Function) -> Option<ControlFlowCode> {
    if function.blocks.is_empty() {
        return None;
    }
    let mut count = 0;
    let visible: BTreeSet<_> = function
        .blocks
        .iter()
        .take_while(|block| {
            let include = count < INSTRUCTION_LIMIT;
            count += block.instruction_count;
            include
        })
        .map(|block| block.address)
        .collect();
    let mut lines = vec![PseudoLine {
        address: None,
        text: format!(
            "/* sub_{:X}: register-level pseudocode */",
            function.address
        ),
    }];
    let mut remaining = INSTRUCTION_LIMIT;
    for block in function
        .blocks
        .iter()
        .take_while(|block| visible.contains(&block.address))
    {
        let destination = |target| {
            if visible.contains(&target) {
                format!("goto loc_{target:X}")
            } else {
                format!("transfer(0x{target:X})")
            }
        };
        lines.push(PseudoLine {
            address: Some(block.address),
            text: format!("loc_{:X}:", block.address),
        });
        let instructions = &function.instructions
            [block.first_instruction..block.first_instruction + block.instruction_count];
        for instruction in instructions.iter().take(remaining) {
            let target = |kind| {
                block
                    .successors
                    .iter()
                    .find(|edge| edge.kind == kind)
                    .map(|edge| destination(edge.target))
            };
            let decoded = &instruction.decoded;
            let text = match decoded.flow_control() {
                FlowControl::ConditionalBranch => {
                    let condition = if decoded.is_jcc_short_or_near() {
                        branch_condition(decoded.condition_code()).to_owned()
                    } else {
                        format!("condition({:?})", decoded.mnemonic())
                    };
                    let taken = target(EdgeKind::Taken).unwrap_or_else(|| {
                        format!("transfer(0x{:X})", decoded.near_branch_target())
                    });
                    let next = target(EdgeKind::Fallthrough)
                        .unwrap_or_else(|| format!("transfer(0x{:X})", decoded.next_ip()));
                    format!("if ({condition}) {{ {taken}; }} else {{ {next}; }}")
                }
                FlowControl::UnconditionalBranch if decoded.is_jmp_short_or_near() => {
                    let jump = destination(decoded.near_branch_target());
                    format!("{jump};")
                }
                FlowControl::Call if decoded.is_call_near() => {
                    format!(
                        "call(0x{:X}); /* effects unresolved */",
                        decoded.near_branch_target()
                    )
                }
                FlowControl::Return
                    if decoded.mnemonic() == Mnemonic::Ret && decoded.op_count() == 0 =>
                {
                    "return; /* return value unresolved */".to_owned()
                }
                _ => describe(instruction),
            };
            lines.push(PseudoLine {
                address: Some(instruction.address),
                text: format!("    {text}"),
            });
        }
        if instructions.len() <= remaining
            && let [edge] = block.successors.as_slice()
            && edge.kind == EdgeKind::Fallthrough
            && !matches!(
                instructions
                    .last()
                    .map(|instruction| instruction.decoded.flow_control()),
                Some(FlowControl::ConditionalBranch)
            )
        {
            lines.push(PseudoLine {
                address: None,
                text: format!("    {};", destination(edge.target)),
            });
        }
        remaining = remaining.saturating_sub(instructions.len());
    }
    let truncated = function.instructions.len() > INSTRUCTION_LIMIT;
    if truncated {
        lines.push(PseudoLine {
            address: None,
            text: format!("/* Listing stopped after {INSTRUCTION_LIMIT} instructions. */"),
        });
    }
    Some(ControlFlowCode { lines, truncated })
}

fn describe(instruction: &Instruction) -> String {
    let decoded = &instruction.decoded;
    if decoded.op_count() == 2
        && matches!(decoded.op0_kind(), OpKind::Register | OpKind::Memory)
        && !decoded.has_lock_prefix()
        && !decoded.has_rep_prefix()
        && !decoded.has_repne_prefix()
        && let Some((_, operands)) = instruction.text.split_once(' ')
        && let Some((left, right)) = operands.trim().split_once(',')
    {
        let (left, right) = (left.trim(), right.trim());
        match decoded.mnemonic() {
            Mnemonic::Mov => return format!("{left} = {right};"),
            Mnemonic::Lea
                if decoded.op0_kind() == OpKind::Register
                    && decoded.op1_kind() == OpKind::Memory =>
            {
                return format!("{left} = address_of({right});");
            }
            Mnemonic::Cmp => return format!("flags = cmp({left}, {right});"),
            Mnemonic::Test => return format!("flags = test({left}, {right});"),
            Mnemonic::Add | Mnemonic::Sub | Mnemonic::And | Mnemonic::Or | Mnemonic::Xor => {
                let operation = match decoded.mnemonic() {
                    Mnemonic::Add => "add",
                    Mnemonic::Sub => "sub",
                    Mnemonic::And => "and",
                    Mnemonic::Or => "or",
                    _ => "xor",
                };
                return format!("({left}, flags) = {operation}({left}, {right});");
            }
            _ => {}
        }
    }
    format!(
        "instruction({:?}); /* effects unresolved */",
        instruction.text
    )
}

fn branch_condition(condition: ConditionCode) -> &'static str {
    match condition {
        ConditionCode::o => "flags.OF",
        ConditionCode::no => "!flags.OF",
        ConditionCode::b => "flags.CF",
        ConditionCode::ae => "!flags.CF",
        ConditionCode::e => "flags.ZF",
        ConditionCode::ne => "!flags.ZF",
        ConditionCode::be => "flags.CF || flags.ZF",
        ConditionCode::a => "!flags.CF && !flags.ZF",
        ConditionCode::s => "flags.SF",
        ConditionCode::ns => "!flags.SF",
        ConditionCode::p => "flags.PF",
        ConditionCode::np => "!flags.PF",
        ConditionCode::l => "flags.SF != flags.OF",
        ConditionCode::ge => "flags.SF == flags.OF",
        ConditionCode::le => "flags.ZF || flags.SF != flags.OF",
        ConditionCode::g => "!flags.ZF && flags.SF == flags.OF",
        ConditionCode::None => "unknown_condition()",
    }
}
