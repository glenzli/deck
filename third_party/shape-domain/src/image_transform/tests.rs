use crate::{
    ImageColorProfile, ImageRasterContract, ImageTransformContractError, ImageTransformOperator,
    RasterTransform,
};

#[test]
fn exact_wire_identity_rejects_unknown_transforms() {
    assert_eq!(
        serde_json::to_string(&RasterTransform::Rotate90Clockwise).unwrap(),
        r#""rotate90_clockwise""#
    );
    assert!(serde_json::from_str::<RasterTransform>(r#""transpose""#).is_err());
}

#[test]
fn quarter_turns_swap_dimensions_and_preserve_color_identity() {
    let source = ImageRasterContract::rgba8(7, 5, ImageColorProfile::Srgb).unwrap();
    let prepared =
        ImageTransformOperator::prepare(RasterTransform::Rotate270Clockwise, &source).unwrap();

    assert_eq!(prepared.source_contract(), &source);
    assert_eq!(prepared.output_contract().width, 5);
    assert_eq!(prepared.output_contract().height, 7);
    assert_eq!(
        prepared.output_contract().color_profile,
        ImageColorProfile::Srgb
    );
}

#[test]
fn flips_keep_dimensions_and_embedded_profile_identity() {
    let profile = ImageColorProfile::EmbeddedIcc {
        digest: crate::ContentDigest::from_bytes(b"profile"),
    };
    let source = ImageRasterContract::rgba8(7, 5, profile.clone()).unwrap();
    let prepared =
        ImageTransformOperator::prepare(RasterTransform::FlipHorizontal, &source).unwrap();

    assert_eq!(prepared.output_contract().width, 7);
    assert_eq!(prepared.output_contract().height, 5);
    assert_eq!(prepared.output_contract().color_profile, profile);
}

#[test]
fn noncanonical_source_fails_closed() {
    let mut source = ImageRasterContract::rgba8(7, 5, ImageColorProfile::Srgb).unwrap();
    source.premultiplied = true;

    assert_eq!(
        ImageTransformOperator::prepare(RasterTransform::Rotate180, &source).unwrap_err(),
        ImageTransformContractError::InvalidSourceContract
    );
}
