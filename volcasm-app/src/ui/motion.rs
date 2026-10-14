//! Motion editor of one part: 16 bars for the chosen parameter row.

use eframe::egui::{self, Align2, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, vec2};
use volcasm_core::pattern::{MotionRow, Part, STEPS};

use super::style;

pub const ROWS: [(MotionRow, &str); 14] = [
    (MotionRow::LevelStart, "level start"),
    (MotionRow::LevelEnd, "level end"),
    (MotionRow::PanStart, "pan start"),
    (MotionRow::PanEnd, "pan end"),
    (MotionRow::SpeedStart, "speed start"),
    (MotionRow::SpeedEnd, "speed end"),
    (MotionRow::AmpEgAttack, "amp eg attack"),
    (MotionRow::AmpEgDecay, "amp eg decay"),
    (MotionRow::PitchEgInt, "pitch eg int"),
    (MotionRow::PitchEgAttack, "pitch eg attack"),
    (MotionRow::PitchEgDecay, "pitch eg decay"),
    (MotionRow::StartPoint, "start point"),
    (MotionRow::Length, "length"),
    (MotionRow::HiCut, "hi cut"),
];

const BAR_HEIGHT: f32 = 70.0;

fn row_name(row: MotionRow) -> &'static str {
    ROWS.iter().find(|(r, _)| *r == row).map_or("?", |(_, n)| n)
}

/// A change made in the editor: `value` at `step` (1-16) of `row`.
pub struct MotionEdit {
    pub row: MotionRow,
    pub step: usize,
    pub value: Option<i8>,
}

/// Shows the editor. `row` is the row selection, kept by the caller.
pub fn show(ui: &mut Ui, id: usize, part: &Part, row: &mut MotionRow) -> Option<MotionEdit> {
    let mut edit = None;
    ui.horizontal(|ui| {
        ui.label(style::label("Motion"));
        egui::ComboBox::from_id_salt(("motion-row", id))
            .selected_text(style::caps(row_name(*row)))
            .width(160.0)
            .show_ui(ui, |ui| {
                for (r, name) in ROWS {
                    let used = (1..=STEPS).any(|s| part.raw_motion(r, s) != 0);
                    let text = if used {
                        style::caps(&format!("{name} •"))
                    } else {
                        style::caps(name)
                    };
                    ui.selectable_value(row, r, text);
                }
            });
        let hint = match row.param() {
            Some(_) => "click or drag to set · right-click to clear",
            None => "speed motion: raw values, read only (encoding unconfirmed)",
        };
        ui.label(
            egui::RichText::new(style::caps(hint))
                .small()
                .color(style::text_dim()),
        );
    });

    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, BAR_HEIGHT + 16.0), Sense::hover());
    let bar_width = rect.width() / STEPS as f32;
    let painter = ui.painter_at(rect);
    let range = row.param().map(|p| p.range());
    for step in 1..=STEPS {
        let x = rect.left() + (step - 1) as f32 * bar_width;
        let column = Rect::from_min_size(
            Pos2::new(x + 1.0, rect.top()),
            vec2(bar_width - 2.0, BAR_HEIGHT),
        );
        painter.rect_filled(column, 2.0, style::background());
        painter.text(
            Pos2::new(column.center().x, column.bottom() + 2.0),
            Align2::CENTER_TOP,
            step.to_string(),
            style::body_font(11.0),
            style::text_dim(),
        );
        let raw = part.raw_motion(*row, step);
        match (range, part.motion(*row, step)) {
            (Some((min, max)), Some(value)) => {
                let t = f32::from(value - min) / f32::from(max - min);
                let top = column.bottom() - t * column.height();
                painter.rect_filled(
                    Rect::from_min_max(Pos2::new(column.left(), top), column.right_bottom()),
                    2.0,
                    style::ACCENT.gamma_multiply(0.8),
                );
                painter.text(
                    Pos2::new(column.center().x, column.top() + 2.0),
                    Align2::CENTER_TOP,
                    value.to_string(),
                    style::body_font(11.0),
                    style::text(),
                );
            }
            (None, _) if raw != 0 => {
                painter.text(
                    column.center(),
                    Align2::CENTER_CENTER,
                    raw.to_string(),
                    style::body_font(11.0),
                    style::text(),
                );
            }
            _ => {
                painter.text(
                    column.center(),
                    Align2::CENTER_CENTER,
                    "-",
                    style::body_font(13.0),
                    style::text_dim(),
                );
            }
        }
        let Some((min, max)) = range else {
            continue;
        };
        let response = ui.interact(
            column,
            ui.id().with(("motion", id, step)),
            Sense::click_and_drag(),
        );
        if response.hovered() {
            painter.rect_stroke(
                column,
                2.0,
                Stroke::new(1.0, style::text_dim()),
                StrokeKind::Inside,
            );
        }
        if response.secondary_clicked() {
            edit = Some(MotionEdit {
                row: *row,
                step,
                value: None,
            });
        } else if (response.is_pointer_button_down_on() || response.dragged())
            && let Some(pos) = ui.input(|i| i.pointer.interact_pos())
        {
            // Dragging sideways sets the step under the pointer.
            let step = (((pos.x - rect.left()) / bar_width).floor() as usize + 1).clamp(1, STEPS);
            let t = ((column.bottom() - pos.y) / column.height()).clamp(0.0, 1.0);
            let value = (f32::from(min) + t * f32::from(max - min)).round() as i8;
            edit = Some(MotionEdit {
                row: *row,
                step,
                value: Some(value),
            });
        }
    }
    edit
}
