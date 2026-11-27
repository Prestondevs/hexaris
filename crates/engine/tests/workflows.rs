use std::{
    fs,
    io::{Seek, SeekFrom, Write},
};

use hexaris_engine::{
    Architecture, BytePatch, BytePattern, EdgeKind, Image, InstructionToken, LoadMode, Project,
    ReferenceKind, analyze, analyze_function, decompile, disassemble, lift, scan_strings,
    write_patched_copy,
};

fn raw(bytes: &[u8], base: u64) -> Image {
    Image::from_bytes(
        bytes.to_vec(),
        LoadMode::Raw {
            architecture: Architecture::X86_64,
            base,
        },
    )
    .unwrap()
}

#[test]
fn guarded_byte_patch_creates_only_a_new_file() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("patched.bin");
    let image = raw(&[0x90, 0x90, 0xC3], 0);
    let wrong = BytePatch::parse(1, "CC", "90").unwrap();
    assert!(wrong.write_copy(&image, &output).is_err());
    assert!(!output.exists());
    assert!(BytePatch::parse(1, "90", "90 90").is_err());
    assert!(
        BytePatch::parse(3, "90", "CC")
            .unwrap()
            .write_copy(&image, &output)
            .is_err()
    );
    let patch = BytePatch::parse(1, "90", "CC").unwrap();
    patch.write_copy(&image, &output).unwrap();
    assert_eq!(fs::read(&output).unwrap(), [0x90, 0xCC, 0xC3]);
    assert_eq!(image.bytes(), [0x90, 0x90, 0xC3]);
    assert!(patch.write_copy(&image, &output).is_err());
}

#[test]
fn queued_edits_invert_short_and_near_branches_without_moving_targets() {
    // JNE short and JL near both branch to the RET at 0x1008.
    let bytes = [0x75, 6, 0x0f, 0x8c, 0, 0, 0, 0, 0xc3];
    let image = raw(&bytes, 0x1000);
    let listing = disassemble(&image, 0x1000, 3).unwrap();
    let mut tokens = Vec::new();
    listing.instructions[0].visit_tokens(|text, kind| tokens.push((text.to_owned(), kind)));
    assert!(
        tokens
            .iter()
            .any(|(text, kind)| text == "jne" && *kind == InstructionToken::ControlFlow)
    );
    assert!(
        tokens
            .iter()
            .any(|(_, kind)| *kind == InstructionToken::Target)
    );
    assert_eq!(
        tokens
            .iter()
            .map(|(text, _)| text.as_str())
            .collect::<String>(),
        listing.instructions[0].text
    );
    let short = BytePatch::invert_branch(&listing.instructions[0]).unwrap();
    let near = BytePatch::invert_branch(&listing.instructions[1]).unwrap();
    assert_eq!(short.replacement(), [0x74, 6]);
    assert_eq!(near.replacement(), [0x0f, 0x8d, 0, 0, 0, 0]);
    assert!(BytePatch::invert_branch(&listing.instructions[2]).is_none());
    assert!(!short.overlaps(&near));

    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("patched.bin");
    let overlap = BytePatch::for_replacement(&image, 1, "C3 CC").unwrap();
    assert!(short.overlaps(&overlap));
    assert!(write_patched_copy(&image, &[short, overlap], &output).is_err());
    assert!(!output.exists());

    let short = BytePatch::invert_branch(&listing.instructions[0]).unwrap();
    write_patched_copy(&image, &[near, short], &output).unwrap();
    let patched = raw(&fs::read(&output).unwrap(), 0x1000);
    let patched = analyze_function(&patched, 0x1000).unwrap();
    assert!(patched.instructions[0].text.starts_with("je "));
    assert!(patched.instructions[1].text.starts_with("jge "));
    assert!(
        patched.blocks[0]
            .successors
            .iter()
            .any(|edge| { edge.kind == EdgeKind::Taken && edge.target == 0x1008 })
    );
    assert!(
        patched.blocks[1]
            .successors
            .iter()
            .any(|edge| { edge.kind == EdgeKind::Taken && edge.target == 0x1008 })
    );
    assert_eq!(image.bytes(), bytes);
}

#[test]
fn control_flow_pseudocode_retains_calls_and_backedges_when_structured_is_unavailable() {
    // call 0x100b; test eax,eax; jne start; ret; pad; inc eax; ret
    let image = raw(
        &[
            0xe8, 6, 0, 0, 0, 0x85, 0xc0, 0x75, 0xf7, 0xc3, 0x90, 0xff, 0xc0, 0xc3,
        ],
        0x1000,
    );
    let function = analyze_function(&image, 0x1000).unwrap();
    let result = decompile(&image, &function);
    assert!(result.code.is_none());
    let listing = result.control_flow.unwrap();
    assert!(!listing.truncated);
    assert!(
        listing
            .lines
            .iter()
            .any(|line| { line.address == Some(0x1000) && line.text.contains("call(0x100B)") })
    );
    assert!(listing.lines.iter().any(|line| {
        line.address == Some(0x1007)
            && line.text.contains("goto loc_1000")
            && line.text.contains("goto loc_1009")
    }));
}

