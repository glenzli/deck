//! Speech synthesis Operator proposal, transient audio Candidate, and atomic acceptance.
//!
//! The first executable audio slice consumes one immutable accepted text Revision
//! and creates a new `audio.clip` Artifact. Candidate WAV bytes and Runtime
//! provenance remain memory-only until explicit acceptance. Voice references,
//! audio transforms, sound generation, playback, and Echo asset resolution are
//! deliberately outside this owner.

use std::{fmt, sync::Arc};

use shape_domain::{
    Artifact, ArtifactContentContract, ArtifactId, ArtifactKind, ArtifactRevision,
    AudioOriginDisclosure, AudioValueContract, Constraint, IntentSpec, RevisionId,
    SpeechSynthesisOperation, SpeechVoiceSelection, Transformation, TransformationKind,
    TransformationOperation,
};
use shape_execution::{
    AUDIO_SPEECH_SYNTHESIZE_CAPABILITY, CapabilityId, ExecutedCandidate, ExecutionCoordinator,
    ExecutionInput, ExecutionReceipt, ExecutionRequest, Executor, parse_pcm_s16le_wav,
};
use shape_store::NewArtifactCommit;

use super::ShapeProject;
use crate::CoreError;

const AUDIO_MEDIA_TYPE: &str = "audio/wav";
const SPEECH_SYNTHESIS_INTENT: &str = "Synthesize speech from accepted text";

/// Transient synthesized WAV output awaiting explicit acceptance.
#[derive(Clone)]
pub struct AudioCandidate {
    artifact: Artifact,
    source_artifact_id: ArtifactId,
    expected_source_head: RevisionId,
    transformation: Transformation,
    receipt: ExecutionReceipt,
    output_bytes: Arc<[u8]>,
    output_media_type: String,
    output_contract: AudioValueContract,
    request_node: Option<shape_domain::WorkingOperatorDraft>,
}

impl fmt::Debug for AudioCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AudioCandidate")
            .field("artifact_id", &self.artifact.id)
            .field("source_artifact_id", &self.source_artifact_id)
            .field("expected_source_head", &self.expected_source_head)
            .field("transformation_id", &self.transformation.id)
            .field("receipt", &self.receipt)
            .field("output_byte_length", &self.output_bytes.len())
            .field("output_media_type", &self.output_media_type)
            .field("output_contract", &self.output_contract)
            .finish_non_exhaustive()
    }
}

impl AudioCandidate {
    #[must_use]
    pub fn request_node(&self) -> Option<&shape_domain::WorkingOperatorDraft> {
        self.request_node.as_ref()
    }
    #[must_use]
    pub const fn expected_target_head(&self) -> Option<RevisionId> {
        self.artifact.accepted_revision
    }
    #[must_use]
    pub fn review_artifact_id(&self) -> ArtifactId {
        if self.request_node.is_some() {
            self.artifact.id
        } else {
            self.source_artifact_id
        }
    }

    /// Returns the identity reserved for the new audio Artifact.
    #[must_use]
    pub const fn artifact_id(&self) -> ArtifactId {
        self.artifact.id
    }

    /// Returns the user-facing name reserved for the new audio Artifact.
    #[must_use]
    pub fn artifact_name(&self) -> &str {
        &self.artifact.name
    }

    /// Returns the accepted text Artifact that owns this Candidate's review context.
    #[must_use]
    pub const fn source_artifact_id(&self) -> ArtifactId {
        self.source_artifact_id
    }

    /// Returns the immutable text head used to synthesize this Candidate.
    #[must_use]
    pub const fn expected_source_head(&self) -> RevisionId {
        self.expected_source_head
    }

    /// Returns exact transient WAV bytes for a future preview owner.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.output_bytes
    }

    /// Returns the re-derived portable audio interpretation.
    #[must_use]
    pub const fn contract(&self) -> &AudioValueContract {
        &self.output_contract
    }

    /// Returns payload-free physical provenance without implying acceptance.
    #[must_use]
    pub const fn receipt(&self) -> &ExecutionReceipt {
        &self.receipt
    }
}

