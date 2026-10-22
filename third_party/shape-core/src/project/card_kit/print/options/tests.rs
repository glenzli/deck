use super::*;
use crate::project::card_kit::print::Layout;
use serde_json::json;

fn request(width: u32, height: u32) -> CardKitRequest {
    serde_json::from_value(json!({"name":"Faces","canvas_width":width,"canvas_height":height,"layout":{"mode":"auto"},"cards":[{"id":"one"}]})).unwrap()
}

#[test]
fn defaults_preserve_legacy_size_and_fit_all_legal_aspect_extremes_on_default_papers() {
    let mut legacy = request(2400, 240);
    legacy.layout = None;
    assert_eq!(defaults(&legacy), CardPrintOptions::default());
    for paper in [CardPrintPaper::A4, CardPrintPaper::Letter] {
        for width in [240, 241, 750, 2400] {
            for height in [240, 241, 1050, 2400] {
                let input = CardPrintRequestOptions {
                    paper,
                    ..Default::default()
                };
                let resolved = input.resolve(&request(width, height)).unwrap();
                Layout::new(&resolved, 1).unwrap();
                assert!((10.0..=300.0).contains(&resolved.card_width_mm));
                assert!((10.0..=300.0).contains(&resolved.card_height_mm));
                let ratio = f64::from(width) / f64::from(height);
                assert!((resolved.card_width_mm - resolved.card_height_mm * ratio).abs() <= 0.006);
            }
        }
    }
    let landscape = defaults(&request(1200, 800));
    assert_eq!(
        (landscape.card_width_mm, landscape.card_height_mm),
        (63.0, 42.0)
    );
    let horizontal = defaults(&request(2400, 240));
    assert_eq!(
        (horizontal.card_width_mm, horizontal.card_height_mm),
        (100.0, 10.0)
    );
    let vertical = defaults(&request(240, 2400));
    assert_eq!(
        (vertical.card_width_mm, vertical.card_height_mm),
        (28.28, 282.8)
    );
}

#[test]
fn one_axis_derives_the_other_both_axes_stay_explicit_and_neither_is_silently_shrunk() {
    let request = request(1200, 800);
    for (input, expected) in [
        (json!({"card_width_mm":90}), (90.0, 60.0)),
        (json!({"card_height_mm":50}), (75.0, 50.0)),
        (
            json!({"card_width_mm":70,"card_height_mm":70}),
            (70.0, 70.0),
        ),
    ] {
        let input: CardPrintRequestOptions = serde_json::from_value(input).unwrap();
        let resolved = input.resolve(&request).unwrap();
        assert_eq!((resolved.card_width_mm, resolved.card_height_mm), expected);
    }
    let too_wide: CardPrintRequestOptions =
        serde_json::from_value(json!({"card_width_mm":250})).unwrap();
    let resolved = too_wide.resolve(&request).unwrap();
    assert!((resolved.card_width_mm - 250.0).abs() < 0.000_001);
    assert!(
        Layout::new(&resolved, 1)
            .err()
            .unwrap()
            .to_string()
            .starts_with("card_print.paper_fit:")
    );
    let too_tall: CardPrintRequestOptions =
        serde_json::from_value(json!({"card_height_mm":300})).unwrap();
    assert!(
        too_tall
            .resolve(&request)
            .unwrap_err()
            .to_string()
            .starts_with("card_print.option_range:")
    );
    assert!(serde_json::from_value::<CardPrintRequestOptions>(json!({"bleed_mm":3})).is_err());
}

#[test]
fn cut_style_is_backward_compatible_and_grid_fitting_reserves_the_line_outer_edge() {
    let legacy: CardPrintRequestOptions = serde_json::from_value(json!({})).unwrap();
    assert_eq!(legacy.cut_line_style, CardPrintCutLineStyle::CornerMarks);
    assert!(
        serde_json::from_value::<CardPrintRequestOptions>(json!({"cut_line_style":"unknown"}))
            .is_err()
    );
    let request = request(750, 1050);
    for paper in [CardPrintPaper::A4, CardPrintPaper::Letter] {
        for margin in [0.0, 5.0, 12.0, 50.0] {
            for (columns, rows) in [(1, 1), (2, 3), (3, 3)] {
                let mut input: CardPrintRequestOptions = serde_json::from_value(json!({
                    "paper":paper,"cut_line_style":"dashed_grid","gap_mm":0.2,
                    "margin_mm":margin,"crop_mark_offset_mm":0,"crop_mark_length_mm":10
                }))
                .unwrap();
                input.fit_grid(&request, columns, rows).unwrap();
                let resolved = input.resolve(&request).unwrap();
                let layout = Layout::with_grid(&resolved, 1, Some((columns, rows))).unwrap();
                assert!(f64::from(layout.origin_x) / 1000.0 >= margin + 0.2 - 1e-9);
                assert!(f64::from(layout.origin_y) / 1000.0 >= margin + 0.2 - 1e-9);
                assert!(
                    (resolved.card_width_mm / resolved.card_height_mm - 750.0 / 1050.0).abs()
                        < 0.0001
                );
            }
        }
    }
}
