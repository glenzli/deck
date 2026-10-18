use std::{
    fs,
    path::{Path, PathBuf},
};

use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use shape_domain::{
    ArtifactContentContract, RasterResize, RasterResizeAspectPolicy, RasterResizeDimensions,
    RasterResizeResampling, TransformationOperation,
};
use shape_execution::RASTER_RESIZE_CAPABILITY;
use uuid::Uuid;

use super::*;

fn test_root() -> PathBuf {
    std::env::temp_dir().join(format!("shape-raster-resize-core-{}", Uuid::now_v7()))
}

fn fixture(path: &Path) {
    let pixels: Vec<u8> = (0_u8..35)
        .flat_map(|value| [value, value.wrapping_add(20), 40, 127])
        .collect();
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(&pixels, 7, 5, ColorType::Rgba8.into())
        .unwrap();
    fs::write(path, bytes).unwrap();
}

fn parameters(width: u32, height: u32) -> RasterResize {
    RasterResize::new(
        RasterResizeDimensions::new(width, height).unwrap(),
        RasterResizeAspectPolicy::FitWithin,
        RasterResizeResampling::Lanczos3,
    )
}

#[test]
fn candidate_accept_and_reopen_keep_resize_layers_separate() {
    let root = test_root();
    let source = root.with_extension("png");
    fixture(&source);
    let mut project = ShapeProject::create(&root, "Raster Resize Project").unwrap();
    let imported = project.import_raster(&source, "Cover").unwrap();
    let artifact_id = imported.artifact_id;
    let resize = parameters(4, 4);
    let candidate = project
        .propose_raster_resize(artifact_id, imported.id, resize)
        .unwrap();
    assert_eq!(
        (candidate.contract().width, candidate.contract().height),
        (4, 3)
    );
    assert_eq!(
        candidate.receipt().capability.as_str(),
        RASTER_RESIZE_CAPABILITY
    );
    assert_eq!(
        candidate.receipt().executor.id,
        "shape.builtin.raster.resize"
    );
    assert_eq!(
        project.snapshot().unwrap().artifacts[0].accepted_revision,
        Some(imported.id)
    );

    let accepted = project.accept_raster_resize(candidate).unwrap();
    assert_eq!(accepted.parents, vec![imported.id]);
    drop(project);

    let reopened = ShapeProject::open(&root).unwrap();
    let content = reopened.read_accepted(artifact_id).unwrap().unwrap();
    let Some(ArtifactContentContract::ImageRaster(contract)) = content.revision.content_contract
    else {
        panic!("resize contract missing");
    };
    assert_eq!((contract.width, contract.height), (4, 3));
    let transformation = reopened
        .transformation(content.revision.transformation_id)
        .unwrap();
    assert_eq!(
        transformation.operation,
        Some(TransformationOperation::RasterResize(resize))
    );
    assert_eq!(
        (
            image::load_from_memory(&content.bytes).unwrap().width(),
            image::load_from_memory(&content.bytes).unwrap().height()
        ),
        (4, 3)
    );
    fs::remove_file(source).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn identity_and_stale_accept_fail_without_overwriting_the_winner() {
    let root = test_root();
    let source = root.with_extension("png");
    fixture(&source);
    let mut project = ShapeProject::create(&root, "Raster Resize Project").unwrap();
    let imported = project.import_raster(&source, "Cover").unwrap();
    let identity = RasterResize::new(
        RasterResizeDimensions::new(7, 5).unwrap(),
        RasterResizeAspectPolicy::Stretch,
        RasterResizeResampling::Nearest,
    );
    assert!(matches!(
        project.propose_raster_resize(imported.artifact_id, imported.id, identity),
        Err(CoreError::NoOpRasterResize)
    ));
    assert_eq!(
        project.snapshot().unwrap().artifacts[0].accepted_revision,
        Some(imported.id)
    );

    let winner = project
        .propose_raster_resize(imported.artifact_id, imported.id, parameters(4, 4))
        .unwrap();
    let stale = project
        .propose_raster_resize(imported.artifact_id, imported.id, parameters(3, 3))
        .unwrap();
    let accepted = project.accept_raster_resize(winner).unwrap();
    assert!(matches!(
        project.accept_raster_resize(stale),
        Err(CoreError::Store(
            shape_store::StoreError::RevisionConflict { .. }
        ))
    ));
    assert_eq!(
        project.snapshot().unwrap().artifacts[0].accepted_revision,
        Some(accepted.id)
    );
    fs::remove_file(source).unwrap();
    fs::remove_dir_all(root).unwrap();
}
