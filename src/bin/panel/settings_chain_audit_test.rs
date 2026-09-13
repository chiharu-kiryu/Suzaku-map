//! F42/F44 regressions. Private windows/configuration only, never the desktop.
use super::*;
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};
use suzaku_map::{ime::settings::ImeSettings, platform::settings_host::display_settings_path};
use winit::platform::x11::EventLoopBuilderExtX11;

fn run_probe(check: fn(&mut PanelApp, &ActiveEventLoop) -> Vec<String>) {
    assert_eq!(
        std::env::var("SUZAKU_SETTINGS_CHAIN_AUDIT").as_deref(),
        Ok("1")
    );
    assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
    for key in [
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_RUNTIME_DIR",
        "SUZAKU_IME_CONFIG",
    ] {
        assert!(PathBuf::from(std::env::var_os(key).unwrap()).starts_with(std::env::temp_dir()));
    }
    assert_ne!(
        std::env::var("DISPLAY").unwrap().split('.').next().unwrap(),
        ":0"
    );
    ImeSettings::default().save().unwrap(); // Disabled LLM; no model is contacted.
    let mut builder = EventLoop::<PanelUserEvent>::with_user_event();
    builder.with_x11().with_any_thread(true);
    let events = builder.build().unwrap();
    struct Probe {
        proxy: EventLoopProxy<PanelUserEvent>,
        check: fn(&mut PanelApp, &ActiveEventLoop) -> Vec<String>,
        findings: Option<Vec<String>>,
    }
    impl ApplicationHandler<PanelUserEvent> for Probe {
        fn resumed(&mut self, events: &ActiveEventLoop) {
            let window = Arc::new(events.create_window(panel_window_attributes()).unwrap());
            let panel = pollster::block_on(PanelState::new(window)).unwrap();
            assert!(!panel.chrome.llm_enabled);
            let mut app = PanelApp::new(self.proxy.clone(), None);
            // Do not run PanelApp::resumed: no tray/service/desktop workers are started.
            app.panel = Some(panel);
            app.open_settings(events);
            self.findings = Some((self.check)(&mut app, events));
            events.exit();
        }
        fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
    }
    let mut probe = Probe {
        proxy: events.create_proxy(),
        check,
        findings: None,
    };
    events.run_app(&mut probe).unwrap();
    let findings = probe.findings.expect("probe completed");
    for finding in &findings {
        eprintln!("AUDIT: {finding}");
    }
    assert!(
        findings.is_empty(),
        "{} settings-chain failures",
        findings.len()
    );
}

fn click_setting(
    app: &mut PanelApp,
    events: &ActiveEventLoop,
    target: InteractionKind,
    search: &str,
) {
    let settings = app.settings.as_mut().unwrap();
    settings.chrome.settings_search_query = search.into();
    settings.chrome.settings_scroll_offset = 0.0;
    settings.last_interaction_action = None;
    let scene = settings.current_scene();
    let rect = scene
        .interactive_targets
        .iter()
        .find(|item| item.kind == target)
        .expect("setting is reachable")
        .rect;
    let point = (rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0);
    assert_eq!(scene.hit_interaction(point.0, point.1), Some(target));
    settings.cursor_position = Some(point);
    settings.last_scene = Some(scene);
    settings.begin_primary_press(false);
    settings.complete_primary_release(false);
    let id = settings.window.id();
    app.window_event(events, id, WindowEvent::RedrawRequested);
}

#[test]
#[ignore = "requires private Xvfb/configuration; run by test-settings-chain-audit.sh in Linux CI"]
fn audit_first_status_must_preserve_a_startup_ui_edit() {
    run_probe(|app, events| {
        let mut findings = Vec::new();
        let native = ImeSettings::default();
        for selected in [
            LlmTemperaturePreset::Focused,
            LlmTemperaturePreset::Expressive,
        ] {
            app.prediction_settings_sync = Default::default();
            app.last_native_ime_settings = None;
            app.panel.as_mut().unwrap().chrome.llm_temperature = LlmTemperaturePreset::Balanced;
            app.settings.as_mut().unwrap().chrome.llm_temperature = LlmTemperaturePreset::Balanced;
            click_setting(
                app,
                events,
                InteractionKind::SetLlmTemperature(selected),
                "Tone",
            );
            assert_eq!(app.panel.as_ref().unwrap().chrome.llm_temperature, selected);
            assert_eq!(load_display_settings().unwrap().llm_temperature, selected);
            assert!(
                app.prediction_settings_sync
                    .next_patch(&app.panel.as_ref().unwrap().chrome)
                    .is_none()
            );
            app.user_event(
                events,
                PanelUserEvent::InputMethodSettingsChanged(native.clone()),
            );
            let actual = app.panel.as_ref().unwrap().chrome.llm_temperature;
            let pending = app
                .prediction_settings_sync
                .next_patch(&app.panel.as_ref().unwrap().chrome);
            if actual != selected
                || pending.and_then(|patch| patch.temperature_tenths) != Some(selected.tenths())
            {
                findings.push(format!("N07 first status after {selected:?} click: panel={actual:?}, settings={:?}, saved={:?}, next_patch={pending:?}",
                    app.settings.as_ref().unwrap().chrome.llm_temperature, load_display_settings().unwrap().llm_temperature));
            }
            // The same click survives once an initial status has been observed.
            app.prediction_settings_sync = Default::default();
            app.apply_native_settings(native.clone(), false);
            click_setting(
                app,
                events,
                InteractionKind::SetLlmTemperature(selected),
                "Tone",
            );
            app.apply_native_settings(native.clone(), false);
            let patch = app
                .prediction_settings_sync
                .next_patch(&app.panel.as_ref().unwrap().chrome)
                .unwrap();
            assert_eq!(patch.temperature_tenths, Some(selected.tenths()));
            let mut acknowledged = native.clone();
            patch.apply(&mut acknowledged).unwrap();
            app.finish_prediction_settings(Some(acknowledged));
            assert_eq!(app.panel.as_ref().unwrap().chrome.llm_temperature, selected);
            assert!(
                app.prediction_settings_sync
                    .next_patch(&app.panel.as_ref().unwrap().chrome)
                    .is_none()
            );
            println!(
                "PASS: click after initial status survives refresh and acknowledgement: {selected:?}"
            );
        }
        findings
    });
}

