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

fn block(id: &str) -> serde_json::Value {
    json!({"id":id,"field":"title","x":0,"y":0,"width":10000,"height":10000,
        "text_scale_percent":100,"text_align":"left","text_vertical_align":"top", "bold":false,"italic":false,"text_color":"#123456","image_fit":"contain"})
}
#[test]
fn blocks_are_bounded_strict_layers_and_override_as_one_list_without_changing_old_bytes() {
    let old: CardLayoutConfig =
        serde_json::from_value(json!({"mode":"text","padding_percent":4})).unwrap();
    assert_eq!(
        serde_json::to_value(&old).unwrap(),
        json!({"mode":"text","padding_percent":4})
    );
    let config: CardLayoutConfig =
        serde_json::from_value(json!({"mode":"blocks","blocks":[block("a"),block("b")]})).unwrap();
    config.validate().unwrap();
    let empty: CardLayoutConfig = serde_json::from_value(json!({"blocks":[]})).unwrap();
    assert_eq!(config.with_overrides(Some(&empty)).blocks, Some(Vec::new()));
    let switched: CardLayoutConfig = serde_json::from_value(json!({"mode":"image"})).unwrap();
    assert_eq!(config.with_overrides(Some(&switched)).blocks, config.blocks);
    assert_eq!(
        config.with_overrides(Some(&switched)).mode,
        Some(CardLayoutMode::Image)
    );
    for (key, value) in [
        ("id", json!("../bad")),
        ("x", json!(10001)),
        ("width", json!(0)),
        ("height", json!(10001)),
        ("text_scale_percent", json!(49)),
        ("text_color", json!("red")),
    ] {
        let mut bad = block("valid");
        bad[key] = value;
        assert!(
            serde_json::from_value::<CardBlock>(bad)
                .unwrap()
                .validate()
                .is_err()
        );
    }
    let mut outside = block("outside");
    outside["x"] = json!(1);
    assert!(
        serde_json::from_value::<CardBlock>(outside)
            .unwrap()
            .validate()
            .is_err()
    );
    for (key, value) in [
        ("field", json!("unknown")),
        ("text_vertical_align", json!("auto")),
        ("image_fit", json!("stretch")),
        ("x", json!(0.5)),
        ("unexpected", json!(true)),
    ] {
        let mut bad = block("valid");
        bad[key] = value;
        assert!(serde_json::from_value::<CardBlock>(bad).is_err());
    }
    let mut missing = block("a");
    missing.as_object_mut().unwrap().remove("bold");
    assert!(serde_json::from_value::<CardBlock>(missing).is_err());
    let duplicate: CardLayoutConfig =
        serde_json::from_value(json!({"blocks":[block("a"),block("a")]})).unwrap();
    assert!(duplicate.validate().is_err());
    let excessive: CardLayoutConfig = serde_json::from_value(
        json!({"blocks":(0..17).map(|n|block(&format!("b{n}"))).collect::<Vec<_>>()}),
    )
    .unwrap();
    assert!(excessive.validate().is_err());
}
