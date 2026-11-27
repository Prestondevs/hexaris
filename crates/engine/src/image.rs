use std::{fs::File, io::Read, ops::Range, path::Path, sync::Arc};

use object::{
    Object, ObjectSection, ObjectSegment, ObjectSymbol, SectionFlags, SegmentFlags, SymbolKind,
};
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

pub(crate) const MAX_FILE: usize = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Architecture {
    X86,
    X86_64,
    Arm,
    Aarch64,
    Mips,
    RiscV,
    PowerPc,
    Other,
}

impl Architecture {
    pub fn name(self) -> &'static str {
        match self {
            Self::X86 => "x86",
            Self::X86_64 => "x86-64",
            Self::Arm => "ARM",
            Self::Aarch64 => "AArch64",
            Self::Mips => "MIPS",
            Self::RiscV => "RISC-V",
            Self::PowerPc => "PowerPC",
            Self::Other => "Unknown",
        }
    }

    pub(crate) fn x86_bitness(self) -> Option<u32> {
        match self {
            Self::X86 => Some(32),
            Self::X86_64 => Some(64),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum LoadMode {
    #[default]
    Auto,
    Raw {
        architecture: Architecture,
        base: u64,
    },
}

#[derive(Debug, Serialize)]
pub struct Region {
    pub name: String,
    pub address: Option<u64>,
    pub size: u64,
    pub file_offset: Option<u64>,
    pub file_size: u64,
    pub executable: bool,
}

impl Region {
    fn mapping(&self, address: u64) -> Option<Range<usize>> {
        let delta = address.checked_sub(self.address?)?;
        if delta >= self.file_size.min(self.size) {
            return None;
        }
        let offset = self.file_offset?;
        let end = offset.checked_add(self.file_size.min(self.size))?;
        Some(usize::try_from(offset.checked_add(delta)?).ok()?..usize::try_from(end).ok()?)
    }
}

#[derive(Debug, Serialize)]
pub struct Symbol {
    pub address: u64,
    pub name: String,
    pub size: u64,
    pub is_function: bool,
}

#[derive(Debug, Serialize)]
pub struct Import {
    pub library: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct Export {
    pub address: u64,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct ImageInfo {
    pub format: &'static str,
    pub architecture: Architecture,
    pub bits: u8,
    pub little_endian: bool,
    pub entry: Option<u64>,
    pub image_base: Option<u64>,
    pub is_relocatable: bool,
    pub sections: Vec<Region>,
    pub segments: Vec<Region>,
    pub symbols: Vec<Symbol>,
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
    pub metadata_warnings: Vec<String>,
}

pub struct Image {
    bytes: Arc<Vec<u8>>,
    pub info: ImageInfo,
    pub mode: LoadMode,
}

impl Image {
    pub fn open(path: &Path, mode: LoadMode) -> Result<Self> {
        let file = File::open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.len() > MAX_FILE as u64 {
            return Err(Error::Input(
                "expected a regular file of at most 256 MiB".into(),
            ));
        }
        let mut bytes = Vec::new();
        file.take(MAX_FILE as u64 + 1).read_to_end(&mut bytes)?;
        Self::from_bytes(bytes, mode)
    }

    pub fn from_bytes(bytes: Vec<u8>, mode: LoadMode) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > MAX_FILE {
            return Err(Error::Input("expected between 1 byte and 256 MiB".into()));
        }
        let info = match mode {
            LoadMode::Auto => parse_object(&bytes)?,
            LoadMode::Raw { architecture, base } => {
                let bits = match architecture.x86_bitness() {
                    Some(bits) => bits,
                    None => {
                        return Err(Error::Input(
                            "raw images currently require x86 or x86-64".into(),
                        ));
                    }
                };
                let size = bytes.len() as u64;
                let end = base
                    .checked_add(size - 1)
                    .ok_or_else(|| Error::Input("raw address range overflows".into()))?;
                if bits == 32 && end > u32::MAX.into() {
                    return Err(Error::Input("raw x86 addresses must fit in 32 bits".into()));
                }
                ImageInfo {
                    format: "Raw",
                    architecture,
                    bits: bits as u8,
                    little_endian: true,
                    entry: Some(base),
                    image_base: Some(base),
                    is_relocatable: false,
                    sections: vec![Region {
                        name: "raw".into(),
                        address: Some(base),
                        size,
                        file_offset: Some(0),
                        file_size: size,
                        executable: true,
                    }],
                    segments: Vec::new(),
                    symbols: Vec::new(),
                    imports: Vec::new(),
                    exports: Vec::new(),
                    metadata_warnings: Vec::new(),
                }
            }
        };
        Ok(Self {
            bytes: Arc::new(bytes),
            info,
            mode,
        })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn shared_bytes(&self) -> Arc<Vec<u8>> {
        Arc::clone(&self.bytes)
    }

    pub fn file_offset(&self, address: u64) -> Option<usize> {
        self.mapping(address).map(|(range, _)| range.start)
    }

    pub fn virtual_address(&self, offset: usize) -> Option<u64> {
        for region in self.regions() {
            let (Some(start), Some(base)) = (region.file_offset, region.address) else {
                continue;
            };
            let Some(delta) = (offset as u64).checked_sub(start) else {
                continue;
            };
            if delta < region.file_size.min(region.size) {
                let address = base.checked_add(delta)?;
                if self.file_offset(address) == Some(offset) {
                    return Some(address);
                }
            }
        }
        None
    }

    pub(crate) fn executable_bytes(&self, address: u64) -> Option<&[u8]> {
        let (range, executable) = self.mapping(address)?;
        if !executable {
            return None;
        }
        self.bytes.get(range)
    }

    fn regions(&self) -> impl Iterator<Item = &Region> {
        self.info.sections.iter().chain(&self.info.segments)
    }

    fn mapping(&self, address: u64) -> Option<(Range<usize>, bool)> {
        self.regions().find_map(|region| {
            let range = region.mapping(address)?;
            (range.end <= self.bytes.len()).then_some((range, region.executable))
        })
    }
}

pub fn parse_address(input: &str) -> std::result::Result<u64, std::num::ParseIntError> {
    let input = input.trim();
    match input
        .strip_prefix("0x")
        .or_else(|| input.strip_prefix("0X"))
    {
        Some(hex) => u64::from_str_radix(hex, 16),
        None => input.parse(),
    }
}

fn parse_object(bytes: &[u8]) -> Result<ImageInfo> {
    let file = object::File::parse(bytes)?;
    let format = match file.format() {
        object::BinaryFormat::Pe => "PE",
        object::BinaryFormat::Elf => "ELF",
        object::BinaryFormat::MachO => "Mach-O",
        object::BinaryFormat::Coff => "COFF",
        other => {
            return Err(Error::Input(format!(
                "unsupported binary format: {other:?}"
            )));
        }
    };
    let architecture = match file.architecture() {
        object::Architecture::I386 => Architecture::X86,
        object::Architecture::X86_64 | object::Architecture::X86_64_X32 => Architecture::X86_64,
        object::Architecture::Arm => Architecture::Arm,
        object::Architecture::Aarch64 | object::Architecture::Aarch64_Ilp32 => {
            Architecture::Aarch64
        }
        object::Architecture::Mips | object::Architecture::Mips64 => Architecture::Mips,
        object::Architecture::Riscv32 | object::Architecture::Riscv64 => Architecture::RiscV,
        object::Architecture::PowerPc | object::Architecture::PowerPc64 => Architecture::PowerPc,
        _ => Architecture::Other,
    };
    let is_relocatable = file.kind() == object::ObjectKind::Relocatable;
    let sections = file
        .sections()
        .map(|section| {
            let (allocated, executable) = match section.flags() {
                SectionFlags::Elf { sh_flags } => (
                    sh_flags & u64::from(object::elf::SHF_ALLOC) != 0,
                    sh_flags & u64::from(object::elf::SHF_EXECINSTR) != 0,
                ),
                SectionFlags::Coff { characteristics } => (
                    true,
                    characteristics & object::pe::IMAGE_SCN_MEM_EXECUTE != 0,
                ),
                SectionFlags::MachO { flags } => (
                    flags & object::macho::S_ATTR_DEBUG == 0,
                    flags
                        & (object::macho::S_ATTR_PURE_INSTRUCTIONS
                            | object::macho::S_ATTR_SOME_INSTRUCTIONS)
                        != 0,
                ),
                _ => (true, section.kind() == object::SectionKind::Text),
            };
            let (file_offset, file_size) = valid_file_range(section.file_range(), bytes.len());
            Region {
                name: section.name().unwrap_or("<unnamed>").into(),
                address: (allocated && !is_relocatable).then(|| section.address()),
                size: section.size(),
                file_offset,
                file_size,
                executable,
            }
        })
        .collect();
    let segments = file
        .segments()
        .enumerate()
        .map(|(index, segment)| {
            let executable = match segment.flags() {
                SegmentFlags::Elf { p_flags } => p_flags & object::elf::PF_X != 0,
                SegmentFlags::MachO { initprot, .. } => {
                    initprot & object::macho::VM_PROT_EXECUTE != 0
                }
                SegmentFlags::Coff { characteristics } => {
                    characteristics & object::pe::IMAGE_SCN_MEM_EXECUTE != 0
                }
                _ => false,
            };
            let (file_offset, file_size) =
                valid_file_range(Some(segment.file_range()), bytes.len());
            Region {
                name: segment
                    .name()
                    .ok()
                    .flatten()
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("Segment {}", index + 1)),
                address: (!is_relocatable).then(|| segment.address()),
                size: segment.size(),
                file_offset,
                file_size,
                executable,
            }
        })
        .collect();
    let mut symbols: Vec<_> = file
        .symbols()
        .chain(file.dynamic_symbols())
        .filter_map(|symbol| {
            if !symbol.is_definition() {
                return None;
            }
            let name = symbol.name().ok().filter(|name| !name.is_empty())?;
            Some(Symbol {
                address: symbol.address(),
                name: name.into(),
                size: symbol.size(),
                is_function: symbol.kind() == SymbolKind::Text,
            })
        })
        .collect();
    symbols.sort_by(|a, b| (a.address, &a.name).cmp(&(b.address, &b.name)));
    symbols.dedup_by(|a, b| {
        a.address == b.address && a.name == b.name && a.is_function == b.is_function
    });
    let entry = file.entry();
    let mut metadata_warnings = Vec::new();
    let mut imports: Vec<_> = match file.imports() {
        Ok(imports) => imports
            .into_iter()
            .map(|import| Import {
                library: String::from_utf8_lossy(import.library()).into_owned(),
                name: String::from_utf8_lossy(import.name()).into_owned(),
            })
            .collect(),
        Err(error) => {
            metadata_warnings.push(format!("Import table: {error}"));
            Vec::new()
        }
    };
    imports.sort_by(|a, b| (&a.library, &a.name).cmp(&(&b.library, &b.name)));
    let mut exports: Vec<_> = match file.exports() {
        Ok(exports) => exports
            .into_iter()
            .map(|export| Export {
                address: export.address(),
                name: String::from_utf8_lossy(export.name()).into_owned(),
            })
            .collect(),
        Err(error) => {
            metadata_warnings.push(format!("Export table: {error}"));
            Vec::new()
        }
    };
    exports.sort_by(|a, b| (a.address, &a.name).cmp(&(b.address, &b.name)));
    Ok(ImageInfo {
        format,
        architecture,
        bits: if file.is_64() { 64 } else { 32 },
        little_endian: file.is_little_endian(),
        entry: (!is_relocatable && entry != 0).then_some(entry),
        image_base: (!is_relocatable).then(|| file.relative_address_base()),
        is_relocatable,
        sections,
        segments,
        symbols,
        imports,
        exports,
        metadata_warnings,
    })
}

fn valid_file_range(range: Option<(u64, u64)>, length: usize) -> (Option<u64>, u64) {
    match range {
        Some((offset, size))
            if size > 0
                && offset
                    .checked_add(size)
                    .is_some_and(|end| end <= length as u64) =>
        {
            (Some(offset), size)
        }
        _ => (None, 0),
    }
}
