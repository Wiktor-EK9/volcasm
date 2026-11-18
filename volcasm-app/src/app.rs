//! The eframe application: layout, background jobs and side effects.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use eframe::egui::{self, ViewportCommand};
use volcasm_audio::{OutputInfo, Phase, Preview, Recorder, Transfer, TransferOptions};
use volcasm_core::sample::{Sample, import, wav};
use volcasm_core::syro::{STREAM_SAMPLE_RATE, encode};

use crate::preview::SequencePreview;
use crate::settings::{self, Settings};
use crate::state::{AppState, PartView, View, name_from_path, wav_file_name};
use crate::ui::menu::{self, MenuAction, UpdateView, WelcomeAction};
use crate::ui::record_panel::{self, RecordAction};
use crate::ui::sequences;
use crate::ui::slot_panel::{self, NameEdit, PanelAction};
use crate::ui::slots::{self, SlotAction};
use crate::ui::style;
use crate::ui::transfer_dialog::{self, DialogAction};
use crate::ui::transfer_panel::{self, Progress, TransferAction};
use crate::update::{self, DownloadEvent, Release, UpdateState};
use volcasm_core::project::Target;

/// Level below which recorded sound counts as silence: -40 dBFS.
const SILENCE_THRESHOLD: i16 = 328;

/// Audio file extensions offered when loading a sample.
const AUDIO_EXTENSIONS: [&str; 9] = [
    "wav", "aif", "aiff", "flac", "mp3", "ogg", "oga", "m4a", "aac",
];

/// `extensions` in lower and upper case for a file dialog filter: the
/// desktop portal matches its patterns case-sensitively, so `SONG.MP3`
/// would not be listed under `*.mp3`.
fn any_case(extensions: &[&str]) -> Vec<String> {
    extensions
        .iter()
        .flat_map(|e| [e.to_lowercase(), e.to_uppercase()])
        .collect()
}

/// Options from the command line.
#[derive(Debug, Default)]
pub struct Options {
    /// `.volcasm` or `.vsj` file to open at start.
    pub open: Option<PathBuf>,
    /// Slot to select at start.
    pub select: Option<usize>,
    /// Save a screenshot of the window to this file and quit (verification).
    pub screenshot: Option<PathBuf>,
    /// Start in the sequence editor.
    pub sequences: bool,
    /// Sequence to show (0-based).
    pub pattern: Option<usize>,
    /// Parts (0-based) to show with knobs or the motion editor.
    pub expand: Vec<(usize, PartView)>,
    /// Theme for this run only (light = true); not saved.
    pub theme: Option<bool>,
    /// Start with the menu open.
    pub menu: bool,
    /// Start with the credits window open.
    pub credits: bool,
    /// Start with the first-start window open, even after it was answered.
    pub welcome: bool,
    /// Start with the factory reset transfer dialog open.
    pub factory_reset: bool,
}

/// An action that discards unsaved changes, waiting for confirmation.
#[derive(Debug, Clone)]
enum Discarding {
    New,
    Open(PathBuf),
    Import(PathBuf),
    Quit,
}

struct ImportJob {
    slot: usize,
    name: String,
    /// Select the slot when loaded (the first of several files).
    select: bool,
    rx: Receiver<Result<Sample, String>>,
}

enum TransferJob {
    Encoding(Receiver<Result<Vec<[i16; 2]>, String>>),
    /// Encoded, waiting for the TRANSFER button in the dialog.
    Ready {
        frames: Vec<[i16; 2]>,
        seconds: f64,
    },
    Running {
        transfer: Transfer,
        seconds: f64,
    },
}

struct ActiveRecording {
    slot: usize,
    recorder: Recorder,
    max_seconds: f64,
    /// Displayed level, decaying.
    level: f32,
}

struct Screenshot {
    path: PathBuf,
    frame: u32,
    requested: bool,
}

pub struct VolcasmApp {
    state: AppState,
    /// Status messages, newest last; `true` = error.
    messages: Vec<(bool, String)>,
    /// Lines shown under the transfer button.
    report: Vec<(bool, String)>,
    discarding: Option<Discarding>,
    imports: Vec<ImportJob>,
    transfer: Option<TransferJob>,
    preview: Option<(usize, Preview)>,
    sequence_preview: Option<SequencePreview>,
    recording: Option<ActiveRecording>,
    outputs: Result<Vec<OutputInfo>, String>,
    inputs: Result<Vec<OutputInfo>, String>,
    name_edit: NameEdit,
    /// Project name being edited in the top bar.
    name_draft: Option<String>,
    /// Transfer dialog open, with the summary of what will be sent;
    /// `true` marks a warning.
    transfer_dialog: Option<Vec<(bool, String)>>,
    /// In the unsaved-changes dialog, DISCARD is selected (else KEEP).
    discard_focused: bool,
    settings: Settings,
    menu_open: bool,
    credits_open: bool,
    /// First-start window (theme, memory unit, updates) open.
    welcome_open: bool,
    update: UpdateState,
    update_check: Option<Receiver<Result<Release, String>>>,
    /// The release being downloaded, and the news from the download.
    update_download: Option<(Release, Receiver<DownloadEvent>)>,
    /// Start this program (the installed update) once the window closes.
    restart: Option<PathBuf>,
    /// The program icon, shown beside the logo text.
    logo: egui::TextureHandle,
    screenshot: Option<Screenshot>,
    allow_close: bool,
    /// Window title last sent.
    title: String,
}

