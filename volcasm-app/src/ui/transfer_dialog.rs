//! Transfer dialog: how to connect the volca, a loudness warning, what will
//! be sent, and the TRANSFER button. Nothing plays before that button.

use eframe::egui::{
    self, Align2, Id, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, epaint::CubicBezierShape,
    vec2,
};
use volcasm_audio::OutputInfo;

use super::style;
use super::transfer_panel::Progress;

/// Where the transfer stands.
pub enum Stage<'a> {
    /// The stream is being encoded.
    Preparing,
    /// Encoded; waiting for TRANSFER. Stream length in seconds.
    Ready(f64),
    Running(&'a Progress),
    /// Finished, cancelled or failed; see the report.
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogAction {
    Start,
    Cancel,
    Close,
    RefreshOutputs,
}

/// Shows the dialog. `summary` lists what will be sent (`true`: a
/// warning), `report` the messages of a finished transfer.
pub fn show(
    ctx: &egui::Context,
    summary: &[(bool, String)],
    stage: Stage<'_>,
    report: &[(bool, String)],
    outputs: &Result<Vec<OutputInfo>, String>,
    device: &mut Option<String>,
) -> Option<DialogAction> {
    let mut action = None;
    egui::Modal::new(Id::new("transfer-dialog")).show(ctx, |ui| {
        ui.set_width(760.0);
        ui.horizontal(|ui| {
            ui.label(style::brand("transfer", 24.0));
        });
        ui.add_space(6.0);
        ui.columns(2, |cols| {
            instructions(&mut cols[0]);
            action = details(&mut cols[1], summary, &stage, report, outputs, device);
        });
    });
    action
}

fn instructions(ui: &mut Ui) {
    ui.label(style::label(
        "Connect the audio output of the computer (headphone jack) to SYNC IN \
         on the volca with a 3.5 mm jack cable.",
    ));
    ui.add_space(6.0);
    diagram(ui);
    ui.add_space(6.0);
    for line in [
        "Set the computer volume to 100%.",
        "Turn the volca on; it receives on its own (do not play it).",
    ] {
        ui.label(style::label(&format!("· {line}")));
    }
    ui.add_space(8.0);
    egui::Frame::new()
        .fill(style::ACCENT.gamma_multiply(0.15))
        .stroke(Stroke::new(1.0, style::ACCENT))
        .corner_radius(4.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(style::caps(
                    "The transfer is a loud, harsh noise. Do not play it through \
                     speakers or headphones: at high volume it can damage equipment \
                     and hearing.",
                ))
                .color(style::text()),
            );
        });
}

/// A simple drawing: laptop, cable, volca jacks with SYNC IN highlighted.
fn diagram(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 205.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let line = Stroke::new(2.0, style::text());
    let dim = style::text_dim();

    // Laptop, top right.
    let screen = Rect::from_min_size(rect.right_top() + vec2(-150.0, 6.0), vec2(120.0, 76.0));
    painter.rect_stroke(screen, 4.0, line, StrokeKind::Inside);
    let base = [
        Pos2::new(screen.left() - 14.0, screen.bottom() + 16.0),
        Pos2::new(screen.right() + 14.0, screen.bottom() + 16.0),
        Pos2::new(screen.right(), screen.bottom()),
        Pos2::new(screen.left(), screen.bottom()),
    ];
    painter.add(Shape::closed_line(base.to_vec(), line));
    let port = Pos2::new(base[0].x + 4.0, base[0].y - 8.0);
    painter.circle_filled(port, 3.5, style::text());
    painter.text(
        port + vec2(-8.0, -4.0),
        Align2::RIGHT_BOTTOM,
        style::caps("audio out"),
        style::body_font(11.0),
        dim,
    );

    // volca top panel, bottom left, with its jacks.
    let volca = Rect::from_min_size(rect.left_bottom() + vec2(6.0, -80.0), vec2(220.0, 74.0));
    painter.rect_stroke(volca, 4.0, line, StrokeKind::Inside);
    painter.text(
        volca.right_bottom() + vec2(-8.0, -6.0),
        Align2::RIGHT_BOTTOM,
        "volca sample",
        style::brand_font(13.0),
        dim,
    );
    let jacks = ["midi in", "sync in", "sync out", "phones"];
    let mut sync_in = volca.center_top();
    for (i, name) in jacks.iter().enumerate() {
        let center = Pos2::new(volca.left() + 30.0 + i as f32 * 53.0, volca.top() + 20.0);
        let highlighted = *name == "sync in";
        let color = if highlighted {
            style::ACCENT
        } else {
            style::text()
        };
        painter.circle_stroke(center, 8.0, Stroke::new(2.0, color));
        if highlighted {
            painter.circle_filled(center, 4.0, style::ACCENT);
            sync_in = center;
        }
        painter.text(
            center + vec2(0.0, 11.0),
            Align2::CENTER_TOP,
            style::caps(name),
            style::body_font(10.0),
            if highlighted { style::ACCENT } else { dim },
        );
    }

    // Cable from the laptop down into SYNC IN.
    let cable = CubicBezierShape::from_points_stroke(
        [
            port,
            port + vec2(-90.0, 10.0),
            sync_in + vec2(10.0, -110.0),
            sync_in,
        ],
        false,
        egui::Color32::TRANSPARENT,
        Stroke::new(3.0, style::ACCENT),
    );
    painter.add(cable);
}

