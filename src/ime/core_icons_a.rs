use crate::ime::gpu;

pub(crate) fn append_rounded_rect_quads(
    quads: &mut Vec<gpu::CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
    radius: f32,
) {
    let [x, y, w, h] = rect;
    if w <= 0.0 || h <= 0.0 {
        return;
    }

    let radius = radius.min(w * 0.5).min(h * 0.5).max(0.0);
    if radius <= 1.0 {
        quads.push(gpu::CandidateQuad { rect, color });
        return;
    }

    quads.push(gpu::CandidateQuad {
        rect: [x + radius, y, w - radius * 2.0, h],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x, y + radius, radius, h - radius * 2.0],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + w - radius, y + radius, radius, h - radius * 2.0],
        color,
    });

    let steps = 6;
    for step in 0..steps {
        let y0 = step as f32 / steps as f32 * radius;
        let y1 = (step + 1) as f32 / steps as f32 * radius;
        let mid = (y0 + y1) * 0.5;
        let inset = radius - (radius * radius - (radius - mid).powi(2)).sqrt();
        let strip_h = (y1 - y0).max(1.0);
        let strip_w = (radius - inset).max(1.0);

        quads.push(gpu::CandidateQuad {
            rect: [x + inset, y + y0, strip_w, strip_h],
            color,
        });
        quads.push(gpu::CandidateQuad {
            rect: [x + w - radius, y + y0, strip_w, strip_h],
            color,
        });
        quads.push(gpu::CandidateQuad {
            rect: [x + inset, y + h - y1, strip_w, strip_h],
            color,
        });
        quads.push(gpu::CandidateQuad {
            rect: [x + w - radius, y + h - y1, strip_w, strip_h],
            color,
        });
    }
}

pub(crate) fn append_soft_card_quads(
    quads: &mut Vec<gpu::CandidateQuad>,
    rect: [f32; 4],
    fill: [f32; 4],
    outline: [f32; 4],
    shadow: [f32; 4],
    cutout: [f32; 4],
    radius: f32,
) {
    let [x, y, w, h] = rect;
    let radius = radius.min(w * 0.22).min(h * 0.35).max(3.0);
    let border = 1.5_f32.min(w * 0.04).min(h * 0.10).max(1.0);

    let deep_shadow = [shadow[0], shadow[1], shadow[2], shadow[3] * 0.78];
    let ambient_shadow = [shadow[0], shadow[1], shadow[2], shadow[3] * 0.38];
    append_rounded_rect_quads(
        quads,
        [x + 1.0, y + 2.0, w, h],
        ambient_shadow,
        radius + 2.0,
    );
    append_rounded_rect_quads(quads, [x + 3.0, y + 6.0, w, h], deep_shadow, radius + 1.5);
    append_rounded_rect_quads(quads, rect, outline, radius);
    append_rounded_rect_quads(
        quads,
        [x + border, y + border, w - border * 2.0, h - border * 2.0],
        fill,
        (radius - border).max(1.0),
    );
    quads.push(gpu::CandidateQuad {
        rect: [
            x + border * 1.5,
            y + h - (h * 0.16).max(5.0) - border,
            w - border * 3.0,
            (h * 0.16).max(5.0),
        ],
        color: [0.12, 0.18, 0.28, 0.035],
    });
    quads.push(gpu::CandidateQuad {
        rect: [
            x + border * 2.0,
            y + border * 2.0,
            w - border * 4.0,
            (h * 0.14).max(4.0),
        ],
        color: [1.0, 1.0, 1.0, 0.11],
    });
    quads.push(gpu::CandidateQuad {
        rect: [
            x + border * 2.2,
            y + border * 1.3,
            w - border * 4.4,
            border.max(1.0),
        ],
        color: [1.0, 1.0, 1.0, 0.15],
    });
    let cut = radius * 0.18;
    append_rounded_rect_quads(quads, [x + border, y + border, cut, cut], fill, cut * 0.6);
    append_rounded_rect_quads(
        quads,
        [x + w - border - cut, y + border, cut, cut],
        fill,
        cut * 0.6,
    );
    append_rounded_rect_quads(
        quads,
        [x + border, y + h - border - cut, cut, cut],
        fill,
        cut * 0.6,
    );
    append_rounded_rect_quads(
        quads,
        [x + w - border - cut, y + h - border - cut, cut, cut],
        fill,
        cut * 0.6,
    );

    let _ = cutout;
}