impl VolcasmApp {
    pub fn new(cc: &eframe::CreationContext<'_>, options: Options) -> Self {
        let mut settings = Settings::load();
        if let Some(light) = options.theme {
            // For this run only: kept in memory, never saved.
            settings.theme = Some(if light {
                settings::Theme::Light
            } else {
                settings::Theme::Dark
            });
        }
        match settings.theme {
            Some(theme) => style::install(&cc.egui_ctx, theme == settings::Theme::Light),
            None => match settings::system_prefers_light() {
                // Read from the desktop (winit cannot on Wayland).
                Some(light) => style::install(&cc.egui_ctx, light),
                None => {
                    style::install(&cc.egui_ctx, false);
                    style::follow_system(&cc.egui_ctx);
                }
            },
        }
        let welcome_open = options.welcome || !settings.welcome_done;
        let mut app = Self {
            state: AppState::default(),
            messages: Vec::new(),
            report: Vec::new(),
            discarding: None,
            imports: Vec::new(),
            transfer: None,
            preview: None,
            sequence_preview: None,
            recording: None,
            outputs: list_outputs(),
            inputs: list_inputs(),
            name_edit: NameEdit::default(),
            name_draft: None,
            discard_focused: false,
            transfer_dialog: None,
            settings,
            menu_open: options.menu,
            credits_open: options.credits,
            welcome_open,
            update: UpdateState::Idle,
            update_check: None,
            update_download: None,
            restart: None,
            logo: logo_texture(&cc.egui_ctx),
            screenshot: options.screenshot.map(|path| Screenshot {
                path,
                frame: 0,
                requested: false,
            }),
            allow_close: false,
            title: String::new(),
        };
        if let Some(path) = options.open {
            app.open_any(&path);
        }
        if let Some(slot) = options.select {
            app.state.select(slot);
        }
        if options.sequences {
            app.state.view = View::Sequences;
        }
        if let Some(n) = options.pattern {
            app.state.select_pattern(n);
        }
        for (part, view) in options.expand {
            if part < app.state.part_views.len() {
                app.state.toggle_part_view(part, view);
            }
            app.state.scroll_to_part.get_or_insert(part);
        }
        if options.factory_reset {
            app.start_transfer(Target::factory_reset(app.state.project.device()));
        }
        // At the first start the check waits for the user's choice.
        if app.settings.check_updates && !app.welcome_open {
            app.check_for_update(&cc.egui_ctx);
        }
        app
    }

    /// Switches to `theme` (`None`: the system's) and remembers it.
    fn set_theme(&mut self, ctx: &egui::Context, theme: Option<settings::Theme>) {
        match theme {
            Some(theme) => style::set_light(ctx, theme == settings::Theme::Light),
            None => match settings::system_prefers_light() {
                Some(light) => style::set_light(ctx, light),
                None => style::follow_system(ctx),
            },
        }
        self.settings.theme = theme;
        self.save_settings();
    }

    fn set_memory_in_mb(&mut self, mb: bool) {
        self.settings.memory_unit = if mb {
            settings::MemoryUnit::Mb
        } else {
            settings::MemoryUnit::Blocks
        };
        self.save_settings();
    }

    fn save_settings(&mut self) {
        if let Err(err) = self.settings.save() {
            self.error(format!("{}: {err}", style::caps("Cannot save settings")));
        }
    }

    fn welcome(&mut self, ctx: &egui::Context) {
        let in_mb = self.settings.memory_unit == settings::MemoryUnit::Mb;
        let chosen = menu::welcome_dialog(
            ctx,
            &self.logo,
            self.settings.theme,
            in_mb,
            self.settings.check_updates,
        );
        match chosen {
            Some(WelcomeAction::Theme(theme)) => self.set_theme(ctx, theme),
            Some(WelcomeAction::MemoryInMb(mb)) => self.set_memory_in_mb(mb),
            Some(WelcomeAction::CheckUpdates(on)) => {
                self.settings.check_updates = on;
                self.save_settings();
            }
            Some(WelcomeAction::Done) => {
                self.welcome_open = false;
                self.settings.welcome_done = true;
                self.save_settings();
                if self.settings.check_updates
                    && matches!(self.update, UpdateState::Idle | UpdateState::Failed(_))
                {
                    self.check_for_update(ctx);
                }
            }
            None => {}
        }
    }

    fn info(&mut self, text: impl Into<String>) {
        self.messages.push((false, text.into()));
    }

    fn error(&mut self, text: impl Into<String>) {
        self.messages.push((true, text.into()));
    }

