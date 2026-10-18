//! Stable Scene identity and immutable accepted Operator Graph revisions.
//!
//! [`OperatorGraph`] owns typed DAG validity. This owner adds the creative
//! document boundary: one stable Scene head, immutable graph revisions, and a
//! portable named interface over every Output node. Mutable graph drafts and
//! execution jobs do not enter this contract.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    DomainError, OperatorGraph, OperatorNodeId, OperatorNodeRole, SceneId, SceneRevisionId,
};

const MAX_SCENE_NAME_BYTES: usize = 160;
const MAX_OUTPUT_NAME_BYTES: usize = 80;

/// Stable user-authored creative orchestration identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scene {
    pub id: SceneId,
    pub name: String,
    pub accepted_revision: Option<SceneRevisionId>,
}

impl Scene {
    /// Creates an empty Scene without an accepted graph head.
    ///
    /// # Errors
    ///
    /// Rejects an empty or oversized display name.
    pub fn new(name: impl Into<String>) -> Result<Self, DomainError> {
        let name = name.into();
        if name.is_empty() || name.len() > MAX_SCENE_NAME_BYTES {
            return Err(DomainError::InvalidSceneName {
                max_bytes: MAX_SCENE_NAME_BYTES,
            });
        }
        Ok(Self {
            id: SceneId::new(),
            name,
            accepted_revision: None,
        })
    }
}

/// Stable portable name of one Scene output interface.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SceneOutputName(String);

impl SceneOutputName {
    /// Creates an output name such as `main`, `thumbnail`, or `audio.mix`.
    ///
    /// # Errors
    ///
    /// Rejects empty, oversized, non-ASCII, or punctuation-unsafe values.
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_OUTPUT_NAME_BYTES
            || !value.is_ascii()
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err(DomainError::InvalidSceneOutputName {
                max_bytes: MAX_OUTPUT_NAME_BYTES,
            });
        }
        Ok(Self(value))
    }

    /// Returns the exact portable output name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SceneOutputName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// One named publication interface bound to an Output node in the graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedSceneOutput {
    pub name: SceneOutputName,
    pub node_id: OperatorNodeId,
}

impl NamedSceneOutput {
    #[must_use]
    pub const fn new(name: SceneOutputName, node_id: OperatorNodeId) -> Self {
        Self { name, node_id }
    }
}

/// One immutable accepted state of a Scene's creative Operator Graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneRevision {
    pub id: SceneRevisionId,
    pub scene_id: SceneId,
    pub parent: Option<SceneRevisionId>,
    pub graph: OperatorGraph,
    pub outputs: Vec<NamedSceneOutput>,
    pub created_at_unix_ms: u64,
}

impl SceneRevision {
    /// Validates the named publication interface for an accepted graph.
    ///
    /// # Errors
    ///
    /// Every Output node must be named exactly once. Names and node bindings
    /// must both be unique, and at least one Output is required.
    pub fn validate_graph_outputs(
        graph: &OperatorGraph,
        outputs: &[NamedSceneOutput],
    ) -> Result<(), DomainError> {
        let graph_output_ids = graph
            .nodes
            .iter()
            .filter(|node| node.role == OperatorNodeRole::Output)
            .map(|node| &node.id)
            .collect::<HashSet<_>>();
        let output_names = outputs
            .iter()
            .map(|output| &output.name)
            .collect::<HashSet<_>>();
        let output_node_ids = outputs
            .iter()
            .map(|output| &output.node_id)
            .collect::<HashSet<_>>();
        if outputs.is_empty()
            || output_names.len() != outputs.len()
            || output_node_ids.len() != outputs.len()
            || output_node_ids != graph_output_ids
        {
            return Err(DomainError::InvalidSceneOutputs);
        }
        Ok(())
    }

    /// Creates an immutable accepted Scene graph state.
    ///
    /// # Errors
    ///
    /// Every Output node must be named exactly once. Names and node bindings
    /// must both be unique, and at least one Output is required.
    pub fn new(
        scene_id: SceneId,
        parent: Option<SceneRevisionId>,
        graph: OperatorGraph,
        outputs: Vec<NamedSceneOutput>,
        created_at_unix_ms: u64,
    ) -> Result<Self, DomainError> {
        Self::validate_graph_outputs(&graph, &outputs)?;
        Ok(Self {
            id: SceneRevisionId::new(),
            scene_id,
            parent,
            graph,
            outputs,
            created_at_unix_ms,
        })
    }
}

#[cfg(test)]
mod tests;
