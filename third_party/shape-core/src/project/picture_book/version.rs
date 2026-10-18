//! Revisioned book composition. Materials remain immutable references; the shelf owns proposals.
use super::{
    PictureBookError, PictureBookExport, PictureBookRequest, ShapeProject, invalid,
    validate_request,
};
use crate::CoreError;
use serde_json::{Value, json};
use shape_domain::{
    Artifact, ArtifactId, ArtifactKind, ArtifactRevision, ContentDigest, IntentSpec,
    PICTURE_BOOK_MEDIA_TYPE, PictureBookDocument, RevisionId, Transformation, TransformationKind,
    WorkingInput, WorkingOperatorDraft,
};
use shape_execution::{
    CapabilityId, ExecutionCoordinator, ExecutionFailure, ExecutionOutput, ExecutionReceipt,
    ExecutionRequest, Executor, ExecutorIdentity,
};
use shape_store::{AcceptedCommit, NewArtifactCommit};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct PictureBookCandidate {
    artifact: Artifact,
    expected_head: Option<RevisionId>,
    existing_target: bool,
    expected_node: Option<WorkingOperatorDraft>,
    document: PictureBookDocument,
    transformation: Transformation,
    receipt: ExecutionReceipt,
    bytes: Vec<u8>,
}
impl PictureBookCandidate {
    pub const fn artifact_id(&self) -> ArtifactId {
        self.artifact.id
    }
    pub fn name(&self) -> &str {
        &self.artifact.name
    }
    pub const fn expected_head(&self) -> Option<RevisionId> {
        self.expected_head
    }
    pub const fn receipt(&self) -> &ExecutionReceipt {
        &self.receipt
    }
    pub const fn composition(&self) -> &PictureBookRequest {
        &self.document.composition
    }
    pub const fn request_node(&self) -> Option<&WorkingOperatorDraft> {
        self.expected_node.as_ref()
    }
    /// Stable review identity excludes random execution/Artifact identities for a new book.
    pub fn review_digest(&self) -> String {
        let payload = json!({"composition":self.document,"expected_head":self.expected_head,
            "artifact_id":self.existing_target.then_some(self.artifact.id)})
        .to_string();
        ContentDigest::from_bytes(payload.as_bytes()).to_string()
    }
}

impl ShapeProject {
    /// Adopts a manually authored script as a new text material, retaining its source.
    /// # Errors
    /// Rejects invalid script syntax, changed source heads, or persistence failure.
    pub fn adopt_picture_book_script(
        &mut self,
        name: &str,
        source: Option<WorkingInput>,
        text: &str,
    ) -> Result<ArtifactRevision, PictureBookError> {
        super::script::parse(text, 64)?;
        if let Some(source) = source {
            let candidate = self.propose_text(
                source.artifact_id,
                Some(source.revision_id),
                text,
                IntentSpec::new("Revise a picture-book script").map_err(CoreError::from)?,
                vec![],
            )?;
            Ok(self.branch_text_candidate(candidate, name)?)
        } else {
            Ok(self.create_text_document(name, text)?)
        }
    }

    /// Validates every pinned material, including scripts, without advancing history.
    /// # Errors
    /// Rejects invalid page roles, stale heads, missing/corrupt materials and oversized input.
    pub fn propose_picture_book(
        &self,
        target: Option<WorkingInput>,
        request: PictureBookRequest,
    ) -> Result<PictureBookCandidate, PictureBookError> {
        let artifact = if let Some(target) = target {
            self.verify_book_head(target)?;
            let artifact = self
                .store
                .artifact(target.artifact_id)
                .map_err(CoreError::from)?
                .ok_or_else(|| invalid("book does not exist"))?;
            if artifact.kind != ArtifactKind::PictureBook {
                return Err(invalid("target is not a picture book"));
            }
            artifact
        } else {
            Artifact::new(&request.name, ArtifactKind::PictureBook).map_err(CoreError::from)?
        };
        self.prepare_picture_book(
            artifact,
            target.map(|t| t.revision_id),
            target.is_some(),
            None,
            request,
        )
    }

