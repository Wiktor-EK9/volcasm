//! Transfer settings, start button and progress.

use eframe::egui::{self, Ui};
use volcasm_audio::OutputInfo;

use super::style;
use crate::state::{AppState, LOSSLESS_BITS, MIN_TRANSFER_BITS};

/// Progress of a running transfer, for display.
#[derive(Clone)]
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

/// Name of a transfer quality in the list.
fn quality_text(bits: u32) -> String {
    if bits == LOSSLESS_BITS {
        style::caps(&format!("{bits} bit · lossless"))
    } else {
        style::caps(&format!("{bits} bit"))
    }
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
    let patterns = state.project.patterns().len();

    style::subsection(ui, &style::with_icon(style::icon::WAVEFORM, "samples"));
    ui.label(style::label(&format!("{} checked", state.checked.len())));
    ui.horizontal_wrapped(|ui| {
        if ui.small_button(style::label("Check all")).clicked() {
            state.check_all();
        }
        if ui.small_button(style::label("Check filled")).clicked() {
            state.check_filled();
        }
        if ui.small_button(style::label("Uncheck all")).clicked() {
            state.checked.clear();
        }
    });
    ui.checkbox(
        &mut state.transfer.erase_empty,
        style::label("Erase checked empty slots"),
    );
    ui.horizontal(|ui| {
        // The list is drawn taller than the row it is laid out in (a
        // button's text and padding against the interaction height) and
        // hangs below it: the label moves down by half the difference to
        // sit on the list's center line.
        let row = ui.spacing().interact_size.y;
        let list = (ui.text_style_height(&egui::TextStyle::Button)
            + 2.0 * ui.spacing().button_padding.y)
            .max(row);
        let text = egui::WidgetText::from(style::label("Quality")).into_galley(
            ui,
            None,
            f32::INFINITY,
            egui::TextStyle::Body,
        );
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(text.size().x, row), egui::Sense::hover());
        let top = rect.center().y + (list - row) / 2.0 - text.size().y / 2.0;
        ui.painter().galley(
            egui::pos2(rect.left(), top),
            text,
            ui.visuals().text_color(),
        );
        egui::ComboBox::from_id_salt("transfer-bits")
            .selected_text(quality_text(state.transfer.bits))
            .width(170.0)
            .show_ui(ui, |ui| {
                for bits in (MIN_TRANSFER_BITS..=LOSSLESS_BITS).rev() {
                    ui.selectable_value(&mut state.transfer.bits, bits, quality_text(bits));
                }
            });
    })
    .response
    .on_hover_text(style::caps(
        "Fewer bits: a shorter transfer, more noise in the sample. \
         The sample takes the same memory on the volca.",
    ));

    style::subsection(ui, &style::with_icon(style::icon::STEPS, "sequences"));
    ui.horizontal_wrapped(|ui| {
        for n in 0..patterns {
            let checked = state.checked_patterns.contains(&n);
            if ui.selectable_label(checked, (n + 1).to_string()).clicked() {
                state.toggle_checked_pattern(n);
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        if ui.small_button(style::label("All")).clicked() {
            state.checked_patterns = (0..patterns).collect();
        }
        if ui.small_button(style::label("None")).clicked() {
            state.checked_patterns.clear();
        }
        let text = style::caps(&format!("send sequence {} only", state.pattern + 1));
        if progress.is_none() && ui.small_button(text).clicked() {
            actions.push(TransferAction::SendCurrentPattern);
        }
    });

    style::subsection(ui, &style::with_icon(style::icon::SPEAKER_HIGH, "output"));
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
                ui.selectable_value(&mut state.transfer.device, None, style::caps("default"));
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
        if ui
            .small_button(style::icon_label(style::icon::ARROWS_CLOCKWISE, "Refresh"))
            .clicked()
        {
            actions.push(TransferAction::RefreshOutputs);
        }
    });
    if let Err(err) = outputs {
        ui.colored_label(style::warning(), err);
    }

    ui.add_space(8.0);
    match progress {
        None => {
            let label = if targets.is_empty() {
                style::with_icon(style::icon::PAPER_PLANE_TILT, "send")
            } else {
                style::with_icon(
                    style::icon::PAPER_PLANE_TILT,
                    &format!("send {} items", targets.len()),
                )
            };
            let button = egui::Button::new(egui::RichText::new(label).size(18.0))
                .fill(style::ACCENT)
                .min_size(egui::vec2(ui.available_width(), 34.0));
            if ui.add_enabled(!targets.is_empty(), button).clicked() {
                actions.push(TransferAction::Send);
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
                style::warning()
            },
            line,
        );
    }
    actions
}
