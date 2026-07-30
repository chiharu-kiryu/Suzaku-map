use std::collections::HashMap;
use std::fs;

use bytemuck::{Pod, Zeroable};
use fontdue::Font;
use suzaku_map::ime::gpu::{AtlasGlyph, CandidateQuad, FontFaceChoice, RenderScene, TextSmoothing};
use suzaku_map::platform::gpu_host::preferred_font_paths;

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

pub(crate) struct FontAtlas {
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) uv_map: HashMap<char, [f32; 4]>,
    pub(crate) uses_runtime_font: bool,
    pub(crate) font_label: String,
}

impl FontAtlas {
    fn uv_for(&self, ch: char) -> [f32; 4] {
        let key = atlas_lookup_char(ch);
        self.uv_map
            .get(&key)
            .copied()
            .or_else(|| self.uv_map.get(&'?').copied())
            .expect("font atlas must contain fallback glyph")
    }
}

pub(crate) fn build_shape_vertices(
    scene: &RenderScene,
    width: f32,
    height: f32,
) -> Vec<PanelVertex> {
    let mut vertices = Vec::with_capacity(scene.quads.len() * 6);

    for quad in &scene.quads {
        push_quad_vertices(&mut vertices, quad, width, height);
    }

    vertices
}

pub(crate) fn build_text_vertices(
    glyphs: &[AtlasGlyph],
    atlas: &FontAtlas,
    width: f32,
    height: f32,
) -> Vec<TextVertex> {
    let mut vertices = Vec::with_capacity(glyphs.len() * 6);
    for glyph in glyphs {
        let uv = atlas.uv_for(glyph.ch);
        push_text_quad_vertices(&mut vertices, glyph, uv, width, height);
    }
    vertices
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

fn px_to_ndc_x(x: f32, width: f32) -> f32 {
    (x / width) * 2.0 - 1.0
}
fn px_to_ndc_y(y: f32, height: f32) -> f32 {
    1.0 - (y / height) * 2.0
}

pub(crate) fn create_font_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bind_group_layout: &wgpu::BindGroupLayout,
    font_face: FontFaceChoice,
    smoothing: TextSmoothing,
    font_scale: f32,
) -> FontAtlas {
    let atlas_scale = font_scale.max(1.0).min(2.5);
    if let Some((font, label)) = load_runtime_font(font_face) {
        return create_runtime_font_atlas(
            device,
            queue,
            bind_group_layout,
            &font,
            atlas_scale,
            smoothing,
            label,
        );
    }
    create_bitmap_font_atlas(
        device,
        queue,
        bind_group_layout,
        atlas_scale,
        smoothing,
    )
}

fn create_runtime_font_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bind_group_layout: &wgpu::BindGroupLayout,
    font: &Font,
    atlas_scale: f32,
    smoothing: TextSmoothing,
    font_label: String,
) -> FontAtlas {
    const GLYPH_SIZE: f32 = 28.0;
    let glyph_size = (GLYPH_SIZE * atlas_scale).round().clamp(24.0, 64.0);
    let glyphs = atlas_charset();
    let mut rendered = Vec::with_capacity(glyphs.len());
    let mut max_w = 0u32;
    let mut max_h = 0u32;

    for ch in &glyphs {
        let (metrics, bitmap) = font.rasterize(*ch, glyph_size);
        max_w = max_w.max(metrics.width as u32);
        max_h = max_h.max(metrics.height as u32);
        rendered.push((*ch, metrics, bitmap));
    }

    let cell_w = max_w.max(12) + 4;
    let cell_h = max_h.max(16) + 4;
    let cols = 16u32;
    let rows = (glyphs.len() as u32).div_ceil(cols);
    let atlas_w = cols * cell_w;
    let atlas_h = rows * cell_h;
    let mut bytes = vec![0u8; (atlas_w * atlas_h) as usize];
    let mut uv_map = HashMap::new();

    for (index, (ch, metrics, bitmap)) in rendered.iter().enumerate() {
        let col = index as u32 % cols;
        let row = index as u32 / cols;
        let origin_x = col * cell_w + ((cell_w - metrics.width as u32) / 2);
        let origin_y = row * cell_h + ((cell_h - metrics.height as u32) / 2);
        for y in 0..metrics.height as u32 {
            for x in 0..metrics.width as u32 {
                let src = bitmap[(y * metrics.width as u32 + x) as usize];
                let dst_x = origin_x + x;
                let dst_y = origin_y + y;
                bytes[(dst_y * atlas_w + dst_x) as usize] = src;
            }
        }
        uv_map.insert(
            *ch,
            [
                (col * cell_w) as f32 / atlas_w as f32,
                (row * cell_h) as f32 / atlas_h as f32,
                ((col + 1) * cell_w) as f32 / atlas_w as f32,
                ((row + 1) * cell_h) as f32 / atlas_h as f32,
            ],
        );
    }

    create_font_atlas_resources(
        device,
        queue,
        bind_group_layout,
        atlas_w,
        atlas_h,
        bytes,
        uv_map,
        true,
        smoothing,
        font_label,
    )
}

