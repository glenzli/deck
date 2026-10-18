use super::*;
fn request() -> PictureBookRequest {
    let source = WorkingInput {
        artifact_id: crate::ArtifactId::new(),
        revision_id: crate::RevisionId::new(),
    };
    serde_json::from_value(serde_json::json!({"name":"Moon / 月亮","default_language":"zh-CN","languages":{"zh-CN":source},
        "pages":[{"kind":"blank"},{"kind":"blank","print_only":true}],"canvas_width":1024,"canvas_height":768})).unwrap()
}
#[test]
fn legacy_composition_defaults_preserve_native_page_semantics() {
    let request = request();
    assert_eq!(request.font_size, 26);
    assert!(!request.pages[0].print_only());
    assert!(request.pages[1].print_only());
    assert!(request.validate().is_ok());
    let mut document = PictureBookDocument::new(request);
    document.schema = "future".into();
    assert!(document.validate().is_err());
}
#[test]
fn cover_visibility_and_one_revision_per_material_are_required() {
    let mut request = request();
    request.pages[0] = PictureBookPage::Blank { print_only: true };
    assert!(request.validate().is_err());
    request.pages[0] = PictureBookPage::Blank { print_only: false };
    let mut second = request.languages["zh-CN"];
    second.revision_id = crate::RevisionId::new();
    request.languages.insert("en".into(), second);
    assert!(request.validate().is_err());
}
