//! Typed, user-authored creative operator graphs.
//!
//! This graph records the visible `Source -> Operator -> Output` structure of
//! one creative scene. It deliberately does not describe executor jobs, model
//! pipelines, media-internal layers, or transient candidate payloads.

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::{ArtifactId, DomainError, RevisionId, TransformationId};

const MAX_IDENTIFIER_BYTES: usize = 160;
const MAX_GRAPH_NODES: usize = 512;
const MAX_GRAPH_EDGES: usize = 2_048;
const MAX_NODE_PORTS: usize = 64;

macro_rules! operator_identifier {
    ($name:ident, $field:literal) => {
        #[doc = concat!("Portable ", $field, " identity.")]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Creates a bounded, language-neutral identifier.
            ///
            /// # Errors
            ///
            /// Rejects empty, oversized, non-ASCII, or unnamespaced values.
            pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
                let value = value.into();
                if value.is_empty()
                    || value.len() > MAX_IDENTIFIER_BYTES
                    || !value.is_ascii()
                    || !value.contains('.')
                {
                    return Err(DomainError::InvalidOperatorIdentifier {
                        field: $field,
                        max_bytes: MAX_IDENTIFIER_BYTES,
                    });
                }
                Ok(Self(value))
            }

            /// Returns the exact stable identifier.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

operator_identifier!(OperatorNodeId, "operator node");
operator_identifier!(OperatorTypeId, "operator type");
operator_identifier!(OperatorPortId, "operator port");
operator_identifier!(OperatorDataTypeId, "operator data type");

impl OperatorNodeId {
    /// Stable source-node identity for one immutable input revision.
    #[must_use]
    pub fn source(revision_id: RevisionId) -> Self {
        Self(format!("source.{revision_id}"))
    }

    /// Stable operator-node identity for one accepted creative transformation.
    #[must_use]
    pub fn transformation(transformation_id: TransformationId) -> Self {
        Self(format!("operator.{transformation_id}"))
    }

    /// Stable output-node identity for one scene-compatible artifact.
    #[must_use]
    pub fn output(artifact_id: ArtifactId) -> Self {
        Self(format!("output.{artifact_id}"))
    }
}

/// User-visible semantic role of a scene graph node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorNodeRole {
    /// Immutable material entering the scene boundary.
    Source,
    /// A creative operation with typed inputs and outputs.
    Operator,
    /// A named scene result. Output nodes never transform their input.
    Output,
}

/// Durable creative identity represented by a graph node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "binding", rename_all = "snake_case")]
pub enum OperatorNodeBinding {
    Source {
        artifact_id: ArtifactId,
        revision_id: RevisionId,
    },
    Transformation {
        transformation_id: TransformationId,
    },
    Output {
        artifact_id: ArtifactId,
        revision_id: RevisionId,
    },
}

/// One declared input or output port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorPort {
    pub id: OperatorPortId,
    pub data_type: OperatorDataTypeId,
}

impl OperatorPort {
    /// Creates one typed port.
    #[must_use]
    pub const fn new(id: OperatorPortId, data_type: OperatorDataTypeId) -> Self {
        Self { id, data_type }
    }
}

/// One visible node in a creative scene graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorGraphNode {
    pub id: OperatorNodeId,
    pub role: OperatorNodeRole,
    pub operator_type: OperatorTypeId,
    pub binding: OperatorNodeBinding,
    pub inputs: Vec<OperatorPort>,
    pub outputs: Vec<OperatorPort>,
}

impl OperatorGraphNode {
    /// Creates and validates one visible scene node.
    ///
    /// # Errors
    ///
    /// Rejects role/binding mismatches, duplicate ports, and oversized port sets.
    pub fn new(
        id: OperatorNodeId,
        role: OperatorNodeRole,
        operator_type: OperatorTypeId,
        binding: OperatorNodeBinding,
        inputs: Vec<OperatorPort>,
        outputs: Vec<OperatorPort>,
    ) -> Result<Self, DomainError> {
        validate_collection("operator input ports", inputs.len(), MAX_NODE_PORTS)?;
        validate_collection("operator output ports", outputs.len(), MAX_NODE_PORTS)?;
        validate_unique_ports("operator input ports", &inputs)?;
        validate_unique_ports("operator output ports", &outputs)?;
        let role_matches_binding = matches!(
            (role, &binding),
            (OperatorNodeRole::Source, OperatorNodeBinding::Source { .. })
                | (
                    OperatorNodeRole::Operator,
                    OperatorNodeBinding::Transformation { .. }
                )
                | (OperatorNodeRole::Output, OperatorNodeBinding::Output { .. })
        );
        if !role_matches_binding
            || (role == OperatorNodeRole::Source && !inputs.is_empty())
            || (role == OperatorNodeRole::Output && !outputs.is_empty())
        {
            return Err(DomainError::InvalidOperatorNode);
        }
        Ok(Self {
            id,
            role,
            operator_type,
            binding,
            inputs,
            outputs,
        })
    }
}

