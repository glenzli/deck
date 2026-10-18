use std::{fs, path::Path};

use shape_domain::{
    ArtifactContentContract, ArtifactKind, AudioOriginDisclosure, AudioValueContract, Constraint,
    ContentDigest, IntentSpec, PresetVoiceAlias, PresetVoiceSelection, RevisionId,
    SpeechSynthesisOperation, SpeechVoiceSelection, TransformationOperation,
};
use shape_execution::{
    AUDIO_SPEECH_SYNTHESIZE_CAPABILITY, CapabilityId, ExecutionFailure, ExecutionOutput, Executor,
    ExecutorIdentity, ExternalAttemptProvenance, ExternalExecutionProvenance,
    ExternalRoutingCandidate, INFER_RUNTIME_CONTRACT_VERSION,
    INFER_SPEECH_VOICE_ALIAS_CATALOG_REVISION, INFER_SPEECH_VOICE_ZH_BRIGHT_FEMALE_LANGUAGE,
    INFER_SPEECH_VOICE_ZH_BRIGHT_FEMALE_V1,
};
use uuid::Uuid;

use super::*;

#[derive(Debug)]
struct SpeechExecutor {
    identity: ExecutorIdentity,
    bytes: Vec<u8>,
}

impl SpeechExecutor {
    fn new() -> Self {
        Self {
            identity: ExecutorIdentity::new(
                "shape.test.speech",
                "1",
                INFER_RUNTIME_CONTRACT_VERSION,
            )
            .unwrap(),
            bytes: wav(24_000, 1, 24_000),
        }
    }
}

impl Executor for SpeechExecutor {
    fn identity(&self) -> &ExecutorIdentity {
        &self.identity
    }

    fn supports(&self, capability: &CapabilityId) -> bool {
        capability.as_str() == AUDIO_SPEECH_SYNTHESIZE_CAPABILITY
    }

    fn execute(&self, _request: &ExecutionRequest) -> Result<ExecutionOutput, ExecutionFailure> {
        Ok(ExecutionOutput {
            bytes: self.bytes.clone(),
            media_type: AUDIO_MEDIA_TYPE.to_owned(),
            executor_job_id: Some("job_shape_audio_core_1".to_owned()),
            external_provenance: Some(provenance()),
            content_contract: Some(ArtifactContentContract::AudioClip(
                AudioValueContract::pcm_s16le_wav(
                    24_000,
                    1,
                    24_000,
                    AudioOriginDisclosure::SyntheticSpeech,
                )
                .unwrap(),
            )),
        })
    }
}

fn test_root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("shape-core-audio-{label}-{}", Uuid::now_v7()))
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

fn operation() -> SpeechSynthesisOperation {
    SpeechSynthesisOperation::new(
        INFER_SPEECH_VOICE_ZH_BRIGHT_FEMALE_LANGUAGE,
        SpeechVoiceSelection::Preset(
            PresetVoiceSelection::new(
                PresetVoiceAlias::new(INFER_SPEECH_VOICE_ZH_BRIGHT_FEMALE_V1).unwrap(),
                INFER_SPEECH_VOICE_ALIAS_CATALOG_REVISION,
            )
            .unwrap(),
        ),
        1_000,
        true,
    )
    .unwrap()
}

fn provenance() -> ExternalExecutionProvenance {
    ExternalExecutionProvenance {
        sound_prompt: None,
        speech_segments: Vec::new(),
        speech_script: None,
        contract_revision: INFER_RUNTIME_CONTRACT_VERSION.to_owned(),
        capability_contract: Some(shape_execution::INFER_RUNTIME_SPEECH_CAPABILITY.to_owned()),
        app_id: "shape".to_owned(),
        intent: "speech.synthesize".to_owned(),
        provider: "mlx-audio-local".to_owned(),
        deployment: "mlx_qwen3_tts_custom_voice_1_7b".to_owned(),
        model_profile: "qwen3_tts_custom_voice_1_7b".to_owned(),
        model_build: "qwen3_tts_custom_voice_1_7b_8bit".to_owned(),
        physical_model: "qwen3-tts-custom-voice".to_owned(),
        placement: "local".to_owned(),
        capability_level: "capable".to_owned(),
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
        requested_latency: Some("interactive".to_owned()),
        fallback: "none".to_owned(),
        requested_deadline_ms: None,
        max_cost_microusd: 0,
        capability_floor: "capable".to_owned(),
        named_route: None,
        routing_candidates: vec![ExternalRoutingCandidate {
            provider: "mlx-audio-local".to_owned(),
            deployment: "mlx_qwen3_tts_custom_voice_1_7b".to_owned(),
            status: "eligible".to_owned(),
            rank: Some(1),
            reason_codes: Vec::new(),
        }],
        attempts: vec![ExternalAttemptProvenance {
            number: 1,
            provider: "mlx-audio-local".to_owned(),
            deployment: "mlx_qwen3_tts_custom_voice_1_7b".to_owned(),
            outcome: "succeeded".to_owned(),
            trigger: "initial".to_owned(),
            error_kind: None,
        }],
    }
}