fn create_bitmap_font_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bind_group_layout: &wgpu::BindGroupLayout,
    atlas_scale: f32,
    smoothing: TextSmoothing,
) -> FontAtlas {
    let scale = atlas_scale.clamp(1.0, 2.0);
    let cell_w = ((8.0 * scale).round() as u32).max(8);
    let cell_h = ((10.0 * scale).round() as u32).max(10);
    const COLS: u32 = 16;
    let glyphs = atlas_charset();
    let rows = (glyphs.len() as u32).div_ceil(COLS);
    let atlas_w = COLS * cell_w;
    let atlas_h = rows * cell_h;
    let mut bytes = vec![0u8; (atlas_w * atlas_h) as usize];
    let mut uv_map = HashMap::new();

    for (index, ch) in glyphs.iter().enumerate() {
        let col = index as u32 % COLS;
        let row = index as u32 / COLS;
        let mut origin_x = col * cell_w + 1;
        let mut origin_y = row * cell_h + 1;
        if scale > 1.25 {
            let x_pad = (cell_w as i32 - 8) / 2;
            let y_pad = (cell_h as i32 - 10) / 2;
            if x_pad > 0 && y_pad > 0 {
                origin_x = col * cell_w + x_pad as u32;
                origin_y = row * cell_h + y_pad as u32;
            }
        }
        for (bitmap_row, pattern) in suzaku_map::ime::gpu::glyph_bitmap(*ch).iter().enumerate() {
            for bitmap_col in 0..5 {
                if (pattern >> (4 - bitmap_col)) & 1 == 1 {
                    let x = origin_x + bitmap_col;
                    let y = origin_y + bitmap_row as u32;
                    bytes[(y * atlas_w + x) as usize] = 255;
                }
            }
        }
        uv_map.insert(
            *ch,
            [
                (col * cell_w) as f32 / atlas_w as f32,
                (row * cell_h) as f32 / atlas_h as f32,
                ((col + 1) * cell_w) as f32 / atlas_w as f32,
                ((row + 1) * cell_h) as f32 / atlas_h as f32,
            ],
        );
    }

    create_font_atlas_resources(
        device,
        queue,
        bind_group_layout,
        atlas_w,
        atlas_h,
        bytes,
        uv_map,
        false,
        smoothing,
        "Fallback".to_string(),
    )
}

fn create_font_atlas_resources(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bind_group_layout: &wgpu::BindGroupLayout,
    atlas_w: u32,
    atlas_h: u32,
    bytes: Vec<u8>,
    uv_map: HashMap<char, [f32; 4]>,
    uses_runtime_font: bool,
    smoothing: TextSmoothing,
    font_label: String,
) -> FontAtlas {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("suzaku-font-atlas"),
        size: wgpu::Extent3d {
            width: atlas_w,
            height: atlas_h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        &bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(atlas_w),
            rows_per_image: Some(atlas_h),
        },
        wgpu::Extent3d {
            width: atlas_w,
            height: atlas_h,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("suzaku-font-atlas-sampler"),
        mag_filter: if smoothing == TextSmoothing::Smooth {
            wgpu::FilterMode::Linear
        } else {
            wgpu::FilterMode::Nearest
        },
        min_filter: if smoothing == TextSmoothing::Smooth {
            wgpu::FilterMode::Linear
        } else {
            wgpu::FilterMode::Nearest
        },
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("suzaku-font-atlas-bind-group"),
        layout: bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });

    FontAtlas {
        bind_group,
        uv_map,
        uses_runtime_font,
        font_label,
    }
}

fn atlas_charset() -> Vec<char> {
    let mut glyphs: Vec<char> = (32u8..=126u8).map(char::from).collect();
    glyphs.push('…');
    glyphs.extend(emoji_glyph_range());
    glyphs
}

fn atlas_lookup_char(ch: char) -> char {
    if ch.is_ascii_alphabetic() {
        ch.to_ascii_lowercase()
    } else if ch == '…' {
        '…'
    } else if ch.is_ascii() {
        ch
    } else {
        ch
    }
}

fn emoji_glyph_range() -> Vec<char> {
    (0x1F300u32..=0x1FAFF)
        .filter_map(char::from_u32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::atlas_charset;
    use super::atlas_lookup_char;

    #[test]
    fn atlas_charset_contains_emoji_block() {
        let charset = atlas_charset();

        assert!(charset.contains(&'😀'));
        assert!(charset.contains(&'🎉'));
        assert!(charset.contains(&'🚀'));
    }

    #[test]
    fn atlas_lookup_does_not_scrub_emoji() {
        assert_eq!(atlas_lookup_char('😀'), '😀');
    }
}

fn load_runtime_font(font_face: FontFaceChoice) -> Option<(Font, String)> {
    for (path, label) in preferred_font_paths(font_face) {
        if let Ok(bytes) = fs::read(path) {
            if let Ok(font) = Font::from_bytes(bytes, fontdue::FontSettings::default()) {
                return Some((font, label.to_string()));
            }
        }
    }
    None
}
