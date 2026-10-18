//! Stable creative contract for deterministic flattened raster drop shadows.

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

use crate::{DomainError, ImageColorProfile, ImageRasterContract};

/// Stable creative Operator identity projected into the Scene graph.
pub const IMAGE_DROP_SHADOW_OPERATOR_TYPE: &str = "image.drop_shadow";
/// First exact JSON parameter and numerical algorithm contract.
pub const IMAGE_DROP_SHADOW_PARAMETERS_REVISION: &str = "20260812.1";
/// The single input and output port data contract for this Operator.
pub const IMAGE_DROP_SHADOW_DATA_TYPE: &str = "image.raster";
/// Largest absolute authored shadow offset.
pub const IMAGE_DROP_SHADOW_MAX_OFFSET: u32 = 4_096;
/// Largest three-box blur radius used by a shadow.
pub const IMAGE_DROP_SHADOW_MAX_BLUR_RADIUS: u16 = 64;
/// Largest output dimension admitted by the portable compositor.
pub const IMAGE_DROP_SHADOW_MAX_DIMENSION: u32 = 32_768;
/// Largest output pixel count admitted by the allocation-heavy compositor.
pub const IMAGE_DROP_SHADOW_MAX_PIXELS: u64 = 16_777_216;

/// A violation of the narrow `image.drop_shadow` planning contract.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ImageDropShadowContractError {
    #[error("image.drop_shadow input is not a canonical image.raster contract")]
    InvalidSourceContract,
    #[error("image.drop_shadow currently requires the canonical sRGB color contract")]
    UnsupportedColorProfile,
    #[error("image.drop_shadow output exceeds {maximum_dimension}px or {maximum_pixels} pixels")]
    OutputTooLarge {
        maximum_dimension: u32,
        maximum_pixels: u64,
    },
    #[error(transparent)]
    Domain(#[from] DomainError),
}

/// Nontransparent sRGB tint applied to the blurred source alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RasterShadowColor {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl RasterShadowColor {
    /// Creates a visible sRGB shadow tint.
    ///
    /// # Errors
    ///
    /// Rejects a fully transparent color because it would produce no change.
    pub fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Result<Self, DomainError> {
        if alpha == 0 {
            return Err(DomainError::InvalidRasterDropShadow {
                maximum_offset: IMAGE_DROP_SHADOW_MAX_OFFSET,
                maximum_radius: IMAGE_DROP_SHADOW_MAX_BLUR_RADIUS,
            });
        }
        Ok(Self {
            red,
            green,
            blue,
            alpha,
        })
    }

    #[must_use]
    pub const fn red(self) -> u8 {
        self.red
    }

    #[must_use]
    pub const fn green(self) -> u8 {
        self.green
    }

    #[must_use]
    pub const fn blue(self) -> u8 {
        self.blue
    }

    #[must_use]
    pub const fn alpha(self) -> u8 {
        self.alpha
    }
}

/// Authored flattened drop-shadow parameters.
///
/// The three-box blur has support `3 * blur_radius`. Output bounds are the
/// union of the original raster and that complete shifted support region, so
/// accepted pixels are never silently clipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RasterDropShadow {
    offset_x: i32,
    offset_y: i32,
    blur_radius: u16,
    color: RasterShadowColor,
}

impl<'de> Deserialize<'de> for RasterDropShadow {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireShadow {
            offset_x: i32,
            offset_y: i32,
            blur_radius: u16,
            color: RasterShadowColor,
        }

        let wire = WireShadow::deserialize(deserializer)?;
        Self::new(
            wire.offset_x,
            wire.offset_y,
            wire.blur_radius,
            RasterShadowColor::new(
                wire.color.red,
                wire.color.green,
                wire.color.blue,
                wire.color.alpha,
            )
            .map_err(D::Error::custom)?,
        )
        .map_err(D::Error::custom)
    }
}

impl RasterDropShadow {
    /// Creates bounded visible drop-shadow parameters.
    ///
    /// # Errors
    ///
    /// Rejects offsets outside ±4096 or a blur radius above 64 pixels.
    pub fn new(
        offset_x: i32,
        offset_y: i32,
        blur_radius: u16,
        color: RasterShadowColor,
    ) -> Result<Self, DomainError> {
        if offset_x.unsigned_abs() > IMAGE_DROP_SHADOW_MAX_OFFSET
            || offset_y.unsigned_abs() > IMAGE_DROP_SHADOW_MAX_OFFSET
            || blur_radius > IMAGE_DROP_SHADOW_MAX_BLUR_RADIUS
        {
            return Err(DomainError::InvalidRasterDropShadow {
                maximum_offset: IMAGE_DROP_SHADOW_MAX_OFFSET,
                maximum_radius: IMAGE_DROP_SHADOW_MAX_BLUR_RADIUS,
            });
        }
        Ok(Self {
            offset_x,
            offset_y,
            blur_radius,
            color,
        })
    }

