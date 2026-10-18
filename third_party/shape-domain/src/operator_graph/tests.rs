use crate::{ArtifactId, RevisionId, TransformationId};

use super::{
    OperatorDataTypeId, OperatorGraph, OperatorGraphEdge, OperatorGraphNode, OperatorNodeBinding,
    OperatorNodeId, OperatorNodeRole, OperatorPort, OperatorPortId, OperatorTypeId,
};

fn data_type(value: &str) -> OperatorDataTypeId {
    OperatorDataTypeId::new(value).unwrap()
}

fn port(value: &str, data_type_id: &str) -> OperatorPort {
    OperatorPort::new(OperatorPortId::new(value).unwrap(), data_type(data_type_id))
}

#[test]
fn source_operator_and_multi_output_ports_form_a_typed_dag() {
    let artifact_id = ArtifactId::new();
    let source_revision = RevisionId::new();
    let result_revision = RevisionId::new();
    let transformation_id = TransformationId::new();
    let source_id = OperatorNodeId::source(source_revision);
    let operator_id = OperatorNodeId::transformation(transformation_id);
    let output_id = OperatorNodeId::output(artifact_id);
    let nodes = vec![
        OperatorGraphNode::new(
            source_id.clone(),
            OperatorNodeRole::Source,
            OperatorTypeId::new("source.image.raster").unwrap(),
            OperatorNodeBinding::Source {
                artifact_id,
                revision_id: source_revision,
            },
            Vec::new(),
            vec![port("output.image", "image.raster")],
        )
        .unwrap(),
        OperatorGraphNode::new(
            operator_id.clone(),
            OperatorNodeRole::Operator,
            OperatorTypeId::new("image.background_replace").unwrap(),
            OperatorNodeBinding::Transformation { transformation_id },
            vec![port("input.image", "image.raster")],
            vec![
                port("output.image", "image.raster"),
                port("output.mask", "image.mask"),
            ],
        )
        .unwrap(),
        OperatorGraphNode::new(
            output_id.clone(),
            OperatorNodeRole::Output,
            OperatorTypeId::new("output.image.raster").unwrap(),
            OperatorNodeBinding::Output {
                artifact_id,
                revision_id: result_revision,
            },
            vec![port("input.value", "image.raster")],
            Vec::new(),
        )
        .unwrap(),
    ];
    let graph = OperatorGraph::new(
        nodes,
        vec![
            OperatorGraphEdge::new(
                source_id,
                OperatorPortId::new("output.image").unwrap(),
                operator_id.clone(),
                OperatorPortId::new("input.image").unwrap(),
            ),
            OperatorGraphEdge::new(
                operator_id,
                OperatorPortId::new("output.image").unwrap(),
                output_id,
                OperatorPortId::new("input.value").unwrap(),
            ),
        ],
    )
    .unwrap();
    assert_eq!(graph.nodes.len(), 3);
    assert_eq!(graph.edges.len(), 2);
    assert_eq!(graph.nodes[1].outputs.len(), 2);
}

#[test]
fn mismatched_ports_and_multiple_bindings_fail_closed() {
    let artifact_id = ArtifactId::new();
    let first_revision = RevisionId::new();
    let second_revision = RevisionId::new();
    let first_id = OperatorNodeId::source(first_revision);
    let second_id = OperatorNodeId::source(second_revision);
    let output_id = OperatorNodeId::output(artifact_id);
    let sources = vec![
        OperatorGraphNode::new(
            first_id.clone(),
            OperatorNodeRole::Source,
            OperatorTypeId::new("source.text.document").unwrap(),
            OperatorNodeBinding::Source {
                artifact_id,
                revision_id: first_revision,
            },
            Vec::new(),
            vec![port("output.value", "text.document")],
        )
        .unwrap(),
        OperatorGraphNode::new(
            second_id.clone(),
            OperatorNodeRole::Source,
            OperatorTypeId::new("source.image.raster").unwrap(),
            OperatorNodeBinding::Source {
                artifact_id,
                revision_id: second_revision,
            },
            Vec::new(),
            vec![port("output.value", "image.raster")],
        )
        .unwrap(),
        OperatorGraphNode::new(
            output_id.clone(),
            OperatorNodeRole::Output,
            OperatorTypeId::new("output.text.document").unwrap(),
            OperatorNodeBinding::Output {
                artifact_id,
                revision_id: second_revision,
            },
            vec![port("input.value", "text.document")],
            Vec::new(),
        )
        .unwrap(),
    ];
    let mismatch = OperatorGraph::new(
        sources.clone(),
        vec![OperatorGraphEdge::new(
            second_id,
            OperatorPortId::new("output.value").unwrap(),
            output_id.clone(),
            OperatorPortId::new("input.value").unwrap(),
        )],
    );
    assert!(mismatch.is_err());
    let multiply_bound = OperatorGraph::new(
        sources,
        vec![
            OperatorGraphEdge::new(
                first_id.clone(),
                OperatorPortId::new("output.value").unwrap(),
                output_id.clone(),
                OperatorPortId::new("input.value").unwrap(),
            ),
            OperatorGraphEdge::new(
                first_id,
                OperatorPortId::new("output.value").unwrap(),
                output_id,
                OperatorPortId::new("input.value").unwrap(),
            ),
        ],
    );
    assert!(multiply_bound.is_err());
}

#[test]
fn cycles_fail_closed() {
    let transform_a = TransformationId::new();
    let transform_b = TransformationId::new();
    let node_a = OperatorNodeId::transformation(transform_a);
    let node_b = OperatorNodeId::transformation(transform_b);
    let cyclic_nodes = vec![
        OperatorGraphNode::new(
            node_a.clone(),
            OperatorNodeRole::Operator,
            OperatorTypeId::new("text.edit.a").unwrap(),
            OperatorNodeBinding::Transformation {
                transformation_id: transform_a,
            },
            vec![port("input.value", "text.document")],
            vec![port("output.value", "text.document")],
        )
        .unwrap(),
        OperatorGraphNode::new(
            node_b.clone(),
            OperatorNodeRole::Operator,
            OperatorTypeId::new("text.edit.b").unwrap(),
            OperatorNodeBinding::Transformation {
                transformation_id: transform_b,
            },
            vec![port("input.value", "text.document")],
            vec![port("output.value", "text.document")],
        )
        .unwrap(),
    ];
    let cycle = OperatorGraph::new(
        cyclic_nodes,
        vec![
            OperatorGraphEdge::new(
                node_a.clone(),
                OperatorPortId::new("output.value").unwrap(),
                node_b.clone(),
                OperatorPortId::new("input.value").unwrap(),
            ),
            OperatorGraphEdge::new(
                node_b,
                OperatorPortId::new("output.value").unwrap(),
                node_a,
                OperatorPortId::new("input.value").unwrap(),
            ),
        ],
    );
    assert!(cycle.is_err());
}
