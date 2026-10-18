//! Persisted interpretation of exact UTF-8 text, independent of authoring UI.

use serde::{Deserialize, Serialize};

use crate::speech_script::{SpeechScriptEventKind, parse_speech_script};

pub const SPEECH_SCRIPT_FORMAT_REVISION: &str = crate::speech_script::SPEECH_SCRIPT_REVISION;

/// A document's format travels with every accepted revision and candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "format", rename_all = "snake_case", deny_unknown_fields)]
pub enum TextDocumentContract {
    Plain,
    SpeechScript { revision: String },
}

impl TextDocumentContract {
    #[must_use]
    pub fn speech_script() -> Self {
        Self::SpeechScript {
            revision: SPEECH_SCRIPT_FORMAT_REVISION.into(),
        }
    }

    #[must_use]
    pub const fn is_script(&self) -> bool {
        matches!(self, Self::SpeechScript { .. })
    }

    /// Checks exact bytes before they can be adopted as this format.
    #[must_use]
    pub fn accepts(&self, bytes: &[u8]) -> bool {
        let Ok(text) = std::str::from_utf8(bytes) else {
            return false;
        };
        if text.trim().is_empty() {
            return false;
        }
        match self {
            Self::Plain => true,
            Self::SpeechScript { revision } => {
                let plan = parse_speech_script(text);
                revision == SPEECH_SCRIPT_FORMAT_REVISION
                    && plan.issues.is_empty()
                    && plan
                        .events
                        .iter()
                        .any(|event| matches!(event.kind, SpeechScriptEventKind::Speech { .. }))
            }
        }
    }
}

#[cfg(test)]
mod tests;
