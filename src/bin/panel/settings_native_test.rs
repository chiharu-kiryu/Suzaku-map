//! Real panel/settings window controls, driven only inside the opt-in Xvfb fixture.
use super::app_state::PersistedDisplaySettings;
use super::*;
use suzaku_map::ime::settings::ImeSettings;
use winit::dpi::PhysicalSize;
use winit::platform::x11::EventLoopBuilderExtX11;

#[test]
#[ignore = "requires isolated Xvfb and SUZAKU_PANEL_NATIVE_QA=1"]
fn native_tone_controls_follow_acknowledgements_and_reload() {
    assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
    let config = std::env::var_os("XDG_CONFIG_HOME").expect("isolated config");
    assert!(std::path::Path::new(&config).starts_with(std::env::temp_dir()));
    assert!(std::env::var("DISPLAY").unwrap().split('.').next().unwrap() != ":0");
    let mut builder = EventLoop::<PanelUserEvent>::with_user_event();
    builder.with_x11().with_any_thread(true);
    let event_loop = builder.build().unwrap();
    let mut probe = SettingsProbe {
        proxy: event_loop.create_proxy(),
        completed: false,
        app: None,
        layout_step: 0,
        layout_expected: None,
        layout_changed: Instant::now(),
    };
    event_loop.run_app(&mut probe).unwrap();
    assert!(probe.completed);
}

struct SettingsProbe {
    proxy: EventLoopProxy<PanelUserEvent>,
    completed: bool,
    app: Option<PanelApp>,
    layout_step: usize,
    layout_expected: Option<PhysicalSize<u32>>,
    layout_changed: Instant,
}

fn assert_ui_language_preferences(app: &mut PanelApp, event_loop: &ActiveEventLoop) {
    use suzaku_map::ui::UiLanguage;
    let original = app.panel.as_ref().unwrap().chrome.ui_language;
    let draft = app.panel.as_ref().unwrap().chrome.seed_text.clone();
    let translation = app.panel.as_ref().unwrap().chrome.translation.clone();
    for language in UiLanguage::ALL.into_iter().chain([original]) {
        let settings = app.settings.as_mut().unwrap();
        settings.chrome.settings_search_query = "Interface".into();
        settings.chrome.settings_scroll_offset = 0.0;
        settings.last_interaction_action = None;
        let kind = InteractionKind::SetUiLanguage(language);
        let scene = settings.current_scene();
        let rect = scene
            .interactive_targets
            .iter()
            .find(|t| t.kind == kind)
            .unwrap()
            .rect;
        let point = (rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0);
        assert_eq!(scene.hit_interaction(point.0, point.1), Some(kind));
        settings.cursor_position = Some(point);
        settings.last_scene = Some(scene);
        settings.begin_primary_press(false);
        settings.complete_primary_release(false);
        assert_eq!(settings.chrome.ui_language, language);
        let id = settings.window.id();
        app.window_event(event_loop, id, WindowEvent::RedrawRequested);
        let panel = app.panel.as_ref().unwrap();
        assert_eq!(panel.chrome.ui_language, language);
        assert_eq!(panel.chrome.seed_text, draft);
        assert_eq!(panel.chrome.translation, translation);
        assert_eq!(load_display_settings().unwrap().ui_language, language);
        assert!(
            app.prediction_settings_sync
                .next_patch(&panel.chrome)
                .is_none()
        );
    }
    println!(
        "PASS: eight UI languages switch live, persist, synchronize windows and leave draft, translation and model settings unchanged"
    );
}

impl ApplicationHandler<PanelUserEvent> for SettingsProbe {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(event_loop.create_window(panel_window_attributes()).unwrap());
        let mut panel = pollster::block_on(PanelState::new(window)).unwrap();
        assert_first_overlay_uses_current_font_metrics(&mut panel);
        let mut app = PanelApp::new(self.proxy.clone(), None);
        app.panel = Some(panel);
        let mut native = ImeSettings::default();
        native.provider.temperature_tenths = 7;
        app.user_event(
            event_loop,
            PanelUserEvent::InputMethodSettingsChanged(native.clone()),
        );
        app.open_settings(event_loop);
        assert_controls(&app, LlmTemperaturePreset::Expressive);

