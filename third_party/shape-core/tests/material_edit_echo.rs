//! Opt-in real-process contract: `SHAPE_ECHO_BINARY` and `SHAPE_ECHO_EVIDENCE` are required.
use shape_core::{MaterialEditTicket, ShapeProject};
use shape_domain::{
    ArtifactContentContract, ContentDigest, MaterialEditor, MaterialReturnNormalization,
    TransformationOperation, WorkingInput,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[test]
#[ignore = "requires the separately built Echo executable; isolated synthetic materials only"]
fn real_echo_return_requires_conversion_then_accepts_and_refreshes_downstream() {
    let binary = PathBuf::from(std::env::var_os("SHAPE_ECHO_BINARY").expect("Echo binary"));
    let root = PathBuf::from(std::env::var_os("SHAPE_ECHO_EVIDENCE").expect("new evidence path"));
    fs::create_dir(&root).unwrap();
    let source = root.join("source.wav");
    let wav = synthetic_wav();
    fs::write(&source, &wav).unwrap();
    let mut project = ShapeProject::create(root.join("fixture.shape"), "Echo integration").unwrap();
    let original = project.import_audio_wav(&source, "Original").unwrap();
    let ticket = project
        .create_material_edit(
            WorkingInput {
                artifact_id: original.artifact_id,
                revision_id: original.id,
            },
            "Echo edited",
            MaterialEditor::Echo,
        )
        .unwrap();
    let handoff = project
        .export_material_edit(&ticket, &root.join("handoff"))
        .unwrap();
    let report = run_echo(&binary, &handoff.input_path, &root);
    assert_eq!(
        report["ready"]["snapshot"]["base"]["content_hash_blake3"],
        ticket.input_content.digest.to_string()
    );
    let mut accepted = Vec::new();
    let mut downstream = None;
    for index in 0..2 {
        let current = project
            .material_edit_ticket(ticket.output_artifact_id)
            .unwrap();
        let returned = root.join(format!("echo/return-{index}.wav"));
        let revision = accept_echo_return(
            &mut project,
            &current,
            &returned,
            &report["receipts"][index],
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
                        MaterialEditor::Echo,
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
    assert_eq!(
        project
            .read_accepted(original.artifact_id)
            .unwrap()
            .unwrap()
            .bytes,
        wav
    );
    drop(project);
    let project = ShapeProject::open(root.join("fixture.shape")).unwrap();
    assert_eq!(
        project
            .material_edit_ticket(ticket.output_artifact_id)
            .unwrap()
            .expected_output_head,
        Some(accepted[1].id)
    );
    fs::write(
        root.join("shape-acceptance.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "original": original, "accepted": accepted, "downstream_before": downstream,
            "downstream_after": refreshed, "real_echo": true, "explicit_pcm24_conversion": true,
            "original_bytes_unchanged": true, "reopened": true,
        }))
        .unwrap(),
    )
    .unwrap();
}

fn synthetic_wav() -> Vec<u8> {
    let samples: Vec<u8> = (0..44_100_i32)
        .flat_map(|n| {
            let value = i16::try_from((n % 441 - 220) * 80).unwrap();
            value.to_le_bytes()
        })
        .collect();
    let len = u32::try_from(samples.len()).unwrap();
    let mut wav = b"RIFF".to_vec();
    wav.extend((36 + len).to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16_u32.to_le_bytes());
    wav.extend(1_u16.to_le_bytes());
    wav.extend(1_u16.to_le_bytes());
    wav.extend(44_100_u32.to_le_bytes());
    wav.extend(88_200_u32.to_le_bytes());
    wav.extend(2_u16.to_le_bytes());
    wav.extend(16_u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend(len.to_le_bytes());
    wav.extend(samples);
    wav
}

fn accept_echo_return(
    project: &mut ShapeProject,
    current: &MaterialEditTicket,
    returned: &Path,
    receipt: &serde_json::Value,
) -> shape_domain::ArtifactRevision {
    let raw = fs::read(returned).unwrap();
    let digest = ContentDigest::from_bytes(&raw);
    assert_eq!(receipt["output_hash_blake3"], digest.to_string());
    assert!(
        project
            .review_material_return_file(current, returned)
            .is_err()
    );
    let candidate = project
        .material_return_file_with_options(current, returned, Some(digest), true)
        .unwrap();
    assert_eq!(
        candidate.normalization(),
        MaterialReturnNormalization::Pcm24ToPcm16
    );
    assert_eq!(candidate.returned_digest(), digest);
    let ArtifactContentContract::AudioClip(contract) = candidate.contract() else {
        panic!("audio")
    };
    assert_eq!(contract.sample_rate_hz, 48_000);
    assert_eq!(contract.channels, 2);
    assert_ne!(ContentDigest::from_bytes(candidate.bytes()), digest);
    let revision = project.accept_material_return(candidate).unwrap();
    let transformation = project.transformation(revision.transformation_id).unwrap();
    let Some(TransformationOperation::MaterialEdit(operation)) = transformation.operation else {
        panic!("material operation")
    };
    assert_eq!(operation.returned_digest, digest);
    assert_eq!(
        operation.normalization,
        MaterialReturnNormalization::Pcm24ToPcm16
    );
    assert!(
        project
            .material_return_file_with_options(current, returned, Some(digest), true)
            .is_err()
    );
    revision
}

fn run_echo(binary: &Path, source: &Path, root: &Path) -> serde_json::Value {
    let output = Command::new("python3")
        .arg("-B")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/test_material_echo.py"))
        .arg("--binary")
        .arg(binary)
        .arg("--source")
        .arg(source)
        .arg("--evidence")
        .arg(root.join("echo"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("echo/report.json")).unwrap()).unwrap();
    report
}
