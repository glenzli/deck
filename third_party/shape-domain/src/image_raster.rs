//! Portable flattened-raster metadata and validation.

use serde::{Deserialize, Serialize};

use crate::{ContentDigest, DomainError};

/// First durable Shape raster contract revision.
pub const IMAGE_RASTER_CONTRACT_REVISION: &str = "20260811.1";
const MAX_RASTER_DIMENSION: u32 = 131_072;

/// Canonical pixel layout currently accepted by Shape's M0 raster slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImagePixelFormat {
    /// Interleaved red, green, blue, and straight-alpha 8-bit channels.
    Rgba8,
}

/// Meaning of the alpha channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageAlphaMode {
    Straight,
}

/// Pixel row/column orientation after import normalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageOrientation {
    TopLeft,
}

/// Declared RGB primaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageColorPrimaries {
    Srgb,
    ProfileDefined,
}

/// Declared electro-optical transfer function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageTransferFunction {
    Srgb,
    ProfileDefined,
}

/// Stable color-profile identity. Embedded bytes remain inside the materialized PNG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImageColorProfile {
    Srgb,
    EmbeddedIcc { digest: ContentDigest },
}

/// Whether values describe display output or scene-linear light.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageLightReference {
    DisplayReferred,
}

/// Explicit contract for one immutable flattened raster payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageRasterContract {
    pub schema_revision: String,
    pub width: u32,
    pub height: u32,
    pub pixel_format: ImagePixelFormat,
    pub bit_depth: u8,
    pub alpha_mode: ImageAlphaMode,
    pub orientation: ImageOrientation,
    pub color_primaries: ImageColorPrimaries,
    pub transfer_function: ImageTransferFunction,
    pub color_profile: ImageColorProfile,
    pub light_reference: ImageLightReference,
    pub premultiplied: bool,
}

impl ImageRasterContract {
    /// Creates the normalized M0 RGBA8 raster contract.
    ///
    /// # Errors
    ///
    /// Rejects zero or non-portable dimensions.
    pub fn rgba8(
        width: u32,
        height: u32,
        color_profile: ImageColorProfile,
    ) -> Result<Self, DomainError> {
        if width == 0
            || height == 0
            || width > MAX_RASTER_DIMENSION
            || height > MAX_RASTER_DIMENSION
        {
            return Err(DomainError::InvalidRasterDimensions {
                width,
                height,
                maximum: MAX_RASTER_DIMENSION,
            });
        }
        let profile_defined = matches!(color_profile, ImageColorProfile::EmbeddedIcc { .. });
        Ok(Self {
            schema_revision: IMAGE_RASTER_CONTRACT_REVISION.to_owned(),
            width,
            height,
            pixel_format: ImagePixelFormat::Rgba8,
            bit_depth: 8,
            alpha_mode: ImageAlphaMode::Straight,
            orientation: ImageOrientation::TopLeft,
            color_primaries: if profile_defined {
                ImageColorPrimaries::ProfileDefined
            } else {
                ImageColorPrimaries::Srgb
            },
            transfer_function: if profile_defined {
                ImageTransferFunction::ProfileDefined
            } else {
                ImageTransferFunction::Srgb
            },
            color_profile,
            light_reference: ImageLightReference::DisplayReferred,
            premultiplied: false,
        })
    }
}

#[cfg(test)]
mod tests;
