use crate::{
    ContentDigest, DomainError, IMAGE_BLUR_MAX_PIXELS, IMAGE_BLUR_MAX_RADIUS,
    ImageBlurContractError, ImageBlurOperator, ImageColorProfile, ImageRasterContract,
    RasterGaussianBlur,
};

#[test]
fn radius_wire_contract_is_bounded_and_exact() {
    let blur = RasterGaussianBlur::new(12).unwrap();
    assert_eq!(serde_json::to_string(&blur).unwrap(), r#"{"radius":12}"#);
    assert_eq!(
        serde_json::from_str::<RasterGaussianBlur>(r#"{"radius":12}"#).unwrap(),
        blur
    );
    assert!(serde_json::from_str::<RasterGaussianBlur>(r#"{"radius":12,"future":true}"#).is_err());
    assert_eq!(
        RasterGaussianBlur::new(0).unwrap_err(),
        DomainError::InvalidRasterBlurRadius {
            radius: 0,
            maximum: IMAGE_BLUR_MAX_RADIUS,
        }
    );
    assert!(RasterGaussianBlur::new(IMAGE_BLUR_MAX_RADIUS + 1).is_err());
}

#[test]
fn srgb_blur_preserves_the_exact_raster_contract() {
    let source = ImageRasterContract::rgba8(17, 11, ImageColorProfile::Srgb).unwrap();
    let prepared =
        ImageBlurOperator::prepare(RasterGaussianBlur::new(3).unwrap(), &source).unwrap();

    assert_eq!(prepared.source_contract(), &source);
    assert_eq!(prepared.output_contract(), &source);
}

#[test]
fn embedded_profiles_fail_until_a_real_color_transform_exists() {
    let source = ImageRasterContract::rgba8(
        17,
        11,
        ImageColorProfile::EmbeddedIcc {
            digest: ContentDigest::from_bytes(b"profile"),
        },
    )
    .unwrap();

    assert_eq!(
        ImageBlurOperator::prepare(RasterGaussianBlur::new(3).unwrap(), &source).unwrap_err(),
        ImageBlurContractError::UnsupportedColorProfile
    );
}

#[test]
fn noncanonical_alpha_contract_fails_closed() {
    let mut source = ImageRasterContract::rgba8(17, 11, ImageColorProfile::Srgb).unwrap();
    source.premultiplied = true;

    assert_eq!(
        ImageBlurOperator::prepare(RasterGaussianBlur::new(3).unwrap(), &source).unwrap_err(),
        ImageBlurContractError::InvalidSourceContract
    );
}

#[test]
fn four_k_square_is_admitted_and_eight_k_square_is_rejected_before_allocation() {
    let four_k = ImageRasterContract::rgba8(4096, 4096, ImageColorProfile::Srgb).unwrap();
    assert!(ImageBlurOperator::prepare(RasterGaussianBlur::new(64).unwrap(), &four_k).is_ok());

    let eight_k = ImageRasterContract::rgba8(8192, 8192, ImageColorProfile::Srgb).unwrap();
    assert_eq!(
        ImageBlurOperator::prepare(RasterGaussianBlur::new(64).unwrap(), &eight_k).unwrap_err(),
        ImageBlurContractError::SourceTooLarge {
            maximum_pixels: IMAGE_BLUR_MAX_PIXELS,
        }
    );
}
