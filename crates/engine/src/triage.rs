use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{Architecture, Image};

const ENTROPY_SAMPLES: usize = 512;
const MIN_SAMPLE_BYTES: usize = 256;

#[derive(Debug, Serialize)]
pub struct ImageTriage {
    pub format: &'static str,
    pub architecture: Architecture,
    pub bits: u8,
    pub little_endian: bool,
    pub size_bytes: usize,
    pub sha256: String,
    pub image_base: Option<u64>,
    pub entry: Option<u64>,
    pub section_count: usize,
    pub executable_section_count: usize,
    pub symbol_count: usize,
    pub named_import_count: usize,
    pub named_export_count: usize,
    pub entropy_bits_per_byte: f64,
    pub samples: Vec<EntropySample>,
    pub metadata_warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct EntropySample {
    pub offset: usize,
    pub byte_count: usize,
    pub entropy_bits_per_byte: f64,
}

/// Describe the on-disk bytes and summarize Shannon entropy across at most
/// 512 windows. Zero-filled virtual memory does not contribute to the result.
pub fn triage(image: &Image) -> ImageTriage {
    let bytes = image.bytes();
    let info = &image.info;
    let window = bytes.len().div_ceil(ENTROPY_SAMPLES).max(MIN_SAMPLE_BYTES);
    let mut totals = [0u32; 256];
    let mut digest = Sha256::new();
    let samples = bytes
        .chunks(window)
        .enumerate()
        .map(|(index, chunk)| {
            let mut counts = [0u32; 256];
            for &byte in chunk {
                counts[usize::from(byte)] += 1;
            }
            for (total, count) in totals.iter_mut().zip(counts) {
                *total += count;
            }
            digest.update(chunk);
            EntropySample {
                offset: index * window,
                byte_count: chunk.len(),
                entropy_bits_per_byte: entropy(&counts, chunk.len()),
            }
        })
        .collect();

    ImageTriage {
        format: info.format,
        architecture: info.architecture,
        bits: info.bits,
        little_endian: info.little_endian,
        size_bytes: bytes.len(),
        sha256: format!("{:x}", digest.finalize()),
        image_base: info.image_base,
        entry: info.entry,
        section_count: info.sections.len(),
        executable_section_count: info
            .sections
            .iter()
            .filter(|region| region.executable)
            .count(),
        symbol_count: info.symbols.len(),
        named_import_count: info.imports.len(),
        named_export_count: info.exports.len(),
        entropy_bits_per_byte: entropy(&totals, bytes.len()),
        samples,
        metadata_warnings: info.metadata_warnings.clone(),
    }
}

fn entropy(counts: &[u32; 256], len: usize) -> f64 {
    counts
        .iter()
        .copied()
        .filter(|&count| count > 0)
        .map(|count| {
            let fraction = f64::from(count) / len as f64;
            -fraction * fraction.log2()
        })
        .sum()
}
