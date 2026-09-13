if !chrome.settings_open || settings_panel_h <= 8.0 {
    settings_scroll_metadata = None;
} else {
    let mut settings = WgpuCandidateRenderer::new(panel_width, settings_panel_h - 8.0)
        .build_settings_scene(chrome, settings_option_text_scroll.copied());
    settings.translate([panel_x, settings_y + 8.0]);
    quads.extend(settings.quads);
    text_quads.extend(settings.text_quads);
    atlas_glyphs.extend(settings.atlas_glyphs);
    text_sections.extend(settings.text_sections);
    interactive_targets.extend(settings.interactive_targets);
    settings_option_truncated = settings.settings_option_truncated;
    settings_scroll_metadata = settings.settings_scroll_metadata;
}
