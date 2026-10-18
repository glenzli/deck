//! Portable audio values and creative Audio Operator contracts.
//!
//! Audio Sources are immutable accepted values: text, audio clips, or an
//! explicitly authorized voice reference. Creation from text is therefore a
//! `speech_synthesize` Operator, never a Source pretending to contain generated
//! bytes. Provider models, endpoints, and mutable preview state do not belong
//! in these contracts.

use serde::{Deserialize, Serialize};

use crate::{DomainError, RevisionId};

const MAX_VOICE_ALIAS_BYTES: usize = 96;
const MAX_VOICE_CATALOG_REVISION_BYTES: usize = 160;
const MAX_LANGUAGE_TAG_BYTES: usize = 35;
const MAX_AUTHORIZATION_ID_BYTES: usize = 160;

/// Current portable interpretation of one accepted audio clip.
pub const AUDIO_VALUE_CONTRACT_REVISION: &str = "20260811.1";

/// Scene-graph data type produced by audio Operators.
pub const AUDIO_CLIP_DATA_TYPE: &str = "audio.clip";
/// Scene-graph data type for an immutable, explicitly authorized voice reference.
pub const AUTHORIZED_VOICE_REFERENCE_DATA_TYPE: &str = "audio.voice_reference.authorized";

/// Container carrying one accepted audio value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioContainer {
    Wav,
}

/// Exact decoded sample representation declared by the accepted container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioSampleFormat {
    PcmS16Le,
}

/// Truthful origin disclosure for an accepted audio value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioOriginDisclosure {
    RecordedSource,
    /// Imported audio whose creation process has not been verified by Shape.
    ImportedUnverified,
    SyntheticSpeech,
    SyntheticSound,
    TransformedAudio,
}

/// Portable interpretation of immutable accepted audio bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioValueContract {
    pub contract_revision: String,
    pub container: AudioContainer,
    pub sample_format: AudioSampleFormat,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub frame_count: u64,
    pub origin: AudioOriginDisclosure,
}

impl AudioValueContract {
    /// Creates the first canonical Shape audio value: PCM S16 LE in a WAV container.
    ///
    /// # Errors
    ///
    /// Rejects empty audio or implausible sample/channel contracts.
    pub fn pcm_s16le_wav(
        sample_rate_hz: u32,
        channels: u16,
        frame_count: u64,
        origin: AudioOriginDisclosure,
    ) -> Result<Self, DomainError> {
        if !(8_000..=384_000).contains(&sample_rate_hz)
            || !(1..=32).contains(&channels)
            || frame_count == 0
        {
            return Err(DomainError::InvalidAudioValueContract);
        }
        Ok(Self {
            contract_revision: AUDIO_VALUE_CONTRACT_REVISION.to_owned(),
            container: AudioContainer::Wav,
            sample_format: AudioSampleFormat::PcmS16Le,
            sample_rate_hz,
            channels,
            frame_count,
            origin,
        })
    }

    /// Exact duration rounded down to whole milliseconds for presentation.
    #[must_use]
    pub fn duration_millis(&self) -> u64 {
        self.frame_count
            .saturating_mul(1_000)
            .checked_div(u64::from(self.sample_rate_hz))
            .unwrap_or(0)
    }
}

/// Evolvable creative Audio Operator families.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioOperatorFamily {
    Generate,
    SpeechSynthesize,
    Transform,
}

/// Data contract allowed at one audio-family port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioPortDataType {
    TextDocument,
    AudioClip,
    AuthorizedVoiceReference,
}

impl AudioPortDataType {
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::TextDocument => "text.document",
            Self::AudioClip => AUDIO_CLIP_DATA_TYPE,
            Self::AuthorizedVoiceReference => AUTHORIZED_VOICE_REFERENCE_DATA_TYPE,
        }
    }
}

/// Whether an instantiated Operator must expose a port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioPortCardinality {
    Required,
    Optional,
    ZeroOrMore,
}

/// One stable port in an audio-family capability contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioPortContract {
    pub id: &'static str,
    pub data_type: AudioPortDataType,
    pub cardinality: AudioPortCardinality,
}

/// Version-independent creative ports for one audio family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioOperatorContract {
    pub operator_type: &'static str,
    pub inputs: &'static [AudioPortContract],
    pub outputs: &'static [AudioPortContract],
}

