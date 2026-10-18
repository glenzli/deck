use std::str::FromStr;

use super::{ArtifactId, ProjectId, RevisionId, TransformationId};

#[test]
fn typed_id_round_trips_without_cross_type_conversion() {
    let project = ProjectId::new();
    let encoded = project.to_string();
    assert_eq!(ProjectId::from_str(&encoded).unwrap(), project);

    assert_ne!(ArtifactId::new().to_string(), RevisionId::new().to_string());
    assert_ne!(TransformationId::new().to_string(), encoded);
}
