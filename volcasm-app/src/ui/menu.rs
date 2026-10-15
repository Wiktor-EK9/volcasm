//! Menu sliding in from the left, opened by clicking the logo. Dims the
//! window behind it; clicking outside or Esc closes it.

use eframe::egui::{self, Color32, Id, Order, Pos2, Rect, Sense, vec2};

use super::style;

/// Width of the menu, about the side bar's.
const WIDTH: f32 = 380.0;
/// Slide-in time in seconds.
const ANIMATION: f32 = 0.18;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    New,
    Open,
    Save,
    SaveAs,
    /// Switch to the light theme (`true`) or the dark one.
    Theme(bool),
    /// Show the memory meter in megabytes (`true`) or blocks.
    MemoryInMb(bool),
    /// Close the program (asks first about unsaved changes).
    Quit,
}

/// Shows the menu while `open` (or sliding out). Sets `open` to false when
/// it should close; returns the chosen action.
/// `memory_in_mb`: the current memory meter unit, for the toggle.
pub fn show(ctx: &egui::Context, open: &mut bool, memory_in_mb: bool) -> Option<MenuAction> {
    let t = ctx.animate_bool_with_time(Id::new("menu-anim"), *open, ANIMATION);
    if t <= 0.0 {
        return None;
    }
    let screen = ctx.content_rect();
    let mut action = None;

    // Backdrop: dims everything, closes on click. It lives one layer below
    // the menu (Middle, still above the panels): egui moves a clicked area
    // to the top of its layer, so in the same layer a click could bring the
    // backdrop over the menu and block it.
    egui::Area::new(Id::new("menu-backdrop"))
        .order(Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter()
                .rect_filled(rect, 0.0, Color32::from_black_alpha((140.0 * t) as u8));
            if response.clicked() {
                *open = false;
            }
        });

    let x = screen.left() - WIDTH * (1.0 - t);
    egui::Area::new(Id::new("menu"))
        .order(Order::Foreground)
        .fixed_pos(Pos2::new(x, screen.top()))
        .show(ctx, |ui| {
            let rect =
                Rect::from_min_size(Pos2::new(x, screen.top()), vec2(WIDTH, screen.height()));
            ui.painter().rect_filled(rect, 0.0, style::panel());
            ui.set_min_size(rect.size());
            ui.set_max_width(WIDTH);
            // Scrolls when the window is too low for the whole menu.
            egui::ScrollArea::vertical()
                .max_height(screen.height())
                .show(ui, |ui| {
                    egui::Frame::new().inner_margin(16.0).show(ui, |ui| {
                        ui.set_width(WIDTH - 32.0);
                        ui.label(style::brand("volcasm", 30.0).color(style::ACCENT));
                        ui.add_space(16.0);
                        let item = |ui: &mut egui::Ui, icon: &str, text: &str| {
                            ui.add(
                                egui::Button::new(style::icon_label(icon, text).size(18.0))
                                    .min_size(vec2(ui.available_width(), 36.0)),
                            )
                            .clicked()
                        };
                        for (icon, text, chosen) in [
                            (style::icon::FILE_PLUS, "new file", MenuAction::New),
                            (style::icon::FOLDER_OPEN, "open…", MenuAction::Open),
                            (style::icon::FLOPPY_DISK, "save", MenuAction::Save),
                            (
                                style::icon::FLOPPY_DISK_BACK,
                                "save as…",
                                MenuAction::SaveAs,
                            ),
                        ] {
                            if item(ui, icon, text) {
                                action = Some(chosen);
                            }
                            ui.add_space(4.0);
                        }
                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(8.0);
                        ui.label(style::label("Theme"));
                        ui.horizontal(|ui| {
                            let light = style::is_light();
                            for (icon, name, value) in [
                                (style::icon::MOON, "dark", false),
                                (style::icon::SUN, "light", true),
                            ] {
                                let text = style::icon_label(icon, name).size(16.0);
                                if ui.selectable_label(light == value, text).clicked()
                                    && light != value
                                {
                                    action = Some(MenuAction::Theme(value));
                                }
                            }
                        });
                        ui.add_space(8.0);
                        ui.label(style::label("Memory shown in"));
                        ui.horizontal(|ui| {
                            for (name, value) in [("blocks", false), ("mb", true)] {
                                let text = egui::RichText::new(style::caps(name)).size(16.0);
                                if ui.selectable_label(memory_in_mb == value, text).clicked()
                                    && memory_in_mb != value
                                {
                                    action = Some(MenuAction::MemoryInMb(value));
                                }
                            }
                        });
                        about(ui);
                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(8.0);
                        if item(ui, style::icon::SIGN_OUT, "quit") {
                            action = Some(MenuAction::Quit);
                        }
                    });
                });
        });

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        *open = false;
    }
    if action.is_some() {
        *open = false;
    }
    action
}

const GITHUB: &str = "https://github.com/Wiktor-EK9/volcasm";
const WEBSITE: &str = "https://volcasm.wmachalica.pl";

/// Program name, author, links and credits for the work Volcasm builds on.
fn about(ui: &mut egui::Ui) {
    ui.add_space(16.0);
    style::subsection(ui, "about");
    ui.label(
        style::label(&format!(
            "volcasm {} · {}",
            env!("CARGO_PKG_VERSION"),
            env!("CARGO_PKG_LICENSE")
        ))
        .size(16.0),
    );
    ui.label(style::label("by Wiktor Machalica").size(16.0));
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.hyperlink_to(
            style::icon_label(style::icon::GITHUB_LOGO, "GitHub")
                .size(16.0)
                .color(style::ACCENT),
            GITHUB,
        );
        ui.add_space(12.0);
        ui.hyperlink_to(
            style::icon_label(style::icon::GLOBE, "Website")
                .size(16.0)
                .color(style::ACCENT),
            WEBSITE,
        );
    });
    ui.add_space(8.0);
    style::subsection(ui, "credits");
    for line in [
        "SYRO: Korg Inc. (BSD-3-Clause)",
        "Fonts: Overpass, Barlow Semi Condensed, DSEG7 (SIL OFL 1.1)",
        "Icons: Phosphor (MIT)",
        "GUI: egui / eframe (MIT/Apache-2.0)",
        "Inspired by Vosyr (Frederikson Labs)",
        "and Volca Sampler (B. Wiley)",
    ] {
        ui.label(style::label(line).small().color(style::text_dim()));
    }
}
