//! Read-only loaders and static analysis for Hexaris.

mod analysis;
mod decompile;
mod image;
mod ir;
mod patch;
mod project;
mod search;

pub use analysis::{
    Analysis, Block, Edge, EdgeKind, Function, Instruction, LinearListing, Reference,
    ReferenceKind, analyze, analyze_function, disassemble,
};
pub use decompile::{Decompilation, decompile};
pub use image::{
    Architecture, Export, Image, ImageInfo, Import, LoadMode, Region, Symbol, parse_address,
};
pub use ir::{LiftedBlock, LiftedFunction, LiftedStatement, RegisterIncoming, RegisterInput, lift};
pub use patch::BytePatch;
pub use project::Project;
pub use search::{BytePattern, StringItem, scan_strings};

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
