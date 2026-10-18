//! Execution of authored text nodes with independent source and output identities.

use super::{
    ArtifactId, CapabilityId, CoreError, ExecutedCandidate, ExecutionCoordinator, ExecutionRequest,
    Executor, IntentSpec, LiteralTextExecutor, ShapeProject, TEXT_GENERATE_CAPABILITY,
    TEXT_LITERAL_CAPABILITY, TEXT_MEDIA_TYPE, TextCandidate, Transformation, TransformationKind,
    generation_instruction, text_candidate, validate_expected_head,
};
use shape_domain::{TextDocumentContract, WorkingOperatorDraft};

impl ShapeProject {
    /// Executes a text creation or derivation node against its persisted binding.
    ///
    /// # Errors
    /// Rejects missing/stale inputs, an invalid node or an invalid output format.
    pub fn propose_text_node(
        &self,
        target: ArtifactId,
        draft: &WorkingOperatorDraft,
        instruction: &str,
        format: TextDocumentContract,
        executor: &dyn Executor,
    ) -> Result<TextCandidate, CoreError> {
        self.execute_text_node(target, draft, instruction, format, Some(executor))
    }

    /// Reviews manually authored text through the same typed node boundary.
    ///
    /// # Errors
    /// Rejects missing/stale inputs or bytes that violate the selected format.
    pub fn propose_text_node_literal(
        &self,
        target: ArtifactId,
        draft: &WorkingOperatorDraft,
        text: &str,
        format: TextDocumentContract,
    ) -> Result<TextCandidate, CoreError> {
        self.execute_text_node(target, draft, text, format, None)
    }

    fn execute_text_node(
        &self,
        target: ArtifactId,
        draft: &WorkingOperatorDraft,
        text: &str,
        format: TextDocumentContract,
        executor: Option<&dyn Executor>,
    ) -> Result<TextCandidate, CoreError> {
        let artifact = self.text_artifact(target)?;
        let graph = self
            .store
            .artifact_working_graph(target)?
            .ok_or(CoreError::InvalidTextCandidate)?;
        if !graph.operators().contains(draft) {
            return Err(CoreError::InvalidTextCandidate);
        }
        validate_expected_head(
            target,
            graph.expected_revision_id(),
            artifact.accepted_revision,
        )?;
        match (draft.operator_type().as_str(), draft.input()) {
            ("text.create", None) | ("text.edit", Some(_)) => {}
            _ => return Err(CoreError::InvalidTextCandidate),
        }
        let mut inputs = Vec::new();
        let mut input_content = Vec::new();
        let mut input_heads = Vec::new();
        let mut source_text = None;
        if let Some(input) = draft.input() {
            if input.artifact_id == target {
                return Err(CoreError::InvalidTextCandidate);
            }
            let source = self.text_artifact(input.artifact_id)?;
            validate_expected_head(source.id, Some(input.revision_id), source.accepted_revision)?;
            let accepted = self
                .read_accepted(source.id)?
                .ok_or(CoreError::InvalidTextCandidate)?;
            inputs.push(input.revision_id);
            input_heads.push((source.id, input.revision_id));
            input_content.push(accepted.revision.content);
            source_text = Some(
                String::from_utf8(accepted.bytes).map_err(|_| CoreError::InvalidTextCandidate)?,
            );
        }
        let kind = if executor.is_some() {
            TransformationKind::GenerativeEdit
        } else {
            TransformationKind::TextRewrite
        };
        let transformation = Transformation::new(
            kind,
            target,
            inputs,
            IntentSpec::new(format!("{} document", draft.operator_type().as_str()))?,
            Vec::new(),
            Vec::new(),
        )?;
        let payload = if executor.is_some() {
            generation_instruction(text, source_text.as_deref())
        } else {
            text.into()
        };
        let request = ExecutionRequest::new(
            transformation.id,
            CapabilityId::new(if executor.is_some() {
                TEXT_GENERATE_CAPABILITY
            } else {
                TEXT_LITERAL_CAPABILITY
            })?,
            input_content,
            payload.into_bytes(),
            TEXT_MEDIA_TYPE,
        )?;
        let literal = LiteralTextExecutor::new()?;
        let ExecutedCandidate { output, receipt } =
            ExecutionCoordinator::execute(executor.unwrap_or(&literal), &request)?;
        if executor.is_none() && !format.accepts(&output.bytes) {
            return Err(CoreError::InvalidTextCandidate);
        }
        let mut candidate = text_candidate(
            target,
            artifact.accepted_revision,
            transformation,
            receipt,
            output,
        )?;
        candidate.content_contract = format;
        candidate.input_heads = input_heads;
        candidate.request_node = Some(draft.clone());
        Ok(candidate)
    }
}

#[cfg(test)]
mod tests;
