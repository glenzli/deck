use crate::{
    ContentDigest, DomainError, IMAGE_DROP_SHADOW_MAX_BLUR_RADIUS, IMAGE_DROP_SHADOW_MAX_DIMENSION,
    IMAGE_DROP_SHADOW_MAX_OFFSET, IMAGE_DROP_SHADOW_MAX_PIXELS, ImageColorProfile,
    ImageDropShadowContractError, ImageDropShadowOperator, ImageRasterContract, RasterDropShadow,
    RasterShadowColor,
};

fn color() -> RasterShadowColor {
    RasterShadowColor::new(12, 24, 48, 128).unwrap()
}

#[test]
fn exact_wire_contract_revalidates_nested_color_and_parameters() {
    let shadow = RasterDropShadow::new(-4, 6, 3, color()).unwrap();
    assert_eq!(
        serde_json::to_string(&shadow).unwrap(),
        r#"{"offset_x":-4,"offset_y":6,"blur_radius":3,"color":{"red":12,"green":24,"blue":48,"alpha":128}}"#
    );
    assert_eq!(
        serde_json::from_str::<RasterDropShadow>(&serde_json::to_string(&shadow).unwrap()).unwrap(),
        shadow
    );
    assert!(serde_json::from_str::<RasterDropShadow>(r#"{"offset_x":0,"offset_y":0,"blur_radius":1,"color":{"red":0,"green":0,"blue":0,"alpha":0}}"#).is_err());
    assert!(serde_json::from_str::<RasterDropShadow>(r#"{"offset_x":0,"offset_y":0,"blur_radius":1,"color":{"red":0,"green":0,"blue":0,"alpha":128},"future":true}"#).is_err());
}

#[test]
fn parameter_bounds_reject_invisible_or_nonportable_shadows() {
    assert_eq!(
        RasterShadowColor::new(0, 0, 0, 0).unwrap_err(),
        DomainError::InvalidRasterDropShadow {
            maximum_offset: IMAGE_DROP_SHADOW_MAX_OFFSET,
            maximum_radius: IMAGE_DROP_SHADOW_MAX_BLUR_RADIUS,
        }
    );
    assert!(RasterDropShadow::new(4097, 0, 1, color()).is_err());
    assert!(RasterDropShadow::new(0, 0, 65, color()).is_err());
    assert!(RasterDropShadow::new(i32::MIN, 0, 1, color()).is_err());
}

#[test]
fn preparation_expands_to_the_complete_shifted_blur_support() {
    let source = ImageRasterContract::rgba8(10, 6, ImageColorProfile::Srgb).unwrap();
    let prepared = ImageDropShadowOperator::prepare(
        RasterDropShadow::new(-4, 5, 2, color()).unwrap(),
        &source,
    )
    .unwrap();

    assert_eq!(
        (
            prepared.output_contract().width,
            prepared.output_contract().height
        ),
        (22, 18)
    );
    assert_eq!(prepared.source_origin(), (10, 1));
    assert_eq!(prepared.shadow_origin(), (6, 6));
}

#[test]
fn embedded_profiles_and_excessive_output_fail_closed() {
    let embedded = ImageRasterContract::rgba8(
        10,
        6,
        ImageColorProfile::EmbeddedIcc {
            digest: ContentDigest::from_bytes(b"profile"),
        },
    )
    .unwrap();
    assert_eq!(
        ImageDropShadowOperator::prepare(
            RasterDropShadow::new(1, 1, 2, color()).unwrap(),
            &embedded,
        )
        .unwrap_err(),
        ImageDropShadowContractError::UnsupportedColorProfile
    );

    let large = ImageRasterContract::rgba8(4096, 4096, ImageColorProfile::Srgb).unwrap();
    assert_eq!(
        ImageDropShadowOperator::prepare(
            RasterDropShadow::new(4096, 4096, 64, color()).unwrap(),
            &large,
        )
        .unwrap_err(),
        ImageDropShadowContractError::OutputTooLarge {
            maximum_dimension: IMAGE_DROP_SHADOW_MAX_DIMENSION,
            maximum_pixels: IMAGE_DROP_SHADOW_MAX_PIXELS,
        }
    );
}
