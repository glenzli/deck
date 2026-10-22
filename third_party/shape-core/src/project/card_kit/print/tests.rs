use super::*;
use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use serde_json::json;
use shape_domain::CardKitRequest;
use std::{fs, path::PathBuf};

struct Fixture {
    root: PathBuf,
    project: ShapeProject,
    source: WorkingInput,
}

impl Fixture {
    fn new(quantities: &[u16]) -> Self {
        let root = std::env::temp_dir().join(format!("shape-card-print-{}", uuid::Uuid::now_v7()));
        let mut project = ShapeProject::create(root.join("print.shape"), "Print fixture").unwrap();
        let mut png = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(&[20, 40, 80, 255], 1, 1, ColorType::Rgba8.into())
            .unwrap();
        let image = project
            .import_raster_bytes(png, "Accepted print art")
            .unwrap();
        let composition: CardKitRequest = serde_json::from_value(json!({"name":"Print kit","cards":quantities.iter().enumerate().map(|(i,q)|json!({"id":format!("card-{i}"),"title":format!("Card {i}"),"rules":"Draw one card.","quantity":q,"artwork":{"artifact_id":image.artifact_id,"revision_id":image.id}})).collect::<Vec<_>>()})).unwrap();
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
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn a4_nine_up_reuses_each_svg_and_keeps_stable_copy_order_on_a_partial_last_page() {
    let mut f = Fixture::new(&[8, 3]);
    let before = f.project.open_card_kit_draft(f.source.artifact_id).unwrap();
    let plan = f
        .project
        .plan_card_print(f.source, &CardPrintOptions::default())
        .unwrap();
    assert_eq!((plan.columns, plan.rows, plan.slots_per_page), (3, 3, 9));
    assert_eq!(
        (plan.total_copies, plan.page_count, plan.cards.len()),
        (11, 2, 2)
    );
    assert_eq!(
        plan.pages.iter().map(|p| p.slots.len()).collect::<Vec<_>>(),
        [9, 2]
    );
    assert_eq!(plan.pages[0].slots[7].card_id, "card-0");
    assert_eq!(plan.pages[0].slots[7].copy_index, 8);
    assert_eq!(plan.pages[0].slots[8].card_id, "card-1");
    assert_eq!(plan.pages[0].slots[8].copy_index, 1);
    assert_eq!(plan.pages[1].slots[0].copy_index, 2);
    assert_eq!(plan.pages[1].index, 1);
    assert_eq!(plan.pages[1].crop_marks.len(), 16);
    assert_eq!(plan.source, f.source);
    assert!(
        plan.cards
            .iter()
            .all(|c| c.svg.matches("data:image/png;base64,").count() == 1)
    );
    let after = f.project.open_card_kit_draft(f.source.artifact_id).unwrap();
    assert_eq!(before.draft_digest, after.draft_digest);
    assert_eq!(before.expected_head, after.expected_head);
    assert_eq!(plan.line_cap, "butt");
    assert_eq!(plan.units, "mm");
    assert!(!plan.commercial_prepress && !plan.duplex);
    assert_safe_geometry(&plan);
}

#[test]
fn letter_and_custom_sizes_have_one_geometry_and_preserve_card_content_proportions() {
    let f = Fixture::new(&[7]);
    let options = CardPrintOptions {
        paper: CardPrintPaper::Letter,
        ..CardPrintOptions::default()
    };
    let plan = f.project.plan_card_print(f.source, &options).unwrap();
    assert_eq!((plan.columns, plan.rows, plan.page_count), (3, 2, 2));
    assert!((plan.sheet_width_mm - 215.9).abs() < 0.000_001);
    assert!((plan.sheet_height_mm - 279.4).abs() < 0.000_001);
    assert_eq!(plan.pages[1].slots.len(), 1);
    assert_safe_geometry(&plan);
    let options = CardPrintOptions {
        card_width_mm: 75.0,
        card_height_mm: 75.0,
        ..options
    };
    let plan = f.project.plan_card_print(f.source, &options).unwrap();
    let slot = &plan.pages[0].slots[0];
    assert!((slot.rect_mm.width - 75.0).abs() < 0.000_001);
    assert!(
        (slot.content_rect_mm.width / slot.content_rect_mm.height - 750.0 / 1050.0).abs()
            < 0.000_001
    );
    assert_safe_geometry(&plan);
}

fn assert_safe_geometry(plan: &CardPrintPlan) {
    let half_stroke = plan.options.stroke_width_mm / 2.0;
    for page in &plan.pages {
        for slot in &page.slots {
            let (outer, inner) = (slot.rect_mm, slot.content_rect_mm);
            assert!(inner.x >= outer.x - 0.000_001 && inner.y >= outer.y - 0.000_001);
            assert!(inner.x + inner.width <= outer.x + outer.width + 0.000_001);
            assert!(inner.y + inner.height <= outer.y + outer.height + 0.000_001);
        }
        for line in &page.crop_marks {
            let left = line.x1.min(line.x2) - half_stroke;
            let right = line.x1.max(line.x2) + half_stroke;
            let top = line.y1.min(line.y2) - half_stroke;
            let bottom = line.y1.max(line.y2) + half_stroke;
            assert!(left >= plan.options.margin_mm - 0.000_001);
            assert!(right <= plan.sheet_width_mm - plan.options.margin_mm + 0.000_001);
            assert!(top >= plan.options.margin_mm - 0.000_001);
            assert!(bottom <= plan.sheet_height_mm - plan.options.margin_mm + 0.000_001);
            for slot in &page.slots {
                let rect = slot.rect_mm;
                assert!(
                    right <= rect.x + 0.000_001
                        || left >= rect.x + rect.width - 0.000_001
                        || bottom <= rect.y + 0.000_001
                        || top >= rect.y + rect.height - 0.000_001,
                    "crop mark crosses artwork: {line:?} / {rect:?}"
                );
            }
        }
    }
}

#[test]
fn shared_cut_grid_deduplicates_full_sheets_and_follows_partial_page_boundaries() {
    for (gap, stroke) in [(0.2, 0.2), (0.201, 0.199)] {
        for margin in [0.0, 5.0, 12.0] {
            let options = CardPrintOptions {
                cut_line_style: CardPrintCutLineStyle::DashedGrid,
                card_width_mm: 40.0,
                card_height_mm: 50.0,
                gap_mm: gap,
                stroke_width_mm: stroke,
                margin_mm: margin,
                crop_mark_offset_mm: 0.0,
                crop_mark_length_mm: 10.0,
                ..Default::default()
            };
            for (count, expected_lines) in [(1, vec![4]), (9, vec![8]), (13, vec![8, 7])] {
                let layout = Layout::with_grid(&options, count, Some((3, 3))).unwrap();
                let copies: Vec<_> = (1..=count)
                    .map(|i| ("card".into(), u16::try_from(i).unwrap()))
                    .collect();
                let pages = layout.pages(&copies, (750.0, 1050.0));
                assert_eq!(
                    pages
                        .iter()
                        .map(|page| page.crop_marks.len())
                        .collect::<Vec<_>>(),
                    expected_lines
                );
                for page in &pages {
                    let first = page.slots[0].rect_mm;
                    let outer_x = first.x - gap / 2.0;
                    assert!(
                        page.crop_marks
                            .iter()
                            .any(|line| (line.x1 - outer_x).abs() < 1e-6
                                && (line.x2 - outer_x).abs() < 1e-6)
                    );
                    if page.slots.len() > 1 {
                        let shared_x = first.x + first.width + gap / 2.0;
                        assert!(
                            page.crop_marks
                                .iter()
                                .any(|line| (line.x1 - shared_x).abs() < 1e-6
                                    && (line.x2 - shared_x).abs() < 1e-6),
                            "the seam has one cut line at its centre"
                        );
                    }
                    for (index, line) in page.crop_marks.iter().enumerate() {
                        let left = line.x1.min(line.x2) - stroke / 2.0;
                        let right = line.x1.max(line.x2) + stroke / 2.0;
                        let top = line.y1.min(line.y2) - stroke / 2.0;
                        let bottom = line.y1.max(line.y2) + stroke / 2.0;
                        assert!(left >= margin - 1e-6 && top >= margin - 1e-6);
                        assert!(right <= 210.0 - margin + 1e-6 && bottom <= 297.0 - margin + 1e-6);
                        for slot in &page.slots {
                            let rect = slot.rect_mm;
                            assert!(
                                right <= rect.x + 1e-6
                                    || left >= rect.x + rect.width - 1e-6
                                    || bottom <= rect.y + 1e-6
                                    || top >= rect.y + rect.height - 1e-6,
                                "shared line crosses a card face"
                            );
                        }
                        for other in &page.crop_marks[index + 1..] {
                            if (line.x1 - line.x2).abs() < 1e-6
                                && (other.x1 - other.x2).abs() < 1e-6
                                && (line.x1 - other.x1).abs() < 1e-6
                            {
                                assert!(
                                    line.y2 < other.y1 || other.y2 < line.y1,
                                    "overlapping vertical cut lines"
                                );
                            }
                            if (line.y1 - line.y2).abs() < 1e-6
                                && (other.y1 - other.y2).abs() < 1e-6
                                && (line.y1 - other.y1).abs() < 1e-6
                            {
                                assert!(
                                    line.x2 < other.x1 || other.x2 < line.x1,
                                    "overlapping horizontal cut lines"
                                );
                            }
                        }
                    }
                }
                let last = pages.last().unwrap();
                if last.slots.len() == 4 {
                    let final_card = last.slots.last().unwrap().rect_mm;
                    let bottom = final_card.y + final_card.height + gap / 2.0;
                    let bottom_line = last
                        .crop_marks
                        .iter()
                        .find(|line| {
                            (line.y1 - bottom).abs() < 1e-6 && (line.y2 - bottom).abs() < 1e-6
                        })
                        .unwrap();
                    assert!(
                        (bottom_line.x2 - (final_card.x + final_card.width + gap / 2.0)).abs()
                            < 1e-6,
                        "partial last row has no cuts for empty card positions"
                    );
                }
            }
        }
    }
}

#[test]
fn shared_cut_grid_requires_a_real_line_width_channel() {
    let mut options = CardPrintOptions {
        cut_line_style: CardPrintCutLineStyle::DashedGrid,
        gap_mm: 0.2,
        stroke_width_mm: 0.2,
        ..Default::default()
    };
    Layout::new(&options, 1).unwrap();
    options.gap_mm = 0.199;
    assert!(
        Layout::new(&options, 1)
            .err()
            .unwrap()
            .to_string()
            .starts_with("card_print.cut_clearance:")
    );
    options.cut_line_style = CardPrintCutLineStyle::CornerMarks;
    options.gap_mm = 0.2;
    assert!(
        Layout::new(&options, 1)
            .err()
            .unwrap()
            .to_string()
            .starts_with("card_print.crop_clearance:")
    );
}

#[test]
fn physical_admission_checks_quantity_pages_gap_stroke_and_nonfinite_values() {
    let defaults = CardPrintOptions::default();
    assert!(Layout::new(&defaults, 0).is_err());
    assert!(Layout::new(&defaults, 4097).is_err());
    assert!(Layout::new(&defaults, 4096).is_err()); // 456 pages, above the page cap.
    let dense = CardPrintOptions {
        card_width_mm: 20.0,
        card_height_mm: 20.0,
        ..defaults
    };
    assert!(Layout::new(&dense, 4096).is_ok());
    assert!(Layout::new(&defaults, 9 * 256).is_ok());
    assert!(Layout::new(&defaults, 9 * 256 + 1).is_err());
    let single = CardPrintOptions {
        card_width_mm: 190.0,
        card_height_mm: 270.0,
        ..defaults
    };
    assert!(Layout::new(&single, 256).is_ok());
    assert!(Layout::new(&single, 257).is_err());
    for options in [
        CardPrintOptions {
            margin_mm: -0.001,
            ..defaults
        },
        CardPrintOptions {
            card_width_mm: f64::NAN,
            ..defaults
        },
        CardPrintOptions {
            card_height_mm: f64::INFINITY,
            ..defaults
        },
        CardPrintOptions {
            card_width_mm: 63.000_1,
            ..defaults
        },
        CardPrintOptions {
            crop_mark_offset_mm: 0.099,
            ..defaults
        },
        CardPrintOptions {
            gap_mm: 2.099,
            ..defaults
        },
        CardPrintOptions {
            stroke_width_mm: 0.049,
            ..defaults
        },
        CardPrintOptions {
            card_height_mm: 300.0,
            ..defaults
        },
    ] {
        assert!(Layout::new(&options, 1).is_err(), "accepted {options:?}");
    }
    let exact_gap = CardPrintOptions {
        gap_mm: 2.1,
        ..defaults
    };
    assert!(Layout::new(&exact_gap, 9).is_ok());
    let crowded = CardPrintOptions {
        card_width_mm: 10.0,
        card_height_mm: 10.0,
        margin_mm: 0.0,
        gap_mm: 0.15,
        crop_mark_offset_mm: 0.025,
        crop_mark_length_mm: 0.1,
        stroke_width_mm: 0.05,
        ..defaults
    };
    assert!(Layout::new(&crowded, 1).is_err());
    assert!(serde_json::from_value::<CardPrintOptions>(json!({"paper":"a3"})).is_err());
    assert!(serde_json::from_value::<CardPrintOptions>(json!({"bleed_mm":3})).is_err());
}

#[test]
fn exact_gap_and_odd_micrometre_stroke_keep_all_marks_clear_of_neighboring_cards() {
    let f = Fixture::new(&[9]);
    for options in [
        CardPrintOptions {
            gap_mm: 2.1,
            ..CardPrintOptions::default()
        },
        CardPrintOptions {
            gap_mm: 2.101,
            stroke_width_mm: 0.201,
            ..CardPrintOptions::default()
        },
        CardPrintOptions {
            margin_mm: 0.0,
            ..CardPrintOptions::default()
        },
    ] {
        let plan = f.project.plan_card_print(f.source, &options).unwrap();
        assert_eq!(plan.pages.len(), 1);
        assert_eq!(plan.pages[0].slots.len(), 9);
        assert_safe_geometry(&plan);
    }
}

#[test]
fn quantity_and_page_admission_happen_before_loading_missing_artwork() {
    for (quantities, options, code) in [
        (
            vec![999; 5],
            CardPrintOptions::default(),
            "card_print.quantity_limit",
        ),
        (
            vec![257],
            CardPrintOptions {
                card_width_mm: 190.0,
                card_height_mm: 270.0,
                ..CardPrintOptions::default()
            },
            "card_print.page_limit",
        ),
    ] {
        let f = Fixture::new(&quantities);
        let request = f.project.read_card_kit(f.source).unwrap();
        let pin = request.cards[0].artwork.unwrap();
        let revision = f.project.revision(pin.revision_id).unwrap();
        let digest = revision.content.digest.to_string();
        fs::remove_file(
            f.root
                .join("print.shape/objects/b3")
                .join(&digest[..2])
                .join(&digest[2..]),
        )
        .unwrap();
        assert!(f.project.read_revision_content(pin.revision_id).is_err());
        let error = f
            .project
            .plan_card_print(f.source, &options)
            .unwrap_err()
            .to_string();
        assert!(error.starts_with(code), "{error}");
    }
}

#[test]
fn raster_metadata_budgets_bound_decoded_pixels_and_encoded_bytes_before_decode() {
    let f = Fixture::new(&[1]);
    let request = f.project.read_card_kit(f.source).unwrap();
    let mut metadata = f
        .project
        .revision(request.cards[0].artwork.unwrap().revision_id)
        .unwrap();
    let Some(ArtifactContentContract::ImageRaster(raster)) = &mut metadata.content_contract else {
        panic!("fixture has accepted raster metadata");
    };
    // Only admission metadata is varied; no fake raster is written into accepted history.
    raster.width = 8192;
    raster.height = 8192;
    let mut budget = ArtworkBudget::default();
    budget.add(&metadata).unwrap();
    budget.add(&metadata).unwrap(); // exactly 128 Mi pixels across two unique card SVGs.
    assert_eq!(budget.pixels, MAX_ARTWORK_PIXELS);
    assert!(
        budget
            .add(&metadata)
            .unwrap_err()
            .to_string()
            .starts_with("card_print.artwork_pixels:")
    );
    let Some(ArtifactContentContract::ImageRaster(raster)) = &mut metadata.content_contract else {
        unreachable!()
    };
    raster.width = 1;
    raster.height = 1;
    metadata.content.byte_length = MAX_ARTWORK_BYTES / 2;
    let mut budget = ArtworkBudget::default();
    budget.add(&metadata).unwrap();
    budget.add(&metadata).unwrap();
    metadata.content.byte_length = 1;
    assert!(
        budget
            .add(&metadata)
            .unwrap_err()
            .to_string()
            .starts_with("card_print.artwork_bytes:")
    );
}

#[test]
fn print_fonts_meet_physical_minima_and_overflow_is_rejected_instead_of_shrinking() {
    let mut f = Fixture::new(&[1]);
    let draft = f.project.open_card_kit_draft(f.source.artifact_id).unwrap();
    let mut composition = draft.composition;
    composition.cards[0].flavor = "Take a breath.".into();
    let saved = f
        .project
        .save_card_kit_draft(draft.artifact_id, &draft.draft_digest, &composition)
        .unwrap();
    let candidate = f
        .project
        .propose_card_kit(saved.artifact_id, &saved.draft_digest)
        .unwrap();
    let accepted = f.project.accept_card_kit(candidate).unwrap();
    let source = WorkingInput {
        artifact_id: accepted.artifact_id,
        revision_id: accepted.id,
    };
    let plan = f
        .project
        .plan_card_print(source, &CardPrintOptions::default())
        .unwrap();
    let svg = &plan.cards[0].svg;
    let mm_per_unit = plan.pages[0].slots[0].content_rect_mm.width / 750.0;
    for (text, min_points) in [("Draw one card.", 7.0), ("Take a breath.", 6.0)] {
        let text_element = svg
            .split_once(&format!(">{text}</text>"))
            .unwrap()
            .0
            .rsplit_once("<text ")
            .unwrap()
            .1;
        let font = text_element
            .split_once("font-size=\"")
            .unwrap()
            .1
            .split('"')
            .next()
            .unwrap()
            .parse::<f64>()
            .unwrap();
        assert!(font * mm_per_unit * 72.0 / 25.4 >= min_points);
    }
    composition.cards[0].rules = "W".repeat(120); // Material preview fits six lines at 24 units.
    let material_preview = f.project.preview_card_kit(&composition, None).unwrap();
    assert_eq!(material_preview["ready_to_export"], true);
    let current = f.project.open_card_kit_draft(f.source.artifact_id).unwrap();
    let saved = f
        .project
        .save_card_kit_draft(current.artifact_id, &current.draft_digest, &composition)
        .unwrap();
    let candidate = f
        .project
        .propose_card_kit(saved.artifact_id, &saved.draft_digest)
        .unwrap();
    let accepted = f.project.accept_card_kit(candidate).unwrap();
    let source = WorkingInput {
        artifact_id: accepted.artifact_id,
        revision_id: accepted.id,
    };
    assert!(
        f.project
            .plan_card_print(source, &CardPrintOptions::default())
            .unwrap_err()
            .to_string()
            .starts_with("card_print.text_overflow: card card-0")
    );
}

#[test]
fn print_reads_exact_accepted_version_and_rejects_incomplete_and_unknown_source() {
    let mut f = Fixture::new(&[1]);
    let draft = f.project.open_card_kit_draft(f.source.artifact_id).unwrap();
    let mut changed = draft.composition;
    changed.cards[0].title = "Changed card".into();
    changed.cards[0].quantity = 2;
    let saved = f
        .project
        .save_card_kit_draft(draft.artifact_id, &draft.draft_digest, &changed)
        .unwrap();
    let candidate = f
        .project
        .propose_card_kit(saved.artifact_id, &saved.draft_digest)
        .unwrap();
    let accepted = f.project.accept_card_kit(candidate).unwrap();
    let historical = f
        .project
        .plan_card_print(f.source, &CardPrintOptions::default())
        .unwrap();
    assert_eq!(historical.total_copies, 1);
    assert!(historical.cards[0].svg.contains(">Card 0</text>"));
    let new_source = WorkingInput {
        artifact_id: accepted.artifact_id,
        revision_id: accepted.id,
    };
    assert_eq!(
        f.project
            .plan_card_print(new_source, &CardPrintOptions::default())
            .unwrap()
            .total_copies,
        2
    );
    changed.cards[0].artwork = None;
    let current = f.project.open_card_kit_draft(f.source.artifact_id).unwrap();
    let saved = f
        .project
        .save_card_kit_draft(current.artifact_id, &current.draft_digest, &changed)
        .unwrap();
    let candidate = f
        .project
        .propose_card_kit(saved.artifact_id, &saved.draft_digest)
        .unwrap();
    let accepted = f.project.accept_card_kit(candidate).unwrap();
    let source = WorkingInput {
        artifact_id: accepted.artifact_id,
        revision_id: accepted.id,
    };
    assert!(
        f.project
            .plan_card_print(source, &CardPrintOptions::default())
            .is_err()
    );
    let wrong = WorkingInput {
        artifact_id: shape_domain::ArtifactId::new(),
        ..source
    };
    assert!(
        f.project
            .plan_card_print(wrong, &CardPrintOptions::default())
            .is_err()
    );
}

#[test]
fn accepted_real_rule_prints_without_a_punctuation_only_line_and_long_clusters_are_rejected() {
    let mut f = Fixture::new(&[1]);
    let draft = f.project.open_card_kit_draft(f.source.artifact_id).unwrap();
    let mut composition = draft.composition;
    let rule = "获得 1 点专注，然后抽 1 张牌。";
    composition.cards[0].rules = rule.into();
    let saved = f
        .project
        .save_card_kit_draft(draft.artifact_id, &draft.draft_digest, &composition)
        .unwrap();
    let candidate = f
        .project
        .propose_card_kit(saved.artifact_id, &saved.draft_digest)
        .unwrap();
    let accepted = f.project.accept_card_kit(candidate).unwrap();
    let source = WorkingInput {
        artifact_id: accepted.artifact_id,
        revision_id: accepted.id,
    };
    let plan = f
        .project
        .plan_card_print(source, &CardPrintOptions::default())
        .unwrap();
    let rules: Vec<_> = plan.cards[0]
        .svg
        .split("<text ")
        .skip(1)
        .filter(|element| element.contains("font-size=\"30\""))
        .map(|element| {
            element
                .split_once('>')
                .unwrap()
                .1
                .split_once("</text>")
                .unwrap()
                .0
        })
        .filter(|text| !text.is_empty())
        .collect();
    assert!(rules.len() > 1);
    assert!(rules.last().unwrap().ends_with("牌。"));
    assert!(!rules.contains(&"。"));
    assert_eq!(rules.concat().replace(' ', ""), rule.replace(' ', ""));
    assert_eq!(
        f.project.read_card_kit(source).unwrap().cards[0].rules,
        rule
    );
    composition.cards[0].rules = format!("文{}", "。".repeat(40));
    let current = f.project.open_card_kit_draft(source.artifact_id).unwrap();
    let saved = f
        .project
        .save_card_kit_draft(current.artifact_id, &current.draft_digest, &composition)
        .unwrap();
    let candidate = f
        .project
        .propose_card_kit(saved.artifact_id, &saved.draft_digest)
        .unwrap();
    let accepted = f.project.accept_card_kit(candidate).unwrap();
    let source = WorkingInput {
        artifact_id: accepted.artifact_id,
        revision_id: accepted.id,
    };
    assert!(
        f.project
            .plan_card_print(source, &CardPrintOptions::default())
            .unwrap_err()
            .to_string()
            .starts_with("card_print.text_overflow:")
    );
}
