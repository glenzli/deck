//! Self-contained inert SVG previews; full content stays in the kit manifest and material files.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use shape_domain::{ArtifactKind, CardKitCard, CardKitRequest};
use std::fmt::Write as _;

use super::{CardKitError, Material, ShapeProject, invalid, layout};

pub(super) struct RenderedCard {
    pub svg: String,
    pub overflow: bool,
}

impl ShapeProject {
    /// Previews exact pinned historical materials and reports stale heads without replacing them.
    /// # Errors
    /// Rejects unsupported content, wrong pins and unknown selected card identities.
    pub fn preview_card_kit(
        &self,
        request: &CardKitRequest,
        selected_id: Option<&str>,
    ) -> Result<Value, CardKitError> {
        let materials = self.card_kit_visible_materials(request)?;
        let selected_id = selected_id.unwrap_or(&request.cards[0].id);
        let selected = request
            .cards
            .iter()
            .find(|c| c.id == selected_id)
            .ok_or_else(|| invalid("selected card id does not exist"))?;
        let stale_inputs = materials
            .iter()
            .filter_map(|m| match self.store.artifact(m.source.artifact_id) {
                Ok(Some(a)) if a.accepted_revision == Some(m.source.revision_id) => None,
                _ => Some(m.source),
            })
            .collect::<Vec<_>>();
        let mut missing_artwork = Vec::new();
        let mut missing_text = Vec::new();
        let mut empty_content = Vec::new();
        let mut overflow = Vec::new();
        let mut ready = true;
        let cards = request
            .cards
            .iter()
            .map(|c| {
                let resolved = layout::resolve(request, c, &materials, true);
                let has_text = if resolved.legacy {
                    !c.title.trim().is_empty() && !rules(c, &materials).trim().is_empty()
                } else {
                    !layout::text_fields(c, &materials).is_empty()
                };
                let mut issues = layout::missing(&resolved, c, &materials);
                let overflowing = layout_overflow(request, c, &materials);
                if issues.contains(&"missing_artwork") {
                    missing_artwork.push(c.id.clone());
                }
                if issues
                    .iter()
                    .any(|v| matches!(*v, "missing_text" | "missing_title" | "missing_body"))
                {
                    missing_text.push(c.id.clone());
                }
                if issues.contains(&"empty_content") {
                    empty_content.push(c.id.clone());
                }
                if overflowing {
                    overflow.push(c.id.clone());
                    issues.push("layout_overflow");
                }
                ready &= issues.is_empty();
                json!({"id":c.id,"title":c.title,"quantity":c.quantity,
                "has_artwork":c.artwork.is_some(),"has_text":has_text,
                "artwork":c.artwork,"text_material":c.text_material,"layout_overflow":overflowing,
                "resolved_layout":resolved,"hidden_content":layout::hidden(&resolved,c,&materials),
                "issues":issues,"content_ready":issues.is_empty()})
            })
            .collect::<Vec<_>>();
        let rendered = render_card(request, selected, &materials);
        Ok(
            json!({"schema":"shape.card-kit-preview/1","cards":cards,"selected_card_id":selected_id,
            "svg":rendered.svg,"ready_to_export":ready,
            "missing_artwork_ids":missing_artwork,"missing_text_ids":missing_text,"overflow_card_ids":overflow,
            "empty_content_ids":empty_content,"default_layout":layout::resolve(request,selected,&materials,false),
            "print_defaults":super::print::default_options(request),
            "stale_inputs":stale_inputs,"output_kind":"card_material_kit","print_ready":false}),
        )
    }
}

pub(super) fn rules<'a>(card: &'a CardKitCard, materials: &'a [Material]) -> &'a str {
    card.text_material
        .and_then(|source| {
            materials
                .iter()
                .find(|m| m.source == source && m.kind == ArtifactKind::TextDocument)
        })
        .and_then(|m| std::str::from_utf8(&m.bytes).ok())
        .unwrap_or(&card.rules)
}

fn layout_overflow(request: &CardKitRequest, card: &CardKitCard, materials: &[Material]) -> bool {
    if request.layout.is_some() {
        layout::overflow(request, card, materials)
    } else {
        copy_overflow(card, materials, &MATERIAL_STYLE)
    }
}

