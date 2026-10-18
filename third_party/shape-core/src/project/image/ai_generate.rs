//! Source-less AI Image proposal, transient Candidate, and atomic acceptance.

use std::{fmt, sync::Arc};

use shape_domain::{
    AiImageGenerateParameters, ArtifactContentContract, ArtifactId, ArtifactKind, ArtifactRevision,
    ImageRasterContract, IntentSpec, Transformation, TransformationKind, TransformationOperation,
};
use shape_execution::{
    CapabilityId, ExecutedCandidate, ExecutionCoordinator, ExecutionReceipt, ExecutionRequest,
    Executor, ExternalExecutionProvenance, IMAGE_GENERATE_CAPABILITY,
    INFER_RUNTIME_CONTRACT_VERSION,
};
use shape_store::AcceptedCommit;

use super::ShapeProject;
use crate::CoreError;

const IMAGE_MEDIA_TYPE: &str = "image/png";
const IMAGE_GENERATION_INTENT: &str = "Generate a raster image from authored intent";

/// One transient generated raster awaiting explicit user acceptance.
#[derive(Clone)]
pub struct AiImageCandidate {
    artifact_id: ArtifactId,
    artifact_name: String,
    transformation: Transformation,
    receipt: ExecutionReceipt,
    output_bytes: Arc<[u8]>,
    output_media_type: String,
    output_contract: ImageRasterContract,
}

impl fmt::Debug for AiImageCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AiImageCandidate")
            .field("artifact_id", &self.artifact_id)
            .field("artifact_name", &self.artifact_name)
            .field("transformation_id", &self.transformation.id)
            .field("receipt", &self.receipt)
            .field("output_byte_length", &self.output_bytes.len())
            .field("output_media_type", &self.output_media_type)
            .field("output_contract", &self.output_contract)
            .finish()
    }
}

impl AiImageCandidate {
    /// Returns the stable Artifact identity reserved for acceptance.
    #[must_use]
    pub const fn artifact_id(&self) -> ArtifactId {
        self.artifact_id
    }

    /// Returns the user-facing Artifact name reserved for acceptance.
    #[must_use]
    pub fn artifact_name(&self) -> &str {
        &self.artifact_name
    }

    /// Returns exact transient canonical PNG bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.output_bytes
    }

    /// Returns the revalidated portable raster interpretation.
    #[must_use]
    pub const fn contract(&self) -> &ImageRasterContract {
        &self.output_contract
    }

    /// Returns payload-free physical provenance without implying acceptance.
    #[must_use]
    pub const fn receipt(&self) -> &ExecutionReceipt {
        &self.receipt
    }

    /// Returns the exact authored source-less generation operation.
    #[must_use]
    pub fn parameters(&self) -> &AiImageGenerateParameters {
        let Some(TransformationOperation::AiImageGenerate(parameters)) =
            self.transformation.operation.as_ref()
        else {
            unreachable!("AI Image Candidates always own typed generation parameters")
        };
        parameters
    }
}

impl ShapeProject {
    /// Executes one source-less `image.generate` operation without writing the project.
    ///
    /// # Errors
    ///
    /// Rejects unsupported physical request shapes, malformed PNG
    /// output, missing stable Core/Capability Job provenance, or executor failure.
    pub fn propose_generated_image(
        &self,
        artifact_id: ArtifactId,
        parameters: &AiImageGenerateParameters,
        executor: &dyn Executor,
    ) -> Result<AiImageCandidate, CoreError> {
        let artifact = self
            .snapshot()?
            .artifacts
            .into_iter()
            .find(|artifact| artifact.id == artifact_id)
            .filter(|artifact| {
                artifact.kind == ArtifactKind::ImageRaster && artifact.accepted_revision.is_none()
            })
            .ok_or(CoreError::InvalidImageGenerationTarget { artifact_id })?;
        let transformation = Transformation::new_with_operation(
            TransformationKind::GenerativeEdit,
            artifact_id,
            Vec::new(),
            IntentSpec::new(IMAGE_GENERATION_INTENT)?,
            parameters.constraints().to_vec(),
            Vec::new(),
            Some(TransformationOperation::AiImageGenerate(parameters.clone())),
        )?;
        let request = ExecutionRequest::new(
            transformation.id,
            CapabilityId::new(IMAGE_GENERATE_CAPABILITY)?,
            Vec::new(),
            serde_json::to_vec(parameters)?,
            IMAGE_MEDIA_TYPE,
        )?;
        let ExecutedCandidate { output, receipt } =
            ExecutionCoordinator::execute(executor, &request)?;
        let Some(ArtifactContentContract::ImageRaster(output_contract)) = output.content_contract
        else {
            return Err(CoreError::ImageGenerationOutputContractMismatch);
        };
        if output.media_type != IMAGE_MEDIA_TYPE
            || output.bytes.is_empty()
            || receipt.executor_job_id.is_none()
            || receipt
                .external_provenance
                .as_ref()
                .is_none_or(|provenance| !valid_provenance(provenance))
        {
            return Err(CoreError::ImageGenerationOutputContractMismatch);
        }
        Ok(AiImageCandidate {
            artifact_id,
            artifact_name: artifact.name,
            transformation,
            receipt,
            output_bytes: output.bytes.into(),
            output_media_type: output.media_type,
            output_contract,
        })
    }

    /// Atomically advances the generation target to its first accepted Revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the target gained a head or durable publication
    /// rejects the successful Candidate.
    pub fn accept_generated_image(
        &mut self,
        candidate: AiImageCandidate,
    ) -> Result<ArtifactRevision, CoreError> {
        Ok(self.store.accept(AcceptedCommit {
            artifact_id: candidate.artifact_id,
            expected_head: None,
            transformation: candidate.transformation,
            receipt: candidate.receipt,
            output_bytes: candidate.output_bytes,
            output_media_type: candidate.output_media_type,
            content_contract: Some(ArtifactContentContract::ImageRaster(
                candidate.output_contract,
            )),
        })?)
    }
}

fn valid_provenance(provenance: &ExternalExecutionProvenance) -> bool {
    provenance.is_bounded()
        && provenance.contract_revision == INFER_RUNTIME_CONTRACT_VERSION
        && provenance.capability_contract.as_deref()
            == Some(shape_execution::INFER_RUNTIME_RESPONSES_CAPABILITY)
        && provenance.app_id == "shape"
        && provenance.intent == IMAGE_GENERATE_CAPABILITY
        && provenance.placement == "cloud"
        && provenance.policy == "balanced"
        && provenance.priority == "interactive"
        && provenance.requested_policy == "balanced"
        && provenance.requested_priority == "interactive"
        && provenance.requested_provider_access_class.as_deref() == Some("subscription")
        && provenance.requested_placement == "cloud_only"
        && provenance.requested_preference == "cloud"
        && !provenance.offline_required
        && provenance.requested_latency.is_none()
        && provenance.fallback == "none"
        && provenance.requested_deadline_ms.is_none()
        && provenance.max_cost_microusd == 0
        && provenance.capability_floor == "capable"
        && provenance
            .attempts
            .iter()
            .all(|attempt| attempt.trigger != "fallback")
}

#[cfg(test)]
mod tests;