        // Real Chinese labels, pointer hit testing, persistence and window-to-window sync.
        for theme in [
            suzaku_map::ime::gpu::ThemePreset::Baihu,
            suzaku_map::ime::gpu::ThemePreset::Qinglong,
            suzaku_map::ime::gpu::ThemePreset::Xuanwu,
            suzaku_map::ime::gpu::ThemePreset::Suzaku,
        ] {
            let settings = app.settings.as_mut().unwrap();
            settings.chrome.settings_search_query = theme.label().into();
            settings.chrome.settings_scroll_offset = 0.0;
            settings.last_interaction_action = None;
            let target = InteractionKind::SetThemePreset(theme);
            let scene = settings.current_scene();
            assert!(!scene.settings_option_truncated.contains(&target));
            let rect = scene
                .interactive_targets
                .iter()
                .find(|item| item.kind == target)
                .unwrap()
                .rect;
            let point = (rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0);
            assert_eq!(scene.hit_interaction(point.0, point.1), Some(target));
            settings.cursor_position = Some(point);
            settings.last_scene = Some(scene);
            settings.begin_primary_press(false);
            settings.complete_primary_release(false);
            assert_eq!(settings.chrome.theme_preset, theme);
            let settings_id = settings.window.id();
            app.window_event(event_loop, settings_id, WindowEvent::RedrawRequested);
            assert_eq!(app.panel.as_ref().unwrap().chrome.theme_preset, theme);
            assert_eq!(app.settings.as_ref().unwrap().chrome.theme_preset, theme);
            assert_eq!(load_display_settings().unwrap().theme_preset, theme);
            // A style choice must not enqueue model configuration or alter the draft.
            assert!(
                app.prediction_settings_sync
                    .next_patch(&app.panel.as_ref().unwrap().chrome)
                    .is_none()
            );
        }
        println!(
            "PASS: guardian theme clicks synchronize panel/settings and survive settings reload"
        );
        assert_native_titlebar_preference(&mut app, event_loop);
        assert_ui_language_preferences(&mut app, event_loop);

        for (selected, succeeds) in [
            (LlmTemperaturePreset::Focused, true),
            (LlmTemperaturePreset::Expressive, false),
        ] {
            let settings = app.settings.as_mut().unwrap();
            settings.chrome.settings_search_query = "Tone".into();
            settings.chrome.settings_scroll_offset = 0.0;
            settings.last_interaction_action = None;
            let target = InteractionKind::SetLlmTemperature(selected);
            let scene = settings.current_scene();
            let rect = scene
                .interactive_targets
                .iter()
                .find(|item| item.kind == target)
                .expect("Tone control")
                .rect;
            let point = (rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0);
            assert_eq!(scene.hit_interaction(point.0, point.1), Some(target));
            settings.cursor_position = Some(point);
            settings.last_scene = Some(scene);
            settings.begin_primary_press(false);
            settings.complete_primary_release(false);
            assert_eq!(settings.chrome.llm_temperature, selected);
            let settings_id = settings.window.id();
            // Use the application's real settings-window -> main-window event path.
            app.window_event(event_loop, settings_id, WindowEvent::RedrawRequested);
            assert_controls(&app, selected);
            let patch = app
                .prediction_settings_sync
                .next_patch(&app.panel.as_ref().unwrap().chrome)
                .unwrap();
            assert_eq!(patch.temperature_tenths, Some(selected.tenths()));
            assert_eq!(patch.enabled, None);
            if succeeds {
                patch.apply(&mut native).unwrap();
            }
            app.user_event(
                event_loop,
                PanelUserEvent::PredictionSettingsApplied(Some(native.clone())),
            );
            assert_controls(
                &app,
                LlmTemperaturePreset::from_tenths(native.provider.temperature_tenths),
            );
            assert!(
                app.prediction_settings_sync
                    .next_patch(&app.panel.as_ref().unwrap().chrome)
                    .is_none()
            );
        }
        native.provider.temperature_tenths = 3;
        app.user_event(
            event_loop,
            PanelUserEvent::InputMethodSettingsChanged(native),
        );
        assert_controls(&app, LlmTemperaturePreset::Custom(3));
        let scene = app.settings.as_mut().unwrap().current_scene();
        assert!(
            scene.interactive_targets.iter().any(|item| item.kind
                == InteractionKind::SetLlmTemperature(LlmTemperaturePreset::Custom(3)))
        );
        println!(
            "PASS: Tone clicks synchronize both windows, acknowledge success, roll back failure and preserve custom reload values"
        );
        self.app = Some(app);
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if let Some(app) = self.app.as_mut() {
            app.window_event(event_loop, id, event);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        use suzaku_map::ime::gpu::SettingsCategory;
        let Some(app) = self.app.as_mut() else {
            return;
        };
        let categories = [
            SettingsCategory::Appearance,
            SettingsCategory::Input,
            SettingsCategory::Model,
            SettingsCategory::Appearance,
        ];
        if self.layout_step == categories.len() {
            self.completed = true;
            println!(
                "PASS: category clicks clear global search, synchronize both windows and resize native settings to content without oscillation"
            );
            event_loop.exit();
            return;
        }
        if self.layout_expected.is_none() {
            let category = categories[self.layout_step];
            let settings = app.settings.as_mut().unwrap();
            let saved_settings = PersistedDisplaySettings::from(&settings.chrome);
            let draft = settings.chrome.seed_text.clone();
            settings.last_interaction_action = None;
            let scene = settings.current_scene();
            let kind = InteractionKind::SetSettingsCategory(category);
            let rect = scene
                .interactive_targets
                .iter()
                .find(|t| t.kind == kind)
                .unwrap()
                .rect;
            settings.cursor_position = Some((rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0));
            settings.last_scene = Some(scene);
            settings.begin_primary_press(false);
            settings.complete_primary_release(false);
            assert!(settings.chrome.settings_search_query.is_empty());
            assert_eq!(settings.chrome.settings_scroll_offset, 0.0);
            let id = settings.window.id();
            app.window_event(event_loop, id, WindowEvent::RedrawRequested);
            assert_eq!(
                app.panel.as_ref().unwrap().chrome.settings_category,
                category
            );
            for state in [app.panel.as_ref().unwrap(), app.settings.as_ref().unwrap()] {
                assert_eq!(
                    PersistedDisplaySettings::from(&state.chrome),
                    saved_settings
                );
                assert_eq!(state.chrome.seed_text, draft);
            }
            assert_eq!(load_display_settings().unwrap(), saved_settings);
            assert!(
                app.prediction_settings_sync
                    .next_patch(&app.panel.as_ref().unwrap().chrome)
                    .is_none()
            );
            let settings = app.settings.as_ref().unwrap();
            let height = settings
                .last_scene
                .as_ref()
                .unwrap()
                .settings_scroll_metadata
                .unwrap()
                .preferred_window_height;
            self.layout_expected = Some(PhysicalSize::new(
                settings.window.inner_size().width,
                (height.ceil() as u32).clamp(220, 680),
            ));
            self.layout_changed = Instant::now();
        }
        app.about_to_wait(event_loop);
        assert!(
            self.layout_changed.elapsed() < Duration::from_secs(5),
            "settings height did not settle"
        );
        let settings = app.settings.as_ref().unwrap();
        if self.layout_changed.elapsed() >= Duration::from_millis(120)
            && Some(settings.window.inner_size()) == self.layout_expected
        {
            assert_eq!(Some(settings.size), self.layout_expected);
            self.layout_step += 1;
            self.layout_expected = None;
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(16),
        ));
    }
}

