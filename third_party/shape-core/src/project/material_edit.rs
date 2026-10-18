//! Persisted material edit stages and explicit, source-bound return acceptance.
//! External files/processes never acquire access to the project store.

use super::{AcceptedArtifactContent, ShapeProject};
use crate::CoreError;
use serde::{Deserialize, Serialize};
use shape_domain::{
    Artifact, ArtifactContentContract, ArtifactId, ArtifactRevision, ArtifactWorkingGraph,
    ContentDigest, ContentRef, IntentSpec, MaterialEditConfiguration, MaterialEditOperation,
    MaterialEditor, OperatorDataTypeId, OperatorTypeId, ProjectId, RevisionId, Transformation,
    TransformationKind, TransformationOperation, WorkingInput, WorkingOperatorDraft,
};
use shape_execution::{
    CapabilityId, ExecutedCandidate, ExecutionCoordinator, ExecutionInput, ExecutionReceipt,
    ExecutionRequest, MaterialReturnExecutor,
};
use shape_store::AcceptedCommit;
mod files;
mod shadow;
pub use files::MaterialEditHandoff;

use std::sync::Arc;

pub const MATERIAL_EDIT_TICKET_SCHEMA: &str = "shape.material-edit-ticket/1";

/// Portable return location and exact source/draft/output identity. No local paths or credentials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialEditTicket {
    pub schema: String,
    pub project_id: ProjectId,
    pub output_artifact_id: ArtifactId,
    pub expected_output_head: Option<RevisionId>,
    pub draft: WorkingOperatorDraft,
    pub input_content: ContentRef,
}

/// Verified edited material awaiting review and explicit adoption.
#[derive(Debug, Clone)]
pub struct MaterialEditCandidate {
    artifact_name: String,
    ticket: MaterialEditTicket,
    transformation: Transformation,
    receipt: ExecutionReceipt,
    bytes: Arc<[u8]>,
    contract: ArtifactContentContract,
}

impl MaterialEditCandidate {
    #[must_use]
    pub fn artifact_name(&self) -> &str {
        &self.artifact_name
    }
    #[must_use]
    pub const fn artifact_id(&self) -> ArtifactId {
        self.ticket.output_artifact_id
    }
    #[must_use]
    pub const fn ticket(&self) -> &MaterialEditTicket {
        &self.ticket
    }
    #[must_use]
    pub const fn receipt(&self) -> &ExecutionReceipt {
        &self.receipt
    }
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    #[must_use]
    pub fn normalization(&self) -> shape_domain::MaterialReturnNormalization {
        let Some(TransformationOperation::MaterialEdit(operation)) = &self.transformation.operation
        else {
            unreachable!("material candidates carry material intent");
        };
        operation.normalization
    }
    #[must_use]
    pub fn returned_digest(&self) -> ContentDigest {
        let Some(TransformationOperation::MaterialEdit(operation)) = &self.transformation.operation
        else {
            unreachable!("material candidates carry material intent");
        };
        operation.returned_digest
    }
    #[must_use]
    pub const fn contract(&self) -> &ArtifactContentContract {
        &self.contract
    }
}

impl ShapeProject {
    /// Reserves a separate named output and stable producer; the source remains immutable.
    /// # Errors
    /// Rejects wrong media, stale source, invalid names, or atomic persistence failure.
    pub fn create_material_edit(
        &mut self,
        source: WorkingInput,
        name: &str,
        editor: MaterialEditor,
    ) -> Result<MaterialEditTicket, CoreError> {
        self.material_source(source, editor)?;
        let artifact = Artifact::new(name, editor.artifact_kind())?;
        let mut graph = ArtifactWorkingGraph::new_source(artifact.id);
        let data = OperatorDataTypeId::new(editor.data_type())?;
        let draft = graph.add_bound_operator(
            OperatorTypeId::new(editor.operator_type())?,
            data.clone(),
            data,
            source,
        )?;
        graph.set_operator_configuration(
            draft.id(),
            Some(MaterialEditConfiguration { editor, session: 1 }.encode()?),
        );
        self.create_source_artifact_draft(&artifact, &graph)?;
        self.material_edit_ticket(artifact.id)
    }