    /// Opens a `.volcasm` or imports a `.vsj`, by extension.
    fn open_any(&mut self, path: &Path) {
        let is_vsj = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("vsj"));
        if is_vsj {
            self.import_vsj(path)
        } else {
            self.open(path)
        }
    }

    fn open(&mut self, path: &Path) {
        match AppState::open(path) {
            Ok(state) => {
                self.replace_state(state);
                self.info(format!("{} {}", style::caps("Opened"), path.display()));
            }
            Err(err) => self.error(format!(
                "{} {}: {err}",
                style::caps("Cannot open"),
                path.display()
            )),
        }
    }

    fn import_vsj(&mut self, path: &Path) {
        match AppState::import_vsj(path) {
            Ok((state, errors)) => {
                self.replace_state(state);
                self.info(format!("{} {}", style::caps("Imported"), path.display()));
                for err in errors {
                    self.error(err);
                }
            }
            Err(err) => self.error(format!(
                "{} {}: {err}",
                style::caps("Cannot import"),
                path.display()
            )),
        }
    }

    fn replace_state(&mut self, state: AppState) {
        let transfer = self.state.transfer.clone();
        let record = self.state.record.clone();
        self.state = state;
        self.state.transfer = transfer;
        self.state.record = record;
        self.recording = None;
        self.preview = None;
        self.sequence_preview = None;
        self.report.clear();
    }

    /// Runs `action` now, or asks first if there are unsaved changes.
    fn discard_then(&mut self, action: Discarding) {
        if self.state.dirty {
            self.discarding = Some(action);
        } else {
            self.discard(action);
        }
    }

    fn discard(&mut self, action: Discarding) {
        match action {
            Discarding::New => {
                self.replace_state(AppState::default());
                self.info(style::caps("New project"));
            }
            Discarding::Open(path) => self.open(&path),
            Discarding::Import(path) => self.import_vsj(&path),
            Discarding::Quit => self.allow_close = true,
        }
    }

    fn save(&mut self, choose: bool) {
        let path = if choose || self.state.path.is_none() {
            let name = format!("{}.volcasm", self.state.project.name);
            match rfd::FileDialog::new()
                .add_filter("Volcasm project", &["volcasm"])
                .set_file_name(name)
                .save_file()
            {
                Some(path) => Some(path.with_extension("volcasm")),
                None => return,
            }
        } else {
            None
        };
        match self.state.save(path.as_deref()) {
            Ok(()) => {
                let shown = self.state.path.as_ref().map(|p| p.display().to_string());
                self.info(format!(
                    "{} {}",
                    style::caps("Saved"),
                    shown.unwrap_or_default()
                ));
            }
            Err(err) => self.error(format!("{}: {err}", style::caps("Cannot save"))),
        }
    }

    /// Imports an audio file into `slot` in the background.
    fn load_sample(&mut self, slot: usize, path: PathBuf, select: bool) {
        let rate = self.state.project.device().sample_rate;
        let (tx, rx) = mpsc::channel();
        let name = name_from_path(&path);
        std::thread::spawn(move || {
            let _ = tx.send(import::import_file(&path, rate).map_err(|e| e.to_string()));
        });
        self.info(format!("{} {name} → {slot}", style::caps("Loading")));
        self.imports.push(ImportJob {
            slot,
            name,
            select,
            rx,
        });
    }

    /// Imports audio files: the first (by file name) into `first`, the
    /// others into the nearest free slots after it.
    fn load_samples(&mut self, first: usize, mut paths: Vec<PathBuf>) {
        paths.sort();
        let busy: Vec<usize> = self.imports.iter().map(|job| job.slot).collect();
        let slots = self.state.slots_for_files(first, paths.len(), &busy);
        let skipped = paths.len() - slots.len();
        for (i, (slot, path)) in slots.into_iter().zip(paths).enumerate() {
            self.load_sample(slot, path, i == 0);
        }
        if skipped > 0 {
            self.error(style::caps(&format!(
                "No free slot for {skipped} of the files"
            )));
        }
    }

    fn pick_sample(&mut self, slot: usize) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Audio", &any_case(&AUDIO_EXTENSIONS))
            .pick_files()
        {
            self.load_samples(slot, paths);
        }
    }

    /// Asks where to save the sample of `slot` and writes it as a WAV.
    fn export_sample(&mut self, slot: usize) {
        let Some(filled) = self.state.project.slot(slot) else {
            return;
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("WAV", &["wav"])
            .set_file_name(wav_file_name(slot, &filled.name))
            .save_file()
        else {
            return;
        };
        let has_wav = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("wav"));
        let path = if has_wav {
            path
        } else {
            path.with_extension("wav")
        };
        match wav::save(&filled.sample, &path) {
            Ok(()) => self.info(format!("{} {}", style::caps("Exported"), path.display())),
            Err(err) => self.error(format!("{}: {err}", style::caps("Cannot export"))),
        }
    }

    fn poll_imports(&mut self) {
        let mut done = Vec::new();
        for (i, job) in self.imports.iter().enumerate() {
            if let Ok(result) = job.rx.try_recv() {
                done.push((i, result));
            }
        }
        for (i, result) in done.into_iter().rev() {
            let job = self.imports.remove(i);
            match result
                .and_then(|sample| self.state.set_sample(job.slot, job.name.clone(), sample))
            {
                Ok(()) => {
                    self.info(format!(
                        "{} {} → {}",
                        style::caps("Loaded"),
                        job.name,
                        job.slot
                    ));
                    if job.select {
                        self.state.select(job.slot);
                    }
                }
                Err(err) => self.error(format!(
                    "{} {}: {err}",
                    style::caps("Cannot load"),
                    job.name
                )),
            }
        }
    }

    /// What `targets` will send, for the transfer dialog.
    fn transfer_summary(&self, targets: &[Target]) -> Vec<(bool, String)> {
        let mut lines = Vec::new();
        let mut erased = Vec::new();
        let mut patterns = Vec::new();
        let mut empty_patterns = Vec::new();
        for target in targets {
            match *target {
                Target::Sample(n) => {
                    let name = self.state.project.slot(n).map_or("", |s| s.name.as_str());
                    lines.push((false, format!("{} {n}: {name}", style::caps("slot"))));
                }
                Target::Erase(n) => erased.push(n.to_string()),
                Target::Pattern(n) => patterns.push((n + 1).to_string()),
                Target::EmptyPattern(n) => empty_patterns.push((n + 1).to_string()),
                Target::FactorySamples => {
                    lines.push((false, style::caps("factory samples in all slots")));
                    lines.push((
                        true,
                        style::caps(
                            "Replaces every sample on the volca. They cannot be read back: \
                             keep a project with the ones you need.",
                        ),
                    ));
                }
                Target::EraseAllSamples => lines.push((
                    true,
                    style::caps(
                        "Erases every sample on the volca. They cannot be read back: \
                         keep a project with the ones you need.",
                    ),
                )),
            }
        }
        if !erased.is_empty() {
            lines.push((
                false,
                format!("{} {}", style::caps("erase slots"), erased.join(", ")),
            ));
        }
        if !patterns.is_empty() {
            lines.push((
                false,
                format!("{} {}", style::caps("sequences"), patterns.join(", ")),
            ));
        }
        if !empty_patterns.is_empty() {
            lines.push((
                false,
                format!(
                    "{} {}",
                    style::caps("empty sequences"),
                    empty_patterns.join(", ")
                ),
            ));
            lines.push((
                true,
                style::caps("Replaces the sequences on the volca with empty ones."),
            ));
        }
        lines
    }

    /// Opens the transfer dialog and encodes the stream in the background.
    /// Nothing plays until TRANSFER is pressed in the dialog.
    fn start_transfer(&mut self, targets: Vec<Target>) {
        if targets.is_empty() || self.transfer.is_some() {
            return;
        }
        self.transfer_dialog = Some(self.transfer_summary(&targets));
        self.report.clear();
        let project = self.state.project.clone();
        let bits = Some(crate::state::TRANSFER_BITS);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = project
                .syro_items(&targets, bits)
                .map_err(|e| e.to_string())
                .and_then(|items| encode(&items).map_err(|e| e.to_string()));
            let _ = tx.send(result);
        });
        self.transfer = Some(TransferJob::Encoding(rx));
    }

    /// Advances the transfer job; returns its progress for display.
    fn poll_transfer(&mut self) -> Option<Progress> {
        match self.transfer.take()? {
            TransferJob::Encoding(rx) => match rx.try_recv() {
                Ok(Ok(frames)) => {
                    let seconds = frames.len() as f64 / f64::from(STREAM_SAMPLE_RATE);
                    self.transfer = Some(TransferJob::Ready { frames, seconds });
                    None
                }
                Ok(Err(err)) => {
                    self.report.push((true, err));
                    None
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.transfer = Some(TransferJob::Encoding(rx));
                    Some(progress("encoding", 0.0, 0.0))
                }
                Err(mpsc::TryRecvError::Disconnected) => None,
            },
            ready @ TransferJob::Ready { .. } => {
                self.transfer = Some(ready);
                None
            }
            TransferJob::Running { transfer, seconds } => {
                if transfer.phase() == Phase::Done {
                    self.finish_transfer(transfer);
                    return None;
                }
                let (sent, total) = transfer.progress();
                let fraction = sent as f32 / total.max(1) as f32;
                let phase = match transfer.phase() {
                    Phase::Checking => "checking audio path",
                    Phase::Sending => "sending",
                    Phase::Draining | Phase::Done => "finishing",
                };
                let shown = progress(phase, fraction, seconds * f64::from(1.0 - fraction));
                self.transfer = Some(TransferJob::Running { transfer, seconds });
                Some(shown)
            }
        }
    }

    /// TRANSFER pressed: starts playing the encoded stream.
    fn begin_transfer(&mut self) {
        if let Some(TransferJob::Ready { frames, seconds }) = self.transfer.take() {
            // Previews share the output with the transfer and would corrupt it.
            self.preview = None;
            self.sequence_preview = None;
            let options = TransferOptions {
                device: self.state.transfer.device.clone(),
            };
            let transfer = volcasm_audio::start(frames, options);
            self.transfer = Some(TransferJob::Running { transfer, seconds });
        }
    }

    /// Closes the dialog; an encoded stream that was not sent is dropped. A
    /// running transfer keeps the dialog open.
    fn close_transfer_dialog(&mut self) {
        match self.transfer {
            Some(TransferJob::Running { .. }) => {}
            _ => {
                self.transfer = None;
                self.transfer_dialog = None;
            }
        }
    }

    fn transfer_dialog(&mut self, ctx: &egui::Context, progress: Option<&Progress>) {
        let Some(summary) = self.transfer_dialog.clone() else {
            return;
        };
        let stage = match (&self.transfer, progress) {
            (Some(TransferJob::Encoding(_)), _) => transfer_dialog::Stage::Preparing,
            (Some(TransferJob::Ready { seconds, .. }), _) => {
                transfer_dialog::Stage::Ready(*seconds)
            }
            (Some(TransferJob::Running { .. }), Some(p)) => transfer_dialog::Stage::Running(p),
            (Some(TransferJob::Running { .. }), None) => transfer_dialog::Stage::Preparing,
            (None, _) => transfer_dialog::Stage::Finished,
        };
        let action = transfer_dialog::show(
            ctx,
            &summary,
            stage,
            &self.report,
            &self.outputs,
            &mut self.state.transfer.device,
        );
        match action {
            Some(DialogAction::Start) => self.begin_transfer(),
            Some(DialogAction::Cancel) => {
                if let Some(TransferJob::Running { transfer, .. }) = &self.transfer {
                    transfer.cancel();
                }
            }
            Some(DialogAction::Close) => self.close_transfer_dialog(),
            Some(DialogAction::RefreshOutputs) => self.outputs = list_outputs(),
            None => {}
        }
    }

    fn finish_transfer(&mut self, transfer: Transfer) {
        match transfer.wait() {
            Ok(report) => {
                // Resampling on a 48 kHz-only device is expected and works
                // (tested on hardware), so it is not shown.
                for warning in &report.warnings {
                    if !matches!(warning, volcasm_audio::check::Warning::Resampled { .. }) {
                        self.report.push((false, warning.to_string()));
                    }
                }
                if report.xruns > 0 {
                    self.report.push((
                        true,
                        style::caps(&format!(
                            "{} buffer underruns: the transfer is likely corrupted",
                            report.xruns
                        )),
                    ));
                }
                let text = style::caps(if report.cancelled {
                    "Transfer cancelled"
                } else {
                    "Transfer done"
                });
                self.report.push((false, text.clone()));
                self.info(text);
            }
            Err(err) => {
                self.report.push((true, err.to_string()));
                self.error(err.to_string());
            }
        }
    }

    fn start_recording(&mut self, slot: usize) {
        let device = self.state.project.device();
        // Longer would not fit into the device memory anyway.
        let max_seconds = device.max_sample_seconds();
        match Recorder::start(self.state.record.input.as_deref(), max_seconds) {
            Ok(recorder) => {
                self.recording = Some(ActiveRecording {
                    slot,
                    recorder,
                    max_seconds,
                    level: 0.0,
                });
                self.info(style::caps(&format!("Recording into slot {slot}")));
            }
            Err(err) => self.error(format!("{}: {err}", style::caps("Cannot record"))),
        }
    }

    /// Stops recording and converts the audio in the background.
    fn stop_recording(&mut self) {
        let Some(active) = self.recording.take() else {
            return;
        };
        let recording = active.recorder.finish();
        let rate = self.state.project.device().sample_rate;
        let trim = self.state.record.trim_silence;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let interleaved: Vec<f64> = recording
                .interleaved
                .iter()
                .map(|&x| f64::from(x))
                .collect();
            let result = import::convert(&interleaved, recording.channels, recording.rate, rate)
                .map(|s| {
                    if trim {
                        volcasm_core::sample::edit::trim_silence(&s, SILENCE_THRESHOLD)
                    } else {
                        s
                    }
                })
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
        self.imports.push(ImportJob {
            slot: active.slot,
            name: "Recording".into(),
            select: true,
            rx,
        });
    }

    fn poll_recording(&mut self) -> Option<record_panel::Recording> {
        let active = self.recording.as_mut()?;
        active.recorder.poll();
        active.level = active.recorder.level().max(active.level * 0.85);
        let view = record_panel::Recording {
            slot: active.slot,
            seconds: active.recorder.seconds(),
            max_seconds: active.max_seconds,
            level: active.level,
        };
        if active.recorder.is_full() {
            self.info(style::caps("Recording reached the device memory size"));
            self.stop_recording();
        }
        Some(view)
    }

    fn toggle_sequence_preview(&mut self) {
        if self.sequence_preview.take().is_some() {
            return;
        }
        match SequencePreview::start(&self.state) {
            Ok(player) => self.sequence_preview = Some(player),
            Err(err) => self.error(format!("{}: {err}", style::caps("Cannot play"))),
        }
    }

    fn toggle_preview(&mut self, slot: usize) {
        if self.preview.as_ref().is_some_and(|(n, _)| *n == slot) {
            self.preview = None;
            return;
        }
        let Some(s) = self.state.project.slot(slot) else {
            return;
        };
        let pcm: Arc<[i16]> = s.sample.pcm.clone().into();
        match volcasm_audio::preview(pcm, s.sample.sample_rate) {
            Ok(preview) => self.preview = Some((slot, preview)),
            Err(err) => self.error(format!("{}: {err}", style::caps("Cannot play"))),
        }
    }

    /// A tile dragged onto another slot: move, or swap with a filled one.
    fn move_slot(&mut self, from: usize, to: usize) {
        // A loading file would land in the wrong slot.
        if self
            .imports
            .iter()
            .any(|job| job.slot == from || job.slot == to)
        {
            self.error(style::caps("Wait until the file has loaded"));
            return;
        }
        let swap = self.state.project.slot(to).is_some();
        if !self.state.move_slot(from, to) {
            return;
        }
        // The preview follows its sample.
        if let Some((slot, _)) = &mut self.preview {
            if *slot == from {
                *slot = to;
            } else if *slot == to {
                *slot = from;
            }
        }
        let text = if swap {
            format!("{} {from} ↔ {to}", style::caps("Swapped"))
        } else {
            format!("{} {from} → {to}", style::caps("Moved"))
        };
        self.info(text);
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        let mut audio = Vec::new();
        for path in dropped {
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            match ext.as_str() {
                "volcasm" => self.discard_then(Discarding::Open(path)),
                "vsj" => self.discard_then(Discarding::Import(path)),
                _ => audio.push(path),
            }
        }
        if audio.is_empty() {
            return;
        }
        let busy: Vec<usize> = self.imports.iter().map(|job| job.slot).collect();
        let first = self.state.selected.or_else(|| {
            (0..self.state.slot_count())
                .find(|&n| self.state.project.slot(n).is_none() && !busy.contains(&n))
        });
        match first {
            Some(first) => self.load_samples(first, audio),
            None => self.error(style::caps("No free slot for the dropped files")),
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let mut wrapped = true;
        let in_mb = self.settings.memory_unit == settings::MemoryUnit::Mb;
        ui.horizontal(|ui| {
            // The logo (icon and name) opens the menu.
            let group = ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.add(egui::Image::new(&self.logo).fit_to_exact_size(egui::vec2(32.0, 32.0)));
                ui.label(style::brand("volcasm", 30.0).color(style::ACCENT));
            });
            let logo = ui
                .interact(
                    group.response.rect,
                    ui.id().with("logo"),
                    egui::Sense::click(),
                )
                .on_hover_text(style::caps("menu"));
            if logo.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if logo.clicked() {
                self.menu_open = true;
            }
            ui.add_space(8.0);
            self.project_name(ui);
            ui.add_space(12.0);
            for (view, name) in [(View::Samples, "samples"), (View::Sequences, "sequences")] {
                let text = egui::RichText::new(style::caps(name)).size(18.0);
                if ui.selectable_label(self.state.view == view, text).clicked() {
                    self.state.view = view;
                }
            }
            ui.add_space(12.0);
            // Meters beside the buttons when they fit, else on a second line.
            if ui.available_width() >= METERS_WIDTH {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    meters(ui, &self.state, in_mb);
                });
                wrapped = false;
            }
        });
        if wrapped {
            ui.add_space(4.0);
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 24.0),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| meters(ui, &self.state, in_mb),
            );
        }
    }

    /// Project name: click to edit, Enter or clicking away to keep, Esc to
    /// cancel. Shown in the brand style; edited as typed.
    fn project_name(&mut self, ui: &mut egui::Ui) {
        if let Some(text) = &mut self.name_draft {
            let response = ui.add(
                egui::TextEdit::singleline(text)
                    .font(style::brand_font(20.0))
                    .desired_width(240.0),
            );
            if !response.has_focus() && !response.lost_focus() {
                response.request_focus();
            }
            let cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
            if cancel {
                self.name_draft = None;
            } else if response.lost_focus() {
                let name = text.trim().to_owned();
                self.name_draft = None;
                if !name.is_empty() && name != self.state.project.name {
                    self.state.project.name = name;
                    self.state.dirty = true;
                }
            }
            return;
        }
        let response = ui
            .add(
                egui::Label::new(style::brand(&self.state.project.name, 20.0))
                    .sense(egui::Sense::click()),
            )
            .on_hover_text(style::caps("click to rename the project"));
        if self.state.dirty {
            // A red dot, as the unsaved marker of many editors.
            let (rect, dot) = ui.allocate_exact_size(egui::vec2(10.0, 20.0), egui::Sense::hover());
            ui.painter()
                .circle_filled(rect.center(), 4.5, style::ACCENT);
            dot.on_hover_text(style::caps("unsaved changes"));
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
        if response.clicked() {
            self.name_draft = Some(self.state.project.name.clone());
        }
    }

    /// Asks for a `.volcasm` or `.vsj` file and opens it.
    fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Volcasm or Vosyr project", &any_case(&["volcasm", "vsj"]))
            .add_filter("Volcasm project", &any_case(&["volcasm"]))
            .add_filter("Vosyr project", &any_case(&["vsj"]))
            .pick_file()
        {
            let is_vsj = path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("vsj"));
            self.discard_then(if is_vsj {
                Discarding::Import(path)
            } else {
                Discarding::Open(path)
            });
        }
    }

    fn check_for_update(&mut self, ctx: &egui::Context) {
        self.update = UpdateState::Checking;
        self.update_check = Some(update::check(ctx));
    }

    fn poll_update_check(&mut self) {
        let Some(result) = self.update_check.as_ref().and_then(|rx| rx.try_recv().ok()) else {
            return;
        };
        self.update_check = None;
        self.update = match result {
            Ok(release) if update::is_newer(update::CURRENT, release.version()) => {
                UpdateState::Available(release)
            }
            Ok(_) => UpdateState::UpToDate,
            Err(reason) => UpdateState::Failed(reason),
        };
    }

    fn poll_update_download(&mut self) {
        let Some((release, rx)) = &self.update_download else {
            return;
        };
        let mut done = None;
        for event in rx.try_iter() {
            match event {
                DownloadEvent::Progress(fraction) => {
                    self.update = UpdateState::Downloading(fraction)
                }
                DownloadEvent::Done(result) => done = Some(result),
            }
        }
        if let Some(result) = done {
            self.update = match result {
                Ok(path) => UpdateState::Installed(path),
                Err(reason) => UpdateState::InstallFailed(release.clone(), reason),
            };
            self.update_download = None;
        }
    }

    fn menu(&mut self, ctx: &egui::Context) {
        let in_mb = self.settings.memory_unit == settings::MemoryUnit::Mb;
        let updates = UpdateView {
            at_start: self.settings.check_updates,
            state: &self.update,
        };
        let theme = self.settings.theme;
        match menu::show(ctx, &mut self.menu_open, theme, in_mb, updates, &self.logo) {
            Some(MenuAction::New) => self.discard_then(Discarding::New),
            Some(MenuAction::Open) => self.open_dialog(),
            Some(MenuAction::Save) => self.save(false),
            Some(MenuAction::SaveAs) => self.save(true),
            // Same path as closing the window: asks about unsaved changes.
            Some(MenuAction::Quit) => ctx.send_viewport_cmd(ViewportCommand::Close),
            Some(MenuAction::Credits) => self.credits_open = true,
            Some(MenuAction::FactoryReset) => {
                self.start_transfer(Target::factory_reset(self.state.project.device()))
            }
            Some(MenuAction::EraseAllSamples) => self.start_transfer(vec![Target::EraseAllSamples]),
            Some(MenuAction::CheckUpdatesAtStart(on)) => {
                self.settings.check_updates = on;
                self.save_settings();
                if on && matches!(self.update, UpdateState::Idle | UpdateState::Failed(_)) {
                    self.check_for_update(ctx);
                }
            }
            Some(MenuAction::CheckForUpdate) => self.check_for_update(ctx),
            Some(MenuAction::GetUpdate) => {
                if let UpdateState::Available(release) = &self.update {
                    match update::install_target() {
                        Some(target) => {
                            let rx = update::download(ctx, release, target);
                            self.update_download = Some((release.clone(), rx));
                            self.update = UpdateState::Downloading(0.0);
                        }
                        // macOS, or not run from an AppImage.
                        None => ctx.open_url(egui::OpenUrl::new_tab(&release.page)),
                    }
                }
            }
            Some(MenuAction::OpenReleasePage) => {
                if let UpdateState::InstallFailed(release, _) = &self.update {
                    ctx.open_url(egui::OpenUrl::new_tab(&release.page));
                }
            }
            Some(MenuAction::RestartToUpdate) => {
                if let UpdateState::Installed(path) = &self.update {
                    self.restart = Some(path.clone());
                    // Same path as Quit: asks about unsaved changes first.
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
            Some(MenuAction::Theme(theme)) => self.set_theme(ctx, theme),
            Some(MenuAction::MemoryInMb(mb)) => self.set_memory_in_mb(mb),
            None => {}
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            match self.messages.last() {
                Some((true, text)) => {
                    ui.colored_label(style::ACCENT, text);
                }
                Some((false, text)) => {
                    ui.label(egui::RichText::new(text).color(style::text_dim()));
                }
                None => {
                    ui.label(egui::RichText::new(style::caps("ready")).color(style::text_dim()));
                }
            }
            // A new version: only this note, the menu has the button.
            if matches!(
                self.update,
                UpdateState::Available(_)
                    | UpdateState::Downloading(_)
                    | UpdateState::Installed(_)
                    | UpdateState::InstallFailed(..)
            ) {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let note = egui::Label::new(
                        egui::RichText::new(style::caps("update available")).color(style::ACCENT),
                    )
                    .sense(egui::Sense::click());
                    if ui
                        .add(note)
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.menu_open = true;
                    }
                });
            }
        });
    }

    fn confirm_discard(&mut self, ctx: &egui::Context) {
        let Some(action) = self.discarding.clone() else {
            return;
        };
        // Keyboard: arrows or Tab move between the buttons, Enter confirms,
        // Y discards, N or Esc keeps editing.
        let mut choice = ctx.input_mut(|i| {
            use egui::{Key, Modifiers};
            if i.consume_key(Modifiers::NONE, Key::ArrowLeft)
                || i.consume_key(Modifiers::NONE, Key::ArrowRight)
                || i.consume_key(Modifiers::NONE, Key::Tab)
            {
                self.discard_focused = !self.discard_focused;
            }
            if i.consume_key(Modifiers::NONE, Key::Y) {
                Some(true)
            } else if i.consume_key(Modifiers::NONE, Key::N)
                || i.consume_key(Modifiers::NONE, Key::Escape)
            {
                Some(false)
            } else if i.consume_key(Modifiers::NONE, Key::Enter) {
                Some(self.discard_focused)
            } else {
                None
            }
        });
        let focused = self.discard_focused;
        egui::Modal::new(egui::Id::new("discard")).show(ctx, |ui| {
            ui.label(style::brand("unsaved changes", 20.0));
            ui.label(style::label("Discard the changes in this project?"));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let button = |text: &str, selected: bool| {
                    let button = egui::Button::new(style::label(text));
                    if selected {
                        button.fill(style::ACCENT)
                    } else {
                        button
                    }
                };
                if ui.add(button("Discard (Y)", focused)).clicked() {
                    choice = Some(true);
                }
                if ui.add(button("Keep editing (N)", !focused)).clicked() {
                    choice = Some(false);
                }
            });
            ui.label(
                egui::RichText::new(style::caps("← → to choose, enter to confirm"))
                    .small()
                    .color(style::text_dim()),
            );
        });
        if choice.is_some() {
            // The safe choice is selected the next time.
            self.discard_focused = false;
        }
        match choice {
            Some(true) => {
                self.discarding = None;
                self.discard(action);
                if self.allow_close {
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
            Some(false) => {
                if matches!(action, Discarding::Quit) {
                    // Not closing, so not restarting either.
                    self.restart = None;
                }
                self.discarding = None;
            }
            None => {}
        }
    }

    /// Space plays or stops the selected slot (SAMPLES) or the sequence
    /// (SEQUENCES); Esc cancels a running transfer. Not while typing or
    /// while the menu or a dialog is open.
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        use egui::{Key, Modifiers};
        if self.transfer_dialog.is_some() {
            // In the transfer dialog Esc closes it, or cancels a running
            // transfer.
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                match &self.transfer {
                    Some(TransferJob::Running { transfer, .. }) => transfer.cancel(),
                    _ => self.close_transfer_dialog(),
                }
            }
            return;
        }
        if self.discarding.is_some()
            || self.menu_open
            || self.credits_open
            || self.welcome_open
            || ctx.text_edit_focused()
        {
            return;
        }
        // Consumed here, so a focused button is not clicked by it as well.
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Space)) {
            match self.state.view {
                View::Samples => {
                    if let Some(slot) = self.state.selected {
                        self.toggle_preview(slot);
                    } else {
                        self.preview = None;
                    }
                }
                View::Sequences => {
                    if self.transfer.is_none() || self.sequence_preview.is_some() {
                        self.toggle_sequence_preview();
                    }
                }
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
            && let Some(TransferJob::Running { transfer, .. }) = &self.transfer
        {
            transfer.cancel();
            self.info(style::caps("Cancelling transfer"));
        }
    }

    fn handle_close(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) {
            return;
        }
        if !self.allow_close {
            let busy = matches!(self.transfer, Some(TransferJob::Running { .. }));
            if self.state.dirty || busy {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                self.discarding = Some(Discarding::Quit);
                return;
            }
        }
        // The window closes now: start the updated program.
        if let Some(path) = self.restart.take()
            && let Err(err) = std::process::Command::new(&path).spawn()
        {
            eprintln!("cannot start {}: {err}", path.display());
        }
    }

    fn screenshot_step(&mut self, ctx: &egui::Context) {
        let Some(shot) = &mut self.screenshot else {
            return;
        };
        ctx.request_repaint();
        shot.frame += 1;
        if shot.frame == 10 && !shot.requested {
            shot.requested = true;
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
        }
        let image = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            let [w, h] = image.size;
            let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            let result = image::RgbaImage::from_raw(w as u32, h as u32, bytes)
                .ok_or_else(|| "invalid screenshot".to_string())
                .and_then(|img| img.save(&shot.path).map_err(|e| e.to_string()));
            match result {
                Ok(()) => eprintln!("screenshot saved to {}", shot.path.display()),
                Err(err) => eprintln!("screenshot failed: {err}"),
            }
            self.allow_close = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }
}

