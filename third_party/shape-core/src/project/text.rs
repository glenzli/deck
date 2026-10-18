//! Text Operator parameters, transient previews, Candidates, and acceptance.
//!
//! The deterministic calibration `text.edit` path consumes one accepted
//! `text.document` revision and produces one full replacement `text.document`
//! candidate. The Infer-backed `text.transform` path consumes the same typed
//! value but carries a stable creative mode rather than making the provider or
//! prompt shape its Operator identity. Draft text and candidate bytes remain
//! memory-only. Explicit acceptance alone delegates to the store's expected-head
//! compare-and-swap and creates an immutable Revision. Provider Jobs remain
//! physical receipt evidence and never become Operators.

use std::fmt;

use shape_domain::{
    Artifact, ArtifactId, ArtifactKind, ArtifactRevision, Constraint, IntentSpec, RevisionId,
    Transformation, TransformationKind,
};
use shape_execution::{
    CapabilityId, ExecutedCandidate, ExecutionCoordinator, ExecutionFailure, ExecutionOutput,
    ExecutionReceipt, ExecutionRequest, Executor, ExecutorIdentity,
};
use shape_store::{AcceptedCommit, NewArtifactCommit};

use super::ShapeProject;
use crate::CoreError;

mod node;

/// Stable creative Operator identity projected into the Scene Graph.
pub const TEXT_EDIT_OPERATOR_TYPE: &str = "text.edit";
/// Stable creative Operator identity for provider-backed text transformation.
pub const TEXT_TRANSFORM_OPERATOR_TYPE: &str = "text.transform";
/// Typed value consumed and produced by the direct Text Operator.
pub const TEXT_DOCUMENT_DATA_TYPE: &str = "text.document";

const TEXT_LITERAL_CAPABILITY: &str = "text.literal";
const TEXT_GENERATE_CAPABILITY: &str = "text.generate";
const TEXT_MEDIA_TYPE: &str = "text/plain; charset=utf-8";
const BUILTIN_CONTRACT_REVISION: &str = "20260810.1";
const INITIAL_TEXT_SOURCE_INTENT: &str = "Create initial text source";

/// Complete parameters for one deterministic calibration `text.edit` proposal.
///
/// The first vertical slice replaces the complete document. Selection, block,
/// span, and semantic-diff parameters remain deferred until a real structured
/// text consumer can define them without changing this operation implicitly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEditParameters {
    replacement_text: String,
}

/// Stable creative mode for one provider-backed `text.transform` proposal.
///
/// These modes describe user intent. They do not select a provider, model,
/// Runtime intent, or physical executor implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextTransformMode {
    /// Re-express the complete accepted document under one creative direction.
    Rewrite,
    /// Add useful detail while retaining the accepted document's subject.
    Expand,
    /// Improve clarity and style without changing the accepted document's intent.
    Polish,
    /// Condense the accepted document while preserving its essential meaning.
    Shorten,
    /// Extract the essential claims into a shorter standalone account.
    Summarize,
}

impl TextTransformMode {
    /// Returns the language-neutral creative mode key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rewrite => "rewrite",
            Self::Expand => "expand",
            Self::Polish => "polish",
            Self::Shorten => "shorten",
            Self::Summarize => "summarize",
        }
    }

    /// Parses one language-neutral authored mode key.
    ///
    /// Unknown keys stay outside the creative contract instead of silently
    /// falling back to Rewrite.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "rewrite" => Some(Self::Rewrite),
            "expand" => Some(Self::Expand),
            "polish" => Some(Self::Polish),
            "shorten" => Some(Self::Shorten),
            "summarize" => Some(Self::Summarize),
            _ => None,
        }
    }

    const fn intent_verb(self) -> &'static str {
        match self {
            Self::Rewrite => "Rewrite text",
            Self::Expand => "Expand text",
            Self::Polish => "Polish text",
            Self::Shorten => "Shorten text",
            Self::Summarize => "Summarize text",
        }
    }
}

/// Typed creative parameters for one Infer-backed Text Transform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextTransformParameters {
    mode: TextTransformMode,
    instruction: String,
}

impl TextTransformParameters {
    /// Creates a transform mode plus its user-authored creative direction.
    ///
    /// # Errors
    ///
    /// Returns an error when the instruction contains no visible text.
    pub fn new(mode: TextTransformMode, instruction: impl Into<String>) -> Result<Self, CoreError> {
        let instruction = instruction.into();
        if instruction.trim().is_empty() {
            return Err(CoreError::InvalidTextTransformInstruction);
        }
        Ok(Self { mode, instruction })
    }

