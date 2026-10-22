//! One shared authored card back, independent of physical sheet and duplex settings.
use serde::{Deserialize, Serialize};

use super::{CardImageFit, CardLayoutConfig};
use crate::WorkingInput;

fn white() -> String {
    "#ffffff".into()
}
fn contain() -> CardImageFit {
    CardImageFit::Contain
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum CardKitBack {
    Solid {
        color: String,
    },
    Image {
        #[serde(default)]
        artwork: Option<WorkingInput>,
        #[serde(default = "contain")]
        image_fit: CardImageFit,
        #[serde(default = "white")]
        background_color: String,
    },
}

impl CardKitBack {
    /// # Errors
    /// Rejects colors beyond the same bounded RGB syntax used by front-face layouts.
    pub fn validate(&self) -> Result<(), &'static str> {
        let color = match self {
            Self::Solid { color } => color,
            Self::Image {
                background_color, ..
            } => background_color,
        };
        CardLayoutConfig {
            background_color: Some(color.clone()),
            ..Default::default()
        }
        .validate()
    }
    pub const fn artwork(&self) -> Option<WorkingInput> {
        match self {
            Self::Image { artwork, .. } => *artwork,
            Self::Solid { .. } => None,
        }
    }
    pub fn replace_artwork(&mut self, previous: WorkingInput, replacement: WorkingInput) {
        if let Self::Image { artwork, .. } = self
            && *artwork == Some(previous)
        {
            *artwork = Some(replacement);
        }
    }
}
