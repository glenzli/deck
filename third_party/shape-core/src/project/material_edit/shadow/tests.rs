use super::*;
use crate::project::material_edit::tests::{Fixture, png};
use std::fs;

#[test]
fn completed_receipt_binds_source_paths_hashes_dimensions_and_success_before_candidate() {
    let mut f = Fixture::image();
    let ticket = f.stage();
    let external =
        std::env::temp_dir().join(format!("shape-shadow-receipt-{}", uuid::Uuid::now_v7()));
    let handoff = f.project.export_material_edit(&ticket, &external).unwrap();
    let request: Value =
        serde_json::from_slice(&fs::read(external.join("shadow-request.json")).unwrap()).unwrap();
    let raw = png(90);
    fs::write(external.join("edited.png"), &raw).unwrap();
    let good = json!({"schema":"shadow.external-edit-result/1","requestId":request["requestId"],"input":request["input"],
        "outcome":"completed","errors":[],"artifact":{"path":request["output"]["path"],"sha256":sha256(&raw),
        "byteLength":raw.len().to_string(),"width":4,"height":2,"format":"png","colorSpace":"sRGB","bitDepth":8},
        "render":{"sessionId":"test-session","photoId":"photo","representationId":"representation","recipeCommitId":"commit","recipeSnapshotDigest":"a".repeat(64)}});
    let path = external.join("shadow-result.json");
    for (pointer, value) in [
        ("/outcome", json!("failed")),
        ("/outcome", json!("cancelled")),
        ("/requestId", json!("foreign")),
        ("/artifact/colorSpace", json!("unknown")),
        ("/artifact/bitDepth", json!(16)),
        ("/artifact/width", json!(9)),
        ("/artifact/sha256", json!("0".repeat(64))),
        ("/artifact/path", json!(external.join("source.png"))),
        ("/input/revision", json!("stale")),
        ("/input/sha256", json!("0".repeat(64))),
    ] {
        let mut bad = good.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(
            f.project
                .propose_shadow_return(&handoff, &path, None)
                .is_err(),
            "{pointer}"
        );
        assert!(
            f.project
                .read_accepted(ticket.output_artifact_id)
                .unwrap()
                .is_none()
        );
    }
    fs::write(&path, serde_json::to_vec(&good).unwrap()).unwrap();
    assert!(
        f.project
            .propose_shadow_return(&handoff, &path, Some(ContentDigest::from_bytes(b"changed")))
            .is_err()
    );
    let candidate = f
        .project
        .propose_shadow_return(&handoff, &path, None)
        .unwrap();
    let mut forged = candidate.clone();
    let Some(TransformationOperation::MaterialEdit(operation)) =
        &mut forged.transformation.operation
    else {
        panic!("material operation")
    };
    operation.normalization = MaterialReturnNormalization::KnownSrgbIccInput {
        profile_digest: ContentDigest::from_bytes(b"forged ICC"),
        materialized_digest: ContentDigest::from_bytes(b"forged working pixels"),
    };
    assert!(
        f.project.accept_material_return(forged).is_err(),
        "store independently rejects a claimed input materialization not derived from source"
    );
    f.project.accept_material_return(candidate).unwrap();
    assert!(
        f.project
            .propose_shadow_return(&handoff, &path, None)
            .is_err()
    );
    // A terminal provider result must not be rebound to a later output-head ticket.
    let current = f
        .project
        .material_edit_ticket(ticket.output_artifact_id)
        .unwrap();
    let next = png(91);
    let candidate = f
        .project
        .propose_material_return(&current, next.clone(), ContentDigest::from_bytes(&next))
        .unwrap();
    f.project.accept_material_return(candidate).unwrap();
    let mut rebound = handoff.clone();
    rebound.ticket = f
        .project
        .material_edit_ticket(ticket.output_artifact_id)
        .unwrap();
    assert!(
        f.project
            .propose_shadow_return(&rebound, &path, None)
            .is_err(),
        "old provider receipt must not bind a new output head"
    );
    fs::remove_dir_all(external).unwrap();
}
