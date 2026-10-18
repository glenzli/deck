use super::*;
use serde_json::json;
use std::{fs, path::PathBuf};

struct Fixture {
    root: PathBuf,
    project: ShapeProject,
    state: PictureBookDraftState,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("shape-book-draft-{}", uuid::Uuid::now_v7()));
        let mut project = ShapeProject::create(&root, "Book drafts").unwrap();
        let text = project
            .create_text_document("English", "[Cover] Moon\n[Author] Ada\n[1] Hello")
            .unwrap();
        let source = WorkingInput {
            artifact_id: text.artifact_id,
            revision_id: text.id,
        };
        let state=serde_json::from_value(json!({"composition":{"name":"Moon book","default_language":"en","languages":{"en":source},"pages":[{"kind":"blank"},{"kind":"blank"}],"canvas_width":1024,"canvas_height":768},"active_language":"en","page_index":1,"script":"[Cover] Moon\n[Author] Ada\n[1] Hello","script_dirty":false})).unwrap();
        Self {
            root,
            project,
            state,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn unadopted_long_script_and_workspace_selection_recover_without_accepting_material() {
    let mut f = Fixture::new();
    let before = f.project.snapshot().unwrap().artifacts;
    f.state.script = "[封面] 未采用的中文草稿 WWW\n".repeat(5000);
    f.state.script_dirty = true;
    f.state.active_language = "zh-CN".into();
    let source = f.state.composition.languages["en"];
    f.state.artwork_edit = Some(shape_domain::PictureBookArtworkEditDraft {
        output_artifact_id: ArtifactId::new(),
        page_index: 0,
        source,
    });
    let draft = f.project.create_picture_book_draft(&f.state).unwrap();
    let reopened = ShapeProject::open(&f.root).unwrap();
    let restored = reopened.open_picture_book_draft(draft.artifact_id).unwrap();
    assert_eq!(restored.state, f.state);
    assert_eq!(restored.draft_digest, draft.draft_digest);
    assert!(restored.source.is_none());
    assert!(
        reopened
            .propose_picture_book_draft(draft.artifact_id, &draft.draft_digest)
            .is_err()
    );
    let after = reopened.snapshot().unwrap().artifacts;
    assert_eq!(
        after
            .iter()
            .filter(|a| a.kind == ArtifactKind::TextDocument)
            .count(),
        1
    );
    assert!(before.iter().all(|a| after.contains(a)));
    assert_eq!(
        reopened
            .read_revision_content(source.revision_id)
            .unwrap()
            .bytes,
        b"[Cover] Moon\n[Author] Ada\n[1] Hello"
    );
}

#[test]
fn concurrent_saves_and_stale_discard_preserve_the_newer_workspace() {
    let mut f = Fixture::new();
    let draft = f.project.create_picture_book_draft(&f.state).unwrap();
    let other = ShapeProject::open(&f.root).unwrap();
    let repeated = other
        .save_picture_book_draft(draft.artifact_id, &draft.draft_digest, &f.state)
        .unwrap();
    assert_eq!(repeated.draft_digest, draft.draft_digest);
    f.state.script.push_str("\n[2] Unadopted");
    f.state.script_dirty = true;
    let changed = other
        .save_picture_book_draft(draft.artifact_id, &draft.draft_digest, &f.state)
        .unwrap();
    let mut losing = f.state.clone();
    losing.composition.pages.swap(0, 1);
    losing.page_index = 0;
    assert!(
        f.project
            .save_picture_book_draft(draft.artifact_id, &draft.draft_digest, &losing)
            .is_err()
    );
    assert!(
        f.project
            .discard_picture_book_draft(draft.artifact_id, &draft.draft_digest)
            .is_err()
    );
    assert_eq!(
        f.project
            .open_picture_book_draft(draft.artifact_id)
            .unwrap()
            .draft_digest,
        changed.draft_digest
    );
}

#[test]
fn accepted_book_uses_reserved_identity_rebases_checkpoint_and_preserves_historical_bytes() {
    let mut f = Fixture::new();
    let draft = f.project.create_picture_book_draft(&f.state).unwrap();
    let candidate = f
        .project
        .propose_picture_book_draft(draft.artifact_id, &draft.draft_digest)
        .unwrap();
    // Page selection is UI state, and must not invalidate an otherwise exact authored review.
    f.state.page_index = 0;
    let viewed = f
        .project
        .save_picture_book_draft(draft.artifact_id, &draft.draft_digest, &f.state)
        .unwrap();
    let accepted = f.project.accept_picture_book(candidate).unwrap();
    assert_eq!(accepted.artifact_id, draft.artifact_id);
    let source = WorkingInput {
        artifact_id: accepted.artifact_id,
        revision_id: accepted.id,
    };
    let bytes = f.project.read_revision_content(accepted.id).unwrap().bytes;
    let current = f
        .project
        .open_picture_book_draft(draft.artifact_id)
        .unwrap();
    assert_eq!(current.expected_head, Some(accepted.id));
    assert_eq!(current.state.page_index, 0);
    assert!(
        f.project
            .save_picture_book_draft(draft.artifact_id, &viewed.draft_digest, &f.state)
            .is_err()
    );
    f.state.composition.name = "Revised Moon".into();
    let edited = f
        .project
        .save_picture_book_draft(draft.artifact_id, &current.draft_digest, &f.state)
        .unwrap();
    let candidate = f
        .project
        .propose_picture_book_draft(draft.artifact_id, &edited.draft_digest)
        .unwrap();
    f.state.script_dirty = true;
    let dirty = f
        .project
        .save_picture_book_draft(draft.artifact_id, &edited.draft_digest, &f.state)
        .unwrap();
    assert!(f.project.accept_picture_book(candidate).is_err());
    assert_eq!(
        f.project.read_picture_book(source).unwrap().name,
        "Moon book"
    );
    assert_eq!(
        f.project.read_revision_content(accepted.id).unwrap().bytes,
        bytes
    );
    f.project
        .discard_picture_book_draft(draft.artifact_id, &dirty.draft_digest)
        .unwrap();
    assert!(f.project.picture_book_drafts().unwrap().is_empty());
    assert_eq!(
        f.project
            .open_picture_book_draft(draft.artifact_id)
            .unwrap()
            .state
            .composition
            .name,
        "Moon book"
    );
}

#[test]
fn checkpoint_bytes_are_verified_and_new_output_discard_does_not_touch_sources() {
    let mut f = Fixture::new();
    let draft = f.project.create_picture_book_draft(&f.state).unwrap();
    let graph = f
        .project
        .store
        .artifact_working_graph(draft.artifact_id)
        .unwrap()
        .unwrap();
    let checkpoint = checkpoint(book_node(&graph).unwrap()).unwrap();
    let digest = checkpoint.script_content.digest.to_string();
    let path = f
        .root
        .join("objects/b3")
        .join(&digest[..2])
        .join(&digest[2..]);
    let original = fs::read(&path).unwrap();
    fs::write(&path, b"corrupt").unwrap();
    assert!(
        f.project
            .open_picture_book_draft(draft.artifact_id)
            .is_err()
    );
    fs::write(path, original).unwrap();
    f.project
        .discard_picture_book_draft(draft.artifact_id, &draft.draft_digest)
        .unwrap();
    assert_eq!(f.project.snapshot().unwrap().artifacts.len(), 1);
}
