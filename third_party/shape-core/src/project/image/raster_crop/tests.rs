use std::{
    fs,
    path::{Path, PathBuf},
};

use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use shape_domain::{ArtifactContentContract, RasterCrop, TransformationOperation};
use shape_execution::RASTER_CROP_CAPABILITY;
use uuid::Uuid;

use super::*;

fn test_root() -> PathBuf {
    std::env::temp_dir().join(format!("shape-raster-crop-core-{}", Uuid::now_v7()))
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
fn candidate_accept_and_reopen_keep_creative_and_physical_layers_separate() {
    let root = test_root();
    let source = root.with_extension("png");
    fixture(&source);
    let mut project = ShapeProject::create(&root, "Raster Crop Project").unwrap();
    let imported = project.import_raster(&source, "Cover").unwrap();
    let artifact_id = imported.artifact_id;
    let crop = RasterCrop::new(1, 1, 3, 2, 6, 4).unwrap();
    let candidate = project
        .propose_raster_crop(artifact_id, imported.id, crop)
        .unwrap();
    assert_eq!(
        (candidate.contract().width, candidate.contract().height),
        (3, 2)
    );
    assert_eq!(
        candidate.receipt().capability.as_str(),
        RASTER_CROP_CAPABILITY
    );
    assert_eq!(candidate.receipt().executor.id, "shape.builtin.raster.crop");
    assert_eq!(
        project.snapshot().unwrap().artifacts[0].accepted_revision,
        Some(imported.id)
    );

    let accepted = project.accept_raster_crop(candidate).unwrap();
    assert_eq!(accepted.parents, vec![imported.id]);
    drop(project);

    let reopened = ShapeProject::open(&root).unwrap();
    let content = reopened.read_accepted(artifact_id).unwrap().unwrap();
    let Some(ArtifactContentContract::ImageRaster(contract)) = content.revision.content_contract
    else {
        panic!("crop contract missing");
    };
    assert_eq!((contract.width, contract.height), (3, 2));
    let transformation = reopened
        .transformation(content.revision.transformation_id)
        .unwrap();
    assert_eq!(
        transformation.operation,
        Some(TransformationOperation::RasterCrop(crop))
    );
    assert_eq!(
        (
            image::load_from_memory(&content.bytes).unwrap().width(),
            image::load_from_memory(&content.bytes).unwrap().height()
        ),
        (3, 2)
    );
    fs::remove_file(source).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn identity_and_stale_requests_fail_without_advancing_history() {
    let root = test_root();
    let source = root.with_extension("png");
    fixture(&source);
    let mut project = ShapeProject::create(&root, "Raster Crop Project").unwrap();
    let imported = project.import_raster(&source, "Cover").unwrap();
    let identity = project.propose_raster_crop(
        imported.artifact_id,
        imported.id,
        RasterCrop::new(0, 0, 6, 4, 6, 4).unwrap(),
    );
    assert!(matches!(identity, Err(CoreError::NoOpRasterCrop)));
    assert_eq!(
        project.snapshot().unwrap().artifacts[0].accepted_revision,
        Some(imported.id)
    );

    let candidate = project
        .propose_raster_crop(
            imported.artifact_id,
            imported.id,
            RasterCrop::new(1, 1, 3, 2, 6, 4).unwrap(),
        )
        .unwrap();
    let accepted = project.accept_raster_crop(candidate).unwrap();
    let stale = project.propose_raster_crop(
        imported.artifact_id,
        imported.id,
        RasterCrop::new(0, 0, 2, 2, 6, 4).unwrap(),
    );
    assert!(matches!(stale, Err(CoreError::StaleCandidate { .. })));
    assert_eq!(
        project.snapshot().unwrap().artifacts[0].accepted_revision,
        Some(accepted.id)
    );
    fs::remove_file(source).unwrap();
    fs::remove_dir_all(root).unwrap();
}
