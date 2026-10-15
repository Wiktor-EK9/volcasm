//! Application state and the operations the UI triggers. No egui here, so
//! the logic is testable on its own.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use volcasm_core::device::VOLCA_SAMPLE;
use volcasm_core::memory::MemoryUsage;
use volcasm_core::pattern::{MotionRow, PARTS, Pattern};
use volcasm_core::project::{Project, Slot, Target, file};
use volcasm_core::sample::{Sample, edit};

/// Samples are always sent compressed to 16 bits: lossless
/// (`docs/hardware.md`, section 2) and a shorter stream.
pub const TRANSFER_BITS: u32 = 16;

/// Name of a new project.
pub const DEFAULT_PROJECT_NAME: &str = "New Project";

/// Undo steps kept per slot (session only, not saved).
pub const UNDO_DEPTH: usize = 10;

/// Transfer settings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransferSettings {
    /// Also erase checked slots that are empty in the project.
    pub erase_empty: bool,
    /// Output id from `volcasm_audio::list_outputs`; `None` = default.
    pub device: Option<String>,
}

/// Main view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Samples,
    Sequences,
}

/// What is shown under a part's steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartView {
    Steps,
    Params,
    Motion,
}

/// Identifies a pattern edit; consecutive edits with the same key (e.g. one
/// knob being dragged) form a single undo step.
pub type EditKey = (&'static str, usize, usize);

/// Recording settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordSettings {
    /// Input id from `volcasm_audio::list_inputs`; `None` = default.
    pub input: Option<String>,
    /// Remove silence before and after the sound.
    pub trim_silence: bool,
}

