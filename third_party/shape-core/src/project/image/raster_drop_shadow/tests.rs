use std::path::PathBuf;

use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use shape_domain::{RasterDropShadow, RasterShadowColor, TransformationOperation};
use uuid::Uuid;

use super::*;

fn project_path() -> PathBuf {
    std::env::temp_dir().join(format!("shape-raster-drop-shadow-core-{}", Uuid::now_v7()))
}

fn source_png(path: &std::path::Path) {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(&[255, 0, 0, 255], 1, 1, ColorType::Rgba8.into())
        .unwrap();
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn candidate_accept_and_reopen_preserve_shadow_semantics() {
    let root = project_path();
    ShapeProject::create(&root, "Drop shadow").unwrap();
    let source = root.join("source.png");
    source_png(&source);
    let mut project = ShapeProject::open(&root).unwrap();
    let imported = project.import_raster(&source, "Badge").unwrap();
    let shadow =
        RasterDropShadow::new(1, 0, 0, RasterShadowColor::new(0, 0, 0, 128).unwrap()).unwrap();

    let candidate = project
        .propose_raster_drop_shadow(imported.artifact_id, imported.id, shadow)
        .unwrap();
    assert_eq!(
        (candidate.contract().width, candidate.contract().height),
        (2, 1)
    );
    assert_eq!(
        project.snapshot().unwrap().artifacts[0].accepted_revision,
        Some(imported.id)
    );
    assert!(matches!(
        candidate.operation(),
        Some(TransformationOperation::RasterDropShadow(parameters)) if *parameters == shadow
    ));
    let decoded = image::load_from_memory(candidate.png_bytes())
        .unwrap()
        .to_rgba8();
    assert_eq!(decoded.get_pixel(0, 0).0, [255, 0, 0, 255]);
    assert_eq!(decoded.get_pixel(1, 0).0, [0, 0, 0, 128]);

    let accepted = project.accept_raster_edit(candidate).unwrap();
    drop(project);
    let reopened = ShapeProject::open(&root).unwrap();
    let content = reopened
        .read_accepted(imported.artifact_id)
        .unwrap()
        .unwrap();
    assert_eq!(content.revision.id, accepted.id);
    let transformation = reopened
        .transformation(content.revision.transformation_id)
        .unwrap();
    assert_eq!(
        transformation.operation,
        Some(TransformationOperation::RasterDropShadow(shadow))
    );
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}