/// One exact typed connection between two visible node ports.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OperatorGraphEdge {
    pub source_node_id: OperatorNodeId,
    pub source_port_id: OperatorPortId,
    pub target_node_id: OperatorNodeId,
    pub target_port_id: OperatorPortId,
}

impl OperatorGraphEdge {
    /// Creates one connection; graph construction validates its endpoints.
    #[must_use]
    pub const fn new(
        source_node_id: OperatorNodeId,
        source_port_id: OperatorPortId,
        target_node_id: OperatorNodeId,
        target_port_id: OperatorPortId,
    ) -> Self {
        Self {
            source_node_id,
            source_port_id,
            target_node_id,
            target_port_id,
        }
    }
}

/// A validated acyclic creative operator graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorGraph {
    pub nodes: Vec<OperatorGraphNode>,
    pub edges: Vec<OperatorGraphEdge>,
}

impl OperatorGraph {
    /// Creates a typed DAG.
    ///
    /// # Errors
    ///
    /// Rejects duplicate identities, missing or mismatched ports, multiply-bound
    /// inputs, self edges, cycles, and oversized graphs.
    pub fn new(
        nodes: Vec<OperatorGraphNode>,
        edges: Vec<OperatorGraphEdge>,
    ) -> Result<Self, DomainError> {
        validate_collection("operator graph nodes", nodes.len(), MAX_GRAPH_NODES)?;
        validate_collection("operator graph edges", edges.len(), MAX_GRAPH_EDGES)?;
        let nodes_by_id = nodes
            .iter()
            .map(|node| (&node.id, node))
            .collect::<HashMap<_, _>>();
        if nodes_by_id.len() != nodes.len() {
            return Err(DomainError::DuplicateOperatorNode);
        }
        let mut unique_edges = HashSet::new();
        let mut bound_inputs = HashSet::new();
        for edge in &edges {
            if edge.source_node_id == edge.target_node_id || !unique_edges.insert(edge) {
                return Err(DomainError::InvalidOperatorEdge);
            }
            let source = nodes_by_id
                .get(&edge.source_node_id)
                .ok_or(DomainError::InvalidOperatorEdge)?;
            let target = nodes_by_id
                .get(&edge.target_node_id)
                .ok_or(DomainError::InvalidOperatorEdge)?;
            let source_port = source
                .outputs
                .iter()
                .find(|port| port.id == edge.source_port_id)
                .ok_or(DomainError::InvalidOperatorEdge)?;
            let target_port = target
                .inputs
                .iter()
                .find(|port| port.id == edge.target_port_id)
                .ok_or(DomainError::InvalidOperatorEdge)?;
            if source_port.data_type != target_port.data_type {
                return Err(DomainError::OperatorPortTypeMismatch);
            }
            if !bound_inputs.insert((&edge.target_node_id, &edge.target_port_id)) {
                return Err(DomainError::OperatorInputAlreadyBound);
            }
        }
        validate_acyclic(&nodes, &edges)?;
        Ok(Self { nodes, edges })
    }
}

fn validate_collection(
    collection: &'static str,
    actual: usize,
    maximum: usize,
) -> Result<(), DomainError> {
    if actual > maximum {
        return Err(DomainError::CollectionTooLarge {
            collection,
            actual,
            maximum,
        });
    }
    Ok(())
}

fn validate_unique_ports(
    collection: &'static str,
    ports: &[OperatorPort],
) -> Result<(), DomainError> {
    let unique = ports.iter().map(|port| &port.id).collect::<HashSet<_>>();
    if unique.len() != ports.len() {
        return Err(DomainError::DuplicateOperatorPort { collection });
    }
    Ok(())
}

fn validate_acyclic(
    nodes: &[OperatorGraphNode],
    edges: &[OperatorGraphEdge],
) -> Result<(), DomainError> {
    let mut indegree = nodes
        .iter()
        .map(|node| (node.id.clone(), 0_usize))
        .collect::<HashMap<_, _>>();
    let mut outgoing: HashMap<&OperatorNodeId, Vec<&OperatorNodeId>> = HashMap::new();
    for edge in edges {
        *indegree
            .get_mut(&edge.target_node_id)
            .ok_or(DomainError::InvalidOperatorEdge)? += 1;
        outgoing
            .entry(&edge.source_node_id)
            .or_default()
            .push(&edge.target_node_id);
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(node_id, degree)| (*degree == 0).then_some(node_id.clone()))
        .collect::<VecDeque<_>>();
    let mut visited = 0_usize;
    while let Some(node_id) = ready.pop_front() {
        visited += 1;
        if let Some(targets) = outgoing.get(&node_id) {
            for target in targets {
                let degree = indegree
                    .get_mut(*target)
                    .ok_or(DomainError::InvalidOperatorEdge)?;
                *degree -= 1;
                if *degree == 0 {
                    ready.push_back((*target).clone());
                }
            }
        }
    }
    if visited != nodes.len() {
        return Err(DomainError::OperatorGraphCycle);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