#[test]
fn follows_reachable_code_and_indexes_direct_references() {
    // call 0x1010; test eax,eax; je 0x100c; xor eax,eax; ret;
    // inc eax; ret; [padding]; mov eax,1; ret
    let image = raw(
        &[
            0xE8, 0x0B, 0, 0, 0, 0x85, 0xC0, 0x74, 3, 0x31, 0xC0, 0xC3, 0xFF, 0xC0, 0xC3, 0x90,
            0xB8, 1, 0, 0, 0, 0xC3,
        ],
        0x1000,
    );
    let analysis = analyze(&image);
    assert_eq!(
        analysis.functions.keys().copied().collect::<Vec<_>>(),
        [0x1000, 0x1010]
    );
    let entry = &analysis.functions[&0x1000];
    assert_eq!(
        entry
            .blocks
            .iter()
            .map(|block| block.address)
            .collect::<Vec<_>>(),
        [0x1000, 0x1009, 0x100C]
    );
    assert!(
        entry.blocks[0]
            .successors
            .iter()
            .any(|edge| edge.target == 0x100C && edge.kind == EdgeKind::Taken)
    );
    assert!(
        entry.blocks[0]
            .successors
            .iter()
            .any(|edge| edge.target == 0x1009 && edge.kind == EdgeKind::Fallthrough)
    );
    assert!(
        entry
            .references
            .iter()
            .any(|reference| reference.target == 0x1010 && reference.kind == ReferenceKind::Call)
    );
    assert!(
        entry
            .instructions
            .iter()
            .all(|instruction| instruction.address != 0x100F)
    );
    assert_eq!(image.virtual_address(0x10), Some(0x1010));
    assert_eq!(image.file_offset(0x1010), Some(0x10));
}

#[test]
fn detects_overlapping_control_flow_and_stops_at_truncated_instruction() {
    // A conditional jump to the middle of the following mov instruction.
    let image = raw(&[0x74, 2, 0xB8, 0, 0, 0, 0, 0xC3], 0x1000);
    let function = analyze_function(&image, 0x1000).unwrap();
    assert!(
        function
            .issues
            .iter()
            .any(|issue| issue.contains("Overlapping"))
    );
    assert!(
        function
            .instructions
            .iter()
            .all(|instruction| instruction.address != 0x1004)
    );
    let truncated = analyze_function(&raw(&[0xE8, 0x01], 0), 0).unwrap();
    assert!(truncated.instructions.is_empty());
    assert!(truncated.issues[0].contains("truncated"));
}

#[test]
fn resolves_rip_relative_operands_and_self_edges() {
    // lea rax,[rip+2]; jmp to itself; ret
    let image = raw(&[0x48, 0x8D, 0x05, 2, 0, 0, 0, 0xEB, 0xFE, 0xC3], 0x1000);
    let function = analyze_function(&image, 0x1000).unwrap();
    assert!(function.references.iter().any(|reference| reference.kind == ReferenceKind::Address && reference.target == 0x1009));
    assert_eq!(function.blocks[1].address, 0x1007);
    assert_eq!(function.blocks[1].successors[0].target, 0x1007);
    assert_eq!(function.instructions.len(), 2);
}

#[test]
fn lifts_wrapping_32_bit_arithmetic_and_withholds_opaque_pseudocode() {
    // mov eax, ecx; add eax,-1; xor edx,edx; sub eax,edx; ret
    let image = raw(
        &[0x89, 0xC8, 0x83, 0xC0, 0xFF, 0x31, 0xD2, 0x29, 0xD0, 0xC3],
        0x1000,
    );
    let lifted = lift(&analyze_function(&image, 0x1000).unwrap());
    let text = lifted.register_pseudocode.unwrap();
    assert!(text.contains("uint32_t ecx_in"));
    assert!(text.contains("eax_1 + 0xFFFFFFFFu"));
    assert!(text.contains("edx_3 = 0u"));
    assert!(text.contains("return eax_4;"));
    // push rax is outside the register expression model and invalidates state.
    let opaque = lift(&analyze_function(&raw(&[0x50, 0xC3], 0), 0).unwrap());
    assert!(opaque.register_pseudocode.is_none());
    assert_eq!(opaque.blocks[0].statements[1].text, "return unknown_1_eax");
}