fn accepted_text(project: &mut ShapeProject) -> (ArtifactId, ArtifactRevision) {
    let artifact = project
        .create_artifact("Narration", ArtifactKind::TextDocument)
        .unwrap();
    let candidate = project
        .propose_text(
            artifact.id,
            None,
            "这是一条普通的合成旁白。",
            IntentSpec::new("Write narration").unwrap(),
            Vec::<Constraint>::new(),
        )
        .unwrap();
    let revision = project.accept_text(candidate).unwrap();
    (artifact.id, revision)
}

#[test]
fn speech_node_reuses_its_output_and_rejects_a_candidate_after_voice_changes() {
    use shape_domain::{
        ArtifactWorkingGraph, OperatorConfigurationSchemaId, OperatorDataTypeId, OperatorTypeId,
        WorkingInput, WorkingOperatorConfiguration,
    };
    let root = test_root("stable-node");
    let mut project = ShapeProject::create(&root, "Speech node").unwrap();
    let (source, original) = accepted_text(&mut project);
    let target = Artifact::new("Recording", ArtifactKind::AudioClip).unwrap();
    let mut graph = ArtifactWorkingGraph::new_source(target.id);
    let node = graph
        .add_bound_operator(
            OperatorTypeId::new("audio.speech_synthesize").unwrap(),
            OperatorDataTypeId::new("text.document").unwrap(),
            OperatorDataTypeId::new("audio.clip").unwrap(),
            WorkingInput {
                artifact_id: source,
                revision_id: original.id,
            },
        )
        .unwrap();
    project
        .create_source_artifact_draft(&target, &graph)
        .unwrap();
    let first = project
        .propose_speech_node(target.id, &node, &operation(), &SpeechExecutor::new())
        .unwrap();
    let first_revision = project.accept_speech_synthesis(first).unwrap();
    let second = project
        .propose_speech_node(target.id, &node, &operation(), &SpeechExecutor::new())
        .unwrap();
    assert_eq!(second.review_artifact_id(), target.id);
    let second_revision = project.accept_speech_synthesis(second).unwrap();
    assert_eq!(second_revision.parents, [first_revision.id]);
    assert_eq!(project.snapshot().unwrap().artifacts.len(), 2);
    let obsolete = project
        .propose_speech_node(target.id, &node, &operation(), &SpeechExecutor::new())
        .unwrap();
    let mut graph = project.artifact_working_graphs().unwrap().remove(0);
    graph.set_operator_configuration(
        node.id(),
        Some(
            WorkingOperatorConfiguration::new(
                OperatorConfigurationSchemaId::new("speech.test").unwrap(),
                "{\"pace\":1100}",
            )
            .unwrap(),
        ),
    );
    project.save_artifact_working_graph(&graph).unwrap();
    assert!(project.accept_speech_synthesis(obsolete).is_err());
    assert_eq!(
        project
            .read_accepted(target.id)
            .unwrap()
            .unwrap()
            .revision
            .id,
        second_revision.id
    );
    fs::remove_dir_all(root).unwrap();
}

fn file_count(root: &Path) -> usize {
    fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .map(|path| if path.is_dir() { file_count(&path) } else { 1 })
        .sum()
}

