//! Shared, resolution-independent guardian emblems for the orb and Linux tray.
use super::{CandidateQuad, ThemePreset, append_suzaku_bird_icon_quads, srgb_color};
use crate::ime::{icon_curve, icon_line, icon_path};

pub fn theme_badge_quads(
    preset: ThemePreset,
    rect: [f32; 4],
    hovered: bool,
    pressed: bool,
) -> Vec<CandidateQuad> {
    let (base, hover, press, ink, detail) = match preset {
        ThemePreset::Baihu => (0xF8F3E7, 0xFFFAEF, 0xE7DFCD, 0x51564E, 0xA48A5D),
        ThemePreset::Qinglong => (0x2B6772, 0x357985, 0x22515F, 0xD5F0EB, 0xCFB8EF),
        ThemePreset::Xuanwu => (0x253258, 0x31426F, 0x1A2443, 0xD5DEF3, 0x95CFD3),
        _ => return suzaku_badge_quads(rect, hovered, pressed),
    };
    let [x, y, w, h] = rect;
    let size = w.min(h);
    let ink = srgb_color(ink);
    let detail = srgb_color(detail);
    let mut quads = Vec::with_capacity(400);
    quads.push(CandidateQuad::rounded(
        rect,
        srgb_color(if pressed {
            press
        } else if hovered {
            hover
        } else {
            base
        }),
        size * 0.5,
    ));
    let rim = size * 0.045;
    quads.push(CandidateQuad::outline(
        [x + rim, y + rim, w - rim * 2.0, h - rim * 2.0],
        [
            detail[0],
            detail[1],
            detail[2],
            if hovered { 0.9 } else { 0.6 },
        ],
        size * 0.5,
        (size * 0.012).max(0.65),
    ));
    let inner = [x + w * 0.10, y + h * 0.10, w * 0.80, h * 0.80];
    match preset {
        ThemePreset::Baihu => append_tiger(&mut quads, inner, ink, detail),
        ThemePreset::Qinglong => append_dragon(&mut quads, inner, ink, detail),
        ThemePreset::Xuanwu => append_tortoise(&mut quads, inner, ink, detail),
        _ => unreachable!(),
    }
    quads
}

fn append_tiger(quads: &mut Vec<CandidateQuad>, rect: [f32; 4], ink: [f32; 4], detail: [f32; 4]) {
    // A symmetric mask: curved cheeks, swept ears and restrained forehead stripes.
    for mirrored in [false, true] {
        let p = |[x, y]: [f32; 2]| [if mirrored { 1.0 - x } else { x }, y];
        for controls in [
            [[0.5, 0.24], [0.39, 0.17], [0.20, 0.22], [0.22, 0.46]],
            [[0.22, 0.46], [0.17, 0.69], [0.36, 0.80], [0.5, 0.82]],
        ] {
            icon_curve(quads, rect, controls.map(p), ink, 0.034, 0.028);
        }
        icon_path(
            quads,
            rect,
            &[p([0.25, 0.31]), p([0.19, 0.17]), p([0.34, 0.22])],
            ink,
            0.034,
        );
        icon_line(quads, rect, p([0.27, 0.43]), p([0.40, 0.48]), ink, 0.033);
        icon_line(quads, rect, p([0.36, 0.48]), p([0.39, 0.48]), detail, 0.047);
        icon_path(
            quads,
            rect,
            &[p([0.26, 0.54]), p([0.35, 0.57]), p([0.27, 0.60])],
            ink,
            0.032,
        );
        icon_line(quads, rect, p([0.28, 0.67]), p([0.39, 0.64]), detail, 0.023);
        icon_curve(
            quads,
            rect,
            [
                p([0.5, 0.64]),
                p([0.49, 0.73]),
                p([0.41, 0.73]),
                p([0.38, 0.69]),
            ],
            ink,
            0.024,
            0.020,
        );
    }
    icon_line(quads, rect, [0.5, 0.29], [0.5, 0.45], detail, 0.035);
    for (y, half) in [(0.30, 0.065), (0.37, 0.045)] {
        icon_line(quads, rect, [0.5 - half, y], [0.5 + half, y], ink, 0.032);
    }
    icon_path(
        quads,
        rect,
        &[[0.44, 0.59], [0.5, 0.64], [0.56, 0.59], [0.44, 0.59]],
        ink,
        0.028,
    );
}

