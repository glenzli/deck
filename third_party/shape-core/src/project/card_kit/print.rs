//! One bounded physical home-print plan; adapters paint these exact millimetre coordinates.
//! Print settings never enter an accepted composition or create a new artifact revision.
use serde::{Deserialize, Serialize};
use shape_domain::{ArtifactContentContract, ArtifactRevision, CardKitRequest, WorkingInput};
use std::collections::BTreeMap;

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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardPrintCutLineStyle {
    #[default]
    CornerMarks,
    DashedGrid,
}

/// Transient export settings, in millimetres with at most three decimal places.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CardPrintOptions {
    pub paper: CardPrintPaper,
    pub cut_line_style: CardPrintCutLineStyle,
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
            cut_line_style: CardPrintCutLineStyle::CornerMarks,
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

impl CardPrintPlan {
    /// Rearranges admitted faces using the same physical grid and cut-line owner as automatic plans.
    /// No card is rendered again, and the accepted source and authored quantities remain unchanged.
    /// # Errors
    /// Rejects unknown or duplicate copy identities and grids outside the paper or job bounds.
    pub fn impose_grid(
        &mut self,
        columns: usize,
        rows: usize,
        copies: &[(String, u16)],
    ) -> Result<(), CardKitError> {
        let layout = Layout::with_grid(&self.options, copies.len(), Some((columns, rows)))?;
        let mut identities = std::collections::BTreeSet::new();
        for (id, index) in copies {
            if *index == 0
                || !self.cards.iter().any(|card| card.id == *id)
                || !identities.insert((id, index))
            {
                return Err(print_invalid(
                    "card_print.selection",
                    "select unique admitted card copies",
                ));
            }
        }
        let template = self
            .pages
            .first()
            .and_then(|page| page.slots.first())
            .ok_or_else(|| {
                print_invalid(
                    "card_print.layout",
                    "print layout is missing an admitted card face",
                )
            })?;
        let canvas = (
            template.content_rect_mm.width,
            template.content_rect_mm.height,
        );
        self.pages = layout.pages(copies, canvas);
        self.cards
            .retain(|card| copies.iter().any(|(id, _)| card.id == *id));
        self.columns = layout.columns;
        self.rows = layout.rows;
        self.slots_per_page = layout.capacity;
        self.total_copies = copies.len();
        self.page_count = self.pages.len();
        Ok(())
    }
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
        self.plan_card_print_composition(source, request, options, None)
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
        self.plan_card_print_composition(source, request, &resolved, None)
    }

    /// Plans transient copy counts for selected faces of an exact accepted kit.
    /// Unselected materials and faces are not loaded or rendered; authored quantities stay intact.
    /// # Errors
    /// Rejects empty, duplicate or unknown card selections and the same bounds as a complete job.
    pub fn plan_card_print_selection_request(
        &self,
        source: WorkingInput,
        options: &CardPrintRequestOptions,
        selection: &[(String, u16)],
    ) -> Result<CardPrintPlan, CardKitError> {
        let mut request = self.read_card_kit(source)?;
        let mut quantities = BTreeMap::new();
        for (id, quantity) in selection {
            if *quantity == 0 || quantities.insert(id.as_str(), *quantity).is_some() {
                return Err(print_invalid(
                    "card_print.selection",
                    "select unique accepted cards with positive copy counts",
                ));
            }
        }
        request
            .cards
            .retain(|card| quantities.contains_key(card.id.as_str()));
        if selection.is_empty() || request.cards.len() != selection.len() {
            return Err(print_invalid(
                "card_print.selection",
                "select cards from this accepted version",
            ));
        }
        let resolved = options.resolve(&request)?;
        self.plan_card_print_composition(source, request, &resolved, Some(&quantities))
    }

    fn plan_card_print_composition(
        &self,
        source: WorkingInput,
        request: CardKitRequest,
        options: &CardPrintOptions,
        quantities: Option<&BTreeMap<&str, u16>>,
    ) -> Result<CardPrintPlan, CardKitError> {
        let quantity = |id: &str, authored| {
            quantities
                .and_then(|counts| counts.get(id))
                .copied()
                .unwrap_or(authored)
        };
        let total_copies = request
            .cards
            .iter()
            .map(|c| usize::from(quantity(&c.id, c.quantity)))
            .sum();
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
        let mut copies = Vec::with_capacity(total_copies);
        for card in &request.cards {
            for copy_index in 1..=quantity(&card.id, card.quantity) {
                copies.push((card.id.clone(), copy_index));
            }
        }
        let pages = layout.pages(&copies, canvas);
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
    cut_line_style: CardPrintCutLineStyle,
}

impl Layout {
    fn new(options: &CardPrintOptions, copies: usize) -> Result<Self, CardKitError> {
        Self::with_grid(options, copies, None)
    }