const AUDIO_OUTPUTS: [AudioPortContract; 1] = [AudioPortContract {
    id: "output.audio",
    data_type: AudioPortDataType::AudioClip,
    cardinality: AudioPortCardinality::Required,
}];
// Prompts are authored parameters; reference-audio generation is a separate future family.
const GENERATE_INPUTS: [AudioPortContract; 0] = [];
const SPEECH_INPUTS: [AudioPortContract; 2] = [
    AudioPortContract {
        id: "input.text",
        data_type: AudioPortDataType::TextDocument,
        cardinality: AudioPortCardinality::Required,
    },
    AudioPortContract {
        id: "input.voice",
        data_type: AudioPortDataType::AuthorizedVoiceReference,
        cardinality: AudioPortCardinality::Optional,
    },
];
const TRANSFORM_INPUTS: [AudioPortContract; 3] = [
    AudioPortContract {
        id: "input.audio",
        data_type: AudioPortDataType::AudioClip,
        cardinality: AudioPortCardinality::Required,
    },
    AudioPortContract {
        id: "input.instruction",
        data_type: AudioPortDataType::TextDocument,
        cardinality: AudioPortCardinality::Optional,
    },
    AudioPortContract {
        id: "input.voice",
        data_type: AudioPortDataType::AuthorizedVoiceReference,
        cardinality: AudioPortCardinality::Optional,
    },
];

impl AudioOperatorFamily {
    /// Returns the creative port contract without claiming runtime executability.
    #[must_use]
    pub const fn contract(self) -> AudioOperatorContract {
        match self {
            Self::Generate => AudioOperatorContract {
                operator_type: "audio.generate",
                inputs: &GENERATE_INPUTS,
                outputs: &AUDIO_OUTPUTS,
            },
            Self::SpeechSynthesize => AudioOperatorContract {
                operator_type: "audio.speech_synthesize",
                inputs: &SPEECH_INPUTS,
                outputs: &AUDIO_OUTPUTS,
            },
            Self::Transform => AudioOperatorContract {
                operator_type: "audio.transform",
                inputs: &TRANSFORM_INPUTS,
                outputs: &AUDIO_OUTPUTS,
            },
        }
    }
}

/// Runtime-owned non-biometric voice alias suitable for ordinary synthesis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PresetVoiceAlias(String);

impl PresetVoiceAlias {
    /// Creates a bounded logical alias, never an arbitrary audio path or Echo identity.
    ///
    /// # Errors
    ///
    /// Rejects empty, oversized, or provider-unsafe aliases.
    pub fn new(alias: impl Into<String>) -> Result<Self, DomainError> {
        let alias = alias.into();
        let value = Self(alias);
        value.validate()?;
        Ok(value)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Revalidates a value loaded through `serde`.
    ///
    /// # Errors
    ///
    /// Rejects empty, oversized, or provider-unsafe aliases.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.0.is_empty()
            || self.0.len() > MAX_VOICE_ALIAS_BYTES
            || !self.0.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            })
        {
            return Err(DomainError::InvalidPresetVoiceAlias {
                max_bytes: MAX_VOICE_ALIAS_BYTES,
            });
        }
        Ok(())
    }
}

/// Explicit consent scope carried by a Voice Reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceAuthorizationScope {
    SpeechSynthesis,
    VoiceClone,
    VoiceConversion,
}

/// Immutable authorization required before a real voice may become a Source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizedVoiceReference {
    pub source_revision_id: RevisionId,
    pub consent_receipt_id: String,
    pub scope: VoiceAuthorizationScope,
    pub local_only: bool,
    pub synthetic_disclosure_required: bool,
}

impl AuthorizedVoiceReference {
    /// Creates a voice reference only when local-only use and synthetic disclosure are explicit.
    ///
    /// # Errors
    ///
    /// Rejects incomplete consent or policies that could silently imitate an identity.
    pub fn new(
        source_revision_id: RevisionId,
        consent_receipt_id: impl Into<String>,
        scope: VoiceAuthorizationScope,
        local_only: bool,
        synthetic_disclosure_required: bool,
    ) -> Result<Self, DomainError> {
        let consent_receipt_id = consent_receipt_id.into();
        let reference = Self {
            source_revision_id,
            consent_receipt_id,
            scope,
            local_only,
            synthetic_disclosure_required,
        };
        reference.validate()?;
        Ok(reference)
    }

