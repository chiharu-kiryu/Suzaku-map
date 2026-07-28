use super::{DockEdge, PanelState, PanelWindowKind};
use std::time::Duration;
use winit::dpi::{LogicalSize, PhysicalPosition};

const COMPACT_TOGGLE_DEBOUNCE: Duration = Duration::from_millis(180);
const COMPACT_DRAG_MOVE_PX: f32 = 2.5;
const COMPACT_SNAP_THRESHOLD_PX: i32 = 7;
const COMPACT_DRAG_TAP_MAX_MS: u64 = 120;

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
            let _ = self.window.request_inner_size(LogicalSize::new(92.0, 92.0));
        } else {
            let restored = self
                .expanded_window_size
                .unwrap_or_else(|| LogicalSize::new(420.0, 520.0));
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
}
