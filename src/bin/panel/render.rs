use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use bytemuck::{Pod, Zeroable};
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
    let atlas_scale = font_scale.clamp(1.0, 2.5);
    if let Some(runtime_font) = load_runtime_font(font_face)
        && let Some(atlas) = rasterize_runtime_font_atlas(
            &runtime_font.font,
            runtime_font.label,
            atlas_scale,
            device.limits().max_texture_dimension_2d,
        )
    {
        return create_font_atlas_resources(
            device,
            queue,
            bind_group_layout,
            atlas.width,
            atlas.height,
            atlas.bytes,
            atlas.uv_map,
            true,
            smoothing,
            atlas.font_label,
        );
    }

    create_bitmap_font_atlas(device, queue, bind_group_layout, atlas_scale, smoothing)
}

#[derive(Clone)]
struct CachedRuntimeFont {
    font: Arc<fontdue::Font>,
    label: String,
}

struct RasterizedRuntimeFontAtlas {
    width: u32,
    height: u32,
    bytes: Vec<u8>,
    uv_map: HashMap<char, [f32; 4]>,
    font_label: String,
}

fn font_face_cache_key(font_face: FontFaceChoice) -> u8 {
    match font_face {
        FontFaceChoice::Auto => 0,
        FontFaceChoice::Monaco => 1,
        FontFaceChoice::Menlo => 2,
        FontFaceChoice::Geneva => 3,
        FontFaceChoice::Helvetica => 4,
        FontFaceChoice::PingFang => 5,
        FontFaceChoice::ArialUnicode => 6,
    }
}

fn load_runtime_font(font_face: FontFaceChoice) -> Option<CachedRuntimeFont> {
    type FontCache = HashMap<u8, Option<CachedRuntimeFont>>;
    static CACHE: OnceLock<Mutex<FontCache>> = OnceLock::new();

    let key = font_face_cache_key(font_face);
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    {
        let guard = cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(cached) = guard.get(&key) {
            return cached.clone();
        }
    }

    let loaded =
        preferred_font_paths(font_face)
            .into_iter()
            .find_map(|(path, configured_label)| {
                let bytes = std::fs::read(path).ok()?;
                let settings = fontdue::FontSettings {
                    scale: 96.0,
                    ..fontdue::FontSettings::default()
                };
                let font = fontdue::Font::from_bytes(bytes, settings).ok()?;
                let label = font
                    .name()
                    .map(str::to_string)
                    .unwrap_or_else(|| configured_label.to_string());
                Some(CachedRuntimeFont {
                    font: Arc::new(font),
                    label,
                })
            });

    let mut guard = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.insert(key, loaded.clone());
    loaded
}

fn runtime_atlas_grid(
    glyph_count: usize,
    cell_width: u32,
    cell_height: u32,
    max_dimension: u32,
) -> Option<(u32, u32, u32, u32)> {
    if glyph_count == 0
        || cell_width == 0
        || cell_height == 0
        || cell_width > max_dimension
        || cell_height > max_dimension
    {
        return None;
    }

    let max_columns = max_dimension / cell_width;
    let max_rows = max_dimension / cell_height;
    if max_columns == 0
        || max_rows == 0
        || u64::from(max_columns) * u64::from(max_rows) < glyph_count as u64
    {
        return None;
    }

    let ideal_columns = ((glyph_count as f64 * f64::from(cell_height) / f64::from(cell_width))
        .sqrt()
        .ceil() as u32)
        .max(1);
    let minimum_columns = (glyph_count as u32).div_ceil(max_rows).max(1);
    let columns = ideal_columns.max(minimum_columns).min(max_columns);
    let rows = (glyph_count as u32).div_ceil(columns);
    let width = columns.checked_mul(cell_width)?;
    let height = rows.checked_mul(cell_height)?;
    (width <= max_dimension && height <= max_dimension).then_some((columns, rows, width, height))
}

