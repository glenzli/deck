use crate::RevisionId;

use super::{
    AudioOperatorFamily, AudioOriginDisclosure, AudioPortCardinality, AudioPortDataType,
    AudioValueContract, AuthorizedVoiceReference, PresetVoiceAlias, PresetVoiceSelection,
    SpeechSynthesisOperation, SpeechVoiceSelection, VoiceAuthorizationScope,
};

#[test]
fn audio_family_ports_keep_sources_typed_and_generation_in_operator_space() {
    let speech = AudioOperatorFamily::SpeechSynthesize.contract();
    assert_eq!(speech.operator_type, "audio.speech_synthesize");
    assert_eq!(speech.inputs[0].data_type, AudioPortDataType::TextDocument);
    assert_eq!(speech.inputs[0].cardinality, AudioPortCardinality::Required);
    assert_eq!(
        speech.inputs[1].data_type,
        AudioPortDataType::AuthorizedVoiceReference
    );
    assert_eq!(speech.inputs[1].cardinality, AudioPortCardinality::Optional);
    assert_eq!(speech.outputs[0].data_type, AudioPortDataType::AudioClip);

    let transform = AudioOperatorFamily::Transform.contract();
    assert_eq!(transform.inputs[0].data_type, AudioPortDataType::AudioClip);
    assert_eq!(
        transform.inputs[2].data_type,
        AudioPortDataType::AuthorizedVoiceReference
    );
}

#[test]
fn audio_value_has_exact_sample_duration_and_synthetic_origin() {
    let contract = AudioValueContract::pcm_s16le_wav(
        24_000,
        1,
        36_000,
        AudioOriginDisclosure::SyntheticSpeech,
    )
    .unwrap();
    assert_eq!(contract.duration_millis(), 1_500);
    assert!(
        AudioValueContract::pcm_s16le_wav(0, 0, 0, AudioOriginDisclosure::SyntheticSpeech).is_err()
    );
}

#[test]
fn preset_and_authorized_voices_cannot_hide_arbitrary_identity_inputs() {
    assert!(PresetVoiceAlias::new("../../echo/recording.wav").is_err());
    let preset = PresetVoiceSelection::new(
        PresetVoiceAlias::new("narrator.neutral.zh_cn").unwrap(),
        "runtime.voice-catalog@1",
    )
    .unwrap();
    let operation =
        SpeechSynthesisOperation::new("zh-CN", SpeechVoiceSelection::Preset(preset), 1_000, true)
            .unwrap();
    assert!(operation.synthetic_disclosure_required);

    assert!(
        AuthorizedVoiceReference::new(
            RevisionId::new(),
            "consent.receipt.1",
            VoiceAuthorizationScope::VoiceClone,
            false,
            true,
        )
        .is_err()
    );
    assert!(
        AuthorizedVoiceReference::new(
            RevisionId::new(),
            "consent.receipt.1",
            VoiceAuthorizationScope::VoiceClone,
            true,
            false,
        )
        .is_err()
    );

    let clone_only = AuthorizedVoiceReference::new(
        RevisionId::new(),
        "consent.receipt.2",
        VoiceAuthorizationScope::VoiceClone,
        true,
        true,
    )
    .unwrap();
    assert!(
        SpeechSynthesisOperation::new(
            "zh-CN",
            SpeechVoiceSelection::AuthorizedReference(clone_only),
            1_000,
            true,
        )
        .is_err()
    );
}

#[test]
fn serde_loaded_speech_parameters_are_revalidated_before_use() {
    let invalid_speed = serde_json::json!({
        "language": "Chinese",
        "voice": {
            "kind": "preset",
            "selection": {
                "alias": "speech.voice.zh.bright_female.v1",
                "catalog_revision": "infer.speech.voice-aliases@20260811.1"
            }
        },
        "speed_milli": 0,
        "synthetic_disclosure_required": true
    });
    let operation: SpeechSynthesisOperation = serde_json::from_value(invalid_speed).unwrap();
    assert!(operation.validate().is_err());

    let invalid_alias = serde_json::json!({
        "language": "Chinese",
        "voice": {
            "kind": "preset",
            "selection": {
                "alias": "../../voice.wav",
                "catalog_revision": "infer.speech.voice-aliases@20260811.1"
            }
        },
        "speed_milli": 1000,
        "synthetic_disclosure_required": true
    });
    let operation: SpeechSynthesisOperation = serde_json::from_value(invalid_alias).unwrap();
    assert!(operation.validate().is_err());
}
