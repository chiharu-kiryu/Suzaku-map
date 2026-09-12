use std::ops::Range;

pub(crate) use crate::font_atlas::{FontAtlas, create_font_atlas};
use bytemuck::{Pod, Zeroable};
use suzaku_map::ime::gpu::{AtlasGlyph, CandidateQuad, RenderScene};

pub(super) fn surface_alpha_mode(
    supported: &[wgpu::CompositeAlphaMode],
) -> wgpu::CompositeAlphaMode {
    use wgpu::CompositeAlphaMode::{Inherit, Opaque, PreMultiplied};
    // Linux/XWayland Vulkan surfaces commonly expose only Opaque + Inherit.
    // Winit's transparent native window supplies the ARGB visual / Wayland alpha
    // semantics in that case; selecting Opaque would discard all rendered alpha.
    [PreMultiplied, Inherit, Opaque]
        .into_iter()
        .filter(|mode| *mode != Inherit || cfg!(target_os = "linux"))
        .find(|mode| supported.contains(mode))
        .or_else(|| supported.first().copied())
        .unwrap_or(Opaque)
}

pub(super) fn surface_clear_color(
    alpha_mode: wgpu::CompositeAlphaMode,
    floating: bool,
    opaque_background: wgpu::Color,
) -> wgpu::Color {
    if floating && surface_supports_alpha(alpha_mode) {
        wgpu::Color::TRANSPARENT
    } else {
        // Opaque-only drivers keep a themed backdrop, never a transparent-black clear.
        opaque_background
    }
}

pub(super) fn surface_supports_alpha(alpha_mode: wgpu::CompositeAlphaMode) -> bool {
    alpha_mode == wgpu::CompositeAlphaMode::PreMultiplied
        || (cfg!(target_os = "linux") && alpha_mode == wgpu::CompositeAlphaMode::Inherit)
}

