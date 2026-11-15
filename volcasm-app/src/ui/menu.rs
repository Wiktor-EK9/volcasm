//! Menu sliding in from the left, opened by clicking the logo. Dims the
//! window behind it; clicking outside or Esc closes it.

use eframe::egui::{self, Color32, Id, Order, Pos2, Rect, Sense, vec2};

use super::style;
use crate::settings::Theme;
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
    /// Switch to this theme; `None` follows the system.
    Theme(Option<Theme>),
    /// Show the memory meter in megabytes (`true`) or blocks.
    MemoryInMb(bool),
    /// Open the credits window.
    Credits,
    /// Check for updates at start (`true`) or only on request.
    CheckUpdatesAtStart(bool),
    CheckForUpdate,
    /// Get the version found by the check.
    GetUpdate,
    /// Close and start the installed new version.
    RestartToUpdate,
    OpenReleasePage,
    /// Send the factory samples and empty sequences to the volca.
    FactoryReset,
    /// Erase all samples on the volca.
    EraseAllSamples,
    /// Close the program (asks first about unsaved changes).
    Quit,
}

impl MenuAction {
    /// File actions change what the window shows and the volca actions
    /// open the transfer dialog, so the menu gets out of the way; settings,
    /// updates and credits keep it open.
    fn closes_menu(self) -> bool {
        matches!(
            self,
            Self::New
                | Self::Open
                | Self::Save
                | Self::SaveAs
                | Self::Quit
                | Self::FactoryReset
                | Self::EraseAllSamples
        )
    }
}

/// What the menu shows about updates.
pub struct UpdateView<'a> {
    /// Checked automatically at start.
    pub at_start: bool,
    pub state: &'a UpdateState,
}

/// Shows the menu while `open` (or sliding out). Sets `open` to false when
/// it should close (a file action, Esc, a click outside); returns the
/// chosen action.
/// `theme` and `memory_in_mb`: the current choices, for the toggles;
/// `logo`: the program icon, shown beside the name.
pub fn show(
    ctx: &egui::Context,
    open: &mut bool,
    theme: Option<Theme>,
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
                        if let Some(chosen) = theme_row(ui, theme) {
                            action = Some(MenuAction::Theme(chosen));
                        }
                        ui.add_space(8.0);
                        if let Some(chosen) = memory_row(ui, memory_in_mb) {
                            action = Some(MenuAction::MemoryInMb(chosen));
                        }
                        if let Some(chosen) = updates_section(ui, &updates) {
                            action = Some(chosen);
                        }
                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(8.0);
                        ui.label(style::label("Volca"));
                        ui.add_space(4.0);
                        for (icon, text, chosen) in [
                            (
                                style::icon::FACTORY,
                                "restore factory samples…",
                                MenuAction::FactoryReset,
                            ),
                            (
                                style::icon::TRASH,
                                "erase all samples…",
                                MenuAction::EraseAllSamples,
                            ),
                        ] {
                            if item(ui, icon, text) {
                                action = Some(chosen);
                            }
                            ui.add_space(4.0);
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

    // Esc in a window over the menu (credits) closes only that window.
    let modal_open = ctx.memory(|m| m.top_modal_layer()).is_some();
    if !modal_open && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        *open = false;
    }
    if action.is_some_and(MenuAction::closes_menu) {
        *open = false;
    }
    action
}

