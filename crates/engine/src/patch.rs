use std::{io::Write, ops::Range, path::Path};

use crate::{Error, Image, Result};

const PATCH_LIMIT: usize = 4096;

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
        let file = image.bytes();
        let range = self.checked_range(file)?;
        let parent = output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(&file[..range.start])?;
        temporary.write_all(&self.replacement)?;
        temporary.write_all(&file[range.end..])?;
        temporary.as_file().sync_all()?;
        temporary
            .persist_noclobber(output)
            .map_err(|error| error.error)?;
        Ok(())
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

fn parse_bytes(text: &str) -> Result<Vec<u8>> {
    let bytes: Vec<_> = text
        .split_ascii_whitespace()
        .take(PATCH_LIMIT + 1)
        .map(|word| {
            if word.len() == 2 && word.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return u8::from_str_radix(word, 16)
                    .map_err(|_| Error::Input(format!("invalid hex byte {word:?}")));
            }
            Err(Error::Input(format!(
                "expected a two-digit hex byte, found {word:?}"
            )))
        })
        .collect::<Result<_>>()?;
    if !(1..=PATCH_LIMIT).contains(&bytes.len()) {
        return Err(Error::Input(format!(
            "a byte patch needs 1 to {PATCH_LIMIT} bytes"
        )));
    }
    Ok(bytes)
}
