use crate::{
    ArtifactId, OperatorDataTypeId, OperatorGraph, OperatorGraphEdge, OperatorGraphNode,
    OperatorNodeBinding, OperatorNodeId, OperatorNodeRole, OperatorPort, OperatorPortId,
    OperatorTypeId, RevisionId,
};

use super::{NamedSceneOutput, Scene, SceneOutputName, SceneRevision};

fn output_graph() -> (OperatorGraph, OperatorNodeId) {
    let artifact_id = ArtifactId::new();
    let revision_id = RevisionId::new();
    let source_id = OperatorNodeId::source(revision_id);
    let output_id = OperatorNodeId::output(artifact_id);
    let data_type = OperatorDataTypeId::new("text.document").unwrap();
    let graph = OperatorGraph::new(
        vec![
            OperatorGraphNode::new(
                source_id.clone(),
                OperatorNodeRole::Source,
                OperatorTypeId::new("source.text.document").unwrap(),
                OperatorNodeBinding::Source {
                    artifact_id,
                    revision_id,
                },
                Vec::new(),
                vec![OperatorPort::new(
                    OperatorPortId::new("output.value").unwrap(),
                    data_type.clone(),
                )],
            )
            .unwrap(),
            OperatorGraphNode::new(
                output_id.clone(),
                OperatorNodeRole::Output,
                OperatorTypeId::new("output.text.document").unwrap(),
                OperatorNodeBinding::Output {
                    artifact_id,
                    revision_id,
                },
                vec![OperatorPort::new(
                    OperatorPortId::new("input.value").unwrap(),
                    data_type,
                )],
                Vec::new(),
            )
            .unwrap(),
        ],
        vec![OperatorGraphEdge::new(
            source_id,
            OperatorPortId::new("output.value").unwrap(),
            output_id.clone(),
            OperatorPortId::new("input.value").unwrap(),
        )],
    )
    .unwrap();
    (graph, output_id)
}

#[test]
fn accepted_scene_revision_names_every_output_node() {
    let scene = Scene::new("Opening").unwrap();
    let (graph, output_id) = output_graph();
    let revision = SceneRevision::new(
        scene.id,
        None,
        graph,
        vec![NamedSceneOutput::new(
            SceneOutputName::new("main").unwrap(),
            output_id,
        )],
        10,
    )
    .unwrap();
    assert_eq!(revision.scene_id, scene.id);
    assert_eq!(revision.outputs[0].name.as_str(), "main");
}

#[test]
fn missing_duplicate_or_non_output_mappings_fail_closed() {
    let scene = Scene::new("Opening").unwrap();
    let (graph, output_id) = output_graph();
    assert!(SceneRevision::new(scene.id, None, graph.clone(), Vec::new(), 10).is_err());
    let source_id = graph.nodes[0].id.clone();
    assert!(
        SceneRevision::new(
            scene.id,
            None,
            graph.clone(),
            vec![NamedSceneOutput::new(
                SceneOutputName::new("main").unwrap(),
                source_id,
            )],
            10,
        )
        .is_err()
    );
    assert!(
        SceneRevision::new(
            scene.id,
            None,
            graph,
            vec![
                NamedSceneOutput::new(SceneOutputName::new("main").unwrap(), output_id.clone()),
                NamedSceneOutput::new(SceneOutputName::new("main").unwrap(), output_id),
            ],
            10,
        )
        .is_err()
    );
}

#[test]
fn scene_and_output_names_are_portably_bounded() {
    assert!(Scene::new("").is_err());
    assert!(SceneOutputName::new("main preview").is_err());
    assert_eq!(
        SceneOutputName::new("preview.zh-CN").unwrap().as_str(),
        "preview.zh-CN"
    );
}