fn append_dragon(quads: &mut Vec<CandidateQuad>, rect: [f32; 4], ink: [f32; 4], detail: [f32; 4]) {
    // An open, coiled body with a violet tail and two antler-like horns.
    for (controls, start, end) in [
        (
            [[0.66, 0.30], [0.40, 0.12], [0.17, 0.29], [0.23, 0.49]],
            0.055,
            0.064,
        ),
        (
            [[0.23, 0.49], [0.29, 0.68], [0.73, 0.50], [0.73, 0.70]],
            0.064,
            0.05,
        ),
        (
            [[0.73, 0.70], [0.76, 0.88], [0.34, 0.89], [0.32, 0.72]],
            0.05,
            0.009,
        ),
    ] {
        icon_curve(quads, rect, controls, ink, start, end);
    }
    icon_curve(
        quads,
        rect,
        [[0.35, 0.47], [0.46, 0.59], [0.77, 0.43], [0.82, 0.63]],
        detail,
        0.024,
        0.005,
    );
    icon_curve(
        quads,
        rect,
        [[0.76, 0.72], [0.78, 0.86], [0.49, 0.93], [0.36, 0.84]],
        detail,
        0.032,
        0.008,
    );
    icon_path(
        quads,
        rect,
        &[[0.59, 0.25], [0.66, 0.12], [0.69, 0.21]],
        detail,
        0.027,
    );
    icon_path(
        quads,
        rect,
        &[[0.48, 0.23], [0.48, 0.11], [0.54, 0.18]],
        detail,
        0.024,
    );
    icon_path(
        quads,
        rect,
        &[
            [0.65, 0.29],
            [0.76, 0.31],
            [0.80, 0.39],
            [0.69, 0.42],
            [0.62, 0.38],
        ],
        ink,
        0.035,
    );
    icon_line(quads, rect, [0.68, 0.33], [0.685, 0.33], detail, 0.034);
    icon_curve(
        quads,
        rect,
        [[0.73, 0.42], [0.80, 0.50], [0.87, 0.46], [0.87, 0.40]],
        detail,
        0.019,
        0.008,
    );
    icon_path(
        quads,
        rect,
        &[[0.34, 0.57], [0.29, 0.65], [0.24, 0.64]],
        detail,
        0.025,
    );
}

fn append_tortoise(
    quads: &mut Vec<CandidateQuad>,
    rect: [f32; 4],
    ink: [f32; 4],
    detail: [f32; 4],
) {
    // Tortoise shell tessellation with a serpent curling around the right edge.
    for controls in [
        [[0.46, 0.29], [0.29, 0.29], [0.22, 0.41], [0.22, 0.54]],
        [[0.22, 0.54], [0.22, 0.70], [0.33, 0.77], [0.46, 0.77]],
        [[0.46, 0.77], [0.60, 0.77], [0.68, 0.69], [0.68, 0.54]],
        [[0.68, 0.54], [0.68, 0.40], [0.61, 0.29], [0.46, 0.29]],
    ] {
        icon_curve(quads, rect, controls, ink, 0.032, 0.032);
    }
    let hex = [
        [0.46, 0.39],
        [0.57, 0.46],
        [0.57, 0.59],
        [0.46, 0.66],
        [0.35, 0.59],
        [0.35, 0.46],
        [0.46, 0.39],
    ];
    icon_path(quads, rect, &hex, ink, 0.022);
    for (start, end) in hex.into_iter().take(6).zip([
        [0.46, 0.30],
        [0.64, 0.40],
        [0.65, 0.67],
        [0.46, 0.76],
        [0.26, 0.67],
        [0.26, 0.40],
    ]) {
        icon_line(quads, rect, start, end, ink, 0.020);
    }
    for path in [
        [[0.40, 0.29], [0.41, 0.19], [0.50, 0.19], [0.52, 0.29]],
        [[0.29, 0.36], [0.19, 0.32], [0.16, 0.42], [0.23, 0.47]],
        [[0.26, 0.64], [0.18, 0.73], [0.25, 0.78], [0.33, 0.74]],
        [[0.61, 0.66], [0.69, 0.75], [0.62, 0.81], [0.55, 0.76]],
    ] {
        icon_path(quads, rect, &path, ink, 0.028);
    }
    for controls in [
        [[0.44, 0.83], [0.75, 0.91], [0.87, 0.67], [0.78, 0.51]],
        [[0.78, 0.51], [0.72, 0.39], [0.72, 0.23], [0.81, 0.23]],
        [[0.81, 0.23], [0.90, 0.25], [0.86, 0.36], [0.79, 0.33]],
    ] {
        icon_curve(quads, rect, controls, detail, 0.028, 0.028);
    }
}

pub fn suzaku_badge_quads(rect: [f32; 4], hovered: bool, pressed: bool) -> Vec<CandidateQuad> {
    let [x, y, w, h] = rect;
    let size = w.min(h);
    let mut quads = Vec::with_capacity(400);
    let red = srgb_color(if pressed {
        0x942B36
    } else if hovered {
        0xC24749
    } else {
        0xB5393F
    });
    let gold = srgb_color(0xF3CD8A);
    quads.push(CandidateQuad::rounded(rect, red, size * 0.5));
    let rim = size * 0.045;
    quads.push(CandidateQuad::outline(
        [x + rim, y + rim, w - rim * 2.0, h - rim * 2.0],
        [gold[0], gold[1], gold[2], if hovered { 0.75 } else { 0.48 }],
        size * 0.5,
        (size * 0.012).max(0.65),
    ));
    append_suzaku_bird_icon_quads(
        &mut quads,
        [x + size * 0.12, y + size * 0.10, size * 0.80, size * 0.80],
        gold,
        srgb_color(0xFFF1D0),
        srgb_color(0xFFE5AB),
        srgb_color(0x70232C),
    );
    quads
}

