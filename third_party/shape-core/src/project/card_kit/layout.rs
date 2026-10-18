//! Responsive card-face presets. Historical kits keep the separate, frozen legacy renderer.
use std::fmt::Write as _;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Serialize;
use shape_domain::{
    CardImageFit, CardImagePosition, CardKitCard, CardKitRequest, CardLayoutMode, CardTextAlign,
    CardTextVerticalAlign,
};

use super::{
    CardKitError, Material,
    preview::{RenderedCard, escape, flex_line_width, rules, wrap_flex_to_width},
};

// The current native PDF writer is 300 dpi, card axes are <=300 mm, and canvas axes >=240.
// Its maximum device scale is <15; reserve 16 against Qt's 64-bit (2^23-1) mapped-axis bound.
// This is a recoverable rendering limit, never a constraint on drafts or accepted source pins.
const MAX_COVER_AXIS: f64 = 524_287.0;

#[derive(Debug, Clone, Serialize)]
pub(super) struct ResolvedLayout {
    pub mode: CardLayoutMode,
    pub legacy: bool,
    pub image_fit: CardImageFit,
    pub background_color: String,
    pub text_color: String,
    pub border: bool,
    pub padding_percent: u8,
    pub text_align: CardTextAlign,
    pub text_scale_percent: u16,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub image_position: CardImagePosition,
    pub image_share_percent: u8,
    pub text_vertical_align: CardTextVerticalAlign,
    pub text_offset_x_percent: i8,
    pub text_offset_y_percent: i8,
}

#[derive(Default, Serialize)]
pub(super) struct HiddenContent {
    pub artwork: bool,
    pub text_fields: Vec<&'static str>,
}

pub(super) fn text_fields(card: &CardKitCard, materials: &[Material]) -> Vec<&'static str> {
    [
        ("title", card.title.as_str()),
        ("card_type", &card.card_type),
        ("cost", &card.cost),
        (
            if card.text_material.is_some() {
                "text_material"
            } else {
                "rules"
            },
            rules(card, materials),
        ),
        ("stats", &card.stats),
        ("flavor", &card.flavor),
    ]
    .into_iter()
    .filter_map(|(name, value)| (!value.trim().is_empty()).then_some(name))
    .collect()
}

pub(super) fn resolve(
    request: &CardKitRequest,
    card: &CardKitCard,
    materials: &[Material],
    use_card_override: bool,
) -> ResolvedLayout {
    let Some(root) = &request.layout else {
        return ResolvedLayout {
            mode: CardLayoutMode::Classic,
            legacy: true,
            image_fit: CardImageFit::Contain,
            background_color: "#f7f2e8".into(),
            text_color: "#2c2721".into(),
            border: true,
            padding_percent: 0,
            text_align: CardTextAlign::Left,
            text_scale_percent: 100,
            canvas_width: 750,
            canvas_height: 1050,
            image_position: CardImagePosition::Auto,
            image_share_percent: 56,
            text_vertical_align: CardTextVerticalAlign::Auto,
            text_offset_x_percent: 0,
            text_offset_y_percent: 0,
        };
    };
    let config = root.with_overrides(use_card_override.then_some(card.layout.as_ref()).flatten());
    let has_text = !text_fields(card, materials).is_empty();
    let mode = match config.mode.unwrap_or(CardLayoutMode::Auto) {
        CardLayoutMode::Auto => match (card.artwork.is_some(), has_text) {
            (true, true) => CardLayoutMode::ImageText,
            (true, false) => CardLayoutMode::Image,
            (false, true) => CardLayoutMode::Text,
            (false, false) => CardLayoutMode::Auto,
        },
        explicit => explicit,
    };
    ResolvedLayout {
        mode,
        legacy: false,
        image_fit: config.image_fit.unwrap_or(CardImageFit::Contain),
        background_color: config.background_color.unwrap_or_else(|| "#ffffff".into()),
        text_color: config.text_color.unwrap_or_else(|| "#202020".into()),
        border: config.border.unwrap_or(false),
        padding_percent: config
            .padding_percent
            .unwrap_or(if mode == CardLayoutMode::Image { 0 } else { 4 }),
        text_align: config
            .text_align
            .unwrap_or(if mode == CardLayoutMode::Text {
                CardTextAlign::Center
            } else {
                CardTextAlign::Left
            }),
        text_scale_percent: config.text_scale_percent.unwrap_or(100),
        canvas_width: request.canvas_width,
        canvas_height: request.canvas_height,
        image_position: config.image_position.unwrap_or(CardImagePosition::Auto),
        image_share_percent: config.image_share_percent.unwrap_or(
            if mode == CardLayoutMode::Classic {
                56
            } else {
                50
            },
        ),
        text_vertical_align: config
            .text_vertical_align
            .unwrap_or(CardTextVerticalAlign::Auto),
        text_offset_x_percent: config.text_offset_x_percent.unwrap_or(0),
        text_offset_y_percent: config.text_offset_y_percent.unwrap_or(0),
    }
}