impl ShapeProject {
    /// Imports a verified immutable WAV cue for a narration script.
    /// # Errors
    /// Rejects unsupported audio or a failed object write.
    pub fn import_speech_cue(&self, bytes: &[u8]) -> Result<shape_domain::ContentRef, CoreError> {
        Ok(self.store.import_speech_cue(bytes)?)
    }

    /// Executes the preset-only speech synthesis Operator against accepted text.
    ///
    /// This operation performs no project write. The returned Candidate must be
    /// passed to [`Self::accept_speech_synthesis`] to publish durable history.
    ///
    /// # Errors
    ///
    /// Rejects unknown, non-text, missing, stale, or invalid UTF-8 input, an
    /// authorized Voice Reference, malformed output, missing provenance, or
    /// physical execution failure.
    pub fn propose_speech_synthesis(
        &self,
        source_artifact_id: ArtifactId,
        expected_source_head: RevisionId,
        artifact_name: impl Into<String>,
        operation: &SpeechSynthesisOperation,
        constraints: Vec<Constraint>,
        executor: &dyn Executor,
    ) -> Result<AudioCandidate, CoreError> {
        self.propose_speech_into_artifact(
            source_artifact_id,
            expected_source_head,
            Artifact::new(artifact_name, ArtifactKind::AudioClip)?,
            operation,
            constraints,
            executor,
            None,
        )
    }

