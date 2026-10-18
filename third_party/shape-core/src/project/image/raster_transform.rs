//! Candidate orchestration for deterministic `image.transform`.

use shape_domain::{
    ArtifactContentContract, ArtifactId, ArtifactKind, ArtifactRevision, ImageRasterContract,
    ImageTransformContractError, ImageTransformOperator, IntentSpec, RasterTransform, RevisionId,
    Transformation, TransformationKind, TransformationOperation,
};
use shape_execution::{
    CapabilityId, ExecutedCandidate, ExecutionCoordinator, ExecutionInput, ExecutionRequest,
    RASTER_TRANSFORM_CAPABILITY, RasterTransformExecutor,
};

use super::{ImageEditCandidate, RASTER_MEDIA_TYPE, ShapeProject, output_contract};
use crate::CoreError;

impl ShapeProject {
    /// Produces a lossless orientation-transform Candidate without advancing history.
    ///
    /// # Errors
    ///
    /// Rejects unknown, stale, non-raster, noncanonical, or output-mismatched input.
    pub fn propose_raster_transform(
        &self,
        artifact_id: ArtifactId,
        expected_head: RevisionId,
        parameters: RasterTransform,
    ) -> Result<ImageEditCandidate, CoreError> {
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
        let prepared =
            ImageTransformOperator::prepare(parameters, source_contract).map_err(|error| {
                match error {
                    ImageTransformContractError::InvalidSourceContract => {
                        CoreError::InvalidRasterContent { artifact_id }
                    }
                    ImageTransformContractError::Domain(error) => CoreError::Domain(error),
                }
            })?;
        let expected_output_contract = prepared.output_contract().clone();
        let transformation = Transformation::new_with_operation(
            TransformationKind::DeterministicEdit,
            artifact_id,
            vec![expected_head],
            IntentSpec::new(intent(parameters))?,
            Vec::new(),
            Vec::new(),
            Some(TransformationOperation::RasterTransform(parameters)),
        )?;
        let input = ExecutionInput::materialized(accepted.revision.content, accepted.bytes)?;
        let request = ExecutionRequest::new_materialized(
            transformation.id,
            CapabilityId::new(RASTER_TRANSFORM_CAPABILITY)?,
            vec![input],
            Vec::new(),
            RASTER_MEDIA_TYPE,
        )?;
        let ExecutedCandidate { output, receipt } =
            ExecutionCoordinator::execute(&RasterTransformExecutor::new(prepared)?, &request)?;
        let output_contract = output_contract(output.content_contract.clone())?;
        if output_contract != expected_output_contract {
            return Err(CoreError::RasterEditOutputContractMismatch);
        }
        Ok(ImageEditCandidate::new(
            artifact_id,
            expected_head,
            transformation,
            receipt,
            output,
            output_contract,
        ))
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

const fn intent(parameters: RasterTransform) -> &'static str {
    match parameters {
        RasterTransform::Rotate90Clockwise => "Rotate raster 90 degrees clockwise",
        RasterTransform::Rotate180 => "Rotate raster 180 degrees",
        RasterTransform::Rotate270Clockwise => "Rotate raster 270 degrees clockwise",
        RasterTransform::FlipHorizontal => "Flip raster horizontally",
        RasterTransform::FlipVertical => "Flip raster vertically",
    }
}

#[cfg(test)]
mod tests;
