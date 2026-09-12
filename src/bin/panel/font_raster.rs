//! Physical-pixel glyph rasters. Layout advances remain fractional; ink is never stretched.
use super::{FontResolver, LoadedFont};
use suzaku_map::ime::gpu::AtlasGlyph;

pub(super) const MAX_GLYPHS: usize = 4096;
const MAX_HEIGHT: u16 = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct RasterKey {
    // Group similarly sized rasters when filling a fresh texture page.
    pub height: u16,
    pub ch: char,
}

impl RasterKey {
    pub fn for_glyph(glyph: &AtlasGlyph) -> Option<Self> {
        if !glyph.rect.iter().all(|value| value.is_finite())
            || glyph.rect[2] <= 0.0
            || glyph.rect[3] <= 0.0
        {
            return None;
        }
        Some(Self {
            ch: glyph.ch,
            height: glyph.rect[3].round().clamp(1.0, f32::from(MAX_HEIGHT)) as u16,
        })
    }
}

pub(super) struct GlyphFont {
    pub font: Option<LoadedFont>,
    pub missing: bool,
    pub width_ratio: f32,
    pixels_per_height: f32,
    baseline_ratio: f32,
    left_ratio: f32,
}

impl GlyphFont {
    pub fn resolve(resolver: &mut FontResolver, ch: char) -> Self {
        // Preserve the old 1x layout boxes (a 77px cell / 1.6), but never use
        // their bitmap for drawing. Advances stay stable across native raster sizes.
        const REFERENCE_PIXELS: f32 = 48.125;
        if let Some((metrics, _, font)) = resolver.rasterize(ch, REFERENCE_PIXELS) {
            let wide = unicode_width::UnicodeWidthChar::width(ch) == Some(2);
            let lines = font.font.horizontal_line_metrics(REFERENCE_PIXELS);
            let ascent = if wide {
                REFERENCE_PIXELS * 0.88
            } else {
                lines.map_or(REFERENCE_PIXELS, |m| m.ascent)
            };
            let descent = if wide {
                -REFERENCE_PIXELS * 0.12
            } else {
                lines.map_or(0.0, |m| m.descent)
            };
            let top = (ascent.ceil() - metrics.ymin as f32 - metrics.height as f32).min(0.0);
            let bottom = (ascent - descent)
                .ceil()
                .max(metrics.height as f32)
                .max(ascent.ceil() - metrics.ymin as f32);
            let height = (bottom - top).max(1.0);
            let left = (metrics.xmin as f32).min(0.0);
            let right = metrics
                .advance_width
                .ceil()
                .max(metrics.width as f32)
                .max(metrics.xmin as f32 + metrics.width as f32);
            return Self {
                font: Some(font),
                missing: false,
                width_ratio: if wide { 1.0 } else { (right - left) / height },
                pixels_per_height: REFERENCE_PIXELS / height,
                baseline_ratio: (ascent.ceil() - top) / height,
                left_ratio: -left / height,
            };
        }
        Self {
            font: None,
            missing: !(ch.is_ascii() && !ch.is_control()),
            width_ratio: if ch.is_ascii() { 5.0 / 7.0 } else { 1.0 },
            pixels_per_height: 1.0,
            baseline_ratio: 1.0,
            left_ratio: 0.0,
        }
    }

    pub fn rasterize(&self, key: RasterKey) -> GlyphRaster {
        let height = f32::from(key.height);
        if let Some(font) = &self.font {
            let pixels = height * self.pixels_per_height;
            let metrics = font.font.metrics(key.ch, pixels);
            // Bound font-provided dimensions before the rasterizer allocates its bitmap.
            if metrics.width <= 512 && metrics.height <= 512 {
                let (metrics, bytes) = font.font.rasterize(key.ch, pixels);
                return GlyphRaster {
                    bytes,
                    width: metrics.width as u32,
                    height: metrics.height as u32,
                    offset: [
                        height * self.left_ratio + metrics.xmin as f32,
                        height * self.baseline_ratio - metrics.ymin as f32 - metrics.height as f32,
                    ],
                };
            }
        }
        let h = u32::from(key.height);
        let w = (height * self.width_ratio).round().max(1.0) as u32;
        let mut bytes = vec![0; (w * h) as usize];
        let bitmap = suzaku_map::ime::gpu::glyph_bitmap(key.ch);
        for y in 0..h {
            for x in 0..w {
                let covered = if self.missing || self.font.is_some() {
                    x == 0 || y == 0 || x + 1 == w || y + 1 == h
                } else {
                    bitmap[(y * 7 / h) as usize] & (1 << (4 - x * 5 / w)) != 0
                };
                if covered {
                    bytes[(y * w + x) as usize] = 255;
                }
            }
        }
        GlyphRaster {
            bytes,
            width: w,
            height: h,
            offset: [0.0; 2],
        }
    }
}