impl Default for RecordSettings {
    fn default() -> Self {
        Self {
            input: None,
            trim_silence: true,
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
    pub record: RecordSettings,
    /// Previous contents of each slot, newest last (`None` = was empty).
    history: HashMap<usize, Vec<Option<Slot>>>,
    pub view: View,
    /// Sequence shown in the editor (0-based).
    pub pattern: usize,
    pub part_views: [PartView; PARTS],
    /// Motion row shown in each part's motion editor.
    pub motion_rows: [MotionRow; PARTS],
    /// Sequences checked for transfer (0-based).
    pub checked_patterns: BTreeSet<usize>,
    pub clipboard: Option<Pattern>,
    /// Sample copied with COPY in the slot panel.
    pub sample_clipboard: Option<Slot>,
    /// Previous versions of each pattern, newest last.
    pattern_history: HashMap<usize, Vec<Pattern>>,
    last_edit: Option<(usize, EditKey)>,
    /// Part to scroll into view in the sequence editor, once.
    pub scroll_to_part: Option<usize>,
    /// Parts soloed in the preview (not saved, not sent to the device).
    pub solo: [bool; PARTS],
    /// Changes whenever a sample changes, so the preview can reload them.
    pub samples_generation: u64,
}

impl Default for AppState {
    fn default() -> Self {
        Self::with_project(Project::new(&VOLCA_SAMPLE, DEFAULT_PROJECT_NAME), None)
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
            record: RecordSettings::default(),
            history: HashMap::new(),
            view: View::Samples,
            pattern: 0,
            part_views: [PartView::Steps; PARTS],
            motion_rows: [MotionRow::LevelStart; PARTS],
            checked_patterns: BTreeSet::new(),
            clipboard: None,
            sample_clipboard: None,
            pattern_history: HashMap::new(),
            last_edit: None,
            scroll_to_part: None,
            solo: [false; PARTS],
            samples_generation: 0,
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

    /// Slots for `count` files loaded at once: `first`, then the nearest
    /// free slots after it, wrapping around to slot 0. Slots in `busy`
    /// (imports in progress) count as taken. Shorter than `count` when there
    /// are not enough free slots.
    pub fn slots_for_files(&self, first: usize, count: usize, busy: &[usize]) -> Vec<usize> {
        if count == 0 {
            return Vec::new();
        }
        let n = self.slot_count();
        let free = (1..n)
            .map(|k| (first + k) % n)
            .filter(|&s| self.project.slot(s).is_none() && !busy.contains(&s));
        std::iter::once(first).chain(free).take(count).collect()
    }

    /// Moves the sample in `from` to `to`; a sample already in `to` goes
    /// to `from` (the two swap). The transfer check mark and the undo
    /// history follow the samples. Sequences keep their slot numbers.
    /// Returns false when nothing moved.
    pub fn move_slot(&mut self, from: usize, to: usize) -> bool {
        let n = self.slot_count();
        if from == to || from >= n || to >= n || self.project.slot(from).is_none() {
            return false;
        }
        let moved = self.project.clear_slot(from).ok().flatten();
        let other = self.project.clear_slot(to).ok().flatten();
        // Both slots were valid before, so they still are.
        if let Some(slot) = moved {
            let _ = self.project.set_slot(to, slot);
        }
        if let Some(slot) = other {
            let _ = self.project.set_slot(from, slot);
        }
        let (history_from, history_to) = (self.history.remove(&from), self.history.remove(&to));
        if let Some(steps) = history_from {
            self.history.insert(to, steps);
        }
        if let Some(steps) = history_to {
            self.history.insert(from, steps);
        }
        let (checked_from, checked_to) = (self.checked.remove(&from), self.checked.remove(&to));
        if checked_from {
            self.checked.insert(to);
        }
        if checked_to {
            self.checked.insert(from);
        }
        self.dirty = true;
        self.samples_generation += 1;
        self.selected = Some(to);
        self.trim = None;
        true
    }

    /// Copies the sample in `slot` (name and audio) for [`Self::paste_sample`].
    pub fn copy_sample(&mut self, slot: usize) {
        if let Some(content) = self.project.slot(slot) {
            self.sample_clipboard = Some(content.clone());
        }
    }

    /// Puts the copied sample into `slot`; [`Self::undo`] restores the
    /// previous content. Returns false when nothing was copied.
    pub fn paste_sample(&mut self, slot: usize) -> Result<bool, String> {
        let Some(content) = self.sample_clipboard.clone() else {
            return Ok(false);
        };
        self.set_sample(slot, content.name, content.sample)?;
        Ok(true)
    }

    /// Checks every slot, filled or empty.
    pub fn check_all(&mut self) {
        self.checked = (0..self.slot_count()).collect();
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
        self.samples_generation += 1;
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
        self.samples_generation += 1;
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
            self.samples_generation += 1;
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

    /// The sequence shown in the editor.
    pub fn current_pattern(&self) -> &Pattern {
        self.project
            .pattern(self.pattern)
            .expect("current pattern is in range")
    }

    /// Changes the current sequence through `edit`. Edits with the same
    /// `key` in a row share one undo step.
    pub fn edit_pattern(&mut self, key: EditKey, edit: impl FnOnce(&mut Pattern)) {
        let n = self.pattern;
        let before = *self.current_pattern();
        let mut after = before;
        edit(&mut after);
        if after == before {
            return;
        }
        if self.last_edit != Some((n, key)) {
            let steps = self.pattern_history.entry(n).or_default();
            steps.push(before);
            if steps.len() > UNDO_DEPTH {
                steps.remove(0);
            }
            self.last_edit = Some((n, key));
        }
        *self
            .project
            .pattern_mut(n)
            .expect("current pattern is in range") = after;
        self.dirty = true;
    }

    /// Starts a new undo step for the next pattern edit (a new click or
    /// drag gesture).
    pub fn end_edit(&mut self) {
        self.last_edit = None;
    }

    /// Opens or closes a part's knobs or motion editor. The motion editor
    /// opens on the first row that has data.
    pub fn toggle_part_view(&mut self, part: usize, view: PartView) {
        let open = self.part_views[part] == view;
        self.part_views[part] = if open { PartView::Steps } else { view };
        if !open && view == PartView::Motion {
            let data = self.current_pattern().part(part);
            let has_data = |row: MotionRow| (1..=16).any(|step| data.raw_motion(row, step) != 0);
            if !has_data(self.motion_rows[part])
                && let Some(row) = crate::ui::motion::ROWS
                    .iter()
                    .map(|(r, _)| *r)
                    .find(|r| has_data(*r))
            {
                self.motion_rows[part] = row;
            }
        }
    }

    pub fn can_undo_pattern(&self) -> bool {
        self.pattern_history
            .get(&self.pattern)
            .is_some_and(|steps| !steps.is_empty())
    }

    /// Restores the previous version of the current sequence.
    pub fn undo_pattern(&mut self) -> bool {
        let n = self.pattern;
        let Some(previous) = self.pattern_history.get_mut(&n).and_then(Vec::pop) else {
            return false;
        };
        *self
            .project
            .pattern_mut(n)
            .expect("current pattern is in range") = previous;
        self.last_edit = None;
        self.dirty = true;
        true
    }

    pub fn select_pattern(&mut self, n: usize) {
        if n < self.project.patterns().len() {
            self.pattern = n;
            self.last_edit = None;
        }
    }

    pub fn copy_pattern(&mut self) {
        self.clipboard = Some(*self.current_pattern());
    }

    pub fn paste_pattern(&mut self) {
        if let Some(pattern) = self.clipboard {
            self.edit_pattern(("paste", 0, 0), |p| *p = pattern);
            self.last_edit = None;
        }
    }

    pub fn clear_pattern(&mut self) {
        self.edit_pattern(("clear", 0, 0), |p| *p = Pattern::default());
        self.last_edit = None;
    }

    pub fn toggle_checked_pattern(&mut self, n: usize) {
        if !self.checked_patterns.remove(&n) {
            self.checked_patterns.insert(n);
        }
    }

    /// Problems of the current sequence that the device would not accept
    /// (sample out of range, no active step, level or accent changed).
    pub fn pattern_warnings(&self) -> Vec<String> {
        self.current_pattern()
            .issues(self.project.device().sample_slots)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// What a transfer sends: checked slots (filled ones as samples, empty
    /// ones erased if `erase_empty` is set), then checked sequences.
    pub fn transfer_targets(&self) -> Vec<Target> {
        let samples = self.checked.iter().filter_map(|&n| {
            if self.project.slot(n).is_some() {
                Some(Target::Sample(n))
            } else if self.transfer.erase_empty {
                Some(Target::Erase(n))
            } else {
                None
            }
        });
        let patterns = self.checked_patterns.iter().map(|&n| Target::Pattern(n));
        samples.chain(patterns).collect()
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
    fn pattern_edits_and_undo() {
        use volcasm_core::pattern::{Func, Param};
        let mut s = AppState::default();
        s.select_pattern(3);
        let original = *s.current_pattern();
        s.edit_pattern(("step", 0, 1), |p| p.part_mut(0).set_step(1, true));
        // Dragging one knob: several edits, one undo step.
        for v in [10, 20, 30] {
            s.edit_pattern(("param", 2, Param::Pan as usize), |p| {
                p.part_mut(2).set_param(Param::Pan, v).unwrap()
            });
        }
        s.edit_pattern(("func", 4, 0), |p| {
            p.part_mut(4).set_func(Func::Reverb, true)
        });
        assert!(s.dirty);
        let edited = *s.current_pattern();
        assert_eq!(edited.part(2).param(Param::Pan), 30);

        // Only the edited bytes changed.
        let (a, b) = (original.to_bytes(), edited.to_bytes());
        let changed: Vec<usize> = (0..a.len()).filter(|&i| a[i] != b[i]).collect();
        let part = |p: usize, off: usize| 32 + 256 * p + off;
        assert_eq!(changed, [part(0, 2), part(2, 10), part(4, 20)]);

        assert!(s.undo_pattern());
        assert!(!s.current_pattern().part(4).func(Func::Reverb));
        assert!(s.undo_pattern());
        assert_eq!(s.current_pattern().part(2).param(Param::Pan), 0);
        assert!(s.undo_pattern());
        assert_eq!(*s.current_pattern(), original);
        assert!(!s.undo_pattern());
        // Other sequences untouched.
        assert_eq!(*s.project.pattern(0).unwrap(), Pattern::default());
    }

    #[test]
    fn motion_editor_opens_on_used_row() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/vosyr/Test2/Test2.vsj");
        let (mut s, _) = AppState::import_vsj(&path).unwrap();
        s.toggle_part_view(7, PartView::Motion);
        assert_eq!(s.part_views[7], PartView::Motion);
        assert_eq!(s.motion_rows[7], MotionRow::AmpEgAttack);
        s.toggle_part_view(7, PartView::Motion);
        assert_eq!(s.part_views[7], PartView::Steps);
    }

    #[test]
    fn copy_paste_clear() {
        let mut s = AppState::default();
        s.edit_pattern(("step", 0, 1), |p| p.part_mut(0).set_steps(0x00ff));
        s.copy_pattern();
        s.select_pattern(5);
        s.paste_pattern();
        assert_eq!(s.current_pattern().part(0).steps(), 0x00ff);
        s.clear_pattern();
        assert_eq!(*s.current_pattern(), Pattern::default());
        assert!(s.undo_pattern());
        assert_eq!(s.current_pattern().part(0).steps(), 0x00ff);
    }

    #[test]
    fn warnings_only_for_device_issues() {
        let mut s = AppState::default();
        s.edit_pattern(("x", 0, 0), |p| {
            // Steps on an empty slot: fine, no warning.
            p.part_mut(1).set_sample(7);
            p.part_mut(1).set_steps(1);
        });
        assert!(s.pattern_warnings().is_empty());
        s.edit_pattern(("y", 0, 0), |p| p.set_active_steps(0));
        assert_eq!(s.pattern_warnings(), ["no active step"]);
    }

    #[test]
    fn copy_and_paste_sample() {
        let mut s = AppState::default();
        assert_eq!(s.paste_sample(1), Ok(false));
        s.set_sample(0, "a".into(), sample(&[1, 2])).unwrap();
        s.set_sample(1, "b".into(), sample(&[3])).unwrap();
        s.copy_sample(0);
        // Copying an empty slot keeps the clipboard.
        s.copy_sample(5);
        assert_eq!(s.paste_sample(1), Ok(true));
        assert_eq!(s.project.slot(1).unwrap().name, "a");
        assert_eq!(s.project.slot(1).unwrap().sample.pcm, [1, 2]);
        assert_eq!(s.paste_sample(7), Ok(true));
        assert_eq!(s.project.slot(7).unwrap().name, "a");
        // Undo brings back what was there.
        assert!(s.undo(1));
        assert_eq!(s.project.slot(1).unwrap().name, "b");
        assert!(s.undo(7));
        assert!(s.project.slot(7).is_none());
    }

    #[test]
    fn move_slot() {
        let mut s = AppState::default();
        s.set_sample(5, "a".into(), sample(&[1])).unwrap();
        s.set_sample(6, "b".into(), sample(&[2, 2])).unwrap();
        s.toggle_checked(5);
        s.dirty = false;
        // Onto a filled slot: the two swap.
        assert!(s.move_slot(5, 6));
        assert_eq!(s.project.slot(6).unwrap().name, "a");
        assert_eq!(s.project.slot(5).unwrap().name, "b");
        assert!(s.checked.contains(&6) && !s.checked.contains(&5));
        assert_eq!(s.selected, Some(6));
        assert!(s.dirty);
        // Onto an empty slot: a plain move; undo history follows.
        assert!(s.move_slot(6, 9));
        assert!(s.project.slot(6).is_none());
        assert_eq!(s.project.slot(9).unwrap().name, "a");
        assert!(s.can_undo(9) && !s.can_undo(6));
        // Nothing to move.
        assert!(!s.move_slot(6, 7));
        assert!(!s.move_slot(9, 9));
        assert!(!s.move_slot(9, s.slot_count()));
    }

    #[test]
    fn slots_for_files() {
        let mut s = AppState::default();
        let n = s.slot_count();
        s.set_sample(5, "a".into(), sample(&[1])).unwrap();
        s.set_sample(7, "b".into(), sample(&[1])).unwrap();
        // The selected slot even when filled, then free slots after it.
        assert_eq!(s.slots_for_files(5, 3, &[]), [5, 6, 8]);
        assert_eq!(s.slots_for_files(5, 3, &[6]), [5, 8, 9]);
        assert_eq!(s.slots_for_files(5, 0, &[]), [] as [usize; 0]);
        // Wraps around to slot 0.
        assert_eq!(s.slots_for_files(n - 1, 3, &[]), [n - 1, 0, 1]);
        // Never more than the free slots.
        assert_eq!(s.slots_for_files(0, 1000, &[]).len(), n - 2);
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
        s.check_all();
        assert_eq!(s.checked.len(), s.slot_count());
        s.transfer.erase_empty = false;
        assert_eq!(s.transfer_targets(), [Target::Sample(0), Target::Sample(9)]);
        s.transfer.erase_empty = true;
        assert_eq!(s.transfer_targets().len(), s.slot_count());
        s.check_filled();
        s.toggle_checked(0);
        assert_eq!(s.transfer_targets(), [Target::Sample(9)]);
        s.toggle_checked_pattern(4);
        s.toggle_checked_pattern(0);
        assert_eq!(
            s.transfer_targets(),
            [Target::Sample(9), Target::Pattern(0), Target::Pattern(4)]
        );
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
