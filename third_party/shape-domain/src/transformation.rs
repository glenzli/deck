//! Creative meaning, preservation constraints, and typed references.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    AiImageGenerateParameters, ArtifactId, DomainError, RasterCrop, RasterDropShadow,
    RasterGaussianBlur, RasterResize, RasterTransform, RasterUnsharpMask, RevisionId,
    SpeechSynthesisOperation, TransformationId,
};

const MAX_INTENT_BYTES: usize = 4_096;
const MAX_CONSTRAINT_BYTES: usize = 1_024;
const MAX_COMPONENT_TARGET_BYTES: usize = 256;
const MAX_REFERENCE_LABEL_BYTES: usize = 160;
const MAX_TRANSFORMATION_ITEMS: usize = 64;

/// Stable creative operation family, independent of its executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransformationKind {
    /// Bring existing material into Shape ownership.
    Import,
    /// Change structured text while preserving declared constraints.
    TextRewrite,
    /// Deterministic image or document operation.
    DeterministicEdit,
    /// Non-deterministic or provider-backed creative edit.
    GenerativeEdit,
    /// Combine several artifact revisions.
    Composite,
    /// Re-import a snapshot changed in an external application.
    ExternalRoundTrip,
}

/// Typed semantic operation parameters for deterministic transformations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", content = "parameters", rename_all = "snake_case")]
pub enum TransformationOperation {
    MaterialEdit(crate::MaterialEditOperation),
    RasterCrop(RasterCrop),
    RasterResize(RasterResize),
    RasterTransform(RasterTransform),
    RasterGaussianBlur(RasterGaussianBlur),
    RasterDropShadow(RasterDropShadow),
    RasterUnsharpMask(RasterUnsharpMask),
    AudioSpeechSynthesis(SpeechSynthesisOperation),
    AiImageGenerate(AiImageGenerateParameters),
    AudioGenerate(crate::SoundGenerationOperation),
}

/// Bounded human-authored creative intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IntentSpec(String);

impl IntentSpec {
    /// Creates a validated intent summary.
    ///
    /// # Errors
    ///
    /// Returns an error when the intent is empty or too large.
    pub fn new(summary: impl Into<String>) -> Result<Self, DomainError> {
        let summary = summary.into();
        if summary.is_empty() || summary.len() > MAX_INTENT_BYTES {
            return Err(DomainError::InvalidIntent {
                max_bytes: MAX_INTENT_BYTES,
            });
        }
        Ok(Self(summary))
    }

    /// Returns the user-visible intent summary.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Semantic preservation or output requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintKind {
    PreserveIdentity,
    PreserveGeometry,
    PreserveContent,
    PreserveTiming,
    RegionScope,
    OutputContract,
    Avoid,
}

/// Whether violating a constraint rejects or merely ranks a candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintStrength {
    Hard,
    Preferred,
    Advisory,
}

/// One explicit creative constraint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Constraint {
    pub kind: ConstraintKind,
    pub strength: ConstraintStrength,
    pub value: String,
    pub target_component: Option<String>,
}

impl Constraint {
    /// Creates a validated constraint.
    ///
    /// # Errors
    ///
    /// Returns an error when its value or component target is not bounded.
    pub fn new(
        kind: ConstraintKind,
        strength: ConstraintStrength,
        value: impl Into<String>,
        target_component: Option<String>,
    ) -> Result<Self, DomainError> {
        let value = value.into();
        if value.is_empty() || value.len() > MAX_CONSTRAINT_BYTES {
            return Err(DomainError::InvalidConstraint {
                max_bytes: MAX_CONSTRAINT_BYTES,
            });
        }
        if target_component
            .as_ref()
            .is_some_and(|target| target.is_empty() || target.len() > MAX_COMPONENT_TARGET_BYTES)
        {
            return Err(DomainError::InvalidComponentTarget {
                max_bytes: MAX_COMPONENT_TARGET_BYTES,
            });
        }
        Ok(Self {
            kind,
            strength,
            value,
            target_component,
        })
    }
}

/// Meaning of a creative reference edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceRole {
    Identity,
    Style,
    Composition,
    Palette,
    Lighting,
    Voice,
    Motion,
    Negative,
}

/// One typed reference from a source revision into a transformation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceBinding {
    pub revision_id: RevisionId,
    pub role: ReferenceRole,
    pub label: String,
}

