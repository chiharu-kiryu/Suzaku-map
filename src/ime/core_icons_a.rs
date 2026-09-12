use crate::ime::gpu::CandidateQuad;

pub(crate) fn append_rounded_rect_quads(
    quads: &mut Vec<CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
    radius: f32,
) {
    if rect[2] > 0.0 && rect[3] > 0.0 {
        quads.push(CandidateQuad::rounded(rect, color, radius));
    }
}

pub(crate) fn append_soft_card_quads(
    quads: &mut Vec<CandidateQuad>,
    rect: [f32; 4],
    fill: [f32; 4],
    outline: [f32; 4],
    shadow: [f32; 4],
    _cutout: [f32; 4],
    radius: f32,
) {
    let [x, y, w, h] = rect;
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let radius = radius.max(0.0).min(w.min(h) * 0.5);
    // Low-contrast elevation, without the old square bevel/highlight strips.
    for (offset, alpha) in [(2.0, 0.12), (1.0, 0.16)] {
        append_rounded_rect_quads(
            quads,
            [x, y + offset, w, h],
            [shadow[0], shadow[1], shadow[2], shadow[3] * alpha],
            radius,
        );
    }
    append_rounded_rect_quads(quads, rect, fill, radius);
    quads.push(CandidateQuad::outline(rect, outline, radius, 0.8));
}

pub(crate) fn icon_point(rect: [f32; 4], point: [f32; 2]) -> [f32; 2] {
    let unit = rect[2].min(rect[3]);
    [
        rect[0] + (rect[2] - unit) * 0.5 + point[0] * unit,
        rect[1] + (rect[3] - unit) * 0.5 + point[1] * unit,
    ]
}

pub(crate) fn icon_line(
    quads: &mut Vec<CandidateQuad>,
    rect: [f32; 4],
    start: [f32; 2],
    end: [f32; 2],
    color: [f32; 4],
    width: f32,
) {
    quads.push(CandidateQuad::line(
        icon_point(rect, start),
        icon_point(rect, end),
        color,
        rect[2].min(rect[3]) * width,
    ));
}

pub(crate) fn icon_path(
    quads: &mut Vec<CandidateQuad>,
    rect: [f32; 4],
    points: &[[f32; 2]],
    color: [f32; 4],
    width: f32,
) {
    for points in points.windows(2) {
        icon_line(quads, rect, points[0], points[1], color, width);
    }
}

pub(crate) fn icon_curve(
    quads: &mut Vec<CandidateQuad>,
    rect: [f32; 4],
    controls: [[f32; 2]; 4],
    color: [f32; 4],
    start_width: f32,
    end_width: f32,
) {
    // Bounded geometry: smooth cubic centerlines with round caps, never a pixel grid.
    let steps = (rect[2].min(rect[3]) * 0.65).ceil().clamp(12.0, 48.0) as usize;
    let mut previous = controls[0];
    for index in 1..=steps {
        let t = index as f32 / steps as f32;
        let u = 1.0 - t;
        let next = std::array::from_fn(|axis| {
            u.powi(3) * controls[0][axis]
                + 3.0 * u * u * t * controls[1][axis]
                + 3.0 * u * t * t * controls[2][axis]
                + t.powi(3) * controls[3][axis]
        });
        let width = start_width + (end_width - start_width) * t;
        icon_line(quads, rect, previous, next, color, width);
        previous = next;
    }
}

pub(crate) fn icon_arc(
    quads: &mut Vec<CandidateQuad>,
    rect: [f32; 4],
    center: [f32; 2],
    radius: f32,
    angles: [f32; 2],
    color: [f32; 4],
    width: f32,
) {
    let mut previous = [
        center[0] + radius * angles[0].cos(),
        center[1] + radius * angles[0].sin(),
    ];
    for index in 1..=24 {
        let angle = angles[0] + (angles[1] - angles[0]) * index as f32 / 24.0;
        let next = [
            center[0] + radius * angle.cos(),
            center[1] + radius * angle.sin(),
        ];
        icon_line(quads, rect, previous, next, color, width);
        previous = next;
    }
}

