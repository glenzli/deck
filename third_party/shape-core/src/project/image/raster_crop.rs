//! Candidate and acceptance orchestration for the deterministic `image.crop` Operator.

use std::{fmt, sync::Arc};

use shape_domain::{
    ArtifactContentContract, ArtifactId, ArtifactKind, ArtifactRevision, ImageCropContractError,
    ImageCropOperator, ImageRasterContract, IntentSpec, RasterCrop, RevisionId, Transformation,
    TransformationKind, TransformationOperation,
};
use shape_execution::{
    CapabilityId, ExecutedCandidate, ExecutionCoordinator, ExecutionInput, ExecutionReceipt,
    ExecutionRequest, RASTER_CROP_CAPABILITY, RasterCropExecutor,
};
use shape_store::AcceptedCommit;

use super::{RASTER_MEDIA_TYPE, ShapeProject, output_contract};
use crate::CoreError;

/// Transient canonical PNG produced by the deterministic Raster Crop Operator.
#[derive(Clone)]
pub struct ImageCandidate {
    artifact_id: ArtifactId,
    expected_head: RevisionId,
    transformation: Transformation,
    receipt: ExecutionReceipt,
    output_bytes: Arc<[u8]>,
    output_media_type: String,
    output_contract: ImageRasterContract,
}

impl fmt::Debug for ImageCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImageCandidate")
            .field("artifact_id", &self.artifact_id)
            .field("expected_head", &self.expected_head)
            .field("transformation_id", &self.transformation.id)
            .field("receipt", &self.receipt)
            .field("output_byte_length", &self.output_bytes.len())
            .field("output_media_type", &self.output_media_type)
            .field("output_contract", &self.output_contract)
            .finish()
    }
}

impl ImageCandidate {
    #[must_use]
    pub const fn artifact_id(&self) -> ArtifactId {
        self.artifact_id
    }

    #[must_use]
    pub const fn expected_head(&self) -> RevisionId {
        self.expected_head
    }

    #[must_use]
    pub const fn contract(&self) -> &ImageRasterContract {
        &self.output_contract
    }

    #[must_use]
    pub fn png_bytes(&self) -> &[u8] {
        &self.output_bytes
    }

    #[must_use]
    pub const fn receipt(&self) -> &ExecutionReceipt {
        &self.receipt
    }

    #[must_use]
    pub const fn operation(&self) -> Option<&TransformationOperation> {
        self.transformation.operation.as_ref()
    }
}

impl ShapeProject {
    /// Produces a deterministic crop Candidate without advancing durable history.
    ///
    /// # Errors
    ///
    /// Rejects unknown, stale, non-raster, full-frame, non-canonical, or
    /// out-of-bounds requests. Pixel-executor success remains transient.
    pub fn propose_raster_crop(
        &self,
        artifact_id: ArtifactId,
        expected_head: RevisionId,
        parameters: RasterCrop,
    ) -> Result<ImageCandidate, CoreError> {
        let artifact = self
            .store
            .artifact(artifact_id)?
            .ok_or(shape_store::StoreError::UnknownArtifact(artifact_id))?;
        if artifact.kind != ArtifactKind::ImageRaster {
            return Err(CoreError::InvalidRasterContent { artifact_id });
        }
        if artifact.accepted_revision != Some(expected_head) {
            return Err(CoreError::StaleCandidate {
                artifact_id,
                expected: Some(expected_head),
                actual: artifact.accepted_revision,
            });
        }
        let accepted =
            self.read_accepted(artifact_id)?
                .ok_or(CoreError::MissingAcceptedRevision {
                    artifact_id,
                    revision_id: expected_head,
                })?;
        let source_contract = raster_contract(&accepted.revision, artifact_id)?;
        let prepared = ImageCropOperator::prepare(parameters, source_contract).map_err(
            |error| match error {
                ImageCropContractError::InvalidSourceContract => {
                    CoreError::InvalidRasterContent { artifact_id }
                }
                ImageCropContractError::Domain(error) => CoreError::Domain(error),
            },
        )?;
        if prepared.is_identity() {
            return Err(CoreError::NoOpRasterCrop);
        }
        let parameters = prepared.parameters();
        let expected_output_contract = prepared.output_contract().clone();
        let transformation = Transformation::new_with_operation(
            TransformationKind::DeterministicEdit,
            artifact_id,
            vec![expected_head],
            IntentSpec::new(format!(
                "Crop raster to x={}, y={}, width={}, height={}",
                parameters.x, parameters.y, parameters.width, parameters.height
            ))?,
            Vec::new(),
            Vec::new(),
            Some(TransformationOperation::RasterCrop(parameters)),
        )?;
        let input = ExecutionInput::materialized(accepted.revision.content, accepted.bytes)?;
        let request = ExecutionRequest::new_materialized(
            transformation.id,
            CapabilityId::new(RASTER_CROP_CAPABILITY)?,
            vec![input],
            serde_json::to_vec(&parameters).map_err(shape_store::StoreError::Json)?,
            RASTER_MEDIA_TYPE,
        )?;
        let ExecutedCandidate { output, receipt } =
            ExecutionCoordinator::execute(&RasterCropExecutor::new()?, &request)?;
        let output_contract = output_contract(output.content_contract)?;
        if output_contract != expected_output_contract {
            return Err(CoreError::RasterCropOutputContractMismatch);
        }
        Ok(ImageCandidate {
            artifact_id,
            expected_head,
            transformation,
            receipt,
            output_bytes: output.bytes.into(),
            output_media_type: output.media_type,
            output_contract,
        })
    }

    /// Explicitly accepts one exact Raster Crop Candidate.
    ///
    /// # Errors
    ///
    /// Returns an error when its expected head is stale or publication fails.
    pub fn accept_raster_crop(
        &mut self,
        candidate: ImageCandidate,
    ) -> Result<ArtifactRevision, CoreError> {
        let contract = candidate.output_contract.clone();
        Ok(self.store.accept(AcceptedCommit {
            artifact_id: candidate.artifact_id,
            expected_head: Some(candidate.expected_head),
            transformation: candidate.transformation,
            receipt: candidate.receipt,
            output_bytes: candidate.output_bytes,
            output_media_type: candidate.output_media_type,
            content_contract: Some(ArtifactContentContract::ImageRaster(contract)),
        })?)
    }
}

fn raster_contract(
    revision: &ArtifactRevision,
    artifact_id: ArtifactId,
) -> Result<&ImageRasterContract, CoreError> {
    match revision.content_contract.as_ref() {
        Some(ArtifactContentContract::ImageRaster(contract)) => Ok(contract),
        Some(_) | None => Err(CoreError::InvalidRasterContent { artifact_id }),
    }
}

#[cfg(test)]
mod tests;