fn progress(phase: &'static str, fraction: f32, remaining_seconds: f64) -> Progress {
    Progress {
        phase,
        fraction,
        remaining_seconds,
    }
}

/// The bundled icon as a texture, mipmapped so it stays smooth when drawn
/// small.
fn logo_texture(ctx: &egui::Context) -> egui::TextureHandle {
    let icon = crate::icon();
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [icon.width as usize, icon.height as usize],
        &icon.rgba,
    );
    let options = egui::TextureOptions {
        mipmap_mode: Some(egui::TextureFilter::Linear),
        ..egui::TextureOptions::LINEAR
    };
    ctx.load_texture("logo", image, options)
}

fn list_inputs() -> Result<Vec<OutputInfo>, String> {
    volcasm_audio::list_inputs().map_err(|e| format!("Cannot list audio inputs: {e}"))
}

fn list_outputs() -> Result<Vec<OutputInfo>, String> {
    volcasm_audio::list_outputs().map_err(|e| format!("Cannot list audio outputs: {e}"))
}

/// Width the two meters need side by side.
const METERS_WIDTH: f32 = 2.0 * METER_WIDTH + 16.0;
const METER_WIDTH: f32 = 228.0;

/// The memory and duration meters, right-aligned in the top bar.
/// `in_mb`: memory in megabytes instead of device blocks.
fn meters(ui: &mut egui::Ui, state: &AppState, in_mb: bool) {
    let width = METER_WIDTH;
    let device = state.project.device();

    let memory = state.memory();
    let text = if in_mb {
        // Whole blocks, as the device fills its memory.
        let mb = |blocks: u64| blocks as f64 * f64::from(device.memory.block_size) / 1e6;
        format!(
            "{} MEMORY {:.2} / {:.2} MB",
            style::icon::MEMORY,
            mb(memory.used_blocks),
            mb(memory.total_blocks)
        )
    } else {
        format!(
            "{} MEMORY {} / {} BLK · {:.2} MB",
            style::icon::MEMORY,
            memory.used_blocks,
            memory.total_blocks,
            memory.used_blocks as f64 * f64::from(device.memory.block_size) / 1e6
        )
    };
    let fraction = memory.used_blocks as f32 / memory.total_blocks.max(1) as f32;
    meter(ui, width, fraction, &text, memory.fits());

    // The memory meter decides what fits (blocks round up), so the duration
    // meter only turns red with it.
    let seconds = state.project.total_duration();
    let max = device.max_sample_seconds();
    let text = format!("{} DURATION {seconds:.2} / {max:.1} S", style::icon::TIMER);
    meter(ui, width, (seconds / max) as f32, &text, memory.fits());
}

