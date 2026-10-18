use std::{fs, path::PathBuf};

use shape_domain::{ArtifactContentContract, ArtifactKind, AudioOriginDisclosure};
use uuid::Uuid;

use super::*;

fn root() -> PathBuf {
    std::env::temp_dir().join(format!("shape-file-import-{}", Uuid::now_v7()))
}

fn wav() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&40_u32.to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&24_000_u32.to_le_bytes());
    bytes.extend_from_slice(&48_000_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&4_u32.to_le_bytes());
    bytes.extend_from_slice(&[0_u8; 4]);
    bytes
}

#[test]
fn code_file_import_is_exact_text_and_survives_source_removal() {
    let root = root();
    let source = root.with_extension("js");
    let code = "export const frame = t => t * 2;\n";
    fs::write(&source, code).unwrap();
    let mut project = ShapeProject::create(&root, "Imported code").unwrap();
    let revision = project
        .import_text_file(&source, "Animation source")
        .unwrap();
    assert_eq!(
        project.snapshot().unwrap().artifacts[0].kind,
        ArtifactKind::TextDocument
    );
    fs::remove_file(source).unwrap();
    drop(project);
    let reopened = ShapeProject::open(&root).unwrap();
    assert_eq!(
        reopened
            .read_accepted(revision.artifact_id)
            .unwrap()
            .unwrap()
            .bytes,
        code.as_bytes()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn external_wav_import_is_exact_and_does_not_claim_synthetic_or_recorded_origin() {
    let root = root();
    let source = root.with_extension("wav");
    let bytes = wav();
    fs::write(&source, &bytes).unwrap();
    let mut project = ShapeProject::create(&root, "Imported audio").unwrap();
    let revision = project.import_audio_wav(&source, "External clip").unwrap();
    let Some(ArtifactContentContract::AudioClip(contract)) = revision.content_contract else {
        panic!("audio contract missing");
    };
    assert_eq!(contract.origin, AudioOriginDisclosure::ImportedUnverified);
    fs::remove_file(source).unwrap();
    drop(project);
    let reopened = ShapeProject::open(&root).unwrap();
    assert_eq!(
        reopened
            .read_accepted(revision.artifact_id)
            .unwrap()
            .unwrap()
            .bytes,
        bytes
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn malformed_audio_and_binary_text_leave_the_project_empty() {
    let root = root();
    let source = root.with_extension("wav");
    fs::write(&source, b"not a wave").unwrap();
    let mut project = ShapeProject::create(&root, "Rejected files").unwrap();
    assert!(project.import_audio_wav(&source, "Bad audio").is_err());
    fs::write(&source, [0xff_u8, 0xfe]).unwrap();
    assert!(project.import_text_file(&source, "Bad text").is_err());
    assert!(project.snapshot().unwrap().artifacts.is_empty());
    fs::remove_file(source).unwrap();
    fs::remove_dir_all(root).unwrap();
}
