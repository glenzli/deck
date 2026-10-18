//! Recoverable card-kit authoring over accepted materials, with explicit reviewed acceptance.
mod draft;
mod export;
mod layout;
mod preview;
mod print;

pub use draft::CardKitDraft;
pub use export::CardKitExport;
pub use print::{CardPrintOptions, CardPrintPaper, CardPrintPlan, CardPrintRequestOptions};

use std::{path::PathBuf, sync::Arc};

use serde::Deserialize;
use serde_json::{Value, json};
use shape_domain::{
    ArtifactContentContract, ArtifactId, ArtifactKind, ArtifactRevision, CARD_KIT_MEDIA_TYPE,
    CardKitDocument, CardKitRequest, ContentDigest, ContentRef, IntentSpec, RevisionId,
    Transformation, TransformationKind, WorkingInput, WorkingOperatorDraft,
};
use shape_execution::{
    CapabilityId, ExecutionCoordinator, ExecutionFailure, ExecutionOutput, ExecutionReceipt,
    ExecutionRequest, Executor, ExecutorIdentity,
};
use shape_store::AcceptedCommit;
use thiserror::Error;

use super::ShapeProject;
use crate::CoreError;

#[derive(Debug, Error)]
pub enum CardKitError {
    #[error("invalid card kit: {0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error("card kit JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("card kit file operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("card kit archive failed: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{code}: {message}")]
    PrintSetting {
        code: &'static str,
        message: &'static str,
    },
    #[error(
        "card_print.text_overflow: card {card_id} text does not fit at readable print sizes; shorten its copy or use larger cards"
    )]
    PrintTextOverflow { card_id: String },
    #[error(
        "card_render.cover_limit: card {card_id} cover expansion exceeds supported rendering bounds; use full image (contain) or different artwork proportions"
    )]
    ImageCropLimit { card_id: String },
}
fn invalid(message: &'static str) -> CardKitError {
    CardKitError::Invalid(message)
}

/// Both public adapters consume this owning use case; no adapter may bypass review or draft CAS.
#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum CardKitAction {
    DraftNew {
        composition: CardKitRequest,
    },
    DraftList,
    DraftLoad {
        artifact_id: ArtifactId,
    },
    DraftSave {
        artifact_id: ArtifactId,
        draft_digest: String,
        composition: CardKitRequest,
    },
    DraftDiscard {
        artifact_id: ArtifactId,
        draft_digest: String,
    },
    Preview {
        artifact_id: ArtifactId,
        draft_digest: String,
        #[serde(default)]
        card_id: Option<String>,
    },
    Accept {
        artifact_id: ArtifactId,
        draft_digest: String,
        review_digest: String,
    },
    Read {
        source: WorkingInput,
        #[serde(default)]
        card_id: Option<String>,
    },
    Export {
        source: WorkingInput,
        destination: PathBuf,
    },
    PrintPlan {
        source: WorkingInput,
        #[serde(default)]
        options: CardPrintRequestOptions,
    },
}

#[derive(Debug, Clone)]
pub struct CardKitCandidate {
    artifact_id: ArtifactId,
    expected_head: Option<RevisionId>,
    node: WorkingOperatorDraft,
    document: CardKitDocument,
    transformation: Transformation,
    receipt: ExecutionReceipt,
    bytes: Arc<[u8]>,
    review_digest: String,
}
impl CardKitCandidate {
    pub const fn artifact_id(&self) -> ArtifactId {
        self.artifact_id
    }
    pub const fn receipt(&self) -> &ExecutionReceipt {
        &self.receipt
    }
    pub fn review_digest(&self) -> &str {
        &self.review_digest
    }
    pub const fn composition(&self) -> &CardKitRequest {
        &self.document.composition
    }
}

#[derive(Debug)]
struct Material {
    source: WorkingInput,
    kind: ArtifactKind,
    content: ContentRef,
    bytes: Vec<u8>,
    image_dimensions: Option<(u32, u32)>,
}