fn assert_controls(app: &PanelApp, expected: LlmTemperaturePreset) {
    assert_eq!(app.panel.as_ref().unwrap().chrome.llm_temperature, expected);
    assert_eq!(
        app.settings.as_ref().unwrap().chrome.llm_temperature,
        expected
    );
    assert_eq!(load_display_settings().unwrap().llm_temperature, expected);
}

fn assert_native_titlebar_preference(app: &mut PanelApp, event_loop: &ActiveEventLoop) {
    let draft = "title bar draft stays editable";
    app.panel
        .as_mut()
        .unwrap()
        .chrome
        .set_seed_text(draft.into());
    app.open_settings(event_loop);
    assert!(settings_window_attributes(&app.panel.as_ref().unwrap().chrome).transparent);
    for state in [app.panel.as_ref().unwrap(), app.settings.as_ref().unwrap()] {
        assert_native_transparent_surface(state);
    }
    assert!(!load_display_settings().unwrap().hide_system_titlebar);
    for hide in [true, false, true] {
        let settings = app.settings.as_mut().unwrap();
        let mut expected = PersistedDisplaySettings::from(&settings.chrome);
        expected.hide_system_titlebar = hide;
        settings.chrome.settings_search_query = "Title Bar".into();
        settings.chrome.settings_scroll_offset = 0.0;
        settings.last_interaction_action = None;
        let kind = InteractionKind::SetHideSystemTitlebar(hide);
        let scene = settings.current_scene();
        assert!(!scene.settings_option_truncated.contains(&kind));
        let rect = scene
            .interactive_targets
            .iter()
            .find(|target| target.kind == kind)
            .unwrap()
            .rect;
        let point = (rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0);
        assert_eq!(scene.hit_interaction(point.0, point.1), Some(kind));
        settings.cursor_position = Some(point);
        settings.last_scene = Some(scene);
        settings.begin_primary_press(false);
        settings.complete_primary_release(false);
        assert_eq!(settings.window.is_decorated(), !hide);
        let settings_id = settings.window.id();
        app.window_event(event_loop, settings_id, WindowEvent::RedrawRequested);
        for state in [app.panel.as_ref().unwrap(), app.settings.as_ref().unwrap()] {
            assert_eq!(state.window.is_decorated(), !hide);
            assert_eq!(PersistedDisplaySettings::from(&state.chrome), expected);
            assert_eq!(state.chrome.seed_text, draft);
        }
        assert_eq!(load_display_settings().unwrap(), expected);
        assert_eq!(panel_window_attributes().decorations, !hide);
        assert_eq!(
            settings_window_attributes(&app.panel.as_ref().unwrap().chrome).decorations,
            !hide
        );
        assert!(
            app.prediction_settings_sync
                .next_patch(&app.panel.as_ref().unwrap().chrome)
                .is_none()
        );

        // Reopening creates a new native settings window with the saved preference.
        app.window_event(event_loop, settings_id, WindowEvent::CloseRequested);
        app.open_settings(event_loop);
        assert_eq!(app.settings.as_ref().unwrap().window.is_decorated(), !hide);
        assert_native_transparent_surface(app.settings.as_ref().unwrap());
    }

    // Exercise the reverse main -> settings synchronization as well.
    let panel = app.panel.as_mut().unwrap();
    let mut updated = panel.chrome.clone();
    updated.hide_system_titlebar = false;
    assert!(panel.adopt_settings_from(&updated));
    let panel_id = panel.window.id();
    app.window_event(event_loop, panel_id, WindowEvent::RedrawRequested);
    assert!(app.settings.as_ref().unwrap().window.is_decorated());
    let settings_id = app.settings.as_ref().unwrap().window.id();
    app.window_event(event_loop, settings_id, WindowEvent::CloseRequested);

    for hide in [true, false] {
        let panel = app.panel.as_mut().unwrap();
        panel.last_compact_toggle = None;
        panel.apply_compact_mode(true);
        assert!(!panel.window.is_decorated());
        let mut updated = panel.chrome.clone();
        updated.hide_system_titlebar = hide;
        panel.adopt_settings_from(&updated);
        assert!(
            !panel.window.is_decorated(),
            "preference must not decorate the orb"
        );
        panel.last_compact_toggle = None;
        panel.apply_compact_mode(false);
        assert_eq!(panel.window.is_decorated(), !hide);
        assert_eq!(panel.chrome.seed_text, draft);
        assert_eq!(load_display_settings().unwrap().hide_system_titlebar, hide);
    }
    app.open_settings(event_loop);
    println!(
        "PASS: system title bar toggles both native windows, persists, reopens and restores from the orb without changing the draft"
    );
}

