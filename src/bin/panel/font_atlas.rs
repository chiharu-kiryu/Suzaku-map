//! Bounded, on-demand Unicode glyph atlas. Only visible characters are rasterized.
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use suzaku_map::ime::gpu::{FontFaceChoice, TextSmoothing};
use suzaku_map::languages::BuiltinLanguage;
use suzaku_map::platform::gpu_host::preferred_font_paths;

const MISSING: char = '\u{fffd}';
const MAX_FONT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct FontSource {
    path: PathBuf,
    index: u32,
}

#[derive(Clone)]
struct LoadedFont {
    font: Arc<fontdue::Font>,
    label: String,
}

fn load_font(source: &FontSource) -> Option<LoadedFont> {
    static CACHE: OnceLock<Mutex<HashMap<FontSource, Option<LoadedFont>>>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    cache
        .entry(source.clone())
        .or_insert_with(|| {
            if std::fs::metadata(&source.path).ok()?.len() > MAX_FONT_BYTES {
                return None;
            }
            let bytes = std::fs::read(&source.path).ok()?;
            let font = std::panic::catch_unwind(|| {
                fontdue::Font::from_bytes(
                    bytes,
                    fontdue::FontSettings {
                        collection_index: source.index,
                        scale: 96.0,
                        load_substitutions: false,
                    },
                )
            })
            .ok()?
            .ok()?;
            let label = font
                .name()
                .unwrap_or("System font")
                .chars()
                .filter(|ch| !ch.is_control())
                .take(80)
                .collect();
            Some(LoadedFont {
                font: Arc::new(font),
                label,
            })
        })
        .clone()
}

