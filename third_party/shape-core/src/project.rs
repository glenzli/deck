//! Project bundle facade and routing to media-specific creative use cases.

mod agent_file;
mod card_kit;
pub use card_kit::{
    CardKitAction, CardKitCandidate, CardKitDraft, CardKitError, CardKitExport,
    CardPrintCutLineStyle, CardPrintOptions, CardPrintPaper, CardPrintPlan,
    CardPrintRequestOptions,
};
mod material_edit;
mod picture_book;
pub use material_edit::{
    MATERIAL_EDIT_TICKET_SCHEMA, MaterialEditCandidate, MaterialEditHandoff, MaterialEditTicket,
};
pub use picture_book::{
    PictureBookCandidate, PictureBookDraft, PictureBookDraftAction, PictureBookError,
    PictureBookExport, PictureBookPage, PictureBookRequest,
};
mod audio;
mod sound_generation;
pub use sound_generation::SoundCandidate;
mod file_import;
mod image;
mod scene;
mod text;

pub use agent_file::AgentTextCandidate;
pub use audio::AudioCandidate;
pub use image::{AiImageCandidate, ImageCandidate, ImageEditCandidate, ImageResizeCandidate};
pub use scene::SceneGraphCandidate;
pub use text::{
    TEXT_DOCUMENT_DATA_TYPE, TEXT_EDIT_OPERATOR_TYPE, TEXT_TRANSFORM_OPERATOR_TYPE, TextCandidate,
    TextEditParameters, TextTransformMode, TextTransformParameters,
};

use std::path::Path;

use shape_domain::{
    Artifact, ArtifactId, ArtifactKind, ArtifactRevision, ArtifactWorkingGraph, RevisionId,
    Transformation, TransformationId,
};
use shape_execution::{AttemptId, ExecutionReceipt};
use shape_store::{ProjectInspection, ProjectSnapshot, ProjectStore};

use crate::CoreError;

/// Open application facade for one Shape project bundle.
#[derive(Debug)]
pub struct ShapeProject {
    store: ProjectStore,
}

/// Verified accepted content loaded from durable history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedArtifactContent {
    pub revision: ArtifactRevision,
    pub bytes: Vec<u8>,
}

impl ShapeProject {
    /// Reads persisted project heads and drafts in one bounded read-only snapshot.
    /// Does not migrate a project or observe another desktop session's transient candidates.
    ///
    /// # Errors
    /// Rejects incompatible schemas, malformed, busy or oversized project metadata.
    pub fn inspect(root: impl AsRef<Path>) -> Result<ProjectInspection, CoreError> {
        Ok(ProjectStore::inspect(root)?)
    }

    /// Inspects one exact accepted image/audio material without migration or history changes.
    /// # Errors
    /// Rejects wrong pins, incompatible schemas, bounded metadata/content failures or corrupt CAS.
    pub fn inspect_material(
        root: impl AsRef<Path>,
        source: shape_domain::WorkingInput,
    ) -> Result<shape_store::MaterialInspection, CoreError> {
        Ok(ProjectStore::inspect_material(root, source)?)
    }

    /// Renames a stable creative object; accepted content remains immutable.
    /// # Errors
    /// Rejects invalid names, missing objects and persistence failures.
    pub fn rename_artifact(&self, artifact_id: ArtifactId, name: &str) -> Result<(), CoreError> {
        Ok(self.store.rename_artifact(artifact_id, name)?)
    }
    /// Deletes mutable node state and its unaccepted reserved output.
    /// # Errors
    /// Returns persistence errors; accepted revisions remain intact.
    pub fn discard_output_working_graph(
        &mut self,
        artifact_id: ArtifactId,
    ) -> Result<(), CoreError> {
        Ok(self.store.discard_output_working_graph(artifact_id)?)
    }
    /// Creates a new `.shape` project bundle.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid metadata or durable write failure.
    pub fn create(root: impl AsRef<Path>, name: impl Into<String>) -> Result<Self, CoreError> {
        Ok(Self {
            store: ProjectStore::create(root, name)?,
        })
    }