/// Bounded 4x supersampling, straight ARGB as required by StatusNotifierItem.
/// Runs once per advertised tray size, never per frame or in response to keystrokes.
pub fn suzaku_icon_argb(size: u32) -> Vec<u8> {
    theme_icon_argb(ThemePreset::Suzaku, size)
}

pub fn theme_icon_argb(preset: ThemePreset, size: u32) -> Vec<u8> {
    let size = size.clamp(1, 256) as usize;
    let scale = 4usize;
    let side = size * scale;
    let mut samples = vec![[0.0f32; 4]; side * side];
    let margin = size as f32 * 0.04;
    let quads = theme_badge_quads(
        preset,
        [
            margin,
            margin,
            size as f32 - margin * 2.0,
            size as f32 - margin * 2.0,
        ],
        false,
        false,
    );
    for quad in quads {
        let [x, y, w, h] = quad.rect;
        let left = (x * scale as f32).floor().max(0.0) as usize;
        let top = (y * scale as f32).floor().max(0.0) as usize;
        let right = ((x + w) * scale as f32).ceil().max(0.0) as usize;
        let bottom = ((y + h) * scale as f32).ceil().max(0.0) as usize;
        for sy in top..bottom.min(side) {
            for sx in left..right.min(side) {
                let point = [
                    (sx as f32 + 0.5) / scale as f32,
                    (sy as f32 + 0.5) / scale as f32,
                ];
                if quad.signed_distance(point) > 0.0 {
                    continue;
                }
                let target = &mut samples[sy * side + sx];
                let alpha = quad.color[3];
                for (value, color) in target[..3].iter_mut().zip(quad.color) {
                    *value = color * alpha + *value * (1.0 - alpha);
                }
                target[3] = alpha + target[3] * (1.0 - alpha);
            }
        }
    }
    let srgb = |linear: f32| -> u8 {
        let v = if linear <= 0.0031308 {
            linear * 12.92
        } else {
            1.055 * linear.powf(1.0 / 2.4) - 0.055
        };
        (v.clamp(0.0, 1.0) * 255.0).round() as u8
    };
    let mut argb = vec![0; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let mut mean = [0.0f32; 4];
            for sy in y * scale..(y + 1) * scale {
                for sx in x * scale..(x + 1) * scale {
                    for (value, sample) in mean.iter_mut().zip(samples[sy * side + sx]) {
                        *value += sample / (scale * scale) as f32;
                    }
                }
            }
            let offset = (y * size + x) * 4;
            argb[offset] = (mean[3] * 255.0).round() as u8;
            if mean[3] > 0.0 {
                for channel in 0..3 {
                    argb[offset + 1 + channel] = srgb(mean[channel] / mean[3]);
                }
            }
        }
    }
    argb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guardian_emblems_are_distinct_smooth_and_bounded() {
        let mut reference = Vec::new();
        for preset in ThemePreset::ALL.into_iter().filter(|p| p.is_guardian()) {
            for size in [16, 22, 32, 48, 64] {
                let pixels = theme_icon_argb(preset, size);
                assert_eq!(pixels.len(), (size * size * 4) as usize);
                assert_eq!(&pixels[..4], &[0; 4]);
                assert!(pixels.chunks_exact(4).any(|p| p[0] > 0 && p[0] < 255));
                if size == 32 {
                    assert!(reference.iter().all(|previous| previous != &pixels));
                    reference.push(pixels);
                }
            }
            for (hovered, pressed) in [(false, false), (true, false), (true, true)] {
                let quads = theme_badge_quads(preset, [0.0, 0.0, 92.0, 92.0], hovered, pressed);
                assert!(quads.len() < 500, "{preset:?} geometry budget");
                assert!(quads.iter().all(|q| q.signed_distance([0.0, 0.0]) > 0.0));
            }
        }
    }
    #[test]
    fn emblem_is_smooth_bounded_and_transparent_at_all_tray_sizes() {
        for size in [16, 22, 32, 48, 64] {
            let pixels = suzaku_icon_argb(size);
            assert_eq!(pixels.len(), (size * size * 4) as usize);
            assert_eq!(&pixels[..4], &[0; 4]);
            assert!(pixels.chunks_exact(4).any(|p| p[0] > 0 && p[0] < 255));
            assert!(
                pixels
                    .chunks_exact(4)
                    .any(|p| p[0] == 255 && p[1] > 200 && p[2] > 150)
            );
        }
        assert!(suzaku_badge_quads([0.0, 0.0, 92.0, 92.0], false, false).len() < 500);
    }
}
