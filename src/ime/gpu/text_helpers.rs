use super::{AtlasGlyph, CandidateQuad, TextAlign, TextBlock, TextLayout, glyph_bitmap};

pub(super) fn point_in_rect(x: f32, y: f32, rect: [f32; 4]) -> bool {
    let [rx, ry, rw, rh] = rect;
    x >= rx && x <= rx + rw && y >= ry && y <= ry + rh
}

pub(super) fn sample_stroke_points(stroke: &[[f32; 2]]) -> Vec<[f32; 2]> {
    if stroke.is_empty() {
        return Vec::new();
    }
    if stroke.len() == 1 {
        return vec![stroke[0]];
    }

    let mut points = Vec::with_capacity(stroke.len() * 4);
    for window in stroke.windows(2) {
        let start = window[0];
        let end = window[1];
        let dx = end[0] - start[0];
        let dy = end[1] - start[1];
        let distance = dx.hypot(dy).max(1.0);
        let steps = (distance / 4.0).ceil() as usize;
        for step in 0..=steps {
            let t = step as f32 / steps.max(1) as f32;
            points.push([start[0] + dx * t, start[1] + dy * t]);
        }
    }
    points
}

pub(super) fn layout_text_block(block: &TextBlock) -> TextLayout {
    let glyph_advance = (block.pixel_size * 6.5 + block.letter_spacing).max(block.pixel_size * 4.0);
    let line_height = block.pixel_size * 7.0 + block.line_gap;
    let max_chars_per_line = ((block.max_width / glyph_advance).floor() as usize).max(1);
    let mut lines = wrap_text(&block.text, max_chars_per_line);
    let mut truncated = false;

    if lines.len() > block.max_lines {
        lines.truncate(block.max_lines);
        if let Some(last) = lines.last_mut() {
            *last = ellipsize(last, max_chars_per_line, true);
        }
        truncated = true;
    } else if lines
        .last()
        .map(|line| line.chars().count() > max_chars_per_line)
        .unwrap_or(false)
    {
        if let Some(last) = lines.last_mut() {
            *last = ellipsize(last, max_chars_per_line, false);
        }
        truncated = true;
    }

    let mut quads = Vec::new();
    let mut atlas_glyphs = Vec::new();
    let mut max_line_width: f32 = 0.0;

    for (line_index, line) in lines.iter().enumerate() {
        let line_width = line.chars().fold(0.0_f32, |acc, ch| {
            acc + if ch == ' ' {
                block.pixel_size * 4.0
            } else {
                glyph_advance
            }
        });
        max_line_width = max_line_width.max(line_width);
        let offset_x = match block.align {
            TextAlign::Left => 0.0,
            TextAlign::Center => ((block.max_width - line_width) / 2.0).max(0.0),
        };
        let mut cursor_x = block.origin[0] + offset_x;
        let cursor_y = block.origin[1] + line_index as f32 * line_height;

        for ch in line.chars() {
            if ch == ' ' {
                cursor_x += block.pixel_size * 4.0;
                continue;
            }

            atlas_glyphs.push(AtlasGlyph {
                ch,
                rect: [
                    cursor_x,
                    cursor_y,
                    block.pixel_size * 5.0,
                    block.pixel_size * 7.0,
                ],
                color: block.color,
            });

            for (row, pattern) in glyph_bitmap(ch).iter().enumerate() {
                for col in 0..5 {
                    if (pattern >> (4 - col)) & 1 == 1 {
                        quads.push(CandidateQuad {
                            rect: [
                                cursor_x + col as f32 * block.pixel_size,
                                cursor_y + row as f32 * block.pixel_size,
                                block.pixel_size,
                                block.pixel_size,
                            ],
                            color: block.color,
                        });
                    }
                }
            }

            cursor_x += glyph_advance;
        }
    }

    let height = if lines.is_empty() {
        0.0
    } else {
        (lines.len() as f32 - 1.0) * line_height + block.pixel_size * 7.0
    };

    TextLayout {
        quads,
        atlas_glyphs,
        lines,
        truncated,
        bounds: [block.origin[0], block.origin[1], max_line_width, height],
        role: block.role,
    }
}

pub(super) fn wrap_text(text: &str, max_chars_per_line: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        let pending_len = if current.is_empty() {
            word.chars().count()
        } else {
            current.chars().count() + 1 + word.chars().count()
        };

        if pending_len <= max_chars_per_line {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
            continue;
        }

        if !current.is_empty() {
            lines.push(current.clone());
            current.clear();
        }

        if word.chars().count() <= max_chars_per_line {
            current.push_str(word);
            continue;
        }

        let mut chunk = String::new();
        for ch in word.chars() {
            if chunk.chars().count() >= max_chars_per_line {
                lines.push(chunk.clone());
                chunk.clear();
            }
            chunk.push(ch);
        }
        current = chunk;
    }

    if !current.is_empty() {
        lines.push(current);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

pub(super) fn ellipsize(text: &str, max_chars: usize, force_suffix: bool) -> String {
    if text.chars().count() <= max_chars && !force_suffix {
        return text.to_string();
    }

    if max_chars <= 1 {
        return "…".to_string();
    }

    let keep = if force_suffix {
        max_chars.saturating_sub(1).min(text.chars().count())
    } else {
        max_chars.saturating_sub(1)
    };
    let mut result = text.chars().take(keep).collect::<String>();
    if result.ends_with(' ') {
        result.pop();
    }
    result.push('…');
    result
}

#[cfg(test)]
mod tests {
    use super::TextAlign;
    use super::{TextBlock, TextRole};

    #[test]
    fn layout_preserves_emoji_in_atlas_glyphs() {
        let block = TextBlock {
            text: "hello 😀 world".to_string(),
            origin: [0.0, 0.0],
            max_width: 320.0,
            pixel_size: 12.0,
            letter_spacing: 0.0,
            line_gap: 2.0,
            max_lines: 2,
            color: [1.0, 1.0, 1.0, 1.0],
            align: TextAlign::Left,
            role: TextRole::InputValue,
        };

        let layout = block.layout();

        assert!(layout
            .atlas_glyphs
            .iter()
            .any(|glyph| glyph.ch == '😀'));
    }
}
