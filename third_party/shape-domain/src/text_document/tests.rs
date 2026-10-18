use super::*;

#[test]
fn format_is_explicit_and_versioned() {
    let bytes = b"[role: Reader]\n[speaker: Reader]\nHello.\n[pause: 2s]";
    assert!(TextDocumentContract::speech_script().accepts(bytes));
    assert!(!TextDocumentContract::speech_script().accepts(b"[pause: nonsense]"));
    assert!(
        !TextDocumentContract::SpeechScript {
            revision: "future".into()
        }
        .accepts(bytes)
    );
    assert!(TextDocumentContract::Plain.accepts(b"[pause: nonsense]"));
    assert!(!TextDocumentContract::Plain.accepts(&[0xff]));
}
