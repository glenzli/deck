use std::fs;

use shape_domain::{
    ArtifactKind, IntentSpec, NamedSceneOutput, OperatorDataTypeId, OperatorGraph,
    OperatorGraphEdge, OperatorGraphNode, OperatorNodeBinding, OperatorNodeId, OperatorNodeRole,
    OperatorPort, OperatorPortId, OperatorTypeId, SceneOutputName,
};
use uuid::Uuid;

use super::*;

fn accepted_text(project: &mut ShapeProject) -> (shape_domain::Artifact, shape_domain::RevisionId) {
    let artifact = project
        .create_artifact("Shared text", ArtifactKind::TextDocument)
        .unwrap();
    let revision = project
        .propose_text(
            artifact.id,
            None,
            "Accepted text",
            IntentSpec::new("Create shared text").unwrap(),
            Vec::new(),
        )
        .and_then(|candidate| project.accept_text(candidate))
        .unwrap();
    (artifact, revision.id)
}

fn graph_for(
    artifact: &shape_domain::Artifact,
    revision_id: shape_domain::RevisionId,
) -> (OperatorGraph, Vec<NamedSceneOutput>) {
    let source_id = OperatorNodeId::source(revision_id);
    let output_id = OperatorNodeId::output(artifact.id);
    let data_type = OperatorDataTypeId::new("text.document").unwrap();
    let graph = OperatorGraph::new(
        vec![
            OperatorGraphNode::new(
                source_id.clone(),
                OperatorNodeRole::Source,
                OperatorTypeId::new("source.text.document").unwrap(),
                OperatorNodeBinding::Source {
                    artifact_id: artifact.id,
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
                    artifact_id: artifact.id,
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
    let outputs = vec![NamedSceneOutput::new(
        SceneOutputName::new("main").unwrap(),
        output_id,
    )];
    (graph, outputs)
}

#[test]
fn scene_graph_candidate_is_transient_and_two_scene_heads_evolve_independently() {
    let root = std::env::temp_dir().join(format!("shape-core-scenes-{}", Uuid::now_v7()));
    let mut project = ShapeProject::create(&root, "Scenes").unwrap();
    let (artifact, revision_id) = accepted_text(&mut project);
    let first = project.create_scene("Opening").unwrap();
    let second = project.create_scene("Ending").unwrap();
    let (graph, outputs) = graph_for(&artifact, revision_id);

    let first_candidate = project
        .propose_scene_graph(first.id, None, graph.clone(), outputs.clone())
        .unwrap();
    assert_eq!(
        project.scene(first.id).unwrap().unwrap().accepted_revision,
        None
    );
    let first_revision = project.accept_scene_graph(first_candidate).unwrap();
    let second_revision = project
        .propose_scene_graph(second.id, None, graph.clone(), outputs.clone())
        .and_then(|candidate| project.accept_scene_graph(candidate))
        .unwrap();
    let stale_first = project
        .propose_scene_graph(
            first.id,
            Some(first_revision.id),
            graph.clone(),
            outputs.clone(),
        )
        .unwrap();
    let next_first = project
        .propose_scene_graph(first.id, Some(first_revision.id), graph, outputs)
        .and_then(|candidate| project.accept_scene_graph(candidate))
        .unwrap();
    assert!(matches!(
        project.accept_scene_graph(stale_first),
        Err(CoreError::Store(
            shape_store::StoreError::SceneRevisionConflict { .. }
        ))
    ));
    assert_eq!(
        project.scene(second.id).unwrap().unwrap().accepted_revision,
        Some(second_revision.id)
    );
    drop(project);

    let reopened = ShapeProject::open(&root).unwrap();
    assert_eq!(reopened.snapshot().unwrap().scenes.len(), 2);
    assert_eq!(
        reopened.scene(first.id).unwrap().unwrap().accepted_revision,
        Some(next_first.id)
    );
    assert_eq!(
        reopened.scene_revision(next_first.id).unwrap().parent,
        Some(first_revision.id)
    );
    fs::remove_dir_all(root).unwrap();
}
