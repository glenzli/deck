use std::{fs, io::Cursor, path::Path};

use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use shape_domain::{
    AiImageOutputCanvas, ArtifactContentContract, ContentDigest, ImageColorProfile,
    ImageRasterContract, TransformationOperation,
};
use shape_execution::{
    CapabilityId, ExecutionFailure, ExecutionOutput, ExecutionRequest, Executor, ExecutorIdentity,
    ExternalAttemptProvenance, ExternalExecutionProvenance, ExternalRoutingCandidate,
};
use uuid::Uuid;

use super::*;

#[derive(Debug)]
struct ImageExecutor {
    identity: ExecutorIdentity,
    bytes: Vec<u8>,
    contract: ImageRasterContract,
    provenance: ExternalExecutionProvenance,
}

impl ImageExecutor {
    fn new(width: u32, height: u32) -> Self {
        Self {
            identity: ExecutorIdentity::new(
                "shape.test.image-generation",
                "1",
                INFER_RUNTIME_CONTRACT_VERSION,
            )
            .unwrap(),
            bytes: png(width, height),
            contract: ImageRasterContract::rgba8(width, height, ImageColorProfile::Srgb).unwrap(),
            provenance: provenance(),
        }
    }
}

impl Executor for ImageExecutor {
    fn identity(&self) -> &ExecutorIdentity {
        &self.identity
    }

    fn supports(&self, capability: &CapabilityId) -> bool {
        capability.as_str() == IMAGE_GENERATE_CAPABILITY
    }

    fn execute(&self, request: &ExecutionRequest) -> Result<ExecutionOutput, ExecutionFailure> {
        assert!(request.inputs.is_empty());
        assert_eq!(request.capability.as_str(), IMAGE_GENERATE_CAPABILITY);
        let parameters: AiImageGenerateParameters =
            serde_json::from_slice(&request.instruction).unwrap();
        assert_eq!(parameters.instruction(), "A pale blue circle on white");
        Ok(ExecutionOutput {
            bytes: self.bytes.clone(),
            media_type: IMAGE_MEDIA_TYPE.to_owned(),
            executor_job_id: Some("resp_shape_image_core_1".to_owned()),
            external_provenance: Some(self.provenance.clone()),
            content_contract: Some(ArtifactContentContract::ImageRaster(self.contract.clone())),
        })
    }
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let pixels = vec![127_u8; width as usize * height as usize * 4];
    let mut bytes = Cursor::new(Vec::new());
    PngEncoder::new(&mut bytes)
        .write_image(&pixels, width, height, ColorType::Rgba8.into())
        .unwrap();
    bytes.into_inner()
}

fn parameters() -> AiImageGenerateParameters {
    AiImageGenerateParameters::new(
        "A pale blue circle on white",
        AiImageOutputCanvas::new(4, 3).unwrap(),
        1,
        Vec::new(),
    )
    .unwrap()
}

fn provenance() -> ExternalExecutionProvenance {
    ExternalExecutionProvenance {
        sound_prompt: None,
        speech_segments: Vec::new(),
        speech_script: None,
        contract_revision: INFER_RUNTIME_CONTRACT_VERSION.to_owned(),
        capability_contract: Some(shape_execution::INFER_RUNTIME_RESPONSES_CAPABILITY.to_owned()),
        app_id: "shape".to_owned(),
        intent: IMAGE_GENERATE_CAPABILITY.to_owned(),
        provider: "codex-subscription".to_owned(),
        deployment: "codex_gpt_5_6_luna".to_owned(),
        model_profile: "codex_gpt_5_6_luna".to_owned(),
        model_build: "codex_gpt_5_6_luna_subscription".to_owned(),
        physical_model: "gpt-5.6-luna".to_owned(),
        placement: "cloud".to_owned(),
        capability_level: "advanced".to_owned(),
        evaluation_status: "provisional".to_owned(),
        resource_class: "standard".to_owned(),
        policy: "balanced".to_owned(),
        priority: "interactive".to_owned(),
        requested_policy: "balanced".to_owned(),
        requested_priority: "interactive".to_owned(),
        requested_provider_access_class: Some("subscription".to_owned()),
        requested_placement: "cloud_only".to_owned(),
        requested_preference: "cloud".to_owned(),
        offline_required: false,
        requested_latency: None,
        fallback: "none".to_owned(),
        requested_deadline_ms: None,
        max_cost_microusd: 0,
        capability_floor: "capable".to_owned(),
        named_route: None,
        routing_candidates: vec![ExternalRoutingCandidate {
            provider: "codex-subscription".to_owned(),
            deployment: "codex_gpt_5_6_luna".to_owned(),
            status: "eligible".to_owned(),
            rank: Some(1),
            reason_codes: Vec::new(),
        }],
        attempts: vec![ExternalAttemptProvenance {
            number: 1,
            provider: "codex-subscription".to_owned(),
            deployment: "codex_gpt_5_6_luna".to_owned(),
            outcome: "succeeded".to_owned(),
            trigger: "initial".to_owned(),
            error_kind: None,
        }],
    }
}

