use super::{
    COMPACT_PANEL_INNER_HEIGHT, COMPACT_PANEL_INNER_WIDTH, DockEdge, MAX_PANEL_INNER_HEIGHT,
    MAX_PANEL_INNER_WIDTH, MIN_PANEL_INNER_HEIGHT, MIN_PANEL_INNER_WIDTH, PANEL_SCALE_MAX,
    PANEL_SCALE_MIN, PANEL_SCALE_STEP, PanelState, PanelWindowKind,
};
use std::time::Duration;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};

fn compute_restored_expanded_position(
    compact_pos: Option<PhysicalPosition<i32>>,
    compact_dock_edge: Option<DockEdge>,
    expanded_window_pos: Option<PhysicalPosition<i32>>,
    monitor_position: PhysicalPosition<i32>,
    monitor_size: PhysicalSize<u32>,
    compact_size: (i32, i32),
    restored_size: (i32, i32),
) -> PhysicalPosition<i32> {
    let margin = 18_i32;
    let (compact_width, compact_height) = compact_size;
    let (restored_width, restored_height) = restored_size;

    let min_x = monitor_position.x + margin;
    let max_x = monitor_position.x + monitor_size.width as i32 - restored_width - margin;
    let min_y = monitor_position.y + margin;
    let max_y = monitor_position.y + monitor_size.height as i32 - restored_height - margin;

    let target = match (compact_pos, compact_dock_edge) {
        (Some(pos), Some(DockEdge::Left)) => PhysicalPosition::new(
            min_x.max(pos.x),
            pos.y.clamp(min_y, max_y.max(min_y)),
        ),
        (Some(pos), Some(DockEdge::Right)) => PhysicalPosition::new(
            (pos.x + compact_width - restored_width).clamp(min_x, max_x.max(min_x)),
            pos.y.clamp(min_y, max_y.max(min_y)),
        ),
        (Some(pos), Some(DockEdge::Top)) => {
            PhysicalPosition::new(pos.x.clamp(min_x, max_x.max(min_x)), min_y)
        }
        (Some(pos), Some(DockEdge::Bottom)) => PhysicalPosition::new(
            pos.x.clamp(min_x, max_x.max(min_x)),
            (pos.y + compact_height - restored_height).clamp(min_y, max_y.max(min_y)),
        ),
        _ => expanded_window_pos.unwrap_or_else(|| {
            PhysicalPosition::new(
                min_x.max(monitor_position.x + (monitor_size.width as i32 - restored_width) / 2),
                min_y.max(monitor_position.y + (monitor_size.height as i32 - restored_height) / 2),
            )
        }),
    };

    PhysicalPosition::new(
        target.x.clamp(min_x, max_x.max(min_x)),
        target.y.clamp(min_y, max_y.max(min_y)),
    )
}

const COMPACT_TOGGLE_DEBOUNCE: Duration = Duration::from_millis(180);
const COMPACT_DRAG_MOVE_PX: f32 = 2.5;
const COMPACT_SNAP_THRESHOLD_PX: i32 = 7;
const COMPACT_DRAG_TAP_MAX_MS: u64 = 120;
const WINDOW_SCALE_DRAG_PIXELS_PER_STEP: f32 = 160.0;
const MIN_SCALE_DRAG_EPSILON: f32 = 0.001;

fn scale_delta_from_drag(start_x: f32, cursor_x: f32) -> f32 {
    (cursor_x - start_x) / WINDOW_SCALE_DRAG_PIXELS_PER_STEP * PANEL_SCALE_STEP
}

fn is_significant_window_scale_change(current_scale: f32, target_scale: f32) -> bool {
    (target_scale - current_scale).abs() >= MIN_SCALE_DRAG_EPSILON
}

fn window_scale_limits_for_base(base: LogicalSize<f64>) -> (f32, f32) {
    let min_scale_for_base = ((MIN_PANEL_INNER_WIDTH / base.width)
        .max(MIN_PANEL_INNER_HEIGHT / base.height))
    .max(PANEL_SCALE_MIN as f64);
    let max_scale_for_base = ((MAX_PANEL_INNER_WIDTH / base.width)
        .min(MAX_PANEL_INNER_HEIGHT / base.height))
    .min(PANEL_SCALE_MAX as f64)
    .max(PANEL_SCALE_MIN as f64);
    (
        (min_scale_for_base as f32).max(PANEL_SCALE_MIN),
        max_scale_for_base as f32,
    )
}

fn resolve_window_scale_request(scale: f32, base: LogicalSize<f64>, quantize: bool) -> f32 {
    let clamped_scale = scale.clamp(PANEL_SCALE_MIN, PANEL_SCALE_MAX);
    let normalized_scale = if quantize {
        (clamped_scale / PANEL_SCALE_STEP).round() * PANEL_SCALE_STEP
    } else {
        clamped_scale
    };
    let (min_scale_for_base, max_scale_for_base) = window_scale_limits_for_base(base);
    normalized_scale.clamp(min_scale_for_base, max_scale_for_base)
}

fn compact_drag_gesture_is_valid(
    start_pos: Option<PhysicalPosition<i32>>,
    end_pos: Option<PhysicalPosition<i32>>,
    elapsed_ms: u64,
    drag_moved_hint: bool,
) -> bool {
    let has_drag_delta = if elapsed_ms > COMPACT_DRAG_TAP_MAX_MS {
        false
    } else {
        drag_moved_hint
    };
    match (start_pos, end_pos) {
        (Some(start), Some(end)) => {
            (end.x - start.x).abs() > 3 || (end.y - start.y).abs() > 3 || has_drag_delta
        }
        _ => has_drag_delta,
    }
}

fn next_scaled_expanded_record(
    logical_size: LogicalSize<f64>,
    window_scale: f32,
    is_main_window: bool,
    is_compact_mode: bool,
    current_size: Option<LogicalSize<f64>>,
    current_base: Option<LogicalSize<f64>>,
) -> (Option<LogicalSize<f64>>, Option<LogicalSize<f64>>) {
    if !is_main_window || is_compact_mode {
        return (current_size, current_base);
    }

    let Some(base_size) = scale_base_size_from_expanded(logical_size, window_scale) else {
        return (current_size, current_base);
    };

    (Some(logical_size), Some(base_size))
}

fn scale_base_size_from_expanded(
    logical_size: LogicalSize<f64>,
    scale: f32,
) -> Option<LogicalSize<f64>> {
    if !logical_size.width.is_finite()
        || !logical_size.height.is_finite()
        || !scale.is_finite()
    {
        return None;
    }
    if logical_size.width <= 0.0 || logical_size.height <= 0.0 || scale <= 0.0 {
        return None;
    }
    Some(LogicalSize::new(
        logical_size.width / scale as f64,
        logical_size.height / scale as f64,
    ))
}