#[test]
fn speech_candidate_is_transient_then_reopens_with_exact_audio_and_provenance() {
    let root = test_root("accept");
    let mut project = ShapeProject::create(&root, "Audio Project").unwrap();
    let (source_id, source_revision) = accepted_text(&mut project);
    let object_count_before = file_count(&root.join("objects"));

    let candidate = project
        .propose_speech_synthesis(
            source_id,
            source_revision.id,
            "Mandarin narration",
            &operation(),
            Vec::new(),
            &SpeechExecutor::new(),
        )
        .unwrap();
    let audio_id = candidate.artifact_id();
    let attempt_id = candidate.receipt().attempt_id;
    let expected_bytes = candidate.bytes().to_vec();
    assert_eq!(candidate.contract().duration_millis(), 1_000);
    assert_eq!(project.snapshot().unwrap().artifacts.len(), 1);
    assert_eq!(file_count(&root.join("objects")), object_count_before);

    let audio_revision = project.accept_speech_synthesis(candidate).unwrap();
    assert_eq!(
        project
            .snapshot()
            .unwrap()
            .artifacts
            .into_iter()
            .find(|artifact| artifact.id == source_id)
            .unwrap()
            .accepted_revision,
        Some(source_revision.id)
    );
    drop(project);

    let reopened = ShapeProject::open(&root).unwrap();
    let content = reopened.read_accepted(audio_id).unwrap().unwrap();
    assert_eq!(content.revision.id, audio_revision.id);
    assert_eq!(content.bytes, expected_bytes);
    assert_eq!(
        content.revision.content.digest,
        ContentDigest::from_bytes(&expected_bytes)
    );
    let Some(ArtifactContentContract::AudioClip(contract)) = content.revision.content_contract
    else {
        panic!("accepted speech must remain an audio clip");
    };
    assert_eq!(contract.sample_rate_hz, 24_000);
    assert_eq!(contract.origin, AudioOriginDisclosure::SyntheticSpeech);

    let transformation = reopened
        .transformation(content.revision.transformation_id)
        .unwrap();
    let Some(TransformationOperation::AudioSpeechSynthesis(operation)) = transformation.operation
    else {
        panic!("speech operation must round-trip");
    };
    let SpeechVoiceSelection::Preset(voice) = operation.voice else {
        panic!("first accepted speech must use a preset");
    };
    assert_eq!(voice.alias.as_str(), INFER_SPEECH_VOICE_ZH_BRIGHT_FEMALE_V1);
    assert_eq!(
        voice.catalog_revision,
        INFER_SPEECH_VOICE_ALIAS_CATALOG_REVISION
    );
    assert!(operation.synthetic_disclosure_required);

    let receipt = reopened.execution_receipt(attempt_id).unwrap();
    assert_eq!(
        receipt.executor_job_id.as_deref(),
        Some("job_shape_audio_core_1")
    );
    assert_eq!(receipt.external_provenance.as_ref(), Some(&provenance()));
    let serialized = serde_json::to_string(&receipt).unwrap();
    assert!(!serialized.contains("这是一条普通的合成旁白"));
    assert!(!serialized.contains("audio/wav"));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn stale_source_rejects_audio_acceptance_without_publishing_the_new_artifact() {
    let root = test_root("stale");
    let mut project = ShapeProject::create(&root, "Audio Project").unwrap();
    let (source_id, source_revision) = accepted_text(&mut project);
    let candidate = project
        .propose_speech_synthesis(
            source_id,
            source_revision.id,
            "Stale narration",
            &operation(),
            Vec::new(),
            &SpeechExecutor::new(),
        )
        .unwrap();

    let edit = project
        .propose_text(
            source_id,
            Some(source_revision.id),
            "A newer accepted narration.",
            IntentSpec::new("Advance source").unwrap(),
            Vec::new(),
        )
        .unwrap();
    project.accept_text(edit).unwrap();
    assert!(matches!(
        project.accept_speech_synthesis(candidate),
        Err(CoreError::StaleCandidate { .. })
    ));
    assert_eq!(
        project
            .snapshot()
            .unwrap()
            .artifacts
            .iter()
            .filter(|artifact| artifact.kind == ArtifactKind::AudioClip)
            .count(),
        0
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn authorized_voice_reference_is_not_executable_in_the_preset_slice() {
    let root = test_root("voice-reference");
    let mut project = ShapeProject::create(&root, "Audio Project").unwrap();
    let (source_id, source_revision) = accepted_text(&mut project);
    let reference = shape_domain::AuthorizedVoiceReference::new(
        RevisionId::new(),
        "consent.receipt.test",
        shape_domain::VoiceAuthorizationScope::SpeechSynthesis,
        true,
        true,
    )
    .unwrap();
    let operation = SpeechSynthesisOperation::new(
        "Chinese",
        SpeechVoiceSelection::AuthorizedReference(reference),
        1_000,
        true,
    )
    .unwrap();
    assert!(matches!(
        project.propose_speech_synthesis(
            source_id,
            source_revision.id,
            "Rejected voice",
            &operation,
            Vec::new(),
            &SpeechExecutor::new(),
        ),
        Err(CoreError::UnsupportedSpeechVoiceReference)
    ));
    fs::remove_dir_all(&root).unwrap();
}