fn assert_native_transparent_surface(state: &PanelState) {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use x11rb::protocol::xproto::ConnectionExt;
    let RawWindowHandle::Xlib(handle) = state.window.window_handle().unwrap().as_raw() else {
        panic!("isolated X11 fixture");
    };
    let (connection, _) = x11rb::connect(None).unwrap();
    let geometry = connection
        .get_geometry(handle.window as u32)
        .unwrap()
        .reply()
        .unwrap();
    assert_eq!(
        geometry.depth, 32,
        "both windows need an alpha-capable native visual"
    );
    // Xvfb can expose only an opaque GLES swapchain even with a 32-bit native
    // visual. It must remain readable; Vulkan/XWayland alpha selection and GPU
    // pixel coverage are tested separately.
    let clear =
        crate::render::surface_clear_color(state.config.alpha_mode, true, wgpu::Color::WHITE);
    assert_eq!(
        clear.a,
        if matches!(
            state.config.alpha_mode,
            wgpu::CompositeAlphaMode::PreMultiplied | wgpu::CompositeAlphaMode::Inherit
        ) {
            0.0
        } else {
            1.0
        }
    );
}

fn assert_first_overlay_uses_current_font_metrics(panel: &mut PanelState) {
    for face in [
        suzaku_map::ime::gpu::FontFaceChoice::Auto,
        suzaku_map::ime::gpu::FontFaceChoice::Monaco,
    ] {
        panel.chrome.font_face = face;
        panel.rebuild_font_atlas();
        panel.last_commit_feedback = Some("ΩλΨЖ  ççç  WWW iii — saved".into());
        panel.commit_feedback_ticks = 180;
        let snapshot = panel.engine.snapshot();
        let (_, first) = panel.current_frame_with_snapshot(&snapshot);
        assert_eq!(first.len(), 1);
        assert!(!first[0].atlas_glyphs.is_empty());
        for glyph in &first[0].atlas_glyphs {
            let measured = panel
                .font_atlas
                .layout_metrics
                .get(&glyph.ch)
                .expect("system glyph metrics");
            assert!(
                (glyph.rect[2] - measured * glyph.rect[3] / 7.0).abs() < 0.001,
                "first-frame overlay stretched {:?} after switching to {face:?}",
                glyph.ch
            );
        }
        let (_, second) = panel.current_frame_with_snapshot(&snapshot);
        assert_eq!(
            first[0].quads, second[0].quads,
            "tooltip surface jumped on its second frame"
        );
        assert_eq!(first[0].atlas_glyphs, second[0].atlas_glyphs);
    }
    panel.last_commit_feedback = None;
    panel.commit_feedback_ticks = 0;
    panel.chrome.font_face = suzaku_map::ime::gpu::FontFaceChoice::Auto;
    panel.rebuild_font_atlas();
}
