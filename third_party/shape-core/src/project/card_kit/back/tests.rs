use super::*;
use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use shape_domain::{RasterTransform, WorkingInput};

#[test]
fn back_artwork_is_an_accepted_dependency_and_unaccepted_back_edits_survive_reopen() {
    let root = std::env::temp_dir().join(format!("deck-back-{}", uuid::Uuid::now_v7()));
    let path = root.join("back.deck");
    let mut project = ShapeProject::create(&path, "Back fixture").unwrap();
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(&[21, 31, 41, 255], 1, 1, ColorType::Rgba8.into())
        .unwrap();
    let artwork = project.import_raster_bytes(png, "Shared back").unwrap();
    let pin = WorkingInput {
        artifact_id: artwork.artifact_id,
        revision_id: artwork.id,
    };
    let request: CardKitRequest = serde_json::from_value(json!({"name":"Stable cards","layout":{"mode":"text"},"cards":[{"id":"old-stable-id","title":"Front","quantity":2}],"back":{"mode":"image","artwork":pin,"image_fit":"cover"}})).unwrap();
    let draft = project.create_card_kit_draft(&request).unwrap();
    let candidate = project
        .propose_card_kit(draft.artifact_id, &draft.draft_digest)
        .unwrap();
    let accepted = project.accept_card_kit(candidate).unwrap();
    let source = WorkingInput {
        artifact_id: accepted.artifact_id,
        revision_id: accepted.id,
    };
    let bytes = project.read_revision_content(accepted.id).unwrap().bytes;
    assert_eq!(project.read_card_kit(source).unwrap(), request);
    assert!(
        project.preview_card_kit_with_back(&request, None).unwrap()["back"]["svg"]
            .as_str()
            .unwrap()
            .contains("data:image/png;base64,")
    );
    let current = project.open_card_kit_draft(draft.artifact_id).unwrap();
    let mut changed = current.composition.clone();
    changed.back = Some(serde_json::from_value(json!({"mode":"solid","color":"#123456"})).unwrap());
    project
        .save_card_kit_draft(draft.artifact_id, &current.draft_digest, &changed)
        .unwrap();
    drop(project);
    let mut project = ShapeProject::open(&path).unwrap();
    assert_eq!(
        project
            .open_card_kit_draft(draft.artifact_id)
            .unwrap()
            .composition,
        changed
    );
    assert_eq!(
        project.read_revision_content(accepted.id).unwrap().bytes,
        bytes
    );
    assert_eq!(project.read_card_kit(source).unwrap().back, request.back);
    let pending = project.create_card_kit_draft(&request).unwrap();
    let pending_candidate = project
        .propose_card_kit(pending.artifact_id, &pending.draft_digest)
        .unwrap();
    let transformed = project
        .propose_raster_transform(pin.artifact_id, pin.revision_id, RasterTransform::Rotate180)
        .unwrap();
    project.accept_raster_edit(transformed).unwrap();
    assert!(
        project.accept_card_kit(pending_candidate).is_err(),
        "back material heads participate in acceptance CAS"
    );
    assert!(
        project
            .render_card_back(&request, 63.0 / 750.0)
            .unwrap()
            .contains("data:image/png;base64,"),
        "historical accepted back pin remains printable"
    );
    drop(project);
    std::fs::remove_dir_all(root).unwrap();
}
