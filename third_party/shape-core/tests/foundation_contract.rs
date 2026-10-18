use std::{fs, path::PathBuf};

use shape_core::ShapeProject;
use shape_domain::{ArtifactKind, Constraint, ConstraintKind, ConstraintStrength, IntentSpec};
use uuid::Uuid;

fn test_root() -> PathBuf {
    std::env::temp_dir().join(format!("shape-core-e2e-{}", Uuid::now_v7()))
}

#[test]
fn candidate_does_not_advance_history_until_acceptance() {
    let root = test_root();
    let mut project = ShapeProject::create(&root, "Foundation").expect("project creates");
    let artifact = project
        .create_artifact("Story", ArtifactKind::TextDocument)
        .expect("artifact creates");
    let candidate = project
        .propose_text(
            artifact.id,
            None,
            "A quiet summer afternoon.",
            IntentSpec::new("Import the opening line").expect("intent valid"),
            vec![
                Constraint::new(
                    ConstraintKind::PreserveContent,
                    ConstraintStrength::Hard,
                    "Retain the subject",
                    None,
                )
                .expect("constraint valid"),
            ],
        )
        .expect("candidate executes");

    assert_eq!(candidate.text(), "A quiet summer afternoon.");
    assert!(
        project
            .read_accepted(artifact.id)
            .expect("head reads")
            .is_none()
    );

    let accepted = project.accept_text(candidate).expect("candidate accepts");
    drop(project);
    let reopened = ShapeProject::open(&root).expect("project reopens");
    let content = reopened
        .read_accepted(artifact.id)
        .expect("head reads")
        .expect("accepted content exists");
    assert_eq!(content.revision.id, accepted.id);
    assert_eq!(content.bytes, b"A quiet summer afternoon.");
    fs::remove_dir_all(&root).expect("test bundle removes");
}

#[test]
fn candidate_can_branch_into_a_new_artifact_without_advancing_its_source() {
    let root = test_root();
    let mut project = ShapeProject::create(&root, "Foundation").expect("project creates");
    let source = project
        .create_artifact("Story", ArtifactKind::TextDocument)
        .expect("artifact creates");
    let initial = project
        .propose_text(
            source.id,
            None,
            "A summer afternoon.",
            IntentSpec::new("Import the opening line").expect("intent valid"),
            Vec::new(),
        )
        .expect("candidate executes");
    let source_revision = project.accept_text(initial).expect("source accepts");
    let branch_candidate = project
        .propose_text(
            source.id,
            Some(source_revision.id),
            "A quiet summer afternoon.",
            IntentSpec::new("Make the opening quieter").expect("intent valid"),
            Vec::new(),
        )
        .expect("branch candidate executes");
    let branch_revision = project
        .branch_text_candidate(branch_candidate, "Story — Quiet")
        .expect("branch accepts");

    assert!(branch_revision.parents.is_empty());
    assert_ne!(branch_revision.artifact_id, source.id);
    assert_eq!(
        project
            .read_accepted(source.id)
            .expect("source reads")
            .expect("source exists")
            .revision
            .id,
        source_revision.id
    );
    let branch_transformation = project
        .transformation(branch_revision.transformation_id)
        .expect("branch transformation reads");
    assert_eq!(branch_transformation.inputs, vec![source_revision.id]);
    drop(project);

    let reopened = ShapeProject::open(&root).expect("project reopens");
    let snapshot = reopened.snapshot().expect("snapshot reads");
    assert_eq!(snapshot.artifacts.len(), 2);
    let branch = snapshot
        .artifacts
        .iter()
        .find(|artifact| artifact.id == branch_revision.artifact_id)
        .expect("branch artifact exists");
    assert_eq!(branch.name, "Story — Quiet");
    assert_eq!(
        reopened
            .read_accepted(branch.id)
            .expect("branch reads")
            .expect("branch content exists")
            .bytes,
        b"A quiet summer afternoon."
    );
    fs::remove_dir_all(&root).expect("test bundle removes");
}