    fn with_grid(
        options: &CardPrintOptions,
        copies: usize,
        grid: Option<(usize, usize)>,
    ) -> Result<Self, CardKitError> {
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
        let extent = cut_line_extent(options.cut_line_style, gap, offset, length, stroke)?;
        let available_columns = (sheet_width - 2 * margin - 2 * extent + gap) / (width + gap);
        let available_rows = (sheet_height - 2 * margin - 2 * extent + gap) / (height + gap);
        let (columns, rows) = if let Some((columns, rows)) = grid {
            if columns == 0 || rows == 0 || columns.saturating_mul(rows) > MAX_SLOTS_PER_PAGE {
                return Err(print_invalid(
                    "card_print.slot_limit",
                    "choose a bounded positive print grid",
                ));
            }
            let columns = i32::try_from(columns).expect("bounded print grid");
            let rows = i32::try_from(rows).expect("bounded print grid");
            if columns > available_columns || rows > available_rows {
                return Err(print_invalid(
                    "card_print.paper_fit",
                    "selected grid and complete cut lines do not fit inside the paper margins",
                ));
            }
            (columns, rows)
        } else {
            (available_columns, available_rows)
        };
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
            cut_line_style: options.cut_line_style,
        })
    }

    fn pages(&self, copies: &[(String, u16)], canvas: (f64, f64)) -> Vec<CardPrintPage> {
        copies
            .chunks(self.capacity)
            .enumerate()
            .map(|(index, copies)| {
                let mut page = CardPrintPage {
                    index,
                    slots: Vec::with_capacity(copies.len()),
                    crop_marks: Vec::new(),
                };
                for (index, (id, copy_index)) in copies.iter().enumerate() {
                    let (slot, marks) = self.slot(index, id, *copy_index, canvas);
                    page.slots.push(slot);
                    page.crop_marks.extend(marks);
                }
                if self.cut_line_style == CardPrintCutLineStyle::DashedGrid {
                    page.crop_marks = self.shared_cut_lines(page.slots.len());
                }
                page
            })
            .collect()
    }

    fn shared_cut_lines(&self, occupied: usize) -> Vec<CardPrintLine> {
        // Doubled micrometres preserve half-gap centres for odd micrometre measurements.
        let mut vertical = BTreeMap::<i32, Vec<(i32, i32)>>::new();
        let mut horizontal = BTreeMap::<i32, Vec<(i32, i32)>>::new();
        for index in 0..occupied {
            let column = i32::try_from(index % self.columns).expect("admitted column");
            let row = i32::try_from(index / self.columns).expect("admitted row");
            let x = self.origin_x + column * (self.width + self.gap);
            let y = self.origin_y + row * (self.height + self.gap);
            let left = 2 * x - self.gap;
            let right = 2 * (x + self.width) + self.gap;
            let top = 2 * y - self.gap;
            let bottom = 2 * (y + self.height) + self.gap;
            vertical.entry(left).or_default().push((top, bottom));
            vertical.entry(right).or_default().push((top, bottom));
            horizontal.entry(top).or_default().push((left, right));
            horizontal.entry(bottom).or_default().push((left, right));
        }
        let mut lines = Vec::new();
        for (vertical, groups) in [(true, vertical), (false, horizontal)] {
            for (coordinate, mut intervals) in groups {
                intervals.sort_unstable();
                let mut merged = Vec::<(i32, i32)>::new();
                for (start, end) in intervals {
                    if let Some(last) = merged.last_mut().filter(|last| start <= last.1) {
                        last.1 = last.1.max(end);
                    } else {
                        merged.push((start, end));
                    }
                }
                for (start, end) in merged {
                    let coordinate = f64::from(coordinate) / 2000.0;
                    let start = f64::from(start) / 2000.0;
                    let end = f64::from(end) / 2000.0;
                    lines.push(if vertical {
                        CardPrintLine {
                            x1: coordinate,
                            y1: start,
                            x2: coordinate,
                            y2: end,
                        }
                    } else {
                        CardPrintLine {
                            x1: start,
                            y1: coordinate,
                            x2: end,
                            y2: coordinate,
                        }
                    });
                }
            }
        }
        lines
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
        if self.cut_line_style == CardPrintCutLineStyle::CornerMarks {
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

fn cut_line_extent(
    style: CardPrintCutLineStyle,
    gap: i32,
    offset: i32,
    length: i32,
    stroke: i32,
) -> Result<i32, CardKitError> {
    let half_stroke = (stroke + 1) / 2;
    match style {
        CardPrintCutLineStyle::CornerMarks => {
            let extent = offset + length + half_stroke;
            if offset < half_stroke || gap < extent {
                return Err(print_invalid(
                    "card_print.crop_clearance",
                    "crop offset must clear half the stroke; gap must contain offset, mark length and half the stroke",
                ));
            }
            Ok(extent)
        }
        CardPrintCutLineStyle::DashedGrid => {
            if gap < stroke {
                return Err(print_invalid(
                    "card_print.cut_clearance",
                    "the card gap must contain the complete shared cut-line stroke",
                ));
            }
            Ok((gap + stroke + 1) / 2)
        }
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