fn compact_drag_exceeded_threshold(start: (f32, f32), current: (f32, f32)) -> bool {
    let (start_x, start_y) = start;
    let (cursor_x, cursor_y) = current;
    (cursor_x - start_x).abs() > COMPACT_DRAG_MOVE_PX
        || (cursor_y - start_y).abs() > COMPACT_DRAG_MOVE_PX
}

fn compact_snap_for_position(
    current_x: i32,
    current_y: i32,
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
    threshold: i32,
) -> (Option<DockEdge>, PhysicalPosition<i32>) {
    let max_x = max_x.max(min_x);
    let max_y = max_y.max(min_y);
    let clamped_y = current_y.clamp(min_y, max_y);
    let clamped_x = current_x.clamp(min_x, max_x);
    let current = PhysicalPosition::new(clamped_x, clamped_y);

    let distances = [
        (
            (clamped_x - min_x).abs(),
            DockEdge::Left,
            PhysicalPosition::new(min_x, clamped_y),
        ),
        (
            (clamped_x - max_x).abs(),
            DockEdge::Right,
            PhysicalPosition::new(max_x, clamped_y),
        ),
        (
            (clamped_y - min_y).abs(),
            DockEdge::Top,
            PhysicalPosition::new(clamped_x, min_y),
        ),
        (
            (clamped_y - max_y).abs(),
            DockEdge::Bottom,
            PhysicalPosition::new(clamped_x, max_y),
        ),
    ];

    let Some((distance, edge, snapped_pos)) = distances.into_iter().min_by_key(|(distance, _, _)| *distance)
    else {
        return (None, current);
    };

    if distance <= threshold {
        (Some(edge), snapped_pos)
    } else {
        (None, current)
    }
}

impl PanelState {
    pub(super) fn apply_compact_mode(&mut self, compact: bool) {
        if self.kind != PanelWindowKind::Main || self.chrome.compact_mode == compact {
            return;
        }

        if let Some(last_toggle) = self.last_compact_toggle {
            if last_toggle.elapsed() < COMPACT_TOGGLE_DEBOUNCE {
                return;
            }
        }

        if compact {
            self.expanded_window_size = Some(
                self.window
                    .inner_size()
                    .to_logical::<f64>(self.window.scale_factor()),
            );
            self.expanded_window_pos = self.window.outer_position().ok();
            self.chrome.settings_open = false;
            self.window.set_decorations(false);
            self.window.set_resizable(false);
            let _ = self.window.request_inner_size(LogicalSize::new(
                COMPACT_PANEL_INNER_WIDTH,
                COMPACT_PANEL_INNER_HEIGHT,
            ));
        } else {
            let restored = self.expanded_window_size.unwrap_or_else(|| {
                LogicalSize::new(
                    super::DEFAULT_PANEL_INNER_WIDTH,
                    super::DEFAULT_PANEL_INNER_HEIGHT,
                )
            });
            self.window.set_decorations(true);
            self.window.set_resizable(true);
            let _ = self.window.request_inner_size(restored);
            self.restore_expanded_window_position(restored);
        }
        self.chrome.compact_mode = compact;
        self.interaction.compact_hovered = false;
        self.interaction.compact_dragging = false;
        self.interaction.compact_drag_moved = false;
        self.interaction.compact_drag_start_cursor = None;
        self.interaction.compact_drag_start_window_pos = None;
        self.interaction.compact_drag_start_instant = None;
        self.interaction.scale_dragging = false;
        self.interaction.scale_drag_start_cursor_x = None;
        self.interaction.scale_drag_start_scale = self.window_scale;
        self.interaction.touch_tap_pending = false;
        self.interaction.touch_start_position = None;
        self.last_compact_toggle = Some(std::time::Instant::now());
    }

    pub(super) fn begin_compact_drag(&mut self) {
        if self.kind != PanelWindowKind::Main || !self.chrome.compact_mode {
            return;
        }
        self.interaction.compact_dragging = true;
        self.interaction.compact_drag_moved = false;
        self.interaction.compact_drag_start_cursor = self.cursor_position;
        self.interaction.compact_drag_start_window_pos = self.window.outer_position().ok();
        self.interaction.compact_drag_start_instant = Some(std::time::Instant::now());
        let _ = self.window.drag_window();
    }

    pub(super) fn record_compact_drag_motion(&mut self, cursor_x: f32, cursor_y: f32) {
        if !self.interaction.compact_dragging {
            self.interaction.compact_drag_moved = false;
            return;
        }
        if let Some((start_x, start_y)) = self.interaction.compact_drag_start_cursor {
            let moved = compact_drag_exceeded_threshold((start_x, start_y), (cursor_x, cursor_y));
            if moved {
                self.interaction.compact_drag_moved = true;
            }
        }
    }

    pub(super) fn update_compact_hover(&mut self) {
        self.interaction.compact_hovered = if self.kind == PanelWindowKind::Main && self.chrome.compact_mode {
            match self.cursor_position {
                Some((x, y)) => {
                    let snapshot = self.engine.snapshot();
                    self.renderer
                        .build_compact_scene(&snapshot, &self.chrome, false, self.interaction.compact_dragging)
                        .hit_interaction(x, y)
                        == Some(suzaku_map::ime::gpu::InteractionKind::ToggleCompactMode)
                }
                None => false,
            }
        } else {
            false
        };
    }

    pub(super) fn end_compact_drag(&mut self) -> bool {
        if !self.interaction.compact_dragging {
            return false;
        }
        let elapsed_ms = self
            .interaction
            .compact_drag_start_instant
            .map(|instant| instant.elapsed().as_millis() as u64)
            .unwrap_or(u64::MAX);
        let moved = match (
            self.interaction.compact_drag_start_window_pos,
            self.window.outer_position().ok(),
        ) {
            (Some(start), Some(end)) => compact_drag_gesture_is_valid(
                Some(start),
                Some(end),
                elapsed_ms,
                self.interaction.compact_drag_moved,
            ),
            _ => compact_drag_gesture_is_valid(
                None,
                None,
                elapsed_ms,
                self.interaction.compact_drag_moved,
            ),
        };
        self.interaction.compact_dragging = false;
        self.interaction.compact_drag_start_cursor = None;
        self.interaction.compact_drag_start_window_pos = None;
        self.interaction.compact_drag_start_instant = None;
        self.interaction.compact_drag_moved = false;
        if moved {
            self.snap_compact_window_to_edge();
        }
        self.update_compact_hover();
        moved
    }

