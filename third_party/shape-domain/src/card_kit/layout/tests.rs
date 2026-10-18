use super::*;
use serde_json::json;

#[test]
fn partial_overrides_keep_explicit_root_values_and_leave_mode_defaults_unset() {
    let root: CardLayoutConfig = serde_json::from_value(json!({"mode":"auto","padding_percent":7,"background_color":"#A1b2C3","text_scale_percent":150})).unwrap();
    let card: CardLayoutConfig =
        serde_json::from_value(json!({"mode":"image","border":false})).unwrap();
    let merged = root.with_overrides(Some(&card));
    merged.validate().unwrap();
    assert_eq!(merged.mode, Some(CardLayoutMode::Image));
    assert_eq!(merged.padding_percent, Some(7));
    assert_eq!(merged.text_scale_percent, Some(150));
    assert_eq!(merged.text_align, None);
    assert_eq!(
        serde_json::to_value(CardLayoutConfig::default()).unwrap(),
        json!({})
    );
}

#[test]
fn colors_scales_padding_and_presets_are_bounded() {
    for value in [
        json!({"padding_percent":21}),
        json!({"text_scale_percent":49}),
        json!({"text_scale_percent":301}),
        json!({"text_color":"red"}),
        json!({"background_color":"#ff00zz"}),
    ] {
        let config: CardLayoutConfig = serde_json::from_value(value).unwrap();
        assert!(config.validate().is_err());
    }
    assert!(serde_json::from_value::<CardLayoutConfig>(json!({"mode":"canvas"})).is_err());
    assert!(serde_json::from_value::<CardLayoutConfig>(json!({"font":"web"})).is_err());
}

#[test]
fn custom_layout_options_round_trip_override_and_bound_offsets() {
    let original = json!({"mode":"image_text","text_align":"center"});
    let old: CardLayoutConfig = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(serde_json::to_value(&old).unwrap(), original);
    let custom = json!({"image_position":"left","image_share_percent":40,"text_vertical_align":"bottom","text_offset_x_percent":-5,"text_offset_y_percent":7});
    let config: CardLayoutConfig = serde_json::from_value(custom.clone()).unwrap();
    config.validate().unwrap();
    assert_eq!(serde_json::to_value(&config).unwrap(), custom);
    let merged = old.with_overrides(Some(&config));
    assert_eq!(merged.mode, Some(CardLayoutMode::ImageText));
    assert_eq!(merged.text_offset_x_percent, Some(-5));
    for value in [
        json!({"image_share_percent":19}),
        json!({"image_share_percent":81}),
        json!({"text_offset_x_percent":31}),
        json!({"text_offset_y_percent":-31}),
    ] {
        assert!(
            serde_json::from_value::<CardLayoutConfig>(value)
                .unwrap()
                .validate()
                .is_err()
        );
    }
}
