use std::{fs::File, io::Read, path::Path};

use quick_xml::{
    Reader, XmlVersion,
    encoding::Decoder,
    events::{BytesRef, BytesStart, Event},
};
use serde::Serialize;

use crate::{Error, Project, Result};

const MAX_XML: usize = 16 * 1024 * 1024;
const MAX_DEPTH: usize = 64;

#[derive(Debug, Default, Serialize)]
pub struct GhidraImport {
    pub function_names: usize,
    pub comments: usize,
    pub bookmarks: usize,
    pub existing: usize,
    pub skipped: usize,
}

impl GhidraImport {
    pub fn added(&self) -> usize {
        self.function_names + self.comments + self.bookmarks
    }
}

enum Annotation {
    Function,
    Comment,
    Bookmark,
}

struct Item {
    kind: Annotation,
    address: Option<u64>,
    value: String,
}

/// Merge supported annotations from a Ghidra XML export. Existing Hexaris
/// annotations win, and the project is updated only after parsing finishes.
pub fn import_ghidra_xml(project: &mut Project, path: &Path) -> Result<GhidraImport> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > MAX_XML as u64 {
        return Err(Error::Input(
            "expected a regular Ghidra XML file of at most 16 MiB".into(),
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_XML as u64 + 1).read_to_end(&mut bytes)?;
    import_xml(project, &bytes)
}

fn import_xml(project: &mut Project, bytes: &[u8]) -> Result<GhidraImport> {
    if bytes.len() > MAX_XML {
        return Err(Error::Input("Ghidra XML exceeds 16 MiB".into()));
    }
    let xml = std::str::from_utf8(bytes).map_err(xml_error)?;
    let mut reader = Reader::from_str(xml);
    let mut path: Vec<Vec<u8>> = Vec::new();
    let mut comment = None;
    let mut items = Vec::new();
    let mut seen_program = false;
    loop {
        let event = reader.read_event().map_err(xml_error)?;
        let decoder = reader.decoder();
        match event {
            Event::Start(ref node) | Event::Empty(ref node) => {
                let tag = node.name().0;
                let push = matches!(event, Event::Start(_));
                if path.is_empty() {
                    if seen_program || tag != b"PROGRAM" {
                        return Err(xml_error("expected one PROGRAM root element"));
                    }
                    seen_program = true;
                    verify_image(project, node, decoder)?;
                } else if path.as_slice() == [b"PROGRAM"] && tag == b"PROCESSOR" {
                    verify_processor(project, node, decoder)?;
                } else if path.len() == 2 {
                    match (path[1].as_slice(), tag) {
                        (b"FUNCTIONS", b"FUNCTION") => items.push(Item {
                            kind: Annotation::Function,
                            address: attribute_address(node, b"ENTRY_POINT", decoder)?,
                            value: attribute(node, b"NAME", decoder)?.unwrap_or_default(),
                        }),
                        (b"BOOKMARKS", b"BOOKMARK") => {
                            let address = attribute_address(node, b"ADDRESS", decoder)?;
                            let value = attribute(node, b"DESCRIPTION", decoder)?
                                .filter(|value| !value.trim().is_empty())
                                .or(attribute(node, b"CATEGORY", decoder)?)
                                .unwrap_or_else(|| "Imported bookmark".into());
                            items.push(Item {
                                kind: Annotation::Bookmark,
                                address,
                                value,
                            });
                        }
                        (b"COMMENTS", b"COMMENT") if push => {
                            comment = Some(Item {
                                kind: Annotation::Comment,
                                address: attribute_address(node, b"ADDRESS", decoder)?,
                                value: String::new(),
                            })
                        }
                        _ => {}
                    }
                }
                if push {
                    if path.len() == MAX_DEPTH {
                        return Err(xml_error("nesting depth exceeds 64"));
                    }
                    if comment.is_some() && path.len() >= 3 {
                        return Err(xml_error("unexpected markup in COMMENT"));
                    }
                    path.push(tag.to_vec());
                }
            }
            Event::End(tag) => {
                if path.last().map(Vec::as_slice) != Some(tag.name().0) {
                    return Err(xml_error("unbalanced element"));
                }
                if path.len() == 3
                    && let Some(comment) = comment.take()
                {
                    items.push(comment);
                }
                path.pop();
            }
            Event::Text(text) => {
                if let Some(comment) = &mut comment {
                    comment.value.push_str(
                        &text
                            .xml_content(XmlVersion::Implicit1_0)
                            .map_err(xml_error)?,
                    );
                } else if path.is_empty() && !text.as_ref().iter().all(u8::is_ascii_whitespace) {
                    return Err(xml_error("unexpected text outside the program"));
                }
            }
            Event::CData(text) => {
                if let Some(comment) = &mut comment {
                    comment.value.push_str(
                        &text
                            .xml_content(XmlVersion::Implicit1_0)
                            .map_err(xml_error)?,
                    );
                } else if path.is_empty() {
                    return Err(xml_error("unexpected data outside the program"));
                }
            }
            Event::GeneralRef(reference) => {
                if let Some(comment) = &mut comment {
                    comment.value.push(entity_character(&reference)?);
                } else if path.is_empty() {
                    return Err(xml_error("unexpected entity outside the program"));
                }
            }
            Event::DocType(doctype) => {
                // Ghidra includes its own internal DTD. No external identifiers
                // or entity declarations are consulted, parsed, or resolved.
                let declaration = doctype.as_ref();
                let name = declaration
                    .split(|&byte| byte == b'[')
                    .next()
                    .unwrap_or_default()
                    .trim_ascii();
                if seen_program
                    || name != b"PROGRAM"
                    || declaration.windows(8).any(|part| part == b"<!ENTITY")
                {
                    return Err(xml_error("unexpected DTD or custom entity declaration"));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !seen_program || !path.is_empty() {
        return Err(xml_error("missing or incomplete PROGRAM element"));
    }

    project.merge_annotations(|project| merge_items(project, items))
}

fn merge_items(project: &mut Project, items: Vec<Item>) -> GhidraImport {
    let mut report = GhidraImport::default();
    for item in items {
        let Some(address) = item
            .address
            .filter(|&address| project.image().file_offset(address).is_some())
        else {
            report.skipped += 1;
            continue;
        };
        if item.value.trim().is_empty() {
            report.skipped += 1;
            continue;
        }
        let (existing, count) = match item.kind {
            Annotation::Function => (
                project.function_name(address).is_some(),
                &mut report.function_names,
            ),
            Annotation::Comment => (project.comment(address).is_some(), &mut report.comments),
            Annotation::Bookmark => (project.bookmark(address).is_some(), &mut report.bookmarks),
        };
        if existing {
            report.existing += 1;
            continue;
        }
        let result = match item.kind {
            Annotation::Function => project.set_function_name(address, item.value),
            Annotation::Comment => project.set_comment(address, item.value),
            Annotation::Bookmark => project.set_bookmark(address, item.value),
        };
        if result.is_ok() {
            *count += 1;
        } else {
            report.skipped += 1;
        }
    }
    report
}

fn verify_image(project: &Project, node: &BytesStart<'_>, decoder: Decoder) -> Result<()> {
    if let Some(base) = attribute(node, b"IMAGE_BASE", decoder)? {
        let address = parse_xml_address(&base).ok_or_else(|| xml_error("invalid IMAGE_BASE"))?;
        if let Some(expected) = project.image().info.image_base
            && address != expected
        {
            return Err(Error::Input(format!(
                "Ghidra XML image base 0x{address:X} differs from Hexaris image base 0x{expected:X}"
            )));
        }
    }
    Ok(())
}

fn verify_processor(project: &Project, node: &BytesStart<'_>, decoder: Decoder) -> Result<()> {
    if let Some(language) = attribute(node, b"LANGUAGE_PROVIDER", decoder)? {
        let info = &project.image().info;
        let mut parts = language.split(':');
        let _ = parts.next();
        let little = match parts.next() {
            Some("LE") => Some(true),
            Some("BE") => Some(false),
            _ => None,
        };
        let bits = parts.next().and_then(|value| value.parse::<u8>().ok());
        if little.is_some_and(|value| value != info.little_endian)
            || bits.is_some_and(|value| value != info.bits)
        {
            return Err(Error::Input(
                "Ghidra XML processor width or byte order differs from the image".into(),
            ));
        }
    }
    Ok(())
}

fn parse_xml_address(input: &str) -> Option<u64> {
    let input = input.trim();
    let input = input.strip_prefix("ram:").unwrap_or(input);
    let digits = input
        .strip_prefix("0x")
        .or_else(|| input.strip_prefix("0X"))
        .unwrap_or(input);
    u64::from_str_radix(digits, 16).ok()
}

fn attribute_address(node: &BytesStart<'_>, key: &[u8], decoder: Decoder) -> Result<Option<u64>> {
    Ok(attribute(node, key, decoder)?
        .as_deref()
        .and_then(parse_xml_address))
}

fn attribute(node: &BytesStart<'_>, key: &[u8], decoder: Decoder) -> Result<Option<String>> {
    node.try_get_attribute(key)
        .map_err(xml_error)?
        .map(|attribute| {
            attribute
                .decoded_and_normalized_value(XmlVersion::Implicit1_0, decoder)
                .map(|value| value.into_owned())
                .map_err(xml_error)
        })
        .transpose()
}

fn entity_character(reference: &BytesRef<'_>) -> Result<char> {
    match reference.as_ref() {
        b"amp" => Ok('&'),
        b"lt" => Ok('<'),
        b"gt" => Ok('>'),
        b"quot" => Ok('"'),
        b"apos" => Ok('\''),
        _ => reference
            .resolve_char_ref()
            .map_err(xml_error)?
            .ok_or_else(|| xml_error("unknown entity")),
    }
}

fn xml_error(error: impl std::fmt::Display) -> Error {
    Error::Input(format!("invalid Ghidra XML: {error}"))
}
