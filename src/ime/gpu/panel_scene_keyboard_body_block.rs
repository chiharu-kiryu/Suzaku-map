{
                    let keyboard_scale =
                        (drawer_rect[3] / (150.0 * responsive_scale)).clamp(0.74, 1.0);
                    let keyboard_padding_x = 7.6 * responsive_scale * keyboard_scale;
                    let keyboard_content_left = drawer_rect[0] + keyboard_padding_x;
                    let keyboard_content_right = drawer_rect[0] + drawer_rect[2] - keyboard_padding_x;
                    let keyboard_content_width = (keyboard_content_right - keyboard_content_left).max(120.0);
                    let keyboard_title_y = drawer_rect[1] + 10.0 * responsive_scale * keyboard_scale;
                    let keyboard_status_y = keyboard_title_y;
                    let row_h = 21.0 * responsive_scale * keyboard_scale;
                    let keyboard_y = drawer_rect[1] + 37.0 * responsive_scale * keyboard_scale;
                    let key_row_gap = 3.2 * responsive_scale * keyboard_scale;
                    let mut keyboard_layouts = Vec::new();
                    let keyboard_header_layouts = vec![
                        TextBlock {
                            text: "Keyboard input".to_string(),
                            origin: [
                                drawer_rect[0] + 12.0 * responsive_scale * keyboard_scale,
                                keyboard_title_y,
                            ],
                            max_width: (drawer_rect[2] - 142.0 * responsive_scale * keyboard_scale)
                                .max(110.0 * keyboard_scale),
                            pixel_size: title_px * keyboard_scale,
                            letter_spacing: heading_tracking,
                            line_gap: base_line_gap * keyboard_scale,
                            max_lines: 1,
                            color: text_secondary,
                            align: TextAlign::Left,
                            role: TextRole::KeyboardKey,
                        }
                        .layout(),
                        TextBlock {
                            text: "Tap to type".to_string(),
                            origin: [
                                drawer_rect[0] + drawer_rect[2]
                                    - (142.0 * responsive_scale * keyboard_scale),
                                keyboard_status_y,
                            ],
                            max_width: (136.0 * responsive_scale * keyboard_scale).max(76.0),
                            pixel_size: helper_px * keyboard_scale,
                            letter_spacing: ui_tracking * keyboard_scale,
                            line_gap: base_line_gap * keyboard_scale,
                            max_lines: 1,
                            color: text_primary,
                            align: TextAlign::Center,
                            role: TextRole::KeyboardKey,
                        }
                        .layout(),
                    ];
                    for layout in &keyboard_header_layouts {
                        text_quads.extend(layout.quads.iter().copied());
                        atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                    }
                    text_sections.push(TextSection {
                        role: TextRole::KeyboardKey,
                        layouts: keyboard_header_layouts,
                    });

                    let key_rows = if chrome.keyboard_numeric {
                        vec![
                            "1234567890"
                                .chars()
                                .map(VirtualKeyboardKey::Character)
                                .collect::<Vec<_>>(),
                            vec![
                                VirtualKeyboardKey::Character('-'),
                                VirtualKeyboardKey::Character('/'),
                                VirtualKeyboardKey::Character(':'),
                                VirtualKeyboardKey::Character(';'),
                                VirtualKeyboardKey::Character('('),
                                VirtualKeyboardKey::Character(')'),
                                VirtualKeyboardKey::Character('$'),
                                VirtualKeyboardKey::Character('&'),
                                VirtualKeyboardKey::Character('@'),
                            ],
                            vec![
                                VirtualKeyboardKey::ToggleAlphabetic,
                                VirtualKeyboardKey::Character('.'),
                                VirtualKeyboardKey::Character(','),
                                VirtualKeyboardKey::Character('?'),
                                VirtualKeyboardKey::Character('!'),
                                VirtualKeyboardKey::Character('\''),
                                VirtualKeyboardKey::Backspace,
                            ],
                        ]
                    } else {
                        vec![
                            "qwertyuiop"
                                .chars()
                                .map(|ch| {
                                    let resolved = if chrome.keyboard_shifted {
                                        ch.to_ascii_uppercase()
                                    } else {
                                        ch
                                    };
                                    VirtualKeyboardKey::Character(resolved)
                                })
                                .collect::<Vec<_>>(),
                            "asdfghjkl"
                                .chars()
                                .map(|ch| {
                                    let resolved = if chrome.keyboard_shifted {
                                        ch.to_ascii_uppercase()
                                    } else {
                                        ch
                                    };
                                    VirtualKeyboardKey::Character(resolved)
                                })
                                .collect::<Vec<_>>(),
                            {
                                let mut row = vec![VirtualKeyboardKey::Shift];
                                row.extend("zxcvbnm".chars().map(|ch| {
                                    let resolved = if chrome.keyboard_shifted {
                                        ch.to_ascii_uppercase()
                                    } else {
                                        ch
                                    };
                                    VirtualKeyboardKey::Character(resolved)
                                }));
                                row.push(VirtualKeyboardKey::Backspace);
                                row
                            },
                        ]
                    };

                    for (row_index, keys) in key_rows.iter().enumerate() {
                        let key_count = keys.len() as f32;
                        let inset = if chrome.keyboard_numeric {
                            if row_index == 1 {
                                12.0 * responsive_scale * keyboard_scale
                            } else {
                                0.0
                            }
                        } else if row_index == 1 {
                            18.0 * responsive_scale * keyboard_scale
                        } else if row_index == 2 {
                            8.0 * responsive_scale * keyboard_scale
                        } else {
                            0.0
                        };

                        let row_width =
                            (keyboard_content_width - inset * 2.0).max(20.0 * responsive_scale);
                        let requested_gap = (row_width / (key_count + 1.0)).clamp(
                            1.8 * responsive_scale * keyboard_scale,
                            6.5 * responsive_scale * keyboard_scale,
                        );
                        let key_count_minus_1 = (key_count - 1.0).max(1.0);
                        let mut key_gap = requested_gap;
                        let mut key_w =
                            (row_width - key_gap * key_count_minus_1) / key_count.max(1.0);
                    let min_key_w = 16.0 * responsive_scale * keyboard_scale;
                    let min_gap = 2.0 * responsive_scale * keyboard_scale;

                        if key_w < min_key_w {
                            key_w = min_key_w;
                            key_gap =
                                (row_width - key_count * key_w).max(0.0) / key_count_minus_1.max(1.0);
                        }
                        if key_gap < min_gap {
                            key_w = (row_width / key_count.max(1.0)).max(8.0 * responsive_scale);
                            key_gap = 0.0;
                        }
                        let content_w = key_w * key_count + key_gap * key_count_minus_1;
                        let row_extra = (row_width - content_w) * 0.5;
                        let row_x = keyboard_content_left + inset + row_extra.max(0.0);
                        let row_y = keyboard_y + row_index as f32 * (row_h + key_row_gap);

                        for (key_index, key) in keys.iter().enumerate() {
                            let x = row_x + key_index as f32 * (key_w + key_gap);
                            let rect = [x, row_y, key_w, row_h];
                            append_soft_card_quads(
                                &mut quads,
                                rect,
                                match key {
                                    VirtualKeyboardKey::Shift if chrome.keyboard_shifted => {
                                        accent_soft
                                    }
                                    VirtualKeyboardKey::ToggleNumeric
                                    | VirtualKeyboardKey::ToggleAlphabetic
                                    | VirtualKeyboardKey::Backspace
                                    | VirtualKeyboardKey::Shift => keyboard_special_surface,
                                    _ => keyboard_surface,
                                },
                                if matches!(key, VirtualKeyboardKey::Shift)
                                    && chrome.keyboard_shifted
                                {
                                    accent
                                } else {
                                    border_dark
                                },
                                soft_shadow,
                                surface,
                                7.0 * responsive_scale * keyboard_scale,
                            );
                            interactive_targets.push(InteractiveTarget {
                                kind: InteractionKind::VirtualKeyboardKey(*key),
                                rect: interaction_hit_rect(rect),
                            });
                            let label = match key {
                                VirtualKeyboardKey::Character(ch) => ch.to_string(),
                                VirtualKeyboardKey::Text(text) => (*text).to_string(),
                                VirtualKeyboardKey::Space => "Space".to_string(),
                                VirtualKeyboardKey::Backspace => "".to_string(),
                                VirtualKeyboardKey::Shift => "Shift".to_string(),
                                VirtualKeyboardKey::ToggleNumeric => "123".to_string(),
                                VirtualKeyboardKey::ToggleAlphabetic => "ABC".to_string(),
                            };
                            let layout = TextBlock {
                                text: label,
                                origin: [
                                    x + 8.0 * responsive_scale * keyboard_scale,
                                    row_y + 7.4 * responsive_scale * keyboard_scale,
                                ],
                            max_width: (key_w - 16.0 * responsive_scale * keyboard_scale).max(0.0),
                                pixel_size: if matches!(
                                    key,
                                    VirtualKeyboardKey::Backspace
                                        | VirtualKeyboardKey::ToggleNumeric
                                        | VirtualKeyboardKey::ToggleAlphabetic
                                        | VirtualKeyboardKey::Shift
                                ) {
                                    (chip_px * 0.76 * keyboard_scale).max(1.8 * responsive_scale)
                                } else {
                                    (chip_px * 1.1 * keyboard_scale)
                                        .max(2.05 * responsive_scale * keyboard_scale)
                                },
                                letter_spacing: ui_tracking * keyboard_scale,
                                line_gap: base_line_gap * keyboard_scale,
                                max_lines: 1,
                                color: if matches!(
                                    key,
                                    VirtualKeyboardKey::Backspace
                                        | VirtualKeyboardKey::ToggleNumeric
                                        | VirtualKeyboardKey::ToggleAlphabetic
                                        | VirtualKeyboardKey::Shift
                                ) {
                                    keyboard_secondary_text
                                } else {
                                    keyboard_text
                                },
                                align: TextAlign::Center,
                                role: TextRole::KeyboardKey,
                            }
                            .layout_in_rect(rect, [5.0 * responsive_scale * keyboard_scale, 2.0 * responsive_scale * keyboard_scale]);
                            text_quads.extend(layout.quads.iter().copied());
                            atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                            keyboard_layouts.push(layout);
                            if matches!(key, VirtualKeyboardKey::Backspace) {
                                append_backspace_icon_quads(
                                    &mut quads,
                                    centered_icon_rect(rect),
                                    keyboard_secondary_text,
                                );
                            }
                        }
                    }

                    let key_block_h = (key_rows.len() as f32).max(1.0) * row_h
                        + (key_rows.len().saturating_sub(1)) as f32 * key_row_gap;
                    let action_y = keyboard_y + key_block_h + 2.2 * responsive_scale * keyboard_scale;
                    let action_gap = 4.6 * responsive_scale * keyboard_scale;
                    let base_left_w = 64.0 * responsive_scale * keyboard_scale;
                    let base_mid_w = 33.0 * responsive_scale * keyboard_scale;
                    let base_right_w = 84.0 * responsive_scale * keyboard_scale;
                    let base_space_w = 74.0 * responsive_scale * keyboard_scale;
                    let min_left_w = 35.0 * responsive_scale * keyboard_scale;
                    let min_mid_w = 18.5 * responsive_scale * keyboard_scale;
                    let min_right_w = 53.0 * responsive_scale * keyboard_scale;
                    let min_space_w = 52.0 * responsive_scale * keyboard_scale;
                    let available_action_w = keyboard_content_width.max(0.0);
                    let total_min =
                        min_left_w + min_right_w + min_space_w + 2.0 * min_mid_w + action_gap * 4.0;
                    let mut left_w = base_left_w;
                    let mut mid_key_w = base_mid_w;
                    let mut right_w = base_right_w;
                    let mut space_w = base_space_w;
                    if total_min >= available_action_w {
                        let shrink = (available_action_w / total_min.max(1.0)).clamp(0.55, 1.0);
                        left_w = (left_w * shrink).max(min_left_w);
                        mid_key_w = (mid_key_w * shrink).max(min_mid_w);
                        right_w = (right_w * shrink).max(min_right_w);
                        space_w = (space_w * shrink).max(min_space_w);
                    } else {
                        let leftover = available_action_w - total_min;
                        let total_grow = (base_left_w - min_left_w)
                            + (base_mid_w - min_mid_w) * 2.0
                            + (base_right_w - min_right_w)
                            + (base_space_w - min_space_w);
                        if total_grow > 0.0 {
                            let grow_ratio = leftover / total_grow;
                            left_w = min_left_w + (base_left_w - min_left_w) * grow_ratio;
                            mid_key_w = min_mid_w + (base_mid_w - min_mid_w) * grow_ratio;
                            right_w = min_right_w + (base_right_w - min_right_w) * grow_ratio;
                            space_w = min_space_w + (base_space_w - min_space_w) * grow_ratio;
                        } else {
                            left_w = base_left_w;
                            mid_key_w = base_mid_w;
                            right_w = base_right_w;
                            space_w = base_space_w;
                        }
                    }

                    let mut cursor_x = keyboard_content_left;
                    let action_keys = if chrome.keyboard_numeric {
                        vec![
                            (
                                VirtualKeyboardKey::ToggleAlphabetic,
                                "ABC",
                                [
                                    cursor_x,
                                    action_y,
                                    left_w.max(min_left_w),
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Character(','),
                                ",",
                                [
                                    {
                                        cursor_x += left_w.max(min_left_w) + action_gap;
                                        cursor_x
                                    },
                                    action_y,
                                    mid_key_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Space,
                                "Space",
                                [
                                    {
                                        cursor_x += mid_key_w + action_gap;
                                        cursor_x
                                    },
                                    action_y,
                                    space_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Character('.'),
                                ".",
                                [
                                    {
                                        cursor_x += space_w + action_gap;
                                        cursor_x
                                    },
                                    action_y,
                                    mid_key_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Backspace,
                                "",
                                [
                                    {
                                        cursor_x += mid_key_w + action_gap;
                                        cursor_x
                                    },
                                    action_y,
                                    right_w,
                                    row_h,
                                ],
                            ),
                        ]
                    } else {
                        vec![
                            (
                                VirtualKeyboardKey::ToggleNumeric,
                                "123",
                                [
                                    cursor_x,
                                    action_y,
                                    left_w.max(min_left_w),
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Character(','),
                                ",",
                                [
                                    {
                                        cursor_x += left_w.max(min_left_w) + action_gap;
                                        cursor_x
                                    },
                                    action_y,
                                    mid_key_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Space,
                                "Space",
                                [
                                    {
                                        cursor_x += mid_key_w + action_gap;
                                        cursor_x
                                    },
                                    action_y,
                                    space_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Character('.'),
                                ".",
                                [
                                    {
                                        cursor_x += space_w + action_gap;
                                        cursor_x
                                    },
                                    action_y,
                                    mid_key_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Backspace,
                                "",
                                [
                                    {
                                        cursor_x += mid_key_w + action_gap;
                                        cursor_x
                                    },
                                    action_y,
                                    right_w,
                                    row_h,
                                ],
                            ),
                        ]
                    };

                    for (key, label, mut rect) in action_keys {
                        rect[2] = rect[2].max(if matches!(
                            key,
                            VirtualKeyboardKey::ToggleNumeric
                                | VirtualKeyboardKey::ToggleAlphabetic
                                | VirtualKeyboardKey::Backspace
                                | VirtualKeyboardKey::Character(',')
                                | VirtualKeyboardKey::Character('.')
                        ) {
                            min_mid_w
                        } else {
                            min_left_w
                        });
                        if rect[0] + rect[2] > keyboard_content_right {
                            rect[2] =
                                (drawer_rect[0] + drawer_rect[2] - keyboard_padding_x - rect[0])
                                    .max(min_mid_w);
                        }
                        append_soft_card_quads(
                            &mut quads,
                            rect,
                            match key {
                                VirtualKeyboardKey::Space => keyboard_surface,
                                VirtualKeyboardKey::ToggleNumeric
                                | VirtualKeyboardKey::ToggleAlphabetic
                                | VirtualKeyboardKey::Backspace => keyboard_special_surface,
                                _ => keyboard_surface,
                            },
                            border_dark,
                            soft_shadow,
                            surface,
                            8.0 * responsive_scale * keyboard_scale,
                        );
                        interactive_targets.push(InteractiveTarget {
                            kind: InteractionKind::VirtualKeyboardKey(key),
                            rect: interaction_hit_rect(rect),
                        });
                        let label_pixel_size = if label.chars().count() > 6 {
                            1.8 * responsive_scale * keyboard_scale
                        } else {
                            2.2 * responsive_scale * keyboard_scale
                        };
                        let layout = TextBlock {
                            text: label.to_string(),
                            origin: [
                                rect[0] + 10.0 * responsive_scale * keyboard_scale,
                                rect[1] + 7.4 * responsive_scale * keyboard_scale,
                            ],
                            max_width: (rect[2] - 20.0 * responsive_scale * keyboard_scale).max(0.0),
                            pixel_size: label_pixel_size,
                            letter_spacing: ui_tracking * keyboard_scale,
                            line_gap: base_line_gap * keyboard_scale,
                            max_lines: if label.chars().count() > 6 { 2 } else { 1 },
                            color: if matches!(
                                key,
                                VirtualKeyboardKey::ToggleNumeric
                                    | VirtualKeyboardKey::ToggleAlphabetic
                                    | VirtualKeyboardKey::Backspace
                            ) {
                                keyboard_secondary_text
                            } else {
                                keyboard_text
                            },
                            align: TextAlign::Center,
                            role: TextRole::KeyboardKey,
                        }
                        .layout_in_rect(rect, [6.0 * responsive_scale * keyboard_scale, 2.0 * responsive_scale * keyboard_scale]);
                        text_quads.extend(layout.quads.iter().copied());
                        atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                        keyboard_layouts.push(layout);
                        if matches!(key, VirtualKeyboardKey::Backspace) {
                            append_backspace_icon_quads(&mut quads, centered_icon_rect(rect), keyboard_secondary_text);
                        }
                    }

                    text_sections.push(TextSection {
                        role: TextRole::KeyboardKey,
                        layouts: keyboard_layouts,
                    });

}
