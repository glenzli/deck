use crate::{
    ContentDigest, DomainError, IMAGE_RESIZE_DATA_TYPE, IMAGE_RESIZE_MAX_DIMENSION,
    IMAGE_RESIZE_MAX_PIXELS, IMAGE_RESIZE_OPERATOR_TYPE, IMAGE_RESIZE_PARAMETERS_REVISION,
    ImageColorProfile, ImageRasterContract, ImageResizeContractError, ImageResizeOperator,
    RasterResize, RasterResizeAspectPolicy, RasterResizeDimensions, RasterResizeResampling,
};

fn resize(width: u32, height: u32, aspect_policy: RasterResizeAspectPolicy) -> RasterResize {
    RasterResize::new(
        RasterResizeDimensions::new(width, height).unwrap(),
        aspect_policy,
        RasterResizeResampling::Lanczos3,
    )
}

#[test]
fn contract_is_typed_versioned_and_exactly_serialized() {
    assert_eq!(ImageResizeOperator::TYPE_ID, IMAGE_RESIZE_OPERATOR_TYPE);
    assert_eq!(
        ImageResizeOperator::PARAMETERS_REVISION,
        IMAGE_RESIZE_PARAMETERS_REVISION
    );
    assert_eq!(ImageResizeOperator::INPUT_COUNT, 1);
    assert_eq!(ImageResizeOperator::OUTPUT_COUNT, 1);
    assert_eq!(ImageResizeOperator::INPUT_DATA_TYPE, IMAGE_RESIZE_DATA_TYPE);
    assert_eq!(
        ImageResizeOperator::OUTPUT_DATA_TYPE,
        IMAGE_RESIZE_DATA_TYPE
    );

    let parameters = resize(1920, 1080, RasterResizeAspectPolicy::FitWithin);
    assert_eq!(
        serde_json::to_string(&parameters).unwrap(),
        r#"{"target":{"width":1920,"height":1080},"aspect_policy":"fit_within","resampling":"lanczos3"}"#
    );
    assert!(
        serde_json::from_str::<RasterResize>(
            r#"{"target":{"width":1920,"height":1080},"aspect_policy":"fit_within","resampling":"lanczos3","future":true}"#
        )
        .is_err()
    );
}

#[test]
fn dimension_and_pixel_boundaries_fail_closed() {
    let boundary = RasterResizeDimensions::new(8192, 8192).unwrap();
    assert_eq!(boundary.pixel_count(), IMAGE_RESIZE_MAX_PIXELS);
    assert!(RasterResizeDimensions::new(IMAGE_RESIZE_MAX_DIMENSION, 1).is_ok());
    assert!(RasterResizeDimensions::new(IMAGE_RESIZE_MAX_DIMENSION + 1, 1).is_err());
    assert_eq!(
        RasterResizeDimensions::new(8193, 8192).unwrap_err(),
        DomainError::InvalidRasterResizeDimensions {
            width: 8193,
            height: 8192,
            maximum_dimension: IMAGE_RESIZE_MAX_DIMENSION,
            maximum_pixels: IMAGE_RESIZE_MAX_PIXELS,
        }
    );
    assert!(RasterResizeDimensions::new(0, 1).is_err());
    assert!(
        serde_json::from_str::<RasterResizeDimensions>(r#"{"width":8193,"height":8192}"#).is_err()
    );
}

#[test]
fn fit_within_freezes_odd_dimension_rounding_and_preserves_interpretation() {
    let source = ImageRasterContract::rgba8(
        7,
        5,
        ImageColorProfile::EmbeddedIcc {
            digest: ContentDigest::from_bytes(b"profile"),
        },
    )
    .unwrap();
    let prepared =
        ImageResizeOperator::prepare(resize(4, 4, RasterResizeAspectPolicy::FitWithin), &source)
            .unwrap();
    assert_eq!(
        (
            prepared.output_dimensions().width(),
            prepared.output_dimensions().height()
        ),
        (4, 3)
    );
    let mut expected = source.clone();
    expected.width = 4;
    expected.height = 3;
    assert_eq!(prepared.output_contract(), &expected);
}

#[test]
fn prepare_identifies_identity_and_rejects_contract_mismatch() {
    let source = ImageRasterContract::rgba8(7, 5, ImageColorProfile::Srgb).unwrap();
    let identity =
        ImageResizeOperator::prepare(resize(7, 5, RasterResizeAspectPolicy::Stretch), &source)
            .unwrap();
    assert!(identity.is_identity());

    let fit_identity =
        ImageResizeOperator::prepare(resize(8, 5, RasterResizeAspectPolicy::FitWithin), &source)
            .unwrap();
    assert!(fit_identity.is_identity());

    let mut invalid = source;
    invalid.premultiplied = true;
    assert_eq!(
        ImageResizeOperator::prepare(resize(4, 3, RasterResizeAspectPolicy::Stretch), &invalid,)
            .unwrap_err(),
        ImageResizeContractError::InvalidSourceContract
    );
}
