use super::{ProjectMetadata, SHAPE_PROJECT_SCHEMA_REVISION};

#[test]
fn new_project_uses_current_schema_revision() {
    let project = ProjectMetadata::new("Summer Portrait").unwrap();
    assert_eq!(project.schema_revision, SHAPE_PROJECT_SCHEMA_REVISION);
}
