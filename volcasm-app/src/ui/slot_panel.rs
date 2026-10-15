//! Panel of the selected slot: name, waveform with trim handles, edits.

use eframe::egui::{self, Ui};
use volcasm_core::sample::edit::{PITCH_RANGE, pitched_frames};

use super::{segment, style, waveform};
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
    Paste(usize),
}

/// Name being edited, kept between frames.
#[derive(Default)]
pub struct NameEdit {
    slot: Option<usize>,
    text: String,
}

/// `playing`: how much of the selected slot's preview has played (0..1),
/// when it is the slot being previewed.
pub fn show(
    ui: &mut Ui,
    state: &mut AppState,
    edit: &mut NameEdit,
    playing: Option<f32>,
) -> Vec<PanelAction> {
    let mut actions = Vec::new();
    let Some(n) = state.selected else {
        ui.label(style::label("Select a slot"));
        return actions;
    };
    ui.label(style::brand(&format!("slot {n}"), 22.0));
    ui.add_space(4.0);

    let Some(slot) = state.project.slot(n) else {
        style::subsection(ui, "empty slot");
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(style::icon_label(style::icon::FILE_AUDIO, "Load file…"))
                .on_hover_text(style::caps(
                    "several files go to this slot and the next free ones",
                ))
                .clicked()
            {
                actions.push(PanelAction::LoadFile(n));
            }
            if let Some(copied) = &state.sample_clipboard
                && ui
                    .button(style::icon_label(style::icon::CLIPBOARD_TEXT, "Paste"))
                    .on_hover_text(&copied.name)
                    .clicked()
            {
                actions.push(PanelAction::Paste(n));
            }
        });
        ui.label(
            style::label("or drop an audio file on the window")
                .small()
                .color(style::text_dim()),
        );
        if state.can_undo(n)
            && ui
                .button(style::icon_label(style::icon::ARROW_U_UP_LEFT, "Undo"))
                .clicked()
        {
            actions.push(PanelAction::Undo(n));
        }
        return actions;
    };

    if edit.slot != Some(n) {
        edit.slot = Some(n);
        edit.text = slot.name.clone();
    }
    style::subsection(ui, "name");
    let response = ui.add(egui::TextEdit::singleline(&mut edit.text).desired_width(f32::INFINITY));
    if response.lost_focus() || response.changed() {
        let name = edit.text.clone();
        state.rename(n, name);
    }

    let slot = state.project.slot(n).expect("slot checked above");
    let frames = slot.sample.frames();
    let rate = f64::from(slot.sample.sample_rate);
    let memory = state.project.device().memory;
    let pcm = slot.sample.pcm.clone();
    let mut range = state.trim.clone().unwrap_or(0..frames);
    if range.end > frames {
        range = 0..frames;
    }

    style::subsection(ui, &style::with_icon(style::icon::WAVEFORM, "waveform"));
    if waveform::show(ui, &pcm, &mut range, 140.0, playing) {
        state.trim = Some(range.clone());
    }
    let seconds = |f: usize| f as f64 / rate;
    let kept = range.end - range.start;
    ui.add_space(4.0);
    egui::Grid::new("trim-info").num_columns(4).show(ui, |ui| {
        ui.label(style::label("Start").color(style::text_dim()));
        ui.label(format!("{:.3} S", seconds(range.start)));
        ui.label(style::label("End").color(style::text_dim()));
        ui.label(format!("{:.3} S", seconds(range.end)));
        ui.end_row();
        ui.label(style::label("Length").color(style::text_dim()));
        ui.label(format!("{:.3} S", seconds(kept)));
        ui.label(style::label("Memory").color(style::text_dim()));
        let before = memory.blocks_for(frames);
        let after = memory.blocks_for(kept);
        ui.label(if before == after {
            format!("{before} BLK")
        } else {
            format!("{before} → {after} BLK")
        });
        ui.end_row();
    });

    style::subsection(ui, "pitch");
    let button_height = ui.spacing().interact_size.y + 2.0 * ui.spacing().button_padding.y;
    // A row as tall as the display, with everything centered in it.
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), button_height),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            let spec = segment::SegmentSpec {
                min: *PITCH_RANGE.start(),
                max: *PITCH_RANGE.end(),
                default: 0,
                digits: 3,
                // Same height as the APPLY PITCH button next to it.
                height: button_height,
            };
            if let Some(p) = segment::show(
                ui,
                egui::Id::new("pitch"),
                state.pitch,
                spec,
                "pitch, semitones",
            ) {
                state.pitch = p;
            }
            let pitched = pitched_frames(frames, state.pitch);
            ui.label(
                egui::RichText::new(format!(
                    "→ {:.3} S · {} BLK",
                    pitched as f64 / rate,
                    memory.blocks_for(pitched)
                ))
                .color(style::text_dim()),
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
        },
    );

    style::subsection(ui, "edit");
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
        if ui.button(style::label("Replace")).clicked() {
            actions.push(PanelAction::LoadFile(n));
        }
    });
    // Same buttons, in the same order, as for sequences.
    ui.horizontal_wrapped(|ui| {
        if ui
            .button(style::icon_label(style::icon::COPY, "Copy"))
            .clicked()
        {
            state.copy_sample(n);
        }
        if ui
            .add_enabled(
                state.sample_clipboard.is_some(),
                egui::Button::new(style::icon_label(style::icon::CLIPBOARD_TEXT, "Paste")),
            )
            .clicked()
        {
            actions.push(PanelAction::Paste(n));
        }
        if ui
            .button(style::icon_label(style::icon::TRASH, "Clear"))
            .clicked()
        {
            actions.push(PanelAction::Clear(n));
        }
        if ui
            .add_enabled(
                state.can_undo(n),
                egui::Button::new(style::icon_label(style::icon::ARROW_U_UP_LEFT, "Undo")),
            )
            .clicked()
        {
            actions.push(PanelAction::Undo(n));
        }
    });
    actions
}
