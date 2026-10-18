use super::*;
use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use std::{fs, path::PathBuf};

pub(super) struct Fixture {
    pub(super) root: PathBuf,
    pub(super) project: ShapeProject,
    source: WorkingInput,
}
impl Fixture {
    pub(super) fn image() -> Self {
        let root = std::env::temp_dir().join(format!("shape-material-{}", uuid::Uuid::now_v7()));
        let mut project = ShapeProject::create(&root, "Material fixture").unwrap();
        let source = project.import_raster_bytes(png(10), "Original").unwrap();
        Self {
            root,
            project,
            source: WorkingInput {
                artifact_id: source.artifact_id,
                revision_id: source.id,
            },
        }
    }
    pub(super) fn stage(&mut self) -> MaterialEditTicket {
        self.project
            .create_material_edit(self.source, "Edited image", MaterialEditor::Shadow)
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
pub(super) fn png(value: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(
            &[value, 20, 30, 255].repeat(8),
            4,
            2,
            ColorType::Rgba8.into(),
        )
        .unwrap();
    bytes
}
fn propose(
    project: &ShapeProject,
    ticket: &MaterialEditTicket,
    value: u8,
) -> MaterialEditCandidate {
    let bytes = png(value);
    let digest = ContentDigest::from_bytes(&bytes);
    project
        .propose_material_return(ticket, bytes, digest)
        .unwrap()
}

#[test]
fn return_is_reviewable_then_accepts_to_own_reopenable_output_and_retains_original() {
    let mut f = Fixture::image();
    let ticket = f.stage();
    let original = f
        .project
        .read_accepted(f.source.artifact_id)
        .unwrap()
        .unwrap();
    assert_eq!(f.project.material_edit_source(&ticket).unwrap(), original);
    let candidate = propose(&f.project, &ticket, 80);
    let duplicate = candidate.clone();
    assert!(
        f.project
            .read_accepted(ticket.output_artifact_id)
            .unwrap()
            .is_none()
    );
    let accepted = f.project.accept_material_return(candidate).unwrap();
    assert_eq!(accepted.artifact_id, ticket.output_artifact_id);
    assert!(f.project.accept_material_return(duplicate).is_err());
    assert_eq!(
        f.project
            .read_accepted(f.source.artifact_id)
            .unwrap()
            .unwrap(),
        original
    );
    let operation = f
        .project
        .transformation(accepted.transformation_id)
        .unwrap();
    assert_eq!(operation.kind, TransformationKind::ExternalRoundTrip);
    assert_eq!(operation.inputs, vec![f.source.revision_id]);
    let reopened = ShapeProject::open(&f.root).unwrap();
    let current = reopened
        .material_edit_ticket(ticket.output_artifact_id)
        .unwrap();
    assert_eq!(current.draft, ticket.draft);
    assert_eq!(current.expected_output_head, Some(accepted.id));
    assert_eq!(
        reopened
            .read_accepted(ticket.output_artifact_id)
            .unwrap()
            .unwrap()
            .bytes,
        png(80)
    );
    assert!(matches!(
        reopened.propose_material_return(&current, png(80), ContentDigest::from_bytes(&png(80))),
        Err(CoreError::UnchangedMaterialReturn)
    ));
}

#[test]
fn invalid_digest_format_and_noop_preserve_a_reviewed_candidate() {
    let mut f = Fixture::image();
    let ticket = f.stage();
    let reviewed = propose(&f.project, &ticket, 81);
    assert!(
        f.project
            .propose_material_return(&ticket, png(99), ContentDigest::from_bytes(&png(98)))
            .is_err()
    );
    let bad = b"not a PNG".to_vec();
    assert!(
        f.project
            .propose_material_return(&ticket, bad.clone(), ContentDigest::from_bytes(&bad))
            .is_err()
    );
    assert!(matches!(
        f.project
            .propose_material_return(&ticket, png(10), ContentDigest::from_bytes(&png(10))),
        Err(CoreError::UnchangedMaterialReturn)
    ));
    f.project.accept_material_return(reviewed).unwrap();
}

#[test]
fn cancel_invalidates_old_ticket_and_candidate_without_deleting_stage_or_original() {
    let mut f = Fixture::image();
    let ticket = f.stage();
    let candidate = propose(&f.project, &ticket, 82);
    let replacement = f.project.restart_material_edit(&ticket, false).unwrap();
    assert_eq!(replacement.output_artifact_id, ticket.output_artifact_id);
    assert_eq!(
        MaterialEditConfiguration::from_draft(&replacement.draft)
            .unwrap()
            .session,
        2
    );
    assert!(f.project.accept_material_return(candidate).is_err());
    assert!(f.project.material_edit_source(&ticket).is_err());
    assert!(f.project.restart_material_edit(&ticket, false).is_err());
    let next = propose(&f.project, &replacement, 83);
    f.project.accept_material_return(next).unwrap();
    assert_eq!(
        f.project
            .read_accepted(f.source.artifact_id)
            .unwrap()
            .unwrap()
            .revision
            .id,
        f.source.revision_id
    );
}

#[test]
fn two_sessions_cannot_replace_each_others_accepted_return() {
    let mut f = Fixture::image();
    let ticket = f.stage();
    let mut peer = ShapeProject::open(&f.root).unwrap();
    let mine = propose(&f.project, &ticket, 84);
    let theirs = propose(&peer, &ticket, 85);
    let accepted = peer.accept_material_return(theirs).unwrap();
    assert!(f.project.accept_material_return(mine).is_err());
    assert_eq!(
        f.project
            .read_accepted(ticket.output_artifact_id)
            .unwrap()
            .unwrap()
            .revision
            .id,
        accepted.id
    );
}

#[test]
fn source_advancement_requires_explicit_refresh_and_keeps_downstream_pinned() {
    let mut f = Fixture::image();
    let ticket = f.stage();
    let candidate = propose(&f.project, &ticket, 90);
    let first = f.project.accept_material_return(candidate).unwrap();
    let downstream = f
        .project
        .create_material_edit(
            WorkingInput {
                artifact_id: first.artifact_id,
                revision_id: first.id,
            },
            "Next stage",
            MaterialEditor::Shadow,
        )
        .unwrap();
    let current = f.project.material_edit_ticket(first.artifact_id).unwrap();
    let second = propose(&f.project, &current, 91);
    f.project.accept_material_return(second).unwrap();
    assert!(f.project.material_edit_source(&downstream).is_err());
    assert_eq!(
        f.project
            .material_edit_ticket(downstream.output_artifact_id)
            .unwrap()
            .draft
            .input(),
        downstream.draft.input()
    );
    let refreshed = f.project.restart_material_edit(&downstream, true).unwrap();
    assert_ne!(refreshed.draft.input(), downstream.draft.input());
    let edited = propose(&f.project, &refreshed, 92);
    f.project.accept_material_return(edited).unwrap();
}

#[test]
fn changed_source_cannot_accept_previously_reviewed_output() {
    let mut f = Fixture::image();
    let upstream = f.stage();
    let first = propose(&f.project, &upstream, 93);
    let first = f.project.accept_material_return(first).unwrap();
    let downstream = f
        .project
        .create_material_edit(
            WorkingInput {
                artifact_id: first.artifact_id,
                revision_id: first.id,
            },
            "Second",
            MaterialEditor::Shadow,
        )
        .unwrap();
    let stale = propose(&f.project, &downstream, 94);
    let current = f.project.material_edit_ticket(first.artifact_id).unwrap();
    let next = propose(&f.project, &current, 95);
    f.project.accept_material_return(next).unwrap();
    assert!(f.project.accept_material_return(stale).is_err());
    assert!(
        f.project
            .read_accepted(downstream.output_artifact_id)
            .unwrap()
            .is_none()
    );
}

#[test]
fn foreign_project_wrong_media_and_tampered_return_location_are_rejected() {
    let mut f = Fixture::image();
    let ticket = f.stage();
    let other = Fixture::image();
    assert!(other.project.material_edit_source(&ticket).is_err());
    assert!(
        f.project
            .create_material_edit(f.source, "Wrong", MaterialEditor::Echo)
            .is_err()
    );
    let mut tampered = ticket.clone();
    tampered.output_artifact_id = f.source.artifact_id;
    assert!(f.project.material_edit_source(&tampered).is_err());
    tampered = ticket.clone();
    tampered.input_content.digest = ContentDigest::from_bytes(b"wrong");
    assert!(f.project.material_edit_source(&tampered).is_err());
    let json = serde_json::to_vec(&ticket).unwrap();
    assert_eq!(
        serde_json::from_slice::<MaterialEditTicket>(&json).unwrap(),
        ticket
    );
}

fn wave(sample: u8) -> Vec<u8> {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend(40_u32.to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16_u32.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(24_000_u32.to_le_bytes());
    bytes.extend(48_000_u32.to_le_bytes());
    bytes.extend(2_u16.to_le_bytes());
    bytes.extend(16_u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(4_u32.to_le_bytes());
    bytes.extend([sample, 0, 2, 0]);
    bytes
}

#[test]
fn audio_stage_roundtrip_preserves_disclosure_and_rejects_pcm24_then_recovers() {
    let mut f = Fixture::image();
    let path = f.root.join("fixture.wav");
    fs::write(&path, wave(1)).unwrap();
    let audio = f.project.import_audio_wav(&path, "External audio").unwrap();
    let ticket = f
        .project
        .create_material_edit(
            WorkingInput {
                artifact_id: audio.artifact_id,
                revision_id: audio.id,
            },
            "Edited audio",
            MaterialEditor::Echo,
        )
        .unwrap();
    let mut wrong = wave(4);
    wrong[34] = 24;
    assert!(
        f.project
            .propose_material_return(&ticket, wrong.clone(), ContentDigest::from_bytes(&wrong))
            .is_err()
    );
    let edited = wave(4);
    let candidate = f
        .project
        .propose_material_return(&ticket, edited.clone(), ContentDigest::from_bytes(&edited))
        .unwrap();
    let accepted = f.project.accept_material_return(candidate).unwrap();
    let Some(ArtifactContentContract::AudioClip(contract)) = accepted.content_contract else {
        panic!("audio contract");
    };
    assert_eq!(
        contract.origin,
        shape_domain::AudioOriginDisclosure::ImportedUnverified
    );
    assert_eq!(
        f.project
            .read_accepted(audio.artifact_id)
            .unwrap()
            .unwrap()
            .bytes,
        wave(1)
    );
    assert_eq!(
        f.project
            .read_accepted(ticket.output_artifact_id)
            .unwrap()
            .unwrap()
            .bytes,
        edited
    );
}

#[test]
fn storage_rejects_forged_provider_receipt() {
    let mut f = Fixture::image();
    let ticket = f.stage();
    let candidate = propose(&f.project, &ticket, 96);
    let input = ticket.draft.input().unwrap();
    let mut receipt = candidate.receipt.clone();
    receipt.executor.id = "shadow".into();
    let commit = AcceptedCommit {
        artifact_id: ticket.output_artifact_id,
        expected_head: None,
        transformation: candidate.transformation.clone(),
        receipt,
        output_bytes: candidate.bytes.clone(),
        output_media_type: "image/png".into(),
        content_contract: Some(candidate.contract.clone()),
    };
    assert!(
        f.project
            .store
            .accept_with_node(
                commit,
                &[(input.artifact_id, input.revision_id)],
                Some(&ticket.draft)
            )
            .is_err()
    );
    f.project.accept_material_return(candidate).unwrap();
}

#[test]
fn concurrent_independent_returns_have_one_winner_and_retain_original() {
    use std::sync::{Arc, Barrier};
    for round in 0..4 {
        let mut f = Fixture::image();
        let ticket = f.stage();
        let barrier = Arc::new(Barrier::new(2));
        let handles: Vec<_> = [110, 120]
            .into_iter()
            .map(|value| {
                let root = f.root.clone();
                let ticket = ticket.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let mut project = ShapeProject::open(root).unwrap();
                    let candidate = propose(&project, &ticket, value + round);
                    barrier.wait();
                    project.accept_material_return(candidate)
                })
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert_eq!(
            f.project
                .read_accepted(f.source.artifact_id)
                .unwrap()
                .unwrap()
                .revision
                .id,
            f.source.revision_id
        );
        assert!(
            f.project
                .read_accepted(ticket.output_artifact_id)
                .unwrap()
                .is_some()
        );
    }
}
