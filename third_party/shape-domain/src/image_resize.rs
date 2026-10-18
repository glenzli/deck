//! Stable creative contract for the built-in `image.resize` Operator.

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

use crate::{DomainError, ImageRasterContract};

/// Stable creative Operator identity projected into the Scene graph.
pub const IMAGE_RESIZE_OPERATOR_TYPE: &str = "image.resize";
/// First exact JSON parameter contract accepted by the Operator executor.
pub const IMAGE_RESIZE_PARAMETERS_REVISION: &str = "20260811.1";
/// The single input and output port data contract for this Operator.
pub const IMAGE_RESIZE_DATA_TYPE: &str = "image.raster";
/// Largest portable dimension accepted by the built-in resize path.
pub const IMAGE_RESIZE_MAX_DIMENSION: u32 = 32_768;
/// Largest decoded or produced pixel count accepted by the built-in resize path.
pub const IMAGE_RESIZE_MAX_PIXELS: u64 = 64 * 1024 * 1024;

/// A violation of the narrow `image.resize` planning contract.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ImageResizeContractError {
    #[error("image.resize input is not a canonical image.raster contract")]
    InvalidSourceContract,
    #[error(transparent)]
    Domain(#[from] DomainError),
}

/// Bounded, non-empty pixel dimensions used by the resize contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RasterResizeDimensions {
    width: u32,
    height: u32,
}

impl<'de> Deserialize<'de> for RasterResizeDimensions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireDimensions {
            width: u32,
            height: u32,
        }

        let wire = WireDimensions::deserialize(deserializer)?;
        Self::new(wire.width, wire.height).map_err(D::Error::custom)
    }
}

impl RasterResizeDimensions {
    /// Creates dimensions inside both the per-axis and total-pixel limits.
    ///
    /// # Errors
    ///
    /// Rejects zero dimensions, dimensions above 32768, or more than 64 Mi pixels.
    pub fn new(width: u32, height: u32) -> Result<Self, DomainError> {
        validate_dimensions(width, height)?;
        Ok(Self { width, height })
    }

    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }

    #[must_use]
    pub const fn pixel_count(self) -> u64 {
        self.width as u64 * self.height as u64
    }
}

/// How authored target dimensions affect source aspect ratio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RasterResizeAspectPolicy {
    /// Produce the exact target width and height, even when aspect ratio changes.
    Stretch,
    /// Preserve aspect ratio inside the target box without padding or cropping.
    ///
    /// The unconstrained axis uses integer round-half-up and is clamped to at
    /// least one pixel. This rule is part of the versioned parameter contract.
    FitWithin,
}

/// Exact portable resampling kernel selected for physical execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RasterResizeResampling {
    Nearest,
    Triangle,
    CatmullRom,
    Lanczos3,
}

/// Authored parameters for one deterministic resize.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RasterResize {
    target: RasterResizeDimensions,
    aspect_policy: RasterResizeAspectPolicy,
    resampling: RasterResizeResampling,
}

impl RasterResize {
    #[must_use]
    pub const fn new(
        target: RasterResizeDimensions,
        aspect_policy: RasterResizeAspectPolicy,
        resampling: RasterResizeResampling,
    ) -> Self {
        Self {
            target,
            aspect_policy,
            resampling,
        }
    }

    #[must_use]
    pub const fn target(self) -> RasterResizeDimensions {
        self.target
    }

    #[must_use]
    pub const fn aspect_policy(self) -> RasterResizeAspectPolicy {
        self.aspect_policy
    }

    #[must_use]
    pub const fn resampling(self) -> RasterResizeResampling {
        self.resampling
    }
}

/// Semantic owner for `image.resize` planning and boundary validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageResizeOperator;

impl ImageResizeOperator {
    pub const TYPE_ID: &'static str = IMAGE_RESIZE_OPERATOR_TYPE;
    pub const PARAMETERS_REVISION: &'static str = IMAGE_RESIZE_PARAMETERS_REVISION;
    pub const INPUT_COUNT: usize = 1;
    pub const OUTPUT_COUNT: usize = 1;
    pub const INPUT_DATA_TYPE: &'static str = IMAGE_RESIZE_DATA_TYPE;
    pub const OUTPUT_DATA_TYPE: &'static str = IMAGE_RESIZE_DATA_TYPE;

