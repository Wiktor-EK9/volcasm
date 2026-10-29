//! Menu sliding in from the left, opened by clicking the logo. Dims the
//! window behind it; clicking outside or Esc closes it.

use eframe::egui::{self, Color32, Id, Order, Pos2, Rect, Sense, vec2};

use super::style;
use crate::update::UpdateState;

/// Width of the menu, about the side bar's.
const WIDTH: f32 = 380.0;
/// Slide-in time in seconds.
const ANIMATION: f32 = 0.18;
/// Space around the menu's contents.
const MARGIN: f32 = 16.0;

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
    /// Open the credits window.
    Credits,
    /// Check for updates at start (`true`) or only on request.
    CheckUpdatesAtStart(bool),
    CheckForUpdate,
    /// Get the version found by the check.
    GetUpdate,
    /// Close the program (asks first about unsaved changes).
    Quit,
}

/// What the menu shows about updates.
pub struct UpdateView<'a> {
    /// Checked automatically at start.
    pub at_start: bool,
    pub state: &'a UpdateState,
}

/// Shows the menu while `open` (or sliding out). Sets `open` to false when
/// it should close; returns the chosen action.
/// `memory_in_mb`: the current memory meter unit, for the toggle; `logo`:
/// the program icon, shown beside the name.
pub fn show(
    ctx: &egui::Context,
    open: &mut bool,
    memory_in_mb: bool,
    updates: UpdateView<'_>,
    logo: &egui::TextureHandle,
) -> Option<MenuAction> {
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
                    egui::Frame::new().inner_margin(MARGIN).show(ui, |ui| {
                        ui.set_width(WIDTH - 2.0 * MARGIN);
                        let top = ui.cursor().top();
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            ui.add(egui::Image::new(logo).fit_to_exact_size(vec2(32.0, 32.0)));
                            ui.label(style::brand("volcasm", 30.0).color(style::ACCENT));
                        });
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
                        if let Some(chosen) = updates_section(ui, &updates) {
                            action = Some(chosen);
                        }
                        // About and Quit sit at the bottom of the menu: the
                        // gap above them fills the window. Their height is
                        // the one measured in the previous frame.
                        let bottom_id = Id::new("menu-bottom-height");
                        let bottom: f32 = ui.data(|d| d.get_temp(bottom_id)).unwrap_or(0.0);
                        let used = ui.cursor().top() - top;
                        ui.add_space((screen.height() - 2.0 * MARGIN - used - bottom).max(0.0));
                        let bottom_top = ui.cursor().top();
                        if about(ui) {
                            action = Some(MenuAction::Credits);
                        }
                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(8.0);
                        if item(ui, style::icon::SIGN_OUT, "quit") {
                            action = Some(MenuAction::Quit);
                        }
                        let height = ui.min_rect().bottom() - bottom_top;
                        ui.data_mut(|d| d.insert_temp(bottom_id, height));
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

/// The auto-check toggle and one button whose text and action follow the
/// update state: check, get the new version, or (dimmed) up to date.
fn updates_section(ui: &mut egui::Ui, view: &UpdateView<'_>) -> Option<MenuAction> {
    let mut action = None;
    ui.add_space(8.0);
    ui.label(style::label("Check for updates at start"));
    ui.horizontal(|ui| {
        for (name, value) in [("on", true), ("off", false)] {
            let text = egui::RichText::new(style::caps(name)).size(16.0);
            if ui.selectable_label(view.at_start == value, text).clicked() && view.at_start != value
            {
                action = Some(MenuAction::CheckUpdatesAtStart(value));
            }
        }
    });
    ui.add_space(8.0);
    let (icon, text, enabled, chosen) = match view.state {
        UpdateState::Checking => ("", "checking…".to_owned(), false, None),
        UpdateState::Available(release) => (
            style::icon::DOWNLOAD_SIMPLE,
            format!("download version {}", release.version()),
            true,
            Some(MenuAction::GetUpdate),
        ),
        UpdateState::UpToDate => (
            style::icon::CHECK_CIRCLE,
            "latest version installed".to_owned(),
            false,
            None,
        ),
        UpdateState::Idle | UpdateState::Failed(_) => (
            style::icon::ARROWS_CLOCKWISE,
            "check for updates".to_owned(),
            true,
            Some(MenuAction::CheckForUpdate),
        ),
    };
    let checking = *view.state == UpdateState::Checking;
    let size = 18.0;
    // While checking, a spinner stands in for the icon: spaces as wide as
    // an icon keep the text where it would be.
    let label = if checking {
        let font = egui::FontId::proportional(size);
        let width = |text: &str| {
            ui.painter()
                .layout_no_wrap(text.to_owned(), font.clone(), Color32::WHITE)
                .size()
                .x
        };
        let spaces = (width(style::icon::ARROWS_CLOCKWISE) / width(" ")).ceil() as usize;
        egui::RichText::new(format!("{} {}", " ".repeat(spaces), style::caps(&text)))
    } else {
        style::icon_label(icon, &text)
    };
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(label.size(size)).min_size(vec2(ui.available_width(), 36.0)),
    );
    if checking {
        let padding = ui.spacing().button_padding.x;
        let center = Pos2::new(
            response.rect.left() + padding + size / 2.0,
            response.rect.center().y,
        );
        egui::Spinner::new()
            .size(size)
            .paint_at(ui, Rect::from_center_size(center, vec2(size, size)));
    }
    if response.clicked() {
        action = chosen;
    }
    if let UpdateState::Failed(reason) = view.state {
        ui.label(
            style::label(&format!("Last check: {reason}"))
                .small()
                .color(style::text_dim()),
        );
    }
    action
}

const GITHUB: &str = "https://github.com/Wiktor-EK9/volcasm";
const WEBSITE: &str = "https://volcasm.wmachalica.pl";

/// Program name, version, author and links. Returns true when the
/// CREDITS button is clicked.
fn about(ui: &mut egui::Ui) -> bool {
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
    ui.button(style::icon_label(style::icon::HEART, "credits").size(16.0))
        .clicked()
}

/// Window with the credits for the work Volcasm builds on, in the middle of
/// the screen. CLOSE, Esc or a click outside sets `open` to false.
pub fn credits_dialog(ctx: &egui::Context, open: &mut bool) {
    let response = egui::Modal::new(Id::new("credits")).show(ctx, |ui| {
        ui.label(style::brand("credits", 24.0));
        ui.add_space(6.0);
        for line in [
            "SYRO: Korg Inc. (BSD-3-Clause)",
            "Fonts: Overpass, Barlow Semi Condensed, DSEG7 (SIL OFL 1.1)",
            "Icons: Phosphor (MIT)",
            "GUI: egui / eframe (MIT/Apache-2.0)",
            "Inspired by Vosyr (Frederikson Labs)",
            "and Volca Sampler (B. Wiley)",
        ] {
            ui.label(style::label(line).color(style::text_dim()));
        }
        ui.add_space(12.0);
        ui.vertical_centered(|ui| ui.button(style::label("Close")).clicked())
            .inner
    });
    if response.inner || response.should_close() {
        *open = false;
    }
}