    /// Revalidates a reference loaded through `serde`.
    ///
    /// # Errors
    ///
    /// Rejects incomplete consent, non-local use, or missing disclosure.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.consent_receipt_id.is_empty()
            || self.consent_receipt_id.len() > MAX_AUTHORIZATION_ID_BYTES
            || !self.consent_receipt_id.is_ascii()
            || !self.local_only
            || !self.synthetic_disclosure_required
        {
            return Err(DomainError::InvalidVoiceAuthorization {
                max_bytes: MAX_AUTHORIZATION_ID_BYTES,
            });
        }
        Ok(())
    }
}

/// Voice selection authored by a speech synthesis Operator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "selection", rename_all = "snake_case")]
pub enum SpeechVoiceSelection {
    Preset(PresetVoiceSelection),
    AuthorizedReference(AuthorizedVoiceReference),
}

/// Versioned Runtime-owned preset voice selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetVoiceSelection {
    pub alias: PresetVoiceAlias,
    pub catalog_revision: String,
}

impl PresetVoiceSelection {
    /// Binds a stable logical alias to the catalog revision that defined it.
    ///
    /// # Errors
    ///
    /// Rejects empty, oversized, or non-portable catalog identities.
    pub fn new(
        alias: PresetVoiceAlias,
        catalog_revision: impl Into<String>,
    ) -> Result<Self, DomainError> {
        let catalog_revision = catalog_revision.into();
        let selection = Self {
            alias,
            catalog_revision,
        };
        selection.validate()?;
        Ok(selection)
    }

    /// Revalidates a preset loaded through `serde`.
    ///
    /// # Errors
    ///
    /// Rejects an invalid alias or catalog revision.
    pub fn validate(&self) -> Result<(), DomainError> {
        self.alias.validate()?;
        if self.catalog_revision.is_empty()
            || self.catalog_revision.len() > MAX_VOICE_CATALOG_REVISION_BYTES
            || !self.catalog_revision.is_ascii()
        {
            return Err(DomainError::InvalidVoiceCatalogRevision {
                max_bytes: MAX_VOICE_CATALOG_REVISION_BYTES,
            });
        }
        Ok(())
    }
}

/// Persisted creative parameters of one speech synthesis Operator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeechSynthesisOperation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery: Option<crate::speech_script::SpeechDelivery>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<crate::speech_script::SpeechScriptOptions>,
    pub language: String,
    pub voice: SpeechVoiceSelection,
    /// Playback-rate multiplier in thousandths (`1000` means 1.0x).
    pub speed_milli: u16,
    pub synthetic_disclosure_required: bool,
}

impl SpeechSynthesisOperation {
    /// Creates a synthesis operation with explicit disclosure.
    ///
    /// # Errors
    ///
    /// Rejects malformed language tags, unsupported speed, or hidden synthesis.
    pub fn new(
        language: impl Into<String>,
        voice: SpeechVoiceSelection,
        speed_milli: u16,
        synthetic_disclosure_required: bool,
    ) -> Result<Self, DomainError> {
        let operation = Self {
            script: None,
            delivery: None,
            language: language.into(),
            voice,
            speed_milli,
            synthetic_disclosure_required,
        };
        operation.validate()?;
        Ok(operation)
    }

    /// Revalidates authored parameters loaded through `serde` before execution or acceptance.
    ///
    /// # Errors
    ///
    /// Rejects invalid voice selection, language, speed, or disclosure.
    pub fn validate(&self) -> Result<(), DomainError> {
        let voice_valid = match &self.voice {
            SpeechVoiceSelection::Preset(selection) => selection.validate().is_ok(),
            SpeechVoiceSelection::AuthorizedReference(reference) => {
                reference.validate().is_ok()
                    && reference.scope == VoiceAuthorizationScope::SpeechSynthesis
            }
        };
        if self
            .script
            .as_ref()
            .is_some_and(|script| !script.is_valid())
            || self.language.is_empty()
            || self.language.len() > MAX_LANGUAGE_TAG_BYTES
            || !self
                .language
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || !(250..=4_000).contains(&self.speed_milli)
            || !self.synthetic_disclosure_required
            || !voice_valid
        {
            return Err(DomainError::InvalidSpeechSynthesisOperation {
                max_language_bytes: MAX_LANGUAGE_TAG_BYTES,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
