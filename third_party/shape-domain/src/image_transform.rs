//! Stable creative contract for deterministic raster orientation transforms.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{DomainError, ImageRasterContract};

/// Stable creative Operator identity projected into the Scene graph.
pub const IMAGE_TRANSFORM_OPERATOR_TYPE: &str = "image.transform";
/// First exact JSON parameter contract accepted by the Operator executor.
pub const IMAGE_TRANSFORM_PARAMETERS_REVISION: &str = "20260812.1";
/// The single input and output port data contract for this Operator.
pub const IMAGE_TRANSFORM_DATA_TYPE: &str = "image.raster";

/// A violation of the narrow `image.transform` planning contract.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ImageTransformContractError {
    #[error("image.transform input is not a canonical image.raster contract")]
    InvalidSourceContract,
    #[error(transparent)]
    Domain(#[from] DomainError),
}

/// Exact lossless pixel rearrangement applied to a normalized top-left raster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RasterTransform {
    Rotate90Clockwise,
    Rotate180,
    Rotate270Clockwise,
    FlipHorizontal,
    FlipVertical,
}

impl RasterTransform {
    #[must_use]
    pub const fn swaps_dimensions(self) -> bool {
        matches!(self, Self::Rotate90Clockwise | Self::Rotate270Clockwise)
    }
}

/// Semantic owner for `image.transform` planning and boundary validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageTransformOperator;

impl ImageTransformOperator {
    pub const TYPE_ID: &'static str = IMAGE_TRANSFORM_OPERATOR_TYPE;
    pub const PARAMETERS_REVISION: &'static str = IMAGE_TRANSFORM_PARAMETERS_REVISION;
    pub const INPUT_COUNT: usize = 1;
    pub const OUTPUT_COUNT: usize = 1;
    pub const INPUT_DATA_TYPE: &'static str = IMAGE_TRANSFORM_DATA_TYPE;
    pub const OUTPUT_DATA_TYPE: &'static str = IMAGE_TRANSFORM_DATA_TYPE;

    /// Prepares one validated lossless transform and its exact output contract.
    ///
    /// # Errors
    ///
    /// Rejects a source that is not Shape's canonical RGBA8 top-left raster.
    pub fn prepare(
        parameters: RasterTransform,
        source: &ImageRasterContract,
    ) -> Result<PreparedImageTransform, ImageTransformContractError> {
        let canonical =
            ImageRasterContract::rgba8(source.width, source.height, source.color_profile.clone())?;
        if source != &canonical {
            return Err(ImageTransformContractError::InvalidSourceContract);
        }
        let (width, height) = if parameters.swaps_dimensions() {
            (source.height, source.width)
        } else {
            (source.width, source.height)
        };
        let output_contract =
            ImageRasterContract::rgba8(width, height, source.color_profile.clone())?;
        Ok(PreparedImageTransform {
            parameters,
            source_contract: source.clone(),
            output_contract,
        })
    }
}

/// Opaque validated plan shared by orchestration and physical execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedImageTransform {
    parameters: RasterTransform,
    source_contract: ImageRasterContract,
    output_contract: ImageRasterContract,
}

impl PreparedImageTransform {
    #[must_use]
    pub const fn parameters(&self) -> RasterTransform {
        self.parameters
    }

    #[must_use]
    pub const fn source_contract(&self) -> &ImageRasterContract {
        &self.source_contract
    }

    #[must_use]
    pub const fn output_contract(&self) -> &ImageRasterContract {
        &self.output_contract
    }
}

#[cfg(test)]
mod tests;
