//! Application state and the operations the UI triggers. No egui here, so
//! the logic is testable on its own.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use volcasm_core::device::VOLCA_SAMPLE;
use volcasm_core::memory::MemoryUsage;
use volcasm_core::project::{Project, Slot, Target, file};
use volcasm_core::sample::{Sample, edit};

/// Compression choices offered for a transfer (bits per sample). 16 bits is
/// lossless (`docs/hardware.md`, section 2) and shortens the stream.
pub const BIT_CHOICES: [u32; 9] = [16, 15, 14, 13, 12, 11, 10, 9, 8];

/// Undo steps kept per slot (session only, not saved).
pub const UNDO_DEPTH: usize = 10;

/// Transfer settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferSettings {
    /// Compression bits; `None` sends uncompressed.
    pub bits: Option<u32>,
    /// Also erase checked slots that are empty in the project.
    pub erase_empty: bool,
    /// Output id from `volcasm_audio::list_outputs`; `None` = default.
    pub device: Option<String>,
}

impl Default for TransferSettings {
    fn default() -> Self {
        Self {
            bits: Some(16),
            erase_empty: false,
            device: None,
        }
    }
}

/// Everything the UI shows and edits.
pub struct AppState {
    pub project: Project,
    /// File the project was opened from or saved to.
    pub path: Option<PathBuf>,
    /// Unsaved changes.
    pub dirty: bool,
    /// Slot shown in the slot panel.
    pub selected: Option<usize>,
    /// Slots checked for transfer.
    pub checked: BTreeSet<usize>,
    /// Trim range (frames) being edited for the selected slot.
    pub trim: Option<std::ops::Range<usize>>,
    /// Semitones chosen in the pitch control.
    pub pitch: i32,
    pub transfer: TransferSettings,
    /// Previous contents of each slot, newest last (`None` = was empty).
    history: HashMap<usize, Vec<Option<Slot>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::with_project(Project::new(&VOLCA_SAMPLE, "untitled"), None)
    }
}

impl AppState {
    pub fn with_project(project: Project, path: Option<PathBuf>) -> Self {
        Self {
            project,
            path,
            dirty: false,
            selected: None,
            checked: BTreeSet::new(),
            trim: None,
            pitch: 0,
            transfer: TransferSettings::default(),
            history: HashMap::new(),
        }
    }

    pub fn slot_count(&self) -> usize {
        self.project.device().sample_slots as usize
    }

    /// Opens a `.volcasm` file.
    pub fn open(path: &Path) -> Result<Self, String> {
        let project = file::load(path).map_err(|e| e.to_string())?;
        Ok(Self::with_project(project, Some(path.to_path_buf())))
    }

    /// Imports a Vosyr project. Returns the new state and the samples that
    /// could not be loaded.
    pub fn import_vsj(path: &Path) -> Result<(Self, Vec<String>), String> {
        let vosyr = volcasm_vosyr_import::load(path).map_err(|e| e.to_string())?;
        let (project, errors) = vosyr.to_project();
        let mut state = Self::with_project(project, None);
        state.dirty = true;
        Ok((state, errors.iter().map(ToString::to_string).collect()))
    }

    /// Saves to `path` (or the current path).
    pub fn save(&mut self, path: Option<&Path>) -> Result<(), String> {
        let path = path
            .map(Path::to_path_buf)
            .or_else(|| self.path.clone())
            .ok_or("no file chosen")?;
        file::save(&self.project, &path).map_err(|e| e.to_string())?;
        self.path = Some(path);
        self.dirty = false;
        Ok(())
    }

    pub fn memory(&self) -> MemoryUsage {
        self.project.memory_usage()
    }

    pub fn select(&mut self, slot: usize) {
        if self.selected != Some(slot) {
            self.selected = Some(slot);
            self.trim = None;
        }
    }

    pub fn toggle_checked(&mut self, slot: usize) {
        if !self.checked.remove(&slot) {
            self.checked.insert(slot);
        }
    }

    /// Checks every filled slot.
    pub fn check_filled(&mut self) {
        self.checked = self.project.slots().map(|(n, _)| n).collect();
    }

    /// Saves the current content of `slot` for [`Self::undo`].
    fn remember(&mut self, slot: usize) {
        let current = self.project.slot(slot).cloned();
        self.push_history(slot, current);
    }

    fn push_history(&mut self, slot: usize, content: Option<Slot>) {
        let steps = self.history.entry(slot).or_default();
        steps.push(content);
        if steps.len() > UNDO_DEPTH {
            steps.remove(0);
        }
    }

