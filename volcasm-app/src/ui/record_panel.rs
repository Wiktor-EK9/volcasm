//! Recording into the selected slot: input, level, REC/STOP.

use eframe::egui::{self, Ui};
use volcasm_audio::OutputInfo;

use super::style;
use crate::state::AppState;

/// A recording in progress, for display.
pub struct Recording {
    pub slot: usize,
    pub seconds: f64,
    pub max_seconds: f64,
    /// Peak level 0..1 (decaying).
    pub level: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordAction {
    Start(usize),
    Stop,
    RefreshInputs,
}

pub fn show(
    ui: &mut Ui,
    state: &mut AppState,
    inputs: &Result<Vec<OutputInfo>, String>,
    recording: Option<&Recording>,
    busy: bool,
) -> Option<RecordAction> {
    let slot = state.selected?;
    let mut action = None;
    ui.label(style::brand("record", 22.0));
    ui.add_space(4.0);

    if let Some(rec) = recording {
        ui.label(style::label(&format!(
            "recording into slot {} · {:.1} / {:.0} s",
            rec.slot, rec.seconds, rec.max_seconds
        )));
        level_meter(ui, rec.level);
        let stop = egui::Button::new(style::icon_label(style::icon::STOP, "stop").size(18.0))
            .fill(style::ACCENT)
            .min_size(egui::vec2(ui.available_width(), 30.0));
        if ui.add(stop).clicked() {
            action = Some(RecordAction::Stop);
        }
        return action;
    }

    style::subsection(ui, &style::with_icon(style::icon::MICROPHONE, "input"));
    ui.horizontal(|ui| {
        let selected = match (&state.record.input, inputs) {
            (None, _) => style::caps("default"),
            (Some(id), Ok(list)) => list
                .iter()
                .find(|i| &i.id == id)
                .map_or_else(|| style::caps(id), |i| style::caps(&i.name)),
            (Some(id), Err(_)) => style::caps(id),
        };
        egui::ComboBox::from_id_salt("record-input")
            .selected_text(selected)
            .width(220.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.record.input, None, style::caps("default"));
                if let Ok(list) = inputs {
                    for input in list {
                        ui.selectable_value(
                            &mut state.record.input,
                            Some(input.id.clone()),
                            style::caps(&input.name),
                        );
                    }
                }
            });
        if ui.small_button(style::label("Refresh")).clicked() {
            action = Some(RecordAction::RefreshInputs);
        }
    });
    if let Err(err) = inputs {
        ui.colored_label(style::warning(), err);
    }
    ui.checkbox(
        &mut state.record.trim_silence,
        style::label("Trim silence at both ends"),
    );
    let label = if state.project.slot(slot).is_some() {
        format!("rec into slot {slot} (replaces, undo keeps it)")
    } else {
        format!("rec into slot {slot}")
    };
    let rec = egui::Button::new(egui::RichText::new(style::caps(&label)).size(16.0))
        .min_size(egui::vec2(ui.available_width(), 28.0));
    if ui
        .add_enabled(!busy, rec)
        .on_disabled_hover_text(style::caps("busy"))
        .clicked()
    {
        action = Some(RecordAction::Start(slot));
    }
    action
}

fn level_meter(ui: &mut Ui, level: f32) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 14.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, style::background());
    let mut fill = rect;
    fill.set_width(rect.width() * level.clamp(0.0, 1.0));
    let color = if level >= 0.99 {
        style::ACCENT
    } else {
        style::wave()
    };
    painter.rect_filled(fill, 2.0, color);
}
