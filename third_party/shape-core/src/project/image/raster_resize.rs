//! Candidate and acceptance orchestration for deterministic `image.resize`.

use std::{fmt, sync::Arc};

use shape_domain::{
    ArtifactContentContract, ArtifactId, ArtifactKind, ArtifactRevision, ImageRasterContract,
    ImageResizeContractError, ImageResizeOperator, IntentSpec, RasterResize, RevisionId,
    Transformation, TransformationKind, TransformationOperation,
};
use shape_execution::{
    CapabilityId, ExecutedCandidate, ExecutionCoordinator, ExecutionInput, ExecutionReceipt,
    ExecutionRequest, RASTER_RESIZE_CAPABILITY, RasterResizeExecutor,
};
use shape_store::AcceptedCommit;

use super::{RASTER_MEDIA_TYPE, ShapeProject, output_contract};
use crate::CoreError;

/// Transient canonical PNG produced by the deterministic Raster Resize Operator.
#[derive(Clone)]
pub struct ImageResizeCandidate {
    artifact_id: ArtifactId,
    expected_head: RevisionId,
    transformation: Transformation,
    receipt: ExecutionReceipt,
    output_bytes: Arc<[u8]>,
    output_media_type: String,
    output_contract: ImageRasterContract,
}

impl fmt::Debug for ImageResizeCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImageResizeCandidate")
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

impl ImageResizeCandidate {
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
    /// Produces a deterministic resize Candidate without advancing history.
    ///
    /// The accepted canonical PNG is materialized once. Domain preparation is
    /// performed once and the opaque validated plan is moved into its physical
    /// executor, preventing request JSON from reinterpreting authored policy.
    ///
    /// # Errors
    ///
    /// Rejects unknown, stale, non-raster, non-canonical, identity, oversized,
    /// or output-contract-mismatched requests.
    pub fn propose_raster_resize(
        &self,
        artifact_id: ArtifactId,
        expected_head: RevisionId,
        parameters: RasterResize,
    ) -> Result<ImageResizeCandidate, CoreError> {
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
        let source_contract = resize_source_contract(&accepted.revision, artifact_id)?;
        let prepared = ImageResizeOperator::prepare(parameters, source_contract).map_err(
            |error| match error {
                ImageResizeContractError::InvalidSourceContract => {
                    CoreError::InvalidRasterContent { artifact_id }
                }
                ImageResizeContractError::Domain(error) => CoreError::Domain(error),
            },
        )?;
        if prepared.is_identity() {
            return Err(CoreError::NoOpRasterResize);
        }
        let dimensions = prepared.output_dimensions();
        let expected_output_contract = prepared.output_contract().clone();
        let transformation = Transformation::new_with_operation(
            TransformationKind::DeterministicEdit,
            artifact_id,
            vec![expected_head],
            IntentSpec::new(format!(
                "Resize raster to width={}, height={}",
                dimensions.width(),
                dimensions.height()
            ))?,
            Vec::new(),
            Vec::new(),
            Some(TransformationOperation::RasterResize(parameters)),
        )?;
        let input = ExecutionInput::materialized(accepted.revision.content, accepted.bytes)?;
        let request = ExecutionRequest::new_materialized(
            transformation.id,
            CapabilityId::new(RASTER_RESIZE_CAPABILITY)?,
            vec![input],
            Vec::new(),
            RASTER_MEDIA_TYPE,
        )?;
        let executor = RasterResizeExecutor::new(prepared)?;
        let ExecutedCandidate { output, receipt } =
            ExecutionCoordinator::execute(&executor, &request)?;
        let output_contract = output_contract(output.content_contract)?;
        if output_contract != expected_output_contract {
            return Err(CoreError::RasterResizeOutputContractMismatch);
        }
        Ok(ImageResizeCandidate {
            artifact_id,
            expected_head,
            transformation,
            receipt,
            output_bytes: output.bytes.into(),
            output_media_type: output.media_type,
            output_contract,
        })
    }

    /// Explicitly accepts one exact Raster Resize Candidate.
    ///
    /// # Errors
    ///
    /// Returns an error when its expected head is stale or publication fails.
    pub fn accept_raster_resize(
        &mut self,
        candidate: ImageResizeCandidate,
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

fn resize_source_contract(
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