    fn snap_compact_window_to_edge(&mut self) {
        if self.kind != PanelWindowKind::Main || !self.chrome.compact_mode {
            return;
        }
        let Ok(current_pos) = self.window.outer_position() else {
            return;
        };
        let Some(monitor) = self.window.current_monitor() else {
            return;
        };
        let monitor_pos = monitor.position();
        let monitor_size = monitor.size();
        let margin = 12_i32;
        let win_w = self.size.width as i32;
        let win_h = self.size.height as i32;
        let min_x = monitor_pos.x + margin;
        let max_x = monitor_pos.x + monitor_size.width as i32 - win_w - margin;
        let min_y = monitor_pos.y + margin;
        let max_y = monitor_pos.y + monitor_size.height as i32 - win_h - margin;
        let threshold = COMPACT_SNAP_THRESHOLD_PX;
        let (edge, snapped) = compact_snap_for_position(
            current_pos.x,
            current_pos.y,
            min_x,
            max_x,
            min_y,
            max_y,
            threshold,
        );

        if let Some(edge) = edge {
            self.window.set_outer_position(snapped);
            self.compact_dock_edge = Some(edge);
        } else {
            self.window.set_outer_position(snapped);
            self.compact_dock_edge = None;
        }
    }

    fn restore_expanded_window_position(&mut self, restored_size: LogicalSize<f64>) {
        let compact_pos = self.window.outer_position().ok();
        let Some(monitor) = self.window.current_monitor() else {
            if let Some(saved) = self.expanded_window_pos {
                self.window.set_outer_position(saved);
            }
            return;
        };
        let monitor_pos = monitor.position();
        let monitor_size = monitor.size();
        let restored_w = restored_size.width.round() as i32;
        let restored_h = restored_size.height.round() as i32;
        let target = compute_restored_expanded_position(
            compact_pos,
            self.compact_dock_edge,
            self.expanded_window_pos,
            monitor_pos,
            monitor_size,
            (self.size.width as i32, self.size.height as i32),
            (restored_w, restored_h),
        );
        self.window.set_outer_position(target);
        self.expanded_window_pos = Some(target);
    }

    pub(super) fn note_expanded_window_position(&mut self) {
        if self.kind == PanelWindowKind::Main && !self.chrome.compact_mode {
            self.expanded_window_pos = self.window.outer_position().ok();
        }
    }

    pub(super) fn adjust_window_scale(&mut self, step_delta: i32) {
        if self.kind != PanelWindowKind::Main || self.chrome.compact_mode {
            return;
        }
        let current_scale = self.window_scale;
        let target_scale = (current_scale + step_delta as f32 * PANEL_SCALE_STEP)
            .clamp(PANEL_SCALE_MIN, PANEL_SCALE_MAX);
        self.set_window_scale(target_scale);
    }

    pub(super) fn set_window_scale(&mut self, scale: f32) {
        self.set_window_scale_internal(scale, true);
    }

    pub(super) fn set_window_scale_continuous(&mut self, scale: f32) {
        self.set_window_scale_internal(scale, false);
    }

    pub(super) fn begin_window_scale_drag(&mut self) {
        if self.kind != PanelWindowKind::Main || self.chrome.compact_mode {
            return;
        }
        self.interaction.scale_dragging = true;
        self.interaction.scale_drag_start_scale = self.window_scale;
        self.interaction.scale_drag_start_cursor_x = self.cursor_position.map(|(x, _)| x);
    }

    pub(super) fn update_window_scale_drag(&mut self, cursor_x: f32) {
        if !self.interaction.scale_dragging || self.kind != PanelWindowKind::Main || self.chrome.compact_mode {
            return;
        }
        let Some(start_x) = self.interaction.scale_drag_start_cursor_x else {
            return;
        };

        let scale_delta = scale_delta_from_drag(start_x, cursor_x);
        let target_scale = self.interaction.scale_drag_start_scale + scale_delta;
        self.set_window_scale_continuous(target_scale);
    }

    pub(super) fn end_window_scale_drag(&mut self) {
        if self.interaction.scale_dragging {
            self.interaction.scale_dragging = false;
            self.interaction.scale_drag_start_cursor_x = None;
            self.interaction.scale_drag_start_scale = self.window_scale;
            self.rebuild_font_atlas();
        }
    }

    fn set_window_scale_internal(&mut self, scale: f32, quantize: bool) {
        if self.kind != PanelWindowKind::Main || self.chrome.compact_mode {
            return;
        }

        let base = match self.expanded_window_base_size {
            Some(base_size) => base_size,
            None => self.expanded_window_size.unwrap_or_else(|| {
                self.window
                    .inner_size()
                    .to_logical::<f64>(self.window.scale_factor())
            }),
        };
        let target_scale = resolve_window_scale_request(scale, base, quantize);
        if !is_significant_window_scale_change(self.window_scale, target_scale) {
            return;
        }

        let target = LogicalSize::new(
            base.width * target_scale as f64,
            base.height * target_scale as f64,
        );
        let _ = self.window.request_inner_size(target);
        self.expanded_window_size = Some(target);
        self.window_scale = target_scale;
        self.expanded_window_base_size = Some(base);
        self.persist_display_settings();
        if quantize {
            self.rebuild_font_atlas();
        }
    }

    pub(super) fn reset_window_scale(&mut self) {
        self.set_window_scale(1.0);
    }

    pub(super) fn scale_window_by_wheel_delta(&mut self, delta: f32) {
        if delta > 0.0 {
            self.adjust_window_scale(1);
        } else if delta < 0.0 {
            self.adjust_window_scale(-1);
        }
    }

