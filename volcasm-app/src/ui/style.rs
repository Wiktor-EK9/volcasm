//! Fonts, colors and text helpers.
//!
//! Typography follows the volca sample panel: Overpass Bold, always lower
//! case, for the logo and important names (like the "volca sample" logo);
//! Barlow Semi Condensed Medium, always upper case, for everything else
//! (like the panel labels). DSEG7 Classic Mini for the seven-segment
//! displays (like the volca's LED display). All fonts: SIL OFL 1.1,
//! `assets/fonts/`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, FontId, RichText, Stroke, Visuals,
};

const BARLOW: &str = "barlow-semi-condensed-medium";
const OVERPASS: &str = "overpass-bold";
const DSEG: &str = "dseg7-classic-mini";
/// Font family for [`segment_font`].
const SEGMENT_FAMILY: &str = "segment";
/// Font family for [`brand`] text.
const BRAND_FAMILY: &str = "brand";

/// Accent: the red of the volca sample LED display. The same in both
/// themes.
pub const ACCENT: Color32 = Color32::from_rgb(0xe8, 0x40, 0x2f);

/// Interface colors of one theme.
struct Palette {
    background: Color32,
    panel: Color32,
    tile_empty: Color32,
    tile_filled: Color32,
    text: Color32,
    text_dim: Color32,
    wave: Color32,
    warning: Color32,
}

const DARK: Palette = Palette {
    background: Color32::from_rgb(0x1c, 0x1d, 0x20),
    panel: Color32::from_rgb(0x26, 0x27, 0x2b),
    tile_empty: Color32::from_rgb(0x30, 0x31, 0x36),
    tile_filled: Color32::from_rgb(0x4a, 0x4c, 0x53),
    text: Color32::from_rgb(0xe6, 0xe6, 0xe6),
    text_dim: Color32::from_rgb(0x8a, 0x8c, 0x92),
    wave: Color32::from_rgb(0xd0, 0xd2, 0xd8),
    warning: Color32::from_rgb(0xf0, 0xb4, 0x32),
};

/// The dark grays inverted (255 - value); the warning amber darkened to
/// stay readable on a light background.
const LIGHT: Palette = Palette {
    background: Color32::from_rgb(0xe3, 0xe2, 0xdf),
    panel: Color32::from_rgb(0xd9, 0xd8, 0xd4),
    tile_empty: Color32::from_rgb(0xcf, 0xce, 0xc9),
    tile_filled: Color32::from_rgb(0xb5, 0xb3, 0xac),
    text: Color32::from_rgb(0x19, 0x19, 0x19),
    text_dim: Color32::from_rgb(0x75, 0x73, 0x6d),
    wave: Color32::from_rgb(0x2f, 0x2d, 0x27),
    warning: Color32::from_rgb(0xa8, 0x6e, 0x00),
};

static LIGHT_THEME: AtomicBool = AtomicBool::new(false);

fn palette() -> &'static Palette {
    if is_light() { &LIGHT } else { &DARK }
}

/// Whether the light theme is active.
pub fn is_light() -> bool {
    LIGHT_THEME.load(Ordering::Relaxed)
}

/// Follows the system's light or dark setting (first start, no saved
/// choice). Call [`sync_theme`] every frame to pick up the result.
pub fn follow_system(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::System);
    sync_theme(ctx);
}

/// Makes the custom colors match the theme egui is using.
pub fn sync_theme(ctx: &egui::Context) {
    LIGHT_THEME.store(ctx.theme() == egui::Theme::Light, Ordering::Relaxed);
}

/// Switches the theme of the whole interface.
pub fn set_light(ctx: &egui::Context, light: bool) {
    LIGHT_THEME.store(light, Ordering::Relaxed);
    ctx.set_theme(if light {
        egui::Theme::Light
    } else {
        egui::Theme::Dark
    });
}

pub fn background() -> Color32 {
    palette().background
}
pub fn panel() -> Color32 {
    palette().panel
}
pub fn tile_empty() -> Color32 {
    palette().tile_empty
}
pub fn tile_filled() -> Color32 {
    palette().tile_filled
}
pub fn text() -> Color32 {
    palette().text
}
pub fn text_dim() -> Color32 {
    palette().text_dim
}
pub fn wave() -> Color32 {
    palette().wave
}
pub fn warning() -> Color32 {
    palette().warning
}

/// egui's own colors (text fields, lists, sliders) for `palette`.
fn visuals(base: Visuals, palette: &Palette) -> Visuals {
    let mut visuals = base;
    visuals.panel_fill = palette.panel;
    visuals.window_fill = palette.panel;
    visuals.extreme_bg_color = palette.background;
    visuals.override_text_color = Some(palette.text);
    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.0, palette.text);
    visuals.hyperlink_color = ACCENT;
    visuals
}

/// Installs the fonts (replacing egui's default fonts) and the colors of
/// both themes, starting with the light theme if `light`.
pub fn install(ctx: &egui::Context, light: bool) {
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
    fonts.font_data.insert(
        DSEG.into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/DSEG7ClassicMini-Regular.ttf"
        ))),
    );
    fonts
        .families
        .insert(FontFamily::Name(SEGMENT_FAMILY.into()), vec![DSEG.into()]);
    fonts.families.insert(
        FontFamily::Name(BRAND_FAMILY.into()),
        vec![OVERPASS.into(), BARLOW.into()],
    );
    ctx.set_fonts(fonts);

    ctx.set_visuals_of(egui::Theme::Dark, visuals(Visuals::dark(), &DARK));
    ctx.set_visuals_of(egui::Theme::Light, visuals(Visuals::light(), &LIGHT));
    set_light(ctx, light);
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

/// Seven-segment display font (digits, minus; `!` is a blank digit).
pub fn segment_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEGMENT_FAMILY.into()))
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
