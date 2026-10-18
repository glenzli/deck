//! Actionable, payload-free script prerequisites shared by desktop and external tools.
use super::{SpeechScriptEventKind, SpeechScriptOptions, SpeechScriptPlan};
use serde::Serialize;
use std::collections::BTreeMap;

/// First playback use requiring an explicit voice or cue choice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpeechScriptRequirement {
    pub name: String,
    pub line: u32,
}

/// Portable syntax and binding checks; never provider readiness or acceptance authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpeechScriptReadiness {
    pub ready: bool,
    pub syntax_valid: bool,
    pub options_valid: bool,
    pub spoken_passages: usize,
    pub missing_roles: Vec<SpeechScriptRequirement>,
    pub unresolved_cues: Vec<SpeechScriptRequirement>,
}

impl SpeechScriptPlan {
    /// Reports unique missing choices with their first used line in deterministic name order.
    /// Declared but unused roles/cues do not block playback. Built-in and skipped cues resolve.
    #[must_use]
    pub fn readiness(&self, options: &SpeechScriptOptions) -> SpeechScriptReadiness {
        let mut roles = BTreeMap::new();
        let mut cues = BTreeMap::new();
        let mut spoken_passages = 0;
        for event in &self.events {
            match &event.kind {
                SpeechScriptEventKind::Speech { role, .. } => {
                    spoken_passages += 1;
                    if !role.is_empty() && !options.roles.contains_key(role) {
                        roles.entry(role.clone()).or_insert(event.line);
                    }
                }
                SpeechScriptEventKind::Cue { label }
                | SpeechScriptEventKind::RepeatStart {
                    between_cue: Some(label),
                    ..
                } if self.cue(label, options).is_none() => {
                    cues.entry(label.clone()).or_insert(event.line);
                }
                _ => {}
            }
        }
        let syntax_valid = self.issues.is_empty();
        let options_valid = options.is_valid();
        SpeechScriptReadiness {
            ready: syntax_valid
                && options_valid
                && spoken_passages > 0
                && roles.is_empty()
                && cues.is_empty(),
            syntax_valid,
            options_valid,
            spoken_passages,
            missing_roles: requirements(roles),
            unresolved_cues: requirements(cues),
        }
    }
}

fn requirements(values: BTreeMap<String, u32>) -> Vec<SpeechScriptRequirement> {
    values
        .into_iter()
        .map(|(name, line)| SpeechScriptRequirement { name, line })
        .collect()
}

#[cfg(test)]
mod tests;