#[test]
fn project_round_trip_is_portable_and_detects_corruption() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("score.hxproj");
    let mut original = Project::new("score".into(), raw(&[0x90, 0xC3], 0x1000));
    original.set_comment(0x1001, "first return".into()).unwrap();
    original.set_function_name(0x1000, "main".into()).unwrap();
    assert!(
        original
            .set_comment(0x2000, "out of bounds".into())
            .is_err()
    );
    original.save_new(&path).unwrap();
    assert!(original.save_new(&path).is_err());
    let mut reopened = Project::open(&path).unwrap();
    assert_eq!(reopened.name(), "score");
    assert_eq!(reopened.comment(0x1001), Some("first return"));
    assert_eq!(reopened.function_name(0x1000), Some("main"));
    assert_eq!(reopened.image().bytes(), [0x90, 0xC3]);
    reopened.set_comment(0x1001, String::new()).unwrap();
    reopened.save(&path).unwrap();
    assert_eq!(Project::open(&path).unwrap().comment(0x1001), None);
    let mut file = fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.seek(SeekFrom::End(-1)).unwrap();
    file.write_all(&[0x90]).unwrap();
    assert!(
        Project::open(&path)
            .err()
            .unwrap()
            .to_string()
            .contains("checksum")
    );
}

#[test]
fn byte_patterns_and_ascii_ranges_handle_boundaries() {
    let pattern = BytePattern::parse("AA ?? aa").unwrap();
    assert_eq!(
        pattern.find(&[0xAA, 0xAA, 0xAA, 0xAA]).collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(BytePattern::parse("?? ??").is_err());
    assert!(BytePattern::parse("A").is_err());
    assert_eq!(pattern.find(&[]).count(), 0);
    let strings = scan_strings(b"\0short\xffA\0tail", 4).collect::<Vec<_>>();
    assert_eq!(
        strings
            .iter()
            .map(|item| (item.offset, item.ascii.as_str()))
            .collect::<Vec<_>>(),
        [(1, "short"), (9, "tail")]
    );
}

#[test]
fn raw_address_ranges_are_checked() {
    assert!(
        Image::from_bytes(
            vec![0x90, 0xC3],
            LoadMode::Raw {
                architecture: Architecture::X86_64,
                base: u64::MAX
            }
        )
        .is_err()
    );
    assert!(
        Image::from_bytes(
            vec![0xC3],
            LoadMode::Raw {
                architecture: Architecture::X86,
                base: 1 << 32
            }
        )
        .is_err()
    );
    assert!(Image::from_bytes(b"not an executable".to_vec(), LoadMode::Auto).is_err());
    let edge = raw(&[0xC3], u64::MAX);
    assert_eq!(edge.file_offset(u64::MAX), Some(0));
}

#[test]
fn linear_decode_continues_after_return_and_reports_a_truncated_instruction() {
    let image = raw(&[0xC3, 0x90, 0xE8, 0x01], 0x1000);
    let first = disassemble(&image, 0x1000, 1).unwrap();
    assert_eq!(first.instructions[0].text, "ret");
    assert_eq!(first.next_address, Some(0x1001));
    let following = disassemble(&image, first.next_address.unwrap(), 10).unwrap();
    assert_eq!(following.instructions[0].text, "nop");
    assert_eq!(following.invalid_at, Some(0x1002));
    assert!(following.next_address.is_none());
}

#[test]
fn register_ssa_merges_values_at_a_join_and_at_a_loop_header() {
    // test ecx,ecx; je right; mov eax,1; jmp exit; right: mov eax,2; exit: ret
    let image = raw(
        &[
            0x85, 0xC9, 0x74, 7, 0xB8, 1, 0, 0, 0, 0xEB, 5, 0xB8, 2, 0, 0, 0, 0xC3,
        ],
        0x1000,
    );
    let lifted = lift(&analyze_function(&image, 0x1000).unwrap());
    let join = lifted
        .blocks
        .iter()
        .find(|block| block.address == 0x1010)
        .unwrap();
    let eax = join
        .inputs
        .iter()
        .find(|input| input.name == "b1010_eax_in")
        .unwrap();
    assert_eq!(
        eax.incoming
            .iter()
            .map(|input| (input.predecessor, input.value.as_str()))
            .collect::<Vec<_>>(),
        [(Some(0x1004), "b1004_eax_1"), (Some(0x100B), "b100b_eax_1")]
    );
    assert_eq!(join.statements[0].text, "return b1010_eax_in");
    assert!(lifted.register_pseudocode.is_none());

    let looping =
        lift(&analyze_function(&raw(&[0x83, 0xC0, 1, 0xEB, 0xFB], 0x1000), 0x1000).unwrap());
    let input = &looping.blocks[0].inputs[0];
    assert_eq!(input.name, "eax_in");
    assert_eq!(input.incoming[0].predecessor, None);
    assert_eq!(input.incoming[0].value, "eax_entry");
    assert_eq!(input.incoming[1].predecessor, Some(0x1000));
    assert_eq!(input.incoming[1].value, "eax_1");
}