pub(crate) fn append_gear_icon_quads(
    quads: &mut Vec<CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
    _cutout: [f32; 4],
) {
    let mut points = Vec::with_capacity(49);
    for index in 0..=48 {
        let angle = index as f32 / 48.0 * std::f32::consts::TAU;
        let radius = if matches!(index % 6, 1..=3) {
            0.33
        } else {
            0.27
        };
        points.push([0.5 + radius * angle.cos(), 0.5 + radius * angle.sin()]);
    }
    icon_path(quads, rect, &points, color, 0.065);
    icon_arc(
        quads,
        rect,
        [0.5, 0.5],
        0.115,
        [0.0, std::f32::consts::TAU],
        color,
        0.065,
    );
}

pub(crate) fn append_suzaku_bird_icon_quads(
    quads: &mut Vec<CandidateQuad>,
    rect: [f32; 4],
    primary: [f32; 4],
    secondary: [f32; 4],
    beak: [f32; 4],
    eye: [f32; 4],
) {
    // A rising phoenix in profile: swept flight feathers, arched neck, three flame tails.
    for (curve, from, to) in [
        (
            [[0.47, 0.51], [0.24, 0.45], [0.16, 0.23], [0.18, 0.09]],
            0.12,
            0.015,
        ),
        (
            [[0.44, 0.51], [0.24, 0.50], [0.11, 0.38], [0.08, 0.24]],
            0.085,
            0.012,
        ),
        (
            [[0.46, 0.54], [0.22, 0.60], [0.13, 0.48], [0.10, 0.43]],
            0.070,
            0.012,
        ),
        (
            [[0.46, 0.50], [0.73, 0.57], [0.64, 0.28], [0.68, 0.29]],
            0.105,
            0.065,
        ),
        (
            [[0.45, 0.54], [0.60, 0.77], [0.31, 0.88], [0.14, 0.79]],
            0.067,
            0.008,
        ),
        (
            [[0.50, 0.53], [0.79, 0.68], [0.59, 0.88], [0.41, 0.92]],
            0.063,
            0.008,
        ),
        (
            [[0.65, 0.28], [0.60, 0.23], [0.64, 0.15], [0.60, 0.11]],
            0.045,
            0.006,
        ),
    ] {
        icon_curve(quads, rect, curve, primary, from, to);
    }
    icon_curve(
        quads,
        rect,
        [[0.40, 0.46], [0.32, 0.33], [0.28, 0.20], [0.30, 0.16]],
        secondary,
        0.045,
        0.008,
    );
    icon_curve(
        quads,
        rect,
        [[0.49, 0.57], [0.54, 0.75], [0.43, 0.81], [0.32, 0.83]],
        secondary,
        0.035,
        0.006,
    );
    icon_curve(
        quads,
        rect,
        [[0.69, 0.31], [0.75, 0.30], [0.79, 0.33], [0.83, 0.34]],
        beak,
        0.060,
        0.008,
    );
    let head = icon_point(rect, [0.67, 0.30]);
    let unit = rect[2].min(rect[3]);
    quads.push(CandidateQuad::rounded(
        [
            head[0] - unit * 0.065,
            head[1] - unit * 0.06,
            unit * 0.13,
            unit * 0.12,
        ],
        primary,
        unit,
    ));
    let eye_at = icon_point(rect, [0.693, 0.285]);
    quads.push(CandidateQuad::rounded(
        [eye_at[0], eye_at[1], unit * 0.021, unit * 0.021],
        eye,
        unit,
    ));
}

pub(crate) fn append_keyboard_icon_quads(
    quads: &mut Vec<CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
) {
    let unit = rect[2].min(rect[3]);
    let origin = icon_point(rect, [0.16, 0.25]);
    quads.push(CandidateQuad::outline(
        [origin[0], origin[1], unit * 0.68, unit * 0.50],
        color,
        unit * 0.09,
        unit * 0.065,
    ));
    for row in 0..2 {
        for column in 0..4 {
            let point = [0.29 + column as f32 * 0.14, 0.38 + row as f32 * 0.12];
            icon_line(quads, rect, point, point, color, 0.050);
        }
    }
    icon_line(quads, rect, [0.36, 0.63], [0.64, 0.63], color, 0.052);
}