impl ShapeProject {
    /// Bounded JSON channel for the real CLI and desktop consumers.
    /// # Errors
    /// Rejects unknown fields, oversized input, stale drafts/heads and invalid material content.
    pub fn card_kit_action_json(&mut self, request: &str) -> Result<String, CardKitError> {
        if request.len() > 96 * 1024 {
            return Err(invalid("card kit action exceeds 96 KiB"));
        }
        let result = self.card_kit_action(serde_json::from_str(request)?)?;
        Ok(serde_json::to_string(&result)?)
    }

    /// # Errors
    /// Every action retains the owning draft, material validation and explicit accept boundaries.
    pub fn card_kit_action(&mut self, action: CardKitAction) -> Result<Value, CardKitError> {
        match action {
            CardKitAction::DraftNew { composition } => {
                Ok(json!({"draft":self.create_card_kit_draft(&composition)?}))
            }
            CardKitAction::DraftList => Ok(json!({"drafts":self.card_kit_drafts()?})),
            CardKitAction::DraftLoad { artifact_id } => {
                Ok(json!({"draft":self.open_card_kit_draft(artifact_id)?}))
            }
            CardKitAction::DraftSave {
                artifact_id,
                draft_digest,
                composition,
            } => Ok(
                json!({"draft":self.save_card_kit_draft(artifact_id, &draft_digest, &composition)?}),
            ),
            CardKitAction::DraftDiscard {
                artifact_id,
                draft_digest,
            } => {
                self.discard_card_kit_draft(artifact_id, &draft_digest)?;
                Ok(json!({"discarded":true}))
            }
            CardKitAction::Preview {
                artifact_id,
                draft_digest,
                card_id,
            } => {
                let draft = self.checked_card_kit_draft(artifact_id, &draft_digest)?;
                let candidate = self.propose_card_kit(artifact_id, &draft_digest)?;
                Ok(
                    json!({"draft":draft,"review_digest":candidate.review_digest(),"preview":self.preview_card_kit(&draft.composition, card_id.as_deref())?}),
                )
            }
            CardKitAction::Accept {
                artifact_id,
                draft_digest,
                review_digest,
            } => {
                let candidate = self.propose_card_kit(artifact_id, &draft_digest)?;
                if candidate.review_digest() != review_digest {
                    return Err(invalid(
                        "review digest does not match this exact saved card kit",
                    ));
                }
                let revision = self.accept_card_kit(candidate)?;
                Ok(
                    json!({"source":WorkingInput {artifact_id:revision.artifact_id,revision_id:revision.id},"draft":self.open_card_kit_draft(artifact_id)?}),
                )
            }
            CardKitAction::Read { source, card_id } => {
                let composition = self.read_card_kit(source)?;
                Ok(
                    json!({"source":source,"preview":self.preview_card_kit(&composition, card_id.as_deref())?,"print_defaults":print::default_options(&composition),"composition":composition}),
                )
            }
            CardKitAction::Export {
                source,
                destination,
            } => Ok(serde_json::to_value(
                self.export_card_kit(source, &destination)?,
            )?),
            CardKitAction::PrintPlan { source, options } => Ok(json!({
                "print_plan": self.plan_card_print_request(source, &options)?
            })),
        }
    }

