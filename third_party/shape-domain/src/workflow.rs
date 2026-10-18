//! Small product template catalog. Concrete consumers own drafts, execution and acceptance.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowTemplate {
    pub id: &'static str,
    pub output_kind: &'static str,
    pub steps: &'static [&'static str],
    pub slots: &'static [&'static str],
}

pub fn workflow_templates() -> Vec<WorkflowTemplate> {
    vec![
        WorkflowTemplate {
            id: "picture_book",
            output_kind: "picture_book",
            steps: &["brief", "script", "artwork", "review", "export"],
            slots: &["script", "page_artwork"],
        },
        WorkflowTemplate {
            id: "card_kit",
            output_kind: "card_kit",
            steps: &["brief", "card_list", "artwork", "review", "export"],
            slots: &["card_text", "card_artwork"],
        },
        WorkflowTemplate {
            id: "material_edit",
            output_kind: "image_raster",
            steps: &["source", "edit", "review", "accept"],
            slots: &["accepted_material"],
        },
    ]
}