pub(super) fn prefer_transparent_adapter(
    current_alpha: wgpu::CompositeAlphaMode,
    candidate_type: wgpu::DeviceType,
    candidate_alpha: &[wgpu::CompositeAlphaMode],
) -> bool {
    !surface_supports_alpha(current_alpha)
        && matches!(
            candidate_type,
            wgpu::DeviceType::DiscreteGpu
                | wgpu::DeviceType::IntegratedGpu
                | wgpu::DeviceType::VirtualGpu
        )
        && surface_supports_alpha(surface_alpha_mode(candidate_alpha))
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct PanelVertex {
    pub(crate) position: [f32; 2],
    pub(crate) color: [f32; 4],
    pub(crate) point: [f32; 2],
    pub(crate) geometry: [f32; 4],
    pub(crate) style: [f32; 4],
}

impl PanelVertex {
    pub(crate) fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<PanelVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 2]>() as u64,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: 24,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: 48,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct TextVertex {
    pub(crate) position: [f32; 2],
    pub(crate) uv: [f32; 2],
    pub(crate) color: [f32; 4],
}

impl TextVertex {
    pub(crate) fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<TextVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 2]>() as u64,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as u64,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

pub(super) struct PanelOverlay {
    pub(super) quads: Vec<CandidateQuad>,
    pub(super) atlas_glyphs: Vec<AtlasGlyph>,
}

pub(super) struct LayerDrawRanges {
    pub(super) shapes: Range<u32>,
    pub(super) text: Range<u32>,
}

pub(super) struct FrameVertices {
    pub(super) shapes: Vec<PanelVertex>,
    pub(super) text: Vec<TextVertex>,
    pub(super) layers: Vec<LayerDrawRanges>,
}

pub(super) fn build_frame_vertices(
    scene: &RenderScene,
    overlays: &[PanelOverlay],
    width: f32,
    height: f32,
    quad_for: impl Fn(&AtlasGlyph) -> ([f32; 4], [f32; 4]),
) -> FrameVertices {
    let shape_count = scene.quads.len()
        + overlays
            .iter()
            .map(|overlay| overlay.quads.len())
            .sum::<usize>();
    let glyph_count = scene.atlas_glyphs.len()
        + overlays
            .iter()
            .map(|overlay| overlay.atlas_glyphs.len())
            .sum::<usize>();
    let mut frame = FrameVertices {
        shapes: Vec::with_capacity(shape_count * 6),
        text: Vec::with_capacity(glyph_count * 6),
        layers: Vec::with_capacity(overlays.len() + 1),
    };
    let sources = std::iter::once((scene.quads.as_slice(), scene.atlas_glyphs.as_slice())).chain(
        overlays
            .iter()
            .map(|overlay| (overlay.quads.as_slice(), overlay.atlas_glyphs.as_slice())),
    );
    for (quads, glyphs) in sources {
        let shape_start = frame.shapes.len() as u32;
        let text_start = frame.text.len() as u32;
        for quad in quads {
            push_quad_vertices(&mut frame.shapes, quad, width, height);
        }
        for glyph in glyphs {
            let (rect, uv) = quad_for(glyph);
            let ink = AtlasGlyph {
                rect,
                ..glyph.clone()
            };
            push_text_quad_vertices(&mut frame.text, &ink, uv, width, height);
        }
        frame.layers.push(LayerDrawRanges {
            shapes: shape_start..frame.shapes.len() as u32,
            text: text_start..frame.text.len() as u32,
        });
    }
    frame
}

fn push_quad_vertices(
    vertices: &mut Vec<PanelVertex>,
    quad: &CandidateQuad,
    width: f32,
    height: f32,
) {
    let [x, y, w, h] = quad.rect;
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    // Leave a pixel around curves for analytic coverage; hit geometry stays unchanged.
    let fringe = if quad.shape == suzaku_map::ime::gpu::QuadShape::Rectangle {
        0.0
    } else {
        1.0
    };
    let mut bounds = [x - fringe, y - fringe, x + w + fringe, y + h + fringe];
    if let Some([cx, cy, cw, ch]) = quad.clip_rect {
        bounds = [
            bounds[0].max(cx),
            bounds[1].max(cy),
            bounds[2].min(cx + cw),
            bounds[3].min(cy + ch),
        ];
    }
    let [left, top, right, bottom] = bounds;
    if right <= left || bottom <= top {
        return;
    }
    let (geometry, style) = quad.shape_parameters();
    for point in [
        [left, top],
        [right, top],
        [right, bottom],
        [left, top],
        [right, bottom],
        [left, bottom],
    ] {
        vertices.push(PanelVertex {
            position: [px_to_ndc_x(point[0], width), px_to_ndc_y(point[1], height)],
            color: quad.color,
            point,
            geometry,
            style,
        });
    }
}

fn push_text_quad_vertices(
    vertices: &mut Vec<TextVertex>,
    glyph: &AtlasGlyph,
    uv: [f32; 4],
    width: f32,
    height: f32,
) {
    let [x, y, w, h] = glyph.rect;
    if !glyph.rect.iter().all(|value| value.is_finite()) || w <= 0.0 || h <= 0.0 {
        return;
    }
    // The atlas supplies pixel-aligned native ink, without changing layout advances.
    // Preserve that geometry through clipping; never round each cropped edge separately.
    let Some(([x, y, w, h], uv)) = clipped_text_quad([x, y, w, h], uv, glyph.clip_rect) else {
        return;
    };
    let color = glyph.color;
    let x1 = px_to_ndc_x(x, width);
    let x2 = px_to_ndc_x(x + w, width);
    let y1 = px_to_ndc_y(y, height);
    let y2 = px_to_ndc_y(y + h, height);
    let [u1, v1, u2, v2] = uv;

    vertices.extend_from_slice(&[
        TextVertex {
            position: [x1, y1],
            uv: [u1, v1],
            color,
        },
        TextVertex {
            position: [x2, y1],
            uv: [u2, v1],
            color,
        },
        TextVertex {
            position: [x2, y2],
            uv: [u2, v2],
            color,
        },
        TextVertex {
            position: [x1, y1],
            uv: [u1, v1],
            color,
        },
        TextVertex {
            position: [x2, y2],
            uv: [u2, v2],
            color,
        },
        TextVertex {
            position: [x1, y2],
            uv: [u1, v2],
            color,
        },
    ]);
}

fn clipped_text_quad(
    rect: [f32; 4],
    uv: [f32; 4],
    clip: Option<[f32; 4]>,
) -> Option<([f32; 4], [f32; 4])> {
    let Some(clip) = clip else {
        return Some((rect, uv));
    };
    let left = rect[0].max(clip[0]);
    let top = rect[1].max(clip[1]);
    let right = (rect[0] + rect[2]).min(clip[0] + clip[2]);
    let bottom = (rect[1] + rect[3]).min(clip[1] + clip[3]);
    if right <= left || bottom <= top {
        return None;
    }
    let du = (uv[2] - uv[0]) / rect[2];
    let dv = (uv[3] - uv[1]) / rect[3];
    Some((
        [left, top, right - left, bottom - top],
        [
            uv[0] + (left - rect[0]) * du,
            uv[1] + (top - rect[1]) * dv,
            uv[2] - (rect[0] + rect[2] - right) * du,
            uv[3] - (rect[1] + rect[3] - bottom) * dv,
        ],
    ))
}

fn px_to_ndc_x(x: f32, width: f32) -> f32 {
    (x / width) * 2.0 - 1.0
}
fn px_to_ndc_y(y: f32, height: f32) -> f32 {
    1.0 - (y / height) * 2.0
}

#[cfg(test)]
mod tests {
    use super::{PanelOverlay, build_frame_vertices, clipped_text_quad};
    use suzaku_map::ime::gpu::{
        CandidateQuad, PanelChromeState, TextAlign, TextBlock, TextRole, WgpuCandidateRenderer,
    };

    #[test]
    fn transparent_surfaces_prefer_explicit_alpha_then_linux_native_inheritance() {
        use wgpu::CompositeAlphaMode::{Inherit, Opaque, PostMultiplied, PreMultiplied};
        assert_eq!(
            super::surface_alpha_mode(&[Opaque, Inherit, PreMultiplied]),
            PreMultiplied
        );
        assert_eq!(
            super::surface_alpha_mode(&[Opaque, Inherit]),
            if cfg!(target_os = "linux") {
                Inherit
            } else {
                Opaque
            }
        );
        assert_eq!(super::surface_alpha_mode(&[Opaque, PostMultiplied]), Opaque);
        assert_eq!(super::surface_alpha_mode(&[Opaque]), Opaque);
        assert_eq!(super::surface_alpha_mode(&[]), Opaque);
    }

    #[test]
    fn floating_windows_clear_to_transparent_only_with_compatible_compositing() {
        use wgpu::CompositeAlphaMode::{Auto, Inherit, Opaque, PostMultiplied, PreMultiplied};
        let backdrop = wgpu::Color {
            r: 0.5,
            g: 0.4,
            b: 0.3,
            a: 1.0,
        };
        for mode in [Auto, Inherit, Opaque, PostMultiplied, PreMultiplied] {
            assert_eq!(super::surface_clear_color(mode, false, backdrop), backdrop);
            let compatible =
                mode == PreMultiplied || (cfg!(target_os = "linux") && mode == Inherit);
            assert_eq!(
                super::surface_clear_color(mode, true, backdrop),
                if compatible {
                    wgpu::Color::TRANSPARENT
                } else {
                    backdrop
                }
            );
        }
    }

    #[test]
    fn alpha_capable_hardware_can_replace_opaque_rendering_without_forcing_software() {
        use wgpu::{
            CompositeAlphaMode::{Opaque, PreMultiplied},
            DeviceType::{Cpu, DiscreteGpu, IntegratedGpu},
        };
        assert!(super::prefer_transparent_adapter(
            Opaque,
            IntegratedGpu,
            &[PreMultiplied]
        ));
        assert!(super::prefer_transparent_adapter(
            Opaque,
            DiscreteGpu,
            &[PreMultiplied]
        ));
        assert!(!super::prefer_transparent_adapter(
            Opaque,
            Cpu,
            &[PreMultiplied]
        ));
        assert!(!super::prefer_transparent_adapter(
            Opaque,
            IntegratedGpu,
            &[Opaque]
        ));
        assert!(!super::prefer_transparent_adapter(
            PreMultiplied,
            DiscreteGpu,
            &[PreMultiplied]
        ));
    }

    #[test]
    fn fractional_glyph_geometry_is_not_rounded_or_stretched() {
        let glyph = super::AtlasGlyph {
            ch: 'i',
            rect: [10.25, 20.125, 4.375, 17.5],
            color: [1.0; 4],
            clip_rect: None,
        };
        let mut vertices = Vec::new();
        super::push_text_quad_vertices(&mut vertices, &glyph, [0.0, 0.0, 1.0, 1.0], 100.0, 100.0);
        assert_eq!(vertices.len(), 6);
        for (index, point) in [(0, [10.25, 20.125]), (2, [14.625, 37.625])] {
            let actual = [
                (vertices[index].position[0] + 1.0) * 50.0,
                (1.0 - vertices[index].position[1]) * 50.0,
            ];
            assert!((actual[0] - point[0]).abs() < 0.0001);
            assert!((actual[1] - point[1]).abs() < 0.0001);
        }
    }

    #[test]
    fn empty_or_invalid_glyphs_do_not_turn_into_one_pixel_artifacts() {
        for rect in [
            [1.0, 2.0, 0.0, 7.0],
            [1.0, 2.0, 4.0, 0.0],
            [1.0, 2.0, -1.0, 7.0],
            [f32::NAN, 2.0, 4.0, 7.0],
        ] {
            let mut vertices = Vec::new();
            super::push_text_quad_vertices(
                &mut vertices,
                &super::AtlasGlyph {
                    ch: 'i',
                    rect,
                    color: [1.0; 4],
                    clip_rect: None,
                },
                [0.0, 0.0, 1.0, 1.0],
                100.0,
                100.0,
            );
            assert!(
                vertices.is_empty(),
                "invalid glyph {rect:?} should not render"
            );
        }
    }

    #[test]
    fn tooltip_layer_gets_its_own_surface_and_text_draws_after_the_base_scene() {
        let scene = WgpuCandidateRenderer::new(520.0, 340.0)
            .build_settings_scene(&PanelChromeState::default(), None);
        let text = TextBlock {
            text: "Hint".into(),
            origin: [20.0, 30.0],
            max_width: 100.0,
            pixel_size: 2.0,
            letter_spacing: 0.0,
            line_gap: 0.0,
            max_lines: 1,
            color: [1.0; 4],
            align: TextAlign::Left,
            role: TextRole::HeaderStatus,
        }
        .layout();
        let overlay = PanelOverlay {
            quads: vec![CandidateQuad {
                shape: Default::default(),
                clip_rect: None,
                rect: [12.0, 22.0, 120.0, 30.0],
                color: [0.0, 0.0, 0.0, 1.0],
            }],
            atlas_glyphs: text.atlas_glyphs,
        };
        let frame = build_frame_vertices(&scene, &[overlay], 520.0, 340.0, |glyph| {
            (glyph.rect, [0.0, 0.0, 1.0, 1.0])
        });
        assert_eq!(frame.layers.len(), 2);
        assert_eq!(frame.layers[0].shapes, 0..scene.quads.len() as u32 * 6);
        assert_eq!(frame.layers[1].shapes.start, frame.layers[0].shapes.end);
        assert_eq!(frame.layers[1].shapes.len(), 6);
        assert_eq!(frame.layers[1].text.start, frame.layers[0].text.end);
        assert_eq!(frame.layers[1].text.len(), 4 * 6);
        assert_eq!(frame.layers[1].shapes.end as usize, frame.shapes.len());
        assert_eq!(frame.layers[1].text.end as usize, frame.text.len());
    }

    #[test]
    fn shape_clipping_preserves_curve_geometry_and_limits_the_antialias_fringe() {
        let mut quad = CandidateQuad::rounded([10.0, 10.0, 20.0, 20.0], [1.0; 4], 10.0);
        quad.clip_rect = Some([20.0, 0.0, 20.0, 40.0]);
        let mut vertices = Vec::new();
        super::push_quad_vertices(&mut vertices, &quad, 100.0, 100.0);
        assert_eq!(vertices.len(), 6);
        for vertex in &vertices {
            assert!(vertex.point[0] >= 20.0 && vertex.point[0] <= 31.0);
            assert_eq!(vertex.geometry, [20.0, 20.0, 10.0, 10.0]);
            assert_eq!(vertex.style, [10.0, 0.0, 1.0, 0.0]);
        }
        quad.clip_rect = Some([60.0, 60.0, 1.0, 1.0]);
        vertices.clear();
        super::push_quad_vertices(&mut vertices, &quad, 100.0, 100.0);
        assert!(vertices.is_empty());
    }

    #[test]
    fn scroll_clipping_crops_uvs_without_stretching_glyphs() {
        let rect = [10.0, 20.0, 20.0, 40.0];
        let uv = [0.0, 0.0, 1.0, 1.0];
        let (clipped, uv) = clipped_text_quad(rect, uv, Some([15.0, 30.0, 10.0, 20.0])).unwrap();
        assert_eq!(clipped, [15.0, 30.0, 10.0, 20.0]);
        assert_eq!(uv, [0.25, 0.25, 0.75, 0.75]);
        assert!(clipped_text_quad(rect, uv, Some([0.0, 70.0, 50.0, 50.0])).is_none());
    }
}