    /// Returns the stable creative mode.
    #[must_use]
    pub const fn mode(&self) -> TextTransformMode {
        self.mode
    }

    /// Returns the exact user-authored direction.
    #[must_use]
    pub fn instruction(&self) -> &str {
        &self.instruction
    }

    fn intent(&self) -> Result<IntentSpec, CoreError> {
        Ok(IntentSpec::new(format!(
            "{}: {}",
            self.mode.intent_verb(),
            self.instruction
        ))?)
    }
}

impl TextEditParameters {
    /// Creates full-document replacement parameters from exact UTF-8 text.
    #[must_use]
    pub fn new(replacement_text: impl Into<String>) -> Self {
        Self {
            replacement_text: replacement_text.into(),
        }
    }

    /// Returns the exact full-document replacement preview.
    #[must_use]
    pub fn replacement_text(&self) -> &str {
        &self.replacement_text
    }

    fn into_replacement_text(self) -> String {
        self.replacement_text
    }
}

/// Transient text output awaiting explicit user acceptance.
#[derive(Clone)]
pub struct TextCandidate {
    artifact_id: ArtifactId,
    expected_head: Option<RevisionId>,
    transformation: Transformation,
    receipt: ExecutionReceipt,
    output_text: String,
    output_media_type: String,
    content_contract: shape_domain::TextDocumentContract,
    input_heads: Vec<(ArtifactId, RevisionId)>,
    request_node: Option<shape_domain::WorkingOperatorDraft>,
}

impl fmt::Debug for TextCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TextCandidate")
            .field("artifact_id", &self.artifact_id)
            .field("expected_head", &self.expected_head)
            .field("transformation_id", &self.transformation.id)
            .field("transformation_kind", &self.transformation.kind)
            .field("receipt", &self.receipt)
            .field("output_byte_length", &self.output_text.len())
            .field("output_media_type", &self.output_media_type)
            .finish_non_exhaustive()
    }
}

impl TextCandidate {
    #[must_use]
    pub fn request_node(&self) -> Option<&shape_domain::WorkingOperatorDraft> {
        self.request_node.as_ref()
    }

    #[must_use]
    pub const fn content_contract(&self) -> &shape_domain::TextDocumentContract {
        &self.content_contract
    }
    /// Returns the target artifact identity.
    #[must_use]
    pub const fn artifact_id(&self) -> ArtifactId {
        self.artifact_id
    }

    /// Returns the immutable head used as this proposal's input.
    #[must_use]
    pub const fn expected_head(&self) -> Option<RevisionId> {
        self.expected_head
    }

    /// Returns candidate text for preview. It is not durable history yet.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.output_text
    }

    /// Returns the creative operation family without exposing physical Jobs.
    #[must_use]
    pub const fn transformation_kind(&self) -> TransformationKind {
        self.transformation.kind
    }

    /// Returns payload-free physical provenance. This is not acceptance state.
    #[must_use]
    pub const fn receipt(&self) -> &ExecutionReceipt {
        &self.receipt
    }
}

impl ShapeProject {
    /// Creates one text document with its initial accepted source revision.
    ///
    /// Artifact identity, import Transformation, execution receipt, content,
    /// and first immutable Revision are published in one store transaction.
    /// This is the compatibility Scene entry used by the desktop until its
    /// persistent Scene editor becomes the navigation authority.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid name, invisible initial text, execution
    /// failure, or atomic durable publication failure.
    pub fn create_text_document(
        &mut self,
        name: impl Into<String>,
        initial_text: impl Into<String>,
    ) -> Result<ArtifactRevision, CoreError> {
        let initial_text = initial_text.into();
        if initial_text.trim().is_empty() {
            return Err(CoreError::InvalidTextCandidate);
        }
        let artifact = Artifact::new(name, ArtifactKind::TextDocument)?;
        let transformation = Transformation::new(
            TransformationKind::Import,
            artifact.id,
            Vec::new(),
            IntentSpec::new(INITIAL_TEXT_SOURCE_INTENT)?,
            Vec::new(),
            Vec::new(),
        )?;
        let request = ExecutionRequest::new(
            transformation.id,
            CapabilityId::new(TEXT_LITERAL_CAPABILITY)?,
            Vec::new(),
            initial_text.into_bytes(),
            TEXT_MEDIA_TYPE,
        )?;
        let ExecutedCandidate { output, receipt } =
            ExecutionCoordinator::execute(&LiteralTextExecutor::new()?, &request)?;
        Ok(self.store.accept_new_artifact(NewArtifactCommit {
            artifact,
            expected_input_heads: Vec::new(),
            transformation,
            receipt,
            output_bytes: output.bytes.into(),
            output_media_type: output.media_type,
            content_contract: None,
        })?)
    }

