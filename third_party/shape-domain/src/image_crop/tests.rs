use crate::{
    ContentDigest, DomainError, IMAGE_CROP_DATA_TYPE, IMAGE_CROP_OPERATOR_TYPE,
    IMAGE_CROP_PARAMETERS_REVISION, ImageColorProfile, ImageCropContractError, ImageCropOperator,
    ImageRasterContract, RasterCrop,
};

#[test]
fn parameters_are_an_exact_bounded_pixel_rectangle_contract() {
    assert_eq!(ImageCropOperator::TYPE_ID, IMAGE_CROP_OPERATOR_TYPE);
    assert_eq!(
        ImageCropOperator::PARAMETERS_REVISION,
        IMAGE_CROP_PARAMETERS_REVISION
    );
    assert_eq!(ImageCropOperator::INPUT_COUNT, 1);
    assert_eq!(ImageCropOperator::OUTPUT_COUNT, 1);
    assert_eq!(ImageCropOperator::INPUT_DATA_TYPE, IMAGE_CROP_DATA_TYPE);
    assert_eq!(ImageCropOperator::OUTPUT_DATA_TYPE, IMAGE_CROP_DATA_TYPE);
}

#[test]
fn crop_rejects_empty_overflowing_and_out_of_bounds_regions() {
    assert_eq!(
        RasterCrop::new(0, 0, 0, 10, 100, 100).unwrap_err(),
        DomainError::InvalidRasterCrop {
            x: 0,
            y: 0,
            width: 0,
            height: 10,
            source_width: 100,
            source_height: 100,
        }
    );
    assert!(RasterCrop::new(90, 0, 11, 10, 100, 100).is_err());
    assert!(RasterCrop::new(u32::MAX, 0, 2, 10, u32::MAX, 100).is_err());
}

#[test]
fn prepare_preserves_raster_interpretation_and_identifies_noop() {
    let source = ImageRasterContract::rgba8(
        100,
        80,
        ImageColorProfile::EmbeddedIcc {
            digest: ContentDigest::from_bytes(b"test profile"),
        },
    )
    .unwrap();
    let prepared =
        ImageCropOperator::prepare(RasterCrop::new(10, 20, 30, 40, 100, 80).unwrap(), &source)
            .unwrap();
    let mut expected = source.clone();
    expected.width = 30;
    expected.height = 40;
    assert_eq!(prepared.output_contract(), &expected);
    assert!(!prepared.is_identity());

    let identity =
        ImageCropOperator::prepare(RasterCrop::new(0, 0, 100, 80, 100, 80).unwrap(), &source)
            .unwrap();
    assert!(identity.is_identity());
}

#[test]
fn prepare_rejects_noncanonical_input_contract() {
    let mut source = ImageRasterContract::rgba8(100, 80, ImageColorProfile::Srgb).unwrap();
    source.premultiplied = true;
    assert_eq!(
        ImageCropOperator::prepare(RasterCrop::new(0, 0, 50, 40, 100, 80).unwrap(), &source)
            .unwrap_err(),
        ImageCropContractError::InvalidSourceContract
    );
}
