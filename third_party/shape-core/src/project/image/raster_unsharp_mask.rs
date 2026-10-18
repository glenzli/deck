//! Candidate orchestration for deterministic alpha-aware `image.unsharp_mask`.

use shape_domain::{
    ArtifactContentContract, ArtifactId, ArtifactKind, ArtifactRevision, ImageRasterContract,
    ImageUnsharpMaskContractError, ImageUnsharpMaskOperator, IntentSpec, RasterUnsharpMask,
    RevisionId, Transformation, TransformationKind, TransformationOperation,
};
use shape_execution::{
    CapabilityId, ExecutedCandidate, ExecutionCoordinator, ExecutionInput, ExecutionRequest,
    RASTER_UNSHARP_MASK_CAPABILITY, RasterUnsharpMaskExecutor,
};

use super::{ImageEditCandidate, RASTER_MEDIA_TYPE, ShapeProject, output_contract};
use crate::CoreError;

impl ShapeProject {
    /// Produces one fixed-point alpha-aware Unsharp Mask Candidate.
    ///
    /// # Errors
    ///
    /// Rejects unknown, stale, non-raster, non-sRGB, oversized, noncanonical,
    /// or output-mismatched input without advancing accepted history.
    pub fn propose_raster_unsharp_mask(
        &self,
        artifact_id: ArtifactId,
        expected_head: RevisionId,
        parameters: RasterUnsharpMask,
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
            ImageUnsharpMaskOperator::prepare(parameters, source_contract).map_err(|error| {
                match error {
                    ImageUnsharpMaskContractError::InvalidSourceContract => {
                        CoreError::InvalidRasterContent { artifact_id }
                    }
                    ImageUnsharpMaskContractError::UnsupportedColorProfile => {
                        CoreError::RasterUnsharpMaskUnsupportedColorProfile
                    }
                    ImageUnsharpMaskContractError::SourceTooLarge { maximum_pixels } => {
                        CoreError::RasterUnsharpMaskSourceTooLarge { maximum_pixels }
                    }
                    ImageUnsharpMaskContractError::Domain(error) => CoreError::Domain(error),
                }
            })?;
        let expected_output_contract = prepared.output_contract().clone();
        let transformation = Transformation::new_with_operation(
            TransformationKind::DeterministicEdit,
            artifact_id,
            vec![expected_head],
            IntentSpec::new(format!(
                "Sharpen raster with radius={}, amount_milli={}, threshold={}",
                parameters.radius(),
                parameters.amount_milli(),
                parameters.threshold()
            ))?,
            Vec::new(),
            Vec::new(),
            Some(TransformationOperation::RasterUnsharpMask(parameters)),
        )?;
        let input = ExecutionInput::materialized(accepted.revision.content, accepted.bytes)?;
        let request = ExecutionRequest::new_materialized(
            transformation.id,
            CapabilityId::new(RASTER_UNSHARP_MASK_CAPABILITY)?,
            vec![input],
            Vec::new(),
            RASTER_MEDIA_TYPE,
        )?;
        let ExecutedCandidate { output, receipt } =
            ExecutionCoordinator::execute(&RasterUnsharpMaskExecutor::new(prepared)?, &request)?;
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
