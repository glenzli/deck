//! Field-bound layers reuse card wrapping, readable font policy and image viewport primitives.
use super::*;
use shape_domain::{CardBlockField, CardBlockVerticalAlign};

pub(super) struct BlockPlan {
    layers: Vec<Layer>,
    pub overflow: bool,
    pub crop_limited: bool,
}
enum Layer {
    Artwork {
        block: CardBlock,
        area: Rect,
        limited: bool,
    },
    Text {
        block: CardBlock,
        area: Rect,
        lines: Vec<String>,
        font: f64,
        height: f64,
        inset: f64,
    },
}
fn field<'a>(field: CardBlockField, card: &'a CardKitCard, materials: &'a [Material]) -> &'a str {
    match field {
        CardBlockField::Title => &card.title,
        CardBlockField::Rules => rules(card, materials),
        CardBlockField::CardType => &card.card_type,
        CardBlockField::Cost => &card.cost,
        CardBlockField::Stats => &card.stats,
        CardBlockField::Flavor => &card.flavor,
        CardBlockField::Artwork => "",
    }
}
fn list(resolved: &ResolvedLayout) -> &[CardBlock] {
    resolved.blocks.as_deref().unwrap_or(&[])
}
fn area(block: &CardBlock, resolved: &ResolvedLayout) -> Rect {
    Rect {
        x: f64::from(block.x) * f64::from(resolved.canvas_width) / 10000.0,
        y: f64::from(block.y) * f64::from(resolved.canvas_height) / 10000.0,
        width: f64::from(block.width) * f64::from(resolved.canvas_width) / 10000.0,
        height: f64::from(block.height) * f64::from(resolved.canvas_height) / 10000.0,
    }
}
fn image_limited(
    block: &CardBlock,
    area: Rect,
    card: &CardKitCard,
    materials: &[Material],
) -> bool {
    if block.image_fit != CardImageFit::Cover {
        return false;
    }
    card.artwork
        .and_then(|pin| materials.iter().find(|material| material.source == pin))
        .and_then(|material| material.image_dimensions)
        .is_some_and(|dimensions| {
            let fitted = fitted_image(area, dimensions, block.image_fit);
            fitted.width > MAX_COVER_AXIS || fitted.height > MAX_COVER_AXIS
        })
}
pub(super) fn hidden(
    resolved: &ResolvedLayout,
    card: &CardKitCard,
    materials: &[Material],
) -> HiddenContent {
    let bound = |field| list(resolved).iter().any(|block| block.field == field);
    let fields = authored_text_fields(card, materials);
    HiddenContent {
        artwork: card.artwork.is_some() && !bound(CardBlockField::Artwork),
        text_fields: fields
            .into_iter()
            .filter(|name| {
                !bound(match *name {
                    "title" => CardBlockField::Title,
                    "rules" | "text_material" => CardBlockField::Rules,
                    "card_type" => CardBlockField::CardType,
                    "cost" => CardBlockField::Cost,
                    "stats" => CardBlockField::Stats,
                    _ => CardBlockField::Flavor,
                })
            })
            .collect(),
    }
}
pub(super) fn missing(
    resolved: &ResolvedLayout,
    card: &CardKitCard,
    materials: &[Material],
) -> Vec<&'static str> {
    let mut issues = Vec::new();
    let images = list(resolved)
        .iter()
        .filter(|block| block.field == CardBlockField::Artwork)
        .collect::<Vec<_>>();
    if !images.is_empty() && card.artwork.is_none() {
        issues.push("missing_artwork");
    }
    if images
        .iter()
        .any(|block| image_limited(block, area(block, resolved), card, materials))
    {
        issues.push("image_crop_limit");
    }
    if !(card.artwork.is_some() && !images.is_empty())
        && !list(resolved)
            .iter()
            .any(|block| !field(block.field, card, materials).trim().is_empty())
    {
        issues.push("empty_content");
    }
    issues
}
pub(super) fn plan(
    resolved: &ResolvedLayout,
    card: &CardKitCard,
    materials: &[Material],
    physical_scale: f64,
) -> BlockPlan {
    let mut plan = BlockPlan {
        layers: Vec::new(),
        overflow: false,
        crop_limited: false,
    };
    let short = f64::from(resolved.canvas_width.min(resolved.canvas_height));
    for block in list(resolved) {
        let area = area(block, resolved);
        if block.field == CardBlockField::Artwork {
            let limited = image_limited(block, area, card, materials);
            plan.crop_limited |= limited;
            plan.layers.push(Layer::Artwork {
                block: block.clone(),
                area,
                limited,
            });
            continue;
        }
        let value = field(block.field, card, materials);
        if value.trim().is_empty() {
            continue;
        }
        let (proportion, points) = match block.field {
            CardBlockField::Title => (0.06, 9.0),
            CardBlockField::Rules => (0.04, 7.0),
            CardBlockField::Flavor => (0.034, 6.0),
            _ => (0.034, 7.0),
        };
        let font = readable_font(
            short,
            proportion,
            f64::from(block.text_scale_percent) / 100.0,
            points,
            physical_scale,
        );
        let inset = if block.italic { font * 0.18 } else { 0.0 };
        let advance = font * if block.bold { 1.08 } else { 1.0 };
        let wrapped = wrap_flex_to_width(value, advance, (area.width - 2.0 * inset).max(0.0));
        let height =
            f64::from(u32::try_from(wrapped.lines.len()).expect("bounded text")) * font * 1.25;
        plan.overflow |=
            wrapped.overflow || height > area.height + 0.001 || area.width < 2.0 * inset;
        plan.layers.push(Layer::Text {
            block: block.clone(),
            area,
            lines: wrapped.lines,
            font,
            height,
            inset,
        });
    }
    plan
}
pub(super) fn paint(
    svg: &mut String,
    plan: &BlockPlan,
    card: &CardKitCard,
    materials: &[Material],
    preview: bool,
) {
    let material = card
        .artwork
        .and_then(|pin| materials.iter().find(|material| material.source == pin));
    if let Some(material) = material
        && plan
            .layers
            .iter()
            .any(|layer| matches!(layer, Layer::Artwork { limited: false, .. }))
    {
        let (width, height) = material.image_dimensions.expect("validated raster");
        write!(svg,r#"<defs><image id="card-block-image-source" x="0" y="0" width="{width}" height="{height}" xlink:href="data:image/png;base64,{}"/></defs>"#,STANDARD.encode(&material.bytes)).expect("String writes are infallible");
    }
    for (index, layer) in plan.layers.iter().enumerate() {
        let (block, area) = match layer {
            Layer::Artwork { block, area, .. } | Layer::Text { block, area, .. } => (block, *area),
        };
        write!(svg,r#"<defs><clipPath id="card-block-clip-{index}"><rect x="{:.3}" y="{:.3}" width="{:.3}" height="{:.3}"/></clipPath></defs><g id="card-block-layer-{index}" data-block-id="{}" clip-path="url(#card-block-clip-{index})">"#,area.x,area.y,area.width,area.height,block.id).expect("String writes are infallible");
        match layer {
            Layer::Artwork { limited, .. } => {
                if !limited {
                    if let Some(material) = material {
                        render_image_primitive(
                            svg,
                            area,
                            material,
                            block.image_fit,
                            &format!("card-block-art-{index}"),
                            Some("card-block-image-source"),
                        );
                    } else if preview {
                        render_placeholder_named(
                            svg,
                            area,
                            &format!("card-block-placeholder-{index}"),
                        );
                    }
                }
            }
            Layer::Text {
                lines,
                font,
                height,
                inset,
                ..
            } => {
                let (x, anchor) = match block.text_align {
                    CardTextAlign::Left => (area.x + inset, "start"),
                    CardTextAlign::Center => (area.x + area.width / 2.0, "middle"),
                    CardTextAlign::Right => (area.x + area.width - inset, "end"),
                };
                let mut y = area.y
                    + match block.text_vertical_align {
                        CardBlockVerticalAlign::Top => 0.0,
                        CardBlockVerticalAlign::Center => (area.height - height).max(0.0) / 2.0,
                        CardBlockVerticalAlign::Bottom => (area.height - height).max(0.0),
                    };
                for line in lines {
                    if y + font * 1.25 <= area.y + area.height + 0.001 {
                        write!(svg,r#"<text x="{x:.3}" y="{:.3}" text-anchor="{anchor}" font-family="sans-serif" font-size="{font:.3}" fill="{}"{}{}>{}</text>"#,y+font,block.text_color,if block.bold {" font-weight=\"bold\""} else {""},if block.italic {" font-style=\"italic\""} else {""},escape(line)).expect("String writes are infallible");
                    }
                    y += font * 1.25;
                }
            }
        }
        svg.push_str("</g>");
    }
}
