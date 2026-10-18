use super::*;
use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use serde_json::{Value, json};
use shape_domain::{CardKitRequest, IntentSpec, WorkingInput};
use std::{fs, io::Read, path::PathBuf};

struct Fixture {
    root: PathBuf,
    project: ShapeProject,
    composition: CardKitRequest,
    png: Vec<u8>,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("shape-card-kit-{}", uuid::Uuid::now_v7()));
        fs::create_dir(&root).unwrap();
        let mut project = ShapeProject::create(root.join("cards.shape"), "Card fixture").unwrap();
        let mut png = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(
                &[80, 120, 180, 255].repeat(4),
                2,
                2,
                ColorType::Rgba8.into(),
            )
            .unwrap();
        let image = project.import_raster_bytes(png.clone(), "Art").unwrap();
        let text = project
            .create_text_document("Rules", "Draw one card. 摸一张牌。")
            .unwrap();
        let pin = |r: &ArtifactRevision| WorkingInput {
            artifact_id: r.artifact_id,
            revision_id: r.id,
        };
        let composition=serde_json::from_value(json!({"name":"Moon <cards>","brief":"A tiny cooperative game","style":"ink and moonlight","cards":[
            {"id":"fox","title":"Fox <&>","card_type":"Creature","cost":"2","stats":"2 / 3","text_material":pin(&text),"artwork":pin(&image),"quantity":2},
            {"id":"owl","title":"夜枭","rules":"Look at a card.","artwork":pin(&image)}]})).unwrap();
        Self {
            root,
            project,
            composition,
            png,
        }
    }
    fn draft(&mut self) -> CardKitDraft {
        self.project
            .create_card_kit_draft(&self.composition)
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn draft_recovers_direction_rows_and_pins_and_saves_use_exact_graph_cas() {
    let mut f = Fixture::new();
    let draft = f.draft();
    let mut other = ShapeProject::open(f.root.join("cards.shape")).unwrap();
    let recovered = other.open_card_kit_draft(draft.artifact_id).unwrap();
    assert_eq!(recovered.composition, f.composition);
    assert_eq!(recovered.draft_digest, draft.draft_digest);
    let repeated = other
        .save_card_kit_draft(draft.artifact_id, &draft.draft_digest, &f.composition)
        .unwrap();
    assert_eq!(repeated.draft_digest, draft.draft_digest);
    let mut edited = f.composition.clone();
    edited.cards.swap(0, 1);
    edited.cards[0].art_prompt = "Moon owl".into();
    let saved = other
        .save_card_kit_draft(draft.artifact_id, &draft.draft_digest, &edited)
        .unwrap();
    assert_ne!(saved.draft_digest, draft.draft_digest);
    assert!(
        f.project
            .save_card_kit_draft(draft.artifact_id, &draft.draft_digest, &f.composition)
            .is_err()
    );
    let current = f.project.open_card_kit_draft(draft.artifact_id).unwrap();
    assert_eq!(current.composition.cards[0].id, "owl");
    assert_eq!(
        current.composition.cards[0].artwork,
        f.composition.cards[1].artwork
    );
    assert!(
        f.project
            .discard_card_kit_draft(draft.artifact_id, &draft.draft_digest)
            .is_err()
    );
    assert_eq!(current.composition, edited);
}

#[test]
fn candidates_do_not_accept_automatically_and_changed_drafts_or_inputs_reject_acceptance() {
    let mut f = Fixture::new();
    let draft = f.draft();
    let candidate = f
        .project
        .propose_card_kit(draft.artifact_id, &draft.draft_digest)
        .unwrap();
    assert!(
        f.project
            .snapshot()
            .unwrap()
            .artifacts
            .iter()
            .find(|a| a.id == draft.artifact_id)
            .unwrap()
            .accepted_revision
            .is_none()
    );
    let mut edited = f.composition.clone();
    edited.cards[0].title = "Changed fox".into();
    let saved = f
        .project
        .save_card_kit_draft(draft.artifact_id, &draft.draft_digest, &edited)
        .unwrap();
    assert!(f.project.accept_card_kit(candidate).is_err());
    let candidate = f
        .project
        .propose_card_kit(draft.artifact_id, &saved.draft_digest)
        .unwrap();
    let source = f.composition.cards[0].text_material.unwrap();
    let updated = f
        .project
        .propose_text(
            source.artifact_id,
            Some(source.revision_id),
            "Changed rules",
            IntentSpec::new("Revise rules").unwrap(),
            vec![],
        )
        .unwrap();
    f.project.accept_text(updated).unwrap();
    assert!(f.project.accept_card_kit(candidate).is_err());
    assert!(
        f.project
            .snapshot()
            .unwrap()
            .artifacts
            .iter()
            .find(|a| a.id == draft.artifact_id)
            .unwrap()
            .accepted_revision
            .is_none()
    );
}

#[test]
fn accept_rebases_draft_old_head_candidates_fail_and_historical_exports_keep_exact_bytes() {
    let mut f = Fixture::new();
    let draft = f.draft();
    let candidate = f
        .project
        .propose_card_kit(draft.artifact_id, &draft.draft_digest)
        .unwrap();
    let stale = candidate.clone();
    let accepted = f.project.accept_card_kit(candidate).unwrap();
    assert!(f.project.accept_card_kit(stale).is_err());
    let first = WorkingInput {
        artifact_id: accepted.artifact_id,
        revision_id: accepted.id,
    };
    let rebased = f.project.open_card_kit_draft(draft.artifact_id).unwrap();
    assert_eq!(rebased.expected_head, Some(accepted.id));
    assert_ne!(rebased.draft_digest, draft.draft_digest);
    assert!(
        f.project
            .save_card_kit_draft(draft.artifact_id, &draft.draft_digest, &f.composition)
            .is_err()
    );
    let mut edited = f.composition.clone();
    edited.cards[0].title = "New fox".into();
    edited.name = "Revised moon cards".into();
    let changed = f
        .project
        .save_card_kit_draft(draft.artifact_id, &rebased.draft_digest, &edited)
        .unwrap();
    let candidate = f
        .project
        .propose_card_kit(draft.artifact_id, &changed.draft_digest)
        .unwrap();
    f.project.accept_card_kit(candidate).unwrap();
    assert_eq!(
        f.project
            .snapshot()
            .unwrap()
            .artifacts
            .iter()
            .find(|a| a.id == draft.artifact_id)
            .unwrap()
            .name,
        "Revised moon cards"
    );
    assert_eq!(
        f.project.read_card_kit(first).unwrap().cards[0].title,
        "Fox <&>"
    );
    let exported = f
        .project
        .export_card_kit(first, &f.root.join("kit.zip"))
        .unwrap();
    assert_eq!(
        exported.output_blake3,
        ContentDigest::from_bytes(&fs::read(&exported.destination).unwrap()).to_string()
    );
    let mut archive = zip::ZipArchive::new(fs::File::open(&exported.destination).unwrap()).unwrap();
    let mut svg = String::new();
    archive
        .by_name("cards/001.svg")
        .unwrap()
        .read_to_string(&mut svg)
        .unwrap();
    assert!(svg.contains("Fox &lt;&amp;&gt;"));
    assert!(svg.contains("data:image/png;base64,"));
    assert!(!svg.contains("<script"));
    let mut manifest = String::new();
    archive
        .by_name("manifest.json")
        .unwrap()
        .read_to_string(&mut manifest)
        .unwrap();
    let manifest: Value = serde_json::from_str(&manifest).unwrap();
    assert_eq!(
        manifest["source"]["revision_id"],
        first.revision_id.to_string()
    );
    let material = manifest["materials"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["content"]["media_type"] == "image/png")
        .unwrap();
    let mut exact = Vec::new();
    archive
        .by_name(material["file"].as_str().unwrap())
        .unwrap()
        .read_to_end(&mut exact)
        .unwrap();
    assert_eq!(exact, f.png);
    assert!(
        f.project
            .export_card_kit(first, &exported.destination)
            .is_err()
    );
    assert!(
        f.project
            .export_card_kit(first, &f.root.join("cards.shape").join("internal.zip"))
            .is_err()
    );
}

#[test]
fn incomplete_or_overflowing_accepted_checklists_are_explicit_and_not_exported() {
    let mut f = Fixture::new();
    f.composition.cards[0].artwork = None;
    f.composition.cards[1].rules = "Long rules ".repeat(200);
    let draft = f.draft();
    let preview = f
        .project
        .preview_card_kit(&draft.composition, Some("fox"))
        .unwrap();
    assert_eq!(preview["ready_to_export"], false);
    assert_eq!(preview["missing_artwork_ids"], json!(["fox"]));
    assert_eq!(preview["overflow_card_ids"], json!(["owl"]));
    let candidate = f
        .project
        .propose_card_kit(draft.artifact_id, &draft.draft_digest)
        .unwrap();
    let accepted = f.project.accept_card_kit(candidate).unwrap();
    assert!(
        f.project
            .export_card_kit(
                WorkingInput {
                    artifact_id: accepted.artifact_id,
                    revision_id: accepted.id
                },
                &f.root.join("incomplete.zip")
            )
            .is_err()
    );
    assert!(!f.root.join("incomplete.zip").exists());
}

#[test]
fn review_digest_is_exact_and_discard_removes_only_unaccepted_reserved_output() {
    let mut f = Fixture::new();
    let draft = f.draft();
    let action = json!({"action":"accept","artifact_id":draft.artifact_id,"draft_digest":draft.draft_digest,"review_digest":"wrong"});
    assert!(f.project.card_kit_action_json(&action.to_string()).is_err());
    f.project
        .discard_card_kit_draft(draft.artifact_id, &draft.draft_digest)
        .unwrap();
    assert!(
        !f.project
            .snapshot()
            .unwrap()
            .artifacts
            .iter()
            .any(|a| a.id == draft.artifact_id)
    );
}

#[test]
fn exported_wide_ascii_and_bilingual_copy_wraps_and_overlong_copy_is_reported() {
    let mut f = Fixture::new();
    f.composition.cards[0].title = "W".repeat(25);
    f.composition.cards[0].text_material = None;
    f.composition.cards[0].rules = format!("{}\n守护月亮 Moon garden together.", "W".repeat(40));
    let preview = f
        .project
        .preview_card_kit(&f.composition, Some("fox"))
        .unwrap();
    assert_eq!(preview["ready_to_export"], true);
    let svg = preview["svg"].as_str().unwrap();
    assert!(!svg.contains(&format!(">{}<", "W".repeat(25))));
    assert!(!svg.contains("Moon garden togeth</text>"));
    let draft = f.draft();
    let candidate = f
        .project
        .propose_card_kit(draft.artifact_id, &draft.draft_digest)
        .unwrap();
    let accepted = f.project.accept_card_kit(candidate).unwrap();
    f.project
        .export_card_kit(
            WorkingInput {
                artifact_id: accepted.artifact_id,
                revision_id: accepted.id,
            },
            &f.root.join("wide-copy.zip"),
        )
        .unwrap();
    f.composition.cards[0].title = "W".repeat(60);
    let preview = f
        .project
        .preview_card_kit(&f.composition, Some("fox"))
        .unwrap();
    assert_eq!(preview["ready_to_export"], false);
    assert_eq!(preview["overflow_card_ids"], json!(["fox"]));
}

#[test]
fn exported_cjk_title_and_rules_fill_lines_without_splitting_ordinary_english_words() {
    let mut f = Fixture::new();
    f.composition.cards[0].title = "静心 · Stillness".into();
    f.composition.cards[0].text_material = None;
    f.composition.cards[0].rules = "恢复 1 点专注。本回合下一张行动牌的费用减少 1。".into();
    let preview = f
        .project
        .preview_card_kit(&f.composition, Some("fox"))
        .unwrap();
    assert_eq!(preview["ready_to_export"], true);
    let svg = preview["svg"].as_str().unwrap();
    assert!(svg.contains(">静心 · Stillness</text>"));
    assert!(svg.contains(">恢复 1 点专注。"));
    assert!(!svg.contains(">恢复 1</text>"));
    let draft = f.draft();
    let candidate = f
        .project
        .propose_card_kit(draft.artifact_id, &draft.draft_digest)
        .unwrap();
    let accepted = f.project.accept_card_kit(candidate).unwrap();
    let exported = f
        .project
        .export_card_kit(
            WorkingInput {
                artifact_id: accepted.artifact_id,
                revision_id: accepted.id,
            },
            &f.root.join("cjk-copy.zip"),
        )
        .unwrap();
    let mut archive = zip::ZipArchive::new(fs::File::open(exported.destination).unwrap()).unwrap();
    let mut actual = String::new();
    archive
        .by_name("cards/001.svg")
        .unwrap()
        .read_to_string(&mut actual)
        .unwrap();
    assert_eq!(actual, svg);

    f.composition.cards[0].rules = "Protect the moon garden with a friend. Draw one card.".into();
    let preview = f
        .project
        .preview_card_kit(&f.composition, Some("fox"))
        .unwrap();
    assert!(
        preview["svg"]
            .as_str()
            .unwrap()
            .contains(">Protect the moon</text>")
    );
    f.composition.cards[0].rules = format!("Cost 1 {}", "W".repeat(40));
    let preview = f
        .project
        .preview_card_kit(&f.composition, Some("fox"))
        .unwrap();
    let svg = preview["svg"].as_str().unwrap();
    assert!(svg.contains(">Cost 1 WWW"));
    assert!(!svg.contains(">Cost 1</text>"));
}

#[test]
fn accepted_square_landscape_and_portrait_artwork_have_centered_proportional_svg_rectangles() {
    let mut f = Fixture::new();
    for (width, height, expected) in [
        (4, 4, [160.0, 150.0, 430.0, 430.0]),
        (4, 2, [48.0, 201.5, 654.0, 327.0]),
        (2, 4, [267.5, 150.0, 215.0, 430.0]),
    ] {
        let mut png = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(
                &[32, 80, 120, 255].repeat(usize::try_from(width * height).unwrap()),
                width,
                height,
                ColorType::Rgba8.into(),
            )
            .unwrap();
        let accepted = f
            .project
            .import_raster_bytes(png, "Aspect fixture")
            .unwrap();
        f.composition.cards[0].artwork = Some(WorkingInput {
            artifact_id: accepted.artifact_id,
            revision_id: accepted.id,
        });
        let preview = f
            .project
            .preview_card_kit(&f.composition, Some("fox"))
            .unwrap();
        let svg = preview["svg"].as_str().unwrap();
        let image = svg.split_once("<image ").unwrap().1;
        let attribute = |name: &str| {
            image
                .split_once(&format!("{name}=\""))
                .unwrap()
                .1
                .split('"')
                .next()
                .unwrap()
                .parse::<f64>()
                .unwrap()
        };
        let rect = [
            attribute("x"),
            attribute("y"),
            attribute("width"),
            attribute("height"),
        ];
        for (actual, expected) in rect.iter().zip(expected) {
            assert!((actual - expected).abs() < 0.000_001);
        }
        assert!((rect[2] / rect[3] - f64::from(width) / f64::from(height)).abs() < 0.000_001);
        assert!((rect[0] + rect[2] / 2.0 - 375.0).abs() < 0.000_001);
        assert!((rect[1] + rect[3] / 2.0 - 365.0).abs() < 0.000_001);
        assert!(rect[0] >= 48.0 && rect[0] + rect[2] <= 702.0);
        assert!(rect[1] >= 150.0 && rect[1] + rect[3] <= 580.0);
    }
}

#[test]
fn layout_switches_recover_hidden_fields_and_accept_new_versions_without_rewriting_history() {
    let mut f = Fixture::new();
    let draft = f.draft();
    let old_candidate = f
        .project
        .propose_card_kit(draft.artifact_id, &draft.draft_digest)
        .unwrap();
    let old_revision = f.project.accept_card_kit(old_candidate).unwrap();
    let old_source = WorkingInput {
        artifact_id: old_revision.artifact_id,
        revision_id: old_revision.id,
    };
    let old_bytes = f
        .project
        .read_revision_content(old_revision.id)
        .unwrap()
        .bytes;
    let old_svg = f
        .project
        .preview_card_kit(&f.composition, Some("fox"))
        .unwrap()["svg"]
        .clone();
    let reopened = f.project.open_card_kit_draft(draft.artifact_id).unwrap();
    let mut edited = f.composition.clone();
    edited.layout = Some(serde_json::from_value(json!({"mode":"auto"})).unwrap());
    edited.canvas_width = 1200;
    edited.canvas_height = 800;
    edited.cards[0].layout = Some(serde_json::from_value(json!({"mode":"image"})).unwrap());
    let saved = f
        .project
        .save_card_kit_draft(draft.artifact_id, &reopened.draft_digest, &edited)
        .unwrap();
    let obsolete = f
        .project
        .propose_card_kit(saved.artifact_id, &saved.draft_digest)
        .unwrap();
    let mut other = ShapeProject::open(f.root.join("cards.shape")).unwrap();
    let recovered = other.open_card_kit_draft(saved.artifact_id).unwrap();
    assert_eq!(recovered.composition, edited);
    let preview = other
        .preview_card_kit(&recovered.composition, Some("fox"))
        .unwrap();
    assert_eq!(
        preview["cards"][0]["hidden_content"]["text_fields"],
        json!(["title", "card_type", "cost", "text_material", "stats"])
    );
    edited.cards[0].layout =
        Some(serde_json::from_value(json!({"mode":"text","text_scale_percent":50})).unwrap());
    let changed = other
        .save_card_kit_draft(saved.artifact_id, &saved.draft_digest, &edited)
        .unwrap();
    assert!(f.project.accept_card_kit(obsolete).is_err());
    let candidate = other
        .propose_card_kit(changed.artifact_id, &changed.draft_digest)
        .unwrap();
    let accepted = other.accept_card_kit(candidate).unwrap();
    let current = other
        .read_card_kit(WorkingInput {
            artifact_id: accepted.artifact_id,
            revision_id: accepted.id,
        })
        .unwrap();
    assert_eq!(current.cards[0].artwork, f.composition.cards[0].artwork);
    assert_eq!(
        current.cards[0].text_material,
        f.composition.cards[0].text_material
    );
    assert_eq!(current.cards[0].stats, "2 / 3");
    let old = other.read_card_kit(old_source).unwrap();
    assert!(old.layout.is_none());
    assert_eq!(
        other.read_revision_content(old_revision.id).unwrap().bytes,
        old_bytes
    );
    assert_eq!(
        other.preview_card_kit(&old, Some("fox")).unwrap()["svg"],
        old_svg
    );
}