    #[must_use]
    pub const fn offset_x(self) -> i32 {
        self.offset_x
    }

    #[must_use]
    pub const fn offset_y(self) -> i32 {
        self.offset_y
    }

    #[must_use]
    pub const fn blur_radius(self) -> u16 {
        self.blur_radius
    }

    #[must_use]
    pub const fn color(self) -> RasterShadowColor {
        self.color
    }
}

/// Semantic owner for `image.drop_shadow` planning and output bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageDropShadowOperator;

impl ImageDropShadowOperator {
    pub const TYPE_ID: &'static str = IMAGE_DROP_SHADOW_OPERATOR_TYPE;
    pub const PARAMETERS_REVISION: &'static str = IMAGE_DROP_SHADOW_PARAMETERS_REVISION;
    pub const INPUT_COUNT: usize = 1;
    pub const OUTPUT_COUNT: usize = 1;
    pub const INPUT_DATA_TYPE: &'static str = IMAGE_DROP_SHADOW_DATA_TYPE;
    pub const OUTPUT_DATA_TYPE: &'static str = IMAGE_DROP_SHADOW_DATA_TYPE;

    /// Prepares exact expanded output bounds for one flattened shadow.
    ///
    /// # Errors
    ///
    /// Rejects noncanonical or non-sRGB sources and output bounds beyond the
    /// portable allocation budget.
    pub fn prepare(
        parameters: RasterDropShadow,
        source: &ImageRasterContract,
    ) -> Result<PreparedImageDropShadow, ImageDropShadowContractError> {
        let canonical =
            ImageRasterContract::rgba8(source.width, source.height, source.color_profile.clone())?;
        if source != &canonical {
            return Err(ImageDropShadowContractError::InvalidSourceContract);
        }
        if source.color_profile != ImageColorProfile::Srgb {
            return Err(ImageDropShadowContractError::UnsupportedColorProfile);
        }

        let support = i64::from(parameters.blur_radius) * 3;
        let shadow_min_x = i64::from(parameters.offset_x) - support;
        let shadow_min_y = i64::from(parameters.offset_y) - support;
        let shadow_max_x = i64::from(parameters.offset_x) + i64::from(source.width) + support;
        let shadow_max_y = i64::from(parameters.offset_y) + i64::from(source.height) + support;
        let canvas_min_x = shadow_min_x.min(0);
        let canvas_min_y = shadow_min_y.min(0);
        let canvas_max_x = shadow_max_x.max(i64::from(source.width));
        let canvas_max_y = shadow_max_y.max(i64::from(source.height));
        let width = u32::try_from(canvas_max_x - canvas_min_x).map_err(|_| output_too_large())?;
        let height = u32::try_from(canvas_max_y - canvas_min_y).map_err(|_| output_too_large())?;
        let pixels = u64::from(width) * u64::from(height);
        if width > IMAGE_DROP_SHADOW_MAX_DIMENSION
            || height > IMAGE_DROP_SHADOW_MAX_DIMENSION
            || pixels > IMAGE_DROP_SHADOW_MAX_PIXELS
        {
            return Err(output_too_large());
        }
        let source_origin = (
            u32::try_from(-canvas_min_x).map_err(|_| output_too_large())?,
            u32::try_from(-canvas_min_y).map_err(|_| output_too_large())?,
        );
        let shadow_origin = (
            u32::try_from(i64::from(parameters.offset_x) - canvas_min_x)
                .map_err(|_| output_too_large())?,
            u32::try_from(i64::from(parameters.offset_y) - canvas_min_y)
                .map_err(|_| output_too_large())?,
        );
        let output_contract = ImageRasterContract::rgba8(width, height, ImageColorProfile::Srgb)?;
        Ok(PreparedImageDropShadow {
            parameters,
            source_contract: source.clone(),
            output_contract,
            source_origin,
            shadow_origin,
        })
    }
}

/// Opaque validated shadow plan consumed unchanged by physical execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedImageDropShadow {
    parameters: RasterDropShadow,
    source_contract: ImageRasterContract,
    output_contract: ImageRasterContract,
    source_origin: (u32, u32),
    shadow_origin: (u32, u32),
}

impl PreparedImageDropShadow {
    #[must_use]
    pub const fn parameters(&self) -> RasterDropShadow {
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

    #[must_use]
    pub const fn source_origin(&self) -> (u32, u32) {
        self.source_origin
    }

    #[must_use]
    pub const fn shadow_origin(&self) -> (u32, u32) {
        self.shadow_origin
    }
}

const fn output_too_large() -> ImageDropShadowContractError {
    ImageDropShadowContractError::OutputTooLarge {
        maximum_dimension: IMAGE_DROP_SHADOW_MAX_DIMENSION,
        maximum_pixels: IMAGE_DROP_SHADOW_MAX_PIXELS,
    }
}

#[cfg(test)]
mod tests;
