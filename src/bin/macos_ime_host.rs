#[cfg(target_os = "macos")]
fn main() {
    use suzaku_map::ime::EngineConfig;
    use suzaku_map::ime_host::HostImeSession;
    use suzaku_map::platform::ime_host_dispatch::current_ime_host_dispatch;
    use suzaku_map::platform::macos_ime::bootstrap_status;

    let bootstrap = bootstrap_status();
    let dispatch = current_ime_host_dispatch();
    let mut session = HostImeSession::new(EngineConfig::default());
    let update = session.activate();

    println!("Suzaku macOS IME host skeleton");
    println!("{}", bootstrap.describe());
    println!("{}", dispatch.describe());
    println!(
        "bundle connection={}",
        bootstrap
            .bundle_connection_name
            .as_deref()
            .unwrap_or("(missing)")
    );
    println!(
        "controller class={}",
        bootstrap
            .controller_class_name
            .as_deref()
            .unwrap_or("(missing)")
    );
    println!(
        "controller lifecycle ready={} active={} init={} activate={} deactivate={} input={} commit={} last_marked={}",
        bootstrap.controller_lifecycle_ready,
        bootstrap.controller_debug_state.active,
        bootstrap.controller_debug_state.init_count,
        bootstrap.controller_debug_state.activate_count,
        bootstrap.controller_debug_state.deactivate_count,
        bootstrap.controller_debug_state.input_count,
        bootstrap.controller_debug_state.commit_count,
        bootstrap
            .controller_debug_state
            .last_marked_text
            .as_deref()
            .unwrap_or("(none)")
    );
    println!(
        "session active={} candidates={} committed=\"{}\"",
        update.active,
        update.candidates.len(),
        update.committed_text
    );
    println!(
        "next step: replace this bootstrap with an InputMethodKit controller bundle and wire HostImeSession into IMKInputController."
    );
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("macOS IME host skeleton is only available on macOS.");
}
