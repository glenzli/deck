use super::*;
use shape_domain::{Artifact, ArtifactKind};
use shape_domain::{
    ArtifactWorkingGraph, OperatorConfigurationSchemaId, OperatorDataTypeId, OperatorTypeId,
    WorkingInput, WorkingOperatorConfiguration,
};

fn add_node(
    project: &mut ShapeProject,
    source: Option<WorkingInput>,
) -> (ArtifactId, WorkingOperatorDraft) {
    let artifact = Artifact::new("Output", ArtifactKind::TextDocument).unwrap();
    let mut graph = ArtifactWorkingGraph::new_source(artifact.id);
    let text = OperatorDataTypeId::new("text.document").unwrap();
    let draft = if let Some(input) = source {
        graph.add_bound_operator(
            OperatorTypeId::new("text.edit").unwrap(),
            text.clone(),
            text,
            input,
        )
    } else {
        graph.add_source_operator(OperatorTypeId::new("text.create").unwrap(), text)
    }
    .unwrap();
    project
        .create_source_artifact_draft(&artifact, &graph)
        .unwrap();
    (artifact.id, draft)
}

#[test]
fn branches_preserve_original_and_nodes_survive_multiple_acceptances_and_reopen() {
    let root = std::env::temp_dir().join(format!("shape-node-{}", uuid::Uuid::now_v7()));
    let mut project = ShapeProject::create(&root, "Nodes").unwrap();
    let original = project
        .create_text_document("Original", "Original words.")
        .unwrap();
    let input = WorkingInput {
        artifact_id: original.artifact_id,
        revision_id: original.id,
    };
    let (a, node_a) = add_node(&mut project, Some(input));
    let (b, node_b) = add_node(&mut project, Some(input));
    let script = "[role: Reader]\n[speaker: Reader]\nHello.\n[pause: 2s]";
    let candidate_a = project
        .propose_text_node_literal(a, &node_a, script, TextDocumentContract::speech_script())
        .unwrap();
    let candidate_b = project
        .propose_text_node_literal(b, &node_b, "Translation.", TextDocumentContract::Plain)
        .unwrap();
    project.accept_text(candidate_a).unwrap();
    project.accept_text(candidate_b).unwrap();
    let repeated = project
        .propose_text_node_literal(a, &node_a, script, TextDocumentContract::speech_script())
        .unwrap();
    project.accept_text(repeated).unwrap();
    assert_eq!(
        project
            .read_accepted(original.artifact_id)
            .unwrap()
            .unwrap()
            .revision
            .id,
        original.id
    );
    drop(project);
    let project = ShapeProject::open(&root).unwrap();
    let graphs = project.artifact_working_graphs().unwrap();
    assert_eq!(graphs.len(), 2);
    assert!(graphs.iter().any(|g| g.context_artifact_id() == a && g.operators() == std::slice::from_ref(&node_a)));
    assert_eq!(
        project
            .read_accepted(a)
            .unwrap()
            .unwrap()
            .revision
            .content_contract,
        Some(shape_domain::ArtifactContentContract::TextDocument(
            TextDocumentContract::speech_script()
        ))
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn stale_source_and_changed_rules_reject_adoption() {
    let root = std::env::temp_dir().join(format!("shape-node-stale-{}", uuid::Uuid::now_v7()));
    let mut project = ShapeProject::create(&root, "Nodes").unwrap();
    let original = project
        .create_text_document("Original", "Original.")
        .unwrap();
    let (target, node) = add_node(
        &mut project,
        Some(WorkingInput {
            artifact_id: original.artifact_id,
            revision_id: original.id,
        }),
    );
    let candidate = project
        .propose_text_node_literal(target, &node, "Result.", TextDocumentContract::Plain)
        .unwrap();
    let mut graph = project.artifact_working_graphs().unwrap().remove(0);
    graph.set_operator_configuration(
        node.id(),
        Some(
            WorkingOperatorConfiguration::new(
                OperatorConfigurationSchemaId::new("text.test").unwrap(),
                "{\"rule\":2}",
            )
            .unwrap(),
        ),
    );
    project.save_artifact_working_graph(&graph).unwrap();
    assert!(project.accept_text(candidate).is_err());
    let node = &graph.operators()[0];
    let candidate = project
        .propose_text_node_literal(target, node, "Result.", TextDocumentContract::Plain)
        .unwrap();
    let change = project
        .propose_text(
            original.artifact_id,
            Some(original.id),
            "Changed.",
            IntentSpec::new("Edit original").unwrap(),
            vec![],
        )
        .unwrap();
    project.accept_text(change).unwrap();
    assert!(project.accept_text(candidate).is_err());
    assert!(project.read_accepted(target).unwrap().is_none());
    assert!(
        project
            .propose_text_node_literal(target, node, "Again.", TextDocumentContract::Plain)
            .is_err()
    );
    std::fs::remove_dir_all(root).unwrap();
}