pub(super) fn ensure_export_ready(
    request: &CardKitRequest,
    materials: &[Material],
) -> Result<(), CardKitError> {
    ensure_complete(request, materials)?;
    if request
        .cards
        .iter()
        .any(|card| layout_overflow(request, card, materials))
    {
        return Err(invalid(
            "card text overflows; review the reported card ids before export",
        ));
    }
    Ok(())
}

pub(super) fn ensure_complete(
    request: &CardKitRequest,
    materials: &[Material],
) -> Result<(), CardKitError> {
    for card in &request.cards {
        let issues = layout::missing(
            &layout::resolve(request, card, materials, true),
            card,
            materials,
        );
        if issues.contains(&"image_crop_limit") {
            return Err(CardKitError::ImageCropLimit {
                card_id: card.id.clone(),
            });
        }
        if !issues.is_empty() {
            return Err(invalid(
                "every card needs the visible content required by its layout before export",
            ));
        }
    }
    Ok(())
}

pub(super) fn render_card(
    request: &CardKitRequest,
    card: &CardKitCard,
    materials: &[Material],
) -> RenderedCard {
    if request.layout.is_some() {
        layout::render(request, card, materials, None)
            .expect("ordinary preview reports overflow without failing")
    } else {
        render_card_with_style(request, card, materials, &MATERIAL_STYLE)
    }
}

pub(super) fn render_print_card(
    request: &CardKitRequest,
    card: &CardKitCard,
    materials: &[Material],
    millimetres_per_unit: f64,
) -> Result<RenderedCard, CardKitError> {
    if request.layout.is_some() {
        return layout::render(request, card, materials, Some(millimetres_per_unit));
    }
    let style = CopyStyle::for_print(millimetres_per_unit)?;
    if copy_overflow(card, materials, &style) {
        return Err(CardKitError::PrintTextOverflow {
            card_id: card.id.clone(),
        });
    }
    Ok(render_card_with_style(request, card, materials, &style))
}

struct CopyStyle {
    font: [u16; 5],
    baseline: [u16; 5],
    spacing: [u16; 5],
    lines: [usize; 5],
}

const MATERIAL_STYLE: CopyStyle = CopyStyle {
    font: [34, 23, 24, 19, 24],
    baseline: [76, 624, 690, 945, 1010],
    spacing: [38, 28, 29, 23, 28],
    lines: [2, 2, 9, 3, 1],
};

impl CopyStyle {
    fn for_print(millimetres_per_unit: f64) -> Result<Self, CardKitError> {
        let mut style = Self {
            font: [0; 5],
            baseline: [0; 5],
            spacing: [0; 5],
            lines: [0; 5],
        };
        let top = [36, 594, 672, 914, 988];
        let height = [104, 70, 236, 64, 36];
        for (i, points) in [9.0, 7.0, 7.0, 6.0, 7.0].into_iter().enumerate() {
            let units = (points * 25.4 / (72.0 * millimetres_per_unit)).ceil();
            if !units.is_finite() || !(1.0..=4096.0).contains(&units) {
                return Err(invalid("invalid readable print text scale"));
            }
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "Finite positive integral font size is bounded to 4096 units"
            )]
            let font = units as u16;
            style.font[i] = font;
            style.baseline[i] = top[i] + font;
            style.spacing[i] = (font * 6).div_ceil(5);
            style.lines[i] = usize::from(height[i] / style.spacing[i]);
        }
        Ok(style)
    }
}

fn copy_overflow(card: &CardKitCard, materials: &[Material], style: &CopyStyle) -> bool {
    let metadata = format!("{}   {}", card.card_type, card.cost);
    [
        &card.title,
        &metadata,
        rules(card, materials),
        &card.flavor,
        &card.stats,
    ]
    .into_iter()
    .enumerate()
    .any(|(i, text)| {
        let wrapped = wrap(text, style.font[i]);
        !text.trim().is_empty() && (wrapped.overflow || wrapped.lines.len() > style.lines[i])
    })
}

