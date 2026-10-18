use crate::{
    AI_IMAGE_GENERATE_FROM_MATERIALS_OPERATOR_TYPE, AI_IMAGE_GENERATE_OPERATOR_TYPE,
    AI_IMAGE_MAX_CANDIDATES, AI_IMAGE_MAX_DIMENSION, AI_IMAGE_MAX_PIXELS,
    AI_IMAGE_PARAMETERS_REVISION, AI_IMAGE_RASTER_DATA_TYPE,
    AiImageGenerateFromMaterialsParameters, AiImageGenerateParameters, AiImageMaterialReference,
    AiImageMaterialRole, AiImageOperation, AiImageOperatorFamily, AiImageOutputCanvas,
    AiImagePortCardinality, AiImagePortDataType, Constraint, ConstraintKind, ConstraintStrength,
    RevisionId,
};

fn output() -> AiImageOutputCanvas {
    AiImageOutputCanvas::new(1536, 1024).unwrap()
}

fn constraint(kind: ConstraintKind) -> Constraint {
    Constraint::new(
        kind,
        ConstraintStrength::Hard,
        "keep this requirement exact",
        None,
    )
    .unwrap()
}

#[test]
fn families_are_distinct_but_share_one_candidate_output_contract() {
    let generate = AiImageOperatorFamily::Generate.contract();
    assert_eq!(generate.operator_type, AI_IMAGE_GENERATE_OPERATOR_TYPE);
    assert!(generate.inputs.is_empty());
    assert_eq!(generate.outputs.len(), 1);
    assert_eq!(generate.outputs[0].id, "output.image");
    assert_eq!(
        generate.outputs[0].data_type,
        AiImagePortDataType::ImageRaster
    );
    assert_eq!(
        generate.outputs[0].data_type.key(),
        AI_IMAGE_RASTER_DATA_TYPE
    );
    assert_eq!(
        generate.outputs[0].cardinality,
        AiImagePortCardinality::Required
    );

    let conditioned = AiImageOperatorFamily::GenerateFromMaterials.contract();
    assert_eq!(
        conditioned.operator_type,
        AI_IMAGE_GENERATE_FROM_MATERIALS_OPERATOR_TYPE
    );
    assert_eq!(conditioned.inputs.len(), 1);
    assert_eq!(conditioned.inputs[0].id, "input.materials");
    assert_eq!(
        conditioned.inputs[0].cardinality,
        AiImagePortCardinality::OneOrMore
    );
    assert_eq!(conditioned.outputs, generate.outputs);
}

#[test]
fn source_less_generation_is_versioned_exact_and_provider_neutral() {
    let parameters = AiImageGenerateParameters::new(
        "A quiet observatory above a sea of clouds",
        output(),
        3,
        vec![constraint(ConstraintKind::Avoid)],
    )
    .unwrap();
    assert_eq!(parameters.schema_revision(), AI_IMAGE_PARAMETERS_REVISION);
    assert_eq!(parameters.candidate_count(), 3);
    assert_eq!(parameters.output().pixel_count(), 1536 * 1024);

    let operation = AiImageOperation::Generate(parameters);
    assert_eq!(operation.family(), AiImageOperatorFamily::Generate);
    let encoded = serde_json::to_value(&operation).unwrap();
    assert_eq!(encoded["operator_type"], AI_IMAGE_GENERATE_OPERATOR_TYPE);
    assert_eq!(encoded["parameters"]["candidate_count"], 3);
    assert!(encoded.get("model").is_none());
    assert!(encoded.get("provider").is_none());
    assert!(encoded["parameters"].get("model").is_none());
    assert!(encoded["parameters"].get("provider").is_none());

    let round_trip: AiImageOperation = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(round_trip, operation);

    let mut unknown = encoded;
    unknown["parameters"]["model"] = serde_json::json!("physical-model-name");
    assert!(serde_json::from_value::<AiImageOperation>(unknown).is_err());

    let mut unknown_envelope = serde_json::to_value(&operation).unwrap();
    unknown_envelope["provider"] = serde_json::json!("physical-provider-name");
    assert!(serde_json::from_value::<AiImageOperation>(unknown_envelope).is_err());
}

#[test]
fn source_less_generation_rejects_unanchored_preservation() {
    assert!(
        AiImageGenerateParameters::new(
            "Preserve the same person",
            output(),
            1,
            vec![constraint(ConstraintKind::PreserveIdentity)],
        )
        .is_err()
    );
    assert!(AiImageGenerateParameters::new("   ", output(), 1, Vec::new(),).is_err());
}