    /// Reloads the current stage after restart without retaining an editor process.
    /// # Errors
    /// Rejects missing/incompatible stages and stale source versions.
    pub fn material_edit_ticket(
        &self,
        output: ArtifactId,
    ) -> Result<MaterialEditTicket, CoreError> {
        let artifact = self.store.artifact(output)?.ok_or_else(invalid)?;
        let graph = self
            .store
            .artifact_working_graph(output)?
            .ok_or_else(invalid)?;
        if graph.operators().len() != 1
            || graph.expected_revision_id() != artifact.accepted_revision
        {
            return Err(invalid());
        }
        let draft = graph.operators()[0].clone();
        let configuration = MaterialEditConfiguration::from_draft(&draft)?;
        if artifact.kind != configuration.editor.artifact_kind() {
            return Err(invalid());
        }
        // A stale source remains inspectable/restartable; dispatch validates its current head.
        let source = draft.input().ok_or_else(invalid)?;
        let revision = self.revision(source.revision_id)?;
        if revision.artifact_id != source.artifact_id {
            return Err(invalid());
        }
        Ok(MaterialEditTicket {
            schema: MATERIAL_EDIT_TICKET_SCHEMA.into(),
            project_id: self.snapshot()?.metadata.id,
            output_artifact_id: output,
            expected_output_head: artifact.accepted_revision,
            draft,
            input_content: revision.content,
        })
    }

    /// Reads verified exact input bytes for an isolated editable copy.
    /// # Errors
    /// Rejects foreign tickets, stale stages/heads, or changed source bytes.
    pub fn material_edit_source(
        &self,
        ticket: &MaterialEditTicket,
    ) -> Result<AcceptedArtifactContent, CoreError> {
        self.validate_material_ticket(ticket)?;
        let configuration = MaterialEditConfiguration::from_draft(&ticket.draft)?;
        let source = self.material_source(
            ticket.draft.input().ok_or_else(invalid)?,
            configuration.editor,
        )?;
        if source.revision.content != ticket.input_content {
            return Err(invalid());
        }
        Ok(source)
    }

    /// Invalidates previous exports/candidates, retaining the stage and accepted history.
    /// `refresh_input` explicitly binds the latest source; false cancels the old session only.
    /// # Errors
    /// Rejects concurrent stage changes without overwriting them.
    pub fn restart_material_edit(
        &self,
        ticket: &MaterialEditTicket,
        refresh_input: bool,
    ) -> Result<MaterialEditTicket, CoreError> {
        let mut graph = self.validate_material_ticket(ticket)?;
        let previous = graph.clone();
        let mut configuration = MaterialEditConfiguration::from_draft(&ticket.draft)?;
        configuration.session = configuration.session.checked_add(1).ok_or_else(invalid)?;
        if refresh_input {
            let mut source = ticket.draft.input().ok_or_else(invalid)?;
            source.revision_id = self
                .store
                .artifact(source.artifact_id)?
                .ok_or_else(invalid)?
                .accepted_revision
                .ok_or_else(invalid)?;
            self.material_source(source, configuration.editor)?;
            if !graph.set_operator_input(ticket.draft.id(), source) {
                return Err(invalid());
            }
        }
        if !graph.set_operator_configuration(ticket.draft.id(), Some(configuration.encode()?)) {
            return Err(invalid());
        }
        self.replace_artifact_working_graph(
            ticket.output_artifact_id,
            Some(&previous),
            Some(&graph),
        )?;
        self.material_edit_ticket(ticket.output_artifact_id)
    }

    /// Checks a selected return against the exact ticket and caller-reviewed byte digest.
    /// Creates no accepted revision; invalid retries leave prior candidates usable.
    /// # Errors
    /// Rejects stale/foreign tickets, digest mismatches, unsupported media and no-op returns.
    pub fn propose_material_return(
        &self,
        ticket: &MaterialEditTicket,
        bytes: Vec<u8>,
        expected_digest: ContentDigest,
    ) -> Result<MaterialEditCandidate, CoreError> {
        self.propose_material_return_with_options(ticket, bytes, expected_digest, false)
    }

