use crate::{Error, Result};
use serde::Serialize;

pub struct BytePattern {
    bytes: Vec<Option<u8>>,
    anchor: (usize, u8),
}

impl BytePattern {
    pub fn parse(input: &str) -> Result<Self> {
        let bytes: Vec<_> = input
            .split_ascii_whitespace()
            .take(1025)
            .map(|word| {
                if word == "??" {
                    return Ok(None);
                }
                if word.len() != 2 {
                    return Err(Error::Input(format!(
                        "expected a two-digit byte or ??, found {word:?}"
                    )));
                }
                u8::from_str_radix(word, 16)
                    .map(Some)
                    .map_err(|_| Error::Input(format!("invalid hex byte {word:?}")))
            })
            .collect::<Result<_>>()?;
        if bytes.len() > 1024 {
            return Err(Error::Input(
                "byte patterns are limited to 1,024 bytes".into(),
            ));
        }
        let anchor = bytes
            .iter()
            .enumerate()
            .find_map(|(index, &byte)| byte.map(|byte| (index, byte)))
            .ok_or_else(|| Error::Input("enter a pattern with at least one literal byte".into()))?;
        Ok(Self { bytes, anchor })
    }

    pub fn find<'a>(&'a self, bytes: &'a [u8]) -> impl Iterator<Item = usize> + 'a {
        let (offset, anchor) = self.anchor;
        bytes
            .windows(self.bytes.len())
            .enumerate()
            .filter_map(move |(index, window)| {
                (window[offset] == anchor
                    && self
                        .bytes
                        .iter()
                        .zip(window)
                        .all(|(expected, &byte)| expected.is_none_or(|value| value == byte)))
                .then_some(index)
            })
    }
}

#[derive(Debug, Serialize)]
pub struct StringItem {
    pub offset: usize,
    pub ascii: String,
    pub byte_count: usize,
}

pub fn scan_strings(bytes: &[u8], min_length: usize) -> impl Iterator<Item = StringItem> + '_ {
    let mut offset = 0;
    bytes
        .split(|byte| !(b' '..=b'~').contains(byte))
        .filter_map(move |range| {
            let start = offset;
            offset += range.len() + 1;
            if range.len() < min_length.max(1) {
                return None;
            }
            let preview = &range[..range.len().min(256)];
            Some(StringItem {
                offset: start,
                ascii: String::from_utf8_lossy(preview).into_owned(),
                byte_count: range.len(),
            })
        })
}
