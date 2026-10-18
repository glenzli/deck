use std::path::PathBuf;

use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use shape_domain::{ArtifactContentContract, RasterTransform, TransformationOperation};
use uuid::Uuid;

use super::*;

fn project_path() -> PathBuf {
    std::env::temp_dir().join(format!("shape-raster-transform-core-{}", Uuid::now_v7()))
}

fn source_png(path: &std::path::Path) {
    let pixels = [
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255, 255, 0, 255, 255, 0, 255,
        255, 255,
    ];
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(&pixels, 3, 2, ColorType::Rgba8.into())
        .unwrap();
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn candidate_accept_and_reopen_keep_transform_layers_separate() {
    let root = project_path();
    ShapeProject::create(&root, "Transform").unwrap();
    let source = root.join("source.png");
    source_png(&source);
    let mut project = ShapeProject::open(&root).unwrap();
    let imported = project.import_raster(&source, "Portrait").unwrap();

    let candidate = project
        .propose_raster_transform(
            imported.artifact_id,
            imported.id,
            RasterTransform::Rotate90Clockwise,
        )
        .unwrap();
    assert_eq!(candidate.contract().width, 2);
    assert_eq!(candidate.contract().height, 3);
    assert_eq!(
        project.snapshot().unwrap().artifacts[0].accepted_revision,
        Some(imported.id)
    );
    assert!(matches!(
        candidate.operation(),
        Some(TransformationOperation::RasterTransform(
            RasterTransform::Rotate90Clockwise
        ))
    ));

    let accepted = project.accept_raster_edit(candidate).unwrap();
    drop(project);
    let reopened = ShapeProject::open(&root).unwrap();
    let content = reopened
        .read_accepted(imported.artifact_id)
        .unwrap()
        .unwrap();
    assert_eq!(content.revision.id, accepted.id);
    let Some(ArtifactContentContract::ImageRaster(contract)) = content.revision.content_contract
    else {
        panic!("transform contract missing");
    };
    assert_eq!((contract.width, contract.height), (2, 3));
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn stale_transform_candidate_cannot_overwrite_an_accepted_edit() {
    let root = project_path();
    ShapeProject::create(&root, "Transform stale").unwrap();
    let source = root.join("source.png");
    source_png(&source);
    let mut project = ShapeProject::open(&root).unwrap();
    let imported = project.import_raster(&source, "Portrait").unwrap();
    let winner = project
        .propose_raster_transform(
            imported.artifact_id,
            imported.id,
            RasterTransform::Rotate180,
        )
        .unwrap();
    let stale = project
        .propose_raster_transform(
            imported.artifact_id,
            imported.id,
            RasterTransform::FlipVertical,
        )
        .unwrap();

    project.accept_raster_edit(winner).unwrap();
    assert!(matches!(
        project.accept_raster_edit(stale),
        Err(CoreError::Store(
            shape_store::StoreError::RevisionConflict { .. }
        ))
    ));
    drop(project);
    std::fs::remove_dir_all(root).unwrap();
}