pub(super) fn hidden(
    resolved: &ResolvedLayout,
    card: &CardKitCard,
    materials: &[Material],
) -> HiddenContent {
    HiddenContent {
        artwork: resolved.mode == CardLayoutMode::Text && card.artwork.is_some(),
        text_fields: if resolved.mode == CardLayoutMode::Image {
            text_fields(card, materials)
        } else {
            Vec::new()
        },
    }
}

pub(super) fn missing(
    resolved: &ResolvedLayout,
    card: &CardKitCard,
    materials: &[Material],
) -> Vec<&'static str> {
    let has_text = !text_fields(card, materials).is_empty();
    if resolved.legacy {
        let mut issues = Vec::new();
        if card.artwork.is_none() {
            issues.push("missing_artwork");
        }
        if card.title.trim().is_empty() {
            issues.push("missing_title");
        }
        if rules(card, materials).trim().is_empty() {
            issues.push("missing_body");
        }
        return issues;
    }
    let mut issues = match resolved.mode {
        CardLayoutMode::Auto => vec!["empty_content"],
        CardLayoutMode::Image if card.artwork.is_none() => vec!["missing_artwork"],
        CardLayoutMode::Text if !has_text => vec!["missing_text"],
        CardLayoutMode::ImageText => {
            let mut issues = Vec::new();
            if card.artwork.is_none() {
                issues.push("missing_artwork");
            }
            if !has_text {
                issues.push("missing_text");
            }
            issues
        }
        CardLayoutMode::Classic if card.artwork.is_none() && !has_text => vec!["empty_content"],
        CardLayoutMode::Classic
            if card.artwork.is_none() && resolved.image_position != CardImagePosition::Auto =>
        {
            vec!["missing_artwork"]
        }
        _ => Vec::new(),
    };
    if crop_limited(resolved, card, materials) {
        issues.push("image_crop_limit");
    }
    issues
}

/// Whether this face embeds artwork. Authoring pins hidden by a mode remain in its document.
pub(super) fn shows_artwork(request: &CardKitRequest, card: &CardKitCard) -> bool {
    request.layout.as_ref().is_none_or(|root| {
        root.with_overrides(card.layout.as_ref()).mode != Some(CardLayoutMode::Text)
    })
}

#[derive(Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

struct TextBlock {
    lines: Vec<String>,
    font: f64,
    spacing: f64,
    italic: bool,
}

struct FacePlan {
    resolved: ResolvedLayout,
    image: Option<Rect>,
    text: Rect,
    blocks: Vec<TextBlock>,
    text_height: f64,
    overflow: bool,
    crop_limited: bool,
}

pub(super) fn overflow(
    request: &CardKitRequest,
    card: &CardKitCard,
    materials: &[Material],
) -> bool {
    plan(request, card, materials, None).overflow
}

