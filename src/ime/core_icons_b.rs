use crate::ime::gpu;

pub(crate) fn append_mic_icon_quads(
    quads: &mut Vec<gpu::CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
) {
    let [x, y, w, h] = rect;
    let unit = w.min(h);
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.40, y + unit * 0.22, unit * 0.20, unit * 0.30],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.35, y + unit * 0.54, unit * 0.30, unit * 0.07],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.48, y + unit * 0.61, unit * 0.04, unit * 0.12],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.37, y + unit * 0.72, unit * 0.26, unit * 0.05],
        color,
    });
}

pub(crate) fn append_pen_icon_quads(
    quads: &mut Vec<gpu::CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
) {
    let [x, y, w, h] = rect;
    let unit = w.min(h);
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.30, y + unit * 0.52, unit * 0.30, unit * 0.09],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.54, y + unit * 0.36, unit * 0.10, unit * 0.20],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.64, y + unit * 0.28, unit * 0.08, unit * 0.08],
        color,
    });
}

pub(crate) fn append_backspace_icon_quads(
    quads: &mut Vec<gpu::CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
) {
    let [x, y, w, h] = rect;
    let unit = w.min(h);
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.32, y + unit * 0.34, unit * 0.26, unit * 0.08],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.32, y + unit * 0.58, unit * 0.26, unit * 0.08],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.22, y + unit * 0.44, unit * 0.14, unit * 0.08],
        color,
    });
}

pub(crate) fn append_refresh_icon_quads(
    quads: &mut Vec<gpu::CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
) {
    let [x, y, w, h] = rect;
    let unit = w.min(h);
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.28, y + unit * 0.26, unit * 0.32, unit * 0.08],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.52, y + unit * 0.34, unit * 0.08, unit * 0.24],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.40, y + unit * 0.58, unit * 0.22, unit * 0.08],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.24, y + unit * 0.48, unit * 0.08, unit * 0.20],
        color,
    });
}

pub(crate) fn append_trash_icon_quads(
    quads: &mut Vec<gpu::CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
) {
    let [x, y, w, h] = rect;
    let unit = w.min(h);
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.34, y + unit * 0.28, unit * 0.32, unit * 0.06],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.30, y + unit * 0.36, unit * 0.40, unit * 0.30],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.44, y + unit * 0.20, unit * 0.12, unit * 0.06],
        color,
    });
}

pub(crate) fn append_undo_icon_quads(
    quads: &mut Vec<gpu::CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
) {
    let [x, y, w, h] = rect;
    let unit = w.min(h);
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.22, y + unit * 0.42, unit * 0.30, unit * 0.08],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.22, y + unit * 0.32, unit * 0.08, unit * 0.18],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.44, y + unit * 0.50, unit * 0.18, unit * 0.08],
        color,
    });
}

pub(crate) fn append_seed_icon_quads(
    quads: &mut Vec<gpu::CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
) {
    let [x, y, w, h] = rect;
    let unit = w.min(h);
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.46, y + unit * 0.24, unit * 0.08, unit * 0.44],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.28, y + unit * 0.42, unit * 0.44, unit * 0.08],
        color,
    });
}

pub(crate) fn append_next_icon_quads(
    quads: &mut Vec<gpu::CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
) {
    let [x, y, w, h] = rect;
    let unit = w.min(h);
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.34, y + unit * 0.28, unit * 0.12, unit * 0.12],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.46, y + unit * 0.40, unit * 0.12, unit * 0.12],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.34, y + unit * 0.52, unit * 0.12, unit * 0.12],
        color,
    });
}

pub(crate) fn append_chevron_icon_quads(
    quads: &mut Vec<gpu::CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
    expanded: bool,
) {
    let [x, y, w, h] = rect;
    let unit = w.min(h);
    if expanded {
        quads.push(gpu::CandidateQuad {
            rect: [x + unit * 0.28, y + unit * 0.46, unit * 0.18, unit * 0.08],
            color,
        });
        quads.push(gpu::CandidateQuad {
            rect: [x + unit * 0.46, y + unit * 0.34, unit * 0.18, unit * 0.08],
            color,
        });
        quads.push(gpu::CandidateQuad {
            rect: [x + unit * 0.64, y + unit * 0.46, unit * 0.08, unit * 0.08],
            color,
        });
    } else {
        quads.push(gpu::CandidateQuad {
            rect: [x + unit * 0.28, y + unit * 0.34, unit * 0.18, unit * 0.08],
            color,
        });
        quads.push(gpu::CandidateQuad {
            rect: [x + unit * 0.46, y + unit * 0.46, unit * 0.18, unit * 0.08],
            color,
        });
        quads.push(gpu::CandidateQuad {
            rect: [x + unit * 0.64, y + unit * 0.34, unit * 0.08, unit * 0.08],
            color,
        });
    }
}
