//! A rotary knob: drag up/down or scroll to change, double-click to reset.

use std::f32::consts::PI;

use eframe::egui::{self, Align2, Id, Pos2, Response, Sense, Shape, Stroke, Ui, Vec2, vec2};

use super::style;

/// Arc from -135° to +135° (0° = up).
const SWEEP: f32 = 0.75 * PI;
const RADIUS: f32 = 17.0;
/// Value change per dragged point.
const DRAG_SPEED: f32 = 0.5;

/// Range and default of a knob.
#[derive(Debug, Clone, Copy)]
pub struct KnobSpec {
    pub min: i32,
    pub max: i32,
    pub default: i32,
    /// Arc starts at the center (bipolar) instead of the left end.
    pub bipolar: bool,
}

/// Result of [`show`].
pub struct KnobResponse {
    /// New value when changed.
    pub value: Option<i32>,
    /// The footer text under the knob was clicked.
    pub footer_clicked: bool,
}

const WIDTH: f32 = 78.0;
const HEIGHT: f32 = 90.0;

/// Shows a knob with `label` above it and an optional clickable `footer`
/// below. All knobs have the same size, so rows line up. `id` must be unique
/// among all knobs on screen.
pub fn show(
    ui: &mut Ui,
    id: Id,
    label: &str,
    value: i32,
    spec: KnobSpec,
    footer: Option<&str>,
) -> KnobResponse {
    let (outer, _) = ui.allocate_exact_size(vec2(WIDTH, HEIGHT), Sense::hover());
    let rect = outer.with_max_y(outer.top() + 70.0);
    let response = ui.interact(rect, id, Sense::click_and_drag());
    let painter = ui.painter_at(outer);
    painter.text(
        Pos2::new(rect.center().x, rect.top()),
        Align2::CENTER_TOP,
        style::caps(label),
        style::body_font(10.5),
        style::text_dim(),
    );
    let center = Pos2::new(rect.center().x, rect.top() + 18.0 + RADIUS + 2.0);

    let angle_of = |v: i32| {
        let t = (v - spec.min) as f32 / (spec.max - spec.min).max(1) as f32;
        -SWEEP + 2.0 * SWEEP * t
    };
    let point =
        |angle: f32, r: f32| Pos2::new(center.x + r * angle.sin(), center.y - r * angle.cos());
    let arc = |from: f32, to: f32| -> Vec<Pos2> {
        let steps = ((to - from).abs() / 0.08).ceil().max(1.0) as usize;
        (0..=steps)
            .map(|i| point(from + (to - from) * i as f32 / steps as f32, RADIUS))
            .collect()
    };

    let active = response.hovered() || response.dragged();
    painter.add(Shape::line(
        arc(-SWEEP, SWEEP),
        Stroke::new(3.0, style::background()),
    ));
    let start = if spec.bipolar { 0.0 } else { -SWEEP };
    let end = angle_of(value);
    let (from, to) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };
    painter.add(Shape::line(arc(from, to), Stroke::new(3.0, style::ACCENT)));
    painter.line_segment(
        [point(end, RADIUS - 9.0), point(end, RADIUS + 1.0)],
        Stroke::new(2.0, if active { style::text() } else { style::ACCENT }),
    );
    painter.text(
        center,
        Align2::CENTER_CENTER,
        value.to_string(),
        style::body_font(13.0),
        style::text(),
    );

    let mut new = value as f32;
    if response.dragged() {
        // Fractional drag accumulates in egui memory between frames.
        let id = response.id.with("acc");
        let mut acc: f32 = ui.data(|d| d.get_temp(id).unwrap_or(0.0));
        acc += -response.drag_delta().y * DRAG_SPEED;
        let whole = acc.trunc();
        ui.data_mut(|d| d.insert_temp(id, acc - whole));
        new += whole;
    }
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            new += scroll.signum();
            // The knob takes the scroll, so the page does not move.
            ui.input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);
        }
    }
    if response.double_clicked() {
        new = spec.default as f32;
    }
    let new = (new as i32).clamp(spec.min, spec.max);
    hover_text(response, spec);

    let mut footer_clicked = false;
    if let Some(text) = footer {
        let area = egui::Rect::from_min_max(Pos2::new(outer.left(), rect.bottom()), outer.max);
        let footer_response = ui.interact(area, id.with("footer"), Sense::click());
        footer_clicked = footer_response.clicked();
        painter.text(
            area.center(),
            Align2::CENTER_CENTER,
            style::caps(text),
            style::body_font(10.5),
            if footer_response.hovered() {
                style::text()
            } else {
                style::ACCENT
            },
        );
    }
    KnobResponse {
        value: (new != value).then_some(new),
        footer_clicked,
    }
}

fn hover_text(response: Response, spec: KnobSpec) {
    response.on_hover_text(style::caps(&format!(
        "{}..{} · drag or scroll · double-click: {}",
        spec.min, spec.max, spec.default
    )));
}
