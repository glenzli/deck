use super::*;
#[test]
fn exact_bounded_text_only_contract() {
    for kind in [
        SoundGenerationKind::SoundEffect,
        SoundGenerationKind::ShortMusic,
    ] {
        for duration in [1, 30] {
            let p =
                SoundGenerationOperation::new("Rain on glass", kind, duration, u32::MAX).unwrap();
            assert_eq!(
                serde_json::from_str::<SoundGenerationOperation>(
                    &serde_json::to_string(&p).unwrap()
                )
                .unwrap(),
                p
            );
        }
    }
    for prompt in ["", "  ", "rain\nwind", "rain\0wind"] {
        assert!(
            SoundGenerationOperation::new(prompt, SoundGenerationKind::SoundEffect, 5, 0).is_err()
        );
    }
    for duration in [0, 31] {
        assert!(
            SoundGenerationOperation::new("Rain", SoundGenerationKind::SoundEffect, duration, 0)
                .is_err()
        );
    }
    let mut p =
        SoundGenerationOperation::new("Rain", SoundGenerationKind::SoundEffect, 5, 42).unwrap();
    p.revision = "future".into();
    assert!(p.validate().is_err());
    assert!(
        crate::AudioOperatorFamily::Generate
            .contract()
            .inputs
            .is_empty()
    );
}
