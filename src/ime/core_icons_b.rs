use crate::ime::core_icons_a::{icon_arc, icon_curve, icon_line, icon_path, icon_point};
use crate::ime::gpu::CandidateQuad;
use std::f32::consts::PI;

pub(crate) fn append_mic_icon_quads(
    quads: &mut Vec<CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
) {
    let unit = rect[2].min(rect[3]);
    let point = icon_point(rect, [0.38, 0.15]);
    quads.push(CandidateQuad::outline(
        [point[0], point[1], unit * 0.24, unit * 0.43],
        color,
        unit * 0.12,
        unit * 0.065,
    ));
    icon_arc(quads, rect, [0.5, 0.46], 0.23, [0.0, PI], color, 0.065);
    icon_line(quads, rect, [0.5, 0.7], [0.5, 0.83], color, 0.065);
    icon_line(quads, rect, [0.36, 0.84], [0.64, 0.84], color, 0.065);
}

pub(crate) fn append_pen_icon_quads(
    quads: &mut Vec<CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
) {
    icon_path(
        quads,
        rect,
        &[
            [0.24, 0.60],
            [0.65, 0.19],
            [0.81, 0.35],
            [0.40, 0.76],
            [0.19, 0.81],
            [0.24, 0.60],
        ],
        color,
        0.060,
    );
    icon_line(quads, rect, [0.57, 0.28], [0.72, 0.43], color, 0.055);
    icon_line(quads, rect, [0.25, 0.62], [0.38, 0.75], color, 0.055);
}

pub(crate) fn append_backspace_icon_quads(
    quads: &mut Vec<CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
) {
    icon_path(
        quads,
        rect,
        &[
            [0.37, 0.26],
            [0.83, 0.26],
            [0.83, 0.74],
            [0.37, 0.74],
            [0.14, 0.5],
            [0.37, 0.26],
        ],
        color,
        0.060,
    );
    icon_line(quads, rect, [0.48, 0.4], [0.68, 0.6], color, 0.060);
    icon_line(quads, rect, [0.48, 0.6], [0.68, 0.4], color, 0.060);
}

pub(crate) fn append_refresh_icon_quads(
    quads: &mut Vec<CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
) {
    icon_arc(
        quads,
        rect,
        [0.5, 0.5],
        0.28,
        [-PI * 0.7, PI * 0.15],
        color,
        0.065,
    );
    icon_arc(
        quads,
        rect,
        [0.5, 0.5],
        0.28,
        [PI * 0.3, PI * 1.15],
        color,
        0.065,
    );
    icon_path(
        quads,
        rect,
        &[[0.65, 0.48], [0.77, 0.63], [0.86, 0.47]],
        color,
        0.065,
    );
    icon_path(
        quads,
        rect,
        &[[0.14, 0.53], [0.23, 0.37], [0.35, 0.52]],
        color,
        0.065,
    );
}

pub(crate) fn append_trash_icon_quads(
    quads: &mut Vec<CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
) {
    icon_path(
        quads,
        rect,
        &[[0.27, 0.34], [0.31, 0.79], [0.69, 0.79], [0.73, 0.34]],
        color,
        0.065,
    );
    icon_path(
        quads,
        rect,
        &[[0.39, 0.25], [0.41, 0.18], [0.59, 0.18], [0.61, 0.25]],
        color,
        0.060,
    );
    icon_line(quads, rect, [0.21, 0.28], [0.79, 0.28], color, 0.065);
    for x in [0.43, 0.57] {
        icon_line(quads, rect, [x, 0.43], [x, 0.68], color, 0.050);
    }
}

pub(crate) fn append_undo_icon_quads(
    quads: &mut Vec<CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
) {
    icon_curve(
        quads,
        rect,
        [[0.24, 0.38], [0.84, 0.15], [0.95, 0.78], [0.46, 0.77]],
        color,
        0.067,
        0.067,
    );
    icon_path(
        quads,
        rect,
        &[[0.23, 0.20], [0.22, 0.40], [0.43, 0.46]],
        color,
        0.067,
    );
}

pub(crate) fn append_seed_icon_quads(
    quads: &mut Vec<CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
) {
    icon_line(quads, rect, [0.5, 0.23], [0.5, 0.77], color, 0.065);
    icon_line(quads, rect, [0.23, 0.5], [0.77, 0.5], color, 0.065);
}

pub(crate) fn append_next_icon_quads(
    quads: &mut Vec<CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
) {
    icon_line(quads, rect, [0.20, 0.5], [0.77, 0.5], color, 0.065);
    icon_path(
        quads,
        rect,
        &[[0.54, 0.25], [0.79, 0.5], [0.54, 0.75]],
        color,
        0.065,
    );
}

pub(crate) fn append_chevron_icon_quads(
    quads: &mut Vec<CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
    expanded: bool,
) {
    let (edge, tip) = if expanded { (0.59, 0.36) } else { (0.41, 0.64) };
    icon_path(
        quads,
        rect,
        &[[0.27, edge], [0.5, tip], [0.73, edge]],
        color,
        0.07,
    );
}

pub(crate) fn append_close_icon_quads(
    quads: &mut Vec<CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
) {
    icon_line(quads, rect, [0.30, 0.30], [0.70, 0.70], color, 0.065);
    icon_line(quads, rect, [0.30, 0.70], [0.70, 0.30], color, 0.065);
}
