//! Read-only loaders and static analysis for Hexaris.

mod analysis;
mod assistant;
mod comparison;
mod decompile;
mod decompile_listing;
mod ghidra_xml;
mod image;
mod ir;
mod patch;
mod project;
mod references;
mod search;
mod triage;

pub use analysis::{
    Analysis, Block, Edge, EdgeKind, Function, Instruction, InstructionToken, LinearListing,
    Reference, ReferenceKind, analyze, analyze_function, disassemble,
};
pub use assistant::{
    AnnotationProposal, AssistantFinding, AssistantResponse, CONTEXT_LIMIT, ContextAddress,
    FunctionContext, ImageIdentity, MAX_RESPONSE_BYTES, ProposalKind,
};
pub use comparison::{BuildComparison, ChangeKind, FunctionChange, MatchBasis, compare_builds};
pub use decompile::{Decompilation, decompile};
pub use decompile_listing::{ControlFlowCode, PseudoLine};
pub use ghidra_xml::{GhidraImport, import_ghidra_xml};
pub use image::{
    Architecture, Export, Image, ImageInfo, Import, LoadMode, Region, Symbol, UnwindFunction,
    parse_address,
};
pub use ir::{LiftedBlock, LiftedFunction, LiftedStatement, RegisterIncoming, RegisterInput, lift};
pub use patch::{BytePatch, write_patched_copy};
pub use project::Project;
pub use references::{CallSite, ReferenceIndex};
pub use search::{BytePattern, StringItem, scan_strings};
pub use triage::{EntropySample, ImageTriage, triage};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Input(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("binary metadata: {0}")]
    Binary(#[from] object::Error),
    #[error("project metadata: {0}")]
    Json(#[from] serde_json::Error),
}
