//! One bounded physical home-print plan; adapters paint these exact millimetre coordinates.
//! Print settings never enter an accepted composition or create a new artifact revision.
use serde::{Deserialize, Serialize};
use shape_domain::{ArtifactContentContract, ArtifactRevision, CardKitRequest, WorkingInput};

mod options;
pub use options::CardPrintRequestOptions;

use super::{
    CardKitError, ShapeProject,
    layout::shows_artwork,
    preview::{ensure_complete, render_print_card},
};

const MAX_COPIES: usize = 4096;
const MAX_PAGES: usize = 256;
const MAX_SLOTS_PER_PAGE: usize = 512;
const MAX_ARTWORK_BYTES: u64 = 32 * 1024 * 1024;
const MAX_ARTWORK_PIXELS: u64 = 128 * 1024 * 1024;

fn print_invalid(code: &'static str, message: &'static str) -> CardKitError {
    CardKitError::PrintSetting { code, message }
}

#[derive(Default)]
struct ArtworkBudget {
    bytes: u64,
    pixels: u64,
}

impl ArtworkBudget {
    fn add(&mut self, revision: &ArtifactRevision) -> Result<(), CardKitError> {
        let Some(ArtifactContentContract::ImageRaster(raster)) = &revision.content_contract else {
            return Err(print_invalid(
                "card_print.artwork_contract",
                "print artwork needs an accepted raster size contract",
            ));
        };
        if raster.width == 0 || raster.height == 0 {
            return Err(print_invalid(
                "card_print.artwork_contract",
                "print artwork needs nonzero raster dimensions",
            ));
        }
        self.bytes = self.bytes.saturating_add(revision.content.byte_length);
        self.pixels = self
            .pixels
            .saturating_add(u64::from(raster.width) * u64::from(raster.height));
        if self.bytes > MAX_ARTWORK_BYTES {
            return Err(print_invalid(
                "card_print.artwork_bytes",
                "print SVG artwork exceeds 32 MiB across unique cards; use smaller accepted artwork or split the kit",
            ));
        }
        if self.pixels > MAX_ARTWORK_PIXELS {
            return Err(print_invalid(
                "card_print.artwork_pixels",
                "print SVG artwork exceeds 128 Mi pixels across unique cards; resize accepted artwork or split the kit",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardPrintPaper {
    #[default]
    A4,
    Letter,
}

/// Transient export settings, in millimetres with at most three decimal places.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CardPrintOptions {
    pub paper: CardPrintPaper,
    pub card_width_mm: f64,
    pub card_height_mm: f64,
    pub margin_mm: f64,
    pub gap_mm: f64,
    pub crop_mark_offset_mm: f64,
    pub crop_mark_length_mm: f64,
    pub stroke_width_mm: f64,
}

impl Default for CardPrintOptions {
    fn default() -> Self {
        Self {
            paper: CardPrintPaper::A4,
            card_width_mm: 63.0,
            card_height_mm: 88.0,
            margin_mm: 5.0,
            gap_mm: 3.0,
            crop_mark_offset_mm: 0.5,
            crop_mark_length_mm: 1.5,
            stroke_width_mm: 0.2,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CardPrintRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CardPrintLine {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

#[derive(Debug, Serialize)]
pub struct CardPrintSlot {
    pub card_id: String,
    /// One-based copy identity within this card; cards retain their authored ordering.
    pub copy_index: u16,
    pub rect_mm: CardPrintRect,
    pub content_rect_mm: CardPrintRect,
}

#[derive(Debug, Serialize)]
pub struct CardPrintPage {
    pub index: usize,
    pub slots: Vec<CardPrintSlot>,
    pub crop_marks: Vec<CardPrintLine>,
}

#[derive(Debug, Serialize)]
pub struct CardPrintCard {
    pub id: String,
    pub title: String,
    pub svg: String,
}

/// Exact accepted source, unique SVG resources and the one layout used by PDF and preview.
#[derive(Debug, Serialize)]
pub struct CardPrintPlan {
    pub schema: &'static str,
    pub source: WorkingInput,
    pub name: String,
    pub options: CardPrintOptions,
    pub units: &'static str,
    pub line_cap: &'static str,
    pub sheet_width_mm: f64,
    pub sheet_height_mm: f64,
    pub columns: usize,
    pub rows: usize,
    pub slots_per_page: usize,
    pub total_copies: usize,
    pub page_count: usize,
    pub cards: Vec<CardPrintCard>,
    pub pages: Vec<CardPrintPage>,
    pub output_kind: &'static str,
    pub commercial_prepress: bool,
    pub duplex: bool,
}

impl ShapeProject {
    /// Plans one accepted historical kit without changing its draft, head or material pins.
    /// Quantity, geometry and resource bounds are admitted before reading material bytes.
    /// # Errors
    /// Rejects invalid options, excessive copies/pages, incomplete kits and corrupt materials.
    pub fn plan_card_print(
        &self,
        source: WorkingInput,
        options: &CardPrintOptions,
    ) -> Result<CardPrintPlan, CardKitError> {
        let request = self.read_card_kit(source)?;
        self.plan_card_print_composition(source, request, options)
    }

    /// Resolves omitted axes proportionally from this accepted face, then admits one print plan.
    /// # Errors
    /// Rejects invalid, unsupported or overflowing dimensions without changing accepted history.
    pub fn plan_card_print_request(
        &self,
        source: WorkingInput,
        options: &CardPrintRequestOptions,
    ) -> Result<CardPrintPlan, CardKitError> {
        let request = self.read_card_kit(source)?;
        let resolved = options.resolve(&request)?;
        self.plan_card_print_composition(source, request, &resolved)
    }

    fn plan_card_print_composition(
        &self,
        source: WorkingInput,
        request: CardKitRequest,
        options: &CardPrintOptions,
    ) -> Result<CardPrintPlan, CardKitError> {
        let total_copies = request.cards.iter().map(|c| usize::from(c.quantity)).sum();
        let layout = Layout::new(options, total_copies)?;
        let mut artwork_budget = ArtworkBudget::default();
        for pin in request
            .cards
            .iter()
            .filter(|c| shows_artwork(&request, c))
            .filter_map(|c| c.artwork)
        {
            artwork_budget.add(&self.revision(pin.revision_id)?)?;
        }
        let materials = self.card_kit_materials(&request, false)?;
        ensure_complete(&request, &materials).map_err(|error| match error {
            error @ CardKitError::ImageCropLimit { .. } => error,
            _ => print_invalid(
                "card_print.incomplete",
                "each card needs visible content required by its layout before printing",
            ),
        })?;
        let canvas = options::canvas(&request);
        let content_scale = (mm(layout.width) / canvas.0).min(mm(layout.height) / canvas.1);
        let cards = request
            .cards
            .iter()
            .map(|card| {
                Ok(CardPrintCard {
                    id: card.id.clone(),
                    title: card.title.clone(),
                    svg: render_print_card(&request, card, &materials, content_scale)?.svg,
                })
            })
            .collect::<Result<_, CardKitError>>()?;
        let mut pages = Vec::<CardPrintPage>::with_capacity(layout.page_count);
        let mut index = 0;
        for card in &request.cards {
            for copy_index in 1..=card.quantity {
                if index % layout.capacity == 0 {
                    pages.push(CardPrintPage {
                        index: pages.len(),
                        slots: Vec::with_capacity(layout.capacity.min(total_copies - index)),
                        crop_marks: Vec::new(),
                    });
                }
                let (slot, marks) =
                    layout.slot(index % layout.capacity, &card.id, copy_index, canvas);
                let page = pages.last_mut().ok_or_else(|| {
                    print_invalid(
                        "card_print.layout",
                        "print layout is missing an admitted page",
                    )
                })?;
                page.slots.push(slot);
                page.crop_marks.extend(marks);
                index += 1;
            }
        }
        Ok(CardPrintPlan {
            schema: "shape.card-print-plan/1",
            source,
            name: request.name,
            options: *options,
            units: "mm",
            line_cap: "butt",
            sheet_width_mm: mm(layout.sheet_width),
            sheet_height_mm: mm(layout.sheet_height),
            columns: layout.columns,
            rows: layout.rows,
            slots_per_page: layout.capacity,
            total_copies,
            page_count: layout.page_count,
            cards,
            pages,
            output_kind: "home_print_layout",
            commercial_prepress: false,
            duplex: false,
        })
    }
}

pub(super) fn default_options(request: &CardKitRequest) -> CardPrintOptions {
    options::defaults(request)
}

/// All trim and mark placement uses integral micrometres, without cumulative float rounding.
struct Layout {
    sheet_width: i32,
    sheet_height: i32,
    width: i32,
    height: i32,
    gap: i32,
    offset: i32,
    length: i32,
    origin_x: i32,
    origin_y: i32,
    columns: usize,
    rows: usize,
    capacity: usize,
    page_count: usize,
}

impl Layout {
    fn new(options: &CardPrintOptions, copies: usize) -> Result<Self, CardKitError> {
        if !(1..=MAX_COPIES).contains(&copies) {
            return Err(print_invalid(
                "card_print.quantity_limit",
                "print requires 1 to 4096 total copies",
            ));
        }
        let (sheet_width, sheet_height) = match options.paper {
            CardPrintPaper::A4 => (210_000, 297_000),
            CardPrintPaper::Letter => (215_900, 279_400),
        };
        let width = micrometres(options.card_width_mm, 10.0, 300.0)?;
        let height = micrometres(options.card_height_mm, 10.0, 300.0)?;
        let margin = micrometres(options.margin_mm, 0.0, 50.0)?;
        let gap = micrometres(options.gap_mm, 0.0, 30.0)?;
        let offset = micrometres(options.crop_mark_offset_mm, 0.0, 10.0)?;
        let length = micrometres(options.crop_mark_length_mm, 0.1, 10.0)?;
        let stroke = micrometres(options.stroke_width_mm, 0.05, 0.5)?;
        let half_stroke = (stroke + 1) / 2;
        let extent = offset + length + half_stroke;
        if offset < half_stroke || gap < extent {
            return Err(print_invalid(
                "card_print.crop_clearance",
                "crop offset must clear half the stroke; gap must contain offset, mark length and half the stroke",
            ));
        }
        let columns = (sheet_width - 2 * margin - 2 * extent + gap) / (width + gap);
        let rows = (sheet_height - 2 * margin - 2 * extent + gap) / (height + gap);
        if columns < 1 || rows < 1 {
            return Err(print_invalid(
                "card_print.paper_fit",
                "card and complete crop marks do not fit inside the paper margins",
            ));
        }
        let capacity = usize::try_from(columns * rows).expect("bounded positive dimensions");
        let page_count = copies.div_ceil(capacity);
        if capacity > MAX_SLOTS_PER_PAGE {
            return Err(print_invalid(
                "card_print.slot_limit",
                "print exceeds 512 slots per page",
            ));
        }
        if page_count > MAX_PAGES {
            return Err(print_invalid(
                "card_print.page_limit",
                "print exceeds 256 pages",
            ));
        }
        let origin_x = (sheet_width - (columns * width + (columns - 1) * gap)) / 2;
        let origin_y = (sheet_height - (rows * height + (rows - 1) * gap)) / 2;
        Ok(Self {
            sheet_width,
            sheet_height,
            width,
            height,
            gap,
            offset,
            length,
            origin_x,
            origin_y,
            columns: usize::try_from(columns).expect("positive bounded columns"),
            rows: usize::try_from(rows).expect("positive bounded rows"),
            capacity,
            page_count,
        })
    }

    fn slot(
        &self,
        index: usize,
        card_id: &str,
        copy_index: u16,
        canvas: (f64, f64),
    ) -> (CardPrintSlot, Vec<CardPrintLine>) {
        let column = i32::try_from(index % self.columns).expect("admitted column count");
        let row = i32::try_from(index / self.columns).expect("admitted row count");
        let x = self.origin_x + column * (self.width + self.gap);
        let y = self.origin_y + row * (self.height + self.gap);
        let rect_mm = CardPrintRect {
            x: mm(x),
            y: mm(y),
            width: mm(self.width),
            height: mm(self.height),
        };
        let scale = (rect_mm.width / canvas.0).min(rect_mm.height / canvas.1);
        let content_rect_mm = CardPrintRect {
            x: rect_mm.x + (rect_mm.width - canvas.0 * scale) / 2.0,
            y: rect_mm.y + (rect_mm.height - canvas.1 * scale) / 2.0,
            width: canvas.0 * scale,
            height: canvas.1 * scale,
        };
        let mut marks = Vec::with_capacity(8);
        for (corner_x, direction_x) in [(x, -1), (x + self.width, 1)] {
            for (corner_y, direction_y) in [(y, -1), (y + self.height, 1)] {
                marks.push(CardPrintLine {
                    x1: mm(corner_x + direction_x * (self.offset + self.length)),
                    y1: mm(corner_y),
                    x2: mm(corner_x + direction_x * self.offset),
                    y2: mm(corner_y),
                });
                marks.push(CardPrintLine {
                    x1: mm(corner_x),
                    y1: mm(corner_y + direction_y * (self.offset + self.length)),
                    x2: mm(corner_x),
                    y2: mm(corner_y + direction_y * self.offset),
                });
            }
        }
        (
            CardPrintSlot {
                card_id: card_id.into(),
                copy_index,
                rect_mm,
                content_rect_mm,
            },
            marks,
        )
    }
}

fn micrometres(value: f64, min: f64, max: f64) -> Result<i32, CardKitError> {
    let scaled = value * 1000.0;
    if !value.is_finite()
        || !(min..=max).contains(&value)
        || (scaled - scaled.round()).abs() > 0.000_001
    {
        return Err(print_invalid(
            "card_print.option_range",
            "print millimetres are out of range or exceed 0.001 mm precision",
        ));
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "Finite bounded input was checked to be an integral micrometre"
    )]
    let integral = scaled.round() as i32;
    Ok(integral)
}

fn mm(value: i32) -> f64 {
    f64::from(value) / 1000.0
}

#[cfg(test)]
mod tests;
