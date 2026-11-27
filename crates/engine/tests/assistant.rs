use hexaris_engine::{
    Architecture, AssistantResponse, ChangeKind, FunctionContext, Image, ImageIdentity, LoadMode,
    Project, ReferenceIndex, analyze, compare_builds,
};
use serde_json::json;

fn project(bytes: &[u8], base: u64) -> Project {
    let image = Image::from_bytes(
        bytes.to_vec(),
        LoadMode::Raw {
            architecture: Architecture::X86_64,
            base,
        },
    )
    .unwrap();
    Project::new("example.bin".into(), image)
}

#[test]
fn export_preserves_full_addresses_and_explains_paging_and_unsupported_pseudocode() {
    let base = 0x9000_0000_0000_0000;
    let project = project(&[0x90, 0x90, 0xC3], base);
    let analysis = analyze(project.image());
    let index = ReferenceIndex::build(&analysis);
    let function = &analysis.functions[&base];
    let page = FunctionContext::build(&project, function, &index, &analysis.notices, 1, 1).unwrap();
    let json = serde_json::to_value(&page).unwrap();
    assert_eq!(json["address"], "0x9000000000000000");
    assert_eq!(json["instructions"][0]["address"], "0x9000000000000001");
    assert_eq!(json["instructions"][0]["file_offset"], 1);
    assert_eq!(json["instructions"][0]["register_ir_supported"], true);
    assert_eq!(json["next_instruction_offset"], 2);
    assert!(page.pseudocode.is_none());
    assert!(
        page.notices
            .iter()
            .any(|notice| notice.contains("Windows x64"))
    );
    assert!(
        page.markdown()
            .unwrap()
            .contains("findings with text and addresses")
    );
    assert!(FunctionContext::build(&project, function, &index, &[], 4, 1).is_err());
}

#[test]
fn review_checks_mapping_identity_and_applies_only_the_selected_item() {
    let mut original = project(&[0x90, 0xC3], 0x1000);
    original
        .set_comment(0x1000, "Original comment".into())
        .unwrap();
    let payload = json!({
        "schema_version": 1,
        "image": ImageIdentity::from_project(&original),
        "findings": [{"text": "The function returns.", "addresses": ["0x1001"]}],
        "suggestions": [
            {"kind": "function_name", "address": "0x1000", "value": "return_immediately"},
            {"kind": "comment", "address": "0x1000", "value": "Proposed comment"},
            {"kind": "bookmark", "address": "0x1002", "value": "Unmapped target"}
        ]
    });
    let rebased = project(&[0x90, 0xC3], 0x2000);
    assert!(AssistantResponse::parse(&payload.to_string(), &rebased).is_err());
    let mut mismatched_format = payload.clone();
    mismatched_format["image"]["format"] = json!("PE");
    assert!(AssistantResponse::parse(&mismatched_format.to_string(), &original).is_err());
    let review = AssistantResponse::parse(&format!("```json\n{payload}\n```"), &original).unwrap();
    assert_eq!(review.findings[0].addresses[0].0, 0x1001);
    assert_eq!(
        review.suggestions[1].current(&original),
        Some("Original comment")
    );
    assert!(review.suggestions[2].apply(&mut original).is_err());
    review.suggestions[0].apply(&mut original).unwrap();
    assert_eq!(original.function_name(0x1000), Some("return_immediately"));
    assert_eq!(original.comment(0x1000), Some("Original comment"));
    assert_eq!(original.image().bytes(), [0x90, 0xC3]);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("reviewed.hxproj");
    original.save_new(&path).unwrap();
    let reopened = Project::open(&path).unwrap();
    assert_eq!(reopened.function_name(0x1000), Some("return_immediately"));
    assert_eq!(reopened.comment(0x1000), Some("Original comment"));
}

#[test]
fn comparison_matches_moved_code_and_shows_the_first_byte_difference() {
    // Both images contain an entry that calls a two-instruction leaf. Move
    // the leaf and keep its bytes identical while changing the call target.
    let before = project(&[0xE8, 1, 0, 0, 0, 0xC3, 0x31, 0xC0, 0xC3], 0x1000);
    let after = project(&[0xE8, 2, 0, 0, 0, 0xC3, 0x90, 0x31, 0xC0, 0xC3], 0x2000);
    let report = compare_builds(
        &before,
        &analyze(before.image()),
        &after,
        &analyze(after.image()),
    )
    .unwrap();
    assert_eq!(report.changes.len(), 2);
    let changed = &report.changes[0];
    assert_eq!(changed.kind, ChangeKind::Changed);
    assert_eq!(
        changed
            .first_difference
            .as_ref()
            .unwrap()
            .before
            .as_ref()
            .unwrap()
            .address
            .0,
        0x1000
    );
    let moved = &report.changes[1];
    assert_eq!(moved.kind, ChangeKind::Moved);
    assert_eq!(moved.before.as_ref().unwrap().address.0, 0x1006);
    assert_eq!(moved.after.as_ref().unwrap().address.0, 0x2007);
    assert!(report.markdown().contains("First differing instruction"));
}
