use std::fs;

use hexaris_engine::{
    Architecture, Image, LoadMode, Project, ReferenceIndex, analyze, import_ghidra_xml, triage,
};

fn raw(bytes: Vec<u8>) -> Image {
    Image::from_bytes(
        bytes,
        LoadMode::Raw {
            architecture: Architecture::X86_64,
            base: 0x1000,
        },
    )
    .unwrap()
}

#[test]
fn stripped_elf_loads_code_from_a_segment_and_keeps_bss_unbacked() {
    // ELF64 ET_EXEC with one PT_LOAD and no section or symbol tables.
    const BASE: u64 = 0x400000;
    const ENTRY: u64 = BASE + 256;
    let mut bytes = vec![0u8; 259];
    bytes[..7].copy_from_slice(b"\x7FELF\x02\x01\x01");
    bytes[16..20].copy_from_slice(&[2, 0, 62, 0]);
    bytes[20..24].copy_from_slice(&1u32.to_le_bytes());
    bytes[24..32].copy_from_slice(&ENTRY.to_le_bytes());
    bytes[32..40].copy_from_slice(&64u64.to_le_bytes());
    bytes[52..58].copy_from_slice(&[64, 0, 56, 0, 1, 0]);
    bytes[64..72].copy_from_slice(&[1, 0, 0, 0, 5, 0, 0, 0]);
    bytes[80..88].copy_from_slice(&BASE.to_le_bytes());
    bytes[88..96].copy_from_slice(&BASE.to_le_bytes());
    bytes[96..104].copy_from_slice(&259u64.to_le_bytes());
    bytes[104..112].copy_from_slice(&272u64.to_le_bytes());
    bytes[112..120].copy_from_slice(&4096u64.to_le_bytes());
    bytes[256..].copy_from_slice(&[0x31, 0xC0, 0xC3]);

    let image = Image::from_bytes(bytes, LoadMode::Auto).unwrap();
    assert_eq!(image.info.format, "ELF");
    assert_eq!(image.info.architecture, Architecture::X86_64);
    assert!(image.info.sections.is_empty());
    assert_eq!(image.file_offset(ENTRY), Some(256));
    assert_eq!(image.virtual_address(258), Some(ENTRY + 2));
    assert_eq!(image.file_offset(ENTRY + 3), None);
    assert_eq!(analyze(&image).functions[&ENTRY].instructions.len(), 2);
}

#[test]
fn triage_distinguishes_uniform_and_full_entropy_windows() {
    let mut bytes = vec![0; 256];
    bytes.extend(0..=255);
    let report = triage(&raw(bytes));
    assert_eq!(report.size_bytes, 512);
    assert_eq!(report.samples.len(), 2);
    assert_eq!(report.samples[0].entropy_bits_per_byte, 0.0);
    assert_eq!(report.samples[1].offset, 256);
    assert_eq!(report.samples[1].entropy_bits_per_byte, 8.0);
    assert!((4.0..5.0).contains(&report.entropy_bits_per_byte));
    assert_eq!(report.sha256.len(), 64);
}

#[test]
fn caller_and_callee_use_the_same_indexed_call_site() {
    let image = raw(vec![0xE8, 1, 0, 0, 0, 0xC3, 0xC3]);
    let references = ReferenceIndex::build(&analyze(&image));
    let callers = references.callers(0x1006);
    assert_eq!(callers.len(), 1);
    assert_eq!(callers, references.callees(0x1000));
    assert_eq!(callers[0].site, 0x1000);
    assert_eq!(references.owner(0x1005), Some(0x1000));
    assert_eq!(references.incoming(0x1006), references.outgoing(0x1000));
}

#[test]
fn bookmarked_projects_round_trip_and_read_the_original_format() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("bookmarks.hxproj");
    let project = Project::new("Notes".into(), raw(vec![0x90, 0xC3]));
    project.save_new(&path).unwrap();
    assert_eq!(fs::read(&path).unwrap()[7], 1);
    let mut project = Project::open(&path).unwrap();
    assert_eq!(project.bookmarks().len(), 0);
    assert!(project.set_bookmark(0x2000, "unmapped".into()).is_err());
    assert!(project.set_bookmark(0x1001, "bad\nlabel".into()).is_err());
    project
        .set_bookmark(0x1001, "  return here  ".into())
        .unwrap();
    project.save(&path).unwrap();
    assert_eq!(fs::read(&path).unwrap()[7], 2);
    let mut project = Project::open(&path).unwrap();
    assert_eq!(
        project.bookmarks().collect::<Vec<_>>(),
        [(0x1001, "return here")]
    );
    project.set_bookmark(0x1001, String::new()).unwrap();
    assert_eq!(project.bookmark(0x1001), None);
}

#[test]
fn ghidra_import_merges_atomic_metadata_and_refuses_bad_inputs() {
    let directory = tempfile::tempdir().unwrap();
    let xml = directory.path().join("export.xml");
    let mut project = Project::new("Notes".into(), raw(vec![0x90, 0xC3]));
    project
        .set_function_name(0x1000, "local_name".into())
        .unwrap();
    fs::write(&xml, r#"<!DOCTYPE PROGRAM [<!ELEMENT PROGRAM ANY>]><PROGRAM IMAGE_BASE="1000"><PROCESSOR LANGUAGE_PROVIDER="x86:LE:64:default:windows"/><FUNCTIONS><FUNCTION ENTRY_POINT="1000" NAME="imported"/></FUNCTIONS><COMMENTS><COMMENT ADDRESS="ram:1001">a &amp; b &#x2192; c</COMMENT><COMMENT ADDRESS="other:1000">unmapped space</COMMENT></COMMENTS><BOOKMARKS><BOOKMARK ADDRESS="1000" DESCRIPTION="Investigate &#x26; verify"/></BOOKMARKS></PROGRAM>"#).unwrap();
    let report = import_ghidra_xml(&mut project, &xml).unwrap();
    assert_eq!(
        (
            report.function_names,
            report.comments,
            report.bookmarks,
            report.existing,
            report.skipped
        ),
        (0, 1, 1, 1, 1)
    );
    assert_eq!(project.function_name(0x1000), Some("local_name"));
    assert_eq!(project.comment(0x1001), Some("a & b → c"));
    assert_eq!(project.bookmark(0x1000), Some("Investigate & verify"));

    for bad in [
        r#"<PROGRAM IMAGE_BASE="1001"><COMMENTS><COMMENT ADDRESS="1000">bad base</COMMENT></COMMENTS></PROGRAM>"#,
        r#"<!DOCTYPE PROGRAM SYSTEM "https://example.invalid/not-used"><PROGRAM/>"#,
        r#"<!DOCTYPE PROGRAM [<!ENTITY x "ignored">]><PROGRAM/>"#,
        r#"<PROGRAM><COMMENTS><COMMENT ADDRESS="1000">truncated"#,
        r#"<PROGRAM><COMMENTS><COMMENT ADDRESS="1000">parsed</COMMENT></COMMENTS><PROCESSOR LANGUAGE_PROVIDER="x86:BE:64:default:windows"/></PROGRAM>"#,
    ] {
        fs::write(&xml, bad).unwrap();
        assert!(import_ghidra_xml(&mut project, &xml).is_err());
        assert_eq!(project.comment(0x1000), None);
    }
}