    /// Executes the first-class direct Text Operator against one accepted input.
    ///
    /// The candidate is tied to `expected_head`; callers preview it and invoke
    /// [`Self::accept_text`] separately to create an immutable Revision.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-text, unknown, missing, or stale input, an
    /// invalid creative contract, or physical execution failure.
    pub fn propose_text_edit(
        &self,
        artifact_id: ArtifactId,
        expected_head: RevisionId,
        parameters: TextEditParameters,
        intent: IntentSpec,
        constraints: Vec<Constraint>,
    ) -> Result<TextCandidate, CoreError> {
        self.propose_literal_text(
            artifact_id,
            Some(expected_head),
            TransformationKind::TextRewrite,
            parameters.into_replacement_text(),
            intent,
            constraints,
        )
    }

    /// Compatibility entry for initial text import and existing Rust consumers.
    ///
    /// Accepted inputs route through the same first-class `text.edit` owner.
    /// A missing input remains an import origin rather than inventing an
    /// input-less Text Operator.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-text, unknown, missing, or stale input, an
    /// invalid creative contract, or physical execution failure.
    pub fn propose_text(
        &self,
        artifact_id: ArtifactId,
        expected_head: Option<RevisionId>,
        text: impl Into<String>,
        intent: IntentSpec,
        constraints: Vec<Constraint>,
    ) -> Result<TextCandidate, CoreError> {
        let parameters = TextEditParameters::new(text);
        match expected_head {
            Some(expected_head) => {
                self.propose_text_edit(artifact_id, expected_head, parameters, intent, constraints)
            }
            None => self.propose_literal_text(
                artifact_id,
                None,
                TransformationKind::Import,
                parameters.into_replacement_text(),
                intent,
                constraints,
            ),
        }
    }

    /// Executes one typed provider-backed Text Transform against an accepted input.
    ///
    /// The creative mode and normalized intent belong to Shape. The supplied
    /// executor remains a physical implementation and contributes only receipt
    /// evidence and transient output bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown, non-text, missing, or stale input,
    /// invalid parameters, or executor failure.
    pub fn propose_text_transform(
        &self,
        artifact_id: ArtifactId,
        expected_head: RevisionId,
        parameters: &TextTransformParameters,
        constraints: Vec<Constraint>,
        executor: &dyn Executor,
    ) -> Result<TextCandidate, CoreError> {
        let intent = parameters.intent()?;
        let physical_instruction = transform_instruction(parameters);
        self.propose_provider_text(
            artifact_id,
            Some(expected_head),
            &physical_instruction,
            intent,
            constraints,
            executor,
        )
    }

    /// Compatibility entry for untyped provider-backed text generation.
    ///
    /// Accepted text is immutable creative context. The provider's Job identity
    /// stays in the physical receipt; the returned bytes remain a Shape Candidate.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown or stale artifact, non-text accepted
    /// content, invalid intent, or executor failure.
    pub fn propose_generated_text(
        &self,
        artifact_id: ArtifactId,
        expected_head: Option<RevisionId>,
        prompt: &str,
        intent: IntentSpec,
        constraints: Vec<Constraint>,
        executor: &dyn Executor,
    ) -> Result<TextCandidate, CoreError> {
        self.propose_provider_text(
            artifact_id,
            expected_head,
            prompt,
            intent,
            constraints,
            executor,
        )
    }

