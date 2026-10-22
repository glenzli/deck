//! Bounded field-bound layers over the complete card canvas, in integral ten-thousandths.
use super::{CardImageFit, CardTextAlign};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardBlockField {
    Title,
    Rules,
    Artwork,
    CardType,
    Cost,
    Stats,
    Flavor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardBlockVerticalAlign {
    Top,
    Center,
    Bottom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardBlock {
    pub id: String,
    pub field: CardBlockField,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub text_scale_percent: u16,
    pub text_align: CardTextAlign,
    pub text_vertical_align: CardBlockVerticalAlign,
    pub bold: bool,
    pub italic: bool,
    pub text_color: String,
    pub image_fit: CardImageFit,
}
impl CardBlock {
    /// # Errors
    /// Rejects unsafe identities, geometry outside the canvas, text scales and non-RGB colors.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.id.is_empty()
            || self.id.len() > 64
            || !self
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
            || self.x > 10000
            || self.y > 10000
            || !(1..=10000).contains(&self.width)
            || !(1..=10000).contains(&self.height)
            || u32::from(self.x) + u32::from(self.width) > 10000
            || u32::from(self.y) + u32::from(self.height) > 10000
            || !(50..=300).contains(&self.text_scale_percent)
            || self.text_color.len() != 7
            || !self.text_color.starts_with('#')
            || !self.text_color.as_bytes()[1..]
                .iter()
                .all(u8::is_ascii_hexdigit)
        {
            return Err(
                "invalid card block identity, canvas rectangle, text scale or #RRGGBB color",
            );
        }
        Ok(())
    }
}