pub(super) struct GlyphRaster {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub offset: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CachedGlyph {
    pub uv: [f32; 4],
    pub size: [u32; 2],
    pub offset: [f32; 2],
}

impl CachedGlyph {
    pub fn quad(self, glyph: &AtlasGlyph) -> ([f32; 4], [f32; 4]) {
        // Only exceptionally large text is scaled to respect the bounded raster size.
        let scale = (glyph.rect[3] / f32::from(MAX_HEIGHT)).max(1.0);
        (
            [
                (glyph.rect[0] + self.offset[0] * scale).round(),
                (glyph.rect[1] + self.offset[1] * scale).round(),
                self.size[0] as f32 * scale,
                self.size[1] as f32 * scale,
            ],
            self.uv,
        )
    }
}

#[derive(Clone, Debug)]
struct Shelf {
    x: u32,
    y: u32,
    height: u32,
}

#[derive(Clone, Debug)]
pub(super) struct RasterPage {
    pub dimension: u32,
    shelves: Vec<Shelf>,
    bottom: u32,
}

impl RasterPage {
    pub fn new(limit: u32) -> Self {
        Self {
            dimension: limit.clamp(1, 2048),
            shelves: Vec::new(),
            bottom: 0,
        }
    }

    /// Reserve a transparent texel on all sides, including between adjacent shelves.
    pub fn allocate(&mut self, width: u32, height: u32) -> Option<[u32; 2]> {
        let width = width.checked_add(2)?;
        let height = height.checked_add(2)?;
        if width > self.dimension || height > self.dimension {
            return None;
        }
        if let Some(shelf) = self
            .shelves
            .iter_mut()
            .filter(|s| s.height >= height && s.x + width <= self.dimension)
            .min_by_key(|s| s.height)
        {
            let origin = [shelf.x, shelf.y];
            shelf.x += width;
            return Some(origin);
        }
        if self.bottom + height > self.dimension {
            return None;
        }
        let origin = [0, self.bottom];
        self.shelves.push(Shelf {
            x: width,
            y: self.bottom,
            height,
        });
        self.bottom += height;
        Some(origin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_keys_follow_physical_height_not_fractional_position_or_layout_width() {
        let mut glyph = AtlasGlyph {
            ch: 'i',
            rect: [10.25, 20.125, 4.375, 17.5],
            color: [1.0; 4],
            clip_rect: None,
        };
        let key = RasterKey::for_glyph(&glyph).unwrap();
        assert_eq!(key.height, 18);
        glyph.rect[0] += 0.5;
        glyph.rect[2] += 0.125;
        assert_eq!(RasterKey::for_glyph(&glyph), Some(key));
        glyph.rect[3] = 23.1;
        assert_ne!(RasterKey::for_glyph(&glyph), Some(key));
        for height in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            glyph.rect[3] = height;
            assert_eq!(RasterKey::for_glyph(&glyph), None);
        }
    }

    #[test]
    fn native_ink_has_integral_origin_and_is_not_resampled_or_stretched() {
        let raster = CachedGlyph {
            uv: [0.1, 0.2, 0.3, 0.4],
            size: [3, 12],
            offset: [1.0, 2.6],
        };
        let glyph = AtlasGlyph {
            ch: 'i',
            rect: [10.25, 20.125, 4.375, 17.5],
            color: [1.0; 4],
            clip_rect: None,
        };
        assert_eq!(raster.quad(&glyph), ([11.0, 23.0, 3.0, 12.0], raster.uv));
    }

    #[test]
    fn mixed_size_glyphs_pack_without_overlap_and_respect_texture_limits() {
        for limit in [32, 256, 2048, 8192] {
            let mut page = RasterPage::new(limit);
            assert!(page.dimension <= limit);
            let mut occupied = Vec::new();
            for index in 0..2000 {
                let (w, h) = (3 + index % 19, 6 + index % 31);
                if let Some([x, y]) = page.allocate(w, h) {
                    assert!(x + w + 2 <= page.dimension && y + h + 2 <= page.dimension);
                    assert!(occupied.iter().all(|&(a, b, c, d)| x + w + 2 <= a
                        || a + c <= x
                        || y + h + 2 <= b
                        || b + d <= y));
                    occupied.push((x, y, w + 2, h + 2));
                }
            }
            assert!(!occupied.is_empty());
            assert_eq!(page.allocate(u32::MAX, 1), None);
        }
    }
}
