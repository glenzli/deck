//! Source-less sound proposal, transient Candidate, and atomic acceptance.

use std::{fmt, sync::Arc};

use shape_domain::{
    ArtifactContentContract, ArtifactId, ArtifactKind, ArtifactRevision, AudioValueContract,
    IntentSpec, SoundGenerationOperation, Transformation, TransformationKind,
    TransformationOperation,
};
use shape_execution::{
    AUDIO_GENERATE_CAPABILITY, CapabilityId, ExecutedCandidate, ExecutionCoordinator,
    ExecutionReceipt, ExecutionRequest, Executor, parse_pcm_s16le_wav, valid_sound_output,
};
use shape_store::AcceptedCommit;

use super::ShapeProject;
use crate::CoreError;

const AUDIO_MEDIA_TYPE: &str = "audio/wav";
const SOUND_GENERATION_INTENT: &str = "Generate sound from authored intent";

/// One transient generated audio awaiting explicit user acceptance.
#[derive(Clone)]
pub struct SoundCandidate {
    artifact_id: ArtifactId,
    artifact_name: String,
    transformation: Transformation,
    receipt: ExecutionReceipt,
    output_bytes: Arc<[u8]>,
    output_media_type: String,
    output_contract: AudioValueContract,
}

impl fmt::Debug for SoundCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SoundCandidate")
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

impl SoundCandidate {
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

    /// Returns exact transient PCM16 WAV bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.output_bytes
    }

    /// Returns the revalidated portable audio interpretation.
    #[must_use]
    pub const fn contract(&self) -> &AudioValueContract {
        &self.output_contract
    }

    /// Returns payload-free physical provenance without implying acceptance.
    #[must_use]
    pub const fn receipt(&self) -> &ExecutionReceipt {
        &self.receipt
    }

    /// Returns the exact authored source-less generation operation.
    #[must_use]
    pub fn parameters(&self) -> &SoundGenerationOperation {
        let Some(TransformationOperation::AudioGenerate(parameters)) =
            self.transformation.operation.as_ref()
        else {
            unreachable!("Sound Candidates always own typed generation parameters")
        };
        parameters
    }
}

impl ShapeProject {
    /// Executes one source-less `audio.generate` operation without writing the project.
    ///
    /// # Errors
    ///
    /// Rejects unsupported physical request shapes, malformed WAV
    /// output, missing stable Core/Capability Job provenance, or executor failure.
    pub fn propose_generated_sound(
        &self,
        artifact_id: ArtifactId,
        parameters: &SoundGenerationOperation,
        executor: &dyn Executor,
    ) -> Result<SoundCandidate, CoreError> {
        parameters.validate()?;
        let artifact = self
            .snapshot()?
            .artifacts
            .into_iter()
            .find(|artifact| artifact.id == artifact_id)
            .filter(|artifact| {
                artifact.kind == ArtifactKind::AudioClip && artifact.accepted_revision.is_none()
            })
            .ok_or(CoreError::InvalidSoundGenerationTarget { artifact_id })?;
        let transformation = Transformation::new_with_operation(
            TransformationKind::GenerativeEdit,
            artifact_id,
            Vec::new(),
            IntentSpec::new(SOUND_GENERATION_INTENT)?,
            Vec::new(),
            Vec::new(),
            Some(TransformationOperation::AudioGenerate(parameters.clone())),
        )?;
        let request = ExecutionRequest::new(
            transformation.id,
            CapabilityId::new(AUDIO_GENERATE_CAPABILITY)?,
            Vec::new(),
            serde_json::to_vec(parameters)?,
            AUDIO_MEDIA_TYPE,
        )?;
        let ExecutedCandidate { output, receipt } =
            ExecutionCoordinator::execute(executor, &request)?;
        let Some(ArtifactContentContract::AudioClip(output_contract)) = output.content_contract
        else {
            return Err(CoreError::AudioOutputContractMismatch);
        };
        if output.media_type != AUDIO_MEDIA_TYPE
            || parse_pcm_s16le_wav(
                &output.bytes,
                shape_domain::AudioOriginDisclosure::SyntheticSound,
            )
            .ok()
            .as_ref()
                != Some(&output_contract)
            || receipt.executor_job_id.is_none()
            || receipt
                .external_provenance
                .as_ref()
                .is_none_or(|provenance| {
                    !valid_sound_output(parameters, &output_contract, provenance)
                })
        {
            return Err(CoreError::AudioOutputContractMismatch);
        }
        Ok(SoundCandidate {
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
    pub fn accept_generated_sound(
        &mut self,
        candidate: SoundCandidate,
    ) -> Result<ArtifactRevision, CoreError> {
        Ok(self.store.accept(AcceptedCommit {
            artifact_id: candidate.artifact_id,
            expected_head: None,
            transformation: candidate.transformation,
            receipt: candidate.receipt,
            output_bytes: candidate.output_bytes,
            output_media_type: candidate.output_media_type,
            content_contract: Some(ArtifactContentContract::AudioClip(
                candidate.output_contract,
            )),
        })?)
    }
}

#[cfg(test)]
mod tests;