    /// Prepares one validated resize and derives its exact output contract.
    ///
    /// Resize preserves canonical pixel layout, straight alpha, orientation,
    /// color interpretation, embedded ICC identity, light reference, and
    /// premultiplication semantics. Only pixel dimensions change.
    ///
    /// # Errors
    ///
    /// Rejects a non-canonical source or dimensions outside the physical bounds.
    pub fn prepare(
        parameters: RasterResize,
        source: &ImageRasterContract,
    ) -> Result<PreparedImageResize, ImageResizeContractError> {
        let canonical =
            ImageRasterContract::rgba8(source.width, source.height, source.color_profile.clone())?;
        if source != &canonical {
            return Err(ImageResizeContractError::InvalidSourceContract);
        }
        validate_dimensions(source.width, source.height)?;
        validate_dimensions(parameters.target.width, parameters.target.height)?;
        let output_dimensions = resolve_output_dimensions(parameters, source.width, source.height)?;
        let output_contract = ImageRasterContract::rgba8(
            output_dimensions.width,
            output_dimensions.height,
            source.color_profile.clone(),
        )?;
        Ok(PreparedImageResize {
            parameters,
            source_contract: source.clone(),
            output_dimensions,
            output_contract,
        })
    }
}

/// Opaque validated plan prepared once and consumed by physical execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedImageResize {
    parameters: RasterResize,
    source_contract: ImageRasterContract,
    output_dimensions: RasterResizeDimensions,
    output_contract: ImageRasterContract,
}

impl PreparedImageResize {
    #[must_use]
    pub const fn parameters(&self) -> RasterResize {
        self.parameters
    }

    #[must_use]
    pub const fn source_contract(&self) -> &ImageRasterContract {
        &self.source_contract
    }

    #[must_use]
    pub const fn output_dimensions(&self) -> RasterResizeDimensions {
        self.output_dimensions
    }

    #[must_use]
    pub const fn output_contract(&self) -> &ImageRasterContract {
        &self.output_contract
    }

    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.output_contract == self.source_contract
    }
}

fn validate_dimensions(width: u32, height: u32) -> Result<(), DomainError> {
    let pixels = u64::from(width) * u64::from(height);
    if width == 0
        || height == 0
        || width > IMAGE_RESIZE_MAX_DIMENSION
        || height > IMAGE_RESIZE_MAX_DIMENSION
        || pixels > IMAGE_RESIZE_MAX_PIXELS
    {
        return Err(DomainError::InvalidRasterResizeDimensions {
            width,
            height,
            maximum_dimension: IMAGE_RESIZE_MAX_DIMENSION,
            maximum_pixels: IMAGE_RESIZE_MAX_PIXELS,
        });
    }
    Ok(())
}

fn resolve_output_dimensions(
    parameters: RasterResize,
    source_width: u32,
    source_height: u32,
) -> Result<RasterResizeDimensions, DomainError> {
    if parameters.aspect_policy == RasterResizeAspectPolicy::Stretch {
        return Ok(parameters.target);
    }
    let target_width = parameters.target.width;
    let target_height = parameters.target.height;
    let width_limited = u64::from(target_width) * u64::from(source_height)
        <= u64::from(target_height) * u64::from(source_width);
    let (width, height) = if width_limited {
        (
            target_width,
            rounded_ratio(source_height, target_width, source_width),
        )
    } else {
        (
            rounded_ratio(source_width, target_height, source_height),
            target_height,
        )
    };
    RasterResizeDimensions::new(width, height)
}

fn rounded_ratio(value: u32, numerator: u32, denominator: u32) -> u32 {
    let scaled = u64::from(value) * u64::from(numerator);
    let rounded = (scaled + u64::from(denominator / 2)) / u64::from(denominator);
    u32::try_from(rounded.max(1)).expect("bounded resize dimensions fit u32")
}

#[cfg(test)]
mod tests;
