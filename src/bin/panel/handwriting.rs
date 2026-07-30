use super::PanelState;
use crate::helpers::point_in_rect;
use suzaku_map::ime::gpu::{InputMode, InteractionKind};
use suzaku_map::panel_support::{recognize_handwriting_candidates, summarize_handwriting_strokes};

const HANDWRITING_EDGE_PADDING: f32 = 4.0;
const HANDWRITING_MIN_POINT_DISTANCE: f32 = 1.2;
const HANDWRITING_INTERPOLATION_STEP: f32 = 3.2;
const HANDWRITING_SMOOTH_ALPHA: f32 = 0.22;
const HANDWRITING_SPEED_REFERENCE: f32 = 16.0;
const HANDWRITING_SPEED_MIN: f32 = 0.55;
const HANDWRITING_SPEED_MAX: f32 = 2.0;
const HANDWRITING_INTERPOLATION_STEP_MIN: f32 = 2.0;
const HANDWRITING_INTERPOLATION_STEP_MAX: f32 = 6.8;
const HANDWRITING_MIN_DISTANCE_MIN: f32 = 0.78;
const HANDWRITING_MIN_DISTANCE_MAX: f32 = 2.0;
const HANDWRITING_SMOOTH_ALPHA_MIN: f32 = 0.14;
const HANDWRITING_SMOOTH_ALPHA_MAX: f32 = 0.34;

fn clamp_handwriting_point(point: [f32; 2], rect: [f32; 4]) -> [f32; 2] {
    [
        point[0].clamp(rect[0] + HANDWRITING_EDGE_PADDING, rect[0] + rect[2] - HANDWRITING_EDGE_PADDING),
        point[1].clamp(rect[1] + HANDWRITING_EDGE_PADDING, rect[1] + rect[3] - HANDWRITING_EDGE_PADDING),
    ]
}

fn handwriting_speed_profile(distance: f32, previous_distance: f32) -> (f32, f32, f32) {
    let speed_hint = (distance.max(0.0) + previous_distance.max(0.0) * 0.65) * 0.5;
    let speed_ratio = (speed_hint / HANDWRITING_SPEED_REFERENCE)
        .clamp(HANDWRITING_SPEED_MIN, HANDWRITING_SPEED_MAX);

    let interpolation_step = (HANDWRITING_INTERPOLATION_STEP / speed_ratio)
        .clamp(HANDWRITING_INTERPOLATION_STEP_MIN, HANDWRITING_INTERPOLATION_STEP_MAX);
    let min_point_distance = (HANDWRITING_MIN_POINT_DISTANCE / speed_ratio)
        .clamp(HANDWRITING_MIN_DISTANCE_MIN, HANDWRITING_MIN_DISTANCE_MAX);
    let smooth_alpha = (HANDWRITING_SMOOTH_ALPHA * speed_ratio.sqrt())
        .clamp(HANDWRITING_SMOOTH_ALPHA_MIN, HANDWRITING_SMOOTH_ALPHA_MAX);

    (min_point_distance, interpolation_step, smooth_alpha)
}

fn append_handwriting_segment(stroke: &mut Vec<[f32; 2]>, next: [f32; 2]) {
    let Some(last) = stroke.last().copied() else {
        stroke.push(next);
        return;
    };

    let dx = next[0] - last[0];
    let dy = next[1] - last[1];
    let distance = dx.hypot(dy);
    let previous_distance = stroke
        .get(stroke.len().saturating_sub(2))
        .map(|previous| (last[0] - previous[0]).hypot(last[1] - previous[1]))
        .unwrap_or(0.0);

    let (min_point_distance, interpolation_step, smooth_alpha) =
        handwriting_speed_profile(distance, previous_distance);

    if distance < min_point_distance {
        return;
    }

    let segment_count = (distance / interpolation_step).ceil() as usize;
    let segment_count = segment_count.max(1);

    for segment in 1..=segment_count {
        let t = segment as f32 / segment_count as f32;
        let sampled = [last[0] + dx * t, last[1] + dy * t];
        let point = if segment == segment_count {
            sampled
        } else {
            let [prev_x, prev_y] = stroke.last().copied().unwrap_or(last);
            [
                prev_x + (sampled[0] - prev_x) * smooth_alpha,
                prev_y + (sampled[1] - prev_y) * smooth_alpha,
            ]
        };
        let [prev_x, prev_y] = stroke.last().copied().unwrap_or(point);
        if (point[0] - prev_x).hypot(point[1] - prev_y) >= min_point_distance * 0.72 {
            stroke.push(point);
        }
    }
}

