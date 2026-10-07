    // Native numbering describes IBus slots, not standalone completion chips.
    // Keep every current-page candidate and its absolute source identity visible.
    if let Some(page) = chrome.native_candidate_page
        && !visible_sentence_candidates.is_empty()
    {
        let gap = 4.0 * responsive_scale;
        let header_h = if bottom_dock { 36.0 } else { 27.0 } * responsive_scale;
        let columns = metrics.candidate_columns;
        let rows = visible_sentence_candidates.len().div_ceil(columns);
        let row_h = ((candidate_area_bottom - suggestions_y - header_h
            - gap * (rows as f32 + 1.0)) / rows as f32).max(0.0);
        let card_w = (panel_width - gap * columns.saturating_sub(1) as f32)
            / columns as f32;
        let button_w = if bottom_dock { 42.0 } else { 35.0 } * responsive_scale;
        let button_h = if bottom_dock { 32.0 } else { 24.0 } * responsive_scale;
        let number_w = 65.0 * responsive_scale;
        let controls_x = panel_x + panel_width - button_w * 2.0 - number_w - gap * 2.0;
        let mut add_text = |text: String, rect: [f32; 4], px: f32, color: [f32; 4],
                            align: TextAlign, role: TextRole, max_lines: usize| {
            let layout = TextBlock {
                text, origin: [0.0; 2], max_width: rect[2], pixel_size: px,
                letter_spacing: heading_tracking, line_gap: 2.0 * responsive_scale,
                max_lines, color, align, role,
            }.layout_in_rect(rect, [7.0 * responsive_scale, 3.0 * responsive_scale]);
            let truncated = layout.truncated;
            text_quads.extend(layout.quads.iter().copied());
            atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
            text_sections.push(TextSection { role, layouts: vec![layout] });
            truncated
        };
        add_text("1–6 · PageUp / PageDown".into(),
            [panel_x, suggestions_y, (controls_x - panel_x - gap).max(0.0), button_h],
            micro_px, text_secondary, TextAlign::Left, TextRole::HeaderStatus, 1);
        add_text(format!("{} / {}", page.start / NativeCandidatePage::SIZE + 1, page.count()),
            [controls_x + button_w + gap, suggestions_y, number_w, button_h],
            micro_px, text_secondary, TextAlign::Center, TextRole::HeaderStatus, 1);
        for (forward, x, label) in [
            (false, controls_x, "‹"),
            (true, controls_x + button_w + number_w + gap * 2.0, "›"),
        ] {
            let kind = InteractionKind::NativeCandidatePage(forward);
            let enabled = page.target(forward).is_some();
            let rect = [x, suggestions_y, button_w, button_h];
            let (hovered, pressed) = interaction_state(kind);
            quads.push(CandidateQuad::rounded(rect,
                if enabled && pressed { accent_soft }
                else if enabled && hovered { surface_alt } else { surface },
                6.0 * responsive_scale));
            quads.push(CandidateQuad::outline(rect,
                if enabled { shell_border } else { compact_divider },
                6.0 * responsive_scale, 0.8 * responsive_scale));
            add_text(label.into(), rect, chip_px,
                if enabled { accent_text } else { text_muted }, TextAlign::Center,
                TextRole::HeaderStatus, 1);
            if enabled {
                interactive_targets.push(InteractiveTarget { kind, rect });
            }
        }
        for (display_index, (source_index, label)) in visible_sentence_candidates.iter().enumerate() {
            let rect = [
                panel_x + (display_index % columns) as f32 * (card_w + gap),
                suggestions_y + header_h + gap + (display_index / columns) as f32 * (row_h + gap),
                card_w, row_h,
            ];
            let selected = *source_index == snapshot.selected_index;
            let kind = InteractionKind::Candidate(*source_index);
            let enabled = !page.busy;
            let (hovered, pressed) = if enabled { interaction_state(kind) } else { (false, false) };
            quads.push(CandidateQuad::rounded(rect,
                if enabled && (selected || pressed) { accent_soft }
                else if hovered { surface_alt } else { surface },
                7.0 * responsive_scale));
            quads.push(CandidateQuad::outline(rect,
                if !enabled { compact_divider }
                else if selected { accent } else { shell_border },
                7.0 * responsive_scale, 0.8 * responsive_scale));
            let display_label = if let Some((scroll_index, started_at)) = sentence_candidate_scroll
                && *scroll_index == *source_index && !label.is_empty()
            {
                let count = label.chars().count();
                let offset = (started_at.elapsed().as_millis() / 220) as usize % count;
                label.chars().cycle().skip(offset).take(count).collect::<String>()
            } else { label.clone() };
            let text = format!("{}  {}", source_index % NativeCandidatePage::SIZE + 1, display_label);
            let px = match chrome.text_scale {
                DisplayTextScale::Small => 1.85,
                DisplayTextScale::Medium => 2.1,
                DisplayTextScale::Large => 2.55,
            } * responsive_scale;
            if add_text(text, rect, px,
                if !enabled { text_muted } else if selected { accent_text } else { text_primary },
                TextAlign::Left, TextRole::CandidatePrimary, 2)
            {
                sentence_candidate_truncated.push(*source_index);
            }
            // Do not enlarge adjacent cards' hit boxes into each other.
            // Busy cards retain their layout, but are read-only until the host
            // confirms the pending edit. Neither pointer hit path may select them.
            if enabled {
                hit_targets.push(HitTarget { index: *source_index, rect });
                interactive_targets.push(InteractiveTarget { kind, rect });
            }
        }
    }