fn plan(
    request: &CardKitRequest,
    card: &CardKitCard,
    materials: &[Material],
    physical_scale: Option<f64>,
) -> FacePlan {
    let resolved = resolve(request, card, materials, true);
    let width = f64::from(request.canvas_width);
    let height = f64::from(request.canvas_height);
    let short = width.min(height);
    let has_text =
        !text_fields(card, materials).is_empty() && resolved.mode != CardLayoutMode::Image;
    let (image, text) = content_regions(&resolved, card, materials);
    // The ordinary preview uses the same readable policy as the default physical print size.
    // Explicit print dimensions may raise the font floor; neither path shrinks overflowing copy.
    let physical_scale = physical_scale.unwrap_or_else(|| {
        let defaults = super::print::default_options(request);
        (defaults.card_width_mm / width).min(defaults.card_height_mm / height)
    });
    let scale = f64::from(resolved.text_scale_percent) / 100.0;
    let prominent = resolved.mode == CardLayoutMode::Text;
    let metadata = [&card.card_type, &card.cost, &card.stats]
        .into_iter()
        .filter(|v| !v.trim().is_empty())
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" · ");
    let mut overflow = false;
    let mut blocks = Vec::new();
    for (value, proportion, points, italic) in [
        (
            card.title.as_str(),
            if prominent { 0.12 } else { 0.06 },
            9.0,
            false,
        ),
        (
            metadata.as_str(),
            if prominent { 0.055 } else { 0.034 },
            7.0,
            false,
        ),
        (
            rules(card, materials),
            if prominent { 0.07 } else { 0.04 },
            7.0,
            false,
        ),
        (
            card.flavor.as_str(),
            if prominent { 0.055 } else { 0.034 },
            6.0,
            true,
        ),
    ] {
        if !has_text || value.trim().is_empty() {
            continue;
        }
        let font =
            (short * proportion * scale).max((points * 25.4 / (72.0 * physical_scale)).ceil());
        let wrapped = wrap_flex_to_width(value, font, text.width);
        overflow |= wrapped.overflow;
        blocks.push(TextBlock {
            lines: wrapped.lines,
            font,
            spacing: font * 1.25,
            italic,
        });
    }
    let text_height = blocks
        .iter()
        .map(|b| {
            f64::from(u32::try_from(b.lines.len()).expect("bounded authored text")) * b.spacing
                + b.font * 0.35
        })
        .sum::<f64>()
        - blocks.last().map_or(0.0, |b| b.font * 0.35);
    overflow |= text_height > text.height + 0.001;
    let mut plan = FacePlan {
        crop_limited: crop_limited(&resolved, card, materials),
        resolved,
        image,
        text,
        blocks,
        text_height,
        overflow,
    };
    if plan.resolved.text_offset_x_percent != 0
        || plan.resolved.text_offset_y_percent != 0
        || plan.resolved.text_vertical_align != CardTextVerticalAlign::Auto
    {
        let (x, y) = text_origin(&plan);
        plan.overflow |= y < 0.0 || y + plan.text_height > height + 0.001;
        for block in &plan.blocks {
            for line in &block.lines {
                let length = flex_line_width(line, block.font);
                let left = match plan.resolved.text_align {
                    CardTextAlign::Left => x,
                    CardTextAlign::Center => x - length / 2.0,
                    CardTextAlign::Right => x - length,
                };
                plan.overflow |= left < -0.001 || left + length > width + 0.001;
            }
        }
    }
    plan
}

