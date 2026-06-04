use super::PanelState;
use crate::helpers::point_in_rect;
use suzaku_map::ime::gpu::{InputMode, InteractionKind};
use suzaku_map::panel_support::{recognize_handwriting_candidates, summarize_handwriting_strokes};

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
        self.handwriting_dragging = true;
        self.chrome.handwriting_strokes.push(vec![[x, y]]);
        self.chrome.handwriting_hint = "Tracing… release to recognize".to_string();
        true
    }

    pub(super) fn extend_handwriting_stroke(&mut self) {
        if !self.handwriting_dragging || self.chrome.active_input_mode != InputMode::Handwriting {
            return;
        }
        let Some((x, y)) = self.cursor_position else {
            return;
        };
        let Some(rect) = self.handwriting_canvas_rect() else {
            return;
        };
        let clamped = [
            x.clamp(rect[0] + 4.0, rect[0] + rect[2] - 4.0),
            y.clamp(rect[1] + 4.0, rect[1] + rect[3] - 4.0),
        ];
        let Some(stroke) = self.chrome.handwriting_strokes.last_mut() else {
            return;
        };
        if stroke
            .last()
            .map(|last| (last[0] - clamped[0]).hypot(last[1] - clamped[1]) >= 3.0)
            .unwrap_or(true)
        {
            stroke.push(clamped);
        }
    }

    pub(super) fn finish_handwriting_stroke(&mut self) {
        if !self.handwriting_dragging {
            return;
        }
        self.handwriting_dragging = false;
        self.refresh_handwriting_candidates();
    }

    pub(super) fn undo_handwriting_stroke(&mut self) {
        self.handwriting_dragging = false;
        if self.chrome.handwriting_strokes.pop().is_none() {
            self.chrome.handwriting_hint = "Draw a seed word with mouse or touch.".to_string();
            return;
        }
        self.refresh_handwriting_candidates();
    }

    pub(super) fn clear_handwriting(&mut self) {
        self.handwriting_dragging = false;
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
