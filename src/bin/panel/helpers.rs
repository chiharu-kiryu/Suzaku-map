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
    use super::window_title;
    use suzaku_map::ime::gpu::RenderScene;

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
    fn window_title_includes_host_and_scene_summary() {
        let scene = RenderScene {
            quads: Vec::new(),
            text_quads: Vec::new(),
            atlas_glyphs: Vec::new(),
            text_sections: Vec::new(),
            hit_targets: Vec::new(),
            interactive_targets: Vec::new(),
            sentence_candidate_truncated: Vec::new(),
            next_token_candidate_truncated: Vec::new(),
            handwriting_candidate_truncated: Vec::new(),
            settings_option_truncated: Vec::new(),
            settings_scroll_metadata: None,
            labels: vec!["hello".to_string()],
            selected_label: Some("selected label".to_string()),
            draft_text: "draft text".to_string(),
        };

        let title = window_title(&scene, "committed", false, "Mono");

        assert!(title.contains("Suzaku XR Candidate Panel"));
        assert!(title.contains("selected: selected label"));
        assert!(title.contains("draft: draft text"));
        assert!(title.contains("committed: committed"));
        assert!(title.contains("font: bitmap-fallback"));
        assert!(title.contains("host: "));
    }

    #[test]
    fn window_title_shows_fallbacks_when_selection_missing() {
        let scene = RenderScene {
            quads: Vec::new(),
            text_quads: Vec::new(),
            atlas_glyphs: Vec::new(),
            text_sections: Vec::new(),
            hit_targets: Vec::new(),
            interactive_targets: Vec::new(),
            sentence_candidate_truncated: Vec::new(),
            next_token_candidate_truncated: Vec::new(),
            handwriting_candidate_truncated: Vec::new(),
            settings_option_truncated: Vec::new(),
            settings_scroll_metadata: None,
            labels: Vec::new(),
            selected_label: None,
            draft_text: String::new(),
        };

        let title = window_title(&scene, "", true, "system") ;

        assert!(title.contains("selected: no candidate"));
        assert!(title.contains("draft: "));
        assert!(title.contains("committed: "));
        assert!(title.contains("font: system-atlas"));
    }
}