#[cfg(target_os = "linux")]
fn fontconfig_source(japanese: bool) -> Option<FontSource> {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    // Fixed arguments, no typed input, and bounded startup work. No shell or per-frame lookup.
    let mut child = Command::new("fc-match")
        .args([
            "-f",
            "%{file}\n%{index}\n",
            if japanese {
                "sans-serif:lang=ja"
            } else {
                "sans-serif:lang=zh-cn"
            },
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_millis(400);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let output = child.wait_with_output().ok()?;
    if !output.status.success() || output.stdout.len() > 8192 {
        return None;
    }
    let output = String::from_utf8(output.stdout).ok()?;
    let mut lines = output.lines();
    let path = PathBuf::from(lines.next()?);
    if !path.is_absolute() {
        return None;
    }
    Some(FontSource {
        path,
        index: lines.next()?.parse::<u32>().ok()? & 0xffff,
    })
}

fn sources_for(face: FontFaceChoice, japanese: bool) -> Vec<FontSource> {
    let mut sources = Vec::new();
    let preferred = preferred_font_paths(face);
    // Preserve the chosen UI face, then consult the locale-specific system fallback.
    if let Some((path, _)) = preferred.first() {
        sources.push(FontSource {
            path: path.into(),
            index: 0,
        });
    }
    #[cfg(target_os = "linux")]
    if let Some(source) = fontconfig_source(japanese) {
        if let Some(primary) = sources.first_mut()
            && primary.path == source.path
        {
            // A manually selected CJK collection must honor the same SC/JP face as Auto.
            primary.index = source.index;
        }
        sources.push(source);
    }

    let fallback_paths = match std::env::consts::OS {
        "linux" => vec![
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ],
        "macos" => vec![
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/Library/Fonts/Arial Unicode.ttf",
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        ],
        "windows" if japanese => vec![
            "C:\\Windows\\Fonts\\YuGothM.ttc",
            "C:\\Windows\\Fonts\\meiryo.ttc",
            "C:\\Windows\\Fonts\\msyh.ttc",
            "C:\\Windows\\Fonts\\arialuni.ttf",
        ],
        "windows" => vec![
            "C:\\Windows\\Fonts\\msyh.ttc",
            "C:\\Windows\\Fonts\\YuGothM.ttc",
            "C:\\Windows\\Fonts\\meiryo.ttc",
            "C:\\Windows\\Fonts\\arialuni.ttf",
        ],
        "android" => vec![
            "/system/fonts/NotoSansCJK-Regular.ttc",
            "/system/fonts/NotoSans-Regular.ttf",
            "/system/fonts/Roboto-Regular.ttf",
        ],
        _ => vec![],
    };
    sources.extend(fallback_paths.into_iter().map(|path| FontSource {
        path: path.into(),
        index: 0,
    }));
    sources.extend(preferred.into_iter().skip(1).map(|(path, _)| FontSource {
        path: path.into(),
        index: 0,
    }));
    sources.extend(
        preferred_font_paths(FontFaceChoice::Auto)
            .into_iter()
            .map(|(path, _)| FontSource {
                path: path.into(),
                index: 0,
            }),
    );
    let mut unique = Vec::new();
    for source in sources {
        // Windows need not be installed on C:. Only fixed system font paths are relocated.
        #[cfg(target_os = "windows")]
        let source = {
            let mut source = source;
            if let Some(root) = std::env::var_os("WINDIR") {
                if let Some(name) = source
                    .path
                    .to_str()
                    .and_then(|path| path.strip_prefix("C:\\Windows\\Fonts\\"))
                {
                    source.path = PathBuf::from(root).join("Fonts").join(name);
                }
            }
            source
        };
        if !unique.contains(&source) {
            unique.push(source);
        }
    }
    unique
}

struct FontResolver {
    sources: Vec<FontSource>,
    loaded: HashMap<usize, Option<LoadedFont>>,
}

impl FontResolver {
    fn new(face: FontFaceChoice, japanese: bool) -> Self {
        Self {
            sources: sources_for(face, japanese),
            loaded: HashMap::new(),
        }
    }

    fn rasterize(
        &mut self,
        ch: char,
        pixels: f32,
    ) -> Option<(fontdue::Metrics, Vec<u8>, LoadedFont)> {
        for (index, source) in self.sources.iter().enumerate() {
            let Some(font) = self
                .loaded
                .entry(index)
                .or_insert_with(|| load_font(source))
            else {
                continue;
            };
            if !font.font.has_glyph(ch) {
                continue;
            }
            let (metrics, bytes) = font.font.rasterize(ch, pixels);
            // Color-only bitmap emoji fonts may report coverage but supply no outline.
            if bytes.iter().any(|coverage| *coverage != 0) {
                return Some((metrics, bytes, font.clone()));
            }
        }
        None
    }
}

#[derive(Clone, Copy, Debug)]
struct AtlasGrid {
    dimension: u32,
    cell: u32,
    columns: u32,
    capacity: usize,
    pixels: f32,
}

impl AtlasGrid {
    fn new(scale: f32, limit: u32) -> Self {
        let scale = if scale.is_finite() {
            scale.clamp(1.0, 2.5)
        } else {
            1.0
        };
        let dimension = limit.min(if scale > 1.5 { 4096 } else { 2048 }).max(1);
        let cell = ((48.0 * scale * 1.6).ceil() as u32).clamp(1, (dimension / 16).max(1));
        let columns = dimension / cell;
        Self {
            dimension,
            cell,
            columns,
            capacity: (columns * columns) as usize,
            pixels: cell as f32 / 1.6,
        }
    }

    fn origin(self, slot: usize) -> [u32; 2] {
        [
            (slot as u32 % self.columns) * self.cell,
            (slot as u32 / self.columns) * self.cell,
        ]
    }
}

struct GlyphCell {
    bytes: Vec<u8>,
    // Coordinates inside the cell, excluding transparent packing padding.
    sample: [u32; 4],
    font_label: Option<String>,
    missing: bool,
}

fn rasterize_cell(resolver: &mut FontResolver, ch: char, grid: AtlasGrid) -> GlyphCell {
    let size = grid.cell;
    let mut bytes = vec![0; (size * size) as usize];
    let padding = (size / 16).max(1).min(size.saturating_sub(1));
    let available = size.saturating_sub(2 * padding).max(1);
    let mut pixels = grid.pixels;
    if let Some((mut metrics, mut bitmap, mut font)) = resolver.rasterize(ch, pixels) {
        let span = metrics.width.max(metrics.height) as f32;
        if span > available as f32 {
            pixels *= available as f32 / span;
            if let Some(rendered) = resolver.rasterize(ch, pixels) {
                (metrics, bitmap, font) = rendered;
            }
        }
        let lines = font.font.horizontal_line_metrics(pixels);
        let wide = unicode_width::UnicodeWidthChar::width(ch) == Some(2);
        let ascent = if wide {
            pixels * 0.88
        } else {
            lines.map(|line| line.ascent).unwrap_or(pixels)
        };
        let descent = if wide {
            -pixels * 0.12
        } else {
            lines.map(|line| line.descent).unwrap_or(0.0)
        };
        let line_height = (ascent - descent)
            .ceil()
            .max(metrics.height as f32)
            .min(available as f32) as u32;
        let advance = if wide {
            pixels.max(metrics.advance_width)
        } else {
            metrics.advance_width
        };
        let sample_width = advance
            .ceil()
            .max(metrics.width as f32)
            .clamp(1.0, available as f32) as u32;
        let left = (size - sample_width) / 2;
        let top = (size - line_height) / 2;
        let destination_x = (left as i32 + metrics.xmin).clamp(
            padding as i32,
            (size - padding - metrics.width.min(available as usize) as u32) as i32,
        );
        let destination_y =
            (top as i32 + ascent.ceil() as i32 - metrics.ymin - metrics.height as i32).clamp(
                padding as i32,
                (size - padding - metrics.height.min(available as usize) as u32) as i32,
            );
        for y in 0..metrics.height.min(available as usize) {
            for x in 0..metrics.width.min(available as usize) {
                bytes[(destination_y as usize + y) * size as usize + destination_x as usize + x] =
                    bitmap[y * metrics.width + x];
            }
        }
        let x1 = left.min(destination_x as u32);
        let y1 = top.min(destination_y as u32);
        let x2 = (left + sample_width)
            .max(destination_x as u32 + metrics.width as u32)
            .min(size);
        let y2 = (top + line_height)
            .max(destination_y as u32 + metrics.height as u32)
            .min(size);
        return GlyphCell {
            bytes,
            sample: [x1, y1, x2, y2],
            font_label: Some(font.label),
            missing: false,
        };
    }
    if ch.is_ascii() && !ch.is_control() {
        let bitmap = suzaku_map::ime::gpu::glyph_bitmap(ch);
        let step = (available / 7).max(1);
        let left = size.saturating_sub(5 * step) / 2;
        let top = size.saturating_sub(7 * step) / 2;
        for (row, pattern) in bitmap.iter().enumerate() {
            for col in 0..5 {
                if (pattern >> (4 - col)) & 1 != 0 {
                    for y in 0..step {
                        for x in 0..step {
                            let dx = left + col * step + x;
                            let dy = top + row as u32 * step + y;
                            if dx < size && dy < size {
                                bytes[(dy * size + dx) as usize] = 255;
                            }
                        }
                    }
                }
            }
        }
        return GlyphCell {
            bytes,
            sample: [
                left,
                top,
                (left + 5 * step).min(size),
                (top + 7 * step).min(size),
            ],
            font_label: None,
            missing: false,
        };
    }
    // A distinct missing-glyph box, never a literal question mark or an empty candidate.
    for y in padding..size.saturating_sub(padding) {
        for x in padding..size.saturating_sub(padding) {
            if x == padding || y == padding || x + padding + 1 == size || y + padding + 1 == size {
                bytes[(y * size + x) as usize] = 255;
            }
        }
    }
    GlyphCell {
        bytes,
        sample: [0, 0, size, size],
        font_label: None,
        missing: true,
    }
}

pub(crate) struct FontAtlas {
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) uses_runtime_font: bool,
    pub(crate) font_label: String,
    texture: wgpu::Texture,
    grid: AtlasGrid,
    resolver: FontResolver,
    uv_map: HashMap<char, [f32; 4]>,
    missing: BTreeSet<char>,
    visible_missing: usize,
    face: FontFaceChoice,
    japanese: bool,
    next_slot: usize,
}

impl FontAtlas {
    pub(super) fn status_label(&self) -> String {
        if self.visible_missing == 0 {
            self.font_label.clone()
        } else {
            format!(
                "{} · {} glyphs unavailable (check installed fonts)",
                self.font_label, self.visible_missing
            )
        }
    }

    pub(super) fn uv_for(&self, ch: char) -> [f32; 4] {
        self.uv_map
            .get(&ch)
            .or_else(|| self.uv_map.get(&MISSING))
            .copied()
            .expect("missing-glyph cell")
    }

    pub(super) fn ensure_glyphs(
        &mut self,
        queue: &wgpu::Queue,
        language: &str,
        characters: impl IntoIterator<Item = char>,
    ) {
        let japanese = BuiltinLanguage::resolve(language) == Some(BuiltinLanguage::Japanese);
        if self.japanese != japanese {
            self.japanese = japanese;
            self.resolver = FontResolver::new(self.face, japanese);
            self.clear(queue);
        }
        let visible: BTreeSet<_> = characters.into_iter().take(16_384).collect();
        let unseen = visible
            .iter()
            .filter(|ch| !self.uv_map.contains_key(ch))
            .count();
        if unseen == 0 {
            self.visible_missing = visible
                .iter()
                .filter(|ch| self.missing.contains(ch))
                .count();
            return;
        }
        if self.next_slot + unseen > self.grid.capacity || self.uv_map.len() + unseen > 4096 {
            self.clear(queue);
        }
        for &ch in &visible {
            if !self.uv_map.contains_key(&ch)
                && self.next_slot < self.grid.capacity
                && self.uv_map.len() < 4096
            {
                self.insert(queue, ch);
            }
        }
        self.visible_missing = visible
            .iter()
            .filter(|ch| self.missing.contains(ch) || !self.uv_map.contains_key(ch))
            .count();
    }

    fn clear(&mut self, queue: &wgpu::Queue) {
        self.uv_map.clear();
        self.missing.clear();
        self.next_slot = 0;
        self.insert(queue, MISSING);
    }

    fn insert(&mut self, queue: &wgpu::Queue, ch: char) {
        let cell = rasterize_cell(&mut self.resolver, ch, self.grid);
        if cell.missing && ch != MISSING {
            self.missing.insert(ch);
            self.uv_map.insert(ch, self.uv_for(MISSING));
            return;
        }
        let [x, y] = self.grid.origin(self.next_slot);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &cell.bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.grid.cell),
                rows_per_image: Some(self.grid.cell),
            },
            wgpu::Extent3d {
                width: self.grid.cell,
                height: self.grid.cell,
                depth_or_array_layers: 1,
            },
        );
        let [left, top, right, bottom] = cell.sample;
        let dimension = self.grid.dimension as f32;
        self.uv_map.insert(
            ch,
            [
                (x + left) as f32 / dimension,
                (y + top) as f32 / dimension,
                (x + right) as f32 / dimension,
                (y + bottom) as f32 / dimension,
            ],
        );
        self.next_slot += 1;
        if let Some(label) = cell.font_label {
            self.uses_runtime_font = true;
            if !self.font_label.contains(&label) {
                if self.font_label == "Builtin bitmap" {
                    self.font_label = label;
                } else {
                    self.font_label.push_str(&format!(" + {label}"));
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "font_atlas_visual_tests.rs"]
mod visual_tests;

pub(crate) fn create_font_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    face: FontFaceChoice,
    smoothing: TextSmoothing,
    scale: f32,
) -> FontAtlas {
    let grid = AtlasGrid::new(scale, device.limits().max_texture_dimension_2d);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("suzaku-unicode-glyph-cache"),
        size: wgpu::Extent3d {
            width: grid.dimension,
            height: grid.dimension,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("suzaku-unicode-font-sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: if smoothing == TextSmoothing::Smooth {
            wgpu::FilterMode::Linear
        } else {
            wgpu::FilterMode::Nearest
        },
        ..Default::default()
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("suzaku-unicode-font-atlas"),
        layout,
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
    let mut atlas = FontAtlas {
        bind_group,
        uses_runtime_font: false,
        font_label: "Builtin bitmap".into(),
        texture,
        grid,
        resolver: FontResolver::new(face, false),
        uv_map: HashMap::new(),
        missing: BTreeSet::new(),
        visible_missing: 0,
        face,
        japanese: false,
        next_slot: 0,
    };
    atlas.insert(queue, MISSING);
    atlas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_is_bounded_at_low_texture_limits_and_high_dpi() {
        for scale in [1.0, 1.25, 2.0, 2.5, f32::NAN] {
            for limit in [256, 512, 2048, 8192] {
                let grid = AtlasGrid::new(scale, limit);
                assert!(grid.dimension <= limit);
                assert!(grid.capacity >= 256);
                let [x, y] = grid.origin(grid.capacity - 1);
                assert!(x + grid.cell <= grid.dimension && y + grid.cell <= grid.dimension);
            }
        }
    }

    #[test]
    fn no_system_fonts_keep_ascii_but_mark_unknown_unicode_distinctly() {
        let mut resolver = FontResolver {
            sources: vec![],
            loaded: HashMap::new(),
        };
        let grid = AtlasGrid::new(1.0, 2048);
        let ascii = rasterize_cell(&mut resolver, '?', grid);
        let missing = rasterize_cell(&mut resolver, '你', grid);
        assert!(!ascii.missing && missing.missing);
        assert!(ascii.bytes.iter().any(|value| *value != 0));
        assert!(missing.bytes.iter().any(|value| *value != 0));
        assert_ne!(ascii.bytes, missing.bytes);
    }

    #[test]
    fn nonexistent_font_is_a_recoverable_fallback() {
        assert!(
            load_font(&FontSource {
                path: "/not/a/suzaku/font.ttf".into(),
                index: 0
            })
            .is_none()
        );
    }

    #[test]
    fn installed_cjk_fallback_rasterizes_real_candidate_glyphs_in_all_font_choices() {
        // Portable CI may not ship a CJK font. The explicit local visual test requires it.
        let mut available = FontResolver::new(FontFaceChoice::Auto, false);
        if available.rasterize('你', 48.0).is_none() {
            return;
        }
        for face in [
            FontFaceChoice::Auto,
            FontFaceChoice::Monaco,
            FontFaceChoice::PingFang,
        ] {
            for japanese in [false, true] {
                let mut resolver = FontResolver::new(face, japanese);
                #[cfg(target_os = "linux")]
                if let Some(locale) = fontconfig_source(japanese)
                    && let Some(primary) = resolver.sources.first()
                    && primary.path == locale.path
                {
                    assert_eq!(primary.index, locale.index);
                }
                for ch in "你好世界日本語こんにちは、。！？hello".chars() {
                    for scale in [1.0, 2.5] {
                        let grid = AtlasGrid::new(scale, 2048);
                        let cell = rasterize_cell(&mut resolver, ch, grid);
                        assert!(!cell.missing, "missing {ch:?}");
                        assert!(cell.bytes.iter().any(|byte| *byte != 0));
                        assert!(cell.sample[0] < cell.sample[2] && cell.sample[1] < cell.sample[3]);
                        assert!(cell.sample[2] <= grid.cell && cell.sample[3] <= grid.cell);
                    }
                }
            }
        }
    }
}
