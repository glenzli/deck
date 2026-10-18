//! Recoverable book workspace state. Unadopted script bytes are checkpoints, never accepted text.
use crate::{
    ArtifactId, ContentRef, OperatorConfigurationSchemaId, PictureBookRequest, WorkingInput,
    WorkingOperatorConfiguration,
};
use serde::{Deserialize, Serialize};

pub const PICTURE_BOOK_DRAFT_SCHEMA: &str = "shape.picture-book-draft/1";
pub const PICTURE_BOOK_DRAFT_SCRIPT_MEDIA_TYPE: &str = "text/plain; charset=utf-8";
pub const PICTURE_BOOK_DRAFT_MAX_SCRIPT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PictureBookArtworkEditDraft {
    pub output_artifact_id: ArtifactId,
    pub page_index: usize,
    pub source: WorkingInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PictureBookDraftState {
    pub composition: PictureBookRequest,
    pub active_language: String,
    pub page_index: usize,
    #[serde(default)]
    pub script: String,
    #[serde(default)]
    pub script_dirty: bool,
    #[serde(default)]
    pub artwork_edit: Option<PictureBookArtworkEditDraft>,
}
impl PictureBookDraftState {
    /// # Errors
    /// Allows incomplete script syntax but rejects unbounded text and invalid workspace identities.
    pub fn validate(&self) -> Result<(), &'static str> {
        self.composition.validate_draft()?;
        if !crate::picture_book::valid_language(&self.active_language)
            || self.page_index >= self.composition.pages.len()
            || self.script.len() > PICTURE_BOOK_DRAFT_MAX_SCRIPT_BYTES
            || self
                .artwork_edit
                .as_ref()
                .is_some_and(|e| e.page_index >= 64)
        {
            return Err("invalid picture book workspace language, page or script size");
        }
        Ok(())
    }

    /// # Errors
    /// Rejects invalid state or a checkpoint reference that is not the exact authored script.
    pub fn checkpoint(
        &self,
        script_content: ContentRef,
    ) -> Result<PictureBookDraftCheckpoint, &'static str> {
        self.validate()?;
        if script_content.media_type != PICTURE_BOOK_DRAFT_SCRIPT_MEDIA_TYPE
            || script_content.byte_length != self.script.len() as u64
            || script_content.digest != crate::ContentDigest::from_bytes(self.script.as_bytes())
        {
            return Err("picture book draft script reference does not match exact text");
        }
        Ok(PictureBookDraftCheckpoint {
            composition: self.composition.clone(),
            active_language: self.active_language.clone(),
            page_index: self.page_index,
            script_content,
            script_dirty: self.script_dirty,
            artwork_edit: self.artwork_edit.clone(),
        })
    }
}

/// The Working Graph contains this bounded metadata and the immutable CAS script reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PictureBookDraftCheckpoint {
    pub composition: PictureBookRequest,
    pub active_language: String,
    pub page_index: usize,
    pub script_content: ContentRef,
    pub script_dirty: bool,
    pub artwork_edit: Option<PictureBookArtworkEditDraft>,
}
impl PictureBookDraftCheckpoint {
    /// # Errors
    /// Rejects malformed metadata, oversized script references and unsupported configuration schemas.
    pub fn validate(&self) -> Result<(), &'static str> {
        self.state(String::new()).validate()?;
        if self.script_content.media_type != PICTURE_BOOK_DRAFT_SCRIPT_MEDIA_TYPE
            || self.script_content.byte_length > PICTURE_BOOK_DRAFT_MAX_SCRIPT_BYTES as u64
        {
            return Err("invalid picture book draft script content reference");
        }
        Ok(())
    }
    pub fn state(&self, script: String) -> PictureBookDraftState {
        PictureBookDraftState {
            composition: self.composition.clone(),
            active_language: self.active_language.clone(),
            page_index: self.page_index,
            script,
            script_dirty: self.script_dirty,
            artwork_edit: self.artwork_edit.clone(),
        }
    }
    /// # Errors
    /// Rejects invalid metadata and Working Graph envelopes larger than the existing 64 KiB bound.
    pub fn configuration(&self) -> Result<WorkingOperatorConfiguration, &'static str> {
        self.validate()?;
        WorkingOperatorConfiguration::new(
            OperatorConfigurationSchemaId::new(PICTURE_BOOK_DRAFT_SCHEMA)
                .map_err(|_| "invalid book draft schema")?,
            serde_json::to_string(self).map_err(|_| "invalid book draft JSON")?,
        )
        .map_err(|_| "picture book draft metadata exceeds its configuration bound")
    }
    /// # Errors
    /// Rejects unsupported or malformed persisted draft configuration.
    pub fn from_configuration(
        configuration: &WorkingOperatorConfiguration,
    ) -> Result<Self, &'static str> {
        if configuration.schema().as_str() != PICTURE_BOOK_DRAFT_SCHEMA {
            return Err("unsupported picture book draft schema");
        }
        let value: Self = serde_json::from_str(configuration.json())
            .map_err(|_| "invalid picture book draft JSON")?;
        value.validate()?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests;
