//! Portable production declarations shared by parsing, authoring and speech execution.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechProduction {
    #[default]
    Narration,
    Listening,
    Dialogue,
}
impl SpeechProduction {
    pub(super) fn parse(value: &str) -> Option<Self> {
        match value {
            "narration" | "口播" => Some(Self::Narration),
            "listening" | "听力" => Some(Self::Listening),
            "dialogue" | "对话" => Some(Self::Dialogue),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechDelivery {
    #[default]
    Neutral,
    Clear,
    Warm,
    Lively,
}
impl SpeechDelivery {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "neutral" | "自然平稳" => Some(Self::Neutral),
            "clear" | "清晰平稳" => Some(Self::Clear),
            "warm" | "温暖亲切" => Some(Self::Warm),
            "lively" | "轻快活泼" => Some(Self::Lively),
            _ => None,
        }
    }
    /// A bounded instruction is sent separately from spoken text, never read aloud.
    #[must_use]
    pub const fn instruction(self) -> &'static str {
        match self {
            Self::Neutral => {
                "Speak naturally with a steady, neutral delivery. Keep the same voice and consistent loudness throughout."
            }
            Self::Clear => {
                "Speak clearly and evenly. Keep a steady voice and loudness, and avoid exaggerated emphasis."
            }
            Self::Warm => {
                "Speak warmly and calmly, with a friendly tone and consistent voice and loudness."
            }
            Self::Lively => {
                "Speak with a light, lively tone. Keep the same voice and consistent loudness; avoid shouting."
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CueLevel {
    Soft,
    Normal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeechRole {
    pub language: String,
    pub delivery: Option<SpeechDelivery>,
    pub description: String,
}
