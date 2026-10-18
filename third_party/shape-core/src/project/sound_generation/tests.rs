use super::*;
use shape_domain::{AudioOriginDisclosure, ContentDigest, SoundGenerationKind};
use shape_execution::{
    ExecutionFailure, ExecutionOutput, ExecutorIdentity, ExternalAttemptProvenance,
    ExternalExecutionProvenance, ExternalRoutingCandidate, INFER_RUNTIME_CONTRACT_VERSION,
};
use std::fs;
fn parameters() -> SoundGenerationOperation {
    SoundGenerationOperation::new(
        "Rain, no speech or music",
        SoundGenerationKind::SoundEffect,
        1,
        987,
    )
    .unwrap()
}
fn wav(sample_rate_hz: u32, channels: u16, frames: u32) -> Vec<u8> {
    let block_align = channels * 2;
    let data_size = frames * u32::from(block_align);
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&sample_rate_hz.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate_hz * u32::from(block_align)).to_le_bytes());
    bytes.extend_from_slice(&block_align.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_size.to_le_bytes());
    bytes.resize(bytes.len() + data_size as usize, 0);
    bytes
}
fn provenance() -> ExternalExecutionProvenance {
    ExternalExecutionProvenance {
        sound_prompt: Some(shape_execution::sound_prompt::SoundPromptProvenance {
            authored_request_digest: ContentDigest::from_bytes(
                &serde_json::to_vec(&parameters()).unwrap(),
            ),
            original_prompt: parameters().prompt.clone(),
            effective_prompt: parameters().prompt,
            rules_revision: "infer.sound-prompt-preparation@20260926.2".into(),
            text_job: None,
            preparation_elapsed_ms: 0,
        }),
        speech_segments: Vec::new(),
        speech_script: None,
        contract_revision: INFER_RUNTIME_CONTRACT_VERSION.to_owned(),
        capability_contract: Some(
            shape_execution::INFER_RUNTIME_SOUND_GENERATION_CAPABILITY.to_owned(),
        ),
        app_id: "shape".to_owned(),
        intent: "audio.generate_sound".to_owned(),
        provider: "stable-audio-3-sfx-local".to_owned(),
        deployment: "stable_audio_3_sm_sfx_mlx".to_owned(),
        model_profile: "stable_audio_3_sm_sfx".to_owned(),
        model_build: "stable_audio_3_sm_sfx_mlx".to_owned(),
        physical_model:
            "stabilityai/stable-audio-3-optimized@da6edc54ddba10bfd79a077102ded687f80e882b:sm-sfx"
                .to_owned(),
        placement: "local".to_owned(),
        capability_level: "foundational".to_owned(),
        evaluation_status: "provisional".to_owned(),
        resource_class: "standard".to_owned(),
        policy: "local-first".to_owned(),
        priority: "interactive".to_owned(),
        requested_policy: "local-first".to_owned(),
        requested_priority: "interactive".to_owned(),
        requested_provider_access_class: None,
        requested_placement: "local_only".to_owned(),
        requested_preference: "local".to_owned(),
        offline_required: true,
        requested_latency: Some("balanced".to_owned()),
        fallback: "none".to_owned(),
        requested_deadline_ms: None,
        max_cost_microusd: 0,
        capability_floor: "foundational".to_owned(),
        named_route: None,
        routing_candidates: vec![ExternalRoutingCandidate {
            provider: "stable-audio-3-sfx-local".to_owned(),
            deployment: "stable_audio_3_sm_sfx_mlx".to_owned(),
            status: "eligible".to_owned(),
            rank: Some(1),
            reason_codes: Vec::new(),
        }],
        attempts: vec![ExternalAttemptProvenance {
            number: 1,
            provider: "stable-audio-3-sfx-local".to_owned(),
            deployment: "stable_audio_3_sm_sfx_mlx".to_owned(),
            outcome: "succeeded".to_owned(),
            trigger: "initial".to_owned(),
            error_kind: None,
        }],
    }
}