    /// Opens and validates an existing `.shape` project bundle.
    ///
    /// # Errors
    ///
    /// Returns an error for an incompatible or corrupt project.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, CoreError> {
        Ok(Self {
            store: ProjectStore::open(root)?,
        })
    }

    /// Creates a stable artifact without accepted content.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid name or durable write failure.
    pub fn create_artifact(
        &self,
        name: impl Into<String>,
        kind: ArtifactKind,
    ) -> Result<Artifact, CoreError> {
        let artifact = Artifact::new(name, kind)?;
        self.store.insert_artifact(&artifact)?;
        Ok(artifact)
    }

    /// Atomically reserves an unaccepted output together with its producer graph.
    /// Its required original binding, when present, is independent of the output head.
    ///
    /// # Errors
    ///
    /// Returns an error when the graph does not target the new Artifact, has
    /// an accepted input anchor, is empty, or cannot be durably published.
    pub fn create_source_artifact_draft(
        &mut self,
        artifact: &Artifact,
        graph: &ArtifactWorkingGraph,
    ) -> Result<(), CoreError> {
        Ok(self
            .store
            .insert_source_artifact_with_working_graph(artifact, graph)?)
    }

    /// Returns project metadata and current artifact heads.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid durable project state.
    pub fn snapshot(&self) -> Result<ProjectSnapshot, CoreError> {
        Ok(self.store.snapshot()?)
    }

    /// Loads every mutable compatibility-Scene Working Graph.
    ///
    /// Working Graphs are project-backed editor state, not accepted creative history.
    ///
    /// # Errors
    ///
    /// Returns an error when the project store cannot load or validate the graphs.
    pub fn artifact_working_graphs(&self) -> Result<Vec<ArtifactWorkingGraph>, CoreError> {
        Ok(self.store.artifact_working_graphs()?)
    }

    /// Saves one non-empty Working Graph against its exact accepted input head.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, expected-head comparison, or storage fails.
    pub fn save_artifact_working_graph(
        &self,
        graph: &ArtifactWorkingGraph,
    ) -> Result<(), CoreError> {
        Ok(self.store.save_artifact_working_graph(graph)?)
    }

    /// Replaces the exact draft state last observed by an interactive consumer.
    /// `None` discards the graph and its still-unaccepted reserved output.
    ///
    /// # Errors
    /// Rejects changed drafts or heads, invalid graphs, and storage failures.
    pub fn replace_artifact_working_graph(
        &self,
        artifact_id: ArtifactId,
        expected: Option<&ArtifactWorkingGraph>,
        replacement: Option<&ArtifactWorkingGraph>,
    ) -> Result<(), CoreError> {
        Ok(self
            .store
            .replace_artifact_working_graph(artifact_id, expected, replacement)?)
    }

    /// Deletes mutable editor state without changing accepted history.
    ///
    /// # Errors
    ///
    /// Returns an error when the project store cannot delete the graph.
    pub fn delete_artifact_working_graph(&self, artifact_id: ArtifactId) -> Result<(), CoreError> {
        Ok(self.store.delete_artifact_working_graph(artifact_id)?)
    }

    /// Loads one accepted creative transformation for inspection and lineage projection.
    ///
    /// # Errors
    ///
    /// Returns an error when durable transformation data is absent or invalid.
    pub fn transformation(
        &self,
        transformation_id: TransformationId,
    ) -> Result<Transformation, CoreError> {
        Ok(self.store.transformation(transformation_id)?)
    }

    /// Loads the accepted receipt for one immutable transformation.
    /// # Errors
    /// Rejects malformed durable provenance or storage failure.
    pub fn transformation_receipt(
        &self,
        id: shape_domain::TransformationId,
    ) -> Result<Option<ExecutionReceipt>, CoreError> {
        Ok(self.store.transformation_receipt(id)?)
    }

    /// Loads one accepted payload-free execution receipt for provenance inspection.
    ///
    /// # Errors
    ///
    /// Returns an error when the receipt is absent or invalid.
    pub fn execution_receipt(&self, attempt_id: AttemptId) -> Result<ExecutionReceipt, CoreError> {
        Ok(self.store.execution_receipt(attempt_id)?)
    }

    /// Loads one immutable accepted revision for lineage resolution.
    ///
    /// # Errors
    ///
    /// Returns an error when durable revision data is absent or invalid.
    pub fn revision(&self, revision_id: RevisionId) -> Result<ArtifactRevision, CoreError> {
        Ok(self.store.revision(revision_id)?)
    }

    /// Loads one immutable revision and verifies its exact content object.
    ///
    /// Unlike [`Self::read_accepted`], this reads the requested historical
    /// revision rather than the Artifact's current head. Read-only graph
    /// projections use it to present the actual material bound to a Source
    /// node.
    ///
    /// # Errors
    ///
    /// Returns an error when the revision is unknown or its durable object is
    /// missing or corrupt.
    pub fn read_revision_content(
        &self,
        revision_id: RevisionId,
    ) -> Result<AcceptedArtifactContent, CoreError> {
        let revision = self.store.revision(revision_id)?;
        let bytes = self.store.read_content(&revision.content)?;
        Ok(AcceptedArtifactContent { revision, bytes })
    }

    /// Loads the current accepted revision and verifies its exact content object.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown artifact or corrupt durable object.
    pub fn read_accepted(
        &self,
        artifact_id: ArtifactId,
    ) -> Result<Option<AcceptedArtifactContent>, CoreError> {
        self.store
            .accepted_revision(artifact_id)?
            .map(|revision| self.read_revision_content(revision.id))
            .transpose()
    }
}