    /// Whether `slot` has an edit to undo.
    pub fn can_undo(&self, slot: usize) -> bool {
        self.history
            .get(&slot)
            .is_some_and(|steps| !steps.is_empty())
    }

    /// Restores the previous content of `slot`.
    pub fn undo(&mut self, slot: usize) -> bool {
        let Some(previous) = self.history.get_mut(&slot).and_then(Vec::pop) else {
            return false;
        };
        match previous {
            Some(content) => {
                // It was a valid slot before, so it still is.
                let _ = self.project.set_slot(slot, content);
            }
            None => {
                let _ = self.project.clear_slot(slot);
            }
        }
        self.dirty = true;
        if self.selected == Some(slot) {
            self.trim = None;
        }
        true
    }

    /// Puts an imported sample into `slot`, named after `name`. The previous
    /// content can be restored with [`Self::undo`].
    pub fn set_sample(&mut self, slot: usize, name: String, sample: Sample) -> Result<(), String> {
        if slot >= self.slot_count() {
            return Err(format!("slot {slot} out of range"));
        }
        let previous = self.project.slot(slot).cloned();
        self.project
            .set_slot(slot, Slot { name, sample })
            .map_err(|e| e.to_string())?;
        self.push_history(slot, previous);
        self.dirty = true;
        if self.selected == Some(slot) {
            self.trim = None;
        }
        Ok(())
    }

    pub fn rename(&mut self, slot: usize, name: String) {
        if let Some(current) = self.project.slot(slot)
            && current.name != name
        {
            let sample = current.sample.clone();
            // Same rate and length as before, so this cannot fail.
            let _ = self.project.set_slot(slot, Slot { name, sample });
            self.dirty = true;
        }
    }

    pub fn clear(&mut self, slot: usize) {
        if self.project.slot(slot).is_some() {
            self.remember(slot);
        }
        if let Ok(Some(_)) = self.project.clear_slot(slot) {
            self.dirty = true;
            if self.selected == Some(slot) {
                self.trim = None;
            }
        }
    }

    /// Applies the trim range to the selected slot.
    pub fn apply_trim(&mut self) -> Result<(), String> {
        let (slot, range) = match (self.selected, self.trim.clone()) {
            (Some(slot), Some(range)) => (slot, range),
            _ => return Err("nothing to trim".into()),
        };
        let current = self.project.slot(slot).ok_or("empty slot")?;
        let sample = edit::trim(&current.sample, range).ok_or("invalid trim range")?;
        let name = current.name.clone();
        self.set_sample(slot, name, sample)
    }

    /// Normalizes the selected slot. Returns the gain.
    pub fn normalize_selected(&mut self) -> Result<f64, String> {
        let slot = self.selected.ok_or("no slot selected")?;
        let current = self.project.slot(slot).ok_or("empty slot")?;
        let mut sample = current.sample.clone();
        let gain = edit::normalize(&mut sample);
        if gain != 1.0 {
            let name = current.name.clone();
            self.set_sample(slot, name, sample)?;
        }
        Ok(gain)
    }

    /// Shifts the selected slot by [`Self::pitch`] semitones.
    pub fn pitch_selected(&mut self) -> Result<(), String> {
        let slot = self.selected.ok_or("no slot selected")?;
        let current = self.project.slot(slot).ok_or("empty slot")?;
        if self.pitch == 0 {
            return Ok(());
        }
        let sample = edit::pitch(&current.sample, self.pitch).map_err(|e| e.to_string())?;
        let name = current.name.clone();
        self.set_sample(slot, name, sample)
    }