fn details(
    ui: &mut Ui,
    summary: &[(bool, String)],
    stage: &Stage<'_>,
    report: &[(bool, String)],
    outputs: &Result<Vec<OutputInfo>, String>,
    device: &mut Option<String>,
) -> Option<DialogAction> {
    let mut action = None;
    ui.label(style::label("Will be transferred:"));
    egui::ScrollArea::vertical()
        .max_height(170.0)
        .show(ui, |ui| {
            for (warning, line) in summary {
                if *warning {
                    ui.colored_label(style::ACCENT, line);
                } else {
                    ui.label(line);
                }
            }
        });
    ui.add_space(8.0);

    let idle = !matches!(stage, Stage::Running(_));
    ui.horizontal(|ui| {
        ui.label(style::icon_label(style::icon::SPEAKER_HIGH, "Output"));
        let selected = match (&*device, outputs) {
            (None, _) => style::caps("default"),
            (Some(id), Ok(list)) => list
                .iter()
                .find(|o| &o.id == id)
                .map_or_else(|| style::caps(id), |o| style::caps(&o.name)),
            (Some(id), Err(_)) => style::caps(id),
        };
        ui.add_enabled_ui(idle, |ui| {
            egui::ComboBox::from_id_salt("dialog-output")
                .selected_text(selected)
                .width(220.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(device, None, style::caps("default"));
                    if let Ok(list) = outputs {
                        for output in list {
                            ui.selectable_value(
                                device,
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
                action = Some(DialogAction::RefreshOutputs);
            }
        });
    });
    ui.add_space(8.0);

    let width = ui.available_width();
    let wide = |text: &str| {
        egui::Button::new(egui::RichText::new(style::caps(text)).size(18.0))
            .min_size(vec2(width, 34.0))
    };
    match stage {
        Stage::Preparing => {
            ui.label(style::label("Preparing the stream…"));
            ui.add(egui::ProgressBar::new(0.0).animate(true));
            if ui.add(wide("close (esc)")).clicked() {
                action = Some(DialogAction::Close);
            }
        }
        Stage::Ready(seconds) => {
            ui.label(style::label(&format!(
                "Transfer time {}",
                duration(*seconds)
            )));
            if ui.add(wide("transfer").fill(style::ACCENT)).clicked() {
                action = Some(DialogAction::Start);
            }
            if ui.add(wide("close (esc)")).clicked() {
                action = Some(DialogAction::Close);
            }
        }
        Stage::Running(progress) => {
            ui.label(style::label(progress.phase));
            ring(ui, progress);
            if ui.add(wide("cancel (esc)")).clicked() {
                action = Some(DialogAction::Cancel);
            }
        }
        Stage::Finished => {
            if ui.add(wide("close (esc)")).clicked() {
                action = Some(DialogAction::Close);
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
    action
}

/// Characters that fly into the ring; all exist in the DSEG font.
const DATA_CHARS: &[u8] = b"0123456789ABCDEFHJLNOPRSUYbcdhnortu-";
/// Characters in flight at once.
const PARTICLES: u64 = 18;
/// Seconds one character takes from the edge to the ring.
const FLIGHT: f64 = 1.6;
/// DSEG sizes, few so the font atlas stays small.
const PARTICLE_SIZES: [f32; 3] = [13.0, 19.0, 28.0];

/// Progress ring: the arc closes clockwise from the top, the percentage and
/// time left in the middle, random LCD characters falling into it from all
/// sides as if data were poured in.
fn ring(ui: &mut Ui, progress: &Progress) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 260.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let radius = 62.0;
    let time = ui.input(|i| i.time);

    // Characters first, so the ring stays on top.
    let outer = rect.width().max(rect.height()) * 0.5;
    for i in 0..PARTICLES {
        let clock = time / FLIGHT + i as f64 / PARTICLES as f64;
        let round = clock.floor() as u64;
        let t = clock.fract() as f32;
        let mut rng = Rng::new(i, round);
        let angle = rng.unit() * std::f32::consts::TAU;
        let size = PARTICLE_SIZES[rng.below(PARTICLE_SIZES.len())];
        let text: String = (0..1 + rng.below(3))
            .map(|_| DATA_CHARS[rng.below(DATA_CHARS.len())] as char)
            .collect();
        // Fast at the edge, slowing down near the ring.
        let eased = 1.0 - (1.0 - t).powi(2);
        let r = egui::lerp(outer..=radius + 10.0 + size * 0.8, eased);
        let alpha = (t / 0.15).min(1.0) * ((1.0 - t) / 0.35).min(1.0);
        let dir = vec2(angle.cos(), angle.sin());
        let pos = center + dir * r;
        let galley = painter.layout_no_wrap(
            text,
            style::segment_font(size),
            style::ACCENT.gamma_multiply(alpha),
        );
        // Read toward the ring: rotate around the text's own middle.
        let rot = egui::emath::Rot2::from_angle(angle + std::f32::consts::PI);
        let top_left = pos - rot * (galley.size() / 2.0);
        painter.add(
            egui::epaint::TextShape::new(top_left, galley, style::ACCENT)
                .with_angle(angle + std::f32::consts::PI),
        );
    }

    // Ring: dim track, then the done part from twelve o'clock clockwise.
    painter.circle_filled(center, radius + 6.0, style::panel());
    painter.circle_stroke(
        center,
        radius,
        Stroke::new(7.0, style::ACCENT.gamma_multiply(0.25)),
    );
    let fraction = progress.fraction.clamp(0.0, 1.0);
    if fraction > 0.0 {
        let steps = (fraction * 120.0).ceil().max(2.0) as usize;
        let points: Vec<Pos2> = (0..=steps)
            .map(|k| {
                let a = -std::f32::consts::FRAC_PI_2
                    + fraction * std::f32::consts::TAU * k as f32 / steps as f32;
                center + radius * vec2(a.cos(), a.sin())
            })
            .collect();
        painter.add(Shape::line(points, Stroke::new(7.0, style::ACCENT)));
    }
    painter.text(
        center + vec2(0.0, -6.0),
        Align2::CENTER_CENTER,
        format!("{:.0}%", fraction * 100.0),
        style::brand_font(34.0),
        style::ACCENT,
    );
    painter.text(
        center + vec2(0.0, 22.0),
        Align2::CENTER_CENTER,
        format!("{} LEFT", duration(progress.remaining_seconds)),
        style::body_font(13.0),
        style::text_dim(),
    );
    ui.ctx().request_repaint();
}

/// Small deterministic generator (SplitMix64), so each character keeps its
/// look for the whole flight without stored state.
struct Rng(u64);

impl Rng {
    fn new(particle: u64, round: u64) -> Self {
        Self(
            particle.wrapping_mul(0x9e37_79b9_7f4a_7c15)
                ^ round.wrapping_mul(0xd1b5_4a32_d192_ed03),
        )
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// 0..1.
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// "1 MIN 05 S" or "42 S".
fn duration(seconds: f64) -> String {
    let s = seconds.max(0.0).round() as u64;
    if s >= 60 {
        format!("{} MIN {:02} S", s / 60, s % 60)
    } else {
        format!("{s} S")
    }
}

#[cfg(test)]
mod tests {
    use super::duration;

    #[test]
    fn durations() {
        assert_eq!(duration(42.4), "42 S");
        assert_eq!(duration(65.0), "1 MIN 05 S");
        assert_eq!(duration(-1.0), "0 S");
    }
}
