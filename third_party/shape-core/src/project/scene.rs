//! Scene graph draft and explicit acceptance use cases.

use shape_domain::{
    NamedSceneOutput, OperatorGraph, Scene, SceneId, SceneRevision, SceneRevisionId,
};
use shape_store::AcceptedSceneRevisionCommit;

use super::ShapeProject;
use crate::CoreError;

/// Transient graph draft awaiting explicit user acceptance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneGraphCandidate {
    scene_id: SceneId,
    expected_head: Option<SceneRevisionId>,
    graph: OperatorGraph,
    outputs: Vec<NamedSceneOutput>,
}

impl SceneGraphCandidate {
    #[must_use]
    pub const fn scene_id(&self) -> SceneId {
        self.scene_id
    }

    #[must_use]
    pub const fn expected_head(&self) -> Option<SceneRevisionId> {
        self.expected_head
    }

    /// Returns the validated draft for preview. It is not durable history yet.
    #[must_use]
    pub const fn graph(&self) -> &OperatorGraph {
        &self.graph
    }

    #[must_use]
    pub fn outputs(&self) -> &[NamedSceneOutput] {
        &self.outputs
    }
}

impl ShapeProject {
    /// Creates a stable Scene without an accepted graph revision.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid name or durable write failure.
    pub fn create_scene(&self, name: impl Into<String>) -> Result<Scene, CoreError> {
        let scene = Scene::new(name)?;
        self.store.insert_scene(&scene)?;
        Ok(scene)
    }

    /// Loads one stable Scene and its current accepted head.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid durable state.
    pub fn scene(&self, scene_id: SceneId) -> Result<Option<Scene>, CoreError> {
        Ok(self.store.scene(scene_id)?)
    }

    /// Loads one immutable accepted Scene graph revision.
    ///
    /// # Errors
    ///
    /// Returns an error when the revision is absent or invalid.
    pub fn scene_revision(&self, revision_id: SceneRevisionId) -> Result<SceneRevision, CoreError> {
        Ok(self.store.scene_revision(revision_id)?)
    }

    /// Prepares a graph candidate without changing the accepted Scene head.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown Scene or stale expected head.
    pub fn propose_scene_graph(
        &self,
        scene_id: SceneId,
        expected_head: Option<SceneRevisionId>,
        graph: OperatorGraph,
        outputs: Vec<NamedSceneOutput>,
    ) -> Result<SceneGraphCandidate, CoreError> {
        let scene = self
            .store
            .scene(scene_id)?
            .ok_or(shape_store::StoreError::UnknownScene(scene_id))?;
        if scene.accepted_revision != expected_head {
            return Err(CoreError::StaleSceneCandidate {
                scene_id,
                expected: expected_head,
                actual: scene.accepted_revision,
            });
        }
        SceneRevision::validate_graph_outputs(&graph, &outputs)?;
        Ok(SceneGraphCandidate {
            scene_id,
            expected_head,
            graph,
            outputs,
        })
    }

    /// Explicitly accepts a graph candidate and advances only its Scene head.
    ///
    /// # Errors
    ///
    /// Returns an error when its expected head is stale, a durable binding is
    /// missing, or atomic publication fails.
    pub fn accept_scene_graph(
        &mut self,
        candidate: SceneGraphCandidate,
    ) -> Result<SceneRevision, CoreError> {
        Ok(self
            .store
            .accept_scene_revision(AcceptedSceneRevisionCommit {
                scene_id: candidate.scene_id,
                expected_head: candidate.expected_head,
                graph: candidate.graph,
                outputs: candidate.outputs,
            })?)
    }
}

#[cfg(test)]
mod tests;
