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
