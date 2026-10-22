use super::*;
use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use serde_json::{Value, json};
use shape_domain::WorkingInput;
use std::{fs, path::PathBuf};

use crate::{CardPrintRequestOptions, ShapeProject};

struct Fixture {
    root: PathBuf,
    project: ShapeProject,
    image: WorkingInput,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("shape-card-face-{}", uuid::Uuid::now_v7()));
        let mut project = ShapeProject::create(root.join("faces.shape"), "General cards").unwrap();
        let mut bytes = Vec::new();
        PngEncoder::new(&mut bytes)
            .write_image(
                &[80, 120, 160, 255].repeat(6),
                3,
                2,
                ColorType::Rgba8.into(),
            )
            .unwrap();
        let art = project
            .import_raster_bytes(bytes, "Finished face.png")
            .unwrap();
        Self {
            root,
            project,
            image: WorkingInput {
                artifact_id: art.artifact_id,
                revision_id: art.id,
            },
        }
    }
    fn composition(card: Value) -> CardKitRequest {
        let mut value = json!({"name":"General cards","canvas_width":1200,"canvas_height":800,"layout":{"mode":"auto"}});
        value["cards"] = Value::Array(vec![card]);
        serde_json::from_value(value).unwrap()
    }
    fn preview(&self, card: Value) -> Value {
        self.project
            .preview_card_kit(&Self::composition(card), None)
            .unwrap()
    }
    fn stripe(&mut self, width: u32, height: u32) -> WorkingInput {
        let mut png = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(
                &[40, 160, 80, 255].repeat(usize::try_from(width * height).unwrap()),
                width,
                height,
                ColorType::Rgba8.into(),
            )
            .unwrap();
        let art = self
            .project
            .import_raster_bytes(png, "Accepted stripe")
            .unwrap();
        WorkingInput {
            artifact_id: art.artifact_id,
            revision_id: art.id,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn bound_block(id: &str, field: &str, x: u16, y: u16, width: u16, height: u16) -> Value {
    json!({"id":id,"field":field,"x":x,"y":y,"width":width,"height":height,
        "text_scale_percent":100,"text_align":"left","text_vertical_align":"top",
        "bold":false,"italic":false,"text_color":"#123456","image_fit":"contain"})
}
fn block_composition(card: Value, blocks: Value) -> CardKitRequest {
    let mut request = Fixture::composition(card);
    request.layout=Some(serde_json::from_value(json!({"mode":"blocks","blocks":blocks,"background_color":"#fefefe","padding_percent":20})).unwrap());
    request
}

#[test]
fn blocks_bind_only_selected_fields_report_hidden_content_and_resolve_whole_list() {
    let f = Fixture::new();
    let request = block_composition(
        json!({"id":"stable","title":"Visible","rules":"Hidden body","artwork":f.image}),
        json!([bound_block("title", "title", 500, 500, 9000, 2000)]),
    );
    let preview = f.project.preview_card_kit(&request, None).unwrap();
    assert_eq!(preview["ready_to_export"], true);
    assert_eq!(preview["cards"][0]["hidden_content"]["artwork"], true);
    assert_eq!(
        preview["cards"][0]["hidden_content"]["text_fields"],
        json!(["rules"])
    );
    let svg = preview["svg"].as_str().unwrap();
    assert!(svg.contains(">Visible</text>"));
    assert!(!svg.contains("Hidden body"));
    assert!(!svg.contains("data:image/png"));
    assert!(
        svg.contains("x=\"60.000\" y=\"40.000\" width=\"1080.000\" height=\"160.000\""),
        "block coordinates ignore global padding and use the complete canvas"
    );
    let empty = block_composition(json!({"id":"empty","title":"Not bound"}), json!([]));
    let preview = f.project.preview_card_kit(&empty, None).unwrap();
    assert_eq!(preview["ready_to_export"], false);
    assert_eq!(preview["cards"][0]["issues"], json!(["empty_content"]));
    assert_eq!(preview["cards"][0]["resolved_layout"]["blocks"], json!([]));
    let old = Fixture::composition(json!({"id":"old","title":"Old"}));
    assert!(
        f.project.preview_card_kit(&old, None).unwrap()["cards"][0]["resolved_layout"]
            .get("blocks")
            .is_none()
    );
    let mut switched = request.clone();
    switched.layout.as_mut().unwrap().mode = Some(CardLayoutMode::Text);
    assert_eq!(f.project.preview_card_kit(&switched,None).unwrap()["cards"][0]["resolved_layout"]["blocks"].as_array().unwrap().len(),1);
}

#[test]
fn overlapping_blocks_keep_layer_order_share_one_image_and_use_distinct_viewports() {
    let f = Fixture::new();
    let mut cover = bound_block("a", "artwork", 1000, 1000, 6000, 7000);
    cover["image_fit"] = json!("cover");
    let request = block_composition(
        json!({"id":"layers","title":"Top title","artwork":f.image}),
        json!([
            bound_block("art-source", "artwork", 0, 0, 10000, 10000),
            cover,
            bound_block("a-bounds", "title", 1000, 1000, 8000, 2500)
        ]),
    );
    let preview = f.project.preview_card_kit(&request, None).unwrap();
    assert_eq!(preview["ready_to_export"], true);
    let svg = preview["svg"].as_str().unwrap();
    assert_eq!(
        svg.matches("data:image/png;base64,").count(),
        1,
        "image payload is shared by repeated field bindings"
    );
    assert!(
        svg.find("data-block-id=\"art-source\"").unwrap()
            < svg.find("data-block-id=\"a\"").unwrap()
    );
    assert!(
        svg.find("data-block-id=\"a\"").unwrap() < svg.find("data-block-id=\"a-bounds\"").unwrap()
    );
    assert!(svg.contains("id=\"card-block-art-0\""));
    assert!(svg.contains("id=\"card-block-art-1-viewport\""));
    assert_eq!(
        svg.matches("xlink:href=\"#card-block-image-source\"")
            .count(),
        2
    );
    let ids = svg
        .split(" id=\"")
        .skip(1)
        .map(|tail| tail.split('"').next().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        ids.iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        ids.len(),
        "legal authored ids cannot collide with SVG internal ids"
    );
    let missing = block_composition(
        json!({"id":"missing","title":"Title"}),
        json!([bound_block("art", "artwork", 0, 0, 10000, 10000)]),
    );
    let preview = f.project.preview_card_kit(&missing, None).unwrap();
    assert!(
        preview["cards"][0]["issues"]
            .as_array()
            .unwrap()
            .contains(&json!("missing_artwork"))
    );
    assert!(
        preview["svg"]
            .as_str()
            .unwrap()
            .contains("card-block-placeholder-0")
    );
}

#[test]
fn block_text_reuses_bound_material_wrapping_styles_and_readable_overflow_limits() {
    let mut f = Fixture::new();
    let text = f
        .project
        .create_text_document("Bound rules", "Spring 春。\nSecond line")
        .unwrap();
    let pin = WorkingInput {
        artifact_id: text.artifact_id,
        revision_id: text.id,
    };
    let mut block = bound_block("body", "rules", 1000, 1000, 8000, 8000);
    block["bold"] = json!(true);
    block["italic"] = json!(true);
    block["text_align"] = json!("right");
    block["text_vertical_align"] = json!("bottom");
    let request = block_composition(json!({"id":"bound","text_material":pin}), json!([block]));
    let materials = f.project.card_kit_visible_materials(&request).unwrap();
    let rendered = render(&request, &request.cards[0], &materials, Some(63.0 / 1200.0)).unwrap();
    assert!(rendered.svg.contains("font-weight=\"bold\""));
    assert!(rendered.svg.contains("font-style=\"italic\""));
    assert!(rendered.svg.contains("text-anchor=\"end\""));
    assert!(rendered.svg.contains(">Spring 春。</text>"));
    assert!(rendered.svg.contains(">Second line</text>"));
    let mut tiny = bound_block("tiny", "rules", 1000, 1000, 1000, 100);
    tiny["text_scale_percent"] = json!(50);
    let overflow = block_composition(
        json!({"id":"overflow","rules":"Readable text cannot shrink"}),
        json!([tiny]),
    );
    let preview = f.project.preview_card_kit(&overflow, None).unwrap();
    assert_eq!(preview["ready_to_export"], false);
    assert_eq!(preview["cards"][0]["layout_overflow"], true);
    assert!(
        render(&overflow, &overflow.cards[0], &[], Some(63.0 / 1200.0))
            .err()
            .unwrap()
            .to_string()
            .starts_with("card_print.text_overflow:")
    );
}

#[test]
fn text_only_blocks_skip_missing_hidden_artwork_but_acceptance_and_zip_keep_its_pin() {
    let mut f = Fixture::new();
    let request = block_composition(
        json!({"id":"keep-id","title":"Visible","artwork":f.image}),
        json!([bound_block("text", "title", 1000, 1000, 8000, 3000)]),
    );
    let draft = f.project.create_card_kit_draft(&request).unwrap();
    let candidate = f
        .project
        .propose_card_kit(draft.artifact_id, &draft.draft_digest)
        .unwrap();
    let accepted = f.project.accept_card_kit(candidate).unwrap();
    let source = WorkingInput {
        artifact_id: accepted.artifact_id,
        revision_id: accepted.id,
    };
    let bytes = f.project.read_revision_content(accepted.id).unwrap().bytes;
    let digest = f
        .project
        .revision(f.image.revision_id)
        .unwrap()
        .content
        .digest
        .to_string();
    fs::remove_file(
        f.root
            .join("faces.shape/objects/b3")
            .join(&digest[..2])
            .join(&digest[2..]),
    )
    .unwrap();
    assert!(
        f.project
            .read_revision_content(f.image.revision_id)
            .is_err()
    );
    assert_eq!(
        f.project.preview_card_kit(&request, None).unwrap()["ready_to_export"],
        true
    );
    let print = f
        .project
        .plan_card_print_request(source, &CardPrintRequestOptions::default())
        .unwrap();
    assert_eq!(print.pages[0].slots[0].card_id, "keep-id");
    assert!(!print.cards[0].svg.contains("data:image/png"));
    assert_eq!(
        f.project.read_card_kit(source).unwrap().cards[0].artwork,
        Some(f.image)
    );
    assert_eq!(
        f.project.read_revision_content(accepted.id).unwrap().bytes,
        bytes
    );
    let current = f.project.open_card_kit_draft(draft.artifact_id).unwrap();
    assert!(
        f.project
            .propose_card_kit(draft.artifact_id, &current.draft_digest)
            .is_err()
    );
    assert!(
        f.project
            .export_card_kit(source, &f.root.join("all-pins.zip"))
            .is_err()
    );
}

#[test]
fn block_drafts_persist_cover_limits_are_explicit_and_duplex_geometry_stays_shared() {
    let mut f = Fixture::new();
    let mut request = block_composition(
        json!({"id":"same-id","title":"Front","quantity":10}),
        json!([bound_block("title", "title", 1000, 1000, 8000, 3000)]),
    );
    request.back = Some(serde_json::from_value(json!({"mode":"solid","color":"#345678"})).unwrap());
    let draft = f.project.create_card_kit_draft(&request).unwrap();
    let candidate = f
        .project
        .propose_card_kit(draft.artifact_id, &draft.draft_digest)
        .unwrap();
    let accepted = f.project.accept_card_kit(candidate).unwrap();
    let source = WorkingInput {
        artifact_id: accepted.artifact_id,
        revision_id: accepted.id,
    };
    let mut dirty = request.clone();
    dirty.layout.as_mut().unwrap().blocks.as_mut().unwrap()[0].x = 1500;
    let current = f.project.open_card_kit_draft(draft.artifact_id).unwrap();
    f.project
        .save_card_kit_draft(draft.artifact_id, &current.draft_digest, &dirty)
        .unwrap();
    let mut other = ShapeProject::open(f.root.join("faces.shape")).unwrap();
    assert_eq!(
        other
            .open_card_kit_draft(draft.artifact_id)
            .unwrap()
            .composition,
        dirty
    );
    assert_eq!(other.read_card_kit(source).unwrap(), request);
    let front = other
        .plan_card_print_request(source, &CardPrintRequestOptions::default())
        .unwrap();
    let duplex = other
        .plan_card_print_job_request(
            source,
            &CardPrintRequestOptions::default(),
            &[("same-id".into(), 10)],
            None,
            &crate::CardPrintSidesOptions {
                sides: crate::CardPrintSides::Duplex,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(duplex.total_copies, 10);
    assert_eq!(duplex.total_placements, Some(20));
    assert_eq!(
        duplex.pages[1].slots[0].copy_index,
        front.pages[0].slots[0].copy_index
    );
    assert!(
        (duplex.pages[1].slots[0].rect_mm.x
            - (210.0 - front.pages[0].slots[0].rect_mm.x - front.pages[0].slots[0].rect_mm.width))
            .abs()
            < 1e-6
    );
    let stripe = f.stripe(16_384, 1);
    let mut cover = bound_block("art", "artwork", 0, 0, 10000, 10000);
    cover["image_fit"] = json!("cover");
    let extreme = block_composition(json!({"id":"extreme","artwork":stripe}), json!([cover]));
    let preview = f.project.preview_card_kit(&extreme, None).unwrap();
    assert_eq!(preview["ready_to_export"], false);
    assert!(
        preview["cards"][0]["issues"]
            .as_array()
            .unwrap()
            .contains(&json!("image_crop_limit"))
    );
}

#[test]
fn learning_card_spring_stays_on_one_line_at_double_size_and_excess_copy_is_explicit() {
    let f = Fixture::new();
    let mut composition = Fixture::composition(
        json!({"id":"spring","rules":"SPRING\n春","layout":{"mode":"text","padding_percent":4,"text_scale_percent":200}}),
    );
    composition.canvas_width = 750;
    composition.canvas_height = 1050;
    let materials = f.project.card_kit_materials(&composition, false).unwrap();
    for physical_scale in [None, Some(63.0 / 750.0)] {
        let rendered = render(
            &composition,
            &composition.cards[0],
            &materials,
            physical_scale,
        )
        .unwrap();
        assert!(!rendered.overflow);
        assert!(rendered.svg.contains("font-size=\"105.000\""));
        assert!(rendered.svg.contains(">SPRING</text>"));
        assert!(rendered.svg.contains(">春</text>"));
        assert!(!rendered.svg.contains(">SPRIN</text>"));
    }
    let long = format!("{}。", "W@".repeat(1000));
    composition.cards[0].rules = long.clone();
    let preview = f.project.preview_card_kit(&composition, None).unwrap();
    assert_eq!(preview["ready_to_export"], false);
    assert_eq!(preview["cards"][0]["issues"], json!(["layout_overflow"]));
    assert!(preview["svg"].as_str().unwrap().contains(&long));
    assert!(
        render(
            &composition,
            &composition.cards[0],
            &materials,
            Some(63.0 / 750.0)
        )
        .is_err()
    );
}

#[test]
fn finished_face_is_edge_to_edge_without_a_game_frame_filename_or_text_and_prints_at_its_ratio() {
    let mut f = Fixture::new();
    let composition = Fixture::composition(json!({"id":"face","artwork":f.image,"quantity":3}));
    let preview = f.project.preview_card_kit(&composition, None).unwrap();
    assert_eq!(preview["ready_to_export"], true);
    assert_eq!(preview["cards"][0]["resolved_layout"]["mode"], "image");
    assert_eq!(preview["cards"][0]["resolved_layout"]["padding_percent"], 0);
    let svg = preview["svg"].as_str().unwrap();
    assert!(svg.contains("viewBox=\"0 0 1200 800\""));
    assert!(svg.contains("<image x=\"0.000\" y=\"0.000\" width=\"1200.000\" height=\"800.000\""));
    assert!(!svg.contains("<text "));
    assert!(!svg.contains("Finished face"));
    assert!(!svg.contains("stroke="));
    let draft = f.project.create_card_kit_draft(&composition).unwrap();
    let candidate = f
        .project
        .propose_card_kit(draft.artifact_id, &draft.draft_digest)
        .unwrap();
    let accepted = f.project.accept_card_kit(candidate).unwrap();
    let source = WorkingInput {
        artifact_id: accepted.artifact_id,
        revision_id: accepted.id,
    };
    let plan = f
        .project
        .plan_card_print_request(source, &CardPrintRequestOptions::default())
        .unwrap();
    assert_eq!(
        (plan.options.card_width_mm, plan.options.card_height_mm),
        (63.0, 42.0)
    );
    assert_eq!(plan.cards[0].svg, svg);
    let slot = &plan.pages[0].slots[0];
    assert!((slot.content_rect_mm.width / slot.content_rect_mm.height - 1.5).abs() < 0.000_001);
    assert_eq!(plan.total_copies, 3);
    let explicit: CardPrintRequestOptions =
        serde_json::from_value(json!({"card_width_mm":70,"card_height_mm":70})).unwrap();
    let plan = f
        .project
        .plan_card_print_request(source, &explicit)
        .unwrap();
    let slot = &plan.pages[0].slots[0];
    assert!((slot.content_rect_mm.width - 70.0).abs() < 0.000_001);
    assert!((slot.content_rect_mm.height - 70.0 / 1.5).abs() < 0.000_001);
}

#[test]
fn auto_uses_every_actual_text_field_and_prompts_or_whitespace_never_become_face_content() {
    let f = Fixture::new();
    for field in ["title", "rules", "card_type", "cost", "stats", "flavor"] {
        let mut card = json!({"id":"text"});
        card[field] = json!("月 Moon");
        let preview = f.preview(card);
        assert_eq!(
            preview["cards"][0]["resolved_layout"]["mode"], "text",
            "{field}"
        );
        assert_eq!(preview["ready_to_export"], true, "{field}");
        assert!(preview["svg"].as_str().unwrap().contains(">月 Moon</text>"));
    }
    let preview = f
        .preview(json!({"id":"empty","title":"  ","rules":"\n\t ","art_prompt":"Generate a face"}));
    assert_eq!(preview["cards"][0]["resolved_layout"]["mode"], "auto");
    assert_eq!(preview["cards"][0]["issues"], json!(["empty_content"]));
    assert_eq!(preview["ready_to_export"], false);
    let preview = f.preview(json!({"id":"mixed","artwork":f.image,"stats":"Care for one plant."}));
    assert_eq!(preview["cards"][0]["resolved_layout"]["mode"], "image_text");
    assert_eq!(preview["ready_to_export"], true);
}

#[test]
fn partial_card_override_keeps_content_and_root_controls_ignore_the_override() {
    let f = Fixture::new();
    let mut composition = Fixture::composition(
        json!({"id":"face","artwork":f.image,"title":"Keep me","rules":"And me","layout":{"mode":"image","padding_percent":0}}),
    );
    composition.layout=Some(serde_json::from_value(json!({"mode":"auto","padding_percent":8,"background_color":"#AABBCC","text_scale_percent":125})).unwrap());
    let preview = f.project.preview_card_kit(&composition, None).unwrap();
    assert_eq!(preview["default_layout"]["mode"], "image_text");
    assert_eq!(preview["default_layout"]["padding_percent"], 8);
    assert_eq!(preview["cards"][0]["resolved_layout"]["mode"], "image");
    assert_eq!(preview["cards"][0]["resolved_layout"]["padding_percent"], 0);
    assert_eq!(
        preview["cards"][0]["resolved_layout"]["background_color"],
        "#AABBCC"
    );
    assert_eq!(
        preview["cards"][0]["resolved_layout"]["text_scale_percent"],
        125
    );
    assert_eq!(
        preview["cards"][0]["hidden_content"]["text_fields"],
        json!(["title", "rules"])
    );
    assert_eq!(composition.cards[0].title, "Keep me");
    composition.cards[0].layout = Some(serde_json::from_value(json!({"mode":"text"})).unwrap());
    let preview = f.project.preview_card_kit(&composition, None).unwrap();
    assert_eq!(preview["cards"][0]["hidden_content"]["artwork"], true);
    assert!(!preview["svg"].as_str().unwrap().contains("data:image/png"));
}

#[test]
fn contain_and_explicit_cover_use_proportional_rectangles_inside_the_same_canvas_clip() {
    let f = Fixture::new();
    let mut composition = Fixture::composition(json!({"id":"face","artwork":f.image}));
    composition.canvas_width = 800;
    composition.canvas_height = 800;
    let preview = f.project.preview_card_kit(&composition, None).unwrap();
    let svg = preview["svg"].as_str().unwrap();
    assert!(svg.contains("<image x=\"0.000\" y=\"133.333\" width=\"800.000\" height=\"533.333\""));
    assert!(!svg.contains("<mask "));
    composition.cards[0].layout =
        Some(serde_json::from_value(json!({"image_fit":"cover"})).unwrap());
    let preview = f.project.preview_card_kit(&composition, None).unwrap();
    let svg = preview["svg"].as_str().unwrap();
    assert!(
        svg.contains("<image x=\"-200.000\" y=\"0.000\" width=\"1200.000\" height=\"800.000\"")
    );
    assert!(svg.contains("<symbol id=\"art-viewport\" x=\"0\" y=\"0\" width=\"800.000\" height=\"800.000\" viewBox=\"0 0 800.000 800.000\" overflow=\"hidden\""));
    assert!(svg.contains("<use x=\"0.000\" y=\"0.000\" width=\"800.000\" height=\"800.000\" xlink:href=\"#art-viewport\""));
    assert!(!svg.contains("<mask "));
}

#[test]
fn padded_image_text_cover_clips_only_a_single_image_viewport_in_material_and_print_svgs() {
    let f = Fixture::new();
    let composition = Fixture::composition(
        json!({"id":"face","artwork":f.image,"rules":"Moon","layout":{"mode":"image_text","image_fit":"cover","padding_percent":10}}),
    );
    let materials = f.project.card_kit_materials(&composition, false).unwrap();
    let card = &composition.cards[0];
    for physical_scale in [None, Some(63.0 / 1200.0)] {
        let svg = render(&composition, card, &materials, physical_scale)
            .unwrap()
            .svg;
        // 1200x800, 80-unit padding, 28-unit gap: left artwork is exactly 506x640.
        let symbol = svg
            .split_once("<symbol ")
            .unwrap()
            .1
            .split_once("</symbol>")
            .unwrap()
            .0;
        assert!(symbol.contains("width=\"506.000\" height=\"640.000\" viewBox=\"0 0 506.000 640.000\" overflow=\"hidden\""));
        assert_eq!(symbol.matches("<image ").count(), 1);
        assert_eq!(symbol.matches("/>").count(), 1);
        assert!(
            symbol
                .contains("<image x=\"-227.000\" y=\"0.000\" width=\"960.000\" height=\"640.000\"")
        );
        assert!(!symbol.contains("<text "));
        assert!(svg.contains("<use x=\"80.000\" y=\"80.000\" width=\"506.000\" height=\"640.000\" xlink:href=\"#art-viewport\"/>"));
        assert!(!svg.contains("<mask "));
        assert!(!svg.contains(" mask="));
        assert!(svg.contains(">Moon</text>"));
    }
}

#[test]
fn wide_accepted_source_uses_one_clipped_viewport_without_mask_buffers_or_rewriting_its_bytes() {
    let mut f = Fixture::new();
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(
            &[40, 160, 80, 255].repeat(4000 * 80),
            4000,
            80,
            ColorType::Rgba8.into(),
        )
        .unwrap();
    let art = f
        .project
        .import_raster_bytes(png, "Wide accepted face")
        .unwrap();
    let source = WorkingInput {
        artifact_id: art.artifact_id,
        revision_id: art.id,
    };
    let original = f.project.read_revision_content(art.id).unwrap().bytes;
    let mut composition = Fixture::composition(
        json!({"id":"wide","artwork":source,"layout":{"mode":"image","image_fit":"cover"}}),
    );
    composition.canvas_width = 750;
    composition.canvas_height = 1050;
    let materials = f.project.card_kit_materials(&composition, false).unwrap();
    for physical_scale in [None, Some(190.0 / 750.0)] {
        let svg = render(
            &composition,
            &composition.cards[0],
            &materials,
            physical_scale,
        )
        .unwrap()
        .svg;
        assert!(svg.contains("<symbol id=\"art-viewport\" x=\"0\" y=\"0\" width=\"750.000\" height=\"1050.000\" viewBox=\"0 0 750.000 1050.000\" overflow=\"hidden\""));
        assert!(svg.contains(
            "<image x=\"-25875.000\" y=\"0.000\" width=\"52500.000\" height=\"1050.000\""
        ));
        assert_eq!(svg.matches("<image ").count(), 1);
        assert_eq!(svg.matches("<use ").count(), 1);
        assert!(!svg.contains("<mask "));
        assert!(!svg.contains("<filter "));
        assert!(svg.contains(&STANDARD.encode(&original)));
    }
    assert_eq!(
        f.project.read_revision_content(art.id).unwrap().bytes,
        original
    );
    assert_eq!(composition.cards[0].artwork, Some(source));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "One persisted lifecycle proves unsupported cover remains recoverable accepted content and only publication is blocked"
)]
fn extreme_cover_is_an_explicit_recoverable_issue_and_never_a_successful_blank_export() {
    let mut f = Fixture::new();
    let image = f.stripe(16_384, 1);
    let mut composition = Fixture::composition(
        json!({"id":"extreme","artwork":image,"layout":{"mode":"image","image_fit":"cover"}}),
    );
    composition.canvas_width = 750;
    composition.canvas_height = 1050;
    composition.validate().unwrap();
    let original = f
        .project
        .read_revision_content(image.revision_id)
        .unwrap()
        .bytes;
    let draft = f.project.create_card_kit_draft(&composition).unwrap();
    let preview = f.project.preview_card_kit(&composition, None).unwrap();
    assert_eq!(preview["cards"][0]["issues"], json!(["image_crop_limit"]));
    assert_eq!(preview["cards"][0]["content_ready"], false);
    assert_eq!(preview["ready_to_export"], false);
    assert_eq!(preview["cards"][0]["artwork"], json!(image));
    assert!(!preview["svg"].as_str().unwrap().contains("<image "));
    assert!(!preview["svg"].as_str().unwrap().contains("data:image/png"));
    let candidate = f
        .project
        .propose_card_kit(draft.artifact_id, &draft.draft_digest)
        .unwrap();
    let accepted = f.project.accept_card_kit(candidate).unwrap();
    let source = WorkingInput {
        artifact_id: accepted.artifact_id,
        revision_id: accepted.id,
    };
    let saved_bytes = f.project.read_revision_content(accepted.id).unwrap().bytes;
    let destination = f.root.join("unsupported.zip");
    assert!(
        f.project
            .export_card_kit(source, &destination)
            .unwrap_err()
            .to_string()
            .starts_with("card_render.cover_limit: card extreme")
    );
    assert!(!destination.exists());
    assert!(
        f.project
            .plan_card_print_request(source, &CardPrintRequestOptions::default())
            .unwrap_err()
            .to_string()
            .starts_with("card_render.cover_limit: card extreme")
    );
    let draft = f.project.open_card_kit_draft(draft.artifact_id).unwrap();
    composition.cards[0].layout =
        Some(serde_json::from_value(json!({"mode":"image","image_fit":"contain"})).unwrap());
    let saved = f
        .project
        .save_card_kit_draft(draft.artifact_id, &draft.draft_digest, &composition)
        .unwrap();
    let mut other = ShapeProject::open(f.root.join("faces.shape")).unwrap();
    assert_eq!(
        other
            .open_card_kit_draft(draft.artifact_id)
            .unwrap()
            .composition,
        composition
    );
    assert_eq!(
        other.preview_card_kit(&composition, None).unwrap()["ready_to_export"],
        true
    );
    let candidate = other
        .propose_card_kit(saved.artifact_id, &saved.draft_digest)
        .unwrap();
    let accepted = other.accept_card_kit(candidate).unwrap();
    let plan = other
        .plan_card_print_request(
            WorkingInput {
                artifact_id: accepted.artifact_id,
                revision_id: accepted.id,
            },
            &CardPrintRequestOptions::default(),
        )
        .unwrap();
    assert!(plan.cards[0].svg.contains("data:image/png"));
    assert_eq!(
        other
            .read_revision_content(image.revision_id)
            .unwrap()
            .bytes,
        original
    );
    assert_eq!(
        other
            .read_revision_content(source.revision_id)
            .unwrap()
            .bytes,
        saved_bytes
    );
    assert_eq!(
        other.read_card_kit(source).unwrap().cards[0]
            .layout
            .as_ref()
            .unwrap()
            .image_fit,
        Some(CardImageFit::Cover)
    );
}

#[test]
fn cover_bound_allows_normal_wide_sources_and_does_not_limit_hidden_artwork_or_contain() {
    let mut f = Fixture::new();
    let wide = f.stripe(4000, 80);
    let mut composition = Fixture::composition(
        json!({"id":"wide","artwork":wide,"layout":{"mode":"image","image_fit":"cover"}}),
    );
    composition.canvas_width = 240;
    composition.canvas_height = 2400;
    let preview = f.project.preview_card_kit(&composition, None).unwrap();
    assert_eq!(preview["ready_to_export"], true);
    assert!(
        preview["svg"]
            .as_str()
            .unwrap()
            .contains("width=\"120000.000\" height=\"2400.000\"")
    );
    let extreme = f.stripe(16_384, 1);
    composition.cards[0].artwork = Some(extreme);
    composition.cards[0].title = "Moon".into();
    composition.cards[0].layout =
        Some(serde_json::from_value(json!({"mode":"text","image_fit":"cover"})).unwrap());
    let preview = f.project.preview_card_kit(&composition, None).unwrap();
    assert_eq!(preview["cards"][0]["issues"], json!([]));
    assert_eq!(preview["cards"][0]["hidden_content"]["artwork"], true);
    assert!(!preview["svg"].as_str().unwrap().contains("data:image/png"));
    let draft = f.project.create_card_kit_draft(&composition).unwrap();
    let candidate = f
        .project
        .propose_card_kit(draft.artifact_id, &draft.draft_digest)
        .unwrap();
    let accepted = f.project.accept_card_kit(candidate).unwrap();
    assert!(
        f.project
            .plan_card_print_request(
                WorkingInput {
                    artifact_id: accepted.artifact_id,
                    revision_id: accepted.id
                },
                &CardPrintRequestOptions::default()
            )
            .is_ok()
    );
    assert_eq!(
        f.project
            .read_card_kit(WorkingInput {
                artifact_id: accepted.artifact_id,
                revision_id: accepted.id
            })
            .unwrap()
            .cards[0]
            .artwork,
        Some(extreme)
    );
}

#[test]
fn new_classic_is_optional_content_and_text_scale_uses_full_card_space_with_honest_overflow() {
    let f = Fixture::new();
    for card in [
        json!({"id":"one","title":"月","layout":{"mode":"classic"}}),
        json!({"id":"one","artwork":f.image,"layout":{"mode":"classic"}}),
    ] {
        assert_eq!(f.preview(card)["ready_to_export"], true);
    }
    let normal = f.preview(json!({"id":"one","rules":"月 Moon"}));
    let large =
        f.preview(json!({"id":"one","rules":"月 Moon","layout":{"text_scale_percent":200}}));
    assert_eq!(normal["ready_to_export"], true);
    assert_eq!(large["ready_to_export"], true);
    assert!(
        normal["svg"]
            .as_str()
            .unwrap()
            .contains("font-size=\"56.000\"")
    );
    assert!(
        large["svg"]
            .as_str()
            .unwrap()
            .contains("font-size=\"112.000\"")
    );
    let long = f.preview(
        json!({"id":"one","rules":"Long copy ".repeat(800),"layout":{"text_scale_percent":300}}),
    );
    assert_eq!(long["ready_to_export"], false);
    assert_eq!(long["cards"][0]["issues"], json!(["layout_overflow"]));
}

#[test]
fn missing_image_has_a_reserved_preview_placeholder_but_cannot_print() {
    let mut f = Fixture::new();
    let request = Fixture::composition(
        json!({"id":"waiting","title":"Moon","layout":{"mode":"image_text","image_position":"top"}}),
    );
    let preview = f.project.preview_card_kit(&request, None).unwrap();
    let preview = serde_json::to_value(preview).unwrap();
    assert!(preview.to_string().contains("image-placeholder"));
    assert!(
        preview["cards"][0]["issues"]
            .as_array()
            .unwrap()
            .contains(&json!("missing_artwork"))
    );
    let draft = f.project.create_card_kit_draft(&request).unwrap();
    let candidate = f
        .project
        .propose_card_kit(draft.artifact_id, &draft.draft_digest)
        .unwrap();
    let accepted = f.project.accept_card_kit(candidate).unwrap();
    let source = WorkingInput {
        artifact_id: accepted.artifact_id,
        revision_id: accepted.id,
    };
    assert!(
        f.project
            .plan_card_print_request(source, &CardPrintRequestOptions::default())
            .is_err()
    );
}

#[test]
fn image_positions_create_different_regions_and_text_offsets_move_the_anchor() {
    for position in ["top", "bottom", "left", "right", "background"] {
        let request = Fixture::composition(
            json!({"id":"a","title":"A","rules":"B","layout":{"mode":"image_text","image_position":position,"image_share_percent":40}}),
        );
        let p = plan(&request, &request.cards[0], &[], None);
        let image = p.image.unwrap();
        match position {
            "top" => assert!(image.y + image.height < p.text.y),
            "bottom" => assert!(p.text.y + p.text.height < image.y),
            "left" => assert!(image.x + image.width < p.text.x),
            "right" => assert!(p.text.x + p.text.width < image.x),
            _ => assert!(image.y < p.text.y && image.height > p.text.height),
        }
    }
    let mut request = Fixture::composition(
        json!({"id":"a","title":"A","layout":{"mode":"text","text_align":"center","text_vertical_align":"center"}}),
    );
    let before = plan(&request, &request.cards[0], &[], None);
    let old = text_origin(&before);
    request.cards[0]
        .layout
        .as_mut()
        .unwrap()
        .text_offset_x_percent = Some(5);
    request.cards[0]
        .layout
        .as_mut()
        .unwrap()
        .text_offset_y_percent = Some(-4);
    let after = plan(&request, &request.cards[0], &[], None);
    let new = text_origin(&after);
    assert!((new.0 - old.0 - 60.0).abs() < 0.001 && (new.1 - old.1 + 32.0).abs() < 0.001);
    assert!(!after.overflow);
    request.cards[0]
        .layout
        .as_mut()
        .unwrap()
        .text_vertical_align = Some(CardTextVerticalAlign::Bottom);
    request.cards[0]
        .layout
        .as_mut()
        .unwrap()
        .text_offset_y_percent = Some(30);
    assert!(plan(&request, &request.cards[0], &[], Some(0.1)).overflow);
}

#[test]
fn custom_composition_reopens_and_prints_the_same_accepted_geometry() {
    let mut f = Fixture::new();
    let request = Fixture::composition(
        json!({"id":"a","title":"Moon","rules":"Observe.","artwork":f.image,"layout":{"mode":"image_text","image_position":"left","image_share_percent":40,"text_vertical_align":"center","text_offset_x_percent":1,"text_offset_y_percent":-1}}),
    );
    let draft = f.project.create_card_kit_draft(&request).unwrap();
    let candidate = f
        .project
        .propose_card_kit(draft.artifact_id, &draft.draft_digest)
        .unwrap();
    let accepted = f.project.accept_card_kit(candidate).unwrap();
    let source = WorkingInput {
        artifact_id: accepted.artifact_id,
        revision_id: accepted.id,
    };
    let reopened = ShapeProject::open(f.root.join("faces.shape")).unwrap();
    assert_eq!(reopened.read_card_kit(source).unwrap(), request);
    let printed = reopened
        .plan_card_print_request(source, &CardPrintRequestOptions::default())
        .unwrap();
    assert!(!printed.cards[0].svg.contains("image-placeholder"));
    assert!(printed.cards[0].svg.contains("data:image/png;base64,"));
    assert_eq!(printed.source, source);
}

#[test]
fn empty_image_text_preserves_both_regions_and_horizontal_overflow_does_not_move_text_up() {
    let request = Fixture::composition(
        json!({"id":"empty","layout":{"mode":"image_text","image_position":"top"}}),
    );
    let p = plan(&request, &request.cards[0], &[], None);
    assert!(p.image.unwrap().height < 500.0 && p.text.height > 300.0);
    let mut request = Fixture::composition(
        json!({"id":"word","title":"ABCDEFGHIJ","layout":{"mode":"text","text_align":"center"}}),
    );
    let before = plan(&request, &request.cards[0], &[], None);
    request.cards[0]
        .layout
        .as_mut()
        .unwrap()
        .text_offset_x_percent = Some(30);
    let after = plan(&request, &request.cards[0], &[], None);
    assert!(after.overflow);
    assert!((text_origin(&before).1 - text_origin(&after).1).abs() < 0.001);
}
