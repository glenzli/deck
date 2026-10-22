//! Shared backs reuse the existing card-face renderer and accepted material loader.
use serde_json::{Value, json};
use shape_domain::{CardKitBack, CardKitRequest};

use super::{CardKitError, ShapeProject, layout, preview};

pub(super) fn composition(request: &CardKitRequest) -> Result<CardKitRequest, CardKitError> {
    let back = request.back.as_ref().ok_or(super::print::print_invalid(
        "card_print.back_required",
        "choose a shared card back before printing backs",
    ))?;
    let (mode, color, artwork, image_fit) = match back {
        CardKitBack::Solid { color } => (
            "text",
            color.as_str(),
            None,
            shape_domain::CardImageFit::Contain,
        ),
        CardKitBack::Image {
            artwork,
            image_fit,
            background_color,
        } => ("image", background_color.as_str(), *artwork, *image_fit),
    };
    let mut result = request.clone();
    if request.layout.is_none() {
        result.canvas_width = 750;
        result.canvas_height = 1050;
    }
    result.layout = Some(serde_json::from_value(
        json!({"mode":mode,"background_color":color,
        "image_fit":image_fit,"border":false,"padding_percent":0}),
    )?);
    result.cards = vec![serde_json::from_value(
        json!({"id":"back","artwork":artwork}),
    )?];
    result.back = None;
    Ok(result)
}

#[cfg(test)]
mod tests;

impl ShapeProject {
    pub(super) fn render_card_back(
        &self,
        request: &CardKitRequest,
        scale: f64,
    ) -> Result<String, CardKitError> {
        let back = composition(request)?;
        if matches!(request.back, Some(CardKitBack::Image { artwork: None, .. })) {
            return Err(super::print::print_invalid(
                "card_print.back_incomplete",
                "choose accepted artwork for the shared card back",
            ));
        }
        let materials = self.card_kit_materials(&back, false)?;
        Ok(preview::render_print_card(&back, &back.cards[0], &materials, scale)?.svg)
    }

    pub(super) fn preview_card_kit_with_back(
        &self,
        request: &CardKitRequest,
        card_id: Option<&str>,
    ) -> Result<Value, CardKitError> {
        let mut result = self.preview_card_kit(request, card_id)?;
        if request.back.is_some() {
            let back = composition(request)?;
            let materials = self.card_kit_materials(&back, false)?;
            let rendered = preview::render_card(&back, &back.cards[0], &materials);
            result["back"] = json!({"svg":rendered.svg,"ready_to_print":
                !matches!(request.back, Some(CardKitBack::Image { artwork: None, .. }))
                && !layout::missing(&layout::resolve(&back, &back.cards[0], &materials, true), &back.cards[0], &materials).contains(&"image_crop_limit")});
        }
        Ok(result)
    }
}
