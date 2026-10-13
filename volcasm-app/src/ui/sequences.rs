//! Sequence editor: ten parts with sample, functions, steps, knobs and
//! motion, plus ACTIVE STEP.

use eframe::egui::{self, Align2, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, vec2};
use volcasm_core::pattern::{Func, PARTS, Param, Pattern, STEPS, Speed};

use super::knob::{self, KnobSpec};
use super::{motion, style};
use crate::state::{AppState, PartView};

const STEP_SIZE: f32 = 28.0;
const STEP_GAP: f32 = 3.0;

const FUNCS: [(Func, &str); 4] = [
    (Func::Motion, "motion"),
    (Func::Loop, "loop"),
    (Func::Reverb, "reverb"),
    (Func::Reverse, "reverse"),
];

/// Knobs in the order of the device and Vosyr (SPEED handled separately).
const KNOBS: [(Param, &str, i32); 10] = [
    (Param::Level, "level", 127),
    (Param::Pan, "pan", 0),
    (Param::StartPoint, "start point", 0),
    (Param::Length, "length", 127),
    (Param::AmpEgAttack, "amp eg attack", 0),
    (Param::AmpEgDecay, "amp eg decay", 127),
    (Param::PitchEgInt, "pitch eg int", 0),
    (Param::PitchEgAttack, "pitch eg attack", 0),
    (Param::PitchEgDecay, "pitch eg decay", 127),
    (Param::HiCut, "hi cut", 127),
];

pub fn show(ui: &mut Ui, state: &mut AppState) {
    header(ui, state);
    for warning in state.pattern_warnings() {
        ui.colored_label(style::WARNING, warning);
    }
    ui.add_space(4.0);
    egui::ScrollArea::vertical().show(ui, |ui| {
        for part in 0..PARTS {
            // Each part gets its own id space: the parts repeat the same
            // widgets (knob labels, buttons).
            ui.push_id(("part", part), |ui| part_row(ui, state, part));
            ui.add_space(6.0);
        }
        ui.separator();
        active_steps(ui, state);
    });
}

fn header(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal_wrapped(|ui| {
        ui.label(style::brand(
            &format!("sequence {}", state.pattern + 1),
            22.0,
        ));
        ui.add_space(8.0);
        for n in 0..state.project.patterns().len() {
            let used = state.project.pattern(n) != Some(&Pattern::default());
            let text = if used {
                format!("{}•", n + 1)
            } else {
                (n + 1).to_string()
            };
            if ui.selectable_label(state.pattern == n, text).clicked() {
                state.select_pattern(n);
            }
        }
        ui.add_space(8.0);
        if ui.button(style::label("Copy")).clicked() {
            state.copy_pattern();
        }
        if ui
            .add_enabled(
                state.clipboard.is_some(),
                egui::Button::new(style::label("Paste")),
            )
            .clicked()
        {
            state.paste_pattern();
        }
        if ui.button(style::label("Clear")).clicked() {
            state.clear_pattern();
        }
        if ui
            .add_enabled(
                state.can_undo_pattern(),
                egui::Button::new(style::label("Undo")),
            )
            .clicked()
        {
            state.undo_pattern();
        }
    });
}

