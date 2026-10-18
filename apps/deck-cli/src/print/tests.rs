use super::*;
use std::{fs, path::PathBuf};

struct Fixture {
    root: PathBuf,
    project: ShapeProject,
    source: WorkingInput,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("deck-sheet-{}", uuid::Uuid::now_v7()));
        let mut project = ShapeProject::create(root.join("cards.deck"), "Sheet fixture").unwrap();
        let composition: CardKitRequest = serde_json::from_value(json!({
            "name":"Sheet fixture", "layout":{"mode":"text"},
            "cards":[{"id":"a","title":"A","quantity":6},
                     {"id":"b","title":"B","quantity":4},
                     {"id":"c","title":"C","quantity":2}]
        }))
        .unwrap();
        let draft = project.create_card_kit_draft(&composition).unwrap();
        let candidate = project
            .propose_card_kit(draft.artifact_id, &draft.draft_digest)
            .unwrap();
        let accepted = project.accept_card_kit(candidate).unwrap();
        Self {
            root,
            project,
            source: WorkingInput {
                artifact_id: accepted.artifact_id,
                revision_id: accepted.id,
            },
        }
    }
    fn plan(&self, imposition: Value) -> Value {
        plan(
            &self.project,
            self.source,
            CardPrintRequestOptions::default(),
            Some(serde_json::from_value(imposition).unwrap()),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn n(value: &Value, key: &str) -> f64 {
    value[key].as_f64().unwrap()
}

fn safe_geometry(plan: &Value) {
    let margin = n(&plan["options"], "margin_mm");
    let half = n(&plan["options"], "stroke_width_mm") / 2.0;
    let w = n(plan, "sheet_width_mm");
    let h = n(plan, "sheet_height_mm");
    for page in plan["pages"].as_array().unwrap() {
        let slots = page["slots"].as_array().unwrap();
        assert_eq!(
            page["crop_marks"].as_array().unwrap().len(),
            slots.len() * 8
        );
        for (i, slot) in slots.iter().enumerate() {
            let r = &slot["rect_mm"];
            let content = &slot["content_rect_mm"];
            assert!(n(content, "x") >= n(r, "x") - 1e-6 && n(content, "y") >= n(r, "y") - 1e-6);
            assert!(n(content, "x") + n(content, "width") <= n(r, "x") + n(r, "width") + 1e-6);
            assert!(n(content, "y") + n(content, "height") <= n(r, "y") + n(r, "height") + 1e-6);
            for other in &slots[i + 1..] {
                let o = &other["rect_mm"];
                assert!(
                    n(r, "x") + n(r, "width") <= n(o, "x") + 1e-6
                        || n(o, "x") + n(o, "width") <= n(r, "x") + 1e-6
                        || n(r, "y") + n(r, "height") <= n(o, "y") + 1e-6
                        || n(o, "y") + n(o, "height") <= n(r, "y") + 1e-6
                );
            }
        }
        for mark in page["crop_marks"].as_array().unwrap() {
            let left = n(mark, "x1").min(n(mark, "x2")) - half;
            let right = n(mark, "x1").max(n(mark, "x2")) + half;
            let top = n(mark, "y1").min(n(mark, "y2")) - half;
            let bottom = n(mark, "y1").max(n(mark, "y2")) + half;
            assert!(
                left >= margin - 1e-6 && top >= margin - 1e-6,
                "grid {}x{}: left {left}, top {top}, margin {margin}",
                plan["columns"],
                plan["rows"]
            );
            assert!(right <= w - margin + 1e-6 && bottom <= h - margin + 1e-6);
            for slot in slots {
                let r = &slot["rect_mm"];
                assert!(
                    right <= n(r, "x") + 1e-6
                        || left >= n(r, "x") + n(r, "width") - 1e-6
                        || bottom <= n(r, "y") + 1e-6
                        || top >= n(r, "y") + n(r, "height") - 1e-6
                );
            }
        }
    }
}

#[test]
fn legacy_plan_is_unchanged_without_imposition() {
    let f = Fixture::new();
    let actual = plan(
        &f.project,
        f.source,
        CardPrintRequestOptions::default(),
        None,
    )
    .unwrap();
    let original = serde_json::to_value(
        f.project
            .plan_card_print_request(f.source, &CardPrintRequestOptions::default())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(actual, original);
}

#[test]
fn quick_grids_reuse_admitted_faces_and_keep_crop_marks_clear() {
    let f = Fixture::new();
    for (c, r) in [(1, 1), (2, 2), (2, 3), (3, 3), (3, 4), (4, 4), (4, 2)] {
        let plan = f.plan(json!({"columns":c,"rows":r}));
        assert_eq!(plan["columns"], c);
        assert_eq!(plan["rows"], r);
        assert_eq!(plan["slots_per_page"], c * r);
        assert_eq!(plan["total_copies"], 12);
        assert_eq!(plan["source"], json!(f.source));
        safe_geometry(&plan);
        let options = serde_json::from_value(plan["options"].clone()).unwrap();
        let original = serde_json::to_value(
            f.project
                .plan_card_print_request(f.source, &options)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            plan["cards"], original["cards"],
            "faces never get independently rendered"
        );
    }
}

#[test]
fn mixed_and_repeat_modes_preserve_copy_identity_and_authored_state() {
    let mut f = Fixture::new();
    let before = f.project.open_card_kit_draft(f.source.artifact_id).unwrap();
    let mixed = f.plan(json!({"columns":2,"rows":3}));
    assert_eq!(mixed["page_count"], 2);
    assert_eq!(mixed["pages"][0]["slots"][5]["card_id"], "a");
    assert_eq!(mixed["pages"][1]["slots"][0]["card_id"], "b");
    let each = f.plan(json!({"columns":3,"rows":3,"content":"one_each"}));
    assert_eq!(each["total_copies"], 3);
    assert_eq!(each["page_count"], 1);
    assert_eq!(each["pages"][0]["slots"][2]["card_id"], "c");
    let fill = f.plan(json!({"columns":3,"rows":4,"content":"repeat","card_id":"b"}));
    assert_eq!(fill["total_copies"], 12);
    assert_eq!(fill["cards"].as_array().unwrap().len(), 1);
    assert!(
        fill["pages"][0]["slots"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["card_id"] == "b")
    );
    let repeat = f.plan(json!({"columns":3,"rows":4,"content":"repeat","card_id":"b","copies":13}));
    assert_eq!(repeat["pages"][1]["slots"][0]["copy_index"], 13);
    safe_geometry(&repeat);
    let after = f.project.open_card_kit_draft(f.source.artifact_id).unwrap();
    assert_eq!(before.draft_digest, after.draft_digest);
    assert_eq!(before.expected_head, after.expected_head);
}

#[test]
fn invalid_grid_selection_quantity_and_readability_are_rejected() {
    let f = Fixture::new();
    for value in [
        json!({"columns":0,"rows":2}),
        json!({"columns":2}),
        json!({"columns":9,"rows":2}),
        json!({"content":"repeat","card_id":"absent"}),
        json!({"content":"repeat","card_id":"a","copies":0}),
        json!({"content":"repeat","card_id":"a","copies":4097}),
    ] {
        let result = plan(
            &f.project,
            f.source,
            CardPrintRequestOptions::default(),
            Some(serde_json::from_value(value).unwrap()),
        );
        assert!(result.is_err());
    }
    let options = CardPrintRequestOptions {
        gap_mm: 0.0,
        ..Default::default()
    };
    assert!(plan(&f.project, f.source, options, Some(Imposition::default())).is_err());
    assert!(
        serde_json::from_value::<Imposition>(json!({"columns":2,"rows":2,"unknown":true})).is_err()
    );
}
