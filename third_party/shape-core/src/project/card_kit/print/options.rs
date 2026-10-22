//! Partial export settings resolve once against the accepted card canvas, before layout admission.
use serde::{Deserialize, Serialize};
use shape_domain::CardKitRequest;

use super::{
    CardKitError, CardPrintCutLineStyle, CardPrintOptions, CardPrintPaper, cut_line_extent,
    micrometres, print_invalid,
};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CardPrintRequestOptions {
    pub paper: CardPrintPaper,
    pub cut_line_style: CardPrintCutLineStyle,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_width_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_height_mm: Option<f64>,
    pub margin_mm: f64,
    pub gap_mm: f64,
    pub crop_mark_offset_mm: f64,
    pub crop_mark_length_mm: f64,
    pub stroke_width_mm: f64,
}

impl Default for CardPrintRequestOptions {
    fn default() -> Self {
        let defaults = CardPrintOptions::default();
        Self {
            paper: defaults.paper,
            cut_line_style: defaults.cut_line_style,
            card_width_mm: None,
            card_height_mm: None,
            margin_mm: defaults.margin_mm,
            gap_mm: defaults.gap_mm,
            crop_mark_offset_mm: defaults.crop_mark_offset_mm,
            crop_mark_length_mm: defaults.crop_mark_length_mm,
            stroke_width_mm: defaults.stroke_width_mm,
        }
    }
}

impl CardPrintRequestOptions {
    /// Returns the admitted automatic grid without loading any card material or rendering a face.
    /// # Errors
    /// Rejects invalid physical settings and paper fits through the same planner as output jobs.
    pub fn grid_dimensions(
        &self,
        request: &CardKitRequest,
    ) -> Result<(usize, usize), CardKitError> {
        let options = self.resolve(request)?;
        let layout = super::Layout::new(&options, 1)?;
        Ok((layout.columns, layout.rows))
    }

    /// Fits proportional card faces in a chosen grid, reserving complete cut lines and paper margins.
    /// # Errors
    /// Rejects invalid grids, physical measurements and card faces below the minimum printable size.
    pub fn fit_grid(
        &mut self,
        request: &CardKitRequest,
        columns: usize,
        rows: usize,
    ) -> Result<(), CardKitError> {
        if columns == 0 || rows == 0 || columns.saturating_mul(rows) > super::MAX_SLOTS_PER_PAGE {
            return Err(print_invalid(
                "card_print.slot_limit",
                "choose a bounded positive print grid",
            ));
        }
        request.validate().map_err(CardKitError::Invalid)?;
        let margin = micrometres(self.margin_mm, 0.0, 50.0)?;
        let gap = micrometres(self.gap_mm, 0.0, 30.0)?;
        let offset = micrometres(self.crop_mark_offset_mm, 0.0, 10.0)?;
        let length = micrometres(self.crop_mark_length_mm, 0.1, 10.0)?;
        let stroke = micrometres(self.stroke_width_mm, 0.05, 0.5)?;
        let extent = cut_line_extent(self.cut_line_style, gap, offset, length, stroke)?;
        let (paper_width, paper_height) = match self.paper {
            CardPrintPaper::A4 => (210_000.0, 297_000.0),
            CardPrintPaper::Letter => (215_900.0, 279_400.0),
        };
        let columns = f64::from(u16::try_from(columns).expect("bounded print grid"));
        let rows = f64::from(u16::try_from(rows).expect("bounded print grid"));
        let max_width =
            (paper_width - 2.0 * f64::from(margin + extent) - (columns - 1.0) * f64::from(gap))
                / columns;
        let max_height =
            (paper_height - 2.0 * f64::from(margin + extent) - (rows - 1.0) * f64::from(gap))
                / rows;
        let (width, height) = canvas(request);
        let scale = (max_width / width).min(max_height / height);
        let width = (width * scale).floor() / 1000.0;
        let height = (height * scale).floor() / 1000.0;
        micrometres(width, 10.0, 300.0)?;
        micrometres(height, 10.0, 300.0)?;
        self.card_width_mm = Some(width);
        self.card_height_mm = Some(height);
        Ok(())
    }

    pub(super) fn resolve(
        &self,
        request: &CardKitRequest,
    ) -> Result<CardPrintOptions, CardKitError> {
        let mut options = CardPrintOptions {
            paper: self.paper,
            cut_line_style: self.cut_line_style,
            card_width_mm: 63.0,
            card_height_mm: 88.0,
            margin_mm: self.margin_mm,
            gap_mm: self.gap_mm,
            crop_mark_offset_mm: self.crop_mark_offset_mm,
            crop_mark_length_mm: self.crop_mark_length_mm,
            stroke_width_mm: self.stroke_width_mm,
        };
        let (width, height) = canvas(request);
        match (self.card_width_mm, self.card_height_mm) {
            (Some(width), Some(height)) => {
                options.card_width_mm = width;
                options.card_height_mm = height;
            }
            (Some(value), None) => {
                micrometres(value, 10.0, 300.0)?;
                options.card_width_mm = value;
                options.card_height_mm = rounded(value * height / width);
            }
            (None, Some(value)) => {
                micrometres(value, 10.0, 300.0)?;
                options.card_height_mm = value;
                options.card_width_mm = rounded(value * width / height);
            }
            (None, None) if request.layout.is_none() => {}
            (None, None) => {
                let margin = f64::from(micrometres(self.margin_mm, 0.0, 50.0)?);
                let offset = micrometres(self.crop_mark_offset_mm, 0.0, 10.0)?;
                let length = micrometres(self.crop_mark_length_mm, 0.1, 10.0)?;
                let stroke = micrometres(self.stroke_width_mm, 0.05, 0.5)?;
                let gap = micrometres(self.gap_mm, 0.0, 30.0)?;
                let extent = f64::from(cut_line_extent(
                    self.cut_line_style,
                    gap,
                    offset,
                    length,
                    stroke,
                )?);
                let (paper_width, paper_height) = match self.paper {
                    CardPrintPaper::A4 => (210_000.0, 297_000.0),
                    CardPrintPaper::Letter => (215_900.0, 279_400.0),
                };
                let max_width = ((paper_width - 2.0 * margin - 2.0 * extent) / 1000.0).min(300.0);
                let max_height = ((paper_height - 2.0 * margin - 2.0 * extent) / 1000.0).min(300.0);
                let ratio = width / height;
                let min_width = 10.0_f64.max(10.0 * ratio);
                let upper_width = max_width.min(max_height * ratio);
                if min_width > upper_width {
                    return Err(print_invalid(
                        "card_print.paper_fit",
                        "card aspect ratio cannot fit minimum card dimensions and complete crop marks inside the paper margins",
                    ));
                }
                let chosen_width = 63.0_f64.clamp(min_width, upper_width);
                options.card_width_mm = rounded(chosen_width).min(max_width);
                options.card_height_mm = rounded(chosen_width / ratio).min(max_height);
            }
        }
        micrometres(options.card_width_mm, 10.0, 300.0)?;
        micrometres(options.card_height_mm, 10.0, 300.0)?;
        Ok(options)
    }
}

fn rounded(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

pub(super) fn canvas(request: &CardKitRequest) -> (f64, f64) {
    if request.layout.is_none() {
        (750.0, 1050.0)
    } else {
        (
            f64::from(request.canvas_width),
            f64::from(request.canvas_height),
        )
    }
}

pub(super) fn defaults(request: &CardKitRequest) -> CardPrintOptions {
    CardPrintRequestOptions::default()
        .resolve(request)
        .expect("validated canvas fits default paper and minimum dimensions")
}

#[cfg(test)]
mod tests;