fn rasterize_runtime_font_atlas(
    font: &fontdue::Font,
    font_label: String,
    atlas_scale: f32,
    max_dimension: u32,
) -> Option<RasterizedRuntimeFontAtlas> {
    let font_px = 48.0 * atlas_scale.clamp(1.0, 2.5);
    let line_metrics = font.horizontal_line_metrics(font_px)?;
    let padding = (font_px * 0.06).ceil().max(2.0) as u32;
    let glyph_line_height = (line_metrics.ascent - line_metrics.descent).ceil().max(1.0) as u32;
    let cell_height = glyph_line_height.checked_add(padding.checked_mul(2)?)?;
    let cell_width = ((cell_height as f32 * 5.0 / 7.0).ceil() as u32).max(1);
    let baseline = padding as i32 + line_metrics.ascent.ceil() as i32;

    let glyphs: Vec<char> = atlas_charset()
        .into_iter()
        .filter(|ch| font.has_glyph(*ch))
        .collect();
    if !glyphs.contains(&'?') {
        return None;
    }
    let (columns, _rows, atlas_width, atlas_height) =
        runtime_atlas_grid(glyphs.len(), cell_width, cell_height, max_dimension)?;
    let byte_len = usize::try_from(u64::from(atlas_width) * u64::from(atlas_height)).ok()?;
    let mut bytes = vec![0_u8; byte_len];
    let mut uv_map = HashMap::with_capacity(glyphs.len());

    for (index, ch) in glyphs.into_iter().enumerate() {
        let column = index as u32 % columns;
        let row = index as u32 / columns;
        let cell_x = column * cell_width;
        let cell_y = row * cell_height;
        let (metrics, bitmap) = font.rasterize(ch, font_px);
        let advance_offset = ((cell_width as f32 - metrics.advance_width) * 0.5).round() as i32;
        let destination_x = cell_x as i32 + advance_offset + metrics.xmin;
        let destination_y = cell_y as i32 + baseline - metrics.ymin - metrics.height as i32;

        for source_y in 0..metrics.height {
            let target_y = destination_y + source_y as i32;
            if target_y < cell_y as i32 || target_y >= (cell_y + cell_height) as i32 {
                continue;
            }
            for source_x in 0..metrics.width {
                let target_x = destination_x + source_x as i32;
                if target_x < cell_x as i32 || target_x >= (cell_x + cell_width) as i32 {
                    continue;
                }
                let source_index = source_y * metrics.width + source_x;
                let target_index = target_y as usize * atlas_width as usize + target_x as usize;
                bytes[target_index] = bitmap[source_index];
            }
        }

        // Sample the font's advance box instead of the entire packing cell. The cell also
        // contains atlas padding; stretching that padding into every text quad made real fonts
        // look unnaturally narrow and widely tracked.
        let advance_origin_x = cell_x as i32 + advance_offset;
        let sample_padding = (padding as i32 / 2).max(1);
        let sample_left = advance_origin_x
            .min(destination_x)
            .saturating_sub(sample_padding)
            .clamp(cell_x as i32, (cell_x + cell_width - 1) as i32);
        let sample_right = (advance_origin_x + metrics.advance_width.ceil() as i32)
            .max(destination_x + metrics.width as i32)
            .saturating_add(sample_padding)
            .clamp(sample_left + 1, (cell_x + cell_width) as i32);

        uv_map.insert(
            ch,
            [
                sample_left as f32 / atlas_width as f32,
                cell_y as f32 / atlas_height as f32,
                sample_right as f32 / atlas_width as f32,
                (cell_y + cell_height) as f32 / atlas_height as f32,
            ],
        );
    }

    Some(RasterizedRuntimeFontAtlas {
        width: atlas_width,
        height: atlas_height,
        bytes,
        uv_map,
        font_label,
    })
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
    let use_linear_filter = uses_runtime_font || smoothing == TextSmoothing::Smooth;
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("suzaku-font-atlas-sampler"),
        mag_filter: if use_linear_filter {
            wgpu::FilterMode::Linear
        } else {
            wgpu::FilterMode::Nearest
        },
        min_filter: if use_linear_filter {
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
    ch
}

fn emoji_glyph_range() -> Vec<char> {
    (0x1F300u32..=0x1FAFF).filter_map(char::from_u32).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        atlas_charset, atlas_lookup_char, load_runtime_font, rasterize_runtime_font_atlas,
        runtime_atlas_grid,
    };
    use suzaku_map::ime::gpu::FontFaceChoice;

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

    #[test]
    fn atlas_lookup_preserves_ascii_case_for_runtime_fonts() {
        assert_eq!(atlas_lookup_char('A'), 'A');
        assert_eq!(atlas_lookup_char('a'), 'a');
    }

    #[test]
    fn runtime_atlas_grid_fits_cells_inside_device_limits() {
        let (columns, rows, width, height) =
            runtime_atlas_grid(1_200, 64, 88, 8_192).expect("atlas grid");

        assert!(columns * rows >= 1_200);
        assert!(width <= 8_192);
        assert!(height <= 8_192);
        assert_eq!(width, columns * 64);
        assert_eq!(height, rows * 88);
    }

    #[test]
    fn runtime_atlas_grid_rejects_impossible_device_limits() {
        assert!(runtime_atlas_grid(10, 64, 88, 32).is_none());
        assert!(runtime_atlas_grid(10_000, 64, 88, 512).is_none());
    }

    #[test]
    fn preferred_runtime_font_rasterizes_real_ascii_glyphs_when_available() {
        let Some(runtime_font) = load_runtime_font(FontFaceChoice::Auto) else {
            return;
        };
        let atlas =
            rasterize_runtime_font_atlas(&runtime_font.font, runtime_font.label, 1.0, 8_192)
                .expect("rasterize preferred font");

        assert!(atlas.uv_map.contains_key(&'A'));
        assert!(atlas.uv_map.contains_key(&'a'));
        assert!(atlas.uv_map.contains_key(&'?'));
        assert!(atlas.bytes.iter().any(|coverage| *coverage > 0));
    }
}
