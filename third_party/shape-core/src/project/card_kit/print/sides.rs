//! Duplex placement reflects physical sheet coordinates, never artwork or authored card order.
use super::{
    CardKitError, CardPrintLine, CardPrintPage, CardPrintPlan, CardPrintRect, micrometres,
    print_invalid,
};
use serde::{Deserialize, Serialize};

pub const CARD_BACK_RESOURCE_ID: &str = "@deck/back";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardPrintSides {
    #[default]
    Fronts,
    Duplex,
    Backs,
}
impl CardPrintSides {
    pub fn is_fronts(&self) -> bool {
        *self == Self::Fronts
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardPrintFlipEdge {
    #[default]
    Long,
    Short,
}
impl CardPrintFlipEdge {
    pub fn is_long(&self) -> bool {
        *self == Self::Long
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CardPrintSide {
    Front,
    Back,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CardPrintSidesOptions {
    pub sides: CardPrintSides,
    pub flip_edge: CardPrintFlipEdge,
    pub back_offset_x_mm: f64,
    pub back_offset_y_mm: f64,
}

impl CardPrintSidesOptions {
    pub(super) fn validate(&self) -> Result<(), CardKitError> {
        for value in [self.back_offset_x_mm, self.back_offset_y_mm] {
            micrometres(value, -10.0, 10.0).map_err(|_| {
                print_invalid(
                    "card_print.back_offset",
                    "back offsets need finite -10 to 10 mm values with 0.001 mm precision",
                )
            })?;
        }
        Ok(())
    }
}

pub(super) fn apply(
    plan: &mut CardPrintPlan,
    options: &CardPrintSidesOptions,
) -> Result<(), CardKitError> {
    options.validate()?;
    if options.sides == CardPrintSides::Fronts {
        return Ok(());
    }
    let sheet_count = plan.pages.len();
    let mut pages = Vec::with_capacity(
        sheet_count
            * if options.sides == CardPrintSides::Duplex {
                2
            } else {
                1
            },
    );
    for (sheet_index, mut front) in std::mem::take(&mut plan.pages).into_iter().enumerate() {
        let mut back = front.clone();
        back.side = Some(CardPrintSide::Back);
        back.sheet_index = Some(sheet_index);
        for slot in &mut back.slots {
            let old = slot.rect_mm;
            slot.rect_mm = reflect_rect(old, plan, options);
            slot.content_rect_mm.x += slot.rect_mm.x - old.x;
            slot.content_rect_mm.y += slot.rect_mm.y - old.y;
            slot.face = Some(CardPrintSide::Back);
            slot.resource_id = Some(CARD_BACK_RESOURCE_ID.into());
        }
        for line in &mut back.crop_marks {
            *line = reflect_line(*line, plan, options);
        }
        admit_back(&back, plan)?;
        if options.sides == CardPrintSides::Duplex {
            front.index = pages.len();
            front.side = Some(CardPrintSide::Front);
            front.sheet_index = Some(sheet_index);
            for slot in &mut front.slots {
                slot.face = Some(CardPrintSide::Front);
            }
            pages.push(front);
        }
        back.index = pages.len();
        pages.push(back);
    }
    plan.total_placements = Some(
        plan.total_copies
            * if options.sides == CardPrintSides::Duplex {
                2
            } else {
                1
            },
    );
    plan.sheet_count = Some(sheet_count);
    plan.duplex = options.sides == CardPrintSides::Duplex;
    plan.page_count = pages.len();
    plan.pages = pages;
    Ok(())
}

fn reflect_rect(
    rect: CardPrintRect,
    plan: &CardPrintPlan,
    options: &CardPrintSidesOptions,
) -> CardPrintRect {
    CardPrintRect {
        x: if options.flip_edge == CardPrintFlipEdge::Long {
            plan.sheet_width_mm - rect.x - rect.width
        } else {
            rect.x
        } + options.back_offset_x_mm,
        y: if options.flip_edge == CardPrintFlipEdge::Short {
            plan.sheet_height_mm - rect.y - rect.height
        } else {
            rect.y
        } + options.back_offset_y_mm,
        ..rect
    }
}
fn reflect_line(
    line: CardPrintLine,
    plan: &CardPrintPlan,
    options: &CardPrintSidesOptions,
) -> CardPrintLine {
    let x = |value| if options.flip_edge == CardPrintFlipEdge::Long { plan.sheet_width_mm - value } else { value } + options.back_offset_x_mm;
    let y = |value| if options.flip_edge == CardPrintFlipEdge::Short { plan.sheet_height_mm - value } else { value } + options.back_offset_y_mm;
    CardPrintLine {
        x1: x(line.x1),
        x2: x(line.x2),
        y1: y(line.y1),
        y2: y(line.y2),
    }
}
fn admit_back(page: &CardPrintPage, plan: &CardPrintPlan) -> Result<(), CardKitError> {
    let half = plan.options.stroke_width_mm / 2.0;
    for rect in page
        .slots
        .iter()
        .flat_map(|slot| [slot.rect_mm, slot.content_rect_mm])
    {
        if rect.x < -1e-6
            || rect.y < -1e-6
            || rect.x + rect.width > plan.sheet_width_mm + 1e-6
            || rect.y + rect.height > plan.sheet_height_mm + 1e-6
        {
            return Err(print_invalid(
                "card_print.back_offset",
                "the back offset moves a card outside the sheet",
            ));
        }
    }
    for line in &page.crop_marks {
        if line.x1.min(line.x2) - half < -1e-6
            || line.y1.min(line.y2) - half < -1e-6
            || line.x1.max(line.x2) + half > plan.sheet_width_mm + 1e-6
            || line.y1.max(line.y2) + half > plan.sheet_height_mm + 1e-6
        {
            return Err(print_invalid(
                "card_print.back_offset",
                "the back offset moves a cut line outside the sheet",
            ));
        }
    }
    Ok(())
}