fn test_root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("shape-core-ai-image-{label}-{}", Uuid::now_v7()))
}

fn file_count(root: &Path) -> usize {
    fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .map(|path| if path.is_dir() { file_count(&path) } else { 1 })
        .sum()
}

#[test]
fn generated_image_stays_transient_then_accepts_exact_typed_history() {
    let root = test_root("accept");
    let mut project = ShapeProject::create(&root, "AI Image Project").unwrap();
    let artifact = project
        .create_artifact("Blue icon", ArtifactKind::ImageRaster)
        .unwrap();
    let objects_before = file_count(&root.join("objects"));
    let candidate = project
        .propose_generated_image(artifact.id, &parameters(), &ImageExecutor::new(4, 3))
        .unwrap();
    let artifact_id = candidate.artifact_id();
    let attempt_id = candidate.receipt().attempt_id;
    let expected_bytes = candidate.bytes().to_vec();
    assert_eq!(candidate.parameters(), &parameters());
    assert_eq!(project.snapshot().unwrap().artifacts.len(), 1);
    assert!(
        project.snapshot().unwrap().artifacts[0]
            .accepted_revision
            .is_none()
    );
    assert_eq!(file_count(&root.join("objects")), objects_before);
    assert!(!format!("{candidate:?}").contains("pale blue circle"));

    let accepted = project.accept_generated_image(candidate).unwrap();
    assert!(accepted.parents.is_empty());
    drop(project);

    let reopened = ShapeProject::open(&root).unwrap();
    let content = reopened.read_accepted(artifact_id).unwrap().unwrap();
    assert_eq!(content.bytes, expected_bytes);
    assert_eq!(
        content.revision.content.digest,
        ContentDigest::from_bytes(&expected_bytes)
    );
    let transformation = reopened
        .transformation(content.revision.transformation_id)
        .unwrap();
    assert_eq!(
        transformation.operation,
        Some(TransformationOperation::AiImageGenerate(parameters()))
    );
    let receipt = reopened.execution_receipt(attempt_id).unwrap();
    assert_eq!(receipt.external_provenance.as_ref(), Some(&provenance()));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn duplicate_accept_and_untrusted_provenance_fail_without_a_second_artifact() {
    let root = test_root("reject");
    let mut project = ShapeProject::create(&root, "AI Image Project").unwrap();
    let artifact = project
        .create_artifact("Blue icon", ArtifactKind::ImageRaster)
        .unwrap();
    let candidate = project
        .propose_generated_image(artifact.id, &parameters(), &ImageExecutor::new(4, 3))
        .unwrap();
    let duplicate = candidate.clone();
    project.accept_generated_image(candidate).unwrap();
    assert!(project.accept_generated_image(duplicate).is_err());
    assert_eq!(project.snapshot().unwrap().artifacts.len(), 1);

    let mut invalid = ImageExecutor::new(4, 3);
    invalid.provenance.requested_provider_access_class = None;
    let rejected = project
        .create_artifact("Rejected", ArtifactKind::ImageRaster)
        .unwrap();
    assert!(matches!(
        project.propose_generated_image(rejected.id, &parameters(), &invalid),
        Err(CoreError::ImageGenerationOutputContractMismatch)
    ));
    assert_eq!(project.snapshot().unwrap().artifacts.len(), 2);
    assert!(
        project.snapshot().unwrap().artifacts[1]
            .accepted_revision
            .is_none()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_image_size_and_requested_canvas_both_survive_acceptance() {
    let root = test_root("native-size");
    let mut project = ShapeProject::create(&root, "Native Image").unwrap();
    let artifact = project
        .create_artifact("Native", ArtifactKind::ImageRaster)
        .unwrap();
    let candidate = project
        .propose_generated_image(artifact.id, &parameters(), &ImageExecutor::new(6, 4))
        .unwrap();
    assert_eq!(
        (candidate.contract().width, candidate.contract().height),
        (6, 4)
    );
    assert_eq!(candidate.parameters(), &parameters());
    let revision = project.accept_generated_image(candidate).unwrap();
    assert_eq!(
        revision.content_contract,
        Some(ArtifactContentContract::ImageRaster(
            ImageExecutor::new(6, 4).contract
        ))
    );
    fs::remove_dir_all(root).unwrap();
}
