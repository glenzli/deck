//! Candidate orchestration for deterministic alpha-aware `image.blur`.

use shape_domain::{
    ArtifactContentContract, ArtifactId, ArtifactKind, ArtifactRevision, ImageBlurContractError,
    ImageBlurOperator, ImageRasterContract, IntentSpec, RasterGaussianBlur, RevisionId,
    Transformation, TransformationKind, TransformationOperation,
};
use shape_execution::{
    CapabilityId, ExecutedCandidate, ExecutionCoordinator, ExecutionInput, ExecutionRequest,
    RASTER_BLUR_CAPABILITY, RasterBlurExecutor,
};

use super::{ImageEditCandidate, RASTER_MEDIA_TYPE, ShapeProject, output_contract};
use crate::CoreError;

impl ShapeProject {
    /// Produces a fixed-point alpha-aware Blur Candidate without advancing history.
    ///
    /// # Errors
    ///
    /// Rejects unknown, stale, non-raster, non-sRGB, oversized, noncanonical,
    /// or output-mismatched input.
    pub fn propose_raster_blur(
        &self,
        artifact_id: ArtifactId,
        expected_head: RevisionId,
        parameters: RasterGaussianBlur,
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
        let prepared = ImageBlurOperator::prepare(parameters, source_contract).map_err(
            |error| match error {
                ImageBlurContractError::InvalidSourceContract => {
                    CoreError::InvalidRasterContent { artifact_id }
                }
                ImageBlurContractError::UnsupportedColorProfile => {
                    CoreError::RasterBlurUnsupportedColorProfile
                }
                ImageBlurContractError::SourceTooLarge { maximum_pixels } => {
                    CoreError::RasterBlurSourceTooLarge { maximum_pixels }
                }
                ImageBlurContractError::Domain(error) => CoreError::Domain(error),
            },
        )?;
        let expected_output_contract = prepared.output_contract().clone();
        let transformation = Transformation::new_with_operation(
            TransformationKind::DeterministicEdit,
            artifact_id,
            vec![expected_head],
            IntentSpec::new(format!("Blur raster with radius={}", parameters.radius()))?,
            Vec::new(),
            Vec::new(),
            Some(TransformationOperation::RasterGaussianBlur(parameters)),
        )?;
        let input = ExecutionInput::materialized(accepted.revision.content, accepted.bytes)?;
        let request = ExecutionRequest::new_materialized(
            transformation.id,
            CapabilityId::new(RASTER_BLUR_CAPABILITY)?,
            vec![input],
            Vec::new(),
            RASTER_MEDIA_TYPE,
        )?;
        let ExecutedCandidate { output, receipt } =
            ExecutionCoordinator::execute(&RasterBlurExecutor::new(prepared)?, &request)?;
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

#[cfg(test)]
mod tests;
