//! Authored zero-input sound generation, independent of provider and platform types.
use crate::DomainError;
use serde::{Deserialize, Serialize};

pub const SOUND_GENERATION_REVISION: &str = "20260926.1";
/// The creative choice also determines which supported local model family executes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoundGenerationKind {
    SoundEffect,
    ShortMusic,
}

/// Exact authored parameters copied into accepted history. Seed is always explicit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoundGenerationOperation {
    pub revision: String,
    pub prompt: String,
    pub kind: SoundGenerationKind,
    pub duration_seconds: u8,
    pub seed: u32,
}
impl SoundGenerationOperation {
    /// Creates a bounded text-only generation request.
    /// # Errors
    /// Rejects empty, oversized or control-bearing prompts and durations outside 1–30 s.
    pub fn new(
        prompt: impl Into<String>,
        kind: SoundGenerationKind,
        duration_seconds: u8,
        seed: u32,
    ) -> Result<Self, DomainError> {
        let value = Self {
            revision: SOUND_GENERATION_REVISION.into(),
            prompt: prompt.into(),
            kind,
            duration_seconds,
            seed,
        };
        value.validate()?;
        Ok(value)
    }
    /// Revalidates deserialized parameters before execution or acceptance.
    /// # Errors
    /// Rejects unsupported revisions or invalid authored parameters.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.revision != SOUND_GENERATION_REVISION
            || self.prompt.trim().is_empty()
            || self.prompt.len() > 2000
            || self.prompt.chars().any(char::is_control)
            || !(1..=30).contains(&self.duration_seconds)
        {
            return Err(DomainError::InvalidSoundGenerationOperation);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests;
