//! Portable, versioned narration scripts and their authored voice/cue bindings.
mod parser;
mod production;
mod readiness;
use crate::{ContentDigest, ContentRef, PresetVoiceSelection};
pub use parser::{parse_speech_script, parse_speech_script_for_review};
pub use production::{CueLevel, SpeechDelivery, SpeechProduction, SpeechRole};
pub use readiness::{SpeechScriptReadiness, SpeechScriptRequirement};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SPEECH_SCRIPT_REVISION: &str = "20260922.2";
pub const MAX_SCRIPT_EVENTS: usize = 2048;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeechScriptOptions {
    pub revision: String,
    #[serde(default)]
    pub roles: BTreeMap<String, PresetVoiceSelection>,
    #[serde(default)]
    pub cues: BTreeMap<String, SpeechCueAction>,
}
impl Default for SpeechScriptOptions {
    fn default() -> Self {
        Self {
            revision: SPEECH_SCRIPT_REVISION.into(),
            roles: BTreeMap::new(),
            cues: BTreeMap::new(),
        }
    }
}
impl SpeechScriptOptions {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.revision == SPEECH_SCRIPT_REVISION
            && self.roles.len() <= 32
            && self.cues.len() <= 128
            && self
                .cues
                .values()
                .try_fold(0_u64, |sum, cue| {
                    sum.checked_add(match cue {
                        SpeechCueAction::Audio { content } => content.byte_length,
                        _ => 0,
                    })
                })
                .is_some_and(|bytes| bytes <= 32 * 1024 * 1024)
            && self
                .roles
                .iter()
                .all(|(role, voice)| valid_label(role) && voice.validate().is_ok())
            && self.cues.iter().all(|(label, action)| {
                valid_label(label)
                    && match action {
                        SpeechCueAction::Audio { content } => {
                            content.media_type == "audio/wav"
                                && content.byte_length > 44
                                && content.byte_length <= 8 * 1024 * 1024
                        }
                        SpeechCueAction::Beep { milliseconds, .. } => {
                            (20..=2000).contains(milliseconds)
                        }
                        SpeechCueAction::Skip | SpeechCueAction::Chime => true,
                    }
            })
    }
}
fn valid_label(label: &str) -> bool {
    !label.trim().is_empty() && label.len() <= 256 && !label.chars().any(char::is_control)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpeechCueAction {
    Skip,
    Chime,
    Beep { milliseconds: u32, level: CueLevel },
    Audio { content: ContentRef },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeechScriptPlan {
    pub revision: String,
    pub source_digest: ContentDigest,
    pub production: SpeechProduction,
    pub delivery: SpeechDelivery,
    pub roles: BTreeMap<String, SpeechRole>,
    pub cues: BTreeMap<String, Option<SpeechCueAction>>,
    pub events: Vec<SpeechScriptEvent>,
    pub issues: Vec<SpeechScriptIssue>,
}
impl SpeechScriptPlan {
    /// Explicit silence in the expanded playback order, including gaps between plays.
    #[must_use]
    pub fn pause_millis(&self) -> u64 {
        let mut total = 0;
        let mut repeat = None;
        for event in &self.events {
            match event.kind {
                SpeechScriptEventKind::Pause { milliseconds } => total += u64::from(milliseconds),
                SpeechScriptEventKind::RepeatStart { count, gap_ms, .. } => {
                    repeat = Some((total, count, gap_ms));
                }
                SpeechScriptEventKind::RepeatEnd => {
                    if let Some((start, count, gap_ms)) = repeat.take() {
                        total += (total - start + u64::from(gap_ms))
                            * u64::from(count.saturating_sub(1));
                    }
                }
                _ => {}
            }
        }
        total
    }

    /// Explicit bindings override declared built-ins; external assets must be resolved.
    #[must_use]
    pub fn cue<'a>(
        &'a self,
        label: &str,
        options: &'a SpeechScriptOptions,
    ) -> Option<&'a SpeechCueAction> {
        options
            .cues
            .get(label)
            .or_else(|| self.cues.get(label).and_then(Option::as_ref))
    }

    /// Roles with identical selected voices are review warnings, never silently rebound.
    #[must_use]
    pub fn shared_voices(&self, options: &SpeechScriptOptions) -> Vec<String> {
        let mut seen = BTreeMap::new();
        let mut shared = Vec::new();
        for (role, voice) in &options.roles {
            if self.roles.contains_key(role) && seen.insert(voice.alias.as_str(), role).is_some() {
                shared.push(role.clone());
            }
        }
        shared
    }

    #[must_use]
    pub fn spoken_text(&self) -> String {
        self.events
            .iter()
            .filter_map(|event| match &event.kind {
                SpeechScriptEventKind::Speech { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }
    #[must_use]
    /// Stable identity of the parsed portable plan.
    /// # Panics
    /// Only if serialization of these fixed portable values fails.
    pub fn digest(&self) -> ContentDigest {
        // All fields are deterministic portable values, with no fallible JSON map keys.
        ContentDigest::from_bytes(
            &serde_json::to_vec(self).expect("portable script plan serializes"),
        )
    }
    #[must_use]
    pub fn ready(&self, options: &SpeechScriptOptions) -> bool {
        self.readiness(options).ready
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeechScriptEvent {
    pub line: u32,
    #[serde(flatten)]
    pub kind: SpeechScriptEventKind,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SpeechScriptEventKind {
    Speech {
        role: String,
        language: Option<String>,
        delivery: SpeechDelivery,
        text: String,
    },
    RepeatStart {
        count: u8,
        gap_ms: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        between_cue: Option<String>,
    },
    RepeatEnd,
    Scene {
        label: String,
    },
    Pause {
        milliseconds: u32,
    },
    Cue {
        label: String,
    },
    Heading {
        text: String,
    },
    Note {
        text: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeechScriptIssue {
    pub line: u32,
    pub code: String,
    pub text: String,
}

#[cfg(test)]
mod tests;