fn render_card_with_style(
    request: &CardKitRequest,
    card: &CardKitCard,
    materials: &[Material],
    style: &CopyStyle,
) -> RenderedCard {
    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{}" height="{}" viewBox="0 0 750 1050"><desc>{}</desc><rect width="750" height="1050" rx="30" fill="#f7f2e8"/><rect x="20" y="20" width="710" height="1010" rx="22" fill="none" stroke="#40382f" stroke-width="4"/><rect x="48" y="150" width="654" height="430" rx="12" fill="#ded7c9"/><defs><clipPath id="art"><rect x="48" y="150" width="654" height="430" rx="12"/></clipPath></defs>"##,
        request.canvas_width,
        request.canvas_height,
        escape(&format!(
            "{}\n{}\n{}",
            card.title,
            rules(card, materials),
            card.flavor
        ))
    );
    if let Some(material) = card
        .artwork
        .and_then(|source| materials.iter().find(|m| m.source == source))
    {
        let href = format!("data:image/png;base64,{}", STANDARD.encode(&material.bytes));
        let (width, height) = material
            .image_dimensions
            .expect("validated artwork carries its raster dimensions");
        let scale = (654.0 / f64::from(width)).min(430.0 / f64::from(height));
        let width = f64::from(width) * scale;
        let height = f64::from(height) * scale;
        let x = 48.0 + (654.0 - width) / 2.0;
        let y = 150.0 + (430.0 - height) / 2.0;
        // Qt SVG stretches the image viewport even with preserveAspectRatio="meet". The
        // explicit rectangle therefore already has the accepted raster's aspect ratio.
        write!(svg,r#"<image x="{x:.3}" y="{y:.3}" width="{width:.3}" height="{height:.3}" preserveAspectRatio="xMidYMid meet" clip-path="url(#art)" xlink:href="{}"/>"#,escape(&href)).expect("String writes are infallible");
    } else {
        svg.push_str(r##"<path d="M290 410l62-74 44 44 48-61 43 91z" fill="#b1a796"/><circle cx="322" cy="282" r="24" fill="#b1a796"/>"##);
    }
    let metadata = format!("{}   {}", card.card_type, card.cost);
    for (i, text) in [
        &card.title,
        &metadata,
        rules(card, materials),
        &card.flavor,
        &card.stats,
    ]
    .into_iter()
    .enumerate()
    {
        text_lines(
            &mut svg,
            &wrap(text, style.font[i]).lines,
            style.lines[i],
            (56, style.baseline[i]),
            style.font[i],
            style.spacing[i],
            i == 3,
        );
    }
    svg.push_str("</svg>");
    RenderedCard {
        svg,
        overflow: copy_overflow(card, materials, style),
    }
}

fn text_lines(
    svg: &mut String,
    lines: &[String],
    limit: usize,
    position: (u16, u16),
    font: u16,
    spacing: u16,
    italic: bool,
) {
    let (x, y) = position;
    for (i, line) in lines.iter().take(limit).enumerate() {
        write!(svg,r##"<text x="{x}" y="{}" font-family="sans-serif" font-size="{font}" fill="#2c2721"{}>{}</text>"##,
            usize::from(y)+i*usize::from(spacing),if italic {" font-style=\"italic\""} else {""},escape(line)).expect("String writes are infallible");
    }
}

pub(super) struct WrappedText {
    pub lines: Vec<String>,
    pub overflow: bool,
}

fn wrap(text: &str, font_size: u16) -> WrappedText {
    // Preserve historical quarter-em capacity and 5/8 glyph costs exactly.
    wrap_with_advances(text, f64::from(font_size), 638.0, 4.0, glyph_units)
}

pub(super) fn wrap_flex_to_width(text: &str, font_size: f64, width: f64) -> WrappedText {
    // Flexible cards reserve 1.05 em for ASCII sans-serif advances, retaining 1.25 em for CJK
    // and 2 em for other Unicode. Twentieth-em integers keep the capacity comparison exact.
    wrap_with_advances(text, font_size, width, 20.0, flex_glyph_units)
}

pub(super) fn flex_line_width(text: &str, font_size: f64) -> f64 {
    let units = text.chars().map(flex_glyph_units).sum::<usize>();
    f64::from(u32::try_from(units).expect("bounded authored card text")) * font_size / 20.0
}

fn wrap_with_advances(
    text: &str,
    font_size: f64,
    width: f64,
    units_per_em: f64,
    advance: fn(char) -> usize,
) -> WrappedText {
    // Keep ordinary words whole; CJK phrases and oversized tokens can fill remaining line space.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "Card geometry bounds the nonnegative advance budget"
    )]
    let capacity = (width * units_per_em / font_size).max(0.0).floor() as usize;
    let space_units = advance(' ');
    let mut lines = Vec::new();
    let mut overflow = false;
    for paragraph in text.replace('\r', "").split('\n') {
        let mut line = String::new();
        let mut units = 0;
        for word in paragraph.split_whitespace() {
            let word_units = word.chars().map(advance).sum::<usize>();
            let can_split = word_units > capacity || word.chars().any(is_cjk);
            if !line.is_empty() {
                let required = if can_split {
                    word.chars().next().map_or(0, advance)
                } else {
                    word_units
                };
                if units + space_units + required <= capacity {
                    line.push(' ');
                    units += space_units;
                } else {
                    lines.push(std::mem::take(&mut line));
                    units = 0;
                }
            }
            let mut chars = word.char_indices().peekable();
            while let Some((start, c)) = chars.next() {
                let mut end = start + c.len_utf8();
                let mut cost = advance(c);
                // Keep a complete character with its following closing punctuation. Explicit
                // paragraph boundaries above remain authored; only automatic breaks change.
                while let Some(&(index, next)) = chars.peek() {
                    if !closing_punctuation(next) {
                        break;
                    }
                    cost += advance(next);
                    end = index + next.len_utf8();
                    chars.next();
                }
                let cluster = &word[start..end];
                if cost > capacity {
                    // A pathological punctuation run cannot be kept together within the budget.
                    // Preserve all characters in bounded lines and reject export via overflow.
                    overflow = true;
                    for (index, c) in cluster.char_indices() {
                        append_wrapped(
                            &mut lines,
                            &mut line,
                            &mut units,
                            &cluster[index..index + c.len_utf8()],
                            advance(c),
                            capacity,
                        );
                    }
                } else {
                    append_wrapped(&mut lines, &mut line, &mut units, cluster, cost, capacity);
                }
            }
        }
        lines.push(line);
    }
    WrappedText { lines, overflow }
}

