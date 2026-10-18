//! Portable material-editing intent, independent of external process or file paths.

use crate::{
    ArtifactKind, ContentDigest, DomainError, OperatorConfigurationSchemaId, OperatorNodeId,
    WorkingInput, WorkingOperatorConfiguration, WorkingOperatorDraft,
};
use serde::{Deserialize, Serialize};

pub const MATERIAL_EDIT_SCHEMA: &str = "shape.material-edit/1";

/// The specialist chosen by the author; this is not proof of a provider execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialEditor {
    Shadow,
    Echo,
}

impl MaterialEditor {
    #[must_use]
    pub const fn operator_type(self) -> &'static str {
        match self {
            Self::Shadow => "image.external_edit",
            Self::Echo => "audio.external_edit",
        }
    }
    #[must_use]
    pub const fn data_type(self) -> &'static str {
        match self {
            Self::Shadow => "image.raster",
            Self::Echo => "audio.clip",
        }
    }
    #[must_use]
    pub const fn artifact_kind(self) -> ArtifactKind {
        match self {
            Self::Shadow => ArtifactKind::ImageRaster,
            Self::Echo => ArtifactKind::AudioClip,
        }
    }
    #[must_use]
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Shadow => "image/png",
            Self::Echo => "audio/wav",
        }
    }
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Shadow => "Shadow",
            Self::Echo => "Echo",
        }
    }
}

/// A restart advances the session so an old exported copy cannot silently return.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialEditConfiguration {
    pub editor: MaterialEditor,
    pub session: u64,
}

impl MaterialEditConfiguration {
    /// Validates and encodes the exact persisted draft configuration.
    /// # Errors
    /// Rejects zero sessions or malformed configuration envelopes.
    pub fn encode(&self) -> Result<WorkingOperatorConfiguration, DomainError> {
        if self.session == 0 {
            return Err(DomainError::InvalidTransformationOperation);
        }
        WorkingOperatorConfiguration::new(
            OperatorConfigurationSchemaId::new(MATERIAL_EDIT_SCHEMA)?,
            serde_json::to_string(self).map_err(|_| DomainError::InvalidTransformationOperation)?,
        )
    }

    /// Checks a complete typed producer, including the bound source family.
    /// # Errors
    /// Rejects unknown schema, fields, missing input, or incompatible ports.
    pub fn from_draft(draft: &WorkingOperatorDraft) -> Result<Self, DomainError> {
        let invalid = || DomainError::InvalidTransformationOperation;
        let configuration = draft.configuration().ok_or_else(invalid)?;
        if configuration.schema().as_str() != MATERIAL_EDIT_SCHEMA {
            return Err(invalid());
        }
        let state: Self = serde_json::from_str(configuration.json()).map_err(|_| invalid())?;
        if state.session == 0
            || draft.input().is_none()
            || draft.operator_type().as_str() != state.editor.operator_type()
            || draft
                .input_data_type()
                .is_none_or(|t| t.as_str() != state.editor.data_type())
            || draft.output_data_type().as_str() != state.editor.data_type()
        {
            return Err(invalid());
        }
        Ok(state)
    }
}

/// Explicit media normalization used for an editor input or accepted return.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialReturnNormalization {
    #[default]
    Canonical,
    /// Quantize signed PCM24 to PCM16, nearest with ties away from zero and saturation.
    Pcm24ToPcm16,
    /// Materialize identical opaque RGB8 samples from one exactly known sRGB ICC profile.
    /// The accepted source bytes remain intact; this identifies only the external working copy.
    KnownSrgbIccInput {
        profile_digest: ContentDigest,
        materialized_digest: ContentDigest,
    },
}

/// Accepted return lineage. Digests describe exact input and selected return bytes;
/// the accepted Revision separately identifies normalized durable output bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialEditOperation {
    pub editor: MaterialEditor,
    pub stage_id: OperatorNodeId,
    pub session: u64,
    pub source: WorkingInput,
    pub input_digest: ContentDigest,
    pub returned_digest: ContentDigest,
    #[serde(default)]
    pub normalization: MaterialReturnNormalization,
}

impl MaterialEditOperation {
    /// Checks portable operation identity; byte and source checks belong to execution/storage.
    /// # Errors
    /// Rejects zero sessions and invalid node identifiers.
    pub fn validate(&self) -> Result<(), DomainError> {
        OperatorNodeId::new(self.stage_id.as_str())?;
        if self.session == 0
            || (self.normalization == MaterialReturnNormalization::Pcm24ToPcm16
                && self.editor != MaterialEditor::Echo)
            || (matches!(
                self.normalization,
                MaterialReturnNormalization::KnownSrgbIccInput { .. }
            ) && self.editor != MaterialEditor::Shadow)
        {
            return Err(DomainError::InvalidTransformationOperation);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