pub(crate) fn append_gear_icon_quads(
    quads: &mut Vec<gpu::CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
    cutout: [f32; 4],
) {
    let [x, y, w, h] = rect;
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;

    let teeth = [
        [cx - 1.5, y + 2.0, 3.0, 4.0],
        [cx - 1.5, y + h - 6.0, 3.0, 4.0],
        [x + 2.0, cy - 1.5, 4.0, 3.0],
        [x + w - 6.0, cy - 1.5, 4.0, 3.0],
        [x + 4.0, y + 4.0, 3.0, 3.0],
        [x + w - 7.0, y + 4.0, 3.0, 3.0],
        [x + 4.0, y + h - 7.0, 3.0, 3.0],
        [x + w - 7.0, y + h - 7.0, 3.0, 3.0],
    ];

    for tooth in teeth {
        quads.push(gpu::CandidateQuad { rect: tooth, color });
    }

    quads.push(gpu::CandidateQuad {
        rect: [cx - 4.0, cy - 4.0, 8.0, 8.0],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [cx - 1.5, cy - 1.5, 3.0, 3.0],
        color: cutout,
    });
}

pub(crate) fn append_suzaku_bird_icon_quads(
    quads: &mut Vec<gpu::CandidateQuad>,
    rect: [f32; 4],
    primary: [f32; 4],
    secondary: [f32; 4],
    beak: [f32; 4],
    eye: [f32; 4],
) {
    let [x, y, w, h] = rect;
    let unit = w.min(h);
    let body = [x + unit * 0.28, y + unit * 0.36, unit * 0.28, unit * 0.22];
    let neck = [x + unit * 0.50, y + unit * 0.24, unit * 0.10, unit * 0.16];
    let head = [x + unit * 0.57, y + unit * 0.20, unit * 0.14, unit * 0.14];
    let crest = [x + unit * 0.49, y + unit * 0.18, unit * 0.10, unit * 0.08];
    let wing_upper = [x + unit * 0.22, y + unit * 0.28, unit * 0.24, unit * 0.12];
    let wing_main = [x + unit * 0.18, y + unit * 0.36, unit * 0.34, unit * 0.14];
    let tail_base = [x + unit * 0.44, y + unit * 0.56, unit * 0.22, unit * 0.09];
    let tail_flare = [x + unit * 0.54, y + unit * 0.64, unit * 0.22, unit * 0.08];
    let tail_tip = [x + unit * 0.64, y + unit * 0.72, unit * 0.14, unit * 0.06];
    let beak_rect = [x + unit * 0.70, y + unit * 0.28, unit * 0.12, unit * 0.06];
    let eye_rect = [x + unit * 0.63, y + unit * 0.28, unit * 0.03, unit * 0.03];

    for bird_rect in [
        body, neck, head, crest, wing_upper, wing_main, tail_base, tail_flare, tail_tip,
    ] {
        quads.push(gpu::CandidateQuad {
            rect: bird_rect,
            color: primary,
        });
    }
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.28, y + unit * 0.52, unit * 0.20, unit * 0.08],
        color: secondary,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.36, y + unit * 0.60, unit * 0.22, unit * 0.06],
        color: secondary,
    });
    quads.push(gpu::CandidateQuad {
        rect: beak_rect,
        color: beak,
    });
    quads.push(gpu::CandidateQuad {
        rect: eye_rect,
        color: eye,
    });
}

pub(crate) fn append_keyboard_icon_quads(
    quads: &mut Vec<gpu::CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
) {
    let [x, y, w, h] = rect;
    let pad = w.min(h) * 0.22;
    quads.push(gpu::CandidateQuad {
        rect: [x + pad, y + pad * 1.1, w - pad * 2.0, h - pad * 2.0],
        color,
    });
    let key_w = (w - pad * 3.2) / 3.0;
    let key_h = (h - pad * 4.8) / 3.0;
    for row in 0..2 {
        for col in 0..3 {
            quads.push(gpu::CandidateQuad {
                rect: [
                    x + pad * 1.6 + col as f32 * (key_w + pad * 0.4),
                    y + pad * 1.6 + row as f32 * (key_h + pad * 0.5),
                    key_w,
                    key_h,
                ],
                color: [0.95, 0.98, 1.0, 0.95],
            });
        }
    }
    quads.push(gpu::CandidateQuad {
        rect: [x + pad * 1.6, y + h - pad * 2.0, w - pad * 3.2, key_h * 0.9],
        color: [0.95, 0.98, 1.0, 0.95],
    });
}
