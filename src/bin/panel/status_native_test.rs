//! Actual panel handlers with a fixed, slow IBus-query stub in a private Xvfb process.
use super::*;
use winit::platform::x11::EventLoopBuilderExtX11;

#[test]
#[ignore = "requires isolated Xvfb and SUZAKU_STATUS_NATIVE_QA=1"]
fn native_panel_input_does_not_wait_for_status_probes() {
    assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
    assert_eq!(std::env::var("SUZAKU_STATUS_NATIVE_QA").as_deref(), Ok("1"));
    let config = std::env::var_os("XDG_CONFIG_HOME").expect("isolated config");
    assert!(std::path::Path::new(&config).starts_with(std::env::temp_dir()));
    assert!(std::env::var("DISPLAY").unwrap().split('.').next().unwrap() != ":0");
    let fixture_bin = std::path::PathBuf::from(
        std::env::var_os("SUZAKU_STATUS_FIXTURE_BIN").expect("isolated probe fixture"),
    );
    assert!(fixture_bin.starts_with(std::env::temp_dir()) && fixture_bin.ends_with("bin"));
    // Cargo prepends its library/binary directories to PATH for test processes.
    let ibus_bin = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .find(|directory| directory.join("ibus").is_file());
    assert_eq!(ibus_bin.as_ref(), Some(&fixture_bin));
    assert!(
        std::fs::read_to_string(fixture_bin.join("ibus"))
            .unwrap()
            .contains("Dedicated, non-recursive fixture")
    );
    let mut builder = EventLoop::<PanelUserEvent>::with_user_event();
    builder.with_x11().with_any_thread(true);
    let event_loop = builder.build().unwrap();
    let mut probe = StatusProbe {
        completed: false,
        proxy: event_loop.create_proxy(),
    };
    event_loop.run_app(&mut probe).unwrap();
    assert!(probe.completed);
}

struct StatusProbe {
    completed: bool,
    proxy: EventLoopProxy<PanelUserEvent>,
}

impl ApplicationHandler<PanelUserEvent> for StatusProbe {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        use suzaku_map::platform::ime_host_dispatch::current_ime_host_dispatch;
        let started = Instant::now();
        let cold = current_ime_host_dispatch();
        assert!(
            !cold.commit_roundtrip,
            "unknown startup status is conservative"
        );
        current_panel_companion_dispatch();
        assert!(
            started.elapsed() < Duration::from_millis(200),
            "cold status query blocked the UI"
        );
        let window = Arc::new(event_loop.create_window(panel_window_attributes()).unwrap());
        let mut panel = pollster::block_on(PanelState::new(window)).unwrap();
        panel.voice.bridge = None;
        panel.chrome.set_seed_text(String::new());
        for (phase, text) in [("startup", "x"), ("expired cache", "y")] {
            if phase == "expired cache" {
                // Test-only wait: the 350 ms bounded probe and its 3 s cache expire.
                std::thread::sleep(Duration::from_millis(3500));
            }
            let started = Instant::now();
            panel.set_window_focus(true);
            panel.chrome.focus_input();
            panel.handle_text_input(text);
            panel.move_candidate_selection(1);
            panel.set_window_focus(false);
            let elapsed = started.elapsed();
            assert!(
                elapsed < Duration::from_millis(200),
                "{phase} handlers blocked: {elapsed:?}"
            );
            println!(
                "PASS: {phase} focus/edit/selection handlers return in {elapsed:?} with a 1.2 s IBus-query fixture"
            );
        }
        assert_eq!(panel.chrome.seed_text, "xy");
        // Allow the bounded final background probe to reap its subprocess before exit.
        std::thread::sleep(Duration::from_millis(500));
        native_sync::assert_workers_cancel_on_shutdown(self.proxy.clone());
        self.completed = true;
        event_loop.exit();
    }
    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}
