use std::path::PathBuf;

use image::{ColorType, ImageEncoder, Rgba, RgbaImage, codecs::png::PngEncoder};
use shape_domain::{RasterGaussianBlur, TransformationOperation};
use uuid::Uuid;

use super::*;

fn project_path() -> PathBuf {
    std::env::temp_dir().join(format!("shape-raster-blur-core-{}", Uuid::now_v7()))
}

fn source_png(path: &std::path::Path) {
    let mut image = RgbaImage::from_pixel(5, 3, Rgba([255, 0, 0, 0]));
    image.put_pixel(2, 1, Rgba([0, 0, 255, 255]));
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(image.as_raw(), 5, 3, ColorType::Rgba8.into())
        .unwrap();
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn candidate_accept_and_reopen_preserve_blur_semantics() {
    let root = project_path();
    ShapeProject::create(&root, "Blur").unwrap();
    let source = root.join("source.png");
    source_png(&source);
    let mut project = ShapeProject::open(&root).unwrap();
    let imported = project.import_raster(&source, "Portrait").unwrap();
    let blur = RasterGaussianBlur::new(2).unwrap();

    let candidate = project
        .propose_raster_blur(imported.artifact_id, imported.id, blur)
        .unwrap();
    assert_eq!(candidate.contract().width, 5);
    assert_eq!(candidate.contract().height, 3);
    assert_eq!(
        project.snapshot().unwrap().artifacts[0].accepted_revision,
        Some(imported.id)
    );
    assert!(matches!(
        candidate.operation(),
        Some(TransformationOperation::RasterGaussianBlur(parameters)) if *parameters == blur
    ));
    let decoded = image::load_from_memory(candidate.png_bytes())
        .unwrap()
        .to_rgba8();
    assert!(decoded.pixels().all(|pixel| pixel.0[0] == 0));

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
        Some(TransformationOperation::RasterGaussianBlur(blur))
    );
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}
