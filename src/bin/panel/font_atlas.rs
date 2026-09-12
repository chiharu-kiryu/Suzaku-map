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

#[path = "font_raster.rs"]
mod raster;
use raster::{CachedGlyph, GlyphFont, MAX_GLYPHS, RasterKey, RasterPage};
use suzaku_map::ime::gpu::AtlasGlyph;

pub(crate) struct FontAtlas {
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) uses_runtime_font: bool,
    pub(crate) font_label: String,
    texture: wgpu::Texture,
    page: RasterPage,
    resolver: FontResolver,
    fonts: HashMap<char, GlyphFont>,
    uv_map: HashMap<RasterKey, CachedGlyph>,
    pub(super) layout_metrics: suzaku_map::ime::gpu::FontLayoutMetrics,
    pub(super) layout_revision: u64,
    missing: BTreeSet<char>,
    visible_missing: usize,
    face: FontFaceChoice,
    japanese: bool,
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

    pub(super) fn quad_for(&self, glyph: &AtlasGlyph) -> ([f32; 4], [f32; 4]) {
        let Some(key) = RasterKey::for_glyph(glyph) else {
            return ([0.0; 4], [0.0; 4]);
        };
        self.uv_map
            .get(&key)
            .or_else(|| self.uv_map.get(&Self::fallback_key()))
            .map_or(([0.0; 4], [0.0; 4]), |cached| cached.quad(glyph))
    }

    fn fallback_key() -> RasterKey {
        RasterKey {
            ch: MISSING,
            height: 18,
        }
    }

    pub(super) fn ensure_glyphs<'a>(
        &mut self,
        queue: &wgpu::Queue,
        language: &str,
        glyphs: impl IntoIterator<Item = &'a AtlasGlyph>,
    ) {
        let japanese = BuiltinLanguage::resolve(language) == Some(BuiltinLanguage::Japanese);
        if self.japanese != japanese {
            self.japanese = japanese;
            self.resolver = FontResolver::new(self.face, japanese);
            self.clear(queue);
        }
        let visible: BTreeSet<_> = glyphs
            .into_iter()
            .take(16_384)
            .filter_map(RasterKey::for_glyph)
            .collect();
        let characters: BTreeSet<_> = visible.iter().map(|key| key.ch).collect();
        let unseen = characters
            .iter()
            .filter(|ch| !self.fonts.contains_key(ch))
            .count();
        if self.fonts.len() + unseen > MAX_GLYPHS {
            self.clear(queue);
        }
        for &ch in characters.iter().take(MAX_GLYPHS - 1) {
            self.ensure_font(ch);
        }
        if !self.populate(queue, &visible) {
            // Evict only bitmap placement. Stable font advances must not trigger a
            // relayout cycle whenever a new size fills the raster page.
            self.clear_rasters(queue);
            self.populate(queue, &visible);
        }
        let mut unavailable: BTreeSet<_> =
            self.missing.intersection(&characters).copied().collect();
        unavailable.extend(
            visible
                .iter()
                .filter(|key| !self.uv_map.contains_key(key))
                .map(|key| key.ch),
        );
        self.visible_missing = unavailable.len();
    }

    fn ensure_font(&mut self, ch: char) {
        if self.fonts.contains_key(&ch) {
            return;
        }
        let glyph = GlyphFont::resolve(&mut self.resolver, ch);
        if glyph.missing && ch != MISSING {
            self.missing.insert(ch);
        }
        if let Some(font) = &glyph.font {
            Arc::make_mut(&mut self.layout_metrics).insert(ch, glyph.width_ratio * 7.0);
            self.layout_revision = self.layout_revision.wrapping_add(1);
            self.uses_runtime_font = true;
            if !self.font_label.contains(&font.label) {
                if self.font_label == "Builtin bitmap" {
                    self.font_label = font.label.clone();
                } else {
                    self.font_label.push_str(&format!(" + {}", font.label));
                }
            }
        }
        self.fonts.insert(ch, glyph);
    }

    fn clear(&mut self, queue: &wgpu::Queue) {
        self.fonts.clear();
        Arc::make_mut(&mut self.layout_metrics).clear();
        self.layout_revision = self.layout_revision.wrapping_add(1);
        self.missing.clear();
        self.font_label = "Builtin bitmap".into();
        self.uses_runtime_font = false;
        self.ensure_font(MISSING);
        self.clear_rasters(queue);
    }

    fn clear_rasters(&mut self, queue: &wgpu::Queue) {
        self.uv_map.clear();
        self.page = RasterPage::new(self.page.dimension);
        self.insert(queue, Self::fallback_key());
    }

    fn populate(&mut self, queue: &wgpu::Queue, visible: &BTreeSet<RasterKey>) -> bool {
        for &key in visible {
            if !self.uv_map.contains_key(&key) && !self.insert(queue, key) {
                return false;
            }
        }
        true
    }

    fn insert(&mut self, queue: &wgpu::Queue, key: RasterKey) -> bool {
        if self.uv_map.len() >= MAX_GLYPHS {
            return false;
        }
        let Some(font) = self.fonts.get(&key.ch) else {
            return false;
        };
        let glyph = font.rasterize(key);
        let Some([x, y]) = self.page.allocate(glyph.width, glyph.height) else {
            return false;
        };
        let width = glyph.width + 2;
        let height = glyph.height + 2;
        let mut padded = vec![0; (width * height) as usize];
        for row in 0..glyph.height as usize {
            let start = (row + 1) * width as usize + 1;
            padded[start..start + glyph.width as usize].copy_from_slice(
                &glyph.bytes[row * glyph.width as usize..(row + 1) * glyph.width as usize],
            );
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &padded,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let dimension = self.page.dimension as f32;
        self.uv_map.insert(
            key,
            CachedGlyph {
                uv: [
                    (x + 1) as f32 / dimension,
                    (y + 1) as f32 / dimension,
                    (x + 1 + glyph.width) as f32 / dimension,
                    (y + 1 + glyph.height) as f32 / dimension,
                ],
                size: [glyph.width, glyph.height],
                offset: glyph.offset,
            },
        );
        true
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
) -> FontAtlas {
    // Use each scene glyph's physical height, not a guessed window zoom/DPI factor.
    let page = RasterPage::new(device.limits().max_texture_dimension_2d);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("suzaku-native-size-glyph-cache"),
        size: wgpu::Extent3d {
            width: page.dimension,
            height: page.dimension,
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
    let sampler = device.create_sampler(&font_sampler_descriptor(smoothing));
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
        page,
        resolver: FontResolver::new(face, false),
        fonts: HashMap::new(),
        uv_map: HashMap::new(),
        layout_metrics: Arc::new(HashMap::new()),
        layout_revision: 0,
        missing: BTreeSet::new(),
        visible_missing: 0,
        face,
        japanese: false,
    };
    atlas.ensure_font(MISSING);
    atlas.clear_rasters(queue);
    atlas
}

fn font_sampler_descriptor(smoothing: TextSmoothing) -> wgpu::SamplerDescriptor<'static> {
    let filter = match smoothing {
        TextSmoothing::Smooth => wgpu::FilterMode::Linear,
        TextSmoothing::Sharp => wgpu::FilterMode::Nearest,
    };
    wgpu::SamplerDescriptor {
        label: Some("suzaku-unicode-font-sampler"),
        // Native-size ink maps one texel to one pixel; keep the preference for
        // exceptional text above the bounded raster-size limit.
        mag_filter: filter,
        min_filter: filter,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_smoothing_retains_the_selected_sampler() {
        for (smoothing, expected) in [
            (TextSmoothing::Smooth, wgpu::FilterMode::Linear),
            (TextSmoothing::Sharp, wgpu::FilterMode::Nearest),
        ] {
            let sampler = font_sampler_descriptor(smoothing);
            assert_eq!(sampler.mag_filter, expected);
            assert_eq!(sampler.min_filter, expected);
        }
    }

    #[test]
    fn no_system_fonts_keep_ascii_but_mark_unknown_unicode_distinctly() {
        let mut resolver = FontResolver {
            sources: vec![],
            loaded: HashMap::new(),
        };
        let ascii = GlyphFont::resolve(&mut resolver, '?');
        let missing = GlyphFont::resolve(&mut resolver, '你');
        assert!(!ascii.missing && missing.missing);
        let ascii = ascii.rasterize(RasterKey {
            ch: '?',
            height: 18,
        });
        let missing = missing.rasterize(RasterKey {
            ch: '你',
            height: 18,
        });
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
    fn narrow_system_glyphs_keep_proportional_advances_and_native_ink_at_each_size() {
        let mut resolver = FontResolver::new(FontFaceChoice::Auto, false);
        let narrow = GlyphFont::resolve(&mut resolver, 'i');
        if narrow.font.is_none() {
            return;
        }
        let normal = GlyphFont::resolve(&mut resolver, 'n');
        assert!(narrow.width_ratio < normal.width_ratio);
        for height in [11, 14, 18, 23, 36, 64] {
            let narrow = narrow.rasterize(RasterKey { ch: 'i', height });
            let normal = normal.rasterize(RasterKey { ch: 'n', height });
            assert!(narrow.width <= normal.width);
            assert!(narrow.height <= u32::from(height) + 1);
            assert!(narrow.bytes.iter().any(|&a| a != 0));
        }
    }

    #[test]
    fn installed_cjk_fallback_rasterizes_real_candidate_glyphs_in_all_font_choices() {
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
                for ch in "你好世界日本語こんにちは、。！？hello+−×…".chars() {
                    let font = GlyphFont::resolve(&mut resolver, ch);
                    assert!(!font.missing, "missing {ch:?}");
                    for height in [14, 18, 23, 36] {
                        let glyph = font.rasterize(RasterKey { ch, height });
                        assert!(glyph.bytes.iter().any(|&a| a != 0), "{ch:?} at {height}");
                        assert_eq!(glyph.bytes.len(), (glyph.width * glyph.height) as usize);
                    }
                }
            }
        }
    }
}