fn part_row(ui: &mut Ui, state: &mut AppState, i: usize) {
    let pattern = *state.current_pattern();
    let part = pattern.part(i);
    let frame = egui::Frame::new()
        .fill(style::TILE_EMPTY)
        .corner_radius(3.0)
        .inner_margin(6.0)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(style::brand(&format!("p{}", i + 1), 18.0));
                sample_combo(ui, state, i, part.sample());
                for (func, name) in FUNCS {
                    if ui
                        .selectable_label(part.func(func), style::label(name))
                        .clicked()
                    {
                        let on = !part.func(func);
                        state.edit_pattern(("func", i, func as usize), |p| {
                            p.part_mut(i).set_func(func, on)
                        });
                    }
                }
                if ui
                    .selectable_label(part.muted(), style::label("mute"))
                    .clicked()
                {
                    let muted = !part.muted();
                    state.edit_pattern(("mute", i, 0), |p| p.part_mut(i).set_muted(muted));
                }
                ui.separator();
                for (view, name) in [
                    (PartView::Params, "params"),
                    (PartView::Motion, "motion edit"),
                ] {
                    let open = state.part_views[i] == view;
                    if ui.selectable_label(open, style::label(name)).clicked() {
                        state.toggle_part_view(i, view);
                    }
                }
            });
            ui.add_space(4.0);
            let steps = part.steps();
            let active = pattern.active_steps();
            if let Some((bits, new_gesture)) =
                step_row(ui, Id::new(("steps", i)), steps, Some(active))
            {
                if new_gesture {
                    state.end_edit();
                }
                state.edit_pattern(("steps", i, 0), |p| p.part_mut(i).set_steps(bits));
            }
            match state.part_views[i] {
                PartView::Steps => {}
                PartView::Params => {
                    ui.add_space(4.0);
                    knobs(ui, state, i);
                }
                PartView::Motion => {
                    ui.add_space(4.0);
                    let mut row = state.motion_rows[i];
                    let edit = motion::show(ui, i, pattern.part(i), &mut row);
                    state.motion_rows[i] = row;
                    if let Some(edit) = edit {
                        state.edit_pattern(("motion", i, edit.row as usize), |p| {
                            // Rows with an API only; values are in range.
                            let _ = p.part_mut(i).set_motion(edit.row, edit.step, edit.value);
                        });
                    }
                }
            }
        });
    if state.scroll_to_part == Some(i) {
        state.scroll_to_part = None;
        frame.response.scroll_to_me(Some(egui::Align::TOP));
    }
}

fn sample_combo(ui: &mut Ui, state: &mut AppState, i: usize, current: u16) {
    let name = |state: &AppState, n: usize| match state.project.slot(n) {
        Some(slot) => style::caps(&format!("{n} {}", slot.name)),
        None => style::caps(&format!("{n} empty")),
    };
    let mut chosen = None;
    egui::ComboBox::from_id_salt(("sample", i))
        .selected_text(name(state, usize::from(current)))
        .width(200.0)
        .height(400.0)
        .show_ui(ui, |ui| {
            for n in 0..state.slot_count() {
                if ui
                    .selectable_label(usize::from(current) == n, name(state, n))
                    .clicked()
                {
                    chosen = Some(n as u16);
                }
            }
        });
    if let Some(n) = chosen {
        state.edit_pattern(("sample", i, 0), |p| p.part_mut(i).set_sample(n));
    }
}

fn knobs(ui: &mut Ui, state: &mut AppState, i: usize) {
    let part = *state.current_pattern().part(i);
    ui.horizontal_wrapped(|ui| {
        // SPEED first after LEVEL and PAN, as on the device.
        for (param, name, default) in &KNOBS[..2] {
            param_knob(ui, state, i, *param, name, *default, part.param(*param));
        }
        speed_knob(ui, state, i, part.speed());
        for (param, name, default) in &KNOBS[2..] {
            param_knob(ui, state, i, *param, name, *default, part.param(*param));
        }
    });
}

fn param_knob(
    ui: &mut Ui,
    state: &mut AppState,
    i: usize,
    param: Param,
    name: &str,
    default: i32,
    value: i8,
) {
    let (min, max) = param.range();
    let spec = KnobSpec {
        min: min.into(),
        max: max.into(),
        default,
        bipolar: param.is_bipolar(),
    };
    if let Some(v) = knob::show(
        ui,
        Id::new(("knob", i, param as usize)),
        name,
        value.into(),
        spec,
        None,
    )
    .value
    {
        state.edit_pattern(("param", i, param as usize), |p| {
            let _ = p.part_mut(i).set_param(param, v as i8);
        });
    }
}

fn speed_knob(ui: &mut Ui, state: &mut AppState, i: usize, speed: Speed) {
    let (value, limit, semitones) = match speed {
        Speed::Continuous(v) => (v, 63, false),
        Speed::Semitones(v) => (v, 24, true),
    };
    let spec = KnobSpec {
        min: -limit,
        max: limit,
        default: 0,
        bipolar: true,
    };
    // The footer switches the mode (FUNC + SPEED on the device).
    let footer = if semitones { "semitones" } else { "continuous" };
    let response = knob::show(
        ui,
        Id::new(("knob-speed", i)),
        "speed",
        value.into(),
        spec,
        Some(footer),
    );
    if let Some(v) = response.value {
        let new = if semitones {
            Speed::Semitones(v as i8)
        } else {
            Speed::Continuous(v as i8)
        };
        state.edit_pattern(("speed", i, 0), |p| {
            let _ = p.part_mut(i).set_speed(new);
        });
    }
    if response.footer_clicked {
        let new = if semitones {
            Speed::Continuous(value)
        } else {
            Speed::Semitones(value.clamp(-24, 24))
        };
        state.end_edit();
        state.edit_pattern(("speed-mode", i, 0), |p| {
            let _ = p.part_mut(i).set_speed(new);
        });
    }
}

