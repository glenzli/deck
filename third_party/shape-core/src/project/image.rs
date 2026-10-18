//! Canonical image-raster import and routing to image-specific use cases.

mod ai_generate;
mod raster_blur;
mod raster_crop;
mod raster_drop_shadow;
mod raster_edit;
mod raster_resize;
mod raster_transform;
mod raster_unsharp_mask;

pub use ai_generate::AiImageCandidate;
pub use raster_crop::ImageCandidate;
pub use raster_edit::ImageEditCandidate;
pub use raster_resize::ImageResizeCandidate;

use std::{fs::File, io::Read, path::Path};

use shape_domain::{
    Artifact, ArtifactContentContract, ArtifactKind, ArtifactRevision, ImageRasterContract,
    IntentSpec, Transformation, TransformationKind,
};
use shape_execution::{
    CapabilityId, ExecutedCandidate, ExecutionCoordinator, ExecutionRequest,
    RASTER_IMPORT_CAPABILITY, RasterExecutor,
};
use shape_store::NewArtifactCommit;

use super::ShapeProject;
use crate::CoreError;

const RASTER_MEDIA_TYPE: &str = "image/png";
const MAX_RASTER_SOURCE_BYTES: u64 = 128 * 1024 * 1024;
const IMPORT_INTENT: &str = "Import raster image";

impl ShapeProject {
    /// Imports PNG/JPEG bytes as a normalized, accepted `image.raster` artifact.
    ///
    /// Artifact, import Transformation, receipt, canonical PNG, and accepted
    /// revision are published atomically. Source paths are never persisted.
    ///
    /// # Errors
    ///
    /// Rejects non-regular, empty, oversized, unsupported, or unsafe image sources.
    pub fn import_raster(
        &mut self,
        source_path: impl AsRef<Path>,
        artifact_name: impl Into<String>,
    ) -> Result<ArtifactRevision, CoreError> {
        let source = read_source(source_path.as_ref())?;
        self.import_raster_bytes(source, artifact_name)
    }

    /// Imports PNG/JPEG bytes directly, including a clipboard snapshot, without
    /// creating or recording a synthetic source path.
    ///
    /// # Errors
    ///
    /// Rejects empty, oversized, unsupported, or unsafe image bytes.
    pub fn import_raster_bytes(
        &mut self,
        source: Vec<u8>,
        artifact_name: impl Into<String>,
    ) -> Result<ArtifactRevision, CoreError> {
        if source.is_empty() || source.len() as u64 > MAX_RASTER_SOURCE_BYTES {
            return Err(CoreError::InvalidRasterSource {
                maximum_bytes: MAX_RASTER_SOURCE_BYTES,
            });
        }
        let artifact = Artifact::new(artifact_name, ArtifactKind::ImageRaster)?;
        let transformation = Transformation::new(
            TransformationKind::Import,
            artifact.id,
            Vec::new(),
            IntentSpec::new(IMPORT_INTENT)?,
            Vec::new(),
            Vec::new(),
        )?;
        let request = ExecutionRequest::new(
            transformation.id,
            CapabilityId::new(RASTER_IMPORT_CAPABILITY)?,
            Vec::new(),
            source,
            RASTER_MEDIA_TYPE,
        )?;
        let ExecutedCandidate { output, receipt } =
            ExecutionCoordinator::execute(&RasterExecutor::new()?, &request)?;
        let contract = output_contract(output.content_contract)?;
        Ok(self.store.accept_new_artifact(NewArtifactCommit {
            artifact,
            expected_input_heads: Vec::new(),
            transformation,
            receipt,
            output_bytes: output.bytes.into(),
            output_media_type: output.media_type,
            content_contract: Some(ArtifactContentContract::ImageRaster(contract)),
        })?)
    }
}

fn read_source(path: &Path) -> Result<Vec<u8>, CoreError> {
    let file = File::open(path).map_err(source_io)?;
    let metadata = file.metadata().map_err(source_io)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_RASTER_SOURCE_BYTES {
        return Err(CoreError::InvalidRasterSource {
            maximum_bytes: MAX_RASTER_SOURCE_BYTES,
        });
    }
    let capacity = usize::try_from(metadata.len()).map_err(|_| CoreError::InvalidRasterSource {
        maximum_bytes: MAX_RASTER_SOURCE_BYTES,
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(MAX_RASTER_SOURCE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(source_io)?;
    if bytes.len() as u64 != metadata.len() {
        return Err(CoreError::InvalidRasterSource {
            maximum_bytes: MAX_RASTER_SOURCE_BYTES,
        });
    }
    Ok(bytes)
}

fn source_io(error: std::io::Error) -> CoreError {
    CoreError::Store(shape_store::StoreError::Io(error))
}

fn output_contract(
    contract: Option<ArtifactContentContract>,
) -> Result<ImageRasterContract, CoreError> {
    match contract {
        Some(ArtifactContentContract::ImageRaster(contract)) => Ok(contract),
        Some(_) | None => Err(CoreError::MissingRasterOutputContract),
    }
}

#[cfg(test)]
mod tests;
