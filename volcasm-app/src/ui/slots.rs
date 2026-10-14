//! The grid of sample slots.

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    self, Align2, CursorIcon, Id, LayerId, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, vec2,
};

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
    /// A tile dragged onto another slot.
    Move {
        from: usize,
        to: usize,
    },
}

/// Drag-and-drop payload: the slot being dragged.
struct SlotDrag(usize);

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
    drag_ghost(ui, state, tile_width);
    actions
}

/// While a tile is dragged: a small copy of it under the pointer.
fn drag_ghost(ui: &Ui, state: &AppState, tile_width: f32) {
    let Some(drag) = egui::DragAndDrop::payload::<SlotDrag>(ui.ctx()) else {
        return;
    };
    let Some(pointer) = ui.ctx().pointer_interact_pos() else {
        return;
    };
    ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    let n = drag.0;
    let name = state
        .project
        .slot(n)
        .map(|s| style::caps(&s.name))
        .unwrap_or_default();
    let rect = Rect::from_min_size(pointer + vec2(8.0, 8.0), vec2(tile_width, 34.0));
    let painter = ui
        .ctx()
        .layer_painter(LayerId::new(Order::Tooltip, Id::new("slot-drag")));
    painter.rect_filled(rect, 3.0, style::tile_filled().gamma_multiply(0.9));
    painter.rect_stroke(
        rect,
        3.0,
        Stroke::new(1.5, style::ACCENT),
        StrokeKind::Inside,
    );
    let inner = rect.shrink(5.0);
    let clip = painter.with_clip_rect(inner);
    clip.text(
        inner.left_top(),
        Align2::LEFT_TOP,
        n.to_string(),
        style::body_font(12.0),
        style::ACCENT,
    );
    clip.text(
        inner.left_bottom(),
        Align2::LEFT_BOTTOM,
        name,
        style::body_font(11.0),
        style::text(),
    );
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
    // Filled tiles can be dragged onto another slot.
    let sense = if slot.is_some() {
        Sense::click_and_drag()
    } else {
        Sense::click()
    };
    let response = ui.interact(rect, ui.id().with(("slot", n)), sense);
    if response.clicked() {
        actions.push(SlotAction::Select(n));
    }
    if slot.is_some() {
        response.dnd_set_drag_payload(SlotDrag(n));
    }
    if let Some(drag) = response.dnd_release_payload::<SlotDrag>()
        && drag.0 != n
    {
        actions.push(SlotAction::Move {
            from: drag.0,
            to: n,
        });
    }
    let dragged = egui::DragAndDrop::payload::<SlotDrag>(ui.ctx());
    let is_source = dragged.as_ref().is_some_and(|d| d.0 == n);
    let is_target = response
        .dnd_hover_payload::<SlotDrag>()
        .is_some_and(|d| d.0 != n);
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
        style::tile_filled()
    } else {
        style::tile_empty()
    };
    let fill = if response.hovered() {
        fill.gamma_multiply(1.15)
    } else {
        fill
    };
    // The dragged tile fades while it is carried.
    let fill = if is_source {
        fill.gamma_multiply(0.5)
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
    if is_target {
        // Where the dragged tile lands: a filled slot swaps with it.
        painter.rect_filled(rect, 3.0, style::ACCENT.gamma_multiply(0.25));
        painter.rect_stroke(
            rect,
            3.0,
            Stroke::new(2.5, style::ACCENT),
            StrokeKind::Inside,
        );
    } else if state.selected == Some(n) {
        painter.rect_stroke(
            rect,
            3.0,
            Stroke::new(2.0, style::ACCENT),
            StrokeKind::Inside,
        );
    }

    let inner = rect.shrink(5.0);
    let dim = style::text_dim();
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
            style::text(),
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
            style::text(),
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
            style::text()
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
