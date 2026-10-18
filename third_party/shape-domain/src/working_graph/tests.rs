use super::*;

fn text_id(value: &str) -> OperatorDataTypeId {
    OperatorDataTypeId::new(value).unwrap()
}

#[test]
fn working_operator_is_recoverable_without_entering_accepted_graph_history() {
    let artifact_id = ArtifactId::new();
    let revision_id = RevisionId::new();
    let mut graph = ArtifactWorkingGraph::new(artifact_id, revision_id);
    let draft = graph
        .add_operator(
            OperatorTypeId::new("text.transform").unwrap(),
            text_id("text.document"),
            text_id("text.document"),
        )
        .unwrap();
    let configuration = WorkingOperatorConfiguration::new(
        OperatorConfigurationSchemaId::new("shape.operator-draft.text-transform@20260811.1")
            .unwrap(),
        r#"{"instruction":"Make it warmer."}"#,
    )
    .unwrap();
    assert!(graph.set_operator_configuration(draft.id(), Some(configuration.clone())));
    let repeated = graph
        .add_operator(
            OperatorTypeId::new("text.transform").unwrap(),
            text_id("text.document"),
            text_id("text.document"),
        )
        .unwrap();
    assert_eq!(draft.id(), repeated.id());
    assert_eq!(repeated.configuration(), Some(&configuration));

    let encoded = serde_json::to_string(&graph).unwrap();
    let recovered: ArtifactWorkingGraph = serde_json::from_str(&encoded).unwrap();
    recovered.validate().unwrap();
    assert_eq!(recovered, graph);
    assert_eq!(recovered.context_artifact_id(), artifact_id);
    assert_eq!(recovered.expected_revision_id(), Some(revision_id));
    assert_eq!(
        recovered.operators()[0].configuration(),
        Some(&configuration)
    );
}

#[test]
fn source_operator_has_no_input_revision_or_input_data_type() {
    let artifact_id = ArtifactId::new();
    let mut graph = ArtifactWorkingGraph::new_source(artifact_id);
    let draft = graph
        .add_source_operator(
            OperatorTypeId::new("image.generate").unwrap(),
            text_id("image.raster"),
        )
        .unwrap();
    assert_eq!(graph.expected_revision_id(), None);
    assert_eq!(draft.input_data_type(), None);
    assert!(
        graph
            .add_operator(
                OperatorTypeId::new("image.resize").unwrap(),
                OperatorDataTypeId::new("image.raster").unwrap(),
                OperatorDataTypeId::new("image.raster").unwrap(),
            )
            .is_err()
    );
    graph.validate().unwrap();

    let encoded = serde_json::to_string(&graph).unwrap();
    assert!(!encoded.contains("expected_revision_id"));
    assert!(!encoded.contains("input_data_type"));
    let recovered: ArtifactWorkingGraph = serde_json::from_str(&encoded).unwrap();
    assert_eq!(recovered, graph);

    let mut anchored = ArtifactWorkingGraph::new(artifact_id, RevisionId::new());
    assert!(matches!(
        anchored.add_source_operator(
            OperatorTypeId::new("image.generate").unwrap(),
            text_id("image.raster")
        ),
        Err(DomainError::InvalidWorkingGraphAnchor)
    ));
}

#[test]
fn working_graph_removes_only_the_addressed_or_finished_operator() {
    let mut graph = ArtifactWorkingGraph::new(ArtifactId::new(), RevisionId::new());
    let edit = graph
        .add_operator(
            OperatorTypeId::new("text.edit").unwrap(),
            text_id("text.document"),
            text_id("text.document"),
        )
        .unwrap();
    graph
        .add_operator(
            OperatorTypeId::new("audio.speech_synthesize").unwrap(),
            text_id("text.document"),
            text_id("audio.clip"),
        )
        .unwrap();
    assert!(graph.remove_operator(edit.id()));
    assert!(!graph.remove_operator(edit.id()));
    assert!(graph.remove_operator_type("audio.speech_synthesize"));
    assert!(graph.is_empty());
}

#[test]
fn accepted_input_graph_rebases_without_losing_reusable_intent() {
    let original_head = RevisionId::new();
    let next_head = RevisionId::new();
    let mut graph = ArtifactWorkingGraph::new(ArtifactId::new(), original_head);
    let draft = graph
        .add_operator(
            OperatorTypeId::new("text.edit").unwrap(),
            text_id("text.document"),
            text_id("text.document"),
        )
        .unwrap();
    let draft_id = draft.id().clone();
    graph.rebase_accepted_input(next_head).unwrap();
    assert_eq!(graph.expected_revision_id(), Some(next_head));
    assert_eq!(graph.operators()[0].id(), &draft_id);

    let mut source = ArtifactWorkingGraph::new_source(ArtifactId::new());
    source.rebase_accepted_input(next_head).unwrap();
    assert_eq!(source.expected_revision_id(), Some(next_head));
    assert!(source.operators().is_empty());
}

#[test]
fn serde_loaded_identifier_is_revalidated() {
    let graph = ArtifactWorkingGraph::new(ArtifactId::new(), RevisionId::new());
    let mut value = serde_json::to_value(graph).unwrap();
    value["operators"] = serde_json::json!([{
        "id": "missing_namespace",
        "operator_type": "text.edit",
        "input_data_type": "text.document",
        "output_data_type": "text.document"
    }]);
    let malformed: ArtifactWorkingGraph = serde_json::from_value(value).unwrap();
    assert!(matches!(
        malformed.validate(),
        Err(DomainError::InvalidOperatorIdentifier { .. })
    ));
}

#[test]
fn configuration_envelope_rejects_unversioned_non_object_and_oversized_json() {
    assert!(OperatorConfigurationSchemaId::new("unversioned").is_err());
    let schema =
        OperatorConfigurationSchemaId::new("shape.operator-draft.text-transform@20260811.1")
            .unwrap();
    assert!(WorkingOperatorConfiguration::new(schema.clone(), "[]").is_err());
    assert!(
        WorkingOperatorConfiguration::new(
            schema,
            format!(r#"{{"value":"{}"}}"#, "x".repeat(64 * 1_024)),
        )
        .is_err()
    );
}
