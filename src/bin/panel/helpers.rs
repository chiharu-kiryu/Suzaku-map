use suzaku_map::ime::gpu::{CandidateQuad, RenderScene};
use suzaku_map::platform::{host_platform, support_for};

pub(super) fn point_in_rect(x: f32, y: f32, rect: [f32; 4]) -> bool {
    let [rx, ry, rw, rh] = rect;
    x >= rx && x <= rx + rw && y >= ry && y <= ry + rh
}

pub(super) fn window_title(
    scene: &RenderScene,
    committed_text: &str,
    uses_runtime_font: bool,
    font_label: &str,
) -> String {
    let selected = scene.selected_label.as_deref().unwrap_or("no candidate");
    let host = support_for(host_platform());
    format!(
        "Suzaku XR Candidate Panel | host: {:?} ({:?}) | font: {}:{} | selected: {selected} | draft: {} | committed: {committed_text} | keys: 1/2/3 seed, arrows move, D degrade, R reset, Space commit",
        host.platform,
        host.tier,
        if uses_runtime_font {
            "system-atlas"
        } else {
            "bitmap-fallback"
        },
        font_label,
        scene.draft_text,
    )
}

#[allow(dead_code)]
pub(super) fn _quad_debug(_quad: &CandidateQuad) {}

#[cfg(test)]
mod tests {
    use super::point_in_rect;

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
}
