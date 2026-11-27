use std::collections::{BTreeMap, BTreeSet};

use iced_x86::{Decoder, DecoderOptions, FlowControl, Formatter, IntelFormatter, OpKind};
use serde::Serialize;

use crate::{Error, Image, Result};

const FUNCTION_LIMIT: usize = 4_096;
const INSTRUCTION_LIMIT: usize = 8_192;
const TOTAL_LIMIT: usize = 100_000;

#[derive(Debug, Serialize)]
pub struct Analysis {
    pub functions: BTreeMap<u64, Function>,
    pub notices: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Function {
    pub address: u64,
    pub name: String,
    pub instructions: Vec<Instruction>,
    pub blocks: Vec<Block>,
    pub references: Vec<Reference>,
    pub issues: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Instruction {
    pub address: u64,
    pub file_offset: usize,
    pub bytes: Vec<u8>,
    pub text: String,
    #[serde(skip)]
    pub(crate) decoded: iced_x86::Instruction,
}

#[derive(Debug, Serialize)]
pub struct LinearListing {
    pub start: u64,
    pub instructions: Vec<Instruction>,
    pub invalid_at: Option<u64>,
    pub next_address: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct Block {
    pub address: u64,
    pub first_instruction: usize,
    pub instruction_count: usize,
    pub successors: Vec<Edge>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct Edge {
    pub target: u64,
    pub kind: EdgeKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum EdgeKind {
    Fallthrough,
    Taken,
    Jump,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ReferenceKind {
    Call,
    Branch,
    Address,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Reference {
    pub source: u64,
    pub target: u64,
    pub kind: ReferenceKind,
}

pub fn analyze(image: &Image) -> Analysis {
    let mut analysis = Analysis {
        functions: BTreeMap::new(),
        notices: Vec::new(),
    };
    if image.info.is_relocatable || image.info.architecture.x86_bitness().is_none() {
        analysis
            .notices
            .push("Metadata inspection only for this image".into());
        return analysis;
    }
    let mut seeds = function_seeds(image);
    let mut pending: BTreeSet<_> = seeds.keys().copied().collect();
    let mut remaining = TOTAL_LIMIT;
    if pending.is_empty() {
        analysis.notices.push(
            "No executable entry point or function symbols. Choose a code address to analyze."
                .into(),
        );
    }
    while let Some(address) = pending.pop_first() {
        if analysis.functions.len() == FUNCTION_LIMIT || remaining == 0 {
            analysis
                .notices
                .push("Automatic analysis reached its work limit".into());
            break;
        }
        let function = decode_function(image, address, &seeds, remaining.min(INSTRUCTION_LIMIT));
        remaining -= function.instructions.len();
        for reference in &function.references {
            if reference.kind == ReferenceKind::Call
                && image.executable_bytes(reference.target).is_some()
                && !seeds.contains_key(&reference.target)
                && seeds.len() < FUNCTION_LIMIT
            {
                seeds.insert(reference.target, Seed::default());
                pending.insert(reference.target);
            }
        }
        analysis.functions.insert(address, function);
    }
    analysis
}

pub fn analyze_function(image: &Image, address: u64) -> Result<Function> {
    if image.info.architecture.x86_bitness().is_none() || image.info.is_relocatable {
        return Err(Error::Input(
            "function analysis requires a mapped x86 or x86-64 image".into(),
        ));
    }
    if image.executable_bytes(address).is_none() {
        return Err(Error::Input(format!(
            "0x{address:X} does not map to executable bytes"
        )));
    }
    Ok(decode_function(
        image,
        address,
        &function_seeds(image),
        INSTRUCTION_LIMIT,
    ))
}

/// Decode sequentially within one file-backed executable region, without
/// classifying reached bytes as functions or following control transfers.
pub fn disassemble(image: &Image, address: u64, count: usize) -> Result<LinearListing> {
    if !(1..=INSTRUCTION_LIMIT).contains(&count) {
        return Err(Error::Input(format!(
            "instruction count must be between 1 and {INSTRUCTION_LIMIT}"
        )));
    }
    let bitness = image
        .info
        .architecture
        .x86_bitness()
        .ok_or_else(|| Error::Input("disassembly requires x86 or x86-64".into()))?;
    let bytes = image
        .executable_bytes(address)
        .ok_or_else(|| Error::Input(format!("0x{address:X} does not map to executable bytes")))?;
    let offset = image
        .file_offset(address)
        .expect("executable bytes are mapped");
    let mut decoder = Decoder::with_ip(bitness, bytes, address, DecoderOptions::NONE);
    let mut formatter = intel_formatter();
    let mut listing = LinearListing {
        start: address,
        instructions: Vec::with_capacity(count.min(bytes.len())),
        invalid_at: None,
        next_address: None,
    };
    while listing.instructions.len() < count && decoder.can_decode() {
        let position = decoder.position();
        let instruction = decoder.decode();
        let Some(ip) = address.checked_add(position as u64) else {
            break;
        };
        if instruction.is_invalid() {
            listing.invalid_at = Some(ip);
            return Ok(listing);
        }
        listing.instructions.push(format_instruction(
            &mut formatter,
            instruction,
            offset + position,
            &bytes[position..decoder.position()],
        ));
    }
    if decoder.can_decode() {
        listing.next_address = address.checked_add(decoder.position() as u64);
    }
    Ok(listing)
}

fn intel_formatter() -> IntelFormatter {
    let mut formatter = IntelFormatter::new();
    formatter.options_mut().set_hex_prefix("0x");
    formatter.options_mut().set_hex_suffix("");
    formatter.options_mut().set_uppercase_hex(false);
    formatter
}

fn format_instruction(
    formatter: &mut IntelFormatter,
    decoded: iced_x86::Instruction,
    file_offset: usize,
    bytes: &[u8],
) -> Instruction {
    let mut text = String::new();
    formatter.format(&decoded, &mut text);
    Instruction {
        address: decoded.ip(),
        file_offset,
        bytes: bytes.to_vec(),
        text,
        decoded,
    }
}

#[derive(Default)]
struct Seed<'a> {
    name: Option<&'a str>,
    end: Option<u64>,
}

fn function_seeds(image: &Image) -> BTreeMap<u64, Seed<'_>> {
    let mut seeds = BTreeMap::new();
    if let Some(entry) = image
        .info
        .entry
        .filter(|&address| image.executable_bytes(address).is_some())
    {
        seeds.insert(
            entry,
            Seed {
                name: Some("entry"),
                end: None,
            },
        );
    }
    for symbol in &image.info.symbols {
        if symbol.is_function && image.executable_bytes(symbol.address).is_some() {
            let seed = seeds.entry(symbol.address).or_default();
            if seed.end.is_none() {
                seed.name = Some(&symbol.name);
                seed.end = (symbol.size > 0)
                    .then(|| symbol.address.checked_add(symbol.size))
                    .flatten();
            }
        }
    }
    seeds
}

fn decode_function(
    image: &Image,
    entry: u64,
    seeds: &BTreeMap<u64, Seed<'_>>,
    limit: usize,
) -> Function {
    let seed = seeds.get(&entry);
    let end = seed.and_then(|seed| seed.end);
    let name = seed
        .and_then(|seed| seed.name)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("sub_{entry:X}"));
    let mut formatter = intel_formatter();
    let mut pending = BTreeSet::from([entry]);
    let mut decoded: BTreeMap<u64, Instruction> = BTreeMap::new();
    let mut leaders = BTreeSet::from([entry]);
    let mut issues = BTreeSet::new();
    let mut references = BTreeSet::new();
    let bitness = image
        .info
        .architecture
        .x86_bitness()
        .expect("required by caller");
    while let Some(address) = pending.pop_first() {
        if decoded.contains_key(&address) {
            continue;
        }
        if decoded.len() == limit {
            issues.insert("Instruction budget exhausted".to_owned());
            break;
        }
        if end.is_some_and(|size_end| address < entry || address >= size_end) {
            continue;
        }
        if address != entry && seeds.contains_key(&address) {
            continue;
        }
        let Some(bytes) = image.executable_bytes(address) else {
            continue;
        };
        let available = end.map_or(bytes.len(), |end| {
            usize::try_from(end - address)
                .unwrap_or(usize::MAX)
                .min(bytes.len())
        });
        let instruction =
            Decoder::with_ip(bitness, &bytes[..available], address, DecoderOptions::NONE).decode();
        if instruction.is_invalid() {
            issues.insert(format!("Invalid or truncated instruction at 0x{address:X}"));
            continue;
        }
        let prior = decoded.range(..address).next_back();
        let next = decoded.range(address..).next();
        if prior.is_some_and(|(start, i)| address - start < i.bytes.len() as u64)
            || next.is_some_and(|(start, _)| start - address < instruction.len() as u64)
        {
            issues.insert(format!("Overlapping instruction at 0x{address:X}"));
            continue;
        }
        let file_offset = image.file_offset(address).expect("decoded mapped bytes");
        decoded.insert(
            address,
            format_instruction(
                &mut formatter,
                instruction,
                file_offset,
                &bytes[..instruction.len()],
            ),
        );

        if let Some(target) = direct_target(&instruction) {
            let kind = if instruction.flow_control() == FlowControl::Call {
                ReferenceKind::Call
            } else {
                ReferenceKind::Branch
            };
            references.insert(Reference {
                source: address,
                target,
                kind,
            });
        }
        if instruction.is_ip_rel_memory_operand() {
            references.insert(Reference {
                source: address,
                target: instruction.ip_rel_memory_address(),
                kind: ReferenceKind::Address,
            });
        }
        for edge in successors(&instruction) {
            pending.insert(edge.target);
            if matches!(
                instruction.flow_control(),
                FlowControl::ConditionalBranch | FlowControl::UnconditionalBranch
            ) {
                leaders.insert(edge.target);
            }
        }
    }
    let instructions: Vec<_> = decoded.into_values().collect();
    let blocks = build_blocks(&instructions, &leaders);
    Function {
        address: entry,
        name,
        instructions,
        blocks,
        references: references.into_iter().collect(),
        issues: issues.into_iter().collect(),
    }
}

fn direct_target(instruction: &iced_x86::Instruction) -> Option<u64> {
    matches!(
        instruction.op0_kind(),
        OpKind::NearBranch16 | OpKind::NearBranch32 | OpKind::NearBranch64
    )
    .then(|| instruction.near_branch_target())
}

fn fallthrough(instruction: &iced_x86::Instruction) -> Option<Edge> {
    instruction
        .ip()
        .checked_add(instruction.len() as u64)
        .map(|target| Edge {
            target,
            kind: EdgeKind::Fallthrough,
        })
}

fn successors(instruction: &iced_x86::Instruction) -> impl Iterator<Item = Edge> {
    let jump = |kind| direct_target(instruction).map(|target| Edge { target, kind });
    let (first, second) = match instruction.flow_control() {
        FlowControl::ConditionalBranch => (jump(EdgeKind::Taken), fallthrough(instruction)),
        FlowControl::UnconditionalBranch => (jump(EdgeKind::Jump), None),
        FlowControl::Next | FlowControl::Call | FlowControl::IndirectCall => {
            (fallthrough(instruction), None)
        }
        _ => (None, None),
    };
    first.into_iter().chain(second)
}

fn build_blocks(instructions: &[Instruction], leaders: &BTreeSet<u64>) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut start = 0;
    for (index, instruction) in instructions.iter().enumerate() {
        let falls_through = matches!(
            instruction.decoded.flow_control(),
            FlowControl::Next | FlowControl::Call | FlowControl::IndirectCall
        );
        let continues = instructions.get(index + 1).is_some_and(|next| {
            falls_through
                && !leaders.contains(&next.address)
                && fallthrough(&instruction.decoded).is_some_and(|edge| edge.target == next.address)
        });
        if !continues {
            blocks.push(Block {
                address: instructions[start].address,
                first_instruction: start,
                instruction_count: index - start + 1,
                successors: successors(&instruction.decoded).collect(),
            });
            start = index + 1;
        }
    }
    blocks
}
