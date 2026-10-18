use super::*;
use crate::speech_script::{SpeechCueAction, parse_speech_script};
use crate::{PresetVoiceAlias, PresetVoiceSelection};

#[test]
fn requirements_cover_repeats_and_deduplicate_only_used_bindings() {
    let source = "[role: Guest]\n[role: unused]\n[cue: turn]\n[cue: unused]\n[repeat: 2; gap: 1s; cue: turn]\n[speaker: Guest]\nHello.\nHello again.\n[audio: turn]\n[end-repeat]";
    let plan = parse_speech_script(source);
    let original = plan.digest();
    let mut options = SpeechScriptOptions::default();
    let missing = plan.readiness(&options);
    assert_eq!(
        missing.missing_roles,
        vec![SpeechScriptRequirement {
            name: "Guest".into(),
            line: 7
        }]
    );
    assert_eq!(
        missing.unresolved_cues,
        vec![SpeechScriptRequirement {
            name: "turn".into(),
            line: 5
        }]
    );
    assert!(missing.syntax_valid && missing.options_valid && missing.spoken_passages == 2);
    assert!(!missing.ready);
    options.roles.insert(
        "Guest".into(),
        PresetVoiceSelection::new(
            PresetVoiceAlias::new("speech.voice.en.warm_male.v1").unwrap(),
            "20260922.1",
        )
        .unwrap(),
    );
    options.cues.insert("turn".into(), SpeechCueAction::Skip);
    let ready = plan.readiness(&options);
    assert!(ready.ready && plan.ready(&options));
    assert!(ready.missing_roles.is_empty() && ready.unresolved_cues.is_empty());
    assert_eq!(
        plan.digest(),
        original,
        "diagnostics never rewrite the portable plan"
    );
}

#[test]
fn builtins_resolve_without_external_choices_but_invalid_settings_still_block() {
    let plan = parse_speech_script("[cue: turn; sound: beep]\nHello.\n[audio: turn]");
    let mut options = SpeechScriptOptions::default();
    assert!(plan.readiness(&options).ready);
    options.revision = "unsupported".into();
    let invalid = plan.readiness(&options);
    assert!(invalid.syntax_valid && invalid.unresolved_cues.is_empty());
    assert!(!invalid.options_valid && !invalid.ready && !plan.ready(&options));
}

#[test]
fn syntax_and_no_speech_remain_distinct_from_missing_choices() {
    let options = SpeechScriptOptions::default();
    let malformed = parse_speech_script("Hello.\n[pause: invalid]").readiness(&options);
    assert!(malformed.spoken_passages == 1 && malformed.missing_roles.is_empty());
    assert!(!malformed.syntax_valid && !malformed.ready);
    let empty = parse_speech_script("[note: remember the pronunciation]").readiness(&options);
    assert!(empty.spoken_passages == 0 && !empty.ready);
}
