//! Place only our non-focusing native candidate view at a public IBus caret.
//! IBus absolute cursor rectangles are already in physical screen coordinates.
//! Missing/relative-only Wayland coordinates must never be guessed.
use super::{PanelState, PanelWindowKind};
use suzaku_map::ime::companion::{NativeComposition, NativeCursorRect};
use winit::dpi::{PhysicalPosition, PhysicalSize};

const EDGE_MARGIN: i64 = 12;
const CARET_GAP: i64 = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Anchor {
    host: String,
    context: u64,
    cursor: NativeCursorRect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Below,
    Above,
}

#[derive(Default)]
pub(super) struct NativePosition {
    anchor: Option<Anchor>,
    manual_anchor: Option<Anchor>,
    side: Option<Side>,
    requested: Option<(PhysicalPosition<i32>, PhysicalSize<u32>)>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Bounds {
    pub(super) position: PhysicalPosition<i32>,
    pub(super) size: PhysicalSize<u32>,
}

/// Geometry is metadata, not a new input transaction. Never let equal-revision
/// frames smuggle a changed candidate, field, selection or draft through here.
pub(super) fn geometry_update(
    previous: &NativeComposition,
    next: &NativeComposition,
) -> Option<Option<NativeCursorRect>> {
    (previous.host == next.host
        && previous.context == next.context
        && previous.revision == next.revision
        && previous.focused == next.focused
        && previous.private == next.private
        && previous.language == next.language
        && previous.seed == next.seed
        && previous.selected == next.selected
        && previous.candidates == next.candidates
        && previous.cursor != next.cursor)
        .then_some(next.cursor)
}

pub(super) fn contains_cursor(bounds: Bounds, cursor: NativeCursorRect) -> bool {
    let x = i64::from(cursor.x) + i64::from(cursor.width) / 2;
    let y = i64::from(cursor.y) + i64::from(cursor.height) / 2;
    let left = i64::from(bounds.position.x);
    let top = i64::from(bounds.position.y);
    left <= x
        && x < left + i64::from(bounds.size.width)
        && top <= y
        && y < top + i64::from(bounds.size.height)
}

fn clamp_axis(value: i64, origin: i64, extent: u32, window: u32) -> i32 {
    let end = origin + i64::from(extent);
    let window = i64::from(window);
    let margin = if window + EDGE_MARGIN * 2 <= i64::from(extent) {
        EDGE_MARGIN
    } else {
        0
    };
    let min = origin + margin;
    let max = (end - window - margin).max(min);
    value
        .clamp(min, max)
        .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn placement(
    cursor: NativeCursorRect,
    window: PhysicalSize<u32>,
    screen: Bounds,
    preferred: Option<Side>,
) -> (PhysicalPosition<i32>, Side) {
    let top = i64::from(screen.position.y);
    let bottom = top + i64::from(screen.size.height);
    let below = i64::from(cursor.y) + i64::from(cursor.height) + CARET_GAP;
    let above = i64::from(cursor.y) - CARET_GAP - i64::from(window.height);
    let below_fits = below + i64::from(window.height) <= bottom - EDGE_MARGIN;
    let above_fits = above >= top + EDGE_MARGIN;
    // Keep a side while it still fits: a short final page must not jump from
    // above to below the same input just because its height changed.
    let side = match preferred {
        Some(Side::Above) if above_fits => Side::Above,
        Some(Side::Below) if below_fits => Side::Below,
        _ if below_fits => Side::Below,
        _ if above_fits => Side::Above,
        _ if i64::from(cursor.y) - top > bottom - below => Side::Above,
        _ => Side::Below,
    };
    (
        PhysicalPosition::new(
            clamp_axis(
                i64::from(cursor.x),
                i64::from(screen.position.x),
                screen.size.width,
                window.width,
            ),
            clamp_axis(
                if side == Side::Below { below } else { above },
                top,
                screen.size.height,
                window.height,
            ),
        ),
        side,
    )
}

impl PanelState {
    fn native_anchor(&self) -> Option<Anchor> {
        let frame = self.native.frame.as_ref().filter(|frame| frame.visible())?;
        Some(Anchor {
            host: frame.host.clone(),
            context: frame.context,
            cursor: frame.cursor?,
        })
    }

    /// A manual drag pins this caret until it moves or another input takes over.
    pub(super) fn suspend_native_position_for_drag(&mut self) {
        self.native_position.manual_anchor = self.native_anchor();
        self.native_position.requested = None;
    }

    pub(super) fn native_position_active(&self) -> bool {
        self.kind == PanelWindowKind::Main
            && !self.bottom_layout_enabled()
            && self.runs_without_window_focus
            && self.native.showing
            && !self.chrome.compact_mode
            && !self.has_local_panel_interaction()
            && !self.interaction.panel_dragging
            && !self.interaction.scale_dragging
            && !self.interaction.handwriting_dragging
            && self.interaction.pressed_interaction.is_none()
            && self
                .native_anchor()
                .is_some_and(|anchor| self.native_position.manual_anchor.as_ref() != Some(&anchor))
    }

    pub(super) fn update_native_position(&mut self) {
        if self.bottom_layout_enabled() {
            self.update_bottom_layout();
            return;
        }
        if !self.native_position_active() {
            return;
        }
        let Some(anchor) = self.native_anchor() else {
            return;
        };
        let mut screens = self.window.available_monitors().map(|monitor| Bounds {
            position: monitor.position(),
            size: monitor.size(),
        });
        // A stale/off-screen rectangle is not a reason to move onto an unrelated
        // monitor. In particular, do not use the panel's previous current_monitor.
        let Some(screen) = screens.find(|screen| contains_cursor(*screen, anchor.cursor)) else {
            return;
        };
        let same_context = self
            .native_position
            .anchor
            .as_ref()
            .is_some_and(|previous| {
                previous.host == anchor.host && previous.context == anchor.context
            });
        let preferred = same_context.then_some(self.native_position.side).flatten();
        let outer = self.window.outer_size();
        let inner = self.window.inner_size();
        let maximum = PhysicalSize::new(
            screen
                .size
                .width
                .saturating_sub(EDGE_MARGIN as u32 * 2)
                .max(1),
            screen
                .size
                .height
                .saturating_sub(EDGE_MARGIN as u32 * 2)
                .max(1),
        );
        if outer.width > maximum.width || outer.height > maximum.height {
            // Account for any native decorations rather than treating inner and
            // outer sizes as interchangeable. No DPI multiplier is applied.
            let target = PhysicalSize::new(
                inner
                    .width
                    .min(
                        maximum
                            .width
                            .saturating_sub(outer.width.saturating_sub(inner.width)),
                    )
                    .max(1),
                inner
                    .height
                    .min(
                        maximum
                            .height
                            .saturating_sub(outer.height.saturating_sub(inner.height)),
                    )
                    .max(1),
            );
            self.request_panel_size(target);
        }
        let size = self.window.outer_size();
        let (position, side) = placement(anchor.cursor, size, screen, preferred);
        self.native_position.anchor = Some(anchor);
        self.native_position.side = Some(side);
        if self.native_position.requested == Some((position, size)) {
            return;
        }
        self.native_position.requested = Some((position, size));
        if self.window.outer_position().ok() != Some(position) {
            self.window.set_outer_position(position);
        }
        self.window.request_redraw();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(x: i32, y: i32, w: u32, h: u32) -> Bounds {
        Bounds {
            position: PhysicalPosition::new(x, y),
            size: PhysicalSize::new(w, h),
        }
    }

    fn cursor(x: i32, y: i32) -> NativeCursorRect {
        NativeCursorRect::new(x, y, 0, 20).unwrap()
    }

    #[test]
    fn native_candidates_prefer_below_and_flip_above_near_bottom() {
        let monitor = screen(0, 0, 1920, 1080);
        let size = PhysicalSize::new(420, 180);
        assert_eq!(
            placement(cursor(500, 100), size, monitor, None),
            (PhysicalPosition::new(500, 128), Side::Below)
        );
        assert_eq!(
            placement(cursor(1880, 1000), size, monitor, None),
            (PhysicalPosition::new(1488, 812), Side::Above)
        );
        assert_eq!(
            placement(cursor(5, 1), size, monitor, Some(Side::Above)),
            (PhysicalPosition::new(12, 29), Side::Below)
        );
    }

    #[test]
    fn shorter_pages_keep_the_same_side_and_do_not_cover_the_caret() {
        let monitor = screen(0, 0, 1024, 768);
        let rect = cursor(800, 520);
        let (_, side) = placement(rect, PhysicalSize::new(600, 260), monitor, None);
        assert_eq!(side, Side::Above);
        let (position, side) = placement(rect, PhysicalSize::new(600, 100), monitor, Some(side));
        assert_eq!(side, Side::Above);
        assert_eq!(position, PhysicalPosition::new(412, 412));
        assert!(position.y + 100 < rect.y);
    }

    #[test]
    fn monitor_selection_handles_negative_origins_and_absolute_scaled_coordinates() {
        let left = screen(-1920, -100, 1920, 1080);
        let right = screen(0, 0, 2560, 1440);
        let rect = cursor(-1500, 300);
        assert!(contains_cursor(left, rect));
        assert!(!contains_cursor(right, rect));
        assert_eq!(
            placement(rect, PhysicalSize::new(600, 200), left, None).0,
            PhysicalPosition::new(-1500, 328)
        );
        // A provider already reporting 2x screen coordinates is not scaled again.
        assert_eq!(
            placement(cursor(1600, 500), PhysicalSize::new(600, 200), right, None).0,
            PhysicalPosition::new(1600, 528)
        );
        assert!(!contains_cursor(left, cursor(9000, 8000)));
    }

    #[test]
    fn extreme_or_oversized_geometry_never_overflows_or_panics() {
        let tiny = screen(-20, -30, 100, 80);
        let (position, _) = placement(cursor(0, 0), PhysicalSize::new(1000, 1000), tiny, None);
        assert_eq!(position, PhysicalPosition::new(-20, -30));
        let edge = screen(i32::MAX - 200, i32::MIN, 200, 300);
        let (position, _) = placement(
            cursor(i32::MAX - 100, i32::MIN + 260),
            PhysicalSize::new(180, 100),
            edge,
            None,
        );
        assert!(position.x >= edge.position.x);
        assert!(position.y >= edge.position.y);
        assert!(i64::from(position.y) + 100 <= i64::from(edge.position.y) + 300);
    }

    #[test]
    fn geometry_only_updates_cannot_replace_semantic_state_at_the_same_revision() {
        use suzaku_map::ime::companion::NativeCandidate;
        let frame = NativeComposition {
            host: "00000000-0000-0000-0000-000000000001".into(),
            context: 2,
            revision: 3,
            focused: true,
            private: false,
            language: "en".into(),
            seed: "hel".into(),
            selected: 0,
            candidates: vec![NativeCandidate {
                text: "hello".into(),
                ..Default::default()
            }],
            cursor: Some(cursor(20, 30)),
        };
        let mut moved = frame.clone();
        moved.cursor = Some(cursor(40, 50));
        assert_eq!(geometry_update(&frame, &moved), Some(moved.cursor));
        moved.cursor = None;
        assert_eq!(geometry_update(&frame, &moved), Some(None));
        assert_eq!(geometry_update(&frame, &frame), None);
        for mutate in [
            |f: &mut NativeComposition| f.host.push('0'),
            |f: &mut NativeComposition| f.context += 1,
            |f: &mut NativeComposition| f.revision -= 1,
            |f: &mut NativeComposition| f.focused = false,
            |f: &mut NativeComposition| f.private = true,
            |f: &mut NativeComposition| f.language = "zh-Hans".into(),
            |f: &mut NativeComposition| f.seed.push('p'),
            |f: &mut NativeComposition| f.selected = 1,
            |f: &mut NativeComposition| f.candidates[0].text = "help".into(),
        ] {
            let mut changed = moved.clone();
            mutate(&mut changed);
            assert_eq!(geometry_update(&frame, &changed), None);
        }
    }
}
