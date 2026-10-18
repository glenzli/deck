use super::*;
use serde_json::json;

fn request() -> CardKitRequest {
    serde_json::from_value(json!({"name":"Moon cards","cards":[{"id":"moon-fox","title":"月狐","rules":"Draw a card."}]})).unwrap()
}

#[test]
fn card_ids_and_pins_are_stable_and_draft_configuration_round_trips() {
    let value = request();
    assert_eq!(value.canvas_width, 750);
    assert_eq!(
        CardKitRequest::from_configuration(&value.configuration().unwrap()).unwrap(),
        value
    );
    let mut duplicate = value.clone();
    duplicate.cards.push(duplicate.cards[0].clone());
    assert!(duplicate.validate().is_err());
    let mut traversal = value.clone();
    traversal.cards[0].id = "../escape".into();
    assert!(traversal.validate().is_err());
}

#[test]
fn ambiguous_rules_and_two_revisions_of_one_material_are_rejected() {
    let mut value = request();
    let source = WorkingInput {
        artifact_id: crate::ArtifactId::new(),
        revision_id: crate::RevisionId::new(),
    };
    value.cards[0].text_material = Some(source);
    assert!(value.validate().is_err());
    value.cards[0].rules.clear();
    value.validate().unwrap();
    value.cards[0].artwork = Some(WorkingInput {
        revision_id: crate::RevisionId::new(),
        ..source
    });
    assert!(value.validate().is_err());
}

#[test]
fn unknown_fields_and_unbounded_text_do_not_enter_drafts() {
    let mut value = serde_json::to_value(request()).unwrap();
    value["model"] = json!("provider-route");
    assert!(serde_json::from_value::<CardKitRequest>(value).is_err());
    let mut value = request();
    value.cards[0].rules = "x".repeat(8193);
    assert!(value.validate().is_err());
}

#[test]
fn legacy_missing_or_null_layout_is_omitted_and_card_overrides_require_opt_in() {
    let value = request();
    let original = serde_json::to_value(&value).unwrap();
    assert!(original.get("layout").is_none());
    assert!(original["cards"][0].get("layout").is_none());
    let mut explicit_null = original.clone();
    explicit_null["layout"] = json!(null);
    explicit_null["cards"][0]["layout"] = json!(null);
    let mut parsed: CardKitRequest = serde_json::from_value(explicit_null).unwrap();
    assert_eq!(serde_json::to_value(&parsed).unwrap(), original);
    parsed.cards[0].layout = Some(CardLayoutConfig::default());
    assert!(parsed.validate().is_err());
    parsed.layout = Some(CardLayoutConfig::default());
    parsed.validate().unwrap();
}
