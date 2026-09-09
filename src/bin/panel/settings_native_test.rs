//! Real panel/settings window controls, driven only inside the opt-in Xvfb fixture.
use super::*;
use suzaku_map::ime::settings::ImeSettings;
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
    };
    event_loop.run_app(&mut probe).unwrap();
    assert!(probe.completed);
}

struct SettingsProbe {
    proxy: EventLoopProxy<PanelUserEvent>,
    completed: bool,
}

impl ApplicationHandler<PanelUserEvent> for SettingsProbe {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(event_loop.create_window(panel_window_attributes()).unwrap());
        let panel = pollster::block_on(PanelState::new(window)).unwrap();
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
        self.completed = true;
        event_loop.exit();
    }
    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}

fn assert_controls(app: &PanelApp, expected: LlmTemperaturePreset) {
    assert_eq!(app.panel.as_ref().unwrap().chrome.llm_temperature, expected);
    assert_eq!(
        app.settings.as_ref().unwrap().chrome.llm_temperature,
        expected
    );
    assert_eq!(load_display_settings().unwrap().llm_temperature, expected);
}
