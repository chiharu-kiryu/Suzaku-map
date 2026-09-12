use super::{
    AtlasGlyph, CandidateQuad, TextAlign, TextBlock, TextLayout, glyph_bitmap, text_char_width,
    text_glyph_advance, text_space_advance,
};

pub(super) fn point_in_rect(x: f32, y: f32, rect: [f32; 4]) -> bool {
    let [rx, ry, rw, rh] = rect;
    x >= rx && x <= rx + rw && y >= ry && y <= ry + rh
}

pub(super) fn intersect_rect(rect: [f32; 4], clip: [f32; 4]) -> [f32; 4] {
    let left = rect[0].max(clip[0]).min(clip[0] + clip[2]);
    let top = rect[1].max(clip[1]).min(clip[1] + clip[3]);
    let right = (rect[0] + rect[2]).min(clip[0] + clip[2]);
    let bottom = (rect[1] + rect[3]).min(clip[1] + clip[3]);
    [left, top, (right - left).max(0.0), (bottom - top).max(0.0)]
}

pub(super) fn centered_icon_rect(rect: [f32; 4]) -> [f32; 4] {
    let size = rect[2].min(rect[3]).max(0.0);
    [
        rect[0] + (rect[2] - size) * 0.5,
        rect[1] + (rect[3] - size) * 0.5,
        size,
        size,
    ]
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
    let glyph_advance = text_glyph_advance(block.pixel_size, block.letter_spacing);
    let mut lines = wrap_text(
        &block.text,
        block.max_width,
        block.pixel_size,
        glyph_advance,
    );
    let force_ellipsis = lines.len() > block.max_lines;
    if force_ellipsis {
        lines.truncate(block.max_lines);
    }
    let mut truncated = force_ellipsis;
    let last_line_index = lines.len().saturating_sub(1);

    for (idx, line) in lines.iter_mut().enumerate() {
        let add_suffix = force_ellipsis && idx == last_line_index;
        let (fitted, changed) = fit_text_to_width(
            line,
            block.max_width,
            block.pixel_size,
            glyph_advance,
            add_suffix,
        );
        *line = fitted;
        truncated |= changed;
    }

    layout_lines(block, lines, truncated, None)
}