impl ReferenceBinding {
    /// Creates a reference edge with a bounded user-facing label.
    ///
    /// # Errors
    ///
    /// Returns an error when the label is empty or too large.
    pub fn new(
        revision_id: RevisionId,
        role: ReferenceRole,
        label: impl Into<String>,
    ) -> Result<Self, DomainError> {
        let label = label.into();
        if label.is_empty() || label.len() > MAX_REFERENCE_LABEL_BYTES {
            return Err(DomainError::InvalidReferenceLabel {
                max_bytes: MAX_REFERENCE_LABEL_BYTES,
            });
        }
        Ok(Self {
            revision_id,
            role,
            label,
        })
    }
}

/// Semantic explanation for why input revisions produce a new artifact state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transformation {
    pub id: TransformationId,
    pub kind: TransformationKind,
    pub target_artifact_id: ArtifactId,
    pub inputs: Vec<RevisionId>,
    pub intent: IntentSpec,
    pub constraints: Vec<Constraint>,
    pub references: Vec<ReferenceBinding>,
    /// Exact authored operation when natural-language intent alone is insufficient.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<TransformationOperation>,
}

impl Transformation {
    /// Creates a validated creative transformation.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized collections or duplicate input revisions.
    pub fn new(
        kind: TransformationKind,
        target_artifact_id: ArtifactId,
        inputs: Vec<RevisionId>,
        intent: IntentSpec,
        constraints: Vec<Constraint>,
        references: Vec<ReferenceBinding>,
    ) -> Result<Self, DomainError> {
        Self::new_with_operation(
            kind,
            target_artifact_id,
            inputs,
            intent,
            constraints,
            references,
            None,
        )
    }

    /// Creates a transformation with an exact typed operation.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized collections, duplicate inputs, or an
    /// operation attached to a non-deterministic transformation family.
    pub fn new_with_operation(
        kind: TransformationKind,
        target_artifact_id: ArtifactId,
        inputs: Vec<RevisionId>,
        intent: IntentSpec,
        constraints: Vec<Constraint>,
        references: Vec<ReferenceBinding>,
        operation: Option<TransformationOperation>,
    ) -> Result<Self, DomainError> {
        validate_size("transformation inputs", inputs.len())?;
        validate_size("transformation constraints", constraints.len())?;
        validate_size("transformation references", references.len())?;
        let unique_inputs = inputs.iter().copied().collect::<HashSet<_>>();
        if unique_inputs.len() != inputs.len() {
            return Err(DomainError::DuplicateTransformationInput);
        }
        let operation_matches_kind = match &operation {
            None => true,
            Some(TransformationOperation::MaterialEdit(operation)) => {
                operation.validate()?;
                kind == TransformationKind::ExternalRoundTrip
                    && inputs == [operation.source.revision_id]
                    && operation.source.artifact_id != target_artifact_id
            }
            Some(
                TransformationOperation::RasterCrop(_)
                | TransformationOperation::RasterResize(_)
                | TransformationOperation::RasterTransform(_)
                | TransformationOperation::RasterGaussianBlur(_)
                | TransformationOperation::RasterDropShadow(_)
                | TransformationOperation::RasterUnsharpMask(_),
            ) => kind == TransformationKind::DeterministicEdit,
            Some(TransformationOperation::AudioSpeechSynthesis(_)) => {
                kind == TransformationKind::GenerativeEdit
            }
            Some(TransformationOperation::AudioGenerate(parameters)) => {
                parameters.validate()?;
                kind == TransformationKind::GenerativeEdit
                    && inputs.is_empty()
                    && references.is_empty()
            }
            Some(TransformationOperation::AiImageGenerate(_)) => {
                kind == TransformationKind::GenerativeEdit && inputs.is_empty()
            }
        };
        if !operation_matches_kind {
            return Err(DomainError::InvalidTransformationOperation);
        }
        Ok(Self {
            id: TransformationId::new(),
            kind,
            target_artifact_id,
            inputs,
            intent,
            constraints,
            references,
            operation,
        })
    }
}

fn validate_size(collection: &'static str, actual: usize) -> Result<(), DomainError> {
    if actual > MAX_TRANSFORMATION_ITEMS {
        return Err(DomainError::CollectionTooLarge {
            collection,
            actual,
            maximum: MAX_TRANSFORMATION_ITEMS,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