    /// Produces a transient composition candidate from the exact saved draft.
    /// # Errors
    /// Rejects changed drafts, stale material heads, wrong media and corrupt bytes.
    pub fn propose_card_kit(
        &self,
        artifact_id: ArtifactId,
        draft_digest: &str,
    ) -> Result<CardKitCandidate, CardKitError> {
        let draft = self.checked_card_kit_draft(artifact_id, draft_digest)?;
        let materials = self.card_kit_materials(&draft.composition, true)?;
        let graph = self
            .store
            .artifact_working_graph(artifact_id)
            .map_err(CoreError::from)?
            .ok_or_else(|| invalid("card kit draft is missing"))?;
        if ContentDigest::from_bytes(&serde_json::to_vec(&graph)?).to_string() != draft_digest {
            return Err(invalid(
                "card kit draft changed; review the current saved draft",
            ));
        }
        let node = draft::node(&graph)?.clone();
        let transformation = Transformation::new(
            TransformationKind::Composite,
            artifact_id,
            draft
                .composition
                .inputs()
                .iter()
                .map(|s| s.revision_id)
                .collect(),
            IntentSpec::new("Compose card text and accepted artwork into a card material kit")
                .map_err(CoreError::from)?,
            vec![],
            vec![],
        )
        .map_err(CoreError::from)?;
        let document = CardKitDocument::new(draft.composition);
        let execution = ExecutionRequest::new(
            transformation.id,
            CapabilityId::new("card_kit.compose").map_err(CoreError::from)?,
            materials.iter().map(|m| m.content.clone()).collect(),
            serde_json::to_vec(&document)?,
            CARD_KIT_MEDIA_TYPE,
        )
        .map_err(CoreError::from)?;
        let executor = CardKitExecutor {
            identity: ExecutorIdentity::new(
                "shape.builtin.card-kit-composition",
                env!("CARGO_PKG_VERSION"),
                "20261018.1",
            )
            .map_err(CoreError::from)?,
        };
        let result =
            ExecutionCoordinator::execute(&executor, &execution).map_err(CoreError::from)?;
        let review_digest = ContentDigest::from_bytes(&serde_json::to_vec(
            &json!({"draft_digest":draft_digest,"document":document}),
        )?)
        .to_string();
        Ok(CardKitCandidate {
            artifact_id,
            expected_head: draft.expected_head,
            node,
            document,
            transformation,
            receipt: result.receipt,
            bytes: result.output.bytes.into(),
            review_digest,
        })
    }

    /// Explicit acceptance checks draft identity, target head and every source head in one transaction.
    /// # Errors
    /// Rejects stale candidates without replacing accepted history or the saved draft.
    pub fn accept_card_kit(
        &mut self,
        candidate: CardKitCandidate,
    ) -> Result<ArtifactRevision, CardKitError> {
        let inputs = candidate
            .document
            .composition
            .inputs()
            .iter()
            .map(|s| (s.artifact_id, s.revision_id))
            .collect::<Vec<_>>();
        Ok(self
            .store
            .accept_with_node(
                AcceptedCommit {
                    artifact_id: candidate.artifact_id,
                    expected_head: candidate.expected_head,
                    transformation: candidate.transformation,
                    receipt: candidate.receipt,
                    output_bytes: candidate.bytes,
                    output_media_type: CARD_KIT_MEDIA_TYPE.into(),
                    content_contract: None,
                },
                &inputs,
                Some(&candidate.node),
            )
            .map_err(CoreError::from)?)
    }

    /// Reads a pinned historical accepted kit. A newer head never silently replaces the pin.
    /// # Errors
    /// Rejects wrong artifact/revision identities, unsupported documents and corrupt objects.
    pub fn read_card_kit(&self, source: WorkingInput) -> Result<CardKitRequest, CardKitError> {
        let artifact = self
            .store
            .artifact(source.artifact_id)
            .map_err(CoreError::from)?
            .ok_or_else(|| invalid("card kit does not exist"))?;
        let revision = self.revision(source.revision_id)?;
        if artifact.kind != ArtifactKind::CardKit
            || revision.artifact_id != source.artifact_id
            || revision.content.media_type != CARD_KIT_MEDIA_TYPE
            || revision.content.byte_length > (shape_domain::CARD_KIT_MAX_BYTES + 1024) as u64
        {
            return Err(invalid("revision is not this card kit"));
        }
        let content = self.read_revision_content(source.revision_id)?;
        let document: CardKitDocument = serde_json::from_slice(&content.bytes)?;
        document.validate().map_err(invalid)?;
        Ok(document.composition)
    }