    pub(super) fn prepare_picture_book(
        &self,
        artifact: Artifact,
        expected_head: Option<RevisionId>,
        existing_target: bool,
        expected_node: Option<WorkingOperatorDraft>,
        request: PictureBookRequest,
    ) -> Result<PictureBookCandidate, PictureBookError> {
        validate_request(&request)?;
        // Reuse media, exact-byte and multilingual role checks for both direct and saved-draft proposals.
        let composition = self.compose_picture_book(&request, true)?;
        let inputs = request.inputs();
        let transformation = Transformation::new(
            TransformationKind::Composite,
            artifact.id,
            inputs.iter().map(|i| i.revision_id).collect(),
            IntentSpec::new("Compose an ordered multilingual picture book")
                .map_err(CoreError::from)?,
            vec![],
            vec![],
        )
        .map_err(CoreError::from)?;
        let document = PictureBookDocument::new(request);
        let execution = ExecutionRequest::new(
            transformation.id,
            CapabilityId::new("book.compose").map_err(CoreError::from)?,
            inputs
                .iter()
                .map(|source| {
                    composition
                        .inputs
                        .iter()
                        .find(|p| p.source == *source)
                        .map(|p| p.content.clone())
                        .ok_or_else(|| invalid("composition is missing a declared material"))
                })
                .collect::<Result<Vec<_>, PictureBookError>>()?,
            serde_json::to_vec(&document)?,
            PICTURE_BOOK_MEDIA_TYPE,
        )
        .map_err(CoreError::from)?;
        let executor = BookCompositionExecutor {
            identity: ExecutorIdentity::new(
                "shape.builtin.book-composition",
                env!("CARGO_PKG_VERSION"),
                "20261004.1",
            )
            .map_err(CoreError::from)?,
        };
        let result =
            ExecutionCoordinator::execute(&executor, &execution).map_err(CoreError::from)?;
        Ok(PictureBookCandidate {
            expected_head,
            existing_target,
            expected_node,
            artifact,
            document,
            transformation,
            receipt: result.receipt,
            bytes: result.output.bytes,
        })
    }

    /// Explicit acceptance rechecks all source heads and the target in one store transaction.
    /// # Errors
    /// Rejects stale output/source heads or invalid durable publication; the caller can retain its proposal.
    pub fn accept_picture_book(
        &mut self,
        candidate: PictureBookCandidate,
    ) -> Result<ArtifactRevision, PictureBookError> {
        let inputs = candidate
            .document
            .composition
            .inputs()
            .iter()
            .map(|i| (i.artifact_id, i.revision_id))
            .collect::<Vec<_>>();
        let expected_node = candidate
            .expected_node
            .as_ref()
            .map(|node| {
                self.current_book_candidate_node(
                    candidate.artifact.id,
                    node,
                    &candidate.document.composition,
                )
            })
            .transpose()?;
        let revision = if candidate.existing_target {
            self.store.accept_with_node(
                AcceptedCommit {
                    artifact_id: candidate.artifact.id,
                    expected_head: candidate.expected_head,
                    transformation: candidate.transformation,
                    receipt: candidate.receipt,
                    output_bytes: candidate.bytes.into(),
                    output_media_type: PICTURE_BOOK_MEDIA_TYPE.into(),
                    content_contract: None,
                },
                &inputs,
                expected_node.as_ref(),
            )
        } else {
            self.store.accept_new_artifact(NewArtifactCommit {
                artifact: candidate.artifact,
                expected_input_heads: inputs,
                transformation: candidate.transformation,
                receipt: candidate.receipt,
                output_bytes: candidate.bytes.into(),
                output_media_type: PICTURE_BOOK_MEDIA_TYPE.into(),
                content_contract: None,
            })
        }
        .map_err(CoreError::from)?;
        Ok(revision)
    }

