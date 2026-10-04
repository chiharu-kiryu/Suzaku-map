//! Optional tablet-style layout. This is a floating, non-reserving bottom panel,
//! not a compositor keyboard surface or a change to the desktop work area.
use super::{PanelState, PanelWindowKind};
use suzaku_map::ime::gpu::{PanelChromeState, PanelLayoutMode, WgpuCandidateRenderer};
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};

const MARGIN: u32 = 12;
const MAX_LOGICAL_WIDTH: f64 = 1100.0;

#[derive(Default)]
pub(super) struct BottomLayoutState {
    floating_base_size: Option<LogicalSize<f64>>,
    floating_position: Option<PhysicalPosition<i32>>,
    requested_position: Option<DockPositionRequest>,
    limits: Option<PhysicalSize<u32>>,
    deferred_render: bool,
}

impl BottomLayoutState {
    fn note_render_wait(&mut self, pending: bool) -> bool {
        self.deferred_render = pending;
        pending
    }

    fn needs_render_retry(&self, pending: bool) -> bool {
        self.deferred_render || pending
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct DockPositionRequest {
    target: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    observed: Option<PhysicalPosition<i32>>,
}

fn dock_width(screen_width: u32, dpi: f64, scale: f32) -> u32 {
    let dpi = if dpi.is_finite() && dpi > 0.0 {
        dpi
    } else {
        1.0
    };
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    ((MAX_LOGICAL_WIDTH * dpi * f64::from(scale)).round() as u32)
        .min(screen_width.saturating_sub(MARGIN * 2).max(1))
        .max(1)
}

fn dock_position(
    origin: PhysicalPosition<i32>,
    screen: PhysicalSize<u32>,
    panel: PhysicalSize<u32>,
) -> PhysicalPosition<i32> {
    let margin = MARGIN.min(screen.height.saturating_sub(panel.height) / 2);
    let x = i64::from(origin.x) + i64::from(screen.width.saturating_sub(panel.width)) / 2;
    let y = i64::from(origin.y)
        + i64::from(
            screen
                .height
                .saturating_sub(panel.height.saturating_add(margin)),
        );
    PhysicalPosition::new(
        x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
    )
}

fn dock_size(
    screen: PhysicalSize<u32>,
    dpi: f64,
    scale: f32,
    chrome: &PanelChromeState,
) -> PhysicalSize<u32> {
    let maximum_height = screen.height.saturating_sub(MARGIN * 2).max(1);
    let mut width = dock_width(screen.width, dpi, scale);
    let minimum_width = ((320.0 * dpi.max(1.0)).round() as u32).min(width).max(1);
    // Responsive key heights grow with width. Prefer a narrower complete
    // keyboard over clipping candidates on short landscape screens. Column
    // transitions can increase the height, so a binary search is not valid.
    while width > minimum_width {
        let natural_height = WgpuCandidateRenderer::new(width as f32, 1.0)
            .preferred_input_panel_height(chrome)
            .ceil() as u32;
        if natural_height <= maximum_height {
            break;
        }
        width = ((f64::from(width) * 0.9).floor() as u32).max(minimum_width);
    }
    super::windowing::content_fitted_size(width, dpi, maximum_height, chrome)
}

impl PanelState {
    pub(super) fn defer_bottom_render(&mut self) -> bool {
        if self.bottom_layout_enabled() {
            self.update_bottom_layout();
        }
        self.bottom_layout
            .note_render_wait(self.bottom_resize_pending())
    }

    pub(super) fn bottom_render_needs_retry(&self) -> bool {
        self.bottom_layout_enabled()
            && self
                .bottom_layout
                .needs_render_retry(self.bottom_resize_pending())
    }

    pub(super) fn bottom_resize_pending(&self) -> bool {
        self.bottom_layout_enabled()
            && self.window_resize_state.awaiting_size(
                PhysicalSize::new(self.config.width, self.config.height),
                std::time::Instant::now(),
            )
    }

    pub(super) fn reset_dock_window_requests(&mut self) {
        self.bottom_layout.requested_position = None;
        self.bottom_layout.limits = None;
        self.bottom_layout.deferred_render = false;
    }

    pub(super) fn bottom_layout_enabled(&self) -> bool {
        self.kind == PanelWindowKind::Main
            && self.chrome.effective_panel_layout() == PanelLayoutMode::BottomDock
            && !self.chrome.compact_mode
    }

    pub(super) fn apply_panel_layout_change(&mut self, previous: PanelLayoutMode) {
        if self.kind != PanelWindowKind::Main || previous == self.chrome.effective_panel_layout() {
            return;
        }
        self.clear_pressed_interaction();
        self.interaction.touch_tap_pending = false;
        self.native_position = Default::default();
        self.reset_dock_window_requests();
        if self.chrome.effective_panel_layout() == PanelLayoutMode::BottomDock {
            let size = if self.chrome.compact_mode {
                self.expanded_window_size.unwrap_or(LogicalSize::new(
                    super::DEFAULT_PANEL_INNER_WIDTH,
                    super::DEFAULT_PANEL_INNER_HEIGHT,
                ))
            } else {
                self.window
                    .inner_size()
                    .to_logical(self.window.scale_factor())
            };
            self.bottom_layout.floating_base_size = Some(LogicalSize::new(
                size.width / f64::from(self.window_scale),
                size.height / f64::from(self.window_scale),
            ));
            self.bottom_layout.floating_position = if self.chrome.compact_mode {
                self.expanded_window_pos
            } else {
                self.window.outer_position().ok()
            };
            // Keep the selected tool and its draft; the default tool is the
            // keyboard. Subsequent frames must respect the user's fold choice.
            self.chrome.input_modes_expanded = true;
        } else if let Some(base) = self.bottom_layout.floating_base_size.take() {
            self.window_scale =
                super::windowing::resolve_window_scale_request(self.window_scale, base, false);
            let restored = LogicalSize::new(
                base.width * f64::from(self.window_scale),
                base.height * f64::from(self.window_scale),
            );
            self.expanded_window_size = Some(restored);
            self.expanded_window_base_size = Some(base);
            self.expanded_window_pos = self.bottom_layout.floating_position.take();
            if !self.chrome.compact_mode {
                self.window.set_min_inner_size(Some(LogicalSize::new(
                    super::MIN_PANEL_INNER_WIDTH,
                    super::MIN_PANEL_INNER_HEIGHT,
                )));
                self.window.set_max_inner_size(Some(LogicalSize::new(
                    super::MAX_PANEL_INNER_WIDTH,
                    super::MAX_PANEL_INNER_HEIGHT,
                )));
                self.request_panel_size(
                    self.fitted_size_for_width(
                        restored
                            .to_physical::<u32>(self.window.scale_factor())
                            .width,
                    ),
                );
                if let Some(position) = self.expanded_window_pos {
                    self.window.set_outer_position(position);
                }
            }
        }
        self.apply_window_decorations();
        self.last_scene = None;
        self.update_native_position();
        self.window.request_redraw();
    }

    pub(super) fn update_bottom_layout(&mut self) {
        if !self.bottom_layout_enabled()
            || self.interaction.pressed_interaction.is_some()
            || self.interaction.panel_dragging
            || self.interaction.handwriting_dragging
        {
            return;
        }
        // Prefer the public input's monitor, not its within-monitor position.
        // Without absolute coordinates (including relative-only Wayland clients),
        // keep the current monitor; never guess a foreign window's position.
        let cursor = self
            .native
            .frame
            .as_ref()
            .filter(|f| f.visible())
            .and_then(|f| f.cursor);
        let monitor = cursor
            .and_then(|cursor| {
                self.window.available_monitors().find(|monitor| {
                    super::native_position::contains_cursor(
                        super::native_position::Bounds {
                            position: monitor.position(),
                            size: monitor.size(),
                        },
                        cursor,
                    )
                })
            })
            .or_else(|| self.window.current_monitor())
            .or_else(|| self.window.primary_monitor());
        let Some(monitor) = monitor else {
            return;
        };
        let bounds = monitor.size();
        // Dock geometry supplies its own bounds, including screens narrower
        // than the ordinary floating editor's minimum width.
        if self.bottom_layout.limits != Some(bounds) {
            self.window
                .set_min_inner_size(Some(PhysicalSize::new(1_u32, 1_u32)));
            self.window.set_max_inner_size(Some(bounds));
            self.bottom_layout.limits = Some(bounds);
        }
        let dpi = monitor.scale_factor();
        let target = dock_size(bounds, dpi, self.window_scale, &self.chrome);
        self.request_panel_size(target);
        let outer = self.window.outer_size();
        let position = dock_position(monitor.position(), bounds, outer);
        let request = DockPositionRequest {
            target: position,
            size: outer,
            observed: self.window.outer_position().ok(),
        };
        // Retry after an external move, but not forever if the WM clamps the
        // same target to the same observed position (or cannot report it).
        if self.bottom_layout.requested_position != Some(request) {
            self.bottom_layout.requested_position = Some(request);
            if request.observed != Some(position) {
                self.window.set_outer_position(position);
            }
            self.window.request_redraw();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deferred_render_gets_one_final_frame_after_resize_timeout() {
        let mut state = BottomLayoutState::default();
        assert!(!state.needs_render_retry(false));
        assert!(state.note_render_wait(true));
        assert!(state.needs_render_retry(true));
        // The timer must still schedule a final frame once awaiting_size has
        // expired, even if a clamping compositor sends no resize event.
        assert!(state.needs_render_retry(false));
        assert!(!state.note_render_wait(false));
        assert!(!state.needs_render_retry(false));
    }

    #[test]
    fn dock_width_fits_small_screens_and_bounds_large_displays() {
        assert_eq!(dock_width(1024, 1.0, 1.0), 1000);
        assert_eq!(dock_width(3840, 1.0, 1.0), 1100);
        assert_eq!(dock_width(3840, 2.0, 1.0), 2200);
        assert_eq!(dock_width(1920, 1.0, 0.8), 880);
        assert_eq!(dock_width(1, f64::NAN, f32::INFINITY), 1);
    }

    #[test]
    fn changing_height_keeps_bottom_edge_and_center_on_negative_monitors() {
        let origin = PhysicalPosition::new(-1920, -100);
        let screen = PhysicalSize::new(1920, 1080);
        for height in [180, 280, 540] {
            let position = dock_position(origin, screen, PhysicalSize::new(1100, height));
            assert_eq!(position.x, -1510);
            assert_eq!(position.y + height as i32, 968);
        }
        assert_eq!(
            dock_position(origin, PhysicalSize::new(10, 10), PhysicalSize::new(20, 20)),
            origin
        );
    }

    #[test]
    fn short_landscape_screen_shrinks_width_before_clipping_candidates() {
        let chrome = PanelChromeState {
            panel_layout_mode: PanelLayoutMode::BottomDock,
            input_modes_expanded: true,
            native_candidate_page: Some(suzaku_map::ime::gpu::NativeCandidatePage::new(
                0, 12, false,
            )),
            sentence_candidates: (0..6).map(|index| format!("candidate {index}")).collect(),
            ..Default::default()
        };
        for scale in [0.8, 1.0, 1.5] {
            let size = dock_size(PhysicalSize::new(1024, 480), 1.0, scale, &chrome);
            let natural = WgpuCandidateRenderer::new(size.width as f32, 1.0)
                .preferred_input_panel_height(&chrome)
                .ceil() as u32;
            assert_eq!(size.height, natural, "clipped at {size:?}");
            assert!(size.height <= 456 && size.width < 1000);
        }
    }
}
