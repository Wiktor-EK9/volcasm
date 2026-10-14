//! A seven-segment number display like the volca sample's LED display, with
//! up/down arrows. Unlit segments show dimly behind the digits.

use eframe::egui::{
    Align2, Color32, CursorIcon, Id, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, vec2,
};

use super::style;

/// Dim red of unlit segments and idle arrows.
const UNLIT: Color32 = Color32::from_rgb(0x2c, 0x15, 0x13);
const ARROW: Color32 = Color32::from_rgb(0x42, 0x1c, 0x18);
const PANEL: Color32 = Color32::from_rgb(0x14, 0x12, 0x13);
/// Background while hovered and while pressed or dragged: like a button.
const PANEL_HOVER: Color32 = Color32::from_rgb(0x1e, 0x1a, 0x1b);
const PANEL_ACTIVE: Color32 = Color32::from_rgb(0x26, 0x1e, 0x1e);
/// Stronger steps for the light theme, where the dark display stands out
/// less from its own changes.
const PANEL_HOVER_LIGHT: Color32 = Color32::from_rgb(0x2c, 0x25, 0x26);
const PANEL_ACTIVE_LIGHT: Color32 = Color32::from_rgb(0x3a, 0x2a, 0x2a);
/// Value change per dragged point.
const DRAG_SPEED: f32 = 0.25;

/// Range, default and number of digits of a display.
#[derive(Debug, Clone, Copy)]
pub struct SegmentSpec {
    pub min: i32,
    pub max: i32,
    pub default: i32,
    pub digits: usize,
    /// Total height in points; match the buttons next to the display.
    pub height: f32,
}

/// Text for `value` in `digits` places: right-aligned, minus in front,
/// `!` (a blank in DSEG fonts) for unused places.
pub fn digits_text(value: i32, digits: usize) -> String {
    let text = value.to_string();
    let pad = digits.saturating_sub(text.chars().count());
    format!("{}{text}", "!".repeat(pad))
}

