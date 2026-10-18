//! Transient sheet choices over Shape's admitted card faces and crop-mark geometry.
//! No project writes, second card renderer, or per-copy image payloads.
use crate::{Result, fail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use shape_core::{CardPrintPaper, CardPrintRequestOptions, ShapeProject};
use shape_domain::{CardKitRequest, WorkingInput};

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Content {
    #[default]
    Quantities,
    OneEach,
    Repeat,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Imposition {
    pub columns: Option<u8>,
    pub rows: Option<u8>,
    pub content: Content,
    pub card_id: Option<String>,
    pub copies: Option<u16>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    action: String,
    source: WorkingInput,
    #[serde(default)]
    options: CardPrintRequestOptions,
    #[serde(default)]
    imposition: Option<Imposition>,
}

pub fn action(project: &ShapeProject, text: &str) -> Result<Value> {
    let request: Request = serde_json::from_str(text)?;
    if request.action != "print_plan" {
        return Err(fail("print_request", "expected print_plan"));
    }
    Ok(json!({"print_plan": plan(project, request.source, request.options, request.imposition)?}))
}

fn grid(imposition: &Imposition) -> Result<Option<(usize, usize)>> {
    match (imposition.columns, imposition.rows) {
        (None, None) => Ok(None),
        (Some(c), Some(r)) if (1..=8).contains(&c) && (1..=8).contains(&r) => {
            Ok(Some((usize::from(c), usize::from(r))))
        }
        _ => Err(fail(
            "print_grid",
            "deck.print.grid: choose 1 to 8 columns and rows",
        )),
    }
}

fn um(value: f64) -> Result<f64> {
    let scaled = value * 1000.0;
    if !value.is_finite() || (scaled - scaled.round()).abs() > 0.000_001 {
        return Err(fail(
            "print_measurement",
            "card_print.option_range: expected integral micrometres",
        ));
    }
    Ok(scaled.round())
}

fn fit_grid(
    options: &mut CardPrintRequestOptions,
    composition: &CardKitRequest,
    c: usize,
    r: usize,
) -> Result<()> {
    let (paper_w, paper_h) = match options.paper {
        CardPrintPaper::A4 => (210_000.0, 297_000.0),
        CardPrintPaper::Letter => (215_900.0, 279_400.0),
    };
    // Physical range and crop-clearance admission still belongs to the pinned planner below.
    let margin = um(options.margin_mm)?;
    let gap = um(options.gap_mm)?;
    let extent = um(options.crop_mark_offset_mm)?
        + um(options.crop_mark_length_mm)?
        + (um(options.stroke_width_mm)? / 2.0).ceil();
    let c = f64::from(u8::try_from(c)?);
    let r = f64::from(u8::try_from(r)?);
    let max_w = (paper_w - 2.0 * (margin + extent) - (c - 1.0) * gap) / c;
    let max_h = (paper_h - 2.0 * (margin + extent) - (r - 1.0) * gap) / r;
    let (w, h) = if composition.layout.is_none() {
        (750.0, 1050.0)
    } else {
        (
            f64::from(composition.canvas_width),
            f64::from(composition.canvas_height),
        )
    };
    let scale = (max_w / w).min(max_h / h);
    options.card_width_mm = Some((w * scale).floor() / 1000.0);
    options.card_height_mm = Some((h * scale).floor() / 1000.0);
    Ok(())
}

fn select_copies(
    composition: &CardKitRequest,
    imposition: &Imposition,
    capacity: usize,
) -> Result<Vec<(String, u16)>> {
    let mut copies = Vec::new();
    match imposition.content {
        Content::Quantities | Content::OneEach => {
            for card in &composition.cards {
                let count = if imposition.content == Content::OneEach {
                    1
                } else {
                    card.quantity
                };
                for i in 1..=count {
                    copies.push((card.id.clone(), i));
                }
            }
        }
        Content::Repeat => {
            let card = composition
                .cards
                .iter()
                .find(|c| Some(&c.id) == imposition.card_id.as_ref())
                .ok_or_else(|| {
                    fail(
                        "print_card",
                        "deck.print.card: choose a card from this accepted version",
                    )
                })?;
            let count = imposition.copies.map_or(capacity, usize::from);
            if !(1..=4096).contains(&count) {
                return Err(fail(
                    "print_copies",
                    "card_print.quantity_limit: choose 1 to 4096 copies",
                ));
            }
            for i in 1..=count {
                copies.push((card.id.clone(), u16::try_from(i)?));
            }
        }
    }
    if capacity == 0
        || copies.is_empty()
        || copies.len() > 4096
        || copies.len().div_ceil(capacity) > 256
    {
        return Err(fail(
            "print_pages",
            "card_print.page_limit: too many copies or pages",
        ));
    }
    Ok(copies)
}

fn plan(
    project: &ShapeProject,
    source: WorkingInput,
    mut options: CardPrintRequestOptions,
    imposition: Option<Imposition>,
) -> Result<Value> {
    let Some(imposition) = imposition else {
        return Ok(serde_json::to_value(
            project.plan_card_print_request(source, &options)?,
        )?);
    };
    let dimensions = grid(&imposition)?;
    let composition = project.read_card_kit(source)?;
    if let Some((columns, rows)) = dimensions {
        fit_grid(&mut options, &composition, columns, rows)?;
    }
    // The upstream service remains the owner of content readiness, SVGs and crop clearance.
    let mut plan = serde_json::to_value(project.plan_card_print_request(source, &options)?)?;
    let columns = dimensions.map_or(
        usize::try_from(plan["columns"].as_u64().unwrap_or(0))?,
        |v| v.0,
    );
    let rows = dimensions.map_or(usize::try_from(plan["rows"].as_u64().unwrap_or(0))?, |v| {
        v.1
    });
    let capacity = columns * rows;
    let copies = select_copies(&composition, &imposition, capacity)?;
    let template = plan["pages"][0]["slots"][0].clone();
    let marks: Vec<_> = plan["pages"][0]["crop_marks"]
        .as_array()
        .ok_or_else(|| fail("print_geometry", "missing admitted crop marks"))?
        .iter()
        .take(8)
        .cloned()
        .collect();
    if marks.len() != 8 {
        return Err(fail("print_geometry", "incomplete admitted crop marks"));
    }
    let number = |value: &Value, key: &str| -> Result<f64> {
        value[key]
            .as_f64()
            .ok_or_else(|| fail("print_geometry", format!("missing admitted {key}")))
    };
    let width = number(&template["rect_mm"], "width")?;
    let height = number(&template["rect_mm"], "height")?;
    let gap = number(&plan["options"], "gap_mm")?;
    let c = f64::from(u8::try_from(columns)?);
    let r = f64::from(u8::try_from(rows)?);
    // Centre using integral micrometres, avoiding a floating-point floor losing 1 µm.
    let origin_x =
        ((um(number(&plan, "sheet_width_mm")?)? - c * um(width)? - (c - 1.0) * um(gap)?) / 2.0)
            .floor()
            / 1000.0;
    let origin_y =
        ((um(number(&plan, "sheet_height_mm")?)? - r * um(height)? - (r - 1.0) * um(gap)?) / 2.0)
            .floor()
            / 1000.0;
    let x = number(&template["rect_mm"], "x")?;
    let y = number(&template["rect_mm"], "y")?;
    let mut pages = Vec::new();
    for (index, chunk) in copies.chunks(capacity).enumerate() {
        let mut slots = Vec::new();
        let mut crop_marks = Vec::new();
        for (index, (card_id, copy_index)) in chunk.iter().enumerate() {
            let dx = origin_x + f64::from(u8::try_from(index % columns)?) * (width + gap) - x;
            let dy = origin_y + f64::from(u8::try_from(index / columns)?) * (height + gap) - y;
            let mut slot = template.clone();
            slot["card_id"] = json!(card_id);
            slot["copy_index"] = json!(copy_index);
            for rect in ["rect_mm", "content_rect_mm"] {
                slot[rect]["x"] = json!(number(&template[rect], "x")? + dx);
                slot[rect]["y"] = json!(number(&template[rect], "y")? + dy);
            }
            slots.push(slot);
            for mark in &marks {
                let mut mark = mark.clone();
                for (key, delta) in [("x1", dx), ("x2", dx), ("y1", dy), ("y2", dy)] {
                    mark[key] = json!(number(&mark, key)? + delta);
                }
                crop_marks.push(mark);
            }
        }
        pages.push(json!({"index":index,"slots":slots,"crop_marks":crop_marks}));
    }
    plan["cards"]
        .as_array_mut()
        .ok_or_else(|| fail("print_geometry", "missing admitted card faces"))?
        .retain(|card| copies.iter().any(|(id, _)| card["id"] == *id));
    plan["columns"] = json!(columns);
    plan["rows"] = json!(rows);
    plan["slots_per_page"] = json!(capacity);
    plan["total_copies"] = json!(copies.len());
    plan["page_count"] = json!(pages.len());
    plan["pages"] = json!(pages);
    plan["imposition"] = json!(imposition);
    Ok(plan)
}

#[cfg(test)]
mod tests;
