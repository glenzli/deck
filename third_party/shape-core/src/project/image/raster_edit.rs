//! Shared transient Candidate lifecycle for deterministic raster edit methods.

use std::{fmt, sync::Arc};

use shape_domain::{
    ArtifactContentContract, ArtifactId, ArtifactRevision, ImageRasterContract, RevisionId,
    Transformation, TransformationOperation,
};
use shape_execution::{ExecutionOutput, ExecutionReceipt};
use shape_store::AcceptedCommit;

use super::ShapeProject;
use crate::CoreError;

/// Transient canonical PNG produced by one deterministic raster edit method.
#[derive(Clone)]
pub struct ImageEditCandidate {
    artifact_id: ArtifactId,
    expected_head: RevisionId,
    transformation: Transformation,
    receipt: ExecutionReceipt,
    output_bytes: Arc<[u8]>,
    output_media_type: String,
    output_contract: ImageRasterContract,
}

impl fmt::Debug for ImageEditCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImageEditCandidate")
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

impl ImageEditCandidate {
    pub(super) fn new(
        artifact_id: ArtifactId,
        expected_head: RevisionId,
        transformation: Transformation,
        receipt: ExecutionReceipt,
        output: ExecutionOutput,
        output_contract: ImageRasterContract,
    ) -> Self {
        Self {
            artifact_id,
            expected_head,
            transformation,
            receipt,
            output_bytes: output.bytes.into(),
            output_media_type: output.media_type,
            output_contract,
        }
    }

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
    /// Explicitly accepts one exact deterministic raster-edit Candidate.
    ///
    /// # Errors
    ///
    /// Returns an error when its expected head is stale or publication fails.
    pub fn accept_raster_edit(
        &mut self,
        candidate: ImageEditCandidate,
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