struct RestorePermissions {
    path: PathBuf,
    permissions: fs::Permissions,
}

impl Drop for RestorePermissions {
    fn drop(&mut self) {
        fs::set_permissions(&self.path, self.permissions.clone()).unwrap();
    }
}

#[test]
#[ignore = "requires private Xvfb and non-root file permissions; run by test-settings-chain-audit.sh in Linux CI"]
fn audit_display_save_failure_must_be_visible_or_rolled_back() {
    run_probe(|app, events| {
        use suzaku_map::ime::gpu::ThemePreset;
        app.apply_native_settings(ImeSettings::default(), false);
        let panel = app.panel.as_mut().unwrap();
        panel.chrome.theme_preset = ThemePreset::Suzaku;
        panel.persist_display_settings();
        panel.last_commit_feedback = None;
        let path = display_settings_path();
        let before = fs::read(&path).unwrap();
        let parent = path.parent().unwrap().to_path_buf();
        let restore = RestorePermissions {
            permissions: fs::metadata(&parent).unwrap().permissions(),
            path: parent.clone(),
        };
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o500)).unwrap();
        assert!(
            app_state::save_display_settings(&app_state::PersistedDisplaySettings::from(
                &panel.chrome
            ))
            .is_err(),
            "fixture must reject writes (do not run as root)"
        );
        click_setting(
            app,
            events,
            InteractionKind::SetThemePreset(ThemePreset::Baihu),
            ThemePreset::Baihu.label(),
        );
        assert_eq!(
            fs::read(&path).unwrap(),
            before,
            "failed save must preserve old bytes"
        );
        let panel = app.panel.as_ref().unwrap();
        let saved = load_display_settings().unwrap().theme_preset;
        let reopened_window = Arc::new(events.create_window(panel_window_attributes()).unwrap());
        let reopened = pollster::block_on(PanelState::new(reopened_window)).unwrap();
        assert_eq!(
            reopened.chrome.theme_preset, saved,
            "a newly created panel loads the old saved theme"
        );
        let mut findings = Vec::new();
        if panel.chrome.theme_preset != saved
            && panel.last_commit_feedback.is_none()
            && app
                .settings
                .as_ref()
                .unwrap()
                .last_commit_feedback
                .is_none()
        {
            findings.push(format!("N09 failed appearance save: panel={:?}, settings={:?}, reloaded={saved:?}, both error messages absent",
                panel.chrome.theme_preset, app.settings.as_ref().unwrap().chrome.theme_preset));
        }
        assert!(app.panel.as_ref().unwrap().chrome.settings_save_failed);
        assert!(app.settings.as_ref().unwrap().chrome.settings_save_failed);
        let scene = app.settings.as_mut().unwrap().current_scene();
        assert!(
            scene
                .interactive_targets
                .iter()
                .any(|item| item.kind == InteractionKind::RetrySaveSettings)
        );
        drop(restore);
        // Redrawing after the filesystem recovers is not a request to keep writing.
        for _ in 0..3 {
            let id = app.settings.as_ref().unwrap().window.id();
            app.window_event(events, id, WindowEvent::RedrawRequested);
            assert_eq!(fs::read(&path).unwrap(), before);
        }
        click_setting(app, events, InteractionKind::RetrySaveSettings, "");
        assert_eq!(
            load_display_settings().unwrap().theme_preset,
            ThemePreset::Baihu
        );
        assert!(!app.panel.as_ref().unwrap().chrome.settings_save_failed);
        assert!(!app.settings.as_ref().unwrap().chrome.settings_save_failed);
        assert!(app.panel.as_ref().unwrap().last_commit_feedback.is_none());
        let scene = app.settings.as_mut().unwrap().current_scene();
        assert!(
            !scene
                .interactive_targets
                .iter()
                .any(|item| item.kind == InteractionKind::RetrySaveSettings)
        );
        click_setting(
            app,
            events,
            InteractionKind::SetThemePreset(ThemePreset::Qinglong),
            ThemePreset::Qinglong.label(),
        );
        assert_eq!(
            load_display_settings().unwrap().theme_preset,
            ThemePreset::Qinglong
        );
        println!(
            "PASS: failed save is visible in both windows, old bytes survive, no redraw retry loop; explicit Retry saves the preview and clears the warning"
        );
        findings
    });
}
