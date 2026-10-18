//! Candidate orchestration for deterministic flattened `image.drop_shadow`.

use shape_domain::{
    ArtifactContentContract, ArtifactId, ArtifactKind, ArtifactRevision,
    ImageDropShadowContractError, ImageDropShadowOperator, ImageRasterContract, IntentSpec,
    RasterDropShadow, RevisionId, Transformation, TransformationKind, TransformationOperation,
};
use shape_execution::{
    CapabilityId, ExecutedCandidate, ExecutionCoordinator, ExecutionInput, ExecutionRequest,
    RASTER_DROP_SHADOW_CAPABILITY, RasterDropShadowExecutor,
};

use super::{ImageEditCandidate, RASTER_MEDIA_TYPE, ShapeProject, output_contract};
use crate::CoreError;

impl ShapeProject {
    /// Produces an expanded flattened drop-shadow Candidate without advancing history.
    ///
    /// # Errors
    ///
    /// Rejects unknown, stale, non-raster, non-sRGB, oversized, noncanonical,
    /// or output-mismatched input.
    pub fn propose_raster_drop_shadow(
        &self,
        artifact_id: ArtifactId,
        expected_head: RevisionId,
        parameters: RasterDropShadow,
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
            ImageDropShadowOperator::prepare(parameters, source_contract).map_err(|error| {
                match error {
                    ImageDropShadowContractError::InvalidSourceContract => {
                        CoreError::InvalidRasterContent { artifact_id }
                    }
                    ImageDropShadowContractError::UnsupportedColorProfile => {
                        CoreError::RasterDropShadowUnsupportedColorProfile
                    }
                    ImageDropShadowContractError::OutputTooLarge {
                        maximum_dimension,
                        maximum_pixels,
                    } => CoreError::RasterDropShadowOutputTooLarge {
                        maximum_dimension,
                        maximum_pixels,
                    },
                    ImageDropShadowContractError::Domain(error) => CoreError::Domain(error),
                }
            })?;
        let expected_output_contract = prepared.output_contract().clone();
        let transformation = Transformation::new_with_operation(
            TransformationKind::DeterministicEdit,
            artifact_id,
            vec![expected_head],
            IntentSpec::new(format!(
                "Add raster drop shadow at x={}, y={}, blur radius={}",
                parameters.offset_x(),
                parameters.offset_y(),
                parameters.blur_radius()
            ))?,
            Vec::new(),
            Vec::new(),
            Some(TransformationOperation::RasterDropShadow(parameters)),
        )?;
        let input = ExecutionInput::materialized(accepted.revision.content, accepted.bytes)?;
        let request = ExecutionRequest::new_materialized(
            transformation.id,
            CapabilityId::new(RASTER_DROP_SHADOW_CAPABILITY)?,
            vec![input],
            Vec::new(),
            RASTER_MEDIA_TYPE,
        )?;
        let ExecutedCandidate { output, receipt } =
            ExecutionCoordinator::execute(&RasterDropShadowExecutor::new(prepared)?, &request)?;
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