    pub(super) fn record_scaled_expanded_size(&mut self, logical_size: LogicalSize<f64>) {
        if self.kind != PanelWindowKind::Main || self.chrome.compact_mode {
            return;
        }
        let (next_size, next_base) = next_scaled_expanded_record(
            logical_size,
            self.window_scale,
            self.kind == PanelWindowKind::Main,
            self.chrome.compact_mode,
            self.expanded_window_size,
            self.expanded_window_base_size,
        );
        self.expanded_window_size = next_size;
        self.expanded_window_base_size = next_base;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ScaleDragDeltaCase {
        name: &'static str,
        start_x: f32,
        cursor_x: f32,
        expected: Option<f32>,
        max_abs: Option<f32>,
    }

    fn run_scale_drag_delta_cases(cases: &[ScaleDragDeltaCase]) {
        for case in cases {
            let delta = scale_delta_from_drag(case.start_x, case.cursor_x);
            if let Some(expected) = case.expected {
                assert_eq!(delta, expected, "{}", case.name);
            }
            if let Some(max_abs) = case.max_abs {
                assert!(delta.abs() < max_abs, "{}", case.name);
            }
        }
    }

    #[test]
    fn window_scale_drag_delta_decision_matrix() {
        let cases = [
            ScaleDragDeltaCase {
                name: "static_cursor_produces_zero_delta",
                start_x: 120.0,
                cursor_x: 120.0,
                expected: Some(0.0),
                max_abs: None,
            },
            ScaleDragDeltaCase {
                name: "upward_drag_tracks_full_pixel_step",
                start_x: 100.0,
                cursor_x: 260.0,
                expected: Some(PANEL_SCALE_STEP),
                max_abs: None,
            },
            ScaleDragDeltaCase {
                name: "downward_drag_tracks_full_negative_pixel_step",
                start_x: 260.0,
                cursor_x: 100.0,
                expected: Some(-PANEL_SCALE_STEP),
                max_abs: None,
            },
            ScaleDragDeltaCase {
                name: "small_drag_stays_under_scale_step",
                start_x: 100.0,
                cursor_x: 180.0,
                expected: None,
                max_abs: Some(PANEL_SCALE_STEP),
            },
        ];

        run_scale_drag_delta_cases(&cases);
    }

    struct WindowScaleLimitsCase {
        name: &'static str,
        base: LogicalSize<f64>,
        expected_min: f32,
        expected_max: f32,
    }

    fn run_window_scale_limits_cases(cases: &[WindowScaleLimitsCase]) {
        for case in cases {
            let (min_scale_for_base, max_scale_for_base) = window_scale_limits_for_base(case.base);
            assert!(
                (min_scale_for_base - case.expected_min).abs() < 1e-6,
                "{}: expected min {}, got {}",
                case.name,
                case.expected_min,
                min_scale_for_base
            );
            assert!(
                (max_scale_for_base - case.expected_max).abs() < 1e-6,
                "{}: expected max {}, got {}",
                case.name,
                case.expected_max,
                max_scale_for_base
            );
            assert!(
                min_scale_for_base <= max_scale_for_base,
                "{}: min should not exceed max",
                case.name
            );
            assert!(
                min_scale_for_base >= PANEL_SCALE_MIN && max_scale_for_base >= PANEL_SCALE_MIN,
                "{}: scales should respect global min",
                case.name
            );
            assert!(
                max_scale_for_base <= PANEL_SCALE_MAX,
                "{}: scales should respect global max",
                case.name
            );
        }
    }

    #[test]
    fn window_scale_limits_for_base_decision_matrix() {
        let cases = [
            WindowScaleLimitsCase {
                name: "limits_for_default_window_keep_expected_range",
                base: LogicalSize::new(900.0, 520.0),
                expected_min: 0.65,
                expected_max: 1.55,
            },
            WindowScaleLimitsCase {
                name: "tiny_width_promotes_min_scale",
                base: LogicalSize::new(300.0, 520.0),
                expected_min: 1.4,
                expected_max: 1.55,
            },
            WindowScaleLimitsCase {
                name: "very_large_base_collapses_to_global_min",
                base: LogicalSize::new(5000.0, 5000.0),
                expected_min: 0.65,
                expected_max: 0.65,
            },
            WindowScaleLimitsCase {
                name: "tall_aspect_limits_by_height",
                base: LogicalSize::new(900.0, 1000.0),
                expected_min: 0.65,
                expected_max: 0.806,
            },
        ];

        run_window_scale_limits_cases(&cases);
    }

    struct CompactDragThresholdCase {
        name: &'static str,
        start: (f32, f32),
        current: (f32, f32),
        expected: bool,
    }

    fn run_compact_drag_threshold_cases(cases: &[CompactDragThresholdCase]) {
        for case in cases {
            assert_eq!(
                compact_drag_exceeded_threshold(case.start, case.current),
                case.expected,
                "{}",
                case.name,
            );
        }
    }

    #[test]
    fn compact_drag_exceeded_threshold_decision_matrix() {
        let cases = [
            CompactDragThresholdCase {
                name: "below_or_at_threshold_x_axis_is_false",
                start: (10.0, 20.0),
                current: (12.5, 20.0),
                expected: false,
            },
            CompactDragThresholdCase {
                name: "above_threshold_x_axis_is_true",
                start: (10.0, 20.0),
                current: (12.6, 20.0),
                expected: true,
            },
            CompactDragThresholdCase {
                name: "above_threshold_on_y_axis_is_true",
                start: (10.0, 20.0),
                current: (10.0, 22.6),
                expected: true,
            },
        ];

        run_compact_drag_threshold_cases(&cases);
    }

    #[test]
    fn compact_snap_decision_matrix() {
        struct CompactSnapCase {
            name: &'static str,
            current_x: i32,
            current_y: i32,
            min_x: i32,
            max_x: i32,
            min_y: i32,
            max_y: i32,
            threshold: i32,
            expected_edge: Option<DockEdge>,
            expected_pos: winit::dpi::PhysicalPosition<i32>,
        }

        let cases = [
            CompactSnapCase {
                name: "snaps_to_left_edge_within_threshold",
                current_x: 103,
                current_y: 400,
                min_x: 100,
                max_x: 500,
                min_y: 100,
                max_y: 500,
                threshold: 7,
                expected_edge: Some(DockEdge::Left),
                expected_pos: PhysicalPosition::new(100, 400),
            },
            CompactSnapCase {
                name: "snaps_to_right_edge_within_threshold",
                current_x: 494,
                current_y: 401,
                min_x: 100,
                max_x: 500,
                min_y: 100,
                max_y: 500,
                threshold: 7,
                expected_edge: Some(DockEdge::Right),
                expected_pos: PhysicalPosition::new(500, 401),
            },
            CompactSnapCase {
                name: "snaps_to_top_edge_within_threshold",
                current_x: 250,
                current_y: 102,
                min_x: 100,
                max_x: 500,
                min_y: 100,
                max_y: 500,
                threshold: 7,
                expected_edge: Some(DockEdge::Top),
                expected_pos: PhysicalPosition::new(250, 100),
            },
            CompactSnapCase {
                name: "snaps_to_bottom_edge_within_threshold",
                current_x: 250,
                current_y: 494,
                min_x: 100,
                max_x: 500,
                min_y: 100,
                max_y: 500,
                threshold: 7,
                expected_edge: Some(DockEdge::Bottom),
                expected_pos: PhysicalPosition::new(250, 500),
            },
            CompactSnapCase {
                name: "does_not_snap_when_far_from_edges",
                current_x: 250,
                current_y: 300,
                min_x: 100,
                max_x: 500,
                min_y: 100,
                max_y: 500,
                threshold: 7,
                expected_edge: None,
                expected_pos: PhysicalPosition::new(250, 300),
            },
        ];

        for case in cases {
            let (edge, snapped) = compact_snap_for_position(
                case.current_x,
                case.current_y,
                case.min_x,
                case.max_x,
                case.min_y,
                case.max_y,
                case.threshold,
            );
            assert_eq!(edge, case.expected_edge, "{}", case.name);
            assert_eq!(snapped, case.expected_pos, "{}", case.name);
        }
    }

    struct ResolveWindowScaleRequestCase {
        name: &'static str,
        base: LogicalSize<f64>,
        request: f32,
        quantize: bool,
        expected_scale: f32,
    }

    fn run_resolve_window_scale_request_cases(cases: &[ResolveWindowScaleRequestCase]) {
        for case in cases {
            let result = resolve_window_scale_request(case.request, case.base, case.quantize);
            assert!(
                (result - case.expected_scale).abs() < 1e-6,
                "{}: expected {}, got {}",
                case.name,
                case.expected_scale,
                result
            );
        }
    }

    #[test]
    fn resolve_window_scale_request_decision_matrix() {
        let base = LogicalSize::new(900.0, 520.0);
        let (_, base_max) = window_scale_limits_for_base(base);
        let base_max_request = base_max + 0.5;
        let quantized_base_max = ((base_max / PANEL_SCALE_STEP).round() * PANEL_SCALE_STEP)
            .clamp(PANEL_SCALE_MIN, base_max);
        let quantized_request = 1.13;
        let quantized_expected = (quantized_request / PANEL_SCALE_STEP).round() * PANEL_SCALE_STEP;
        let max_scale_base = LogicalSize::new(2000.0, 2000.0);
        let (base_min_for_large, _) = window_scale_limits_for_base(max_scale_base);
        let base_min_request = base_min_for_large - 0.5;

        let cases = [
            ResolveWindowScaleRequestCase {
                name: "unquantized_scale_request_preserves_scale_inside_bounds",
                base,
                request: 1.2,
                quantize: false,
                expected_scale: 1.2,
            },
            ResolveWindowScaleRequestCase {
                name: "quantized_scale_request_rounds_to_step",
                base,
                request: quantized_request,
                quantize: true,
                expected_scale: quantized_expected,
            },
            ResolveWindowScaleRequestCase {
                name: "scale_request_above_max_is_clamped_to_base_max",
                base,
                request: base_max_request,
                quantize: false,
                expected_scale: base_max,
            },
            ResolveWindowScaleRequestCase {
                name: "quantized_scale_request_above_max_is_clamped_to_base_max",
                base,
                request: base_max_request,
                quantize: true,
                expected_scale: quantized_base_max,
            },
            ResolveWindowScaleRequestCase {
                name: "scale_request_below_min_is_clamped_to_base_min",
                base: max_scale_base,
                request: base_min_request,
                quantize: false,
                expected_scale: base_min_for_large,
            },
            ResolveWindowScaleRequestCase {
                name: "quantized_scale_request_below_min_is_clamped_to_base_min",
                base: max_scale_base,
                request: base_min_request,
                quantize: true,
                expected_scale: base_min_for_large,
            },
        ];

        run_resolve_window_scale_request_cases(&cases);
    }

    enum ScaleBaseExpectation {
        None,
        SomeSize { width: f64, height: f64 },
    }

    struct ScaleBaseSizeCase {
        name: &'static str,
        logical_size: LogicalSize<f64>,
        scale: f32,
        expected: ScaleBaseExpectation,
        assert_idempotent: bool,
    }

    fn run_scale_base_size_from_expanded_cases(cases: &[ScaleBaseSizeCase]) {
        for case in cases {
            let result =
                scale_base_size_from_expanded(case.logical_size, case.scale);
            match case.expected {
                ScaleBaseExpectation::None => assert!(result.is_none(), "{}", case.name),
                ScaleBaseExpectation::SomeSize { width, height } => {
                    let base = result.expect(case.name);
                    assert!((base.width - width).abs() < 1e-6, "{} width mismatch", case.name);
                    assert!((base.height - height).abs() < 1e-6, "{} height mismatch", case.name);
                    if case.assert_idempotent {
                        let again = scale_base_size_from_expanded(case.logical_size, case.scale);
                        assert_eq!(base, again.expect(case.name), "{}", case.name);
                    }
                }
            }
        }
    }

    #[test]
    fn scale_base_size_from_expanded_decision_matrix() {
        let cases = [
            ScaleBaseSizeCase {
                name: "rejects_non_finite_width",
                logical_size: LogicalSize::new(f64::INFINITY, 520.0),
                scale: 1.2,
                expected: ScaleBaseExpectation::None,
                assert_idempotent: false,
            },
            ScaleBaseSizeCase {
                name: "rejects_non_finite_height",
                logical_size: LogicalSize::new(900.0, f64::NAN),
                scale: 1.2,
                expected: ScaleBaseExpectation::None,
                assert_idempotent: false,
            },
            ScaleBaseSizeCase {
                name: "rejects_non_finite_scale",
                logical_size: LogicalSize::new(900.0, 520.0),
                scale: f32::INFINITY,
                expected: ScaleBaseExpectation::None,
                assert_idempotent: false,
            },
            ScaleBaseSizeCase {
                name: "rejects_zero_scale",
                logical_size: LogicalSize::new(900.0, 520.0),
                scale: 0.0,
                expected: ScaleBaseExpectation::None,
                assert_idempotent: false,
            },
            ScaleBaseSizeCase {
                name: "rejects_zero_width",
                logical_size: LogicalSize::new(0.0, 520.0),
                scale: 1.2,
                expected: ScaleBaseExpectation::None,
                assert_idempotent: false,
            },
            ScaleBaseSizeCase {
                name: "rejects_negative_width",
                logical_size: LogicalSize::new(-1.0, 520.0),
                scale: 1.2,
                expected: ScaleBaseExpectation::None,
                assert_idempotent: false,
            },
            ScaleBaseSizeCase {
                name: "rejects_zero_height",
                logical_size: LogicalSize::new(900.0, 0.0),
                scale: 1.2,
                expected: ScaleBaseExpectation::None,
                assert_idempotent: false,
            },
            ScaleBaseSizeCase {
                name: "keeps_scale_math_stable_for_valid_inputs",
                logical_size: LogicalSize::new(900.0, 520.0),
                scale: 1.3,
                expected: ScaleBaseExpectation::SomeSize {
                    width: 900.0 / 1.3f32 as f64,
                    height: 520.0 / 1.3f32 as f64,
                },
                assert_idempotent: false,
            },
            ScaleBaseSizeCase {
                name: "is_idempotent_for_same_input",
                logical_size: LogicalSize::new(900.0, 520.0),
                scale: 1.3,
                expected: ScaleBaseExpectation::SomeSize {
                    width: 900.0 / 1.3f32 as f64,
                    height: 520.0 / 1.3f32 as f64,
                },
                assert_idempotent: true,
            },
        ];

        run_scale_base_size_from_expanded_cases(&cases);
    }

    struct WindowScaleSignificanceCase {
        name: &'static str,
        current: f32,
        target: f32,
        expected: bool,
    }

    fn run_window_scale_significance_cases(cases: &[WindowScaleSignificanceCase]) {
        for case in cases {
            assert_eq!(
                is_significant_window_scale_change(case.current, case.target),
                case.expected,
                "{}",
                case.name,
            );
        }
    }

    #[test]
    fn is_significant_window_scale_change_decision_matrix() {
        let tiny_drag_delta = scale_delta_from_drag(100.0, 100.2);
        let cases = [
            WindowScaleSignificanceCase {
                name: "sub_epsilon_delta_is_false",
                current: 1.0,
                target: 1.0 + MIN_SCALE_DRAG_EPSILON * 0.75,
                expected: false,
            },
            WindowScaleSignificanceCase {
                name: "exact_epsilon_delta_is_true",
                current: 1.0,
                target: 1.0 + MIN_SCALE_DRAG_EPSILON,
                expected: true,
            },
            WindowScaleSignificanceCase {
                name: "above_epsilon_negative_delta_is_true",
                current: 1.0,
                target: 1.0 - MIN_SCALE_DRAG_EPSILON * 1.5,
                expected: true,
            },
            WindowScaleSignificanceCase {
                name: "tiny_drag_delta_from_drag_is_not_significant",
                current: 1.0,
                target: 1.0 + tiny_drag_delta,
                expected: false,
            },
        ];

        run_window_scale_significance_cases(&cases);
    }

    struct NextScaledExpandedRecordCase {
        name: &'static str,
        logical_size: LogicalSize<f64>,
        window_scale: f32,
        is_main_window: bool,
        is_compact_mode: bool,
        current_size: Option<LogicalSize<f64>>,
        current_base: Option<LogicalSize<f64>>,
        expected_size: Option<LogicalSize<f64>>,
        expected_base: Option<LogicalSize<f64>>,
    }

    fn run_next_scaled_expanded_record_cases(cases: &[NextScaledExpandedRecordCase]) {
        for case in cases {
            let (size, base) = next_scaled_expanded_record(
                case.logical_size,
                case.window_scale,
                case.is_main_window,
                case.is_compact_mode,
                case.current_size,
                case.current_base,
            );
            assert_eq!(size, case.expected_size, "{}", case.name);
            assert_eq!(base, case.expected_base, "{}", case.name);
        }
    }

    #[test]
    fn next_scaled_expanded_record_decision_matrix() {
        let cases = [
            NextScaledExpandedRecordCase {
                name: "updates_state_for_valid_main_window_size",
                logical_size: LogicalSize::new(900.0, 520.0),
                window_scale: 1.3,
                is_main_window: true,
                is_compact_mode: false,
                current_size: Some(LogicalSize::new(800.0, 500.0)),
                current_base: Some(LogicalSize::new(600.0, 380.0)),
                expected_size: Some(LogicalSize::new(900.0, 520.0)),
                expected_base: Some(LogicalSize::new(
                    900.0 / 1.3f32 as f64,
                    520.0 / 1.3f32 as f64,
                )),
            },
            NextScaledExpandedRecordCase {
                name: "keeps_state_for_non_main_window",
                logical_size: LogicalSize::new(900.0, 520.0),
                window_scale: 1.3,
                is_main_window: false,
                is_compact_mode: false,
                current_size: Some(LogicalSize::new(800.0, 500.0)),
                current_base: Some(LogicalSize::new(600.0, 380.0)),
                expected_size: Some(LogicalSize::new(800.0, 500.0)),
                expected_base: Some(LogicalSize::new(600.0, 380.0)),
            },
            NextScaledExpandedRecordCase {
                name: "keeps_state_for_compact_mode_or_invalid_size",
                logical_size: LogicalSize::new(-1.0, 520.0),
                window_scale: 1.3,
                is_main_window: true,
                is_compact_mode: false,
                current_size: Some(LogicalSize::new(800.0, 500.0)),
                current_base: Some(LogicalSize::new(600.0, 380.0)),
                expected_size: Some(LogicalSize::new(800.0, 500.0)),
                expected_base: Some(LogicalSize::new(600.0, 380.0)),
            },
            NextScaledExpandedRecordCase {
                name: "keeps_state_for_compact_mode",
                logical_size: LogicalSize::new(900.0, 520.0),
                window_scale: 1.3,
                is_main_window: true,
                is_compact_mode: true,
                current_size: Some(LogicalSize::new(700.0, 450.0)),
                current_base: Some(LogicalSize::new(500.0, 300.0)),
                expected_size: Some(LogicalSize::new(700.0, 450.0)),
                expected_base: Some(LogicalSize::new(500.0, 300.0)),
            },
        ];

        run_next_scaled_expanded_record_cases(&cases);
    }

    struct CompactDragGestureCase {
        name: &'static str,
        start: Option<PhysicalPosition<i32>>,
        end: Option<PhysicalPosition<i32>>,
        elapsed_ms: u64,
        hint: bool,
        expected: bool,
    }

    const IN_TAP_WINDOW_MS: u64 = COMPACT_DRAG_TAP_MAX_MS;
    const OUTSIDE_TAP_WINDOW_MS: u64 = COMPACT_DRAG_TAP_MAX_MS + 1;
    const FAR_ELAPSED_MS: u64 = 180;
    const ZERO_MS: u64 = 0;
    const ONE_MS: u64 = 1;
    const EARLY_MS: u64 = IN_TAP_WINDOW_MS - 1;

    fn run_compact_drag_gesture_cases(cases: &[CompactDragGestureCase]) {
        for case in cases {
            assert_eq!(
                compact_drag_gesture_is_valid(case.start, case.end, case.elapsed_ms, case.hint),
                case.expected,
                "{}",
                case.name,
            );
        }
    }

    #[test]
    fn compact_drag_gesture_decision_matrix() {
        // Keep all tap-window boundaries in one place for easier updates when
        // COMPACT_DRAG_TAP_MAX_MS changes.
        let base = PhysicalPosition::new(100, 100);
        let nearby = PhysicalPosition::new(103, 100);
        let far = PhysicalPosition::new(104, 100);
        let far_left = PhysicalPosition::new(96, 100);
        let _far_up = PhysicalPosition::new(100, 97);
        let far_down = PhysicalPosition::new(100, 104);

        let cases = [
            // Exact and just-over threshold (strictly > 3 is valid) on x/y axes.
            CompactDragGestureCase {
                name: "nearby_x_delta_never_snaps",
                start: Some(base),
                end: Some(nearby),
                elapsed_ms: FAR_ELAPSED_MS,
                hint: false,
                expected: false,
            },
            CompactDragGestureCase {
                name: "far_x_delta_crosses_threshold",
                start: Some(base),
                end: Some(far),
                elapsed_ms: FAR_ELAPSED_MS,
                hint: false,
                expected: true,
            },
            CompactDragGestureCase {
                name: "far_negative_x_delta_crosses_threshold",
                start: Some(base),
                end: Some(far_left),
                elapsed_ms: FAR_ELAPSED_MS,
                hint: false,
                expected: true,
            },
            CompactDragGestureCase {
                name: "far_negative_y_delta_crosses_threshold",
                start: Some(base),
                end: Some(far_left),
                elapsed_ms: FAR_ELAPSED_MS,
                hint: false,
                expected: true,
            },
            CompactDragGestureCase {
                name: "nearby_y_delta_never_snaps",
                start: Some(base),
                end: Some(PhysicalPosition::new(100, 103)),
                elapsed_ms: FAR_ELAPSED_MS,
                hint: false,
                expected: false,
            },
            CompactDragGestureCase {
                name: "far_y_delta_crosses_threshold",
                start: Some(base),
                end: Some(far_down),
                elapsed_ms: FAR_ELAPSED_MS,
                hint: false,
                expected: true,
            },
            // Hint only case: valid inside tap window, invalid once window passes.
            CompactDragGestureCase {
                name: "hint_accepted_inside_tap_window",
                start: Some(base),
                end: Some(base),
                elapsed_ms: 80,
                hint: true,
                expected: true,
            },
            CompactDragGestureCase {
                name: "hint_rejected_far_elapsed_for_complete_positions",
                start: Some(base),
                end: Some(base),
                elapsed_ms: FAR_ELAPSED_MS,
                hint: true,
                expected: false,
            },
            CompactDragGestureCase {
                name: "hint_accepted_on_tap_window_edge",
                start: Some(base),
                end: Some(base),
                elapsed_ms: IN_TAP_WINDOW_MS,
                hint: true,
                expected: true,
            },
            CompactDragGestureCase {
                name: "hint_accepted_at_zero_elapsed",
                start: Some(base),
                end: Some(base),
                elapsed_ms: ZERO_MS,
                hint: true,
                expected: true,
            },
            CompactDragGestureCase {
                name: "hint_accepted_at_one_elapsed",
                start: Some(base),
                end: Some(base),
                elapsed_ms: ONE_MS,
                hint: true,
                expected: true,
            },
            CompactDragGestureCase {
                name: "hint_rejected_after_tap_window_edge",
                start: Some(base),
                end: Some(base),
                elapsed_ms: OUTSIDE_TAP_WINDOW_MS,
                hint: true,
                expected: false,
            },
            CompactDragGestureCase {
                name: "hint_rejected_before_tap_window_starts_fails",
                start: Some(base),
                end: Some(base),
                elapsed_ms: IN_TAP_WINDOW_MS - 1,
                hint: true,
                expected: true,
            },
            // No-movement + no-hint is always invalid.
            CompactDragGestureCase {
                name: "no_hint_rejected_before_tap_window_ends",
                start: Some(base),
                end: Some(base),
                elapsed_ms: EARLY_MS,
                hint: false,
                expected: false,
            },
            CompactDragGestureCase {
                name: "no_hint_rejected_after_tap_window",
                start: Some(base),
                end: Some(base),
                elapsed_ms: FAR_ELAPSED_MS,
                hint: false,
                expected: false,
            },
            CompactDragGestureCase {
                name: "complete_position_hint_rejected_far_elapsed",
                start: Some(base),
                end: Some(nearby),
                elapsed_ms: FAR_ELAPSED_MS,
                hint: false,
                expected: false,
            },
        ];

        run_compact_drag_gesture_cases(&cases);
    }

    #[test]
    fn compact_drag_gesture_decision_matrix_without_complete_positions() {
        // Keep position-missing corner cases in one place because this path is easy to regress.
        let base = PhysicalPosition::new(100, 100);
        let cases = [
            // Partial positions: only `drag_moved_hint` + tap window decides validity.
            CompactDragGestureCase {
                name: "partial_position_hint_accepted_inside_tap_window",
                start: Some(base),
                end: None,
                elapsed_ms: 80,
                hint: true,
                expected: true,
            },
            CompactDragGestureCase {
                name: "partial_position_hint_rejected_far_elapsed",
                start: Some(base),
                end: None,
                elapsed_ms: FAR_ELAPSED_MS,
                hint: true,
                expected: false,
            },
            CompactDragGestureCase {
                name: "partial_position_no_hint_rejected_far_elapsed",
                start: Some(base),
                end: None,
                elapsed_ms: FAR_ELAPSED_MS,
                hint: false,
                expected: false,
            },
            CompactDragGestureCase {
                name: "reverse_partial_position_hint_accepted_on_edge",
                start: None,
                end: Some(base),
                elapsed_ms: IN_TAP_WINDOW_MS,
                hint: true,
                expected: true,
            },
            CompactDragGestureCase {
                name: "reverse_partial_position_hint_rejected_after_edge",
                start: None,
                end: Some(base),
                elapsed_ms: OUTSIDE_TAP_WINDOW_MS,
                hint: false,
                expected: false,
            },
            CompactDragGestureCase {
                name: "partial_position_hint_accepted_at_zero_elapsed",
                start: Some(base),
                end: None,
                elapsed_ms: ZERO_MS,
                hint: true,
                expected: true,
            },
            CompactDragGestureCase {
                name: "partial_position_hint_accepted_at_one_elapsed",
                start: Some(base),
                end: None,
                elapsed_ms: ONE_MS,
                hint: true,
                expected: true,
            },
            // Missing both positions: same tap-window rule and overflow guard.
            CompactDragGestureCase {
                name: "missing_positions_hint_accepted_on_tap_window_edge",
                start: None,
                end: None,
                elapsed_ms: IN_TAP_WINDOW_MS,
                hint: true,
                expected: true,
            },
            CompactDragGestureCase {
                name: "missing_positions_hint_rejected_after_tap_window",
                start: None,
                end: None,
                elapsed_ms: OUTSIDE_TAP_WINDOW_MS,
                hint: true,
                expected: false,
            },
            CompactDragGestureCase {
                name: "missing_positions_hint_rejected_far_elapsed",
                start: None,
                end: None,
                elapsed_ms: FAR_ELAPSED_MS,
                hint: true,
                expected: false,
            },
            CompactDragGestureCase {
                name: "missing_positions_hint_rejected_if_elapsed_overflows",
                start: None,
                end: None,
                elapsed_ms: u64::MAX,
                hint: true,
                expected: false,
            },
            CompactDragGestureCase {
                name: "missing_positions_hint_accepted_at_zero_elapsed",
                start: None,
                end: None,
                elapsed_ms: ZERO_MS,
                hint: true,
                expected: true,
            },
            CompactDragGestureCase {
                name: "missing_positions_hint_accepted_at_one_elapsed",
                start: None,
                end: None,
                elapsed_ms: ONE_MS,
                hint: true,
                expected: true,
            },
            CompactDragGestureCase {
                name: "missing_positions_without_hint_rejected_before_tap_window",
                start: None,
                end: None,
                elapsed_ms: EARLY_MS,
                hint: false,
                expected: false,
            },
            CompactDragGestureCase {
                name: "missing_positions_without_hint_rejected_after_tap_window",
                start: None,
                end: None,
                elapsed_ms: OUTSIDE_TAP_WINDOW_MS,
                hint: false,
                expected: false,
            },
            CompactDragGestureCase {
                name: "missing_positions_no_hint_rejected_far_elapsed",
                start: None,
                end: None,
                elapsed_ms: FAR_ELAPSED_MS,
                hint: false,
                expected: false,
            },
        ];

        run_compact_drag_gesture_cases(&cases);
    }

    fn compute_monitor() -> (PhysicalPosition<i32>, PhysicalSize<u32>) {
        (PhysicalPosition::new(20, 30), PhysicalSize::new(1024, 768))
    }

    struct RestoredExpandedPositionCase {
        name: &'static str,
        compact_pos: Option<PhysicalPosition<i32>>,
        compact_dock_edge: Option<DockEdge>,
        expanded_window_pos: Option<PhysicalPosition<i32>>,
        monitor_position: PhysicalPosition<i32>,
        monitor_size: PhysicalSize<u32>,
        compact_size: (i32, i32),
        restored_size: (i32, i32),
        expected: PhysicalPosition<i32>,
    }

    fn run_compute_restored_expanded_position_cases(cases: &[RestoredExpandedPositionCase]) {
        for case in cases {
            let target = compute_restored_expanded_position(
                case.compact_pos,
                case.compact_dock_edge,
                case.expanded_window_pos,
                case.monitor_position,
                case.monitor_size,
                case.compact_size,
                case.restored_size,
            );
            assert_eq!(target, case.expected, "{}", case.name);
        }
    }

    #[test]
    fn compute_restored_expanded_position_decision_matrix() {
        let (monitor_pos, monitor_size) = compute_monitor();
        let cases = [
            RestoredExpandedPositionCase {
                name: "prefers_left_edge_offset",
                compact_pos: Some(PhysicalPosition::new(35, 400)),
                compact_dock_edge: Some(DockEdge::Left),
                expanded_window_pos: None,
                monitor_position: monitor_pos,
                monitor_size,
                compact_size: (90, 90),
                restored_size: (400, 300),
                expected: PhysicalPosition::new(monitor_pos.x + 18, 400),
            },
            RestoredExpandedPositionCase {
                name: "prefers_right_edge_offset",
                compact_pos: Some(PhysicalPosition::new(35, 410)),
                compact_dock_edge: Some(DockEdge::Right),
                expanded_window_pos: None,
                monitor_position: monitor_pos,
                monitor_size,
                compact_size: (90, 90),
                restored_size: (400, 300),
                expected: PhysicalPosition::new(monitor_pos.x + 18, 410),
            },
            RestoredExpandedPositionCase {
                name: "prefers_top_edge_offset",
                compact_pos: Some(PhysicalPosition::new(300, 33)),
                compact_dock_edge: Some(DockEdge::Top),
                expanded_window_pos: None,
                monitor_position: monitor_pos,
                monitor_size,
                compact_size: (90, 90),
                restored_size: (400, 300),
                expected: PhysicalPosition::new(300, monitor_pos.y + 18),
            },
            RestoredExpandedPositionCase {
                name: "prefers_bottom_edge_offset",
                compact_pos: Some(PhysicalPosition::new(300, 680)),
                compact_dock_edge: Some(DockEdge::Bottom),
                expanded_window_pos: None,
                monitor_position: monitor_pos,
                monitor_size,
                compact_size: (90, 90),
                restored_size: (400, 300),
                expected: PhysicalPosition::new(300, 470),
            },
            RestoredExpandedPositionCase {
                name: "falls_back_to_saved_position_when_not_docked",
                compact_pos: Some(PhysicalPosition::new(20, 20)),
                compact_dock_edge: None,
                expanded_window_pos: Some(PhysicalPosition::new(400, 500)),
                monitor_position: monitor_pos,
                monitor_size,
                compact_size: (90, 90),
                restored_size: (400, 300),
                expected: PhysicalPosition::new(400, monitor_pos.y + 450),
            },
            RestoredExpandedPositionCase {
                name: "clamps_to_monitor_bounds_for_left_dock",
                compact_pos: Some(PhysicalPosition::new(2000, 2000)),
                compact_dock_edge: Some(DockEdge::Left),
                expanded_window_pos: Some(PhysicalPosition::new(-1000, -1000)),
                monitor_position: monitor_pos,
                monitor_size,
                compact_size: (90, 90),
                restored_size: (400, 300),
                expected: PhysicalPosition::new(626, monitor_pos.y + 450),
            },
        ];

        run_compute_restored_expanded_position_cases(&cases);
    }
}