fn active_steps(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.label(style::label("Active step"));
        ui.label(
            egui::RichText::new(style::caps("steps the sequence plays"))
                .small()
                .color(style::TEXT_DIM),
        );
    });
    let bits = state.current_pattern().active_steps();
    if let Some((bits, new_gesture)) = step_row(ui, Id::new("active-steps"), bits, None) {
        if new_gesture {
            state.end_edit();
        }
        state.edit_pattern(("active", 0, 0), |p| p.set_active_steps(bits));
    }
}

/// 16 step buttons for `bits`. Pressing a step toggles it; dragging across
/// steps sets them all to that new state. `active` dims inactive steps.
/// Returns the new bits when changed, and whether a new click or drag began.
fn step_row(ui: &mut Ui, id: Id, bits: u16, active: Option<u16>) -> Option<(u16, bool)> {
    // Shrink the steps to fit narrow windows.
    let size = (ui.available_width() / STEPS as f32 - STEP_GAP).clamp(14.0, STEP_SIZE);
    let pitch = size + STEP_GAP;
    let (rect, response) =
        ui.allocate_exact_size(vec2(STEPS as f32 * pitch, size), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    let step_at = |pos: Pos2| -> Option<usize> {
        if !rect.contains(pos) {
            return None;
        }
        let index = ((pos.x - rect.left()) / pitch).floor() as usize;
        (index < STEPS).then_some(index)
    };

    let mut new = bits;
    let paint_id = id.with("paint");
    let started = response.drag_started() || response.clicked();
    if started && let Some(index) = response.interact_pointer_pos().and_then(step_at) {
        let on = bits & (1 << index) == 0;
        ui.data_mut(|d| d.insert_temp(paint_id, on));
        set(&mut new, index, on);
    }
    if response.dragged() {
        let on: Option<bool> = ui.data(|d| d.get_temp(paint_id));
        if let (Some(on), Some(index)) = (on, response.interact_pointer_pos().and_then(step_at)) {
            set(&mut new, index, on);
        }
    }
    if response.drag_stopped() || response.clicked() {
        ui.data_mut(|d| d.remove::<bool>(paint_id));
    }

    for index in 0..STEPS {
        let min = Pos2::new(rect.left() + index as f32 * pitch, rect.top());
        let cell = Rect::from_min_size(min, vec2(size, size));
        let on = new & (1 << index) != 0;
        let enabled = active.is_none_or(|a| a & (1 << index) != 0);
        let fill = match (on, enabled) {
            (true, true) => style::ACCENT,
            (true, false) => style::ACCENT.gamma_multiply(0.35),
            (false, _) => style::BACKGROUND,
        };
        painter.rect_filled(cell, 3.0, fill);
        // Beats of four, as on the device's step buttons.
        let stroke = if index % 4 == 0 {
            style::TEXT_DIM
        } else {
            style::TILE_FILLED
        };
        painter.rect_stroke(cell, 3.0, Stroke::new(1.0, stroke), StrokeKind::Inside);
        painter.text(
            cell.center(),
            Align2::CENTER_CENTER,
            (index + 1).to_string(),
            style::body_font(12.0),
            if on { style::TEXT } else { style::TEXT_DIM },
        );
    }
    (new != bits).then_some((new, started))
}

fn set(bits: &mut u16, index: usize, on: bool) {
    if on {
        *bits |= 1 << index;
    } else {
        *bits &= !(1 << index);
    }
}

#[cfg(test)]
mod tests {
    use super::set;

    #[test]
    fn set_bits() {
        let mut bits = 0u16;
        set(&mut bits, 0, true);
        set(&mut bits, 15, true);
        assert_eq!(bits, 0x8001);
        set(&mut bits, 0, false);
        assert_eq!(bits, 0x8000);
    }
}