struct SoundExecutor {
    identity: ExecutorIdentity,
}
impl SoundExecutor {
    fn new() -> Self {
        Self {
            identity: ExecutorIdentity::new(
                "shape.test.sound",
                "1",
                INFER_RUNTIME_CONTRACT_VERSION,
            )
            .unwrap(),
        }
    }
}
impl Executor for SoundExecutor {
    fn identity(&self) -> &ExecutorIdentity {
        &self.identity
    }
    fn supports(&self, c: &CapabilityId) -> bool {
        c.as_str() == AUDIO_GENERATE_CAPABILITY
    }
    fn execute(&self, request: &ExecutionRequest) -> Result<ExecutionOutput, ExecutionFailure> {
        assert!(request.inputs.is_empty());
        assert_eq!(
            serde_json::from_slice::<SoundGenerationOperation>(&request.instruction).unwrap(),
            parameters()
        );
        Ok(ExecutionOutput {
            bytes: wav(44_100, 2, 44_100),
            media_type: "audio/wav".into(),
            executor_job_id: Some("job_sound_1".into()),
            external_provenance: Some(provenance()),
            content_contract: Some(ArtifactContentContract::AudioClip(
                AudioValueContract::pcm_s16le_wav(
                    44_100,
                    2,
                    44_100,
                    AudioOriginDisclosure::SyntheticSound,
                )
                .unwrap(),
            )),
        })
    }
}
#[test]
fn sound_candidate_is_transient_and_acceptance_preserves_exact_history_after_reopen() {
    let root = std::env::temp_dir().join(format!("shape-sound-{}", uuid::Uuid::now_v7()));
    let mut project = ShapeProject::create(&root, "Sound test").unwrap();
    let artifact = project
        .create_artifact("Rain", ArtifactKind::AudioClip)
        .unwrap();
    let candidate = project
        .propose_generated_sound(artifact.id, &parameters(), &SoundExecutor::new())
        .unwrap();
    assert!(project.read_accepted(artifact.id).unwrap().is_none());
    assert!(!format!("{candidate:?}").contains("Rain, no speech"));
    let duplicate = candidate.clone();
    let expected = candidate.bytes().to_vec();
    let attempt = candidate.receipt().attempt_id;
    project.accept_generated_sound(candidate).unwrap();
    assert!(project.accept_generated_sound(duplicate).is_err());
    drop(project);
    let reopened = ShapeProject::open(&root).unwrap();
    let accepted = reopened.read_accepted(artifact.id).unwrap().unwrap();
    assert_eq!(accepted.bytes, expected);
    assert_eq!(
        accepted.revision.content.digest,
        ContentDigest::from_bytes(&expected)
    );
    assert_eq!(
        reopened
            .transformation(accepted.revision.transformation_id)
            .unwrap()
            .operation,
        Some(TransformationOperation::AudioGenerate(parameters()))
    );
    assert_eq!(
        reopened
            .execution_receipt(attempt)
            .unwrap()
            .external_provenance,
        Some(provenance())
    );
    assert_eq!(
        reopened
            .transformation_receipt(accepted.revision.transformation_id)
            .unwrap()
            .unwrap()
            .attempt_id,
        attempt
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn accept_rechecks_prompt_seed_and_wav_against_the_generation_receipt() {
    let root = std::env::temp_dir().join(format!("shape-sound-reject-{}", uuid::Uuid::now_v7()));
    let mut project = ShapeProject::create(&root, "Reject sound").unwrap();
    let artifact = project
        .create_artifact("Rain", ArtifactKind::AudioClip)
        .unwrap();
    let original = project
        .propose_generated_sound(artifact.id, &parameters(), &SoundExecutor::new())
        .unwrap();
    let mut changed = original.clone();
    let Some(TransformationOperation::AudioGenerate(op)) = &mut changed.transformation.operation
    else {
        panic!()
    };
    op.seed += 1;
    assert!(project.accept_generated_sound(changed).is_err());
    let mut changed = original.clone();
    changed
        .receipt
        .external_provenance
        .as_mut()
        .unwrap()
        .sound_prompt
        .as_mut()
        .unwrap()
        .effective_prompt = "Wind".into();
    assert!(project.accept_generated_sound(changed).is_err());
    let mut changed = original.clone();
    let legacy = changed.receipt.external_provenance.as_mut().unwrap();
    legacy.sound_prompt.as_mut().unwrap().rules_revision =
        "infer.sound-prompt-preparation@20260926.1".into();
    assert!(
        legacy.is_bounded(),
        "accepted historical receipts remain readable"
    );
    assert!(project.accept_generated_sound(changed).is_err());
    let mut changed = original;
    changed.output_bytes = wav(24_000, 1, 24_000).into();
    assert!(project.accept_generated_sound(changed).is_err());
    assert!(project.read_accepted(artifact.id).unwrap().is_none());
    fs::remove_dir_all(root).unwrap();
}
