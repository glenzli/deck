//! One accepted text/code source sent as exact bytes to a bounded Agent file task.
//!
//! The source is immutable. A successful task creates a transient candidate for
//! a separate text Artifact; only explicit acceptance publishes that Artifact.

use shape_domain::{
    Artifact, ArtifactContentContract, ArtifactId, ArtifactKind, ArtifactRevision, IntentSpec,
    RevisionId, TextDocumentContract, Transformation, TransformationKind,
};
use shape_execution::{
    CapabilityId, ExecutedCandidate, ExecutionCoordinator, ExecutionInput, ExecutionReceipt,
    ExecutionRequest, Executor,
};
use shape_store::NewArtifactCommit;

use super::ShapeProject;
use crate::CoreError;

const AGENT_FILE_CAPABILITY: &str = "agent.file_task";
const TEXT_MEDIA_TYPE: &str = "text/plain; charset=utf-8";
const MAX_INSTRUCTION_BYTES: usize = 16 * 1024;
const MAX_RESULT_BYTES: usize = 1024 * 1024;

/// Text/code file task result awaiting a separate, explicit Accept.
#[derive(Debug, Clone)]
pub struct AgentTextCandidate {
    artifact: Artifact,
    source_artifact_id: ArtifactId,
    expected_source_head: RevisionId,
    transformation: Transformation,
    receipt: ExecutionReceipt,
    output_text: String,
}

impl AgentTextCandidate {
    #[must_use]
    pub const fn artifact_id(&self) -> ArtifactId {
        self.artifact.id
    }
    #[must_use]
    pub fn artifact_name(&self) -> &str {
        &self.artifact.name
    }
    #[must_use]
    pub const fn source_artifact_id(&self) -> ArtifactId {
        self.source_artifact_id
    }
    #[must_use]
    pub const fn expected_source_head(&self) -> RevisionId {
        self.expected_source_head
    }
    #[must_use]
    pub fn text(&self) -> &str {
        &self.output_text
    }
    #[must_use]
    pub const fn receipt(&self) -> &ExecutionReceipt {
        &self.receipt
    }
}

impl ShapeProject {
    /// Executes a file task against one verified, accepted text source.
    ///
    /// No durable state is changed. The executor receives exact materialized
    /// source bytes, never a host path or an unaccepted editor buffer.
    /// # Errors
    /// Rejects stale or non-text input, invalid instructions, and invalid output.
    pub fn propose_agent_text_file(
        &self,
        source_artifact_id: ArtifactId,
        expected_source_head: RevisionId,
        output_name: impl Into<String>,
        instruction: &str,
        executor: &dyn Executor,
    ) -> Result<AgentTextCandidate, CoreError> {
        if instruction.trim().is_empty() || instruction.len() > MAX_INSTRUCTION_BYTES {
            return Err(CoreError::InvalidTextTransformInstruction);
        }
        let source = self
            .store
            .artifact(source_artifact_id)?
            .ok_or(shape_store::StoreError::UnknownArtifact(source_artifact_id))?;
        if source.kind != ArtifactKind::TextDocument {
            return Err(CoreError::InvalidTextArtifact {
                artifact_id: source_artifact_id,
            });
        }
        if source.accepted_revision != Some(expected_source_head) {
            return Err(CoreError::StaleCandidate {
                artifact_id: source_artifact_id,
                expected: Some(expected_source_head),
                actual: source.accepted_revision,
            });
        }
        let accepted =
            self.read_accepted(source_artifact_id)?
                .ok_or(CoreError::MissingAcceptedRevision {
                    artifact_id: source_artifact_id,
                    revision_id: expected_source_head,
                })?;
        if accepted.revision.id != expected_source_head
            || std::str::from_utf8(&accepted.bytes).is_err()
        {
            return Err(CoreError::InvalidTextCandidate);
        }
        let artifact = Artifact::new(output_name, ArtifactKind::TextDocument)?;
        let summary = instruction.chars().take(180).collect::<String>();
        let transformation = Transformation::new(
            TransformationKind::GenerativeEdit,
            artifact.id,
            vec![expected_source_head],
            IntentSpec::new(format!("Agent file task: {summary}"))?,
            Vec::new(),
            Vec::new(),
        )?;
        let input = ExecutionInput::materialized(accepted.revision.content, accepted.bytes)?;
        let request = ExecutionRequest::new_materialized(
            transformation.id,
            CapabilityId::new(AGENT_FILE_CAPABILITY)?,
            vec![input],
            instruction.as_bytes().to_vec(),
            TEXT_MEDIA_TYPE,
        )?;
        let ExecutedCandidate { output, receipt } =
            ExecutionCoordinator::execute(executor, &request)?;
        if output.media_type != TEXT_MEDIA_TYPE
            || output.bytes.len() > MAX_RESULT_BYTES
            || !TextDocumentContract::Plain.accepts(&output.bytes)
        {
            return Err(CoreError::InvalidTextCandidate);
        }
        let output_text =
            String::from_utf8(output.bytes).map_err(|_| CoreError::InvalidTextCandidate)?;
        Ok(AgentTextCandidate {
            artifact,
            source_artifact_id,
            expected_source_head,
            transformation,
            receipt,
            output_text,
        })
    }

    /// Publishes the reviewed task output as a new text Artifact if its source
    /// still has the exact accepted head used during execution.
    /// # Errors
    /// Rejects stale source heads and failed atomic publication.
    pub fn accept_agent_text_file(
        &mut self,
        candidate: AgentTextCandidate,
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
        match self.store.accept_new_artifact(NewArtifactCommit {
            artifact: candidate.artifact,
            expected_input_heads: vec![(
                candidate.source_artifact_id,
                candidate.expected_source_head,
            )],
            transformation: candidate.transformation,
            receipt: candidate.receipt,
            output_bytes: candidate.output_text.into_bytes().into(),
            output_media_type: TEXT_MEDIA_TYPE.to_owned(),
            content_contract: Some(ArtifactContentContract::TextDocument(
                TextDocumentContract::Plain,
            )),
        }) {
            Ok(revision) => Ok(revision),
            Err(shape_store::StoreError::RevisionConflict { actual, .. }) => {
                Err(CoreError::StaleCandidate {
                    artifact_id: candidate.source_artifact_id,
                    expected: Some(candidate.expected_source_head),
                    actual,
                })
            }
            Err(error) => Err(error.into()),
        }
    }
}

#[cfg(test)]
mod tests;