    fn propose_provider_text(
        &self,
        artifact_id: ArtifactId,
        expected_head: Option<RevisionId>,
        creative_instruction: &str,
        intent: IntentSpec,
        constraints: Vec<Constraint>,
        executor: &dyn Executor,
    ) -> Result<TextCandidate, CoreError> {
        let artifact = self.text_artifact(artifact_id)?;
        validate_expected_head(artifact_id, expected_head, artifact.accepted_revision)?;

        let (inputs, input_content, accepted_text) = match expected_head {
            Some(revision_id) => {
                let accepted =
                    self.read_accepted(artifact_id)?
                        .ok_or(CoreError::MissingAcceptedRevision {
                            artifact_id,
                            revision_id,
                        })?;
                debug_assert_eq!(accepted.revision.id, revision_id);
                let text = String::from_utf8(accepted.bytes)
                    .map_err(|_| CoreError::InvalidTextCandidate)?;
                (
                    vec![revision_id],
                    vec![accepted.revision.content],
                    Some(text),
                )
            }
            None => (Vec::new(), Vec::new(), None),
        };
        let transformation = Transformation::new(
            TransformationKind::GenerativeEdit,
            artifact_id,
            inputs,
            intent,
            constraints,
            Vec::new(),
        )?;
        let instruction = generation_instruction(creative_instruction, accepted_text.as_deref());
        let request = ExecutionRequest::new(
            transformation.id,
            CapabilityId::new(TEXT_GENERATE_CAPABILITY)?,
            input_content,
            instruction.into_bytes(),
            TEXT_MEDIA_TYPE,
        )?;
        let ExecutedCandidate { output, receipt } =
            ExecutionCoordinator::execute(executor, &request)?;
        text_candidate(artifact_id, expected_head, transformation, receipt, output)
    }

    /// Explicitly accepts a previously executed text Candidate.
    ///
    /// # Errors
    ///
    /// Returns an error when its expected head is stale or durable publication fails.
    pub fn accept_text(&mut self, candidate: TextCandidate) -> Result<ArtifactRevision, CoreError> {
        Ok(self.store.accept_with_node(
            AcceptedCommit {
                artifact_id: candidate.artifact_id,
                expected_head: candidate.expected_head,
                transformation: candidate.transformation,
                receipt: candidate.receipt,
                output_bytes: candidate.output_text.into_bytes().into(),
                output_media_type: candidate.output_media_type,
                content_contract: Some(shape_domain::ArtifactContentContract::TextDocument(
                    candidate.content_contract,
                )),
            },
            &candidate.input_heads,
            candidate.request_node.as_ref(),
        )?)
    }

    /// Accepts a text Candidate as a newly named artifact without advancing its source.
    ///
    /// The exact candidate bytes are re-executed so the branch receives truthful,
    /// target-specific physical evidence. Acceptance remains one atomic commit.
    ///
    /// # Errors
    ///
    /// Returns an error when the source has no accepted head, its head became
    /// stale, the name is invalid, execution fails, or atomic publication fails.
    pub fn branch_text_candidate(
        &mut self,
        candidate: TextCandidate,
        artifact_name: impl Into<String>,
    ) -> Result<ArtifactRevision, CoreError> {
        let source_artifact = self.text_artifact(candidate.artifact_id)?;
        validate_expected_head(
            candidate.artifact_id,
            candidate.expected_head,
            source_artifact.accepted_revision,
        )?;
        let source_revision_id =
            candidate
                .expected_head
                .ok_or(CoreError::BranchRequiresAcceptedSource {
                    artifact_id: candidate.artifact_id,
                })?;
        let source_revision = self.store.revision(source_revision_id)?;
        let target = Artifact::new(artifact_name, ArtifactKind::TextDocument)?;
        let transformation = Transformation::new(
            TransformationKind::TextRewrite,
            target.id,
            vec![source_revision_id],
            candidate.transformation.intent,
            candidate.transformation.constraints,
            candidate.transformation.references,
        )?;
        let request = ExecutionRequest::new(
            transformation.id,
            CapabilityId::new(TEXT_LITERAL_CAPABILITY)?,
            vec![source_revision.content],
            candidate.output_text.into_bytes(),
            TEXT_MEDIA_TYPE,
        )?;
        let ExecutedCandidate { output, receipt } =
            ExecutionCoordinator::execute(&LiteralTextExecutor::new()?, &request)?;
        Ok(self.store.accept_new_artifact(NewArtifactCommit {
            artifact: target,
            expected_input_heads: vec![(candidate.artifact_id, source_revision_id)],
            transformation,
            receipt,
            output_bytes: output.bytes.into(),
            output_media_type: output.media_type,
            content_contract: None,
        })?)
    }

