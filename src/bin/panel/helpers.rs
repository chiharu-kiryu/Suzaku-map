use suzaku_map::ime::gpu::CandidateQuad;
use suzaku_map::ui::UiLanguage;

pub(super) fn point_in_rect(x: f32, y: f32, rect: [f32; 4]) -> bool {
    let [rx, ry, rw, rh] = rect;
    x >= rx && x <= rx + rw && y >= ry && y <= ry + rh
}

/// Window managers, task switchers and desktop integrations can read this title.
/// It must never receive drafts, candidates or committed text.
pub(super) fn window_title(ui: UiLanguage, compact: bool) -> String {
    if compact {
        "Suzaku".into()
    } else {
        ui.tr("Suzaku · Input").into()
    }
}

#[allow(dead_code)]
pub(super) fn _quad_debug(_quad: &CandidateQuad) {}

#[cfg(test)]
mod tests {
    use super::point_in_rect;
    use super::window_title;
    use suzaku_map::ui::UiLanguage;

    #[test]
    fn point_in_rect_includes_edges() {
        assert!(point_in_rect(0.0, 0.0, [0.0, 0.0, 10.0, 20.0]));
        assert!(point_in_rect(10.0, 20.0, [0.0, 0.0, 10.0, 20.0]));
    }

    #[test]
    fn point_in_rect_rejects_points_outside_bounds() {
        assert!(!point_in_rect(-0.1, 0.0, [0.0, 0.0, 10.0, 20.0]));
        assert!(!point_in_rect(10.1, 10.0, [0.0, 0.0, 10.0, 20.0]));
        assert!(!point_in_rect(5.0, 20.1, [0.0, 0.0, 10.0, 20.0]));
    }

    #[test]
    fn point_in_rect_supports_negative_origins_and_zero_size() {
        assert!(point_in_rect(-4.0, -3.0, [-4.0, -3.0, 0.0, 0.0]));
        assert!(!point_in_rect(-4.1, -3.0, [-4.0, -3.0, 0.0, 0.0]));
    }

    #[test]
    fn window_title_is_localized_and_contains_no_diagnostics() {
        for ui in UiLanguage::ALL {
            assert_eq!(window_title(ui, false), ui.tr("Suzaku · Input"));
            assert!(!window_title(ui, false).contains('|'));
        }
    }

    #[test]
    fn compact_window_title_is_only_the_application_name() {
        for ui in UiLanguage::ALL {
            assert_eq!(window_title(ui, true), "Suzaku");
        }
    }
}
