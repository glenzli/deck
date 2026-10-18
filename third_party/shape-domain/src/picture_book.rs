//! Portable ordered-page composition over immutable Shape materials.
use crate::WorkingInput;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const PICTURE_BOOK_SCHEMA: &str = "shape.picture-book/1";
pub const PICTURE_BOOK_MEDIA_TYPE: &str = "application/vnd.shape.picture-book+json";
const fn default_font_size() -> u16 {
    26
}

/// Bounded native-project composition over explicitly pinned, accepted materials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PictureBookRequest {
    pub name: String,
    pub default_language: String,
    pub languages: BTreeMap<String, WorkingInput>,
    pub pages: Vec<PictureBookPage>,
    pub canvas_width: u32,
    pub canvas_height: u32,
    #[serde(default = "default_font_size")]
    pub font_size: u16,
}

/// Ordered cover/title/body pages share geometry across languages, as in Storybook.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PictureBookPage {
    Image {
        source: WorkingInput,
        #[serde(default)]
        print_only: bool,
    },
    Blank {
        #[serde(default)]
        print_only: bool,
    },
}

impl PictureBookPage {
    pub const fn source(&self) -> Option<WorkingInput> {
        match self {
            Self::Image { source, .. } => Some(*source),
            Self::Blank { .. } => None,
        }
    }
    pub const fn print_only(&self) -> bool {
        match self {
            Self::Image { print_only, .. } | Self::Blank { print_only } => *print_only,
        }
    }
}
impl PictureBookRequest {
    /// One source may occur on several independently editable pages.
    pub fn inputs(&self) -> Vec<WorkingInput> {
        let mut unique = BTreeMap::new();
        for source in self
            .languages
            .values()
            .copied()
            .chain(self.pages.iter().filter_map(PictureBookPage::source))
        {
            unique.insert((source.artifact_id, source.revision_id), source);
        }
        unique.into_values().collect()
    }
    /// # Errors
    /// Rejects unsupported or out-of-bounds picture-book contracts.
    pub fn validate(&self) -> Result<(), &'static str> {
        self.validate_draft()?;
        if self.name.trim().is_empty()
            || self.languages.is_empty()
            || !self.languages.contains_key(&self.default_language)
        {
            return Err("an accepted picture book needs a name and a default language material");
        }
        Ok(())
    }

    /// Checks recoverable composition state before every language material has been adopted.
    /// # Errors
    /// Rejects unsupported layout, malformed language ids and ambiguous accepted pins.
    pub fn validate_draft(&self) -> Result<(), &'static str> {
        if self.name.len() > 256
            || self.name.chars().any(char::is_control)
            || self.pages.is_empty()
            || self.pages.len() > 64
            || self.languages.len() > 8
            || !valid_language(&self.default_language)
            || !(1..=8192).contains(&self.canvas_width)
            || !(1..=8192).contains(&self.canvas_height)
            || !(12..=96).contains(&self.font_size)
            || self.languages.keys().any(|s| !valid_language(s))
        {
            return Err(
                "invalid name, language, canvas, font size or page count (maximum 64 pages / 8 languages)",
            );
        }
        if self.pages[0].print_only() {
            return Err("the cover must be visible in the reading edition");
        }
        let inputs = self.inputs();
        let ids: BTreeSet<_> = inputs.iter().map(|s| s.artifact_id).collect();
        if inputs.len() > 64 || ids.len() != inputs.len() {
            return Err("use at most 64 source materials and one revision per material");
        }
        Ok(())
    }
}

pub(crate) fn valid_language(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 35
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PictureBookDocument {
    pub schema: String,
    pub composition: PictureBookRequest,
}
impl PictureBookDocument {
    pub fn new(composition: PictureBookRequest) -> Self {
        Self {
            schema: PICTURE_BOOK_SCHEMA.into(),
            composition,
        }
    }
    /// # Errors
    /// Rejects unsupported or out-of-bounds picture-book contracts.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != PICTURE_BOOK_SCHEMA {
            return Err("unsupported picture-book schema");
        }
        self.composition.validate()
    }
}

#[cfg(test)]
mod tests;
