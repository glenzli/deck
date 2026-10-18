use crate::{Artifact, ArtifactKind, ContentDigest, ContentRef, RevisionId, TransformationId};

use super::ArtifactRevision;

#[test]
fn artifact_starts_without_an_accepted_revision() {
    let artifact = Artifact::new("Cover", ArtifactKind::ImageComposite).unwrap();
    assert_eq!(artifact.name, "Cover");
    assert_eq!(artifact.accepted_revision, None);
}

#[test]
fn revision_binds_content_and_creative_cause() {
    let artifact = Artifact::new("Story", ArtifactKind::TextDocument).unwrap();
    let content = ContentRef::new(ContentDigest::from_bytes(b"hello"), "text/plain", 5).unwrap();
    let parent = RevisionId::new();
    let transformation = TransformationId::new();
    let revision =
        ArtifactRevision::new(artifact.id, vec![parent], content, transformation, 42).unwrap();
    assert_eq!(revision.parents, vec![parent]);
    assert!(revision.content_contract.is_none());
    assert_eq!(revision.transformation_id, transformation);
}
