use super::*;
use crate::{ImageColorProfile, ImageRasterContract};

#[test]
fn parameters_are_exact_bounded_and_revalidated_after_deserialization() {
    let parameters = RasterUnsharpMask::new(12, 1_250, 8).unwrap();
    let json = serde_json::to_string(&parameters).unwrap();
    assert_eq!(json, r#"{"radius":12,"amount_milli":1250,"threshold":8}"#);
    assert_eq!(
        serde_json::from_str::<RasterUnsharpMask>(&json).unwrap(),
        parameters
    );
    assert!(RasterUnsharpMask::new(0, 1_000, 0).is_err());
    assert!(RasterUnsharpMask::new(65, 1_000, 0).is_err());
    assert!(RasterUnsharpMask::new(1, 0, 0).is_err());
    assert!(RasterUnsharpMask::new(1, 4_001, 0).is_err());
    assert!(
        serde_json::from_str::<RasterUnsharpMask>(
            r#"{"radius":1,"amount_milli":1000,"threshold":0,"future":true}"#
        )
        .is_err()
    );
}

#[test]
fn preparation_preserves_the_exact_srgb_raster_contract() {
    let source = ImageRasterContract::rgba8(4096, 2160, ImageColorProfile::Srgb).unwrap();
    let prepared =
        ImageUnsharpMaskOperator::prepare(RasterUnsharpMask::new(3, 1_000, 4).unwrap(), &source)
            .unwrap();
    assert_eq!(prepared.source_contract(), &source);
    assert_eq!(prepared.output_contract(), &source);
}

#[test]
fn embedded_profiles_and_noncanonical_alpha_fail_closed() {
    let embedded = ImageRasterContract::rgba8(
        1024,
        768,
        ImageColorProfile::EmbeddedIcc {
            digest: crate::ContentDigest::from_bytes(b"icc"),
        },
    )
    .unwrap();
    assert_eq!(
        ImageUnsharpMaskOperator::prepare(RasterUnsharpMask::new(2, 1_000, 0).unwrap(), &embedded,)
            .unwrap_err(),
        ImageUnsharpMaskContractError::UnsupportedColorProfile
    );

    let mut noncanonical = ImageRasterContract::rgba8(1024, 768, ImageColorProfile::Srgb).unwrap();
    noncanonical.premultiplied = true;
    assert_eq!(
        ImageUnsharpMaskOperator::prepare(
            RasterUnsharpMask::new(2, 1_000, 0).unwrap(),
            &noncanonical,
        )
        .unwrap_err(),
        ImageUnsharpMaskContractError::InvalidSourceContract
    );
}

#[test]
fn four_k_square_is_admitted_and_eight_k_square_is_rejected_before_allocation() {
    let parameters = RasterUnsharpMask::new(64, 4_000, u8::MAX).unwrap();
    let four_k = ImageRasterContract::rgba8(4096, 4096, ImageColorProfile::Srgb).unwrap();
    assert!(ImageUnsharpMaskOperator::prepare(parameters, &four_k).is_ok());

    let eight_k = ImageRasterContract::rgba8(8192, 8192, ImageColorProfile::Srgb).unwrap();
    assert_eq!(
        ImageUnsharpMaskOperator::prepare(parameters, &eight_k).unwrap_err(),
        ImageUnsharpMaskContractError::SourceTooLarge {
            maximum_pixels: IMAGE_UNSHARP_MASK_MAX_PIXELS,
        }
    );
}
