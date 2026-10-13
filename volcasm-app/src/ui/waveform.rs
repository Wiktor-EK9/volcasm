//! Waveform of a sample with draggable start and end handles.

use std::ops::Range;

use eframe::egui::{Color32, CursorIcon, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, vec2};

use super::style;

/// Width of the grab area around a handle, in points.
const HANDLE_GRAB: f32 = 10.0;

/// Peak (min, max) of each of `columns` equal parts of `pcm`, in -1..=1.
pub fn peaks(pcm: &[i16], columns: usize) -> Vec<(f32, f32)> {
    if pcm.is_empty() || columns == 0 {
        return Vec::new();
    }
    (0..columns)
        .map(|c| {
            let start = c * pcm.len() / columns;
            let end = ((c + 1) * pcm.len() / columns)
                .max(start + 1)
                .min(pcm.len());
            let (lo, hi) = pcm[start..end]
                .iter()
                .fold((i16::MAX, i16::MIN), |(lo, hi), &s| (lo.min(s), hi.max(s)));
            (f32::from(lo) / 32_768.0, f32::from(hi) / 32_768.0)
        })
        .collect()
}

/// Shows the waveform of `pcm` with the trim range `range` (frames). Returns
/// true when a handle was dragged.
pub fn show(ui: &mut Ui, pcm: &[i16], range: &mut Range<usize>, height: f32) -> bool {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, style::BACKGROUND);

    let frames = pcm.len().max(1);
    let x_of = |frame: usize| rect.left() + rect.width() * frame as f32 / frames as f32;
    let frame_of = |x: f32| {
        (((x - rect.left()) / rect.width()).clamp(0.0, 1.0) * frames as f32).round() as usize
    };

    // Dimmed outside the kept range.
    let (start_x, end_x) = (x_of(range.start), x_of(range.end));
    let mid = rect.center().y;
    for (i, (lo, hi)) in peaks(pcm, rect.width().max(1.0) as usize)
        .into_iter()
        .enumerate()
    {
        let x = rect.left() + i as f32 + 0.5;
        let color = if (start_x..=end_x).contains(&x) {
            style::WAVE
        } else {
            style::TEXT_DIM.gamma_multiply(0.5)
        };
        let half = rect.height() / 2.0 - 2.0;
        painter.line_segment(
            [Pos2::new(x, mid - hi * half), Pos2::new(x, mid - lo * half)],
            Stroke::new(1.0, color),
        );
    }
    painter.line_segment(
        [Pos2::new(rect.left(), mid), Pos2::new(rect.right(), mid)],
        Stroke::new(1.0, style::TEXT_DIM.gamma_multiply(0.4)),
    );

    let mut changed = false;
    for is_start in [true, false] {
        let x = if is_start {
            x_of(range.start)
        } else {
            x_of(range.end)
        };
        let grab = Rect::from_min_max(
            Pos2::new(x - HANDLE_GRAB / 2.0, rect.top()),
            Pos2::new(x + HANDLE_GRAB / 2.0, rect.bottom()),
        );
        let id = ui.id().with(("trim-handle", is_start));
        let response = ui.interact(grab, id, Sense::drag());
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
        }
        if let Some(pos) = response
            .interact_pointer_pos()
            .filter(|_| response.dragged())
        {
            let frame = frame_of(pos.x);
            if is_start {
                range.start = frame.min(range.end.saturating_sub(1));
            } else {
                range.end = frame.max(range.start + 1).min(pcm.len());
            }
            changed = true;
        }
        let color = if response.hovered() || response.dragged() {
            style::TEXT
        } else {
            style::ACCENT
        };
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(2.0, color),
        );
        let flag = if is_start {
            Rect::from_min_size(Pos2::new(x, rect.top()), vec2(8.0, 8.0))
        } else {
            Rect::from_min_size(Pos2::new(x - 8.0, rect.top()), vec2(8.0, 8.0))
        };
        painter.rect_filled(flag, 0.0, color);
    }
    painter.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0, Color32::from_gray(70)),
        StrokeKind::Inside,
    );
    changed
}

#[cfg(test)]
mod tests {
    use super::peaks;

    #[test]
    fn peaks_cover_all_samples() {
        let pcm = [0, 16_384, -32_768, 0, 8_192, 0];
        assert_eq!(peaks(&pcm, 2), [(-1.0, 0.5), (0.0, 0.25)]);
        // More columns than samples: every column still has a sample.
        assert_eq!(peaks(&pcm[..2], 4).len(), 4);
        assert!(peaks(&[], 10).is_empty());
    }
}
