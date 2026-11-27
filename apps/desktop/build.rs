use std::{env, fs, path::PathBuf};

use resvg::{tiny_skia, usvg};

const ICON_PATH: &str = "../../assets/branding/hexaris-icon.svg";

fn main() {
    println!("cargo:rerun-if-changed={ICON_PATH}");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    let source = fs::read(ICON_PATH).expect("read Hexaris icon");
    let tree =
        usvg::Tree::from_data(&source, &usvg::Options::default()).expect("parse Hexaris icon");
    fs::write(out.join("hexaris-icon.png"), render(&tree, 128)).expect("write window icon");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        let resource = out.join("hexaris-icon.res");
        fs::write(&resource, icon_resource(&tree)).expect("write Windows icon resource");
        println!("cargo:rustc-link-arg={}", resource.display());
    }
}

fn render(tree: &usvg::Tree, size: u32) -> Vec<u8> {
    let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("allocate icon");
    let scale = size as f32 / tree.size().width();
    resvg::render(
        tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().expect("encode icon")
}

fn icon_resource(tree: &usvg::Tree) -> Vec<u8> {
    const SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
    let mut resource = Vec::new();
    write_resource(&mut resource, 0, 0, &[]);
    // GRPICONDIR followed by one directory entry for each RT_ICON resource.
    let mut group = vec![0, 0, 1, 0];
    group.extend_from_slice(&(SIZES.len() as u16).to_le_bytes());
    for (index, size) in SIZES.into_iter().enumerate() {
        let id = index as u16 + 1;
        let png = render(tree, size);
        write_resource(&mut resource, 3, id, &png);
        let dimension = size as u8; // The icon format represents 256 as zero.
        group.extend_from_slice(&[dimension, dimension, 0, 0, 1, 0, 32, 0]);
        group.extend_from_slice(&(png.len() as u32).to_le_bytes());
        group.extend_from_slice(&id.to_le_bytes());
    }
    write_resource(&mut resource, 14, 1, &group);
    resource
}

fn write_resource(output: &mut Vec<u8>, kind: u16, id: u16, data: &[u8]) {
    output.extend_from_slice(&(data.len() as u32).to_le_bytes());
    output.extend_from_slice(&32_u32.to_le_bytes());
    for ordinal in [kind, id] {
        output.extend_from_slice(&[0xff, 0xff]);
        output.extend_from_slice(&ordinal.to_le_bytes());
    }
    output.extend_from_slice(&[0; 4]); // Data version.
    let flags: u16 = if kind == 0 { 0 } else { 0x1030 };
    output.extend_from_slice(&flags.to_le_bytes());
    output.extend_from_slice(&[0; 10]); // Language, version and characteristics.
    output.extend_from_slice(data);
    output.resize(output.len().next_multiple_of(4), 0);
}