impl PanelState {
    fn refresh_handwriting_candidates(&mut self) {
        self.last_handwriting_summary = Some(summarize_handwriting_strokes(
            &self.chrome.handwriting_strokes,
        ));
        self.chrome.handwriting_candidates =
            recognize_handwriting_candidates(&self.chrome.handwriting_strokes);
        self.reconfigure_llama_plugin();
        self.chrome.handwriting_hint = if self.chrome.handwriting_strokes.is_empty() {
            "Draw a seed word with mouse or touch.".to_string()
        } else if self.chrome.handwriting_candidates.is_empty() {
            "Try a clearer trace, undo a stroke, or tap Clear.".to_string()
        } else {
            "Tap a recognized seed, or undo the last stroke.".to_string()
        };
    }

    fn handwriting_canvas_rect(&self) -> Option<[f32; 4]> {
        self.current_scene()
            .interactive_targets
            .iter()
            .find(|target| matches!(target.kind, InteractionKind::HandwritingCanvas))
            .map(|target| target.rect)
    }

    pub(super) fn try_begin_handwriting_stroke(&mut self) -> bool {
        if self.chrome.active_input_mode != InputMode::Handwriting {
            return false;
        }
        let Some((x, y)) = self.cursor_position else {
            return false;
        };
        let Some(rect) = self.handwriting_canvas_rect() else {
            return false;
        };
        if !point_in_rect(x, y, rect) {
            return false;
        }

        self.chrome.blur_input();
        self.interaction.handwriting_dragging = true;
        self.chrome
            .handwriting_strokes
            .push(vec![clamp_handwriting_point([x, y], rect)]);
        self.chrome.handwriting_hint = "Tracing… release to recognize".to_string();
        true
    }

    pub(super) fn extend_handwriting_stroke(&mut self) {
        if !self.interaction.handwriting_dragging || self.chrome.active_input_mode != InputMode::Handwriting {
            return;
        }
        let Some((x, y)) = self.cursor_position else {
            return;
        };
        let Some(rect) = self.handwriting_canvas_rect() else {
            return;
        };
        let clamped = clamp_handwriting_point([x, y], rect);
        let Some(stroke) = self.chrome.handwriting_strokes.last_mut() else {
            return;
        };
        append_handwriting_segment(stroke, clamped);
    }

    pub(super) fn finish_handwriting_stroke(&mut self) {
        if !self.interaction.handwriting_dragging {
            return;
        }
        self.interaction.handwriting_dragging = false;
        self.refresh_handwriting_candidates();
    }

    pub(super) fn undo_handwriting_stroke(&mut self) {
        self.interaction.handwriting_dragging = false;
        if self.chrome.handwriting_strokes.pop().is_none() {
            self.chrome.handwriting_hint = "Draw a seed word with mouse or touch.".to_string();
            return;
        }
        self.refresh_handwriting_candidates();
    }

    pub(super) fn clear_handwriting(&mut self) {
        self.interaction.handwriting_dragging = false;
        self.chrome.handwriting_strokes.clear();
        self.chrome.handwriting_candidates.clear();
        self.chrome.handwriting_hint = "Draw a seed word with mouse or touch.".to_string();
        self.last_handwriting_summary = None;
        self.reconfigure_llama_plugin();
    }

    pub(super) fn insert_handwriting_candidate(&mut self, index: usize) {
        let Some(candidate) = self.chrome.handwriting_candidates.get(index).cloned() else {
            return;
        };
        if !self.chrome.seed_text.is_empty() && !self.chrome.seed_text.ends_with(' ') {
            self.chrome.insert_text(" ");
        }
        self.chrome.insert_text(&candidate);
        self.chrome.active_input_mode = InputMode::VirtualKeyboard;
        self.chrome.input_modes_expanded = false;
        self.chrome.focus_input();
        self.chrome.move_caret_to_end();
        self.sync_manual_seed_base();
        self.refresh_seed();
        self.clear_handwriting();
    }
}
