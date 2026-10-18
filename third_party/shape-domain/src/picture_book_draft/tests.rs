use super::*;
use serde_json::json;

fn state() -> PictureBookDraftState {
    serde_json::from_value(json!({"composition":{"name":"Unfinished book","default_language":"en","languages":{},"pages":[{"kind":"blank"}],"canvas_width":1024,"canvas_height":768},"active_language":"zh-CN","page_index":0,"script":"[封面] 尚未采用","script_dirty":true})).unwrap()
}
#[test]
fn incomplete_language_bindings_and_script_syntax_are_only_workspace_state() {
    let state = state();
    state.validate().unwrap();
    assert!(state.composition.validate().is_err());
    let content = ContentRef::new(
        crate::ContentDigest::from_bytes(state.script.as_bytes()),
        PICTURE_BOOK_DRAFT_SCRIPT_MEDIA_TYPE,
        state.script.len() as u64,
    )
    .unwrap();
    let checkpoint = state.checkpoint(content).unwrap();
    let restored =
        PictureBookDraftCheckpoint::from_configuration(&checkpoint.configuration().unwrap())
            .unwrap();
    assert_eq!(restored.state(state.script.clone()), state);
}
#[test]
fn large_script_is_referenced_and_forged_content_identity_is_rejected() {
    let mut state = state();
    state.script = "W未采用".repeat(20_000);
    let mut content = ContentRef::new(
        crate::ContentDigest::from_bytes(state.script.as_bytes()),
        PICTURE_BOOK_DRAFT_SCRIPT_MEDIA_TYPE,
        state.script.len() as u64,
    )
    .unwrap();
    let checkpoint = state.checkpoint(content.clone()).unwrap();
    assert!(checkpoint.configuration().unwrap().json().len() < 4096);
    content.digest = crate::ContentDigest::from_bytes(b"different");
    assert!(state.checkpoint(content).is_err());
    state.page_index = 1;
    assert!(state.validate().is_err());
}