fn append_wrapped(
    lines: &mut Vec<String>,
    line: &mut String,
    units: &mut usize,
    chunk: &str,
    cost: usize,
    capacity: usize,
) {
    if *units + cost > capacity && !line.is_empty() {
        lines.push(std::mem::take(line));
        *units = 0;
    }
    line.push_str(chunk);
    *units += cost;
}

const fn closing_punctuation(value: char) -> bool {
    matches!(
        value,
        '，' | '。'
            | '、'
            | '；'
            | '：'
            | '！'
            | '？'
            | '）'
            | '］'
            | '｝'
            | '〉'
            | '》'
            | '」'
            | '』'
            | '】'
            | '〕'
            | '〗'
            | '〙'
            | '〛'
            | '’'
            | '”'
            | '…'
            | '‥'
            | ','
            | '.'
            | ';'
            | ':'
            | '!'
            | '?'
            | ')'
            | ']'
            | '}'
    )
}

const fn glyph_units(value: char) -> usize {
    if value.is_ascii() || is_cjk(value) {
        5
    } else {
        8
    }
}

const fn flex_glyph_units(value: char) -> usize {
    if value.is_ascii() {
        21
    } else if is_cjk(value) {
        25
    } else {
        40
    }
}

const fn is_cjk(value: char) -> bool {
    matches!(value,
        '\u{1100}'..='\u{11ff}' // Hangul Jamo
        | '\u{2e80}'..='\u{303f}' // Radicals, ideographic descriptions and punctuation
        | '\u{3040}'..='\u{31ff}' // Kana, Bopomofo and CJK strokes
        | '\u{3400}'..='\u{4dbf}' // Han extension A
        | '\u{4e00}'..='\u{9fff}' // Common Han ideographs
        | '\u{ac00}'..='\u{d7af}' // Hangul syllables
        | '\u{f900}'..='\u{faff}' // Compatibility ideographs
        | '\u{ff01}'..='\u{ffdc}' // Full-width forms and half-width Kana/Hangul
        | '\u{ffe0}'..='\u{ffe6}'
        | '\u{20000}'..='\u{2fa1f}' // Supplementary Han and compatibility ideographs
        | '\u{30000}'..='\u{323af}' // Supplementary Han extensions
    )
}

pub(super) fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests;