    /// Reads the exact accepted book version, retaining old materials after their heads advance.
    /// # Errors
    /// Rejects mismatched book identities, malformed manifests or corrupt CAS bytes.
    pub fn read_picture_book(
        &self,
        source: WorkingInput,
    ) -> Result<PictureBookRequest, PictureBookError> {
        let artifact = self
            .store
            .artifact(source.artifact_id)
            .map_err(CoreError::from)?
            .ok_or_else(|| invalid("book does not exist"))?;
        let content = self.read_revision_content(source.revision_id)?;
        if artifact.kind != ArtifactKind::PictureBook
            || content.revision.artifact_id != source.artifact_id
            || content.revision.content.media_type != PICTURE_BOOK_MEDIA_TYPE
        {
            return Err(invalid("revision is not this picture book"));
        }
        let document: PictureBookDocument = serde_json::from_slice(&content.bytes)?;
        document.validate().map_err(invalid)?;
        Ok(document.composition)
    }

    /// Source-independent preview data; missing language blocks are explicit, never borrowed.
    /// # Errors
    /// Rejects bad references, content identities, media types and ambiguous language page roles.
    pub fn preview_picture_book(
        &self,
        request: &PictureBookRequest,
    ) -> Result<Value, PictureBookError> {
        validate_request(request)?;
        let composition = self.compose_picture_book(request, false)?;
        let heads = self.snapshot()?.artifacts;
        let stale = request
            .inputs()
            .into_iter()
            .filter(|i| {
                !heads
                    .iter()
                    .any(|a| a.id == i.artifact_id && a.accepted_revision == Some(i.revision_id))
            })
            .collect::<Vec<_>>();
        let mut missing = Vec::new();
        for (language, parsed) in composition.origin["parsed_scripts"]
            .as_object()
            .ok_or_else(|| invalid("composition is missing parsed scripts"))?
        {
            for index in 0..request.pages.len() {
                if !request.pages[index].print_only()
                    && parsed["page_text"].get(index.to_string()).is_none()
                {
                    missing.push(json!({"language":language,"page":index}));
                }
            }
        }
        Ok(
            json!({"schema":"shape.picture-book-preview/1","composition":request,
            "scripts":composition.project["languages"],"parsed_scripts":composition.origin["parsed_scripts"],
            "stale_inputs":stale,"missing_text":missing}),
        )
    }

    /// Materialization keeps native export useful even if a source now has a newer accepted version.
    /// # Errors
    /// Rejects invalid books, existing destinations or bad referenced bytes.
    pub fn export_accepted_picture_book(
        &self,
        source: WorkingInput,
        destination: &Path,
    ) -> Result<PictureBookExport, PictureBookError> {
        let request = self.read_picture_book(source)?;
        self.export_book_snapshot(&request, destination, false)
    }
}

#[derive(Debug)]
struct BookCompositionExecutor {
    identity: ExecutorIdentity,
}
impl Executor for BookCompositionExecutor {
    fn identity(&self) -> &ExecutorIdentity {
        &self.identity
    }
    fn supports(&self, capability: &CapabilityId) -> bool {
        capability.as_str() == "book.compose"
    }
    fn execute(&self, request: &ExecutionRequest) -> Result<ExecutionOutput, ExecutionFailure> {
        let document: PictureBookDocument = serde_json::from_slice(&request.instruction)
            .map_err(|_| ExecutionFailure::new("invalid_book", "invalid book manifest", false))?;
        document
            .validate()
            .map_err(|e| ExecutionFailure::new("invalid_book", e, false))?;
        Ok(ExecutionOutput {
            bytes: request.instruction.clone(),
            media_type: PICTURE_BOOK_MEDIA_TYPE.into(),
            executor_job_id: None,
            external_provenance: None,
            content_contract: None,
        })
    }
}