fn content_regions(
    resolved: &ResolvedLayout,
    card: &CardKitCard,
    materials: &[Material],
) -> (Option<Rect>, Rect) {
    let width = f64::from(resolved.canvas_width);
    let height = f64::from(resolved.canvas_height);
    let short = width.min(height);
    let padding = short * f64::from(resolved.padding_percent) / 100.0;
    let inner = Rect {
        x: padding,
        y: padding,
        width: width - 2.0 * padding,
        height: height - 2.0 * padding,
    };
    let has_image = (card.artwork.is_some()
        || matches!(
            resolved.mode,
            CardLayoutMode::Image | CardLayoutMode::ImageText
        )
        || resolved.image_position != CardImagePosition::Auto)
        && resolved.mode != CardLayoutMode::Text;
    let has_text = (!text_fields(card, materials).is_empty()
        || resolved.mode == CardLayoutMode::ImageText
        || (resolved.mode == CardLayoutMode::Classic
            && resolved.image_position != CardImagePosition::Auto))
        && resolved.mode != CardLayoutMode::Image;
    regions(inner, has_image, has_text, resolved, short * 0.035)
}

fn crop_limited(resolved: &ResolvedLayout, card: &CardKitCard, materials: &[Material]) -> bool {
    if resolved.legacy || resolved.image_fit != CardImageFit::Cover {
        return false;
    }
    let (Some(area), Some(dimensions)) = (
        content_regions(resolved, card, materials).0,
        card.artwork
            .and_then(|pin| materials.iter().find(|m| m.source == pin))
            .and_then(|m| m.image_dimensions),
    ) else {
        return false;
    };
    let bounds = fitted_image(area, dimensions, CardImageFit::Cover);
    bounds.width > MAX_COVER_AXIS || bounds.height > MAX_COVER_AXIS
}

fn regions(
    inner: Rect,
    image: bool,
    text: bool,
    resolved: &ResolvedLayout,
    gap: f64,
) -> (Option<Rect>, Rect) {
    if !image {
        return (None, inner);
    }
    if !text {
        return (Some(inner), inner);
    }
    // The classic preset reserves more room for artwork; all presets use the real canvas axes.
    let share = f64::from(resolved.image_share_percent) / 100.0;
    let position = resolved.image_position;
    if position == CardImagePosition::Background {
        return (
            Some(inner),
            Rect {
                y: inner.y + inner.height * share,
                height: inner.height * (1.0 - share),
                ..inner
            },
        );
    }
    if matches!(position, CardImagePosition::Left | CardImagePosition::Right)
        || (position == CardImagePosition::Auto && inner.width > inner.height * 1.15)
    {
        let image_width = (inner.width - gap) * share;
        (
            Some(Rect {
                width: image_width,
                x: if position == CardImagePosition::Right {
                    inner.x + inner.width - image_width
                } else {
                    inner.x
                },
                ..inner
            }),
            Rect {
                x: if position == CardImagePosition::Right {
                    inner.x
                } else {
                    inner.x + image_width + gap
                },
                width: inner.width - image_width - gap,
                ..inner
            },
        )
    } else {
        let image_height = (inner.height - gap) * share;
        (
            Some(Rect {
                height: image_height,
                y: if position == CardImagePosition::Bottom {
                    inner.y + inner.height - image_height
                } else {
                    inner.y
                },
                ..inner
            }),
            Rect {
                y: if position == CardImagePosition::Bottom {
                    inner.y
                } else {
                    inner.y + image_height + gap
                },
                height: inner.height - image_height - gap,
                ..inner
            },
        )
    }
}