fn layout_lines(
    block: &TextBlock,
    lines: Vec<String>,
    truncated: bool,
    viewport: Option<[f32; 4]>,
) -> TextLayout {
    let glyph_advance = text_glyph_advance(block.pixel_size, block.letter_spacing);
    let line_height = block.pixel_size * 7.0 + block.line_gap;
    let mut quads = Vec::new();
    let mut atlas_glyphs = Vec::new();
    let mut max_line_width: f32 = 0.0;
    let max_content_width = block.max_width.max(0.0);

    for (line_index, line) in lines.iter().enumerate() {
        let advance_width = line.chars().fold(0.0_f32, |acc, ch| {
            acc + glyph_advance_width(ch, block.pixel_size, glyph_advance)
        });
        // The final glyph has no trailing letter spacing. Center the visible glyph boxes.
        let line_width = if let Some(ch) = line.chars().last().filter(|ch| *ch != ' ') {
            advance_width - glyph_advance_width(ch, block.pixel_size, glyph_advance)
                + text_char_width(ch, block.pixel_size)
        } else {
            advance_width
        };
        max_line_width = max_line_width.max(line_width.min(max_content_width));
        let offset_x = match block.align {
            TextAlign::Left => 0.0,
            TextAlign::Center => ((block.max_width - line_width) / 2.0).max(0.0),
        };
        let mut cursor_x = block.origin[0] + offset_x;
        let cursor_y = block.origin[1] + line_index as f32 * line_height;

        for ch in line.chars() {
            if ch == ' ' {
                cursor_x += text_space_advance(block.pixel_size);
                continue;
            }

            let glyph_rect = [
                cursor_x,
                cursor_y,
                text_char_width(ch, block.pixel_size),
                block.pixel_size * 7.0,
            ];
            if viewport.is_some_and(|viewport| {
                let visible = intersect_rect(glyph_rect, viewport);
                visible[2] <= 0.0 || visible[3] <= 0.0
            }) {
                // A long editable draft must not allocate bitmap/atlas geometry for
                // the entire off-screen prefix on every key press.
                cursor_x += glyph_advance_width(ch, block.pixel_size, glyph_advance);
                continue;
            }
            atlas_glyphs.push(AtlasGlyph {
                ch,
                rect: glyph_rect,
                color: block.color,
                clip_rect: None,
            });

            for (row, pattern) in glyph_bitmap(ch).iter().enumerate() {
                for col in 0..5 {
                    if (pattern >> (4 - col)) & 1 == 1 {
                        quads.push(CandidateQuad {
                            shape: Default::default(),
                            clip_rect: None,
                            rect: [
                                cursor_x + col as f32 * block.pixel_size * 0.88,
                                cursor_y + row as f32 * block.pixel_size,
                                block.pixel_size * 0.88,
                                block.pixel_size,
                            ],
                            color: block.color,
                        });
                    }
                }
            }

            cursor_x += glyph_advance_width(ch, block.pixel_size, glyph_advance);
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
        bounds: [
            block.origin[0]
                + if block.align == TextAlign::Center {
                    ((block.max_width - max_line_width) * 0.5).max(0.0)
                } else {
                    0.0
                },
            block.origin[1],
            max_line_width,
            height,
        ],
        role: block.role,
    }
}

/// Editors must not use label wrapping: it collapses spaces and loses the caret's character map.
pub(super) fn layout_input_line(
    block: &TextBlock,
    viewport: [f32; 4],
    caret_index: usize,
    caret_width: f32,
) -> (TextLayout, [f32; 4]) {
    let mut block = block.clone();
    block.align = TextAlign::Left;
    block.pixel_size = block.pixel_size.min(viewport[3].max(0.0) / 7.0).max(0.0);
    block.max_width = viewport[2].max(0.0);
    block.origin = [
        viewport[0],
        viewport[1] + (viewport[3] - block.pixel_size * 7.0).max(0.0) * 0.5,
    ];
    // A single-line view of control characters, without changing the underlying draft or indices.
    block.text = block
        .text
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect();
    let caret_width = caret_width.max(0.0).min(block.max_width);
    let caret_advance = super::measure_text_prefix_width(
        &block.text,
        caret_index,
        block.pixel_size,
        block.letter_spacing,
    );
    let scroll = (caret_advance - (block.max_width - caret_width)).max(0.0);
    block.origin[0] -= scroll;
    let caret = [
        (block.origin[0] + caret_advance)
            .clamp(viewport[0], viewport[0] + block.max_width - caret_width),
        block.origin[1],
        caret_width,
        block.pixel_size * 7.0,
    ];
    let mut layout = layout_lines(&block, vec![block.text.clone()], false, Some(viewport));
    // Bounds describe the visible row, not the off-screen prefix used to place the caret.
    layout.bounds = [
        viewport[0],
        block.origin[1],
        block.max_width,
        block.pixel_size * 7.0,
    ];
    layout.clip_to_rect(viewport);
    layout
        .quads
        .retain(|quad| quad.rect[2] > 0.0 && quad.rect[3] > 0.0);
    layout.atlas_glyphs.retain(|glyph| {
        let visible = intersect_rect(glyph.rect, viewport);
        visible[2] > 0.0 && visible[3] > 0.0
    });
    (layout, caret)
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
    text.chars().fold(0.0_f32, |acc, ch| {
        acc + glyph_advance_width(ch, pixel_size, glyph_advance)
    })
}

fn glyph_advance_width(ch: char, pixel_size: f32, glyph_advance: f32) -> f32 {
    if ch == ' ' {
        text_space_advance(pixel_size)
    } else {
        glyph_advance + (text_char_width(ch, pixel_size) - pixel_size * 4.4)
    }
}

fn wrap_text(text: &str, max_width: f32, pixel_size: f32, glyph_advance: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0.0_f32;
    let space_width = text_space_advance(pixel_size);

    let push_word =
        |word: &str, lines: &mut Vec<String>, current: &mut String, current_width: &mut f32| {
            for ch in word.chars() {
                let char_width = glyph_advance_width(ch, pixel_size, glyph_advance);
                if !current.is_empty() && *current_width + char_width > max_width {
                    lines.push(std::mem::take(current));
                    *current_width = 0.0;
                }
                current.push(ch);
                *current_width += char_width;
            }
        };

    for word in text.split_whitespace() {
        let word_width = estimate_line_width(word, pixel_size, glyph_advance);

        if current.is_empty() {
            push_word(word, &mut lines, &mut current, &mut current_width);
        } else if current_width + space_width + word_width <= max_width {
            current.push(' ');
            current_width += space_width;
            push_word(word, &mut lines, &mut current, &mut current_width);
        } else {
            lines.push(std::mem::take(&mut current));
            current_width = 0.0;
            push_word(word, &mut lines, &mut current, &mut current_width);
        }
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
    use super::{TextAlign, TextBlock, text_glyph_advance, text_space_advance, wrap_text};
    use crate::ime::gpu::TextRole;
    use crate::panel_support::{
        derive_next_token_candidates, derive_sentence_candidates_with_indices,
    };

    #[test]
    fn input_line_preserves_spaces_and_tracks_caret_at_both_ends() {
        use crate::ime::gpu::{FontLayoutMetrics, with_font_metrics};
        use crate::ime::measure_text_prefix_width;
        use std::{collections::HashMap, sync::Arc};
        let metrics: FontLayoutMetrics = Arc::new(HashMap::from([('W', 5.5), ('i', 1.5)]));
        with_font_metrics(metrics, || {
            let block = TextBlock {
                text: "  Wi  你好   ".repeat(100),
                origin: [0.0; 2],
                max_width: 100.0,
                pixel_size: 3.0,
                letter_spacing: 0.0,
                line_gap: 0.0,
                max_lines: 1,
                color: [1.0; 4],
                align: TextAlign::Left,
                role: TextRole::InputValue,
            };
            let viewport = [10.0, 20.0, 90.0, 21.0];
            for index in [0, 2, 4, block.text.chars().count()] {
                let (layout, caret) = super::layout_input_line(&block, viewport, index, 2.0);
                assert_eq!(layout.lines, [block.text.clone()]);
                assert!(!layout.truncated);
                let advance = measure_text_prefix_width(&block.text, index, 3.0, 0.0);
                assert!((caret[0] - (10.0 + advance.min(88.0))).abs() < 0.001);
                assert_eq!(caret[1], 20.0);
                assert!(
                    layout.atlas_glyphs.len() < 12,
                    "off-screen glyphs must not fill the atlas"
                );
                assert!(
                    layout.atlas_glyphs.capacity() < 128 && layout.quads.capacity() <= 1024,
                    "allocate only visible input geometry, not the full draft then discard it"
                );
                if index == 0 {
                    let first = &layout.atlas_glyphs[0];
                    assert_eq!(first.ch, 'W');
                    assert!((first.rect[0] - 10.0 - text_space_advance(3.0) * 2.0).abs() < 0.001);
                }
            }
        });
    }

    #[test]
    fn wrapping_accounts_for_narrow_spaces() {
        let pixel_size = 4.0;
        let glyph_advance = text_glyph_advance(pixel_size, 0.0);
        let text = "Apple can continue with the next suggestion.";
        let actual_width = text
            .chars()
            .map(|ch| {
                if ch == ' ' {
                    text_space_advance(pixel_size)
                } else {
                    glyph_advance
                }
            })
            .sum::<f32>();

        assert_eq!(
            wrap_text(text, actual_width + 0.01, pixel_size, glyph_advance),
            vec![text]
        );
    }

    #[test]
    fn cjk_glyphs_use_square_boxes_and_matching_caret_and_wrap_widths() {
        use crate::ime::{measure_text_prefix_width, text_char_advance};
        for text in ["你好日本語", "こんにちは、世界", "hello 你好"] {
            let pixel_size = 3.0;
            let block = TextBlock {
                text: text.into(),
                origin: [0.0, 0.0],
                max_width: 400.0,
                pixel_size,
                letter_spacing: 0.5,
                line_gap: 0.0,
                max_lines: 1,
                color: [1.0; 4],
                align: TextAlign::Left,
                role: TextRole::CandidatePrimary,
            };
            let layout = block.layout();
            for glyph in &layout.atlas_glyphs {
                if unicode_width::UnicodeWidthChar::width(glyph.ch) == Some(2) {
                    assert_eq!(
                        glyph.rect[2], glyph.rect[3],
                        "CJK must not be squeezed into a Latin cell"
                    );
                }
            }
            let last = layout.atlas_glyphs.last().unwrap();
            let caret = measure_text_prefix_width(text, text.chars().count(), pixel_size, 0.5);
            assert!(
                (caret - last.rect[0] - text_char_advance(last.ch, pixel_size, 0.5)).abs() < 0.001
            );
        }
        let advance = text_glyph_advance(3.0, 0.0);
        assert_eq!(
            wrap_text(
                "你好世界",
                2.0 * text_char_advance('你', 3.0, 0.0) + 0.01,
                3.0,
                advance
            ),
            ["你好", "世界"]
        );
    }

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

        assert!(layout.atlas_glyphs.iter().any(|glyph| glyph.ch == '😀'));
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
        let sentence = derive_sentence_candidates_with_indices(&composed_seed, &base_candidates, 4)
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