#[test]
fn material_conditioned_generation_requires_explicit_unique_references() {
    let source_revision = RevisionId::new();
    let style_revision = RevisionId::new();
    let materials = vec![
        AiImageMaterialReference::new(
            source_revision,
            AiImageMaterialRole::Source,
            "Portrait source",
        )
        .unwrap(),
        AiImageMaterialReference::new(
            style_revision,
            AiImageMaterialRole::Lighting,
            "Soft window light",
        )
        .unwrap(),
    ];
    let parameters = AiImageGenerateFromMaterialsParameters::new(
        "Move the portrait into a quiet library",
        materials,
        output(),
        4,
        vec![constraint(ConstraintKind::PreserveIdentity)],
    )
    .unwrap();
    assert_eq!(parameters.materials().len(), 2);
    assert_eq!(parameters.materials()[0].revision_id(), source_revision);
    assert_eq!(
        parameters.materials()[1].role(),
        AiImageMaterialRole::Lighting
    );
    assert_eq!(parameters.materials()[1].label(), "Soft window light");

    let operation = AiImageOperation::GenerateFromMaterials(parameters);
    assert_eq!(
        operation.family(),
        AiImageOperatorFamily::GenerateFromMaterials
    );
    let encoded = serde_json::to_value(&operation).unwrap();
    assert_eq!(
        encoded["operator_type"],
        AI_IMAGE_GENERATE_FROM_MATERIALS_OPERATOR_TYPE
    );
    assert_eq!(
        encoded["parameters"]["materials"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        serde_json::from_value::<AiImageOperation>(encoded).unwrap(),
        operation
    );

    assert!(
        AiImageGenerateFromMaterialsParameters::new(
            "No hidden optional material path",
            Vec::new(),
            output(),
            1,
            Vec::new(),
        )
        .is_err()
    );
    let duplicate = vec![
        AiImageMaterialReference::new(source_revision, AiImageMaterialRole::Source, "Source")
            .unwrap(),
        AiImageMaterialReference::new(source_revision, AiImageMaterialRole::Style, "Style")
            .unwrap(),
    ];
    assert!(
        AiImageGenerateFromMaterialsParameters::new(
            "Duplicate bindings are ambiguous",
            duplicate,
            output(),
            1,
            Vec::new(),
        )
        .is_err()
    );
}

#[test]
fn every_nested_authored_field_is_revalidated_on_deserialization() {
    let valid = serde_json::json!({
        "schema_revision": AI_IMAGE_PARAMETERS_REVISION,
        "instruction": "Create a misty mountain valley",
        "output": {"width": 1024, "height": 1024},
        "candidate_count": 2,
        "constraints": [{
            "kind": "avoid",
            "strength": "hard",
            "value": "no text",
            "target_component": null
        }]
    });
    assert!(serde_json::from_value::<AiImageGenerateParameters>(valid.clone()).is_ok());

    let mut wrong_revision = valid.clone();
    wrong_revision["schema_revision"] = serde_json::json!("future");
    assert!(serde_json::from_value::<AiImageGenerateParameters>(wrong_revision).is_err());

    let mut unknown_canvas_field = valid.clone();
    unknown_canvas_field["output"]["sampler"] = serde_json::json!("hidden");
    assert!(serde_json::from_value::<AiImageGenerateParameters>(unknown_canvas_field).is_err());

    let mut unknown_constraint_field = valid.clone();
    unknown_constraint_field["constraints"][0]["provider_hint"] = serde_json::json!("hidden");
    assert!(serde_json::from_value::<AiImageGenerateParameters>(unknown_constraint_field).is_err());

    let mut invalid_candidate_count = valid.clone();
    invalid_candidate_count["candidate_count"] = serde_json::json!(AI_IMAGE_MAX_CANDIDATES + 1);
    assert!(serde_json::from_value::<AiImageGenerateParameters>(invalid_candidate_count).is_err());

    let material = RevisionId::new();
    let invalid_material = serde_json::json!({
        "schema_revision": AI_IMAGE_PARAMETERS_REVISION,
        "instruction": "Use the reference",
        "materials": [{
            "revision_id": material,
            "role": "voice",
            "label": "not an image role"
        }],
        "output": {"width": 1024, "height": 1024},
        "candidate_count": 1,
        "constraints": []
    });
    assert!(
        serde_json::from_value::<AiImageGenerateFromMaterialsParameters>(invalid_material).is_err()
    );
}

#[test]
fn output_and_candidate_bounds_fail_closed() {
    assert_eq!(
        AiImageOutputCanvas::new(8192, 8192).unwrap().pixel_count(),
        AI_IMAGE_MAX_PIXELS
    );
    assert!(AiImageOutputCanvas::new(AI_IMAGE_MAX_DIMENSION, 1).is_ok());
    assert!(AiImageOutputCanvas::new(AI_IMAGE_MAX_DIMENSION + 1, 1).is_err());
    assert!(AiImageOutputCanvas::new(8193, 8192).is_err());
    assert!(AiImageOutputCanvas::new(0, 1024).is_err());

    assert!(AiImageGenerateParameters::new("one candidate", output(), 1, Vec::new()).is_ok());
    assert!(
        AiImageGenerateParameters::new(
            "maximum candidates",
            output(),
            AI_IMAGE_MAX_CANDIDATES,
            Vec::new(),
        )
        .is_ok()
    );
    assert!(AiImageGenerateParameters::new("zero candidates", output(), 0, Vec::new()).is_err());
}
