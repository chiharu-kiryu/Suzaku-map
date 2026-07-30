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
    let force_ellipsis = lines.len() > block.max_lines;
    if force_ellipsis {
        lines.truncate(block.max_lines);
    }
    let mut truncated = force_ellipsis;
    let last_line_index = lines.len().saturating_sub(1);

    for (idx, line) in lines.iter_mut().enumerate() {
        let add_suffix = force_ellipsis && idx == last_line_index;
        let (fitted, changed) =
            fit_text_to_width(line, block.max_width, block.pixel_size, glyph_advance, add_suffix);
        *line = fitted;
        truncated |= changed;
    }

    let mut quads = Vec::new();
    let mut atlas_glyphs = Vec::new();
    let mut max_line_width: f32 = 0.0;
    let max_content_width = block.max_width.max(0.0);

    for (line_index, line) in lines.iter().enumerate() {
        let line_width = line.chars().fold(0.0_f32, |acc, ch| {
            acc + glyph_advance_width(ch, block.pixel_size, glyph_advance)
        });
        max_line_width = max_line_width.max(line_width.min(max_content_width));
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

fn fit_text_to_width(
    text: &str,
    max_width: f32,
    pixel_size: f32,
    glyph_advance: f32,
    force_suffix: bool,
) -> (String, bool) {
    if max_width <= 0.0 {
        return (String::new(), !text.is_empty());
    }

    let mut cursor = String::new();
    let mut width = 0.0_f32;

    for ch in text.chars() {
        let char_width = glyph_advance_width(ch, pixel_size, glyph_advance);
        if width + char_width > max_width {
            return fit_text_with_suffix(cursor, max_width, pixel_size, glyph_advance);
        }
        cursor.push(ch);
        width += char_width;
    }

    if !force_suffix {
        return (cursor, false);
    }

    fit_text_with_suffix(cursor, max_width, pixel_size, glyph_advance)
}

fn fit_text_with_suffix(
    text: String,
    max_width: f32,
    pixel_size: f32,
    glyph_advance: f32,
) -> (String, bool) {
    if text.is_empty() {
        return ("…".to_string(), true);
    }

    let suffix_width = glyph_advance_width('…', pixel_size, glyph_advance);
    if suffix_width > max_width {
        return ("…".to_string(), true);
    }

    let mut trimmed = text;
    while estimate_line_width(&trimmed, pixel_size, glyph_advance) + suffix_width > max_width {
        trimmed.pop();
        while trimmed.ends_with(' ') {
            trimmed.pop();
        }
        if trimmed.is_empty() {
            return ("…".to_string(), true);
        }
    }

    trimmed.push('…');
    (trimmed, true)
}

fn estimate_line_width(text: &str, pixel_size: f32, glyph_advance: f32) -> f32 {
    text.chars()
        .fold(0.0_f32, |acc, ch| acc + glyph_advance_width(ch, pixel_size, glyph_advance))
}

fn glyph_advance_width(ch: char, pixel_size: f32, glyph_advance: f32) -> f32 {
    if ch == ' ' {
        pixel_size * 4.0
    } else {
        glyph_advance
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

#[cfg(test)]
mod tests {
    use crate::panel_support::{
        derive_next_token_candidates, derive_sentence_candidates_with_indices,
    };
    use super::TextAlign;
    use super::{TextBlock};
    use crate::ime::gpu::TextRole;

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

    #[test]
    fn layout_truncates_narrow_controls_with_ellipsis() {
        let block = TextBlock {
            text: "very long control".to_string(),
            origin: [0.0, 0.0],
            max_width: 12.0,
            pixel_size: 2.0,
            letter_spacing: 0.0,
            line_gap: 1.0,
            max_lines: 1,
            color: [1.0, 1.0, 1.0, 1.0],
            align: TextAlign::Center,
            role: TextRole::ToolButton,
        };

        let layout = block.layout();

        assert!(layout.truncated);
        assert_eq!(layout.lines.len(), 1);
        assert!(layout.lines[0].ends_with('…'));
        assert!(layout.lines[0].chars().count() < "very long control".chars().count());
        assert!(layout.bounds[2] <= block.max_width + 0.001);
    }

    #[test]
    fn emoji_sentence_candidate_layout_keeps_emoji() {
        let raw_candidates = vec![
            "hello there 😀 can continue by tapping the next suggestion".into(),
            "hello there 😀 is ready as the next full sentence".into(),
        ];
        let seed = "hello there 😀";

        let sentence = derive_sentence_candidates_with_indices(seed, &raw_candidates, 4)
            .into_iter()
            .find(|(_, candidate)| candidate.contains('😀'))
            .map(|(_, candidate)| candidate)
            .expect("emoji sentence candidate should be present");

        let block = TextBlock {
            text: sentence,
            origin: [0.0, 0.0],
            max_width: 1600.0,
            pixel_size: 12.0,
            letter_spacing: 0.0,
            line_gap: 2.0,
            max_lines: 2,
            color: [1.0, 1.0, 1.0, 1.0],
            align: TextAlign::Left,
            role: TextRole::InputValue,
        };

        let layout = block.layout();

        assert!(layout.atlas_glyphs.iter().any(|glyph| glyph.ch == '😀'));
        assert!(layout.atlas_glyphs.iter().any(|glyph| glyph.ch == 'H'));
    }

    #[test]
    fn emoji_next_token_to_sentence_to_layout_chain() {
        let base_candidates = vec![
            "hello there 😀 can continue with confidence".into(),
            "hello there 😀 can now explore".into(),
            "hello there 😀 is ready as the next full sentence".into(),
        ];

        let seed = "hello there 😀";
        let next_tokens = derive_next_token_candidates(seed, &base_candidates, 6);
        let can_token = next_tokens
            .into_iter()
            .find(|token| token == "can")
            .expect("next-token chain should include can for emoji seed");

        let composed_seed = format!("{seed} {can_token}");
        let sentence = derive_sentence_candidates_with_indices(
            &composed_seed,
            &base_candidates,
            4,
        )
        .first()
        .map(|(_, sentence)| sentence.clone())
        .expect("sentence derivation should return a completed candidate");

        let block = TextBlock {
            text: sentence,
            origin: [0.0, 0.0],
            max_width: 1600.0,
            pixel_size: 12.0,
            letter_spacing: 0.0,
            line_gap: 2.0,
            max_lines: 2,
            color: [1.0, 1.0, 1.0, 1.0],
            align: TextAlign::Left,
            role: TextRole::InputValue,
        };

        let layout = block.layout();

        assert!(layout.atlas_glyphs.iter().any(|glyph| glyph.ch == '😀'));
        assert!(layout.atlas_glyphs.iter().any(|glyph| glyph.ch == 'H'));
    }

    #[test]
    fn multi_codepoint_emoji_preserves_layout_input_chars() {
        let block = TextBlock {
            text: "hello 👨‍👩‍👧‍👦".to_string(),
            origin: [0.0, 0.0],
            max_width: 1600.0,
            pixel_size: 12.0,
            letter_spacing: 0.0,
            line_gap: 2.0,
            max_lines: 2,
            color: [1.0, 1.0, 1.0, 1.0],
            align: TextAlign::Left,
            role: TextRole::InputValue,
        };

        let layout = block.layout();

        assert!(layout.atlas_glyphs.iter().any(|glyph| glyph.ch == '👨'));
        assert!(layout.atlas_glyphs.iter().any(|glyph| glyph.ch == '👩'));
        assert!(layout.atlas_glyphs.iter().any(|glyph| glyph.ch == '👧'));
        assert!(layout.atlas_glyphs.iter().any(|glyph| glyph.ch == '👦'));
    }
}
