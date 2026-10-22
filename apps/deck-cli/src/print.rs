//! Transient sheet choices over Shape's admitted card faces and crop-mark geometry.
//! No project writes, second card renderer, or per-copy image payloads.
use crate::{Result, fail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use shape_core::{
    CardPrintFlipEdge, CardPrintRequestOptions, CardPrintSides, CardPrintSidesOptions, ShapeProject,
};
use shape_domain::{CardKitRequest, WorkingInput};
use std::collections::BTreeMap;

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
    #[serde(skip_serializing_if = "CardPrintSides::is_fronts")]
    pub sides: CardPrintSides,
    #[serde(skip_serializing_if = "CardPrintFlipEdge::is_long")]
    pub flip_edge: CardPrintFlipEdge,
    #[serde(skip_serializing_if = "is_zero")]
    pub back_offset_x_mm: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub back_offset_y_mm: f64,
}
fn is_zero(value: &f64) -> bool {
    *value == 0.0
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
    if capacity == 0 || copies.is_empty() || copies.len() > 4096 {
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
        options.fit_grid(&composition, columns, rows)?;
    }
    // Resolve capacity with geometry alone, then admit the exact requested copies before resources.
    let (columns, rows) = match dimensions {
        Some(grid) => grid,
        None => options.grid_dimensions(&composition)?,
    };
    let copies = select_copies(&composition, &imposition, columns * rows)?;
    if copies.len().div_ceil(columns * rows) > 256 {
        return Err(fail(
            "print_pages",
            "card_print.page_limit: too many copies or pages",
        ));
    }
    let mut selected_quantities = BTreeMap::<String, u16>::new();
    for (id, _) in &copies {
        *selected_quantities.entry(id.clone()).or_default() += 1;
    }
    let selection: Vec<_> = selected_quantities.into_iter().collect();
    // The upstream service remains the owner of content readiness, SVGs and crop clearance.
    let sides = CardPrintSidesOptions {
        sides: imposition.sides,
        flip_edge: imposition.flip_edge,
        back_offset_x_mm: imposition.back_offset_x_mm,
        back_offset_y_mm: imposition.back_offset_y_mm,
    };
    let plan =
        project.plan_card_print_job_request(source, &options, &selection, dimensions, &sides)?;
    let mut plan = serde_json::to_value(plan)?;
    plan["imposition"] = json!(imposition);
    Ok(plan)
}

#[cfg(test)]
mod tests;