/// Shows the display. Returns the new value when changed: arrows, dragging
/// up or down, the scroll wheel, or a double click for the default.
pub fn show(ui: &mut Ui, id: Id, value: i32, spec: SegmentSpec, hint: &str) -> Option<i32> {
    // Font size chosen so the digits fill the height minus a margin.
    let pad = (spec.height * 0.12).round();
    let measure = |size: f32| {
        ui.fonts_mut(|f| {
            f.layout_no_wrap("8".into(), style::segment_font(size), Color32::WHITE)
                .size()
        })
    };
    let nominal = 10.0;
    let font_size = nominal * (spec.height - 2.0 * pad) / measure(nominal).y;
    let font = style::segment_font(font_size);
    let digit = measure(font_size);
    // Arrows about as wide as on the sketch: most of a digit.
    let arrow_w = digit.x * 0.8;
    let size = vec2(
        digit.x * spec.digits as f32 + arrow_w + pad * 3.0,
        spec.height,
    );
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    let radius = spec.height * 0.15;
    // Filled in at the end, once hover and press are known.
    let background = painter.add(Shape::Noop);

    // Digits: unlit "8"s, then the value on top.
    let digits_rect = Rect::from_min_size(
        rect.min + Vec2::splat(pad),
        vec2(digit.x * spec.digits as f32, digit.y),
    );
    let text_pos = digits_rect.right_center();
    painter.text(
        text_pos,
        Align2::RIGHT_CENTER,
        "8".repeat(spec.digits),
        font.clone(),
        UNLIT,
    );
    painter.text(
        text_pos,
        Align2::RIGHT_CENTER,
        digits_text(value, spec.digits),
        font,
        style::ACCENT,
    );

    let mut new = value;
    let display = ui.interact(digits_rect, id, Sense::click_and_drag());
    if display.hovered() || display.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
    }
    if display.dragged() {
        let acc_id = id.with("acc");
        let mut acc: f32 = ui.data(|d| d.get_temp(acc_id).unwrap_or(0.0));
        acc += -display.drag_delta().y * DRAG_SPEED;
        let whole = acc.trunc();
        ui.data_mut(|d| d.insert_temp(acc_id, acc - whole));
        new += whole as i32;
    }
    if display.double_clicked() {
        new = spec.default;
    }
    let hovered = display.hovered();
    let mut active = display.dragged() || display.is_pointer_button_down_on();

    // Arrows, stacked on the right.
    let arrows = Rect::from_min_max(
        Pos2::new(digits_rect.right() + pad, rect.top() + pad),
        Pos2::new(rect.right() - pad, rect.bottom() - pad),
    );
    let half = arrows.height() / 2.0;
    for up in [true, false] {
        let area = if up {
            Rect::from_min_size(arrows.min, vec2(arrows.width(), half))
        } else {
            Rect::from_min_size(arrows.min + vec2(0.0, half), vec2(arrows.width(), half))
        };
        let response = ui.interact(area, id.with(("arrow", up)), Sense::click());
        if response.clicked() {
            new += if up { 1 } else { -1 };
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        active |= response.is_pointer_button_down_on();
        let color = if response.is_pointer_button_down_on() {
            style::ACCENT
        } else if response.hovered() {
            style::ACCENT.gamma_multiply(0.6)
        } else {
            ARROW
        };
        let t = area.shrink2(vec2(0.0, area.height() * 0.12));
        let points = if up {
            vec![t.center_top(), t.right_bottom(), t.left_bottom()]
        } else {
            vec![t.left_top(), t.right_top(), t.center_bottom()]
        };
        painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
    }

    if hovered || ui.rect_contains_pointer(rect) {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            new += scroll.signum() as i32;
            ui.input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);
        }
    }
    // Button-like feedback: lighter when hovered, red outline while pressed
    // or dragged.
    let over = ui.rect_contains_pointer(rect);
    // On the light theme the display stays dark, so a thin inner outline
    // barely shows: there it is thicker, darker and drawn outside the box.
    let light = style::is_light();
    let fill = match (active, over, light) {
        (true, _, false) => PANEL_ACTIVE,
        (true, _, true) => PANEL_ACTIVE_LIGHT,
        (false, true, false) => PANEL_HOVER,
        (false, true, true) => PANEL_HOVER_LIGHT,
        (false, false, _) => PANEL,
    };
    // Leave room for the outside outline within the widget's own area,
    // which is the painter's clip rect.
    let panel = if light { rect.shrink(2.0) } else { rect };
    painter.set(background, Shape::rect_filled(panel, radius, fill));
    let (width, kind) = if light {
        (2.0, StrokeKind::Outside)
    } else {
        (1.0, StrokeKind::Inside)
    };
    if active {
        painter.rect_stroke(panel, radius, Stroke::new(width + 0.5, style::ACCENT), kind);
    } else if over {
        let color = if light {
            style::text()
        } else {
            style::text_dim()
        };
        painter.rect_stroke(panel, radius, Stroke::new(width, color), kind);
    }

    let response = ui.interact(rect, id.with("hint"), Sense::hover());
    response.on_hover_text(style::caps(&format!(
        "{hint} · {}..{} · drag, scroll or arrows · double-click: {}",
        spec.min, spec.max, spec.default
    )));
    let new = new.clamp(spec.min, spec.max);
    (new != value).then_some(new)
}

#[cfg(test)]
mod tests {
    use super::digits_text;

    #[test]
    fn digits_are_right_aligned_with_blanks() {
        assert_eq!(digits_text(125, 3), "125");
        assert_eq!(digits_text(56, 3), "!56");
        assert_eq!(digits_text(-12, 3), "-12");
        assert_eq!(digits_text(-3, 3), "!-3");
        assert_eq!(digits_text(0, 3), "!!0");
    }
}
