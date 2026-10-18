//! Actual Shadow external-stage roundtrip; opt in with `SHAPE_SHADOW_BINARY` and `SHAPE_SHADOW_EVIDENCE`.
use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use shape_core::ShapeProject;
use shape_domain::{
    ContentDigest, MaterialEditor, MaterialReturnNormalization, TransformationOperation,
    WorkingInput,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn run_shadow(binary: &Path, directory: &Path, evidence: &Path, exposure: &str) {
    let output = Command::new("python3")
        .arg("-B")
        .arg(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/test_material_shadow.py"),
        )
        .arg("--binary")
        .arg(binary)
        .arg("--request")
        .arg(directory.join("shadow-request.json"))
        .arg("--result")
        .arg(directory.join("shadow-result.json"))
        .arg("--evidence")
        .arg(evidence)
        .arg("--exposure")
        .arg(exposure)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires separately built Shadow; isolated synthetic pixels only"]
#[allow(
    clippy::too_many_lines,
    reason = "One real lifecycle preserves source, repeated ICC handoff, accepted lineage and reopened state together"
)]
fn real_shadow_completed_return_accepts_separate_versions_and_refreshes_downstream() {
    let binary = PathBuf::from(std::env::var_os("SHAPE_SHADOW_BINARY").expect("Shadow binary"));
    let root =
        PathBuf::from(std::env::var_os("SHAPE_SHADOW_EVIDENCE").expect("new evidence directory"));
    fs::create_dir(&root).unwrap();
    let png = synthetic_png();
    let mut project =
        ShapeProject::create(root.join("fixture.shape"), "Shadow integration").unwrap();
    let original = project.import_raster_bytes(png, "Original").unwrap();
    let original_bytes = project
        .read_accepted(original.artifact_id)
        .unwrap()
        .unwrap()
        .bytes;
    let ticket = project
        .create_material_edit(
            WorkingInput {
                artifact_id: original.artifact_id,
                revision_id: original.id,
            },
            "Shadow edited",
            MaterialEditor::Shadow,
        )
        .unwrap();
    let mut accepted = Vec::new();
    let mut downstream = None;
    for (index, exposure) in ["0.75", "-0.75"].into_iter().enumerate() {
        let current = project
            .material_edit_ticket(ticket.output_artifact_id)
            .unwrap();
        let directory = root.join(format!("handoff-{index}"));
        let handoff = project.export_material_edit(&current, &directory).unwrap();
        run_shadow(
            &binary,
            &directory,
            &root.join(format!("shadow-{index}")),
            exposure,
        );
        let result = directory.join("shadow-result.json");
        let candidate = project
            .propose_shadow_return(&handoff, &result, None)
            .unwrap();
        assert_eq!(
            candidate.returned_digest(),
            ContentDigest::from_bytes(&fs::read(directory.join("edited.png")).unwrap())
        );
        let revision = project.accept_material_return(candidate).unwrap();
        assert!(
            project
                .propose_shadow_return(&handoff, &result, None)
                .is_err()
        );
        if index == 0 {
            downstream = Some(
                project
                    .create_material_edit(
                        WorkingInput {
                            artifact_id: revision.artifact_id,
                            revision_id: revision.id,
                        },
                        "Downstream",
                        MaterialEditor::Shadow,
                    )
                    .unwrap(),
            );
        }
        accepted.push(revision);
    }
    let downstream = downstream.unwrap();
    assert!(project.material_edit_source(&downstream).is_err());
    assert_eq!(
        downstream.draft.input().unwrap().revision_id,
        accepted[0].id
    );
    let refreshed = project.restart_material_edit(&downstream, true).unwrap();
    assert_eq!(refreshed.draft.input().unwrap().revision_id, accepted[1].id);
    // Re-enter an actual ICC-tagged Shadow result as the next page-edit source.
    let returned_source = WorkingInput {
        artifact_id: accepted[1].artifact_id,
        revision_id: accepted[1].id,
    };
    let returned_original = project
        .read_revision_content(returned_source.revision_id)
        .unwrap()
        .bytes;
    let repeated_ticket = project
        .create_material_edit(
            returned_source,
            "Repeated Shadow adjustment",
            MaterialEditor::Shadow,
        )
        .unwrap();
    let repeated_directory = root.join("handoff-repeated");
    let repeated_handoff = project
        .export_material_edit(&repeated_ticket, &repeated_directory)
        .unwrap();
    assert!(matches!(
        repeated_handoff.input_materialization,
        Some(MaterialReturnNormalization::KnownSrgbIccInput { .. })
    ));
    assert_eq!(
        fs::read(&repeated_handoff.input_path).unwrap(),
        returned_original
    );
    let normalized_path = repeated_directory.join("shadow-source.png");
    let normalized = fs::read(&normalized_path).unwrap();
    let decoded = image::load_from_memory(&normalized).unwrap().to_rgba8();
    assert_eq!(
        decoded,
        image::load_from_memory(&returned_original)
            .unwrap()
            .to_rgba8()
    );
    let mut changed = normalized.clone();
    changed[32] ^= 1;
    fs::write(&normalized_path, changed).unwrap();
    assert!(
        project.shadow_material_request(&repeated_handoff).is_err(),
        "altered working input cannot launch or return"
    );
    fs::write(&normalized_path, normalized).unwrap();
    run_shadow(
        &binary,
        &repeated_directory,
        &root.join("shadow-repeated"),
        "0.25",
    );
    let repeated_candidate = project
        .propose_shadow_return(
            &repeated_handoff,
            &repeated_directory.join("shadow-result.json"),
            None,
        )
        .unwrap();
    assert_eq!(
        Some(repeated_candidate.normalization()),
        repeated_handoff.input_materialization
    );
    let repeated = project.accept_material_return(repeated_candidate).unwrap();
    let transformation = project.transformation(repeated.transformation_id).unwrap();
    let Some(TransformationOperation::MaterialEdit(operation)) = transformation.operation else {
        panic!("material operation")
    };
    assert_eq!(
        Some(operation.normalization),
        repeated_handoff.input_materialization
    );
    assert_eq!(operation.source, returned_source);
    assert_eq!(
        project
            .read_revision_content(returned_source.revision_id)
            .unwrap()
            .bytes,
        returned_original
    );
    assert_eq!(
        project
            .read_accepted(original.artifact_id)
            .unwrap()
            .unwrap()
            .bytes,
        original_bytes
    );
    drop(project);
    let project = ShapeProject::open(root.join("fixture.shape")).unwrap();
    assert_eq!(
        project
            .read_accepted(repeated.artifact_id)
            .unwrap()
            .unwrap()
            .revision
            .id,
        repeated.id
    );
    assert_eq!(
        project
            .material_edit_ticket(ticket.output_artifact_id)
            .unwrap()
            .expected_output_head,
        Some(accepted[1].id)
    );
    fs::write(root.join("shape-acceptance.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "original":original,"accepted":accepted,"downstream_before":downstream,"downstream_after":refreshed,
        "completed_receipts_verified":true,"original_bytes_unchanged":true,"reopened":true,
        "repeated_icc_source":returned_source,"repeated":repeated,
        "input_materialization":repeated_handoff.input_materialization,"repeated_pixels_preserved":true
    })).unwrap()).unwrap();
}

fn synthetic_png() -> Vec<u8> {
    let mut pixels = Vec::new();
    for y in 0..426_u32 {
        for x in 0..640_u32 {
            pixels.extend([
                u8::try_from(20 + x * 130 / 640).unwrap(),
                u8::try_from(24 + y * 115 / 426).unwrap(),
                70,
                255,
            ]);
        }
    }
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(&pixels, 640, 426, ColorType::Rgba8.into())
        .unwrap();
    png
}
