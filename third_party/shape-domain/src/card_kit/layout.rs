//! Small card-face presets, independent of rendering, physical printing and runtime types.
use serde::{Deserialize, Serialize};
mod blocks;
pub use blocks::{CardBlock, CardBlockField, CardBlockVerticalAlign};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardLayoutMode {
    Auto,
    Image,
    Text,
    ImageText,
    Classic,
    Blocks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardImageFit {
    Contain,
    Cover,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardTextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardImagePosition {
    Auto,
    Top,
    Bottom,
    Left,
    Right,
    Background,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardTextVerticalAlign {
    Auto,
    Top,
    Center,
    Bottom,
}

/// Only explicit fields override inherited values. Missing mode-specific values resolve at use.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardLayoutConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<CardLayoutMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_fit: Option<CardImageFit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding_percent: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_align: Option<CardTextAlign>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_scale_percent: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_position: Option<CardImagePosition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_share_percent: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_vertical_align: Option<CardTextVerticalAlign>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_offset_x_percent: Option<i8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_offset_y_percent: Option<i8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocks: Option<Vec<CardBlock>>,
}

impl CardLayoutConfig {
    /// # Errors
    /// Rejects unbounded geometry, text scales and colors containing anything except RGB hex.
    pub fn validate(&self) -> Result<(), &'static str> {
        if let Some(blocks) = &self.blocks {
            if blocks.len() > 16 {
                return Err("use at most 16 card blocks");
            }
            let mut ids = std::collections::BTreeSet::new();
            for block in blocks {
                block.validate()?;
                if !ids.insert(&block.id) {
                    return Err("card block ids must be unique");
                }
            }
        }
        if self.padding_percent.is_some_and(|v| v > 20)
            || self
                .image_share_percent
                .is_some_and(|v| !(20..=80).contains(&v))
            || [self.text_offset_x_percent, self.text_offset_y_percent]
                .into_iter()
                .flatten()
                .any(|v| !(-30..=30).contains(&v))
            || self
                .text_scale_percent
                .is_some_and(|v| !(50..=300).contains(&v))
            || [&self.background_color, &self.text_color]
                .into_iter()
                .flatten()
                .any(|v| {
                    v.len() != 7
                        || !v.starts_with('#')
                        || !v.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
                })
        {
            return Err("invalid card layout padding, text scale or #RRGGBB color");
        }
        Ok(())
    }

    #[must_use]
    pub fn with_overrides(&self, card: Option<&Self>) -> Self {
        let Some(card) = card else {
            return self.clone();
        };
        Self {
            mode: card.mode.or(self.mode),
            image_fit: card.image_fit.or(self.image_fit),
            background_color: card
                .background_color
                .clone()
                .or_else(|| self.background_color.clone()),
            text_color: card.text_color.clone().or_else(|| self.text_color.clone()),
            border: card.border.or(self.border),
            padding_percent: card.padding_percent.or(self.padding_percent),
            text_align: card.text_align.or(self.text_align),
            text_scale_percent: card.text_scale_percent.or(self.text_scale_percent),
            image_position: card.image_position.or(self.image_position),
            image_share_percent: card.image_share_percent.or(self.image_share_percent),
            text_vertical_align: card.text_vertical_align.or(self.text_vertical_align),
            text_offset_x_percent: card.text_offset_x_percent.or(self.text_offset_x_percent),
            text_offset_y_percent: card.text_offset_y_percent.or(self.text_offset_y_percent),
            blocks: card.blocks.clone().or_else(|| self.blocks.clone()),
        }
    }
}

#[cfg(test)]
mod tests;