    /// Executes a stable speech node whose named output survives repeated runs.
    /// # Errors
    /// Rejects stale or invalid bindings, output heads and media execution.
    pub fn propose_speech_node(
        &self,
        target: ArtifactId,
        draft: &shape_domain::WorkingOperatorDraft,
        operation: &SpeechSynthesisOperation,
        executor: &dyn Executor,
    ) -> Result<AudioCandidate, CoreError> {
        let graph =
            self.store
                .artifact_working_graph(target)?
                .ok_or(CoreError::InvalidSpeechSource {
                    artifact_id: target,
                })?;
        let artifact = self
            .store
            .artifact(target)?
            .ok_or(shape_store::StoreError::UnknownArtifact(target))?;
        let input = draft.input().ok_or(CoreError::InvalidSpeechSource {
            artifact_id: target,
        })?;
        if artifact.kind != ArtifactKind::AudioClip
            || input.artifact_id == target
            || draft.operator_type().as_str() != "audio.speech_synthesize"
            || !graph.operators().contains(draft)
            || graph.expected_revision_id() != artifact.accepted_revision
        {
            return Err(CoreError::InvalidSpeechSource {
                artifact_id: target,
            });
        }
        self.propose_speech_into_artifact(
            input.artifact_id,
            input.revision_id,
            artifact,
            operation,
            Vec::new(),
            executor,
            Some(draft.clone()),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn propose_speech_into_artifact(
        &self,
        source_artifact_id: ArtifactId,
        expected_source_head: RevisionId,
        artifact: Artifact,
        operation: &SpeechSynthesisOperation,
        constraints: Vec<Constraint>,
        executor: &dyn Executor,
        request_node: Option<shape_domain::WorkingOperatorDraft>,
    ) -> Result<AudioCandidate, CoreError> {
        let (accepted, resolved) =
            self.speech_source(source_artifact_id, expected_source_head, operation)?;
        let operation = &resolved;
        let transformation = Transformation::new_with_operation(
            TransformationKind::GenerativeEdit,
            artifact.id,
            vec![expected_source_head],
            IntentSpec::new(SPEECH_SYNTHESIS_INTENT)?,
            constraints,
            Vec::new(),
            Some(TransformationOperation::AudioSpeechSynthesis(
                operation.clone(),
            )),
        )?;
        let input = ExecutionInput::materialized(accepted.revision.content, accepted.bytes)?;
        let mut inputs = vec![input];
        if let Some(options) = &operation.script {
            for action in options.cues.values() {
                if let shape_domain::speech_script::SpeechCueAction::Audio { content } = action
                    && !inputs.iter().any(|input| input.content() == content)
                {
                    inputs.push(ExecutionInput::materialized(
                        content.clone(),
                        self.store.read_content(content)?,
                    )?);
                }
            }
        }
        let request = ExecutionRequest::new_materialized(
            transformation.id,
            CapabilityId::new(AUDIO_SPEECH_SYNTHESIZE_CAPABILITY)?,
            inputs,
            serde_json::to_vec(operation)?,
            AUDIO_MEDIA_TYPE,
        )?;
        let ExecutedCandidate { output, receipt } =
            ExecutionCoordinator::execute(executor, &request)?;
        if output.media_type != AUDIO_MEDIA_TYPE {
            return Err(CoreError::MissingAudioOutputContract);
        }
        let Some(ArtifactContentContract::AudioClip(output_contract)) = output.content_contract
        else {
            return Err(CoreError::MissingAudioOutputContract);
        };
        let derived = parse_pcm_s16le_wav(&output.bytes, AudioOriginDisclosure::SyntheticSpeech)
            .map_err(|_| CoreError::AudioOutputContractMismatch)?;
        if output_contract != derived
            || receipt.executor_job_id.is_none()
            || receipt.external_provenance.is_none()
        {
            return Err(CoreError::AudioOutputContractMismatch);
        }
        validate_speech_sources(&receipt, &request, operation, &output.bytes)?;
        Ok(AudioCandidate {
            artifact,
            request_node,
            source_artifact_id,
            expected_source_head,
            transformation,
            receipt,
            output_bytes: output.bytes.into(),
            output_media_type: output.media_type,
            output_contract,
        })
    }
    fn speech_source(
        &self,
        source_artifact_id: ArtifactId,
        expected_source_head: RevisionId,
        operation: &SpeechSynthesisOperation,
    ) -> Result<(super::AcceptedArtifactContent, SpeechSynthesisOperation), CoreError> {
        operation.validate()?;
        if matches!(
            &operation.voice,
            SpeechVoiceSelection::AuthorizedReference(_)
        ) {
            return Err(CoreError::UnsupportedSpeechVoiceReference);
        }
        let source_artifact = self
            .store
            .artifact(source_artifact_id)?
            .ok_or(shape_store::StoreError::UnknownArtifact(source_artifact_id))?;
        if source_artifact.kind != ArtifactKind::TextDocument {
            return Err(CoreError::InvalidSpeechSource {
                artifact_id: source_artifact_id,
            });
        }
        if source_artifact.accepted_revision != Some(expected_source_head) {
            return Err(CoreError::StaleCandidate {
                artifact_id: source_artifact_id,
                expected: Some(expected_source_head),
                actual: source_artifact.accepted_revision,
            });
        }
        let accepted =
            self.read_accepted(source_artifact_id)?
                .ok_or(CoreError::MissingAcceptedRevision {
                    artifact_id: source_artifact_id,
                    revision_id: expected_source_head,
                })?;
        if accepted.revision.id != expected_source_head
            || !accepted.revision.content.media_type.starts_with("text/")
            || std::str::from_utf8(&accepted.bytes).is_err()
        {
            return Err(CoreError::InvalidSpeechSource {
                artifact_id: source_artifact_id,
            });
        }

        let mut resolved = operation.clone();
        if let Some(ArtifactContentContract::TextDocument(format)) =
            &accepted.revision.content_contract
        {
            if !format.accepts(&accepted.bytes) {
                return Err(CoreError::InvalidSpeechSource {
                    artifact_id: source_artifact_id,
                });
            }
            if format.is_script() {
                resolved.script.get_or_insert_with(Default::default);
            } else if resolved.script.is_some() {
                return Err(CoreError::InvalidSpeechSource {
                    artifact_id: source_artifact_id,
                });
            }
        }
        Ok((accepted, resolved))
    }

    /// Atomically creates the synthesized `AudioClip` if the source text head is unchanged.
    ///
    /// # Errors
    ///
    /// Rejects a stale source or any mismatch between exact WAV bytes, typed
    /// operation, successful receipt, and bounded Runtime provenance.
    pub fn accept_speech_synthesis(
        &mut self,
        candidate: AudioCandidate,
    ) -> Result<ArtifactRevision, CoreError> {
        let source = self.store.artifact(candidate.source_artifact_id)?.ok_or(
            shape_store::StoreError::UnknownArtifact(candidate.source_artifact_id),
        )?;
        if source.accepted_revision != Some(candidate.expected_source_head) {
            return Err(CoreError::StaleCandidate {
                artifact_id: candidate.source_artifact_id,
                expected: Some(candidate.expected_source_head),
                actual: source.accepted_revision,
            });
        }
        let source_artifact_id = candidate.source_artifact_id;
        let expected_source_head = candidate.expected_source_head;
        if let Some(node) = &candidate.request_node {
            return Ok(self.store.accept_with_node(
                shape_store::AcceptedCommit {
                    artifact_id: candidate.artifact.id,
                    expected_head: candidate.artifact.accepted_revision,
                    transformation: candidate.transformation,
                    receipt: candidate.receipt,
                    output_bytes: candidate.output_bytes,
                    output_media_type: candidate.output_media_type,
                    content_contract: Some(ArtifactContentContract::AudioClip(
                        candidate.output_contract,
                    )),
                },
                &[(source_artifact_id, expected_source_head)],
                Some(node),
            )?);
        }
        match self.store.accept_new_artifact(NewArtifactCommit {
            artifact: candidate.artifact,
            expected_input_heads: vec![(source_artifact_id, expected_source_head)],
            transformation: candidate.transformation,
            receipt: candidate.receipt,
            output_bytes: candidate.output_bytes,
            output_media_type: candidate.output_media_type,
            content_contract: Some(ArtifactContentContract::AudioClip(
                candidate.output_contract,
            )),
        }) {
            Ok(revision) => Ok(revision),
            Err(shape_store::StoreError::RevisionConflict { actual, .. }) => {
                Err(CoreError::StaleCandidate {
                    artifact_id: source_artifact_id,
                    expected: Some(expected_source_head),
                    actual,
                })
            }
            Err(error) => Err(error.into()),
        }
    }
}

fn validate_speech_sources(
    receipt: &ExecutionReceipt,
    request: &ExecutionRequest,
    operation: &SpeechSynthesisOperation,
    output: &[u8],
) -> Result<(), CoreError> {
    if operation.script.is_some() {
        let source = request.inputs[0]
            .bytes()
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .ok_or(CoreError::AudioOutputContractMismatch)?;
        shape_execution::speech_script::validate_output(
            source,
            operation,
            &request.inputs,
            receipt
                .external_provenance
                .as_ref()
                .ok_or(CoreError::AudioOutputContractMismatch)?,
            output,
        )
        .map_err(|_| CoreError::AudioOutputContractMismatch)?;
    } else {
        if receipt
            .external_provenance
            .as_ref()
            .is_some_and(|p| p.speech_script.is_some())
        {
            return Err(CoreError::AudioOutputContractMismatch);
        }
        validate_segment_sources(receipt, request)?;
    }
    Ok(())
}

fn validate_segment_sources(
    receipt: &ExecutionReceipt,
    request: &ExecutionRequest,
) -> Result<(), CoreError> {
    let provenance = receipt
        .external_provenance
        .as_ref()
        .ok_or(CoreError::AudioOutputContractMismatch)?;
    if provenance.speech_segments.is_empty() {
        return Ok(());
    }
    let source = request
        .inputs
        .first()
        .and_then(ExecutionInput::bytes)
        .ok_or(CoreError::AudioOutputContractMismatch)?;
    if provenance
        .speech_segments
        .last()
        .is_none_or(|part| part.input_end as usize != source.len())
        || provenance.speech_segments.iter().any(|part| {
            source
                .get(part.input_start as usize..part.input_end as usize)
                .is_none_or(|bytes| {
                    shape_domain::ContentDigest::from_bytes(bytes) != part.input_digest
                })
        })
    {
        return Err(CoreError::AudioOutputContractMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
