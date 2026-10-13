//! Transfer settings, start button and progress.

use eframe::egui::{self, Ui};
use volcasm_audio::OutputInfo;

use super::style;
use crate::state::{AppState, BIT_CHOICES};

/// Progress of a running transfer, for display.
pub struct Progress {
    pub phase: &'static str,
    /// 0..=1.
    pub fraction: f32,
    pub remaining_seconds: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferAction {
    Send,
    /// Send only the sequence shown in the editor.
    SendCurrentPattern,
    Cancel,
    RefreshOutputs,
}

pub fn show(
    ui: &mut Ui,
    state: &mut AppState,
    outputs: &Result<Vec<OutputInfo>, String>,
    progress: Option<Progress>,
    report: &[(bool, String)],
) -> Vec<TransferAction> {
    let mut actions = Vec::new();
    ui.label(style::brand("transfer", 22.0));
    ui.add_space(4.0);

    let targets = state.transfer_targets();
    ui.horizontal_wrapped(|ui| {
        ui.label(style::label(&format!("{} checked", state.checked.len())));
        if ui.small_button(style::label("Check filled")).clicked() {
            state.check_filled();
        }
        if ui.small_button(style::label("Uncheck all")).clicked() {
            state.checked.clear();
        }
    });

    ui.horizontal_wrapped(|ui| {
        ui.label(style::label("Sequences"));
        for n in 0..state.project.patterns().len() {
            let checked = state.checked_patterns.contains(&n);
            if ui.selectable_label(checked, (n + 1).to_string()).clicked() {
                state.toggle_checked_pattern(n);
            }
        }
        if ui.small_button(style::label("All")).clicked() {
            state.checked_patterns = (0..state.project.patterns().len()).collect();
        }
        if ui.small_button(style::label("None")).clicked() {
            state.checked_patterns.clear();
        }
    });

    ui.add_space(4.0);
    egui::Grid::new("transfer-settings")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label(style::label("Compression"));
            let text = |bits: Option<u32>| match bits {
                Some(16) => style::caps("16 bit (lossless)"),
                Some(b) => style::caps(&format!("{b} bit")),
                None => style::caps("off"),
            };
            egui::ComboBox::from_id_salt("bits")
                .selected_text(text(state.transfer.bits))
                .show_ui(ui, |ui| {
                    for bits in BIT_CHOICES.map(Some).into_iter().chain([None]) {
                        ui.selectable_value(&mut state.transfer.bits, bits, text(bits));
                    }
                });
            ui.end_row();

            ui.label(style::label("Output"));
            ui.horizontal(|ui| {
                let selected = match (&state.transfer.device, outputs) {
                    (None, _) => style::caps("default"),
                    (Some(id), Ok(list)) => list
                        .iter()
                        .find(|o| &o.id == id)
                        .map_or_else(|| style::caps(id), |o| style::caps(&o.name)),
                    (Some(id), Err(_)) => style::caps(id),
                };
                egui::ComboBox::from_id_salt("output")
                    .selected_text(selected)
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut state.transfer.device,
                            None,
                            style::caps("default"),
                        );
                        if let Ok(list) = outputs {
                            for output in list {
                                ui.selectable_value(
                                    &mut state.transfer.device,
                                    Some(output.id.clone()),
                                    style::caps(&output.name),
                                );
                            }
                        }
                    });
                if ui.small_button(style::label("Refresh")).clicked() {
                    actions.push(TransferAction::RefreshOutputs);
                }
            });
            ui.end_row();
        });
    if let Err(err) = outputs {
        ui.colored_label(style::WARNING, err);
    }
    ui.checkbox(
        &mut state.transfer.erase_empty,
        style::label("Erase checked empty slots"),
    );

    ui.add_space(8.0);
    match progress {
        None => {
            let label = if targets.is_empty() {
                style::caps("send")
            } else {
                style::caps(&format!("send {} items", targets.len()))
            };
            let button = egui::Button::new(egui::RichText::new(label).size(18.0))
                .fill(style::ACCENT)
                .min_size(egui::vec2(ui.available_width(), 34.0));
            if ui.add_enabled(!targets.is_empty(), button).clicked() {
                actions.push(TransferAction::Send);
            }
            if state.view == crate::state::View::Sequences {
                let text = style::caps(&format!("send sequence {} only", state.pattern + 1));
                if ui
                    .add(egui::Button::new(text).min_size(egui::vec2(ui.available_width(), 26.0)))
                    .clicked()
                {
                    actions.push(TransferAction::SendCurrentPattern);
                }
            }
        }
        Some(progress) => {
            ui.label(style::label(progress.phase));
            ui.add(
                egui::ProgressBar::new(progress.fraction)
                    .fill(style::ACCENT)
                    .text(format!(
                        "{:.0}%  ·  {:.0} S LEFT",
                        progress.fraction * 100.0,
                        progress.remaining_seconds.max(0.0)
                    )),
            );
            if ui.button(style::label("Cancel")).clicked() {
                actions.push(TransferAction::Cancel);
            }
        }
    }
    for (is_error, line) in report {
        ui.colored_label(
            if *is_error {
                style::ACCENT
            } else {
                style::WARNING
            },
            line,
        );
    }
    actions
}
