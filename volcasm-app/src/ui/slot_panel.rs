//! Panel of the selected slot: name, waveform with trim handles, edits.

use eframe::egui::{self, Ui};
use volcasm_core::sample::edit::{PITCH_RANGE, pitched_frames};

use super::{style, waveform};
use crate::state::AppState;

/// Actions with side effects, handled by the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelAction {
    LoadFile(usize),
    Clear(usize),
    ApplyTrim,
    Normalize,
    Pitch,
    Undo(usize),
}

/// Name being edited, kept between frames.
#[derive(Default)]
pub struct NameEdit {
    slot: Option<usize>,
    text: String,
}

pub fn show(ui: &mut Ui, state: &mut AppState, edit: &mut NameEdit) -> Vec<PanelAction> {
    let mut actions = Vec::new();
    let Some(n) = state.selected else {
        ui.label(style::label("Select a slot"));
        return actions;
    };
    ui.label(style::brand(&format!("slot {n}"), 22.0));
    ui.add_space(4.0);

    let Some(slot) = state.project.slot(n) else {
        ui.label(style::label("Empty slot").color(style::TEXT_DIM));
        ui.add_space(6.0);
        if ui.button(style::label("Load file…")).clicked() {
            actions.push(PanelAction::LoadFile(n));
        }
        ui.label(
            style::label("or drop an audio file on the window")
                .small()
                .color(style::TEXT_DIM),
        );
        if state.can_undo(n) && ui.button(style::label("Undo")).clicked() {
            actions.push(PanelAction::Undo(n));
        }
        return actions;
    };

    if edit.slot != Some(n) {
        edit.slot = Some(n);
        edit.text = slot.name.clone();
    }
    ui.horizontal(|ui| {
        ui.label(style::label("Name"));
        let response =
            ui.add(egui::TextEdit::singleline(&mut edit.text).desired_width(f32::INFINITY));
        if response.lost_focus() || response.changed() {
            let name = edit.text.clone();
            state.rename(n, name);
        }
    });

    let slot = state.project.slot(n).expect("slot checked above");
    let frames = slot.sample.frames();
    let rate = f64::from(slot.sample.sample_rate);
    let memory = state.project.device().memory;
    let pcm = slot.sample.pcm.clone();
    let mut range = state.trim.clone().unwrap_or(0..frames);
    if range.end > frames {
        range = 0..frames;
    }

    ui.add_space(6.0);
    if waveform::show(ui, &pcm, &mut range, 140.0) {
        state.trim = Some(range.clone());
    }
    let seconds = |f: usize| f as f64 / rate;
    let kept = range.end - range.start;
    ui.add_space(4.0);
    egui::Grid::new("trim-info").num_columns(4).show(ui, |ui| {
        ui.label(style::label("Start").color(style::TEXT_DIM));
        ui.label(format!("{:.3} S", seconds(range.start)));
        ui.label(style::label("End").color(style::TEXT_DIM));
        ui.label(format!("{:.3} S", seconds(range.end)));
        ui.end_row();
        ui.label(style::label("Length").color(style::TEXT_DIM));
        ui.label(format!("{:.3} S", seconds(kept)));
        ui.label(style::label("Memory").color(style::TEXT_DIM));
        let before = memory.blocks_for(frames);
        let after = memory.blocks_for(kept);
        ui.label(if before == after {
            format!("{before} BLK")
        } else {
            format!("{before} → {after} BLK")
        });
        ui.end_row();
    });

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(style::label("Pitch"));
        ui.add(
            egui::DragValue::new(&mut state.pitch)
                .range(PITCH_RANGE)
                .speed(0.1)
                .suffix(" ST"),
        );
        let pitched = pitched_frames(frames, state.pitch);
        ui.label(
            egui::RichText::new(format!(
                "→ {:.3} S · {} BLK",
                pitched as f64 / rate,
                memory.blocks_for(pitched)
            ))
            .color(style::TEXT_DIM),
        );
        if ui
            .add_enabled(
                state.pitch != 0,
                egui::Button::new(style::label("Apply pitch")),
            )
            .clicked()
        {
            actions.push(PanelAction::Pitch);
        }
    });

    ui.add_space(6.0);
    let trimmed = range != (0..frames);
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(trimmed, egui::Button::new(style::label("Trim")))
            .clicked()
        {
            actions.push(PanelAction::ApplyTrim);
        }
        if ui
            .add_enabled(trimmed, egui::Button::new(style::label("Reset")))
            .clicked()
        {
            state.trim = None;
        }
        if ui.button(style::label("Normalize")).clicked() {
            actions.push(PanelAction::Normalize);
        }
        if ui
            .add_enabled(state.can_undo(n), egui::Button::new(style::label("Undo")))
            .clicked()
        {
            actions.push(PanelAction::Undo(n));
        }
        if ui.button(style::label("Replace…")).clicked() {
            actions.push(PanelAction::LoadFile(n));
        }
        if ui.button(style::label("Clear")).clicked() {
            actions.push(PanelAction::Clear(n));
        }
    });
    actions
}