/// A horizontal bar with centered text; red when `ok` is false.
fn meter(ui: &mut egui::Ui, width: f32, fraction: f32, text: &str, ok: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 22.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, style::background());
    let mut fill = rect;
    fill.set_width(rect.width() * fraction.clamp(0.0, 1.0));
    let color = if ok {
        style::tile_filled()
    } else {
        style::ACCENT
    };
    painter.rect_filled(fill, 3.0, color);
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        style::body_font(14.0),
        style::text(),
    );
}

impl eframe::App for VolcasmApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.settings.theme.is_none() && ctx.system_theme().is_some() {
            // No saved choice and the window system reports the theme:
            // follow it as it changes.
            style::sync_theme(&ctx);
        }
        self.poll_imports();
        self.poll_update_check();
        self.poll_update_download();
        self.handle_dropped_files(&ctx);
        self.handle_close(&ctx);
        self.handle_shortcuts(&ctx);
        if self.preview.as_ref().is_some_and(|(_, p)| p.finished()) {
            self.preview = None;
        }
        let progress = self.poll_transfer();
        let progress_for_dialog = progress.clone();
        let recording_view = self.poll_recording();
        if let Some(player) = &mut self.sequence_preview {
            player.sync(&self.state);
        }
        if self.transfer.is_some()
            || !self.imports.is_empty()
            || self.preview.is_some()
            || self.sequence_preview.is_some()
            || self.recording.is_some()
        {
            // Fast enough for the playhead at high tempos.
            let interval = if self.sequence_preview.is_some() {
                16
            } else {
                100
            };
            ctx.request_repaint_after(Duration::from_millis(interval));
        }
        let title = self.state.title();
        if self.title != title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title = title;
        }

        egui::Panel::top("top").show(ui, |ui| {
            ui.add_space(4.0);
            self.top_bar(ui);
            ui.add_space(4.0);
        });
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        let transport = sequences::Transport {
            playing: self.sequence_preview.is_some(),
            step: self
                .sequence_preview
                .as_ref()
                .and_then(SequencePreview::step),
            can_play: self.transfer.is_none(),
        };
        let mut play_toggled = false;
        egui::Panel::right("side")
            .resizable(true)
            .default_size(380.0)
            .show(ui, |ui| {
                // Scrolls when the panel is taller than the window (e.g. with
                // transfer messages).
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(6.0);
                    if self.state.view == View::Sequences
                        && sequences::sidebar(ui, &mut self.state, &transport)
                    {
                        play_toggled = true;
                    }
                    let actions = if self.state.view == View::Samples {
                        let playing = self
                            .preview
                            .as_ref()
                            .filter(|(n, _)| Some(*n) == self.state.selected)
                            .map(|(_, p)| {
                                let (played, total) = p.position();
                                played as f32 / total.max(1) as f32
                            });
                        slot_panel::show(ui, &mut self.state, &mut self.name_edit, playing)
                    } else {
                        Vec::new()
                    };
                    for action in actions {
                        match action {
                            PanelAction::LoadFile(slot) => self.pick_sample(slot),
                            PanelAction::Export(slot) => self.export_sample(slot),
                            PanelAction::Clear(slot) => {
                                if self.preview.as_ref().is_some_and(|(n, _)| *n == slot) {
                                    self.preview = None;
                                }
                                self.state.clear(slot);
                            }
                            PanelAction::ApplyTrim => {
                                if let Err(err) = self.state.apply_trim() {
                                    self.error(err);
                                }
                            }
                            PanelAction::Normalize => match self.state.normalize_selected() {
                                Ok(gain) => {
                                    self.info(style::caps(&format!("Normalized, gain {gain:.2}")))
                                }
                                Err(err) => self.error(err),
                            },
                            PanelAction::Pitch => match self.state.pitch_selected() {
                                Ok(()) => {
                                    let text = format!("Pitch {:+} semitones", self.state.pitch);
                                    self.info(style::caps(&text));
                                }
                                Err(err) => self.error(err),
                            },
                            PanelAction::Paste(slot) => match self.state.paste_sample(slot) {
                                Ok(true) => {
                                    if self.preview.as_ref().is_some_and(|(n, _)| *n == slot) {
                                        self.preview = None;
                                    }
                                    self.info(style::caps(&format!("Pasted into slot {slot}")));
                                }
                                Ok(false) => {}
                                Err(err) => self.error(err),
                            },
                            PanelAction::Undo(slot) => {
                                if self.state.undo(slot) {
                                    self.info(style::caps(&format!("Undo in slot {slot}")));
                                }
                            }
                        }
                    }
                    if self.state.view == View::Samples && self.state.selected.is_some() {
                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(6.0);
                        let busy = self.transfer.is_some();
                        match record_panel::show(
                            ui,
                            &mut self.state,
                            &self.inputs,
                            recording_view.as_ref(),
                            busy,
                        ) {
                            Some(RecordAction::Start(slot)) => self.start_recording(slot),
                            Some(RecordAction::Stop) => self.stop_recording(),
                            Some(RecordAction::RefreshInputs) => self.inputs = list_inputs(),
                            None => {}
                        }
                    }
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(6.0);
                    let actions = transfer_panel::show(
                        ui,
                        &mut self.state,
                        &self.outputs,
                        progress,
                        &self.report,
                    );
                    for action in actions {
                        match action {
                            TransferAction::Send => {
                                self.start_transfer(self.state.transfer_targets())
                            }
                            TransferAction::SendCurrentPattern => {
                                self.start_transfer(vec![Target::Pattern(self.state.pattern)])
                            }
                            TransferAction::Cancel => {
                                if let Some(TransferJob::Running { transfer, .. }) = &self.transfer
                                {
                                    transfer.cancel();
                                }
                            }
                            TransferAction::RefreshOutputs => self.outputs = list_outputs(),
                        }
                    }
                });
            });
        egui::CentralPanel::default().show(ui, |ui| {
            if self.state.view == View::Sequences {
                sequences::show(ui, &mut self.state, &transport);
                return;
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                let playing = self.preview.as_ref().map(|(n, p)| {
                    let (played, total) = p.position();
                    (*n, played as f32 / total.max(1) as f32)
                });
                for action in slots::show(ui, &self.state, playing) {
                    match action {
                        SlotAction::Select(n) => self.state.select(n),
                        SlotAction::ToggleChecked(n) => self.state.toggle_checked(n),
                        SlotAction::Play(n) => self.toggle_preview(n),
                        SlotAction::Move { from, to } => self.move_slot(from, to),
                    }
                }
            });
        });

        if play_toggled {
            self.toggle_sequence_preview();
        }
        self.transfer_dialog(&ctx, progress_for_dialog.as_ref());
        self.menu(&ctx);
        if self.credits_open {
            menu::credits_dialog(&ctx, &mut self.credits_open);
        }
        if self.welcome_open {
            self.welcome(&ctx);
        }
        self.confirm_discard(&ctx);
        self.screenshot_step(&ctx);
    }
}
