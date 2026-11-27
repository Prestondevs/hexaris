use std::{io::Write, ops::Range, path::Path};

use crate::{Error, Image, Instruction, Result};

const PATCH_LIMIT: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchEdit {
    Invert,
    AlwaysTaken,
    AlwaysFallthrough,
}

/// A fixed-length replacement at a file offset, guarded by the expected bytes.
pub struct BytePatch {
    pub offset: usize,
    expected: Vec<u8>,
    replacement: Vec<u8>,
}

impl BytePatch {
    pub fn parse(offset: usize, expected: &str, replacement: &str) -> Result<Self> {
        let expected = parse_bytes(expected)?;
        let replacement = parse_bytes(replacement)?;
        if expected.len() != replacement.len() {
            return Err(Error::Input(
                "expected and replacement byte counts must match".into(),
            ));
        }
        Ok(Self {
            offset,
            expected,
            replacement,
        })
    }

    pub fn write_copy(&self, image: &Image, output: &Path) -> Result<()> {
        write_patched_copy(image, std::slice::from_ref(self), output)
    }

    pub fn for_replacement(image: &Image, offset: usize, replacement: &str) -> Result<Self> {
        let replacement = parse_bytes(replacement)?;
        let end = offset
            .checked_add(replacement.len())
            .ok_or_else(|| Error::Input("byte patch extends past the end of the image".into()))?;
        let expected = image
            .bytes()
            .get(offset..end)
            .ok_or_else(|| Error::Input("byte patch extends past the end of the image".into()))?
            .to_vec();
        if expected == replacement {
            return Err(Error::Input(
                "Replacement matches the original bytes.".into(),
            ));
        }
        Ok(Self {
            offset,
            expected,
            replacement,
        })
    }

    pub fn invert_branch(instruction: &Instruction) -> Option<Self> {
        Self::edit_branch(instruction, BranchEdit::Invert)
    }

    pub fn edit_branch(instruction: &Instruction, edit: BranchEdit) -> Option<Self> {
        let opcode = match instruction.bytes.as_slice() {
            [0x70..=0x7f, _] if instruction.decoded.is_jcc_short_or_near() => 0,
            [0x0f, 0x80..=0x8f, _, _, _, _] if instruction.decoded.is_jcc_short_or_near() => 1,
            _ => return None,
        };
        let mut replacement = instruction.bytes.clone();
        match edit {
            BranchEdit::Invert => replacement[opcode] ^= 1,
            BranchEdit::AlwaysTaken if opcode == 0 => replacement[0] = 0xeb,
            // Both encodings end at the same IP, so the displacement stays unchanged.
            BranchEdit::AlwaysTaken => replacement[..2].copy_from_slice(&[0x90, 0xe9]),
            BranchEdit::AlwaysFallthrough => replacement.fill(0x90),
        }
        Some(Self {
            offset: instruction.file_offset,
            expected: instruction.bytes.clone(),
            replacement,
        })
    }

    pub fn expected(&self) -> &[u8] {
        &self.expected
    }

    pub fn replacement(&self) -> &[u8] {
        &self.replacement
    }

    pub fn overlaps(&self, other: &Self) -> bool {
        self.offset < other.offset.saturating_add(other.expected.len())
            && other.offset < self.offset.saturating_add(self.expected.len())
    }

    fn checked_range(&self, file: &[u8]) -> Result<Range<usize>> {
        let end = self
            .offset
            .checked_add(self.expected.len())
            .filter(|&end| end <= file.len())
            .ok_or_else(|| Error::Input("byte patch extends past the end of the image".into()))?;
        let range = self.offset..end;
        if file[range.clone()] != self.expected {
            return Err(Error::Input(
                "expected bytes do not match the image at that offset".into(),
            ));
        }
        Ok(range)
    }
}

/// Verify all changes before writing a separate output, in file order.
pub fn write_patched_copy(image: &Image, patches: &[BytePatch], output: &Path) -> Result<()> {
    if patches.is_empty() {
        return Err(Error::Input(
            "Queue at least one patch before exporting.".into(),
        ));
    }
    let bytes = image.bytes();
    let mut ordered: Vec<_> = patches.iter().collect();
    ordered.sort_unstable_by_key(|patch| patch.offset);
    let mut start = 0;
    let mut ranges = Vec::with_capacity(ordered.len());
    for patch in ordered {
        let range = patch.checked_range(bytes)?;
        if range.start < start {
            return Err(Error::Input("Byte patches overlap.".into()));
        }
        start = range.end;
        ranges.push((range, patch));
    }
    let parent = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    start = 0;
    for (range, patch) in ranges {
        temporary.write_all(&bytes[start..range.start])?;
        temporary.write_all(patch.replacement())?;
        start = range.end;
    }
    temporary.write_all(&bytes[start..])?;
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(output)
        .map_err(|error| error.error)?;
    Ok(())
}

pub fn parse_bytes(text: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for word in text.split_ascii_whitespace() {
        if word.len() % 2 != 0 || !word.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(Error::Input(
                "Enter complete hex bytes, such as 48 8B or 488B.".into(),
            ));
        }
        if word.len() / 2 > PATCH_LIMIT - bytes.len() {
            return Err(Error::Input(format!(
                "a byte patch needs 1 to {PATCH_LIMIT} bytes"
            )));
        }
        for offset in (0..word.len()).step_by(2) {
            bytes.push(
                u8::from_str_radix(&word[offset..offset + 2], 16).expect("validated ASCII hex"),
            );
        }
    }
    if !(1..=PATCH_LIMIT).contains(&bytes.len()) {
        return Err(Error::Input(format!(
            "a byte patch needs 1 to {PATCH_LIMIT} bytes"
        )));
    }
    Ok(bytes)
}
