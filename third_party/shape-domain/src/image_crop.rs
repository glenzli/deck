//! Stable creative contract for the built-in `image.crop` Operator.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{DomainError, ImageRasterContract};

/// Stable creative Operator identity projected into the Scene graph.
pub const IMAGE_CROP_OPERATOR_TYPE: &str = "image.crop";
/// First exact JSON parameter contract accepted by the Operator executor.
pub const IMAGE_CROP_PARAMETERS_REVISION: &str = "20260811.1";
/// The single input and output port data contract for this Operator.
pub const IMAGE_CROP_DATA_TYPE: &str = "image.raster";

/// A violation of the narrow `image.crop` planning contract.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ImageCropContractError {
    #[error("image.crop input is not a canonical image.raster contract")]
    InvalidSourceContract,
    #[error(transparent)]
    Domain(#[from] DomainError),
}

/// Pixel-space parameters authored against one exact accepted raster revision.
///
/// Coordinates use the normalized top-left raster orientation. The rectangle
/// includes `x..x + width` and `y..y + height`; zero-sized, overflowing, and
/// out-of-bounds rectangles are rejected. Unknown serialized fields are also
/// rejected so executor behavior cannot drift behind a newer caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RasterCrop {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl RasterCrop {
    /// Validates parameters against exact source dimensions.
    ///
    /// # Errors
    ///
    /// Rejects empty, overflowing, or out-of-bounds rectangles.
    pub fn new(
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        source_width: u32,
        source_height: u32,
    ) -> Result<Self, DomainError> {
        let parameters = Self {
            x,
            y,
            width,
            height,
        };
        parameters.validate_dimensions(source_width, source_height)?;
        Ok(parameters)
    }

    fn validate_dimensions(self, source_width: u32, source_height: u32) -> Result<(), DomainError> {
        let right = self.x.checked_add(self.width);
        let bottom = self.y.checked_add(self.height);
        if self.width == 0
            || self.height == 0
            || right.is_none_or(|value| value > source_width)
            || bottom.is_none_or(|value| value > source_height)
        {
            return Err(DomainError::InvalidRasterCrop {
                x: self.x,
                y: self.y,
                width: self.width,
                height: self.height,
                source_width,
                source_height,
            });
        }
        Ok(())
    }
}

/// Semantic owner for `image.crop` planning and boundary validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageCropOperator;

impl ImageCropOperator {
    pub const TYPE_ID: &'static str = IMAGE_CROP_OPERATOR_TYPE;
    pub const PARAMETERS_REVISION: &'static str = IMAGE_CROP_PARAMETERS_REVISION;
    pub const INPUT_COUNT: usize = 1;
    pub const OUTPUT_COUNT: usize = 1;
    pub const INPUT_DATA_TYPE: &'static str = IMAGE_CROP_DATA_TYPE;
    pub const OUTPUT_DATA_TYPE: &'static str = IMAGE_CROP_DATA_TYPE;

    /// Prepares one validated crop and derives its exact output contract.
    ///
    /// Crop preserves the canonical pixel layout, alpha, orientation, color,
    /// ICC identity, light reference, and premultiplication semantics. Only the
    /// output dimensions change.
    ///
    /// # Errors
    ///
    /// Rejects a non-canonical source contract or invalid rectangle.
    pub fn prepare(
        parameters: RasterCrop,
        source: &ImageRasterContract,
    ) -> Result<PreparedImageCrop, ImageCropContractError> {
        let canonical =
            ImageRasterContract::rgba8(source.width, source.height, source.color_profile.clone())?;
        if source != &canonical {
            return Err(ImageCropContractError::InvalidSourceContract);
        }
        parameters.validate_dimensions(source.width, source.height)?;
        let output_contract = ImageRasterContract::rgba8(
            parameters.width,
            parameters.height,
            source.color_profile.clone(),
        )?;
        Ok(PreparedImageCrop {
            parameters,
            source_width: source.width,
            source_height: source.height,
            output_contract,
        })
    }
}

/// Validated ephemeral plan shared by creative orchestration and pixel execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedImageCrop {
    parameters: RasterCrop,
    source_width: u32,
    source_height: u32,
    output_contract: ImageRasterContract,
}

impl PreparedImageCrop {
    #[must_use]
    pub const fn parameters(&self) -> RasterCrop {
        self.parameters
    }

    #[must_use]
    pub const fn output_contract(&self) -> &ImageRasterContract {
        &self.output_contract
    }

    /// Reports a full-frame crop that has no creative effect.
    #[must_use]
    pub const fn is_identity(&self) -> bool {
        self.parameters.x == 0
            && self.parameters.y == 0
            && self.parameters.width == self.source_width
            && self.parameters.height == self.source_height
    }
}

#[cfg(test)]
mod tests;