    /// What a transfer of the checked slots sends: filled slots as samples,
    /// empty ones erased if `erase_empty` is set.
    pub fn transfer_targets(&self) -> Vec<Target> {
        self.checked
            .iter()
            .filter_map(|&n| {
                if self.project.slot(n).is_some() {
                    Some(Target::Sample(n))
                } else if self.transfer.erase_empty {
                    Some(Target::Erase(n))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Window title: project name, file and unsaved marker.
    pub fn title(&self) -> String {
        let file = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| format!(" ({})", n.to_string_lossy()))
            .unwrap_or_default();
        format!(
            "{}{}{} - volcasm",
            if self.dirty { "*" } else { "" },
            self.project.name,
            file
        )
    }
}

/// Slot name for an imported file: its file stem.
pub fn name_from_path(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "sample".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(pcm: &[i16]) -> Sample {
        Sample {
            sample_rate: 31_250,
            pcm: pcm.to_vec(),
        }
    }

    #[test]
    fn set_rename_clear() {
        let mut s = AppState::default();
        s.set_sample(4, "kick".into(), sample(&[1, 2, 3])).unwrap();
        assert!(s.dirty);
        s.rename(4, "snare".into());
        assert_eq!(s.project.slot(4).unwrap().name, "snare");
        s.clear(4);
        assert!(s.project.slot(4).is_none());
        assert!(s.set_sample(100, "x".into(), sample(&[1])).is_err());
    }

    #[test]
    fn trim_and_normalize_selected() {
        let mut s = AppState::default();
        s.set_sample(0, "a".into(), sample(&[0, 100, -200, 50, 0]))
            .unwrap();
        s.select(0);
        s.trim = Some(1..4);
        s.apply_trim().unwrap();
        assert_eq!(s.project.slot(0).unwrap().sample.pcm, [100, -200, 50]);
        assert!(s.trim.is_none());
        let gain = s.normalize_selected().unwrap();
        assert_eq!(gain, 32767.0 / 200.0);
        assert_eq!(s.project.slot(0).unwrap().sample.pcm[1], -32767);
        // Out-of-range trim is refused and changes nothing.
        s.trim = Some(2..10);
        assert!(s.apply_trim().is_err());
        assert_eq!(s.project.slot(0).unwrap().sample.frames(), 3);
    }

    #[test]
    fn selecting_another_slot_resets_trim() {
        let mut s = AppState::default();
        s.select(1);
        s.trim = Some(0..1);
        s.select(1);
        assert!(s.trim.is_some());
        s.select(2);
        assert!(s.trim.is_none());
    }

    #[test]
    fn undo_restores_previous_versions() {
        let mut s = AppState::default();
        s.set_sample(2, "a".into(), sample(&[0, 100, -200, 50]))
            .unwrap();
        s.select(2);
        s.trim = Some(1..3);
        s.apply_trim().unwrap();
        s.normalize_selected().unwrap();
        s.pitch = 12;
        s.pitch_selected().unwrap();
        s.clear(2);
        assert!(s.project.slot(2).is_none());
        // clear -> pitch -> normalize -> trim -> load
        assert!(s.undo(2));
        assert_eq!(s.project.slot(2).unwrap().sample.frames(), 1);
        assert!(s.undo(2));
        assert_eq!(s.project.slot(2).unwrap().sample.pcm, [16_384, -32_767]);
        assert!(s.undo(2));
        assert_eq!(s.project.slot(2).unwrap().sample.pcm, [100, -200]);
        assert!(s.undo(2));
        assert_eq!(s.project.slot(2).unwrap().sample.pcm, [0, 100, -200, 50]);
        assert!(s.undo(2));
        assert!(s.project.slot(2).is_none());
        assert!(!s.can_undo(2) && !s.undo(2));
    }

    #[test]
    fn undo_depth_is_limited() {
        let mut s = AppState::default();
        for i in 0..UNDO_DEPTH as i16 + 5 {
            s.set_sample(0, "x".into(), sample(&[i + 1])).unwrap();
        }
        let mut steps = 0;
        while s.undo(0) {
            steps += 1;
        }
        assert_eq!(steps, UNDO_DEPTH);
    }

    #[test]
    fn transfer_targets() {
        let mut s = AppState::default();
        s.set_sample(0, "a".into(), sample(&[1])).unwrap();
        s.set_sample(9, "b".into(), sample(&[1])).unwrap();
        s.toggle_checked(0);
        s.toggle_checked(5);
        assert_eq!(s.transfer_targets(), [Target::Sample(0)]);
        s.transfer.erase_empty = true;
        assert_eq!(s.transfer_targets(), [Target::Sample(0), Target::Erase(5)]);
        s.check_filled();
        assert_eq!(s.transfer_targets(), [Target::Sample(0), Target::Sample(9)]);
        s.toggle_checked(0);
        assert_eq!(s.transfer_targets(), [Target::Sample(9)]);
    }

    #[test]
    fn save_and_open() {
        let mut s = AppState::default();
        s.set_sample(3, "a".into(), sample(&[5, 6])).unwrap();
        let path =
            std::env::temp_dir().join(format!("volcasm-state-{}.volcasm", std::process::id()));
        s.save(Some(&path)).unwrap();
        assert!(!s.dirty);
        assert!(s.title().contains(".volcasm"));
        let opened = AppState::open(&path).unwrap();
        assert_eq!(opened.project, s.project);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn import_vsj_marks_dirty() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/vosyr/Test2/Test2.vsj");
        let (s, errors) = AppState::import_vsj(&path).unwrap();
        assert!(errors.is_empty());
        assert!(s.dirty && s.path.is_none());
        assert_eq!(s.project.slots().count(), 2);
    }
}