/// The auto-check toggle and one button whose text and action follow the
/// update state: check, get the new version, or (dimmed) up to date.
fn updates_section(ui: &mut egui::Ui, view: &UpdateView<'_>) -> Option<MenuAction> {
    let mut action = None;
    ui.add_space(8.0);
    if let Some(chosen) = updates_row(ui, view.at_start) {
        action = Some(MenuAction::CheckUpdatesAtStart(chosen));
    }
    ui.add_space(8.0);
    // Show a spinner in place of the icon.
    let mut busy = false;
    let (icon, text, enabled, chosen) = match view.state {
        UpdateState::Checking => {
            busy = true;
            ("", "checking…".to_owned(), false, None)
        }
        UpdateState::Downloading(done) => {
            busy = true;
            let text = format!("downloading… {:.0}%", done * 100.0);
            ("", text, false, None)
        }
        UpdateState::Available(release) => (
            style::icon::DOWNLOAD_SIMPLE,
            format!("download version {}", release.version()),
            true,
            Some(MenuAction::GetUpdate),
        ),
        UpdateState::Installed(_) => (
            style::icon::ARROW_COUNTER_CLOCKWISE,
            "restart to update".to_owned(),
            true,
            Some(MenuAction::RestartToUpdate),
        ),
        UpdateState::InstallFailed(..) => (
            style::icon::ARROW_SQUARE_OUT,
            "open download page".to_owned(),
            true,
            Some(MenuAction::OpenReleasePage),
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
    let size = 18.0;
    // Spaces as wide as an icon keep the text where it would be.
    let label = if busy {
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
    if busy {
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
    if let UpdateState::Failed(reason) | UpdateState::InstallFailed(_, reason) = view.state {
        ui.label(
            style::label(&format!("Last check: {reason}"))
                .small()
                .color(style::text_dim()),
        );
    }
    action
}

/// One row of toggles under a label; returns the newly chosen value.
fn choice_row<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    label: &str,
    current: T,
    options: &[(egui::RichText, T)],
) -> Option<T> {
    let mut chosen = None;
    ui.label(style::label(label));
    ui.horizontal(|ui| {
        for (text, value) in options {
            if ui
                .selectable_label(current == *value, text.clone())
                .clicked()
                && current != *value
            {
                chosen = Some(*value);
            }
        }
    });
    chosen
}

/// Dark, light or the system's theme (`None`).
fn theme_row(ui: &mut egui::Ui, current: Option<Theme>) -> Option<Option<Theme>> {
    let option = |icon: &str, name: &str, value| (style::icon_label(icon, name).size(16.0), value);
    choice_row(
        ui,
        "Theme",
        current,
        &[
            option(style::icon::MOON, "dark", Some(Theme::Dark)),
            option(style::icon::SUN, "light", Some(Theme::Light)),
            option(style::icon::MONITOR, "system", None),
        ],
    )
}

/// Memory meter in blocks or megabytes (`true`).
fn memory_row(ui: &mut egui::Ui, in_mb: bool) -> Option<bool> {
    let option = |name: &str, value| (egui::RichText::new(style::caps(name)).size(16.0), value);
    choice_row(
        ui,
        "Memory shown in",
        in_mb,
        &[option("blocks", false), option("mb", true)],
    )
}

/// Checking for a new version at start, on or off.
fn updates_row(ui: &mut egui::Ui, on: bool) -> Option<bool> {
    let option = |name: &str, value| (egui::RichText::new(style::caps(name)).size(16.0), value);
    choice_row(
        ui,
        "Check for updates at start",
        on,
        &[option("on", true), option("off", false)],
    )
}

/// What the first-start window reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WelcomeAction {
    Theme(Option<Theme>),
    MemoryInMb(bool),
    CheckUpdates(bool),
    /// START, Esc or a click outside: keep the choices shown.
    Done,
}

/// Small window at the first start: theme, memory unit and update checks,
/// showing the current choices. Each toggle is reported as it changes, so
/// the theme switches at once.
pub fn welcome_dialog(
    ctx: &egui::Context,
    logo: &egui::TextureHandle,
    theme: Option<Theme>,
    memory_in_mb: bool,
    check_updates: bool,
) -> Option<WelcomeAction> {
    let mut action = None;
    let response = egui::Modal::new(Id::new("welcome")).show(ctx, |ui| {
        ui.set_width(340.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.add(egui::Image::new(logo).fit_to_exact_size(vec2(28.0, 28.0)));
            ui.label(style::brand("welcome", 24.0));
        });
        ui.add_space(4.0);
        ui.label(
            style::label("Pick your settings. You can change them later in the menu.")
                .color(style::text_dim()),
        );
        ui.add_space(12.0);
        if let Some(chosen) = theme_row(ui, theme) {
            action = Some(WelcomeAction::Theme(chosen));
        }
        ui.add_space(8.0);
        if let Some(chosen) = memory_row(ui, memory_in_mb) {
            action = Some(WelcomeAction::MemoryInMb(chosen));
        }
        ui.add_space(8.0);
        if let Some(chosen) = updates_row(ui, check_updates) {
            action = Some(WelcomeAction::CheckUpdates(chosen));
        }
        ui.add_space(16.0);
        ui.vertical_centered(|ui| {
            ui.add(
                egui::Button::new(style::icon_label(style::icon::CHECK, "start").size(18.0))
                    .min_size(vec2(160.0, 36.0)),
            )
            .clicked()
        })
        .inner
    });
    if response.inner || response.should_close() {
        action = Some(WelcomeAction::Done);
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
    ui.label(style::label("by Wiktor-EK9").size(16.0));
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