pub(super) fn render(
    request: &CardKitRequest,
    card: &CardKitCard,
    materials: &[Material],
    physical_scale: Option<f64>,
) -> Result<RenderedCard, CardKitError> {
    let plan = plan(request, card, materials, physical_scale);
    if physical_scale.is_some() && plan.crop_limited {
        return Err(CardKitError::ImageCropLimit {
            card_id: card.id.clone(),
        });
    }
    if physical_scale.is_some() && plan.overflow {
        return Err(CardKitError::PrintTextOverflow {
            card_id: card.id.clone(),
        });
    }
    let resolved = &plan.resolved;
    let width = resolved.canvas_width;
    let height = resolved.canvas_height;
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{width}" height="{height}" viewBox="0 0 {width} {height}"><desc>{}</desc><rect width="{width}" height="{height}" fill="{}"/>"#,
        escape(&format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            card.title,
            card.card_type,
            card.cost,
            rules(card, materials),
            card.stats,
            card.flavor
        )),
        resolved.background_color
    );
    if !plan.crop_limited
        && let (Some(area), Some(material)) = (
            plan.image,
            card.artwork
                .and_then(|pin| materials.iter().find(|m| m.source == pin)),
        )
    {
        render_image(&mut svg, area, material, resolved.image_fit);
    } else if physical_scale.is_none()
        && !plan.crop_limited
        && let Some(area) = plan.image
    {
        render_placeholder(&mut svg, area);
    }
    render_text(&mut svg, &plan);
    if resolved.border {
        let stroke = f64::from(width.min(height)) / 250.0;
        write!(svg, r#"<rect x="{:.3}" y="{:.3}" width="{:.3}" height="{:.3}" fill="none" stroke="{}" stroke-width="{stroke:.3}"/>"#, stroke/2.0, stroke/2.0, f64::from(width)-stroke, f64::from(height)-stroke, resolved.text_color).expect("String writes are infallible");
    }
    svg.push_str("</svg>");
    Ok(RenderedCard {
        svg,
        overflow: plan.overflow,
    })
}

