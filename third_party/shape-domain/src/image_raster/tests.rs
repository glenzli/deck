use crate::{
    IMAGE_RASTER_CONTRACT_REVISION, ImageColorPrimaries, ImageColorProfile, ImageRasterContract,
    ImageTransferFunction,
};

#[test]
fn raster_contract_declares_normalized_display_pixels() {
    let contract = ImageRasterContract::rgba8(1920, 1080, ImageColorProfile::Srgb).unwrap();
    assert_eq!(contract.schema_revision, IMAGE_RASTER_CONTRACT_REVISION);
    assert_eq!(contract.bit_depth, 8);
    assert_eq!(contract.color_primaries, ImageColorPrimaries::Srgb);
    assert_eq!(contract.transfer_function, ImageTransferFunction::Srgb);
    assert!(!contract.premultiplied);
}
