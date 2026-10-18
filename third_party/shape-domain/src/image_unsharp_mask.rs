//! Stable creative contract for deterministic alpha-aware unsharp masking.

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

use crate::{DomainError, ImageColorProfile, ImageRasterContract};

/// Stable creative Operator identity projected into the Scene graph.
pub const IMAGE_UNSHARP_MASK_OPERATOR_TYPE: &str = "image.unsharp_mask";
/// First exact JSON parameter and numerical algorithm contract.
pub const IMAGE_UNSHARP_MASK_PARAMETERS_REVISION: &str = "20260813.1";
/// The single input and output port data contract for this Operator.
pub const IMAGE_UNSHARP_MASK_DATA_TYPE: &str = "image.raster";
/// Largest authored blur radius used to derive the low-frequency image.
pub const IMAGE_UNSHARP_MASK_MAX_RADIUS: u16 = 64;
/// Largest sharpening amount, where 1000 means 100 percent.
pub const IMAGE_UNSHARP_MASK_MAX_AMOUNT_MILLI: u16 = 4_000;
/// Largest pixel count admitted by the allocation-heavy portable path.
pub const IMAGE_UNSHARP_MASK_MAX_PIXELS: u64 = 16_777_216;

/// A violation of the narrow `image.unsharp_mask` planning contract.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ImageUnsharpMaskContractError {
    #[error("image.unsharp_mask input is not a canonical image.raster contract")]
    InvalidSourceContract,
    #[error("image.unsharp_mask currently requires the canonical sRGB color contract")]
    UnsupportedColorProfile,
    #[error("image.unsharp_mask input exceeds the {maximum_pixels}-pixel working-set limit")]
    SourceTooLarge { maximum_pixels: u64 },
    #[error(transparent)]
    Domain(#[from] DomainError),
}

/// Authored parameters for Shape's versioned unsharp-mask implementation.
///
/// Radius selects the same three-box Gaussian approximation as `image.blur`.
/// Amount is fixed-point thousandths (`1000 == 100%`). Threshold is expanded
/// from 8-bit to 16-bit linear-premultiplied channel distance before the
/// sharpened delta is admitted. Source alpha is preserved exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RasterUnsharpMask {
    radius: u16,
    amount_milli: u16,
    threshold: u8,
}

impl<'de> Deserialize<'de> for RasterUnsharpMask {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireUnsharpMask {
            radius: u16,
            amount_milli: u16,
            threshold: u8,
        }

        let wire = WireUnsharpMask::deserialize(deserializer)?;
        Self::new(wire.radius, wire.amount_milli, wire.threshold).map_err(D::Error::custom)
    }
}

impl RasterUnsharpMask {
    /// Creates one bounded, non-identity unsharp-mask parameter set.
    ///
    /// # Errors
    ///
    /// Rejects radius zero or above 64 and amount zero or above 400 percent.
    pub fn new(radius: u16, amount_milli: u16, threshold: u8) -> Result<Self, DomainError> {
        if radius == 0
            || radius > IMAGE_UNSHARP_MASK_MAX_RADIUS
            || amount_milli == 0
            || amount_milli > IMAGE_UNSHARP_MASK_MAX_AMOUNT_MILLI
        {
            return Err(DomainError::InvalidRasterUnsharpMask {
                radius,
                amount_milli,
                maximum_radius: IMAGE_UNSHARP_MASK_MAX_RADIUS,
                maximum_amount_milli: IMAGE_UNSHARP_MASK_MAX_AMOUNT_MILLI,
            });
        }
        Ok(Self {
            radius,
            amount_milli,
            threshold,
        })
    }

    #[must_use]
    pub const fn radius(self) -> u16 {
        self.radius
    }

    #[must_use]
    pub const fn amount_milli(self) -> u16 {
        self.amount_milli
    }

    #[must_use]
    pub const fn threshold(self) -> u8 {
        self.threshold
    }
}

/// Semantic owner for `image.unsharp_mask` planning and boundary validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageUnsharpMaskOperator;

impl ImageUnsharpMaskOperator {
    pub const TYPE_ID: &'static str = IMAGE_UNSHARP_MASK_OPERATOR_TYPE;
    pub const PARAMETERS_REVISION: &'static str = IMAGE_UNSHARP_MASK_PARAMETERS_REVISION;
    pub const INPUT_COUNT: usize = 1;
    pub const OUTPUT_COUNT: usize = 1;
    pub const INPUT_DATA_TYPE: &'static str = IMAGE_UNSHARP_MASK_DATA_TYPE;
    pub const OUTPUT_DATA_TYPE: &'static str = IMAGE_UNSHARP_MASK_DATA_TYPE;

    /// Prepares one alpha-aware unsharp mask without changing raster geometry.
    ///
    /// # Errors
    ///
    /// Rejects noncanonical rasters, embedded profiles, and sources beyond the
    /// portable allocation budget before materialized execution begins.
    pub fn prepare(
        parameters: RasterUnsharpMask,
        source: &ImageRasterContract,
    ) -> Result<PreparedImageUnsharpMask, ImageUnsharpMaskContractError> {
        let canonical =
            ImageRasterContract::rgba8(source.width, source.height, source.color_profile.clone())?;
        if source != &canonical {
            return Err(ImageUnsharpMaskContractError::InvalidSourceContract);
        }
        if source.color_profile != ImageColorProfile::Srgb {
            return Err(ImageUnsharpMaskContractError::UnsupportedColorProfile);
        }
        if u64::from(source.width) * u64::from(source.height) > IMAGE_UNSHARP_MASK_MAX_PIXELS {
            return Err(ImageUnsharpMaskContractError::SourceTooLarge {
                maximum_pixels: IMAGE_UNSHARP_MASK_MAX_PIXELS,
            });
        }
        Ok(PreparedImageUnsharpMask {
            parameters,
            source_contract: source.clone(),
            output_contract: source.clone(),
        })
    }
}

/// Opaque validated unsharp-mask plan consumed unchanged by execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedImageUnsharpMask {
    parameters: RasterUnsharpMask,
    source_contract: ImageRasterContract,
    output_contract: ImageRasterContract,
}

impl PreparedImageUnsharpMask {
    #[must_use]
    pub const fn parameters(&self) -> RasterUnsharpMask {
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