    fn card_kit_materials(
        &self,
        request: &CardKitRequest,
        require_current: bool,
    ) -> Result<Vec<Material>, CardKitError> {
        request.validate().map_err(invalid)?;
        let mut materials = Vec::new();
        let mut remaining = 128 * 1024 * 1024_u64;
        for source in request.inputs() {
            let kind = if request.cards.iter().any(|c| c.artwork == Some(source)) {
                ArtifactKind::ImageRaster
            } else {
                ArtifactKind::TextDocument
            };
            if request.cards.iter().any(|c| c.artwork == Some(source))
                && request
                    .cards
                    .iter()
                    .any(|c| c.text_material == Some(source))
            {
                return Err(invalid(
                    "one material cannot be both card artwork and rules text",
                ));
            }
            let artifact = self
                .store
                .artifact(source.artifact_id)
                .map_err(CoreError::from)?
                .ok_or_else(|| invalid("card material does not exist"))?;
            let revision = self.revision(source.revision_id)?;
            if artifact.kind != kind
                || revision.artifact_id != source.artifact_id
                || (kind == ArtifactKind::ImageRaster && revision.content.media_type != "image/png")
                || (kind == ArtifactKind::TextDocument
                    && !revision.content.media_type.starts_with("text/"))
            {
                return Err(invalid("card material kind or revision does not match"));
            }
            if require_current && artifact.accepted_revision != Some(source.revision_id) {
                return Err(invalid(
                    "a card material head changed; choose its version explicitly",
                ));
            }
            let max_bytes = if kind == ArtifactKind::ImageRaster {
                20 * 1024 * 1024
            } else {
                8192
            };
            if revision.content.byte_length > max_bytes {
                return Err(invalid("card material exceeds its image or text limit"));
            }
            remaining = remaining
                .checked_sub(revision.content.byte_length)
                .ok_or_else(|| invalid("card kit materials exceed 128 MiB"))?;
            let content = self.read_revision_content(source.revision_id)?;
            let image_dimensions = if kind == ArtifactKind::ImageRaster {
                match &content.revision.content_contract {
                    Some(ArtifactContentContract::ImageRaster(raster))
                        if raster.width > 0 && raster.height > 0 =>
                    {
                        Some((raster.width, raster.height))
                    }
                    _ => {
                        return Err(invalid(
                            "card artwork requires a valid raster size contract",
                        ));
                    }
                }
            } else {
                None
            };
            if kind == ArtifactKind::TextDocument {
                let text = std::str::from_utf8(&content.bytes)
                    .map_err(|_| invalid("card rules material must be UTF-8"))?;
                if text
                    .chars()
                    .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
                {
                    return Err(invalid(
                        "card rules material contains unsupported control characters",
                    ));
                }
            }
            materials.push(Material {
                source,
                kind,
                content: content.revision.content,
                bytes: content.bytes,
                image_dimensions,
            });
        }
        Ok(materials)
    }
}

#[derive(Debug)]
struct CardKitExecutor {
    identity: ExecutorIdentity,
}
impl Executor for CardKitExecutor {
    fn identity(&self) -> &ExecutorIdentity {
        &self.identity
    }
    fn supports(&self, capability: &CapabilityId) -> bool {
        capability.as_str() == "card_kit.compose"
    }
    fn execute(&self, request: &ExecutionRequest) -> Result<ExecutionOutput, ExecutionFailure> {
        let document: CardKitDocument =
            serde_json::from_slice(&request.instruction).map_err(|_| {
                ExecutionFailure::new("invalid_card_kit", "invalid card kit manifest", false)
            })?;
        document
            .validate()
            .map_err(|e| ExecutionFailure::new("invalid_card_kit", e, false))?;
        Ok(ExecutionOutput {
            bytes: request.instruction.clone(),
            media_type: CARD_KIT_MEDIA_TYPE.into(),
            executor_job_id: None,
            external_provenance: None,
            content_contract: None,
        })
    }
}

#[cfg(test)]
mod tests;
