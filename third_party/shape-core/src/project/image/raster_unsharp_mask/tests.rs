use std::path::PathBuf;

use image::{ColorType, ImageEncoder, Rgba, RgbaImage, codecs::png::PngEncoder};
use shape_domain::{RasterUnsharpMask, TransformationOperation};
use uuid::Uuid;

use super::*;

fn project_path() -> PathBuf {
    std::env::temp_dir().join(format!("shape-raster-unsharp-core-{}", Uuid::now_v7()))
}

fn source_png(path: &std::path::Path) {
    let mut image = RgbaImage::from_pixel(5, 3, Rgba([255, 0, 0, 0]));
    image.put_pixel(1, 1, Rgba([64, 64, 64, 128]));
    image.put_pixel(2, 1, Rgba([160, 160, 160, 255]));
    image.put_pixel(3, 1, Rgba([64, 64, 64, 128]));
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(image.as_raw(), 5, 3, ColorType::Rgba8.into())
        .unwrap();
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn candidate_accept_and_reopen_preserve_unsharp_mask_semantics() {
    let root = project_path();
    ShapeProject::create(&root, "Unsharp Mask").unwrap();
    let source = root.join("source.png");
    source_png(&source);
    let mut project = ShapeProject::open(&root).unwrap();
    let imported = project.import_raster(&source, "Portrait").unwrap();
    let parameters = RasterUnsharpMask::new(2, 1_250, 4).unwrap();

    let candidate = project
        .propose_raster_unsharp_mask(imported.artifact_id, imported.id, parameters)
        .unwrap();
    assert_eq!(candidate.contract().width, 5);
    assert_eq!(candidate.contract().height, 3);
    assert_eq!(
        project.snapshot().unwrap().artifacts[0].accepted_revision,
        Some(imported.id)
    );
    assert!(matches!(
        candidate.operation(),
        Some(TransformationOperation::RasterUnsharpMask(existing)) if *existing == parameters
    ));

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
        Some(TransformationOperation::RasterUnsharpMask(parameters))
    );
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn stale_unsharp_candidate_cannot_overwrite_an_accepted_edit() {
    let root = project_path();
    ShapeProject::create(&root, "Unsharp Stale").unwrap();
    let source = root.join("source.png");
    source_png(&source);
    let mut project = ShapeProject::open(&root).unwrap();
    let imported = project.import_raster(&source, "Portrait").unwrap();
    let first = project
        .propose_raster_unsharp_mask(
            imported.artifact_id,
            imported.id,
            RasterUnsharpMask::new(1, 1_000, 0).unwrap(),
        )
        .unwrap();
    let stale = project
        .propose_raster_unsharp_mask(
            imported.artifact_id,
            imported.id,
            RasterUnsharpMask::new(2, 1_000, 0).unwrap(),
        )
        .unwrap();
    let winner = project.accept_raster_edit(first).unwrap();
    assert!(matches!(
        project.accept_raster_edit(stale),
        Err(CoreError::Store(
            shape_store::StoreError::RevisionConflict { .. }
        ))
    ));
    assert_eq!(
        project.snapshot().unwrap().artifacts[0].accepted_revision,
        Some(winner.id)
    );
    std::fs::remove_dir_all(root).unwrap();
}
