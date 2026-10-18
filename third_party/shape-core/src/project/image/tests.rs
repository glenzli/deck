use std::{fs, path::PathBuf};

use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use shape_domain::ArtifactContentContract;
use uuid::Uuid;

use super::*;

fn test_root() -> PathBuf {
    std::env::temp_dir().join(format!("shape-raster-core-{}", Uuid::now_v7()))
}

fn fixture(path: &Path) {
    let pixels: Vec<u8> = (0_u8..24).flat_map(|value| [value, 20, 40, 255]).collect();
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(&pixels, 6, 4, ColorType::Rgba8.into())
        .unwrap();
    fs::write(path, bytes).unwrap();
}

#[test]
fn clipboard_bytes_create_a_reopenable_image_without_a_source_file() {
    let root = test_root();
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(&[25, 60, 90, 255], 1, 1, ColorType::Rgba8.into())
        .unwrap();
    let mut project = ShapeProject::create(&root, "Clipboard Raster").unwrap();
    let revision = project.import_raster_bytes(png, "Pasted image").unwrap();
    drop(project);

    let reopened = ShapeProject::open(&root).unwrap();
    let accepted = reopened
        .read_accepted(revision.artifact_id)
        .unwrap()
        .unwrap();
    assert!(accepted.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(reopened.snapshot().unwrap().artifacts.len(), 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn empty_clipboard_image_does_not_create_an_artifact() {
    let root = test_root();
    let mut project = ShapeProject::create(&root, "Clipboard Raster").unwrap();
    assert!(matches!(
        project.import_raster_bytes(Vec::new(), "Pasted image"),
        Err(CoreError::InvalidRasterSource { .. })
    ));
    assert!(project.snapshot().unwrap().artifacts.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn import_materializes_an_accepted_canonical_raster_origin() {
    let root = test_root();
    let source = root.with_extension("png");
    fixture(&source);
    let mut project = ShapeProject::create(&root, "Raster Project").unwrap();
    let imported = project.import_raster(&source, "Cover").unwrap();
    let Some(ArtifactContentContract::ImageRaster(import_contract)) =
        imported.content_contract.as_ref()
    else {
        panic!("import contract missing");
    };
    assert_eq!((import_contract.width, import_contract.height), (6, 4));
    assert_eq!(
        project.snapshot().unwrap().artifacts[0].accepted_revision,
        Some(imported.id)
    );
    fs::remove_file(source).unwrap();
    fs::remove_dir_all(root).unwrap();
}
