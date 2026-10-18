use crate::{
    AiImageGenerateParameters, AiImageOutputCanvas, Artifact, ArtifactKind, Constraint,
    ConstraintKind, ConstraintStrength, IntentSpec, RasterCrop, RasterDropShadow,
    RasterGaussianBlur, RasterResize, RasterResizeAspectPolicy, RasterResizeDimensions,
    RasterResizeResampling, RasterShadowColor, RasterTransform, RasterUnsharpMask, RevisionId,
    TransformationOperation,
};

use super::{Transformation, TransformationKind};

#[test]
fn change_and_preserve_are_distinct_contracts() {
    let artifact = Artifact::new("Portrait", ArtifactKind::ImageComposite).unwrap();
    let constraint = Constraint::new(
        ConstraintKind::PreserveIdentity,
        ConstraintStrength::Hard,
        "character identity",
        Some("subject".to_owned()),
    )
    .unwrap();
    let transformation = Transformation::new(
        TransformationKind::GenerativeEdit,
        artifact.id,
        Vec::new(),
        IntentSpec::new("change the background to a quiet summer afternoon").unwrap(),
        vec![constraint],
        Vec::new(),
    )
    .unwrap();
    assert_eq!(transformation.constraints.len(), 1);
    assert_eq!(
        transformation.intent.as_str(),
        "change the background to a quiet summer afternoon"
    );
}

#[test]
fn typed_resize_operation_belongs_only_to_deterministic_edits() {
    let artifact = Artifact::new("Portrait", ArtifactKind::ImageRaster).unwrap();
    let operation = TransformationOperation::RasterResize(RasterResize::new(
        RasterResizeDimensions::new(1280, 720).unwrap(),
        RasterResizeAspectPolicy::FitWithin,
        RasterResizeResampling::Lanczos3,
    ));
    let deterministic = Transformation::new_with_operation(
        TransformationKind::DeterministicEdit,
        artifact.id,
        vec![RevisionId::new()],
        IntentSpec::new("resize the raster").unwrap(),
        Vec::new(),
        Vec::new(),
        Some(operation.clone()),
    )
    .unwrap();
    assert_eq!(deterministic.operation, Some(operation.clone()));

    assert!(
        Transformation::new_with_operation(
            TransformationKind::GenerativeEdit,
            artifact.id,
            Vec::new(),
            IntentSpec::new("create a variation").unwrap(),
            Vec::new(),
            Vec::new(),
            Some(operation),
        )
        .is_err()
    );
}

#[test]
fn typed_transform_and_blur_belong_only_to_deterministic_edits() {
    let artifact = Artifact::new("Portrait", ArtifactKind::ImageRaster).unwrap();
    for operation in [
        TransformationOperation::RasterTransform(RasterTransform::FlipVertical),
        TransformationOperation::RasterGaussianBlur(RasterGaussianBlur::new(4).unwrap()),
        TransformationOperation::RasterDropShadow(
            RasterDropShadow::new(4, 6, 3, RasterShadowColor::new(0, 0, 0, 128).unwrap()).unwrap(),
        ),
        TransformationOperation::RasterUnsharpMask(RasterUnsharpMask::new(2, 1_250, 4).unwrap()),
    ] {
        let deterministic = Transformation::new_with_operation(
            TransformationKind::DeterministicEdit,
            artifact.id,
            vec![RevisionId::new()],
            IntentSpec::new("deterministic raster edit").unwrap(),
            Vec::new(),
            Vec::new(),
            Some(operation.clone()),
        );
        assert!(deterministic.is_ok());

        let generative = Transformation::new_with_operation(
            TransformationKind::GenerativeEdit,
            artifact.id,
            Vec::new(),
            IntentSpec::new("wrong family").unwrap(),
            Vec::new(),
            Vec::new(),
            Some(operation),
        );
        assert!(matches!(
            generative,
            Err(crate::DomainError::InvalidTransformationOperation)
        ));
    }
}

#[test]
fn duplicate_inputs_are_rejected() {
    let artifact = Artifact::new("Story", ArtifactKind::TextDocument).unwrap();
    let revision = RevisionId::new();
    let result = Transformation::new(
        TransformationKind::TextRewrite,
        artifact.id,
        vec![revision, revision],
        IntentSpec::new("make it quieter").unwrap(),
        Vec::new(),
        Vec::new(),
    );
    assert!(result.is_err());
}

#[test]
fn typed_crop_operation_belongs_only_to_deterministic_edits() {
    let artifact = Artifact::new("Portrait", ArtifactKind::ImageRaster).unwrap();
    let operation =
        TransformationOperation::RasterCrop(RasterCrop::new(1, 2, 30, 20, 100, 80).unwrap());
    let deterministic = Transformation::new_with_operation(
        TransformationKind::DeterministicEdit,
        artifact.id,
        vec![RevisionId::new()],
        IntentSpec::new("crop the raster").unwrap(),
        Vec::new(),
        Vec::new(),
        Some(operation.clone()),
    )
    .unwrap();
    assert_eq!(deterministic.operation, Some(operation.clone()));

    let invalid = Transformation::new_with_operation(
        TransformationKind::GenerativeEdit,
        artifact.id,
        Vec::new(),
        IntentSpec::new("create a variation").unwrap(),
        Vec::new(),
        Vec::new(),
        Some(operation),
    );
    assert!(invalid.is_err());
}

#[test]
fn typed_ai_image_generation_is_source_less_and_generative() {
    let artifact = Artifact::new("Generated cover", ArtifactKind::ImageRaster).unwrap();
    let parameters = AiImageGenerateParameters::new(
        "A quiet blue circle on white",
        AiImageOutputCanvas::new(1024, 1024).unwrap(),
        1,
        Vec::new(),
    )
    .unwrap();
    let operation = TransformationOperation::AiImageGenerate(parameters.clone());
    let transformation = Transformation::new_with_operation(
        TransformationKind::GenerativeEdit,
        artifact.id,
        Vec::new(),
        IntentSpec::new("Generate a raster image from authored intent").unwrap(),
        Vec::new(),
        Vec::new(),
        Some(operation.clone()),
    )
    .unwrap();
    assert_eq!(transformation.operation, Some(operation.clone()));
    let encoded = serde_json::to_value(&transformation).unwrap();
    assert_eq!(encoded["operation"]["operation"], "ai_image_generate");

    for (kind, inputs) in [
        (TransformationKind::DeterministicEdit, Vec::new()),
        (TransformationKind::GenerativeEdit, vec![RevisionId::new()]),
    ] {
        assert!(
            Transformation::new_with_operation(
                kind,
                artifact.id,
                inputs,
                IntentSpec::new("Invalid AI Image operation").unwrap(),
                Vec::new(),
                Vec::new(),
                Some(operation.clone()),
            )
            .is_err()
        );
    }
}
