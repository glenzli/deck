//! Authored card material lists and exact accepted artwork/text bindings.
//!
//! This is a content composition contract, not a game engine or print layout.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{OperatorConfigurationSchemaId, WorkingInput, WorkingOperatorConfiguration};

mod back;
mod layout;
pub use back::CardKitBack;
pub use layout::{
    CardBlock, CardBlockField, CardBlockVerticalAlign, CardImageFit, CardImagePosition,
    CardLayoutConfig, CardLayoutMode, CardTextAlign, CardTextVerticalAlign,
};

pub const CARD_KIT_SCHEMA: &str = "shape.card-kit/1";
pub const CARD_KIT_DRAFT_SCHEMA: &str = "shape.card-kit-draft/1";
pub const CARD_KIT_MEDIA_TYPE: &str = "application/vnd.shape.card-kit+json";
pub const CARD_KIT_OPERATOR_TYPE: &str = "card_kit.compose";
pub const CARD_KIT_DATA_TYPE: &str = "card.kit";
pub const CARD_KIT_MAX_BYTES: usize = 60 * 1024;

const fn default_width() -> u32 {
    750
}
const fn default_height() -> u32 {
    1050
}
const fn default_quantity() -> u16 {
    1
}

/// Shared direction and independently addressable cards. Missing materials are valid draft state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardKitRequest {
    pub name: String,
    #[serde(default)]
    pub brief: String,
    #[serde(default)]
    pub style: String,
    #[serde(default = "default_width")]
    pub canvas_width: u32,
    #[serde(default = "default_height")]
    pub canvas_height: u32,
    /// Absent in historical kits: their original renderer and completeness rules stay intact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<CardLayoutConfig>,
    /// One optional shared back; absent historical compositions retain their original JSON.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub back: Option<CardKitBack>,
    pub cards: Vec<CardKitCard>,
}

/// The stable id follows a card across reordering; visible text never becomes a path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardKitCard {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub card_type: String,
    #[serde(default)]
    pub cost: String,
    #[serde(default)]
    pub rules: String,
    #[serde(default)]
    pub stats: String,
    #[serde(default)]
    pub flavor: String,
    #[serde(default)]
    pub art_prompt: String,
    #[serde(default = "default_quantity")]
    pub quantity: u16,
    #[serde(default)]
    pub artwork: Option<WorkingInput>,
    /// When bound, this exact UTF-8 material supplies rules; authored rules must be empty.
    #[serde(default)]
    pub text_material: Option<WorkingInput>,
    /// Partial overrides of the kit layout; switching modes never removes authored content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<CardLayoutConfig>,
}

impl CardKitRequest {
    /// # Errors
    /// Rejects ambiguous identities, text sources, unsafe dimensions and excessive content.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.name.trim().is_empty()
            || !text(&self.name, 120, false)
            || !text(&self.brief, 8192, true)
            || !text(&self.style, 4096, true)
            || !(240..=2400).contains(&self.canvas_width)
            || !(240..=2400).contains(&self.canvas_height)
            || self.cards.is_empty()
            || self.cards.len() > 64
        {
            return Err("invalid card kit name, direction, canvas or card count (1 to 64)");
        }
        if let Some(layout) = &self.layout {
            layout.validate()?;
        }
        if let Some(back) = &self.back {
            back.validate()?;
        }
        let mut ids = BTreeSet::new();
        for card in &self.cards {
            if let Some(layout) = &card.layout {
                if self.layout.is_none() {
                    return Err("per-card layout requires a kit layout");
                }
                layout.validate()?;
            }
            if card.id.is_empty()
                || card.id.len() > 64
                || !card
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
                || !ids.insert(&card.id)
                || !text(&card.title, 256, false)
                || !text(&card.card_type, 128, false)
                || !text(&card.cost, 64, false)
                || !text(&card.stats, 128, false)
                || !text(&card.rules, 8192, true)
                || !text(&card.flavor, 2048, true)
                || !text(&card.art_prompt, 4096, true)
                || !(1..=999).contains(&card.quantity)
                || (card.text_material.is_some() && !card.rules.is_empty())
            {
                return Err("invalid card id, text, quantity or ambiguous rules material");
            }
        }
        let inputs = self.inputs();
        let unique_artifacts: BTreeSet<_> = inputs.iter().map(|s| s.artifact_id).collect();
        if inputs.len() > 64 || unique_artifacts.len() != inputs.len() {
            return Err("use at most 64 accepted materials and one revision per material");
        }
        if serde_json::to_vec(self)
            .map_err(|_| "invalid card kit JSON")?
            .len()
            > CARD_KIT_MAX_BYTES
        {
            return Err("card kit exceeds 60 KiB");
        }
        Ok(())
    }

    pub fn inputs(&self) -> Vec<WorkingInput> {
        let mut inputs = BTreeMap::new();
        for source in self
            .cards
            .iter()
            .flat_map(|c| [c.artwork, c.text_material])
            .flatten()
            .chain(self.back.as_ref().and_then(CardKitBack::artwork))
        {
            inputs.insert((source.artifact_id, source.revision_id), source);
        }
        inputs.into_values().collect()
    }

    pub fn uses_artwork(&self, source: WorkingInput) -> bool {
        self.cards.iter().any(|card| card.artwork == Some(source))
            || self.back.as_ref().and_then(CardKitBack::artwork) == Some(source)
    }

    /// # Errors
    /// Rejects invalid composition or a malformed working configuration envelope.
    pub fn configuration(&self) -> Result<WorkingOperatorConfiguration, &'static str> {
        self.validate()?;
        WorkingOperatorConfiguration::new(
            OperatorConfigurationSchemaId::new(CARD_KIT_DRAFT_SCHEMA)
                .map_err(|_| "invalid card kit draft schema")?,
            serde_json::to_string(self).map_err(|_| "invalid card kit JSON")?,
        )
        .map_err(|_| "invalid card kit draft configuration")
    }

    /// # Errors
    /// Rejects unknown draft schemas and invalid bounded card content.
    pub fn from_configuration(
        configuration: &WorkingOperatorConfiguration,
    ) -> Result<Self, &'static str> {
        if configuration.schema().as_str() != CARD_KIT_DRAFT_SCHEMA
            || configuration.json().len() > CARD_KIT_MAX_BYTES
        {
            return Err("unsupported card kit draft configuration");
        }
        let value: Self = serde_json::from_str(configuration.json())
            .map_err(|_| "invalid card kit draft JSON")?;
        value.validate()?;
        Ok(value)
    }
}

fn text(value: &str, max: usize, multiline: bool) -> bool {
    value.len() <= max
        && !value
            .chars()
            .any(|c| c.is_control() && !(multiline && matches!(c, '\n' | '\r' | '\t')))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardKitDocument {
    pub schema: String,
    pub composition: CardKitRequest,
}
impl CardKitDocument {
    pub fn new(composition: CardKitRequest) -> Self {
        Self {
            schema: CARD_KIT_SCHEMA.into(),
            composition,
        }
    }
    /// # Errors
    /// Rejects unsupported or invalid card material manifests.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != CARD_KIT_SCHEMA {
            return Err("unsupported card kit schema");
        }
        self.composition.validate()
    }
}

#[cfg(test)]
mod tests;
