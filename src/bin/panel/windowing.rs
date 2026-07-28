use super::{
    COMPACT_PANEL_INNER_HEIGHT, COMPACT_PANEL_INNER_WIDTH, DockEdge, MIN_PANEL_INNER_HEIGHT,
    MIN_PANEL_INNER_WIDTH, PANEL_SCALE_MAX, PANEL_SCALE_MIN, PANEL_SCALE_STEP, PanelState,
    PanelWindowKind,
};
use std::time::Duration;
use winit::dpi::{LogicalSize, PhysicalPosition};

const COMPACT_TOGGLE_DEBOUNCE: Duration = Duration::from_millis(180);
const COMPACT_DRAG_MOVE_PX: f32 = 2.5;
const COMPACT_SNAP_THRESHOLD_PX: i32 = 7;
const COMPACT_DRAG_TAP_MAX_MS: u64 = 120;
const WINDOW_SCALE_DRAG_PIXELS_PER_STEP: f32 = 160.0;
const MIN_SCALE_DRAG_EPSILON: f32 = 0.001;

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
        self.compact_hovered = false;
        self.compact_dragging = false;
        self.compact_drag_moved = false;
        self.compact_drag_start_cursor = None;
        self.compact_drag_start_window_pos = None;
        self.compact_drag_start_instant = None;
        self.scale_dragging = false;
        self.scale_drag_start_cursor_x = None;
        self.scale_drag_start_scale = self.window_scale;
        self.touch_tap_pending = false;
        self.touch_start_position = None;
        self.last_compact_toggle = Some(std::time::Instant::now());
    }

    pub(super) fn begin_compact_drag(&mut self) {
        if self.kind != PanelWindowKind::Main || !self.chrome.compact_mode {
            return;
        }
        self.compact_dragging = true;
        self.compact_drag_moved = false;
        self.compact_drag_start_cursor = self.cursor_position;
        self.compact_drag_start_window_pos = self.window.outer_position().ok();
        self.compact_drag_start_instant = Some(std::time::Instant::now());
        let _ = self.window.drag_window();
    }

    pub(super) fn record_compact_drag_motion(&mut self, cursor_x: f32, cursor_y: f32) {
        if !self.compact_dragging {
            self.compact_drag_moved = false;
            return;
        }
        if let Some((start_x, start_y)) = self.compact_drag_start_cursor {
            let moved = (cursor_x - start_x).abs() > COMPACT_DRAG_MOVE_PX
                || (cursor_y - start_y).abs() > COMPACT_DRAG_MOVE_PX;
            if moved {
                self.compact_drag_moved = true;
            }
        }
    }

    pub(super) fn update_compact_hover(&mut self) {
        self.compact_hovered = if self.kind == PanelWindowKind::Main && self.chrome.compact_mode {
            match self.cursor_position {
                Some((x, y)) => {
                    let snapshot = self.engine.snapshot();
                    self.renderer
                        .build_compact_scene(&snapshot, &self.chrome, false, self.compact_dragging)
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
        if !self.compact_dragging {
            return false;
        }
        let elapsed_ms = self
            .compact_drag_start_instant
            .map(|instant| instant.elapsed().as_millis() as u64)
            .unwrap_or(u64::MAX);
        let has_drag_delta = if elapsed_ms > COMPACT_DRAG_TAP_MAX_MS {
            false
        } else {
            self.compact_drag_moved
        };
        let moved = match (
            self.compact_drag_start_window_pos,
            self.window.outer_position().ok(),
        ) {
            (Some(start), Some(end)) => {
                (end.x - start.x).abs() > 3 || (end.y - start.y).abs() > 3 || has_drag_delta
            }
            _ => self.compact_drag_moved,
        };
        self.compact_dragging = false;
        self.compact_drag_start_cursor = None;
        self.compact_drag_start_window_pos = None;
        self.compact_drag_start_instant = None;
        self.compact_drag_moved = false;
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
        let clamped_y = current_pos.y.clamp(min_y, max_y.max(min_y));
        let clamped_x = current_pos.x.clamp(min_x, max_x.max(min_x));
        let current = PhysicalPosition::new(clamped_x, clamped_y);
        let threshold = COMPACT_SNAP_THRESHOLD_PX;
        let distances = [
            (
                (clamped_x - min_x).abs(),
                DockEdge::Left,
                PhysicalPosition::new(min_x, clamped_y),
            ),
            (
                (clamped_x - max_x).abs(),
                DockEdge::Right,
                PhysicalPosition::new(max_x.max(min_x), clamped_y),
            ),
            (
                (clamped_y - min_y).abs(),
                DockEdge::Top,
                PhysicalPosition::new(clamped_x, min_y),
            ),
            (
                (clamped_y - max_y).abs(),
                DockEdge::Bottom,
                PhysicalPosition::new(clamped_x, max_y.max(min_y)),
            ),
        ];
        if let Some((_, edge, snapped)) = distances
            .into_iter()
            .min_by_key(|(distance, _, _)| *distance)
        {
            let distance = match edge {
                DockEdge::Left => (current.x - min_x).abs(),
                DockEdge::Right => (current.x - max_x.max(min_x)).abs(),
                DockEdge::Top => (current.y - min_y).abs(),
                DockEdge::Bottom => (current.y - max_y.max(min_y)).abs(),
            };
            if distance <= threshold {
                self.window.set_outer_position(snapped);
                self.compact_dock_edge = Some(edge);
            } else {
                self.window.set_outer_position(current);
                self.compact_dock_edge = None;
            }
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
        let margin = 18_i32;
        let restored_w = restored_size.width.round() as i32;
        let restored_h = restored_size.height.round() as i32;
        let min_x = monitor_pos.x + margin;
        let max_x = monitor_pos.x + monitor_size.width as i32 - restored_w - margin;
        let min_y = monitor_pos.y + margin;
        let max_y = monitor_pos.y + monitor_size.height as i32 - restored_h - margin;

        let target = match (compact_pos, self.compact_dock_edge) {
            (Some(pos), Some(DockEdge::Left)) => {
                PhysicalPosition::new(min_x.max(pos.x), pos.y.clamp(min_y, max_y.max(min_y)))
            }
            (Some(pos), Some(DockEdge::Right)) => PhysicalPosition::new(
                (pos.x + self.size.width as i32 - restored_w).clamp(min_x, max_x.max(min_x)),
                pos.y.clamp(min_y, max_y.max(min_y)),
            ),
            (Some(pos), Some(DockEdge::Top)) => {
                PhysicalPosition::new(pos.x.clamp(min_x, max_x.max(min_x)), min_y)
            }
            (Some(pos), Some(DockEdge::Bottom)) => PhysicalPosition::new(
                pos.x.clamp(min_x, max_x.max(min_x)),
                (pos.y + self.size.height as i32 - restored_h).clamp(min_y, max_y.max(min_y)),
            ),
            _ => self.expanded_window_pos.unwrap_or_else(|| {
                PhysicalPosition::new(
                    min_x.max(monitor_pos.x + (monitor_size.width as i32 - restored_w) / 2),
                    min_y.max(monitor_pos.y + (monitor_size.height as i32 - restored_h) / 2),
                )
            }),
        };
        self.window.set_outer_position(PhysicalPosition::new(
            target.x.clamp(min_x, max_x.max(min_x)),
            target.y.clamp(min_y, max_y.max(min_y)),
        ));
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
        self.scale_dragging = true;
        self.scale_drag_start_scale = self.window_scale;
        self.scale_drag_start_cursor_x = self.cursor_position.map(|(x, _)| x);
    }

    pub(super) fn update_window_scale_drag(&mut self, cursor_x: f32) {
        if !self.scale_dragging || self.kind != PanelWindowKind::Main || self.chrome.compact_mode {
            return;
        }
        let Some(start_x) = self.scale_drag_start_cursor_x else {
            return;
        };

        let scale_delta =
            (cursor_x - start_x) / WINDOW_SCALE_DRAG_PIXELS_PER_STEP * PANEL_SCALE_STEP;
        let target_scale = self.scale_drag_start_scale + scale_delta;
        self.set_window_scale_continuous(target_scale);
    }

    pub(super) fn end_window_scale_drag(&mut self) {
        if self.scale_dragging {
            self.scale_dragging = false;
            self.scale_drag_start_cursor_x = None;
            self.scale_drag_start_scale = self.window_scale;
        }
    }

    fn set_window_scale_internal(&mut self, scale: f32, quantize: bool) {
        if self.kind != PanelWindowKind::Main || self.chrome.compact_mode {
            return;
        }

        let clamped_scale = scale.clamp(PANEL_SCALE_MIN, PANEL_SCALE_MAX);
        let requested_scale = if quantize {
            (clamped_scale / PANEL_SCALE_STEP).round() * PANEL_SCALE_STEP
        } else {
            clamped_scale
        };
        let base = match self.expanded_window_base_size {
            Some(base_size) => base_size,
            None => self.expanded_window_size.unwrap_or_else(|| {
                self.window
                    .inner_size()
                    .to_logical::<f64>(self.window.scale_factor())
            }),
        };

        let min_scale_for_base = ((MIN_PANEL_INNER_WIDTH / base.width)
            .max(MIN_PANEL_INNER_HEIGHT / base.height))
        .max(PANEL_SCALE_MIN as f64);
        let target_scale = requested_scale.clamp(
            (min_scale_for_base as f32).max(PANEL_SCALE_MIN),
            PANEL_SCALE_MAX,
        );
        if (target_scale - self.window_scale).abs() < MIN_SCALE_DRAG_EPSILON {
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
        self.expanded_window_size = Some(logical_size);
        self.expanded_window_base_size = Some(LogicalSize::new(
            logical_size.width / self.window_scale as f64,
            logical_size.height / self.window_scale as f64,
        ));
    }
}
