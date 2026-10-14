//! The grid of sample slots.

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{self, Align2, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, vec2};

use super::style;
use crate::state::AppState;

const COLUMNS: usize = 10;
const GAP: f32 = 4.0;
const TILE_HEIGHT: f32 = 80.0;
/// Size of the check box and play button: large enough to hit easily.
const BOX: f32 = 20.0;

/// What the user did in the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotAction {
    Select(usize),
    ToggleChecked(usize),
    Play(usize),
}

/// Draws all slots. `playing` is the slot being previewed and how much of
/// it has played (0..1), if any.
pub fn show(ui: &mut Ui, state: &AppState, playing: Option<(usize, f32)>) -> Vec<SlotAction> {
    let mut actions = Vec::new();
    let width = ui.available_width();
    let tile_width = ((width - GAP * (COLUMNS - 1) as f32) / COLUMNS as f32).floor();
    let rows = state.slot_count().div_ceil(COLUMNS);
    let (area, _) = ui.allocate_exact_size(
        vec2(width, rows as f32 * (TILE_HEIGHT + GAP)),
        Sense::hover(),
    );
    for n in 0..state.slot_count() {
        let (row, col) = (n / COLUMNS, n % COLUMNS);
        let min = area.min
            + vec2(
                col as f32 * (tile_width + GAP),
                row as f32 * (TILE_HEIGHT + GAP),
            );
        let rect = Rect::from_min_size(min, vec2(tile_width, TILE_HEIGHT));
        let progress = playing.filter(|(slot, _)| *slot == n).map(|(_, f)| f);
        tile(ui, state, n, rect, progress, &mut actions);
    }
    actions
}

fn tile(
    ui: &mut Ui,
    state: &AppState,
    n: usize,
    rect: Rect,
    progress: Option<f32>,
    actions: &mut Vec<SlotAction>,
) {
    let slot = state.project.slot(n);
    let response = ui.interact(rect, ui.id().with(("slot", n)), Sense::click());
    if response.clicked() {
        actions.push(SlotAction::Select(n));
    }
    if let Some(slot) = slot {
        let blocks = state
            .project
            .device()
            .memory
            .blocks_for(slot.sample.frames());
        response.clone().on_hover_text(format!(
            "{}\n{:.3} S · {blocks} BLK",
            slot.name,
            slot.sample.duration()
        ));
    }
    let painter = ui.painter_at(rect);
    let fill = if slot.is_some() {
        style::TILE_FILLED
    } else {
        style::TILE_EMPTY
    };
    let fill = if response.hovered() {
        fill.gamma_multiply(1.15)
    } else {
        fill
    };
    painter.rect_filled(rect, 3.0, fill);
    // Preview progress: a translucent bar filling the tile from the left.
    if let Some(fraction) = progress {
        let mut bar = rect;
        bar.set_width(rect.width() * fraction.clamp(0.0, 1.0));
        painter.rect_filled(bar, 3.0, style::ACCENT.gamma_multiply(0.35));
    }
    if state.selected == Some(n) {
        painter.rect_stroke(
            rect,
            3.0,
            Stroke::new(2.0, style::ACCENT),
            StrokeKind::Inside,
        );
    }

    let inner = rect.shrink(5.0);
    let dim = style::TEXT_DIM;
    let text_painter = ui.painter_at(inner);
    text_painter.text(
        inner.left_top(),
        Align2::LEFT_TOP,
        n.to_string(),
        style::body_font(13.0),
        if slot.is_some() { style::ACCENT } else { dim },
    );
    if let Some(slot) = slot {
        // Up to two rows, broken anywhere: names often lack spaces.
        let mut job = LayoutJob::simple(
            style::caps(&slot.name),
            style::body_font(12.0),
            style::TEXT,
            inner.width(),
        );
        job.wrap = TextWrapping {
            max_width: inner.width(),
            max_rows: 2,
            break_anywhere: true,
            overflow_character: Some('…'),
        };
        let galley = ui.fonts_mut(|f| f.layout_job(job));
        text_painter.galley(
            Pos2::new(inner.left(), inner.top() + 15.0),
            galley,
            style::TEXT,
        );
        // Top line, right of the slot number: the bottom row holds the
        // check box and play button.
        text_painter.text(
            inner.right_top(),
            Align2::RIGHT_TOP,
            format!("{:.2}S", slot.sample.duration()),
            style::body_font(10.0),
            dim,
        );
    }

    // Transfer check box, bottom left.
    let check = Rect::from_min_size(
        Pos2::new(inner.left(), inner.bottom() - BOX),
        vec2(BOX, BOX),
    );
    let check_response = ui.interact(check, ui.id().with(("check", n)), Sense::click());
    if check_response.clicked() {
        actions.push(SlotAction::ToggleChecked(n));
    }
    let checked = state.checked.contains(&n);
    painter.rect_stroke(
        check,
        2.0,
        Stroke::new(1.0, if checked { style::ACCENT } else { dim }),
        StrokeKind::Inside,
    );
    if checked {
        painter.rect_filled(check.shrink(4.0), 2.0, style::ACCENT);
    }

    // Preview button, bottom right.
    if slot.is_some() {
        let play = Rect::from_min_size(
            Pos2::new(inner.right() - BOX, inner.bottom() - BOX),
            vec2(BOX, BOX),
        );
        let play_response = ui.interact(play, ui.id().with(("play", n)), Sense::click());
        if play_response.clicked() {
            actions.push(SlotAction::Play(n));
        }
        let playing = progress.is_some();
        let color = if playing || play_response.hovered() {
            style::ACCENT
        } else {
            style::TEXT
        };
        if playing {
            painter.rect_filled(play.shrink(4.0), 1.0, color);
        } else {
            let c = play.center();
            painter.add(egui::Shape::convex_polygon(
                vec![
                    Pos2::new(c.x - 5.0, c.y - 7.0),
                    Pos2::new(c.x + 7.0, c.y),
                    Pos2::new(c.x - 5.0, c.y + 7.0),
                ],
                color,
                Stroke::NONE,
            ));
        }
    }
}
