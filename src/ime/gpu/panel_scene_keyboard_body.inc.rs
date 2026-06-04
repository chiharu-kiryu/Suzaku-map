                    let keyboard_y = drawer_rect[1] + 16.0 * responsive_scale;
                    let key_gap = 5.0 * responsive_scale;
                    let row_h = 24.0 * responsive_scale;
                    let mut keyboard_layouts = Vec::new();
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
                        let row_y = keyboard_y + row_index as f32 * (row_h + key_gap);
                        let inset = if chrome.keyboard_numeric {
                            if row_index == 1 {
                                12.0 * responsive_scale
                            } else {
                                0.0
                            }
                        } else if row_index == 1 {
                            18.0 * responsive_scale
                        } else if row_index == 2 {
                            8.0 * responsive_scale
                        } else {
                            0.0
                        };
                        let row_width = drawer_rect[2] - inset * 2.0;
                        let key_w = (row_width - key_gap * (key_count - 1.0)) / key_count;

                        for (key_index, key) in keys.iter().enumerate() {
                            let x =
                                drawer_rect[0] + inset + key_index as f32 * (key_w + key_gap);
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
                                7.0 * responsive_scale,
                            );
                            interactive_targets.push(InteractiveTarget {
                                kind: InteractionKind::VirtualKeyboardKey(*key),
                                rect,
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
                                    x + 8.0 * responsive_scale,
                                    row_y + 8.0 * responsive_scale,
                                ],
                                max_width: key_w - 16.0 * responsive_scale,
                                pixel_size: if matches!(
                                    key,
                                    VirtualKeyboardKey::Backspace
                                        | VirtualKeyboardKey::ToggleNumeric
                                        | VirtualKeyboardKey::ToggleAlphabetic
                                ) {
                                    (chip_px * 0.9).max(1.75 * responsive_scale)
                                } else {
                                    (chip_px * 1.1).max(2.2 * responsive_scale)
                                },
                                letter_spacing: ui_tracking,
                                line_gap: base_line_gap,
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
                            .layout();
                            text_quads.extend(layout.quads.iter().copied());
                            atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                            keyboard_layouts.push(layout);
                            if matches!(key, VirtualKeyboardKey::Backspace) {
                                append_backspace_icon_quads(
                                    &mut quads,
                                    rect,
                                    keyboard_secondary_text,
                                );
                            }
                        }
                    }

                    let action_y = keyboard_y + 3.0 * (row_h + key_gap);
                    let left_w = 70.0 * responsive_scale;
                    let mid_key_w = 36.0 * responsive_scale;
                    let right_w = 104.0 * responsive_scale;
                    let space_w =
                        drawer_rect[2] - left_w - right_w - key_gap * 3.0 - mid_key_w * 2.0;
                    let action_keys = if chrome.keyboard_numeric {
                        vec![
                            (
                                VirtualKeyboardKey::ToggleAlphabetic,
                                "ABC",
                                [drawer_rect[0], action_y, left_w, row_h],
                            ),
                            (
                                VirtualKeyboardKey::Character(','),
                                ",",
                                [
                                    drawer_rect[0] + left_w + key_gap,
                                    action_y,
                                    mid_key_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Space,
                                "Space",
                                [
                                    drawer_rect[0] + left_w + key_gap * 2.0 + mid_key_w,
                                    action_y,
                                    space_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Character('.'),
                                ".",
                                [
                                    drawer_rect[0]
                                        + left_w
                                        + key_gap * 3.0
                                        + mid_key_w
                                        + space_w,
                                    action_y,
                                    mid_key_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Backspace,
                                "",
                                [
                                    drawer_rect[0]
                                        + left_w
                                        + key_gap * 4.0
                                        + mid_key_w * 2.0
                                        + space_w,
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
                                [drawer_rect[0], action_y, left_w, row_h],
                            ),
                            (
                                VirtualKeyboardKey::Character(','),
                                ",",
                                [
                                    drawer_rect[0] + left_w + key_gap,
                                    action_y,
                                    mid_key_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Space,
                                "Space",
                                [
                                    drawer_rect[0] + left_w + key_gap * 2.0 + mid_key_w,
                                    action_y,
                                    space_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Character('.'),
                                ".",
                                [
                                    drawer_rect[0]
                                        + left_w
                                        + key_gap * 3.0
                                        + mid_key_w
                                        + space_w,
                                    action_y,
                                    mid_key_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Backspace,
                                "",
                                [
                                    drawer_rect[0]
                                        + left_w
                                        + key_gap * 4.0
                                        + mid_key_w * 2.0
                                        + space_w,
                                    action_y,
                                    right_w,
                                    row_h,
                                ],
                            ),
                        ]
                    };

                    for (key, label, rect) in action_keys {
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
                            8.0 * responsive_scale,
                        );
                        interactive_targets.push(InteractiveTarget {
                            kind: InteractionKind::VirtualKeyboardKey(key),
                            rect,
                        });
                        let layout = TextBlock {
                            text: label.to_string(),
                            origin: [
                                rect[0] + 10.0 * responsive_scale,
                                rect[1] + 8.0 * responsive_scale,
                            ],
                            max_width: rect[2] - 20.0 * responsive_scale,
                            pixel_size: if label.chars().count() > 6 {
                                1.8 * responsive_scale
                            } else {
                                2.2 * responsive_scale
                            },
                            letter_spacing: ui_tracking,
                            line_gap: base_line_gap,
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
                        .layout();
                        text_quads.extend(layout.quads.iter().copied());
                        atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                        keyboard_layouts.push(layout);
                        if matches!(key, VirtualKeyboardKey::Backspace) {
                            append_backspace_icon_quads(&mut quads, rect, keyboard_secondary_text);
                        }
                    }

                    text_sections.push(TextSection {
                        role: TextRole::KeyboardKey,
                        layouts: keyboard_layouts,
                    });