fn render_placeholder(svg: &mut String, area: Rect) {
    let icon = (area.width.min(area.height) * 0.22).clamp(24.0, 72.0);
    let x = area.x + (area.width - icon) / 2.0;
    let y = area.y + (area.height - icon) / 2.0;
    write!(svg, r##"<g id="image-placeholder"><rect x="{:.3}" y="{:.3}" width="{:.3}" height="{:.3}" rx="3" fill="#f0f0f0" stroke="#b5b5b5" stroke-width="2" stroke-dasharray="7 5"/><g transform="translate({x:.3} {y:.3}) scale({:.5})" fill="none" stroke="#929292" stroke-width="1.4" stroke-linejoin="round"><rect x="2" y="2" width="20" height="20" rx="2"/><circle cx="8" cy="8" r="2"/><path d="M3 19 10 12 14 16 17 12 22 17"/></g></g>"##, area.x, area.y, area.width, area.height, icon/24.0).expect("String writes are infallible");
}

fn render_image(svg: &mut String, area: Rect, material: &Material, fit: CardImageFit) {
    let dimensions = material
        .image_dimensions
        .expect("validated raster dimensions");
    let Rect {
        x,
        y,
        width,
        height,
    } = fitted_image(area, dimensions, fit);
    if fit == CardImageFit::Cover {
        // Qt clips a symbol's viewport with QPainter before drawing its single image child.
        // A mask instead allocates buffers for the entire expanded image before clipping.
        // Keep a local zero-origin viewBox: Qt's symbol transform does not apply viewBox x/y.
        let local_x = (area.width - width) / 2.0;
        let local_y = (area.height - height) / 2.0;
        write!(svg, r##"<defs><symbol id="art-viewport" x="0" y="0" width="{:.3}" height="{:.3}" viewBox="0 0 {:.3} {:.3}" overflow="hidden"><image x="{local_x:.3}" y="{local_y:.3}" width="{width:.3}" height="{height:.3}" xlink:href="data:image/png;base64,{}"/></symbol></defs><use x="{:.3}" y="{:.3}" width="{:.3}" height="{:.3}" xlink:href="#art-viewport"/>"##, area.width, area.height, area.width, area.height, STANDARD.encode(&material.bytes), area.x, area.y, area.width, area.height).expect("String writes are infallible");
    } else {
        // The explicit rectangle already fits the area and preserves aspect ratio in Qt.
        write!(svg, r#"<defs><clipPath id="art"><rect x="{:.3}" y="{:.3}" width="{:.3}" height="{:.3}"/></clipPath></defs><image x="{x:.3}" y="{y:.3}" width="{width:.3}" height="{height:.3}" clip-path="url(#art)" xlink:href="data:image/png;base64,{}"/>"#, area.x, area.y, area.width, area.height, STANDARD.encode(&material.bytes)).expect("String writes are infallible");
    }
}

fn fitted_image(area: Rect, dimensions: (u32, u32), fit: CardImageFit) -> Rect {
    let (width, height) = dimensions;
    let scales = (
        area.width / f64::from(width),
        area.height / f64::from(height),
    );
    let scale = if fit == CardImageFit::Cover {
        scales.0.max(scales.1)
    } else {
        scales.0.min(scales.1)
    };
    let width = f64::from(width) * scale;
    let height = f64::from(height) * scale;
    Rect {
        x: area.x + (area.width - width) / 2.0,
        y: area.y + (area.height - height) / 2.0,
        width,
        height,
    }
}

fn text_origin(plan: &FacePlan) -> (f64, f64) {
    let area = plan.text;
    let x = match plan.resolved.text_align {
        CardTextAlign::Left => area.x,
        CardTextAlign::Center => area.x + area.width / 2.0,
        CardTextAlign::Right => area.x + area.width,
    };
    let mut y = area.y;
    match plan.resolved.text_vertical_align {
        CardTextVerticalAlign::Center => y += (area.height - plan.text_height).max(0.0) / 2.0,
        CardTextVerticalAlign::Bottom => y += (area.height - plan.text_height).max(0.0),
        CardTextVerticalAlign::Auto
            if plan.resolved.mode == CardLayoutMode::Text
                && (!plan.overflow
                    || plan.resolved.text_offset_x_percent != 0
                    || plan.resolved.text_offset_y_percent != 0) =>
        {
            y += (area.height - plan.text_height).max(0.0) / 2.0;
        }
        _ => {}
    }
    (
        x + f64::from(plan.resolved.text_offset_x_percent) * f64::from(plan.resolved.canvas_width)
            / 100.0,
        y + f64::from(plan.resolved.text_offset_y_percent) * f64::from(plan.resolved.canvas_height)
            / 100.0,
    )
}

fn render_text(svg: &mut String, plan: &FacePlan) {
    let area = plan.text;
    let custom = plan.resolved.text_offset_x_percent != 0
        || plan.resolved.text_offset_y_percent != 0
        || plan.resolved.text_vertical_align != CardTextVerticalAlign::Auto;
    let clip = if custom {
        Rect {
            x: 0.0,
            y: 0.0,
            width: f64::from(plan.resolved.canvas_width),
            height: f64::from(plan.resolved.canvas_height),
        }
    } else {
        area
    };
    let (x, mut y) = text_origin(plan);
    let anchor = match plan.resolved.text_align {
        CardTextAlign::Left => "start",
        CardTextAlign::Center => "middle",
        CardTextAlign::Right => "end",
    };
    if plan.resolved.image_position == CardImagePosition::Background && plan.image.is_some() {
        write!(svg, r#"<rect x="{:.3}" y="{:.3}" width="{:.3}" height="{:.3}" fill="{}" fill-opacity="0.94"/>"#,area.x,area.y,area.width,area.height,plan.resolved.background_color).expect("String writes are infallible");
    }
    write!(svg, r#"<defs><clipPath id="copy"><rect x="{:.3}" y="{:.3}" width="{:.3}" height="{:.3}"/></clipPath></defs><g clip-path="url(#copy)">"#, clip.x,clip.y,clip.width,clip.height).expect("String writes are infallible");
    for block in &plan.blocks {
        for line in &block.lines {
            if y >= clip.y && y + block.font <= clip.y + clip.height + 0.001 {
                write!(svg, r#"<text x="{x:.3}" y="{:.3}" text-anchor="{anchor}" font-family="sans-serif" font-size="{:.3}" fill="{}"{}>{}</text>"#, y+block.font, block.font, plan.resolved.text_color, if block.italic {" font-style=\"italic\""} else {""}, escape(line)).expect("String writes are infallible");
            }
            y += block.spacing;
        }
        y += block.font * 0.35;
    }
    svg.push_str("</g>");
}

#[cfg(test)]
mod tests;