    /// Reviews a return with an explicit optional PCM24-to-PCM16 precision reduction.
    /// # Errors
    /// Rejects invalid/stale returns; conversion is never enabled by the default API.
    pub fn propose_material_return_with_options(
        &self,
        ticket: &MaterialEditTicket,
        bytes: Vec<u8>,
        expected_digest: ContentDigest,
        allow_pcm24_conversion: bool,
    ) -> Result<MaterialEditCandidate, CoreError> {
        if bytes.is_empty()
            || bytes.len() > 128 * 1024 * 1024
            || ContentDigest::from_bytes(&bytes) != expected_digest
        {
            return Err(invalid());
        }
        let source = self.material_edit_source(ticket)?;
        let configuration = MaterialEditConfiguration::from_draft(&ticket.draft)?;
        let source_contract = source
            .revision
            .content_contract
            .clone()
            .ok_or_else(invalid)?;
        let operation = MaterialEditOperation {
            editor: configuration.editor,
            stage_id: ticket.draft.id().clone(),
            session: configuration.session,
            source: ticket.draft.input().ok_or_else(invalid)?,
            input_digest: ticket.input_content.digest,
            returned_digest: expected_digest,
            normalization: if allow_pcm24_conversion
                && configuration.editor == MaterialEditor::Echo
                && shape_execution::is_pcm24_material_return(&bytes)
            {
                shape_domain::MaterialReturnNormalization::Pcm24ToPcm16
            } else {
                shape_domain::MaterialReturnNormalization::Canonical
            },
        };
        let transformation = Transformation::new_with_operation(
            TransformationKind::ExternalRoundTrip,
            ticket.output_artifact_id,
            vec![operation.source.revision_id],
            IntentSpec::new(format!(
                "Review edited material from {}",
                configuration.editor.name()
            ))?,
            Vec::new(),
            Vec::new(),
            Some(TransformationOperation::MaterialEdit(operation)),
        )?;
        let request = ExecutionRequest::new_materialized(
            transformation.id,
            CapabilityId::new(configuration.editor.operator_type())?,
            vec![ExecutionInput::materialized(
                source.revision.content,
                source.bytes,
            )?],
            bytes,
            configuration.editor.media_type(),
        )?;
        let ExecutedCandidate { output, receipt } = ExecutionCoordinator::execute(
            &MaterialReturnExecutor::new(configuration.editor, source_contract)?
                .with_pcm24_conversion(allow_pcm24_conversion),
            &request,
        )?;
        let digest = ContentDigest::from_bytes(&output.bytes);
        if digest == ticket.input_content.digest
            || self
                .read_accepted(ticket.output_artifact_id)?
                .is_some_and(|current| current.revision.content.digest == digest)
        {
            return Err(CoreError::UnchangedMaterialReturn);
        }
        Ok(MaterialEditCandidate {
            artifact_name: self
                .store
                .artifact(ticket.output_artifact_id)?
                .ok_or_else(invalid)?
                .name,
            ticket: ticket.clone(),
            transformation,
            receipt,
            bytes: output.bytes.into(),
            contract: output.content_contract.ok_or_else(invalid)?,
        })
    }

    /// Advances only the stage output, atomically checking source, target and exact authored node.
    /// # Errors
    /// Rejects stale/cancelled candidates and duplicate acceptance without changing originals.
    pub fn accept_material_return(
        &mut self,
        candidate: MaterialEditCandidate,
    ) -> Result<ArtifactRevision, CoreError> {
        self.material_edit_source(&candidate.ticket)?;
        let configuration = MaterialEditConfiguration::from_draft(&candidate.ticket.draft)?;
        let input = candidate.ticket.draft.input().ok_or_else(invalid)?;
        Ok(self.store.accept_with_node(
            AcceptedCommit {
                artifact_id: candidate.ticket.output_artifact_id,
                expected_head: candidate.ticket.expected_output_head,
                transformation: candidate.transformation,
                receipt: candidate.receipt,
                output_bytes: candidate.bytes,
                output_media_type: configuration.editor.media_type().into(),
                content_contract: Some(candidate.contract),
            },
            &[(input.artifact_id, input.revision_id)],
            Some(&candidate.ticket.draft),
        )?)
    }

    fn validate_material_ticket(
        &self,
        ticket: &MaterialEditTicket,
    ) -> Result<ArtifactWorkingGraph, CoreError> {
        if ticket.schema != MATERIAL_EDIT_TICKET_SCHEMA
            || ticket.project_id != self.snapshot()?.metadata.id
        {
            return Err(invalid());
        }
        let current = self.material_edit_ticket(ticket.output_artifact_id)?;
        if &current != ticket {
            return Err(CoreError::StaleMaterialSession);
        }
        self.store
            .artifact_working_graph(ticket.output_artifact_id)?
            .ok_or_else(invalid)
    }

    fn material_source(
        &self,
        input: WorkingInput,
        editor: MaterialEditor,
    ) -> Result<AcceptedArtifactContent, CoreError> {
        let artifact = self
            .store
            .artifact(input.artifact_id)?
            .ok_or_else(invalid)?;
        if artifact.kind != editor.artifact_kind() {
            return Err(invalid());
        }
        if artifact.accepted_revision != Some(input.revision_id) {
            return Err(CoreError::StaleCandidate {
                artifact_id: input.artifact_id,
                expected: Some(input.revision_id),
                actual: artifact.accepted_revision,
            });
        }
        let source = self.read_revision_content(input.revision_id)?;
        if source.revision.artifact_id != input.artifact_id
            || source.revision.content.media_type != editor.media_type()
        {
            return Err(invalid());
        }
        Ok(source)
    }
}

fn invalid() -> CoreError {
    CoreError::InvalidMaterialEdit
}

#[cfg(test)]
mod tests;