    fn propose_literal_text(
        &self,
        artifact_id: ArtifactId,
        expected_head: Option<RevisionId>,
        kind: TransformationKind,
        replacement_text: String,
        intent: IntentSpec,
        constraints: Vec<Constraint>,
    ) -> Result<TextCandidate, CoreError> {
        let artifact = self.text_artifact(artifact_id)?;
        validate_expected_head(artifact_id, expected_head, artifact.accepted_revision)?;
        let (inputs, input_content) = match expected_head {
            Some(revision_id) => {
                let revision = self.store.accepted_revision(artifact_id)?.ok_or(
                    CoreError::MissingAcceptedRevision {
                        artifact_id,
                        revision_id,
                    },
                )?;
                debug_assert_eq!(revision.id, revision_id);
                (vec![revision_id], vec![revision.content])
            }
            None => (Vec::new(), Vec::new()),
        };
        let transformation =
            Transformation::new(kind, artifact_id, inputs, intent, constraints, Vec::new())?;
        let request = ExecutionRequest::new(
            transformation.id,
            CapabilityId::new(TEXT_LITERAL_CAPABILITY)?,
            input_content,
            replacement_text.into_bytes(),
            TEXT_MEDIA_TYPE,
        )?;
        let ExecutedCandidate { output, receipt } =
            ExecutionCoordinator::execute(&LiteralTextExecutor::new()?, &request)?;
        text_candidate(artifact_id, expected_head, transformation, receipt, output)
    }

    fn text_artifact(&self, artifact_id: ArtifactId) -> Result<Artifact, CoreError> {
        let artifact = self
            .store
            .artifact(artifact_id)?
            .ok_or(shape_store::StoreError::UnknownArtifact(artifact_id))?;
        if artifact.kind != ArtifactKind::TextDocument {
            return Err(CoreError::InvalidTextArtifact { artifact_id });
        }
        Ok(artifact)
    }
}

fn validate_expected_head(
    artifact_id: ArtifactId,
    expected: Option<RevisionId>,
    actual: Option<RevisionId>,
) -> Result<(), CoreError> {
    if actual != expected {
        return Err(CoreError::StaleCandidate {
            artifact_id,
            expected,
            actual,
        });
    }
    Ok(())
}

fn text_candidate(
    artifact_id: ArtifactId,
    expected_head: Option<RevisionId>,
    transformation: Transformation,
    receipt: ExecutionReceipt,
    output: ExecutionOutput,
) -> Result<TextCandidate, CoreError> {
    let output_text =
        String::from_utf8(output.bytes).map_err(|_| CoreError::InvalidTextCandidate)?;
    Ok(TextCandidate {
        artifact_id,
        expected_head,
        transformation,
        receipt,
        output_text,
        output_media_type: output.media_type,
        content_contract: shape_domain::TextDocumentContract::Plain,
        input_heads: Vec::new(),
        request_node: None,
    })
}

fn generation_instruction(prompt: &str, accepted_text: Option<&str>) -> String {
    match accepted_text {
        Some(text) => format!(
            "Return only the complete replacement text.\n\nCreative instruction:\n{prompt}\n\nCurrent accepted text:\n{text}"
        ),
        None => {
            format!("Return only the complete text to create.\n\nCreative instruction:\n{prompt}")
        }
    }
}

fn transform_instruction(parameters: &TextTransformParameters) -> String {
    format!(
        "Creative text transform mode: {}\n{}",
        parameters.mode.as_str(),
        parameters.instruction
    )
}

#[derive(Debug)]
struct LiteralTextExecutor {
    identity: ExecutorIdentity,
}

impl LiteralTextExecutor {
    fn new() -> Result<Self, shape_execution::ExecutionError> {
        Ok(Self {
            identity: ExecutorIdentity::new(
                "shape.builtin.literal-text",
                env!("CARGO_PKG_VERSION"),
                BUILTIN_CONTRACT_REVISION,
            )?,
        })
    }
}

impl Executor for LiteralTextExecutor {
    fn identity(&self) -> &ExecutorIdentity {
        &self.identity
    }

    fn supports(&self, capability: &CapabilityId) -> bool {
        capability.as_str() == TEXT_LITERAL_CAPABILITY
    }

    fn execute(&self, request: &ExecutionRequest) -> Result<ExecutionOutput, ExecutionFailure> {
        let text = std::str::from_utf8(&request.instruction).map_err(|_| {
            ExecutionFailure::new(
                "invalid_utf8",
                "text candidate instruction is not valid UTF-8",
                false,
            )
        })?;
        Ok(ExecutionOutput {
            bytes: text.as_bytes().to_vec(),
            media_type: request.output_media_type.clone(),
            executor_job_id: None,
            external_provenance: None,
            content_contract: None,
        })
    }
}

#[cfg(test)]
mod tests;
