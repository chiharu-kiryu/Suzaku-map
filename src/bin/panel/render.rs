use std::ops::Range;

pub(crate) use crate::font_atlas::{FontAtlas, create_font_atlas};
use bytemuck::{Pod, Zeroable};
use suzaku_map::ime::gpu::{AtlasGlyph, CandidateQuad, RenderScene};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct PanelVertex {
    pub(crate) position: [f32; 2],
    pub(crate) color: [f32; 4],
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
    uv_for: impl Fn(char) -> [f32; 4],
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
            push_text_quad_vertices(&mut frame.text, glyph, uv_for(glyph.ch), width, height);
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
    let color = quad.color;
    let x1 = px_to_ndc_x(x, width);
    let x2 = px_to_ndc_x(x + w, width);
    let y1 = px_to_ndc_y(y, height);
    let y2 = px_to_ndc_y(y + h, height);

    vertices.extend_from_slice(&[
        PanelVertex {
            position: [x1, y1],
            color,
        },
        PanelVertex {
            position: [x2, y1],
            color,
        },
        PanelVertex {
            position: [x2, y2],
            color,
        },
        PanelVertex {
            position: [x1, y1],
            color,
        },
        PanelVertex {
            position: [x2, y2],
            color,
        },
        PanelVertex {
            position: [x1, y2],
            color,
        },
    ]);
}

fn push_text_quad_vertices(
    vertices: &mut Vec<TextVertex>,
    glyph: &AtlasGlyph,
    uv: [f32; 4],
    width: f32,
    height: f32,
) {
    let [x, y, w, h] = glyph.rect;
    let x = x.round();
    let y = y.round();
    let w = w.round().max(1.0);
    let h = h.round().max(1.0);
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
                rect: [12.0, 22.0, 120.0, 30.0],
                color: [0.0, 0.0, 0.0, 1.0],
            }],
            atlas_glyphs: text.atlas_glyphs,
        };
        let frame =
            build_frame_vertices(&scene, &[overlay], 520.0, 340.0, |_| [0.0, 0.0, 1.0, 1.0]);
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
    fn scroll_clipping_crops_uvs_without_stretching_glyphs() {
        let rect = [10.0, 20.0, 20.0, 40.0];
        let uv = [0.0, 0.0, 1.0, 1.0];
        let (clipped, uv) = clipped_text_quad(rect, uv, Some([15.0, 30.0, 10.0, 20.0])).unwrap();
        assert_eq!(clipped, [15.0, 30.0, 10.0, 20.0]);
        assert_eq!(uv, [0.25, 0.25, 0.75, 0.75]);
        assert!(clipped_text_quad(rect, uv, Some([0.0, 70.0, 50.0, 50.0])).is_none());
    }
}
