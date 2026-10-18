//! Stable artifact identity and immutable accepted revisions.

use serde::{Deserialize, Serialize};

use crate::{
    ArtifactId, AudioValueContract, ContentRef, DomainError, ImageRasterContract, RevisionId,
    TransformationId,
};

const MAX_ARTIFACT_NAME_BYTES: usize = 120;
const MAX_REVISION_PARENTS: usize = 8;

/// The media-specific content schema owned by an artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    /// Structured text blocks and spans.
    TextDocument,
    /// Ordered multilingual book composition over pinned accepted materials.
    PictureBook,
    /// Card text, artwork bindings and authored material-kit direction.
    CardKit,
    /// One immutable raster with an explicit image contract.
    ImageRaster,
    /// A structured layer/mask composition.
    ImageComposite,
    /// A named set of typed creative references.
    ReferenceSet,
    /// One immutable materialized audio value.
    AudioClip,
}

/// Media-specific contract carried by one immutable accepted revision.
///
/// The payload remains content-addressed separately; this value makes its
/// interpretation explicit without teaching the generic CAS about media.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "contract", rename_all = "snake_case")]
pub enum ArtifactContentContract {
    TextDocument(crate::TextDocumentContract),
    ImageRaster(ImageRasterContract),
    AudioClip(AudioValueContract),
}

/// One stable user-recognizable creative object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    /// Persistent artifact identity.
    pub id: ArtifactId,
    /// User-visible name.
    pub name: String,
    /// Media schema family.
    pub kind: ArtifactKind,
    /// Current accepted immutable state, if any.
    pub accepted_revision: Option<RevisionId>,
}

impl Artifact {
    /// Creates an artifact without an accepted state.
    ///
    /// # Errors
    ///
    /// Returns an error when the name is empty or exceeds the portable limit.
    pub fn new(name: impl Into<String>, kind: ArtifactKind) -> Result<Self, DomainError> {
        let name = name.into();
        if name.is_empty() || name.len() > MAX_ARTIFACT_NAME_BYTES {
            return Err(DomainError::InvalidArtifactName {
                max_bytes: MAX_ARTIFACT_NAME_BYTES,
            });
        }
        Ok(Self {
            id: ArtifactId::new(),
            name,
            kind,
            accepted_revision: None,
        })
    }
}

/// One immutable accepted state of an [`Artifact`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRevision {
    /// Persistent revision identity.
    pub id: RevisionId,
    /// Stable artifact this state belongs to.
    pub artifact_id: ArtifactId,
    /// Previous accepted state(s); multiple parents are reserved for explicit merge semantics.
    pub parents: Vec<RevisionId>,
    /// Durable content bytes.
    pub content: ContentRef,
    /// Typed interpretation of media payload bytes. Legacy text revisions omit it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_contract: Option<ArtifactContentContract>,
    /// Creative transformation that produced this state.
    pub transformation_id: TransformationId,
    /// Wall-clock evidence for display and audit, never identity.
    pub created_at_unix_ms: u64,
}

impl ArtifactRevision {
    /// Creates a validated immutable revision value.
    ///
    /// # Errors
    ///
    /// Returns an error when more parent revisions are supplied than the contract permits.
    pub fn new(
        artifact_id: ArtifactId,
        parents: Vec<RevisionId>,
        content: ContentRef,
        transformation_id: TransformationId,
        created_at_unix_ms: u64,
    ) -> Result<Self, DomainError> {
        Self::new_with_content_contract(
            artifact_id,
            parents,
            content,
            None,
            transformation_id,
            created_at_unix_ms,
        )
    }

    /// Creates a validated immutable revision with an explicit media contract.
    ///
    /// # Errors
    ///
    /// Returns an error when more parent revisions are supplied than the contract permits.
    pub fn new_with_content_contract(
        artifact_id: ArtifactId,
        parents: Vec<RevisionId>,
        content: ContentRef,
        content_contract: Option<ArtifactContentContract>,
        transformation_id: TransformationId,
        created_at_unix_ms: u64,
    ) -> Result<Self, DomainError> {
        if parents.len() > MAX_REVISION_PARENTS {
            return Err(DomainError::CollectionTooLarge {
                collection: "revision parents",
                actual: parents.len(),
                maximum: MAX_REVISION_PARENTS,
            });
        }
        Ok(Self {
            id: RevisionId::new(),
            artifact_id,
            parents,
            content,
            content_contract,
            transformation_id,
            created_at_unix_ms,
        })
    }
}

#[cfg(test)]
mod tests;
