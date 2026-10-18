//! Stable creative contract for deterministic alpha-aware raster blur.

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

use crate::{DomainError, ImageColorProfile, ImageRasterContract};

/// Stable creative Operator identity projected into the Scene graph.
pub const IMAGE_BLUR_OPERATOR_TYPE: &str = "image.blur";
/// First exact JSON parameter and numerical algorithm contract.
pub const IMAGE_BLUR_PARAMETERS_REVISION: &str = "20260812.1";
/// The single input and output port data contract for this Operator.
pub const IMAGE_BLUR_DATA_TYPE: &str = "image.raster";
/// Largest authored blur radius accepted by the portable CPU implementation.
pub const IMAGE_BLUR_MAX_RADIUS: u16 = 64;
/// Largest pixel count admitted by the allocation-heavy portable blur path.
pub const IMAGE_BLUR_MAX_PIXELS: u64 = 16_777_216;

/// A violation of the narrow `image.blur` planning contract.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ImageBlurContractError {
    #[error("image.blur input is not a canonical image.raster contract")]
    InvalidSourceContract,
    #[error("image.blur currently requires the canonical sRGB color contract")]
    UnsupportedColorProfile,
    #[error("image.blur input exceeds the {maximum_pixels}-pixel working-set limit")]
    SourceTooLarge { maximum_pixels: u64 },
    #[error(transparent)]
    Domain(#[from] DomainError),
}

/// Authored radius for Shape's versioned three-box Gaussian approximation.
///
/// Physical execution converts sRGB to linear light, premultiplies alpha, runs
/// three separable box passes with clamped edges and fixed-point round-half-up,
/// then returns straight-alpha sRGB. These rules are part of revision
/// `20260812.1`, not an implementation detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RasterGaussianBlur {
    radius: u16,
}

impl<'de> Deserialize<'de> for RasterGaussianBlur {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireBlur {
            radius: u16,
        }

        let wire = WireBlur::deserialize(deserializer)?;
        Self::new(wire.radius).map_err(D::Error::custom)
    }
}

impl RasterGaussianBlur {
    /// Creates a non-identity blur within the portable support bound.
    ///
    /// # Errors
    ///
    /// Rejects radius zero or a radius above 64 pixels.
    pub fn new(radius: u16) -> Result<Self, DomainError> {
        if radius == 0 || radius > IMAGE_BLUR_MAX_RADIUS {
            return Err(DomainError::InvalidRasterBlurRadius {
                radius,
                maximum: IMAGE_BLUR_MAX_RADIUS,
            });
        }
        Ok(Self { radius })
    }

    #[must_use]
    pub const fn radius(self) -> u16 {
        self.radius
    }
}

/// Semantic owner for `image.blur` planning and boundary validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageBlurOperator;

impl ImageBlurOperator {
    pub const TYPE_ID: &'static str = IMAGE_BLUR_OPERATOR_TYPE;
    pub const PARAMETERS_REVISION: &'static str = IMAGE_BLUR_PARAMETERS_REVISION;
    pub const INPUT_COUNT: usize = 1;
    pub const OUTPUT_COUNT: usize = 1;
    pub const INPUT_DATA_TYPE: &'static str = IMAGE_BLUR_DATA_TYPE;
    pub const OUTPUT_DATA_TYPE: &'static str = IMAGE_BLUR_DATA_TYPE;

    /// Prepares one alpha-aware blur without changing the raster contract.
    ///
    /// # Errors
    ///
    /// Rejects noncanonical rasters and embedded profiles until Shape has a
    /// color transform owner capable of producing the required linear working
    /// space instead of merely preserving ICC bytes.
    pub fn prepare(
        parameters: RasterGaussianBlur,
        source: &ImageRasterContract,
    ) -> Result<PreparedImageBlur, ImageBlurContractError> {
        let canonical =
            ImageRasterContract::rgba8(source.width, source.height, source.color_profile.clone())?;
        if source != &canonical {
            return Err(ImageBlurContractError::InvalidSourceContract);
        }
        if source.color_profile != ImageColorProfile::Srgb {
            return Err(ImageBlurContractError::UnsupportedColorProfile);
        }
        if u64::from(source.width) * u64::from(source.height) > IMAGE_BLUR_MAX_PIXELS {
            return Err(ImageBlurContractError::SourceTooLarge {
                maximum_pixels: IMAGE_BLUR_MAX_PIXELS,
            });
        }
        Ok(PreparedImageBlur {
            parameters,
            source_contract: source.clone(),
            output_contract: source.clone(),
        })
    }
}

/// Opaque validated blur plan consumed unchanged by physical execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedImageBlur {
    parameters: RasterGaussianBlur,
    source_contract: ImageRasterContract,
    output_contract: ImageRasterContract,
}

impl PreparedImageBlur {
    #[must_use]
    pub const fn parameters(&self) -> RasterGaussianBlur {
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
