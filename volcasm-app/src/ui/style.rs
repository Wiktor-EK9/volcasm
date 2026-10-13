//! Fonts, colors and text helpers.
//!
//! Typography follows the volca sample panel: Overpass Bold, always lower
//! case, for the logo and important names (like the "volca sample" logo);
//! Barlow Semi Condensed Medium, always upper case, for everything else
//! (like the panel labels). Both fonts: SIL OFL 1.1, `assets/fonts/`.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, FontId, RichText, Stroke, Visuals,
};

const BARLOW: &str = "barlow-semi-condensed-medium";
const OVERPASS: &str = "overpass-bold";
/// Font family for [`brand`] text.
const BRAND_FAMILY: &str = "brand";

/// Accent: the red of the volca sample LED display.
pub const ACCENT: Color32 = Color32::from_rgb(0xe8, 0x40, 0x2f);
pub const BACKGROUND: Color32 = Color32::from_rgb(0x1c, 0x1d, 0x20);
pub const PANEL: Color32 = Color32::from_rgb(0x26, 0x27, 0x2b);
pub const TILE_EMPTY: Color32 = Color32::from_rgb(0x30, 0x31, 0x36);
pub const TILE_FILLED: Color32 = Color32::from_rgb(0x4a, 0x4c, 0x53);
pub const TEXT: Color32 = Color32::from_rgb(0xe6, 0xe6, 0xe6);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x8a, 0x8c, 0x92);
pub const WAVE: Color32 = Color32::from_rgb(0xd0, 0xd2, 0xd8);
pub const WARNING: Color32 = Color32::from_rgb(0xf0, 0xb4, 0x32);

/// Installs the two fonts and replaces egui's default fonts.
pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::empty();
    fonts.font_data.insert(
        BARLOW.into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/BarlowSemiCondensed-Medium.ttf"
        ))),
    );
    fonts.font_data.insert(
        OVERPASS.into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/Overpass-Bold.ttf"
        ))),
    );
    // Overpass as fallback for symbols Barlow lacks (e.g. arrows).
    fonts.families.insert(
        FontFamily::Proportional,
        vec![BARLOW.into(), OVERPASS.into()],
    );
    fonts
        .families
        .insert(FontFamily::Monospace, vec![BARLOW.into()]);
    fonts.families.insert(
        FontFamily::Name(BRAND_FAMILY.into()),
        vec![OVERPASS.into(), BARLOW.into()],
    );
    ctx.set_fonts(fonts);

    let mut visuals = Visuals::dark();
    visuals.panel_fill = PANEL;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = BACKGROUND;
    visuals.override_text_color = Some(TEXT);
    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.0, TEXT);
    visuals.hyperlink_color = ACCENT;
    ctx.set_theme(egui::Theme::Dark);
    ctx.set_visuals_of(egui::Theme::Dark, visuals);
    ctx.all_styles_mut(|style| {
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(15.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(15.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(12.0));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, FontId::proportional(20.0));
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
    });
}

/// Upper-case text: [`label`] for widgets, this for painted text.
pub fn caps(text: &str) -> String {
    text.to_uppercase()
}

/// Lower-case text: the brand style.
pub fn lower(text: &str) -> String {
    text.to_lowercase()
}

/// Interface text: Barlow, always upper case.
pub fn label(text: &str) -> RichText {
    RichText::new(caps(text))
}

/// Brand text (logo, project name, section titles): Overpass Bold, always
/// lower case.
pub fn brand(text: &str, size: f32) -> RichText {
    RichText::new(lower(text)).font(brand_font(size))
}

pub fn brand_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BRAND_FAMILY.into()))
}

pub fn body_font(size: f32) -> FontId {
    FontId::proportional(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_rules() {
        assert_eq!(caps("Send selected"), "SEND SELECTED");
        assert_eq!(lower("Volcasm Test1"), "volcasm test1");
        assert_eq!(caps("zażółć"), "ZAŻÓŁĆ");
    }
}
