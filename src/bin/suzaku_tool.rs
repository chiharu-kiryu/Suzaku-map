use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, exit};
use std::time::Duration;

#[path = "suzaku_tool/model.rs"]
mod model;

#[path = "suzaku_tool/data.rs"]
mod data;

const CONNECTION_NAME: &str = "dev.suzaku.linux.ime";
const IBUS_COMPONENT_NAME: &str = "org.freedesktop.IBus.Suzaku";
const IBUS_USER_SERVICE_NAME: &str = "suzaku-ibus.service";
const GNOME_INPUT_SOURCES_SCHEMA: &str = "org.gnome.desktop.input-sources";
const GNOME_INPUT_SOURCES_KEY: &str = "sources";
const FCITX_CONFIG_NAME: &str = "dev_suzaku_linux_ime.conf";
const DEFAULT_ANDROID_SDK_ROOT: &str = "/opt/homebrew/share/android-commandlinetools";
const DEFAULT_JAVA_HOME: &str = "/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home";
const DEFAULT_ANDROID_PROFILE: &str = "debug";

fn main() {
    let code = run();
    exit(code);
}

fn run() -> i32 {
    let args = env::args().collect::<Vec<_>>();
    if args.len() <= 1 {
        print_help();
        return 0;
    }

    match args[1].as_str() {
        "--version" | "-V" => {
            println!("suzaku-map {}", env!("CARGO_PKG_VERSION"));
            0
        }
        "data" => data::run(&args[2..]),
        "model" | "llama" => model::run(&args[2..]),
        "linux-register" => linux_register(&args[2..]),
        "linux-register-ime" => linux_register(&args[2..]),
        "android" => android_dispatch(&args[2..]),
        "android-env" => android_env(&args[2..]),
        "android-build-native" => {
            let release = args.iter().any(|arg| arg == "--release");
            run_android_build_native_with_flags(release)
        }
        "android-install-debug" => android_install_debug(&args[2..]),
        "android-enable-ime" => android_enable_ime(&args[2..]),
        "macos" => macos_dispatch(&args[2..]),
        "build-macos-app" => macos_build_app(),
        "open-macos-app" => macos_open_app(),
        "help" | "--help" | "-h" => {
            print_help();
            0
        }
        _ => {
            eprintln!("unknown command: {}", args[1]);
            print_help();
            1
        }
    }
}

fn print_help() {
    println!("Usage:");
    println!(
        "  suzaku_tool data [status|backup [FILE]|validate FILE|restore FILE [--apply]|open [ime|panel|backups]]"
    );
    println!(
        "  suzaku_tool model [status|discover|warmup|probe [all|en|zh-Hans|ja]|configure [--scope local|cloud] [--protocol auto|ollama|openai-compatible] [--model auto|NAME] [--endpoint URL] [--timeout-ms N] [--api-key-env NAME|none] [--cloud-consent true|false]] (alias: llama)"
    );
    println!("  suzaku_tool linux-register [install|status|verify|uninstall|diag]");
    println!("  suzaku_tool linux-register-ime [install|status|verify|uninstall|diag]");
    println!(
        "  # Build native IBus host first: cargo build --features linux-ibus --bin linux_ime_host"
    );
    println!("  suzaku_tool android env");
    println!("  suzaku_tool android build-native [--release]");
    println!("  suzaku_tool android install-debug");
    println!("  suzaku_tool android enable-ime");
    println!("  suzaku_tool macos build-app [panel|ime]");
    println!("  suzaku_tool macos open-app [panel|ime]");
    println!("Legacy entry alias:");
    println!("  suzaku_tool linux-register-ime ...");
    println!("  suzaku_tool android-env");
    println!("  suzaku_tool android-build-native [--release]");
    println!("  suzaku_tool android-install-debug");
    println!("  suzaku_tool android-enable-ime");
    println!("  suzaku_tool build-macos-app");
    println!("  suzaku_tool open-macos-app");
}

fn command_exists(name: &str) -> bool {
    resolve_command_path(name).is_some()
}

fn resolve_command_path(name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        return None;
    }

    env::var_os("PATH").and_then(|value| {
        env::split_paths(&value).find_map(|dir| {
            if !dir.is_absolute() {
                return None;
            }
            let candidate = dir.join(name);
            if is_executable_file(&candidate) {
                Some(candidate)
            } else {
                None
            }
        })
    })
}

fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        let mode = fs::metadata(path)
            .ok()
            .map(|metadata| metadata.permissions().mode())
            .unwrap_or(0);
        mode & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn command(name: &str) -> Option<Command> {
    resolve_command_path(name).map(Command::new)
}

fn repo_root() -> Result<PathBuf, String> {
    let mut dir = env::current_dir().map_err(|e| format!("failed to get cwd: {e}"))?;
    loop {
        if dir.join("Cargo.toml").is_file() && dir.join("src").is_dir() {
            return Ok(dir);
        }
        if !dir.pop() {
            return Err("unable to locate repository root".to_string());
        }
    }
}

fn run_status(mut cmd: Command) -> Result<(), String> {
    let status = cmd
        .status()
        .map_err(|e| format!("failed to spawn command: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("command failed with status: {status}"))
    }
}

fn check_exit_status(status: &ExitStatus, label: &str) -> Result<(), String> {
    if status.success() {
        Ok(())
    } else {
        Err(format!("{label} failed: {status}"))
    }
}

#[derive(Clone, Copy)]
enum LinuxFramework {
    IBus,
    Fcitx,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DesktopInputSourceChange {
    Unavailable,
    Unchanged,
    Updated,
}

impl LinuxFramework {
    fn from_env() -> Self {
        if env::var("SUZAKU_LINUX_IME_FRAMEWORK")
            .map(|value| value.eq_ignore_ascii_case("fcitx"))
            .unwrap_or(false)
        {
            Self::Fcitx
        } else {
            Self::IBus
        }
    }
}

fn linux_register(args: &[String]) -> i32 {
    let args = if args.first().is_some_and(|arg| arg == "--") {
        &args[1..]
    } else {
        args
    };
    if args.len() > 1 {
        eprintln!(
            "linux-register takes exactly one action; no options or extra arguments are accepted"
        );
        return 1;
    }
    let action = args.first().map(String::as_str).unwrap_or("--help");
    if matches!(action, "--help" | "-h" | "help") {
        println!("Usage: suzaku-tool linux-register [install|status|verify|uninstall|diag]");
        println!("Install/uninstall only in your desktop user session, never with sudo.");
        return 0;
    }
    if matches!(action, "install" | "uninstall" | "diag") {
        #[cfg(not(target_os = "linux"))]
        {
            eprintln!("Linux input-method registration is only available on Linux");
            return 1;
        }
        #[cfg(target_os = "linux")]
        // SAFETY: geteuid has no arguments or side effects.
        if unsafe { libc::geteuid() } == 0 {
            eprintln!("Run input-method registration as your desktop user, not root or sudo");
            return 1;
        }
    }
    let framework = LinuxFramework::from_env();
    match action {
        "install" => linux_install(framework),
        "status" => linux_status(framework),
        "verify" => linux_verify(framework),
        "uninstall" => linux_uninstall(framework),
        "diag" => {
            if linux_install(framework) != 0 {
                1
            } else {
                println!();
                linux_verify(framework)
            }
        }
        _ => {
            eprintln!("unknown linux-register action: {action}");
            print_help();
            1
        }
    }
}

fn linux_home_path() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| {
            path.is_absolute()
                && path.parent().is_some()
                && path.is_dir()
                && path
                    .to_str()
                    .is_some_and(|value| !value.chars().any(char::is_control))
        })
        .ok_or_else(|| "HOME must name an existing absolute user directory".to_string())
}

fn linux_registration_output(
    command: &mut Command,
    timeout: Duration,
) -> Result<std::process::Output, String> {
    #[cfg(target_os = "linux")]
    let result = suzaku_map::platform::linux_command::run(command, timeout);
    #[cfg(not(target_os = "linux"))]
    let result = {
        let _ = timeout;
        command.output()
    };
    result.map_err(|error| format!("Linux registration command failed: {error}"))
}

fn linux_registration_status(mut command: Command) -> Result<(), String> {
    let result = linux_registration_output(&mut command, Duration::from_secs(10))?;
    check_exit_status(&result.status, "Linux registration command")
}

fn preflight_ibus_registration(home: &Path, installing: bool) -> Result<(), String> {
    if installing {
        if !is_executable_file(Path::new("/usr/bin/env")) {
            return Err("/usr/bin/env is required to launch the IBus user service".into());
        }
        let unit = ibus_user_service_path(home);
        match fs::symlink_metadata(&unit) {
            Ok(metadata) if !metadata.is_file() => {
                return Err(format!(
                    "User service {} is masked, linked or not a regular file; it was not replaced. Resolve that explicitly before reinstalling.",
                    unit.display()
                ));
            }
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                return Err(format!("inspect user service {}: {error}", unit.display()));
            }
            _ => {}
        }
    }
    let mut systemctl =
        command("systemctl").ok_or("systemctl is required for user-level IBus registration")?;
    let result = linux_registration_output(
        systemctl.args(["--user", "show", "--property=Version", "--value"]),
        Duration::from_secs(2),
    )?;
    if !result.status.success() {
        return Err("No systemd user manager is available; run this in your desktop user session. No registration files were changed.".into());
    }
    let mut ibus = command("ibus").ok_or("ibus is required for input-method registration")?;
    let result = linux_registration_output(ibus.arg("engine"), Duration::from_secs(2))?;
    if !result.status.success() {
        return Err(
            "Could not query the current IBus engine. No registration files were changed.".into(),
        );
    }
    if String::from_utf8_lossy(&result.stdout).trim() == CONNECTION_NAME {
        return Err("Release Suzaku and finish the current composition before reinstalling or uninstalling it. No registration files were changed.".into());
    }
    Ok(())
}

fn ibus_component_path(home: &Path) -> PathBuf {
    linux_xdg_directory("XDG_DATA_HOME", home.join(".local/share"))
        .join("ibus/component")
        .join(format!("{CONNECTION_NAME}.xml"))
}

fn linux_xdg_directory(name: &str, fallback: PathBuf) -> PathBuf {
    env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or(fallback)
}

fn fcitx_config_paths(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".local/share/fcitx5/inputmethod")
            .join(FCITX_CONFIG_NAME),
        home.join(".config/fcitx/inputmethod")
            .join(FCITX_CONFIG_NAME),
        home.join(".config/fcitx5/inputmethod")
            .join(FCITX_CONFIG_NAME),
    ]
}

fn linux_install(framework: LinuxFramework) -> i32 {
    let home = match linux_home_path() {
        Ok(home) => home,
        Err(err) => {
            eprintln!("{err}");
            return 1;
        }
    };
    match framework {
        LinuxFramework::IBus => {
            if let Err(error) = preflight_ibus_registration(&home, true) {
                eprintln!("{error}");
                return 1;
            }
            let previous_engine =
                ibus_current_engine().filter(|engine| should_restore_ibus_engine(engine));
            let source_binary = match resolve_linux_ime_host_binary() {
                Ok(path) => path,
                Err(error) => {
                    eprintln!("{error}");
                    return 1;
                }
            };
            let host_binary = match install_linux_ime_host_binary(&home, &source_binary) {
                Ok(path) => path,
                Err(error) => {
                    eprintln!("{error}");
                    return 1;
                }
            };
            match write_ibus_marker(&home, &host_binary) {
                Ok(_) => {
                    println!(
                        "Wrote executable IBus component: {}",
                        ibus_component_path(&home).display()
                    );
                    println!("  host: {}", host_binary.display());
                    let service_result = install_ibus_user_service(&home, &host_binary);
                    let restore_result = previous_engine
                        .as_deref()
                        .map(restore_ibus_engine)
                        .unwrap_or(Ok(()));
                    if let Err(error) = service_result {
                        eprintln!("{error}");
                        if let Err(restore_error) = restore_result {
                            eprintln!(
                                "Additionally failed to restore the IBus engine: {restore_error}"
                            );
                        }
                        return 1;
                    }
                    if let Err(error) = restore_result {
                        eprintln!(
                            "IBus host was installed, but the previous engine could not be restored: {error}"
                        );
                        return 1;
                    }
                    match reconcile_gnome_ibus_input_source(true) {
                        Ok(DesktopInputSourceChange::Updated) => {
                            println!("Added Suzaku to the GNOME input-source switcher.");
                        }
                        Ok(DesktopInputSourceChange::Unchanged) => {
                            println!("GNOME input-source switcher already includes Suzaku.");
                        }
                        Ok(DesktopInputSourceChange::Unavailable) => {
                            println!(
                                "GNOME input-source settings are unavailable; IBus registration remains usable directly."
                            );
                        }
                        Err(error) => {
                            eprintln!(
                                "IBus host was installed, but Suzaku could not be added to the GNOME input-source switcher: {error}"
                            );
                            return 1;
                        }
                    }
                    println!(
                        "Installed user service: {IBUS_USER_SERVICE_NAME} (existing autostart preference preserved)"
                    );
                    if let Some(engine) = previous_engine {
                        println!("Preserved active IBus engine: {engine}");
                    }
                    0
                }
                Err(err) => {
                    eprintln!("{err}");
                    1
                }
            }
        }
        LinuxFramework::Fcitx => {
            for path in fcitx_config_paths(&home) {
                if let Some(parent) = path.parent()
                    && let Err(err) = fs::create_dir_all(parent)
                {
                    eprintln!(
                        "failed to create marker directory {}: {err}",
                        parent.display()
                    );
                    return 1;
                }

                let contents = "[Suzaku Linux IME]\nName=Suzaku IME\nExec=dev.suzaku.linux.ime\nDescription=Framework: Fcitx marker for Suzaku IME\n";
                if let Err(err) = fs::write(&path, contents) {
                    eprintln!("failed to write {}: {err}", path.display());
                    return 1;
                }
                println!("Wrote Fcitx marker: {}", path.display());
            }
            0
        }
    }
}

fn linux_status(framework: LinuxFramework) -> i32 {
    let home = match linux_home_path() {
        Ok(home) => home,
        Err(err) => {
            eprintln!("{err}");
            return 1;
        }
    };
    match framework {
        LinuxFramework::IBus => {
            let marker = ibus_component_path(&home);
            if ibus_component_is_valid(&marker) {
                println!("IBus: executable component is valid");
                println!("  component: {}", marker.display());
                match ibus_component_host_binary(&marker) {
                    Some(binary) if is_executable_file(&binary) => {
                        println!("  executable: {}", binary.display());
                    }
                    Some(binary) => {
                        println!("  executable missing or not runnable: {}", binary.display());
                    }
                    None => println!("  executable: invalid <exec> entry"),
                }
            } else if marker.exists() {
                println!("IBus: component exists but is incomplete or invalid");
                println!("  {}", marker.display());
            } else {
                println!("IBus: component not found at {}", marker.display());
            }

            if ibus_runtime_has_engine() {
                println!("IBus runtime exposes {CONNECTION_NAME}.");
            } else {
                println!("IBus runtime does not expose {CONNECTION_NAME}.");
            }
            match ibus_current_engine() {
                Some(engine) if engine == CONNECTION_NAME => {
                    println!("Suzaku is the active IBus engine.");
                }
                Some(engine) => println!("Active IBus engine: {engine}"),
                None => println!("Active IBus engine could not be queried."),
            }
            if process_running("linux_ime_host") {
                println!("Suzaku native host process is running.");
            } else {
                println!("Suzaku native host process is stopped.");
            }
            let unit = ibus_user_service_path(&home);
            if ibus_user_service_active() {
                println!("User service is active: {IBUS_USER_SERVICE_NAME}");
            } else if unit.exists() {
                println!("User service is installed but inactive: {}", unit.display());
            } else {
                println!("User service is not installed: {}", unit.display());
            }
        }
        LinuxFramework::Fcitx => {
            let mut found = false;
            for path in fcitx_config_paths(&home) {
                if path.exists() {
                    println!("Fcitx: marker exists");
                    println!("  {}", path.display());
                    found = true;
                }
            }
            if !found {
                println!("Fcitx: marker not found at expected paths.");
                for path in fcitx_config_paths(&home) {
                    println!("  {}", path.display());
                }
            }
        }
    }
    0
}

fn linux_verify(framework: LinuxFramework) -> i32 {
    match framework {
        LinuxFramework::IBus => {
            let home = match linux_home_path() {
                Ok(home) => home,
                Err(err) => {
                    eprintln!("{err}");
                    return 1;
                }
            };
            println!("Verifying IBus host registration (non-root).");

            let daemon_ok = process_running("ibus-daemon")
                || process_running("ibus-x11")
                || process_running("ibus-portal");
            if daemon_ok {
                println!("✓ IBus daemon process found.");
            } else {
                println!(
                    "⚠ IBus daemon process not found (you may need to start it, e.g., ibus-daemon -drx)."
                );
            }

            let marker_path = ibus_component_path(&home);
            let marker_ok = ibus_component_is_valid(&marker_path);
            if marker_ok {
                println!("✓ IBus component file is valid: {}", marker_path.display());
            } else {
                println!(
                    "⚠ IBus component file is missing or invalid: {}",
                    marker_path.display()
                );
            }

            let executable_ok = ibus_component_host_binary(&marker_path)
                .is_some_and(|binary| is_executable_file(&binary));
            if executable_ok {
                println!("✓ Component points to an executable native host.");
            } else {
                println!("⚠ Component host executable is missing or invalid.");
            }

            let runtime_ok = ibus_runtime_has_engine();
            if runtime_ok {
                println!("✓ IBus runtime exposes {CONNECTION_NAME}.");
            } else {
                println!("⚠ IBus runtime does not expose {CONNECTION_NAME} yet.");
            }

            let active = ibus_current_engine().is_some_and(|engine| engine == CONNECTION_NAME);
            println!(
                "{} Suzaku engine is {}active.",
                if active { "✓" } else { "·" },
                if active { "" } else { "not " }
            );
            let host_running = process_running("linux_ime_host");
            println!(
                "{} Native host process is {}running.",
                if host_running { "✓" } else { "·" },
                if host_running { "" } else { "not " }
            );

            let service_path = ibus_user_service_path(&home);
            let service_installed = service_path.is_file();
            let service_active = ibus_user_service_active();
            println!(
                "{} User service is {}installed and {}active.",
                if service_installed && service_active {
                    "✓"
                } else {
                    "⚠"
                },
                if service_installed { "" } else { "not " },
                if service_active { "" } else { "not " }
            );

            if marker_ok
                && executable_ok
                && runtime_ok
                && daemon_ok
                && host_running
                && service_installed
                && service_active
            {
                println!("Verdict: PASS (native IBus registration is ready).");
                0
            } else if marker_ok || runtime_ok {
                println!("Verdict: PENDING (component detected, but runtime setup is incomplete).");
                2
            } else {
                println!("Verdict: FAIL (no detectable executable component/runtime).");
                1
            }
        }
        LinuxFramework::Fcitx => {
            let home = match linux_home_path() {
                Ok(home) => home,
                Err(err) => {
                    eprintln!("{err}");
                    return 1;
                }
            };
            println!("Verifying Fcitx host registration (non-root).");

            let daemon_ok = process_running("fcitx")
                || process_running("fcitx5")
                || process_running("fcitx5-qt");
            if daemon_ok {
                println!("✓ Fcitx daemon process found.");
            } else {
                println!(
                    "⚠ Fcitx daemon process not found (you may need to start it, e.g., fcitx5 -r)."
                );
            }

            let mut marker_ok = false;
            for path in fcitx_config_paths(&home) {
                if path.exists()
                    && fs::read_to_string(&path)
                        .map(|content| content.contains("Suzaku") || content.contains("suzaku"))
                        .unwrap_or(false)
                {
                    println!("✓ Fcitx marker file detected: {}", path.display());
                    marker_ok = true;
                }
            }
            if !marker_ok {
                println!("⚠ Fcitx marker files missing from expected paths:");
                for path in fcitx_config_paths(&home) {
                    println!("  {}", path.display());
                }
            }

            if marker_ok && daemon_ok {
                println!("Verdict: PASS (host registration ready).");
                0
            } else if marker_ok {
                println!("Verdict: PENDING (registration marker exists, but daemon not running).");
                2
            } else {
                println!("Verdict: FAIL (no detectable registration marker).");
                1
            }
        }
    }
}

fn linux_uninstall(framework: LinuxFramework) -> i32 {
    let home = match linux_home_path() {
        Ok(home) => home,
        Err(err) => {
            eprintln!("{err}");
            return 1;
        }
    };
    match framework {
        LinuxFramework::IBus => {
            if let Err(error) = preflight_ibus_registration(&home, false) {
                eprintln!("{error}");
                return 1;
            }
            match reconcile_gnome_ibus_input_source(false) {
                Ok(DesktopInputSourceChange::Updated) => {
                    println!("Removed Suzaku from the GNOME input-source switcher.");
                }
                Ok(DesktopInputSourceChange::Unchanged)
                | Ok(DesktopInputSourceChange::Unavailable) => {}
                Err(error) => {
                    eprintln!(
                        "could not remove Suzaku from the GNOME input-source switcher: {error}"
                    );
                    return 1;
                }
            }
            if let Err(error) = uninstall_ibus_user_service(&home) {
                eprintln!("{error}");
                return 1;
            }
            let marker = ibus_component_path(&home);
            if marker.exists() {
                if let Err(err) = fs::remove_file(&marker) {
                    eprintln!("failed to remove {}: {err}", marker.display());
                    return 1;
                }
                println!("Removed IBus marker: {}", marker.display());
            } else {
                println!("IBus marker not present: {}", marker.display());
            }
            let installed_host = installed_linux_ime_host_path(&home);
            if installed_host.exists() {
                if let Err(error) = fs::remove_file(&installed_host) {
                    eprintln!("failed to remove {}: {error}", installed_host.display());
                    return 1;
                }
                println!("Removed installed IBus host: {}", installed_host.display());
            }
        }
        LinuxFramework::Fcitx => {
            for path in fcitx_config_paths(&home) {
                if path.exists() {
                    if let Err(err) = fs::remove_file(&path) {
                        eprintln!("failed to remove {}: {err}", path.display());
                        return 1;
                    }
                    println!("Removed Fcitx marker: {}", path.display());
                }
            }
        }
    }
    0
}

fn gnome_ibus_source_entry() -> String {
    format!("('ibus', '{CONNECTION_NAME}')")
}

fn split_gvariant_tuple_array(value: &str) -> Result<Vec<String>, String> {
    let trimmed = value.trim();
    let trimmed = trimmed
        .strip_prefix("@a(ss)")
        .map(str::trim_start)
        .unwrap_or(trimmed);
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return Err(format!("unexpected input-source value: {trimmed}"));
    }

    let inner = &trimmed[1..trimmed.len() - 1];
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    let mut start = 0;
    let mut tuple_depth = 0_i32;
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in inner.char_indices() {
        if let Some(delimiter) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == delimiter {
                quote = None;
            }
            continue;
        }

        match character {
            '\'' | '"' => quote = Some(character),
            '(' => tuple_depth += 1,
            ')' if tuple_depth > 0 => tuple_depth -= 1,
            ')' => return Err("unbalanced input-source tuple".to_string()),
            ',' if tuple_depth == 0 => {
                let entry = inner[start..index].trim();
                if entry.is_empty() {
                    return Err("empty input-source tuple".to_string());
                }
                entries.push(entry.to_string());
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    if quote.is_some() || escaped || tuple_depth != 0 {
        return Err("unterminated input-source tuple".to_string());
    }

    let entry = inner[start..].trim();
    if entry.is_empty() {
        return Err("empty input-source tuple".to_string());
    }
    entries.push(entry.to_string());
    Ok(entries)
}

fn updated_gnome_input_sources(
    value: &str,
    include_suzaku: bool,
) -> Result<Option<String>, String> {
    let mut entries = split_gvariant_tuple_array(value)?;
    let target = gnome_ibus_source_entry();
    let contains_target = entries.iter().any(|entry| entry == &target);
    if contains_target == include_suzaku {
        return Ok(None);
    }

    if include_suzaku {
        entries.push(target);
    } else {
        entries.retain(|entry| entry != &target);
    }
    Ok(Some(format!("[{}]", entries.join(", "))))
}

fn read_gnome_input_sources() -> Result<Option<String>, String> {
    let Some(mut gsettings) = command("gsettings") else {
        return Ok(None);
    };
    let output = gsettings
        .arg("get")
        .arg(GNOME_INPUT_SOURCES_SCHEMA)
        .arg(GNOME_INPUT_SOURCES_KEY)
        .output()
        .map_err(|error| format!("run `gsettings get`: {error}"))?;
    if !output.status.success() {
        return Ok(None);
    }
    String::from_utf8(output.stdout)
        .map(|value| Some(value.trim().to_string()))
        .map_err(|error| format!("GNOME input-source settings are not UTF-8: {error}"))
}

fn reconcile_gnome_ibus_input_source(
    include_suzaku: bool,
) -> Result<DesktopInputSourceChange, String> {
    let Some(current) = read_gnome_input_sources()? else {
        return Ok(DesktopInputSourceChange::Unavailable);
    };
    let Some(updated) = updated_gnome_input_sources(&current, include_suzaku)? else {
        return Ok(DesktopInputSourceChange::Unchanged);
    };
    let mut gsettings = command("gsettings")
        .ok_or_else(|| "gsettings disappeared while updating input sources".to_string())?;
    let output = gsettings
        .arg("set")
        .arg(GNOME_INPUT_SOURCES_SCHEMA)
        .arg(GNOME_INPUT_SOURCES_KEY)
        .arg(&updated)
        .output()
        .map_err(|error| format!("run `gsettings set`: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "`gsettings set` failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(DesktopInputSourceChange::Updated)
}

fn uninstall_ibus_user_service(home: &Path) -> Result<(), String> {
    let unit_path = ibus_user_service_path(home);
    if unit_path.exists() {
        if let Some(mut systemctl) = command("systemctl") {
            systemctl
                .arg("--user")
                .arg("disable")
                .arg("--now")
                .arg(IBUS_USER_SERVICE_NAME);
            linux_registration_status(systemctl)?;
        }
        fs::remove_file(&unit_path)
            .map_err(|error| format!("remove {}: {error}", unit_path.display()))?;
        println!("Removed user service: {}", unit_path.display());
    }

    if let Some(mut systemctl) = command("systemctl") {
        systemctl.arg("--user").arg("daemon-reload");
        linux_registration_status(systemctl)?;
    }
    Ok(())
}

fn ibus_runtime_has_engine() -> bool {
    let statically_registered = command("ibus")
        .and_then(|mut command| command.arg("list-engine").output().ok())
        .filter(|output| output.status.success())
        .is_some_and(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .any(|line| line.split_whitespace().next() == Some(CONNECTION_NAME))
        });
    statically_registered || ibus_dynamic_registry_has_engine()
}

fn ibus_dynamic_registry_has_engine() -> bool {
    let Some(address_output) =
        command("ibus").and_then(|mut command| command.arg("address").output().ok())
    else {
        return false;
    };
    if !address_output.status.success() {
        return false;
    }
    let address = String::from_utf8_lossy(&address_output.stdout);
    let address = address.trim();
    if address.is_empty() {
        return false;
    }

    command("gdbus")
        .and_then(|mut command| {
            command
                .arg("call")
                .arg("--address")
                .arg(address)
                .arg("--dest")
                .arg("org.freedesktop.IBus")
                .arg("--object-path")
                .arg("/org/freedesktop/IBus")
                .arg("--method")
                .arg("org.freedesktop.DBus.Properties.Get")
                .arg("org.freedesktop.IBus")
                .arg("ActiveEngines")
                .output()
                .ok()
        })
        .filter(|output| output.status.success())
        .is_some_and(|output| String::from_utf8_lossy(&output.stdout).contains(CONNECTION_NAME))
}

fn ibus_current_engine() -> Option<String> {
    let output = command("ibus")?.arg("engine").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let engine = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!engine.is_empty()).then_some(engine)
}

fn should_restore_ibus_engine(engine: &str) -> bool {
    let engine = engine.trim();
    !engine.is_empty() && engine != "dummy"
}

fn restore_ibus_engine(engine: &str) -> Result<(), String> {
    const MAX_ATTEMPTS: usize = 40;
    const RETRY_DELAY: Duration = Duration::from_millis(50);

    if ibus_current_engine().as_deref() == Some(engine) {
        return Ok(());
    }

    let mut last_error = "IBus did not report the requested engine".to_string();
    for attempt in 0..MAX_ATTEMPTS {
        if ibus_current_engine().as_deref() == Some(engine) {
            return Ok(());
        }

        let mut ibus = command("ibus").ok_or_else(|| "ibus command not found".to_string())?;
        let output = ibus
            .arg("engine")
            .arg(engine)
            .output()
            .map_err(|error| format!("failed to run `ibus engine`: {error}"))?;
        if ibus_current_engine().as_deref() == Some(engine) {
            return Ok(());
        }

        if output.status.success() {
            last_error = "IBus accepted the engine switch but did not confirm it".to_string();
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            last_error = format!(
                "`ibus engine {engine}` failed with {}: {}",
                output.status,
                stderr.trim()
            );
        }

        if attempt + 1 < MAX_ATTEMPTS {
            std::thread::sleep(RETRY_DELAY);
        }
    }

    if ibus_current_engine().as_deref() == Some(engine) {
        return Ok(());
    }

    Err(last_error)
}

fn ibus_component_is_valid(path: &Path) -> bool {
    let Ok(contents) = fs::read_to_string(path) else {
        return false;
    };
    xml_tag_value(&contents, "name").as_deref() == Some(IBUS_COMPONENT_NAME)
        && contents.contains(&format!("<name>{CONNECTION_NAME}</name>"))
        && ibus_component_host_binary_from_contents(&contents).is_some()
}

fn ibus_component_host_binary(path: &Path) -> Option<PathBuf> {
    let contents = fs::read_to_string(path).ok()?;
    ibus_component_host_binary_from_contents(&contents)
}

fn ibus_component_host_binary_from_contents(contents: &str) -> Option<PathBuf> {
    let command = xml_unescape(&xml_tag_value(contents, "exec")?);
    let path = command.strip_suffix(" --ibus").unwrap_or(&command).trim();
    if path.starts_with('\'') {
        let inner = path.strip_prefix('\'')?.strip_suffix('\'')?;
        let decoded = inner.replace("'\\''", "'");
        return (ibus_quote(&decoded) == path).then(|| PathBuf::from(decoded));
    }
    // Continue reading components written by previous releases.
    (!path.is_empty()).then(|| PathBuf::from(path))
}

fn xml_tag_value(contents: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{tag}>");
    let end_tag = format!("</{tag}>");
    let start = contents.find(&start_tag)? + start_tag.len();
    let end = contents[start..].find(&end_tag)? + start;
    Some(contents[start..end].trim().to_string())
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&apos;", "'")
        .replace("&quot;", "\"")
        .replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&amp;", "&")
}

fn resolve_linux_ime_host_binary() -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("SUZAKU_LINUX_IME_HOST_BIN") {
        let path = PathBuf::from(path);
        if !is_executable_file(&path) {
            return Err(format!(
                "Configured SUZAKU_LINUX_IME_HOST_BIN is not executable: {}",
                path.display()
            ));
        }
        return fs::canonicalize(&path)
            .map_err(|error| format!("resolve {}: {error}", path.display()));
    }
    if let Ok(current_exe) = env::current_exe()
        && let Some(parent) = current_exe.parent()
    {
        candidates.push(parent.join("linux_ime_host"));
    }
    if let Ok(root) = repo_root() {
        candidates.push(root.join("target/release/linux_ime_host"));
        candidates.push(root.join("target/debug/linux_ime_host"));
    }

    for candidate in candidates {
        if !is_executable_file(&candidate) {
            continue;
        }
        return fs::canonicalize(&candidate)
            .map_err(|error| format!("resolve {}: {error}", candidate.display()));
    }

    Err(
        "linux_ime_host was not found. Build it first with `cargo build --features linux-ibus --bin linux_ime_host`, or set SUZAKU_LINUX_IME_HOST_BIN."
            .to_string(),
    )
}

fn ibus_user_service_path(home: &Path) -> PathBuf {
    linux_xdg_directory("XDG_CONFIG_HOME", home.join(".config"))
        .join("systemd/user")
        .join(IBUS_USER_SERVICE_NAME)
}

fn installed_linux_ime_host_path(home: &Path) -> PathBuf {
    home.join(".local/libexec/suzaku/linux_ime_host")
}

fn install_linux_ime_host_binary(home: &Path, source: &Path) -> Result<PathBuf, String> {
    // The Debian package owns this binary. Point the user's service to it directly so
    // upgrades do not leave a stale per-user copy shadowing the new package version.
    if source == Path::new("/usr/lib/suzaku/linux_ime_host") && is_executable_file(source) {
        return Ok(source.into());
    }
    let destination = installed_linux_ime_host_path(home);
    if fs::canonicalize(source).ok().as_ref() == fs::canonicalize(&destination).ok().as_ref()
        && is_executable_file(&destination)
    {
        return Ok(destination);
    }

    let Some(parent) = destination.parent() else {
        return Err("unable to resolve Linux IME host install directory".to_string());
    };
    fs::create_dir_all(parent).map_err(|error| format!("create {}: {error}", parent.display()))?;
    static NEXT_HOST_COPY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    for _ in 0..32 {
        let temporary = parent.join(format!(
            ".linux_ime_host-new-{}-{}",
            std::process::id(),
            NEXT_HOST_COPY.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o700);
        }
        let mut staged = match options.open(&temporary) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("stage {}: {error}", destination.display())),
        };
        let copied = (|| -> std::io::Result<()> {
            std::io::copy(&mut fs::File::open(source)?, &mut staged)?;
            #[cfg(unix)]
            staged.set_permissions(fs::Permissions::from_mode(0o755))?;
            staged.sync_all()?;
            fs::rename(&temporary, &destination)
        })();
        if let Err(error) = copied {
            let _ = fs::remove_file(&temporary);
            return Err(format!(
                "install {} as {}: {error}",
                source.display(),
                destination.display()
            ));
        }
        return Ok(destination);
    }
    Err("unable to reserve a private host installation temporary file".into())
}

fn systemd_quote(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
            .replace('$', "$$")
    )
}

fn install_ibus_user_service(home: &Path, host_binary: &Path) -> Result<(), String> {
    let unit_path = ibus_user_service_path(home);
    let newly_installed = !unit_path.exists();
    let Some(parent) = unit_path.parent() else {
        return Err("unable to resolve systemd user unit directory".to_string());
    };
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "create systemd user unit directory {}: {error}",
            parent.display()
        )
    })?;
    let executable = host_binary.to_str().ok_or_else(|| {
        format!(
            "IBus host path is not valid UTF-8: {}",
            host_binary.display()
        )
    })?;
    // systemd rejects literal quotes/backslashes in ExecStart's executable even
    // when quoted correctly. env directly execs the absolute host path as an
    // argument, without a shell or PATH lookup, and $$ survives as a literal $.
    let unit = format!(
        "[Unit]\nDescription=Suzaku native IBus engine host\nAfter=graphical-session.target\n\n[Service]\nType=simple\nExecStart=/usr/bin/env -- {} --ibus\nRestart=always\nRestartSec=1\nTimeoutStopSec=5\n\n[Install]\nWantedBy=default.target\n",
        systemd_quote(executable)
    );
    suzaku_map::data::files::atomic_write(&unit_path, unit.as_bytes())
        .map_err(|error| format!("write {}: {error}", unit_path.display()))?;

    let mut reload = command("systemctl")
        .ok_or_else(|| "systemctl is required for user-level IBus registration".to_string())?;
    reload.arg("--user").arg("daemon-reload");
    linux_registration_status(reload)?;

    if newly_installed {
        let mut enable = command("systemctl")
            .ok_or_else(|| "systemctl is required for user-level IBus registration".to_string())?;
        enable
            .arg("--user")
            .arg("enable")
            .arg(IBUS_USER_SERVICE_NAME);
        linux_registration_status(enable)?;
    }

    let mut restart = command("systemctl")
        .ok_or_else(|| "systemctl is required for user-level IBus registration".to_string())?;
    restart
        .arg("--user")
        .arg("restart")
        .arg(IBUS_USER_SERVICE_NAME);
    linux_registration_status(restart)
}

fn ibus_user_service_active() -> bool {
    command("systemctl")
        .and_then(|mut command| {
            command
                .arg("--user")
                .arg("is-active")
                .arg("--quiet")
                .arg(IBUS_USER_SERVICE_NAME)
                .output()
                .ok()
        })
        .is_some_and(|output| output.status.success())
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn write_ibus_marker(home: &Path, host_binary: &Path) -> Result<(), String> {
    let path = ibus_component_path(home);
    write_ibus_marker_at(&path, host_binary)
}

fn ibus_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn write_ibus_marker_at(path: &Path, host_binary: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create dir {}: {e}", parent.display()))?;
    }
    let host_binary = host_binary.to_str().ok_or_else(|| {
        format!(
            "IBus host path is not valid UTF-8: {}",
            host_binary.display()
        )
    })?;
    let marker = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<component>
  <name>{IBUS_COMPONENT_NAME}</name>
  <description>Suzaku adaptive input method</description>
  <exec>{host_binary} --ibus</exec>
  <version>{version}</version>
  <author>Suzaku contributors</author>
  <license>MIT</license>
  <homepage>https://github.com/chiharu-kiryu/Suzaku-map</homepage>
  <textdomain>suzaku-map</textdomain>
  <engines>
    <engine>
      <name>{CONNECTION_NAME}</name>
      <longname>Suzaku</longname>
      <description>Suzaku adaptive candidate engine</description>
      <language>zh</language>
      <license>MIT</license>
      <author>Suzaku contributors</author>
      <layout>default</layout>
      <icon>input-keyboard</icon>
      <rank>80</rank>
      <symbol>朱</symbol>
    </engine>
  </engines>
</component>
"#,
        host_binary = xml_escape(&ibus_quote(host_binary)),
        version = env!("CARGO_PKG_VERSION"),
    );
    suzaku_map::data::files::atomic_write(path, marker.as_bytes())
        .map_err(|e| format!("write {}: {e}", path.display()))
}

fn process_running(name: &str) -> bool {
    if !command_exists("pgrep") {
        return false;
    }
    if let Some(mut cmd) = command("pgrep") {
        cmd.arg("-x")
            .arg(name)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    } else {
        false
    }
}

struct AndroidEnv {
    android_sdk_root: PathBuf,
    android_home: PathBuf,
    java_home: PathBuf,
    android_ndk_home: PathBuf,
    suzaku_android_ndk_bin: PathBuf,
}

fn detect_latest_ndk(sdk_root: &Path) -> Result<PathBuf, String> {
    let ndk_root = sdk_root.join("ndk");
    let mut versions: Vec<PathBuf> = fs::read_dir(&ndk_root)
        .map_err(|e| format!("unable to read {}: {e}", ndk_root.display()))?
        .filter_map(|entry| {
            entry.ok().and_then(|e| {
                let path = e.path();
                if path.is_dir() { Some(path) } else { None }
            })
        })
        .collect();
    versions.sort();
    versions
        .pop()
        .ok_or_else(|| "no ndk folder found under $ANDROID_SDK_ROOT/ndk".to_string())
}

fn resolve_java_home() -> PathBuf {
    if let Some(java_home) = env::var_os("JAVA_HOME")
        .map(PathBuf::from)
        .filter(|path| is_jdk_home(path))
    {
        return java_home;
    }

    ["javac", "java"]
        .into_iter()
        .find_map(java_home_from_command)
        .filter(|path| is_jdk_home(path))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_JAVA_HOME))
}

fn java_home_from_command(command: &str) -> Option<PathBuf> {
    resolve_command_path(command)
        .and_then(|path| fs::canonicalize(path).ok())
        .and_then(|path| path.parent()?.parent().map(Path::to_path_buf))
}

fn is_jdk_home(path: &Path) -> bool {
    is_executable_file(&path.join("bin/java")) && is_executable_file(&path.join("bin/javac"))
}

fn resolve_android_env() -> Result<AndroidEnv, String> {
    let android_sdk_root = env::var("ANDROID_SDK_ROOT")
        .or_else(|_| env::var("ANDROID_HOME"))
        .unwrap_or_else(|_| DEFAULT_ANDROID_SDK_ROOT.to_string());
    let android_home = env::var("ANDROID_HOME").unwrap_or_else(|_| android_sdk_root.clone());
    if !Path::new(&android_sdk_root).is_dir() {
        return Err(format!("ANDROID_SDK_ROOT not found: {android_sdk_root}"));
    }

    let java_home = resolve_java_home();
    if !is_jdk_home(&java_home) {
        return Err(format!(
            "JDK not found (both java and javac are required): {}",
            java_home.display()
        ));
    }

    let android_ndk_home = detect_latest_ndk(Path::new(&android_sdk_root))?;
    let host_tag = if cfg!(target_os = "macos") {
        "darwin-x86_64"
    } else if cfg!(target_os = "linux") {
        "linux-x86_64"
    } else {
        "windows-x86_64"
    };
    let suzaku_android_ndk_bin = android_ndk_home
        .join("toolchains")
        .join("llvm")
        .join("prebuilt")
        .join(host_tag)
        .join("bin");
    if !suzaku_android_ndk_bin.is_dir() {
        return Err(format!(
            "NDK LLVM toolchain not found: {}",
            suzaku_android_ndk_bin.display()
        ));
    }

    Ok(AndroidEnv {
        android_sdk_root: PathBuf::from(android_sdk_root),
        android_home: PathBuf::from(android_home),
        java_home,
        android_ndk_home,
        suzaku_android_ndk_bin,
    })
}

fn android_env(args: &[String]) -> i32 {
    let _ = args;
    let env = match resolve_android_env() {
        Ok(env) => env,
        Err(err) => {
            eprintln!("{err}");
            return 1;
        }
    };

    println!("export ANDROID_SDK_ROOT={}", env.android_sdk_root.display());
    println!("export ANDROID_HOME={}", env.android_home.display());
    println!("export JAVA_HOME={}", env.java_home.display());
    println!("export ANDROID_NDK_HOME={}", env.android_ndk_home.display());
    println!(
        "export SUZAKU_ANDROID_NDK_BIN={}",
        env.suzaku_android_ndk_bin.display()
    );
    println!(
        "export PATH={}:{}",
        env.java_home.join("bin").display(),
        env_var_path()
    );
    0
}

fn env_var_path() -> String {
    env::var("PATH").unwrap_or_default()
}

fn run_android_build_native_with_flags(release_flag: bool) -> i32 {
    let profile = if release_flag {
        "release".to_string()
    } else {
        env::var("SUZAKU_ANDROID_PROFILE").unwrap_or_else(|_| DEFAULT_ANDROID_PROFILE.to_string())
    };

    let root = match repo_root() {
        Ok(root) => root,
        Err(err) => {
            eprintln!("{err}");
            return 1;
        }
    };
    let env = match resolve_android_env() {
        Ok(resolved) => resolved,
        Err(err) => {
            eprintln!("{err}");
            return 1;
        }
    };

    let specs = [
        (
            "arm64-v8a",
            "aarch64-linux-android",
            "aarch64-linux-android29-clang",
            "CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER",
        ),
        (
            "armeabi-v7a",
            "armv7-linux-androideabi",
            "armv7a-linux-androideabi29-clang",
            "CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER",
        ),
        (
            "x86",
            "i686-linux-android",
            "i686-linux-android29-clang",
            "CARGO_TARGET_I686_LINUX_ANDROID_LINKER",
        ),
        (
            "x86_64",
            "x86_64-linux-android",
            "x86_64-linux-android29-clang",
            "CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER",
        ),
    ];

    for (abi, rust_target, clang, linker_var) in specs {
        if let Err(err) = run_android_one_target(
            &root,
            &env,
            &profile,
            abi,
            rust_target,
            clang,
            linker_var,
            release_flag,
        ) {
            eprintln!("{err}");
            return 1;
        }
    }

    println!("Rust Android libraries copied into android/app/src/main/jniLibs");
    0
}

fn run_android_one_target(
    root: &Path,
    env: &AndroidEnv,
    profile: &str,
    abi: &str,
    rust_target: &str,
    clang: &str,
    linker_var: &str,
    release_flag: bool,
) -> Result<(), String> {
    let target_path = root
        .join(format!("target/{rust_target}/{profile}"))
        .join("libsuzaku_map.so");
    let out_dir = root.join("android/app/src/main/jniLibs").join(abi);
    fs::create_dir_all(&out_dir).map_err(|e| format!("create {}: {e}", out_dir.display()))?;

    let mut build_cmd = match command("cargo") {
        Some(cmd) => cmd,
        None => return Err("cargo command not found".to_string()),
    };
    build_cmd
        .current_dir(root)
        .arg("build")
        .arg("--features")
        .arg("gpu")
        .arg("--target")
        .arg(rust_target);
    if release_flag {
        build_cmd.arg("--release");
    }
    let linker = env.suzaku_android_ndk_bin.join(clang);
    let status_cmd = build_cmd
        .env(linker_var, linker)
        .status()
        .map_err(|e| format!("cargo build failed for {rust_target}: {e}"))?;
    check_exit_status(&status_cmd, &format!("cargo build for {abi}"))?;

    if !target_path.exists() {
        return Err(format!(
            "expected Rust library missing: {}",
            target_path.display()
        ));
    }
    fs::copy(&target_path, out_dir.join("libsuzaku_map.so"))
        .map_err(|e| format!("copy library to {} failed: {e}", out_dir.display()))
        .map(|_| ())
}

fn android_dispatch(args: &[String]) -> i32 {
    if args.is_empty() {
        print_help();
        return 1;
    }
    match args[0].as_str() {
        "env" => android_env(&args[1..]),
        "build-native" => {
            let release = args.iter().any(|arg| arg == "--release");
            run_android_build_native_with_flags(release)
        }
        "install-debug" => android_install_debug(&args[1..]),
        "enable-ime" => android_enable_ime(&args[1..]),
        _ => {
            eprintln!("unknown android command: {}", args[0]);
            print_help();
            1
        }
    }
}

fn android_install_debug(_args: &[String]) -> i32 {
    let root = match repo_root() {
        Ok(root) => root,
        Err(err) => {
            eprintln!("{err}");
            return 1;
        }
    };
    let apk_path = match resolve_android_apk_path(&root) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("{err}");
            eprintln!("Run ./gradlew assembleDebug in android/ first.");
            return 1;
        }
    };
    if !command_exists("adb") {
        eprintln!("adb command not found.");
        return 1;
    }
    let state = match command("adb") {
        Some(mut cmd) => cmd
            .arg("get-state")
            .status()
            .map_err(|e| format!("failed to run adb: {e}")),
        None => Err("adb command not found".to_string()),
    };
    if let Ok(status) = state {
        if !status.success() {
            eprintln!("No Android device or emulator is online.");
            return 1;
        }
    } else {
        eprintln!("No Android device or emulator is online.");
        return 1;
    }

    if let Some(mut cmd) = command("adb") {
        cmd.arg("install").arg("-r").arg(apk_path.clone());
        if let Err(err) = run_status(cmd) {
            eprintln!("{err}");
            return 1;
        }
    } else {
        eprintln!("adb command not found.");
        return 1;
    }
    println!("Installed: {}", apk_path.display());
    0
}

fn resolve_android_apk_path(root: &Path) -> Result<PathBuf, String> {
    let apk_path = match env::var("SUZAKU_ANDROID_APK_PATH") {
        Ok(path) => PathBuf::from(path),
        Err(_) => root.join("android/app/build/outputs/apk/debug/app-debug.apk"),
    };
    let canonical = fs::canonicalize(&apk_path)
        .map_err(|_| format!("Debug APK not found: {}", apk_path.display()))?;
    if !canonical.is_file() {
        return Err(format!("Debug APK not found: {}", canonical.display()));
    }
    if canonical
        .extension()
        .and_then(|ext| ext.to_str())
        .is_none_or(|ext| ext.to_ascii_lowercase() != "apk")
    {
        return Err(format!(
            "Invalid APK path (must be *.apk): {}",
            canonical.display()
        ));
    }
    Ok(canonical)
}

fn android_enable_ime(_args: &[String]) -> i32 {
    const IME_ID: &str = "dev.suzaku.android.ime/.SuzakuInputMethodService";
    if !command_exists("adb") {
        eprintln!("adb command not found.");
        return 1;
    }
    let state = match command("adb") {
        Some(mut cmd) => cmd
            .arg("get-state")
            .status()
            .map_err(|e| format!("failed to run adb: {e}")),
        None => Err("adb command not found".to_string()),
    };
    if let Ok(status) = state {
        if !status.success() {
            eprintln!("No Android device or emulator is online.");
            return 1;
        }
    } else {
        eprintln!("No Android device or emulator is online.");
        return 1;
    }
    if let Some(mut cmd) = command("adb") {
        cmd.arg("shell").arg("ime").arg("enable").arg(IME_ID);
        if let Err(err) = run_status(cmd) {
            eprintln!("{err}");
            return 1;
        }
    } else {
        eprintln!("adb command not found.");
        return 1;
    }
    if let Some(mut cmd) = command("adb") {
        cmd.arg("shell").arg("ime").arg("set").arg(IME_ID);
        if let Err(err) = run_status(cmd) {
            eprintln!("{err}");
            return 1;
        }
    } else {
        eprintln!("adb command not found.");
        return 1;
    }
    println!("Enabled and selected IME: {IME_ID}");
    0
}

#[derive(Clone, Copy)]
enum MacTarget {
    Panel,
    Ime,
}

fn macos_dispatch(args: &[String]) -> i32 {
    if args.is_empty() {
        print_help();
        return 1;
    }
    let target = if args.get(1).is_some() {
        parse_macos_target(args[1].as_str())
    } else {
        parse_macos_target("panel")
    };

    match args[0].as_str() {
        "build-app" => macos_build_app_for(target),
        "open-app" => macos_open_app_for(target),
        _ => {
            eprintln!("unknown macos command: {}", args[0]);
            print_help();
            1
        }
    }
}

fn parse_macos_target(arg: &str) -> MacTarget {
    if arg == "ime" {
        MacTarget::Ime
    } else {
        MacTarget::Panel
    }
}

fn macos_build_app() -> i32 {
    macos_build_app_for(MacTarget::Panel)
}

fn macos_build_app_for(target: MacTarget) -> i32 {
    let root = match repo_root() {
        Ok(root) => root,
        Err(err) => {
            eprintln!("{err}");
            return 1;
        }
    };

    let (binary, app_name, plist) = match target {
        MacTarget::Panel => ("panel", "Suzaku Panel", "src/macos/SuzakuPanel-Info.plist"),
        MacTarget::Ime => (
            "macos_ime_host",
            "Suzaku Input Method",
            "src/macos/SuzakuInputMethod-Info.plist",
        ),
    };

    let mut build = match command("cargo") {
        Some(cmd) => cmd,
        None => {
            eprintln!("cargo command not found.");
            return 1;
        }
    };
    build
        .current_dir(&root)
        .arg("build")
        .arg("--bin")
        .arg(binary);
    if matches!(target, MacTarget::Panel) {
        build.arg("--features").arg("gpu");
    }
    let status = match build.status() {
        Ok(status) => status,
        Err(err) => {
            eprintln!("failed to run cargo build for {binary}: {err}");
            return 1;
        }
    };
    if let Err(err) = check_exit_status(&status, &format!("cargo build for {binary}")) {
        eprintln!("{err}");
        return 1;
    }

    let target_dir = root.join("target/debug");
    let app_dir = target_dir.join(format!("{}.app", app_name));
    let contents = app_dir.join("Contents");
    let macos = contents.join("MacOS");
    let resources = contents.join("Resources");
    let bin_path = target_dir.join(binary);
    let app_bin = macos.join(app_name);
    if let Err(err) = fs::create_dir_all(&macos) {
        eprintln!("failed to create {}: {err}", macos.display());
        return 1;
    }
    if let Err(err) = fs::create_dir_all(&resources) {
        eprintln!("failed to create {}: {err}", resources.display());
        return 1;
    }
    let plist_src = root.join(plist);
    if let Err(err) = fs::copy(plist_src, contents.join("Info.plist")) {
        eprintln!("failed to write Info.plist: {err}");
        return 1;
    }
    if let Err(err) = fs::copy(&bin_path, &app_bin) {
        eprintln!("failed to copy binary to bundle: {err}");
        return 1;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(mut perms) = fs::metadata(&app_bin).map(|m| m.permissions()) {
            perms.set_mode(0o755);
            let _ = fs::set_permissions(&app_bin, perms);
        }
    }

    println!("{}", app_dir.display());
    0
}

fn macos_open_app() -> i32 {
    macos_open_app_for(MacTarget::Panel)
}

fn macos_open_app_for(target: MacTarget) -> i32 {
    let app = match target {
        MacTarget::Panel => PathBuf::from("target/debug/Suzaku Panel.app"),
        MacTarget::Ime => PathBuf::from("target/debug/Suzaku Input Method.app"),
    };
    let app = match repo_root() {
        Ok(root) => root.join(app),
        Err(err) => {
            eprintln!("{err}");
            return 1;
        }
    };
    if !app.is_dir() {
        if macos_build_app_for(target) != 0 {
            return 1;
        }
    }
    if let Some(mut open_cmd) = command("open") {
        if let Err(err) = open_cmd.arg(&app).status() {
            eprintln!("failed to open {}: {err}", app.display());
            return 1;
        }
    } else {
        eprintln!("open command not found.");
        return 1;
    }
    println!("{}", app.display());
    0
}

#[cfg(test)]
mod tests {
    use super::{
        CONNECTION_NAME, IBUS_COMPONENT_NAME, ibus_component_host_binary, ibus_component_is_valid,
        install_linux_ime_host_binary, should_restore_ibus_engine, split_gvariant_tuple_array,
        updated_gnome_input_sources, write_ibus_marker_at, xml_escape, xml_unescape,
    };
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_test_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!("suzaku-{label}-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn xml_escape_round_trips_component_paths() {
        let path = "/tmp/Suzaku & friends/'host'";
        assert_eq!(xml_unescape(&xml_escape(path)), path);
    }

    #[test]
    fn only_real_ibus_engines_are_preserved_during_service_restart() {
        assert!(should_restore_ibus_engine("rime"));
        assert!(should_restore_ibus_engine("xkb:us::eng"));
        assert!(should_restore_ibus_engine("  rime  "));
        assert!(!should_restore_ibus_engine(""));
        assert!(!should_restore_ibus_engine("   "));
        assert!(!should_restore_ibus_engine("dummy"));
    }

    #[test]
    fn gnome_input_source_update_preserves_existing_sources_and_order() {
        let current = "[('xkb', 'us+symbolic'), ('ibus', 'rime'), ('ibus', 'mozc-jp')]";
        let updated = updated_gnome_input_sources(current, true)
            .expect("parse sources")
            .expect("add Suzaku");

        assert_eq!(
            updated,
            "[('xkb', 'us+symbolic'), ('ibus', 'rime'), ('ibus', 'mozc-jp'), ('ibus', 'dev.suzaku.linux.ime')]"
        );
        assert_eq!(
            updated_gnome_input_sources(&updated, true).expect("parse updated sources"),
            None
        );
    }

    #[test]
    fn gnome_input_source_update_only_removes_suzaku() {
        let current =
            "[('xkb', 'us+symbolic'), ('ibus', 'dev.suzaku.linux.ime'), ('ibus', 'rime')]";
        let updated = updated_gnome_input_sources(current, false)
            .expect("parse sources")
            .expect("remove Suzaku");

        assert_eq!(updated, "[('xkb', 'us+symbolic'), ('ibus', 'rime')]");
    }

    #[test]
    fn gnome_input_source_parser_accepts_typed_empty_arrays() {
        assert!(split_gvariant_tuple_array("@a(ss) []").unwrap().is_empty());
        assert_eq!(
            updated_gnome_input_sources("@a(ss) []", true).unwrap(),
            Some("[('ibus', 'dev.suzaku.linux.ime')]".to_string())
        );
    }

    #[cfg(unix)]
    #[test]
    fn ibus_component_contains_real_factory_executable() {
        let home = unique_test_dir("ibus-component");
        let host = home.join("bin/linux_ime_host");
        fs::create_dir_all(host.parent().expect("host parent")).expect("create host parent");
        fs::write(&host, b"test host").expect("write test host");
        let mut permissions = fs::metadata(&host).expect("host metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&host, permissions).expect("make test host executable");

        let component = home
            .join(".local/share/ibus/component")
            .join(format!("{CONNECTION_NAME}.xml"));
        write_ibus_marker_at(&component, &host).expect("write component");
        let contents = fs::read_to_string(&component).expect("read component");

        assert!(ibus_component_is_valid(&component));
        assert_eq!(ibus_component_host_binary(&component), Some(host));
        assert!(contents.contains(&format!("<name>{IBUS_COMPONENT_NAME}</name>")));
        assert!(contents.contains(&format!("<name>{CONNECTION_NAME}</name>")));
        assert!(contents.contains("<exec>"));
        assert!(!contents.contains("\\\""));

        fs::remove_dir_all(home).expect("remove test home");
    }

    #[cfg(unix)]
    #[test]
    fn linux_ime_host_installs_into_user_libexec() {
        let home = unique_test_dir("ibus-host-install");
        let source = home.join("build/linux_ime_host");
        fs::create_dir_all(source.parent().expect("source parent")).expect("create source parent");
        fs::write(&source, b"native host binary").expect("write source host");
        let mut permissions = fs::metadata(&source)
            .expect("source metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&source, permissions).expect("make source executable");

        let installed = install_linux_ime_host_binary(&home, &source).expect("install native host");

        assert_eq!(installed, home.join(".local/libexec/suzaku/linux_ime_host"));
        assert_eq!(
            fs::read(&installed).expect("read installed host"),
            b"native host binary"
        );
        assert!(fs::metadata(&installed).unwrap().permissions().mode() & 0o111 != 0);

        fs::remove_dir_all(home).expect("remove test home");
    }

    #[cfg(unix)]
    #[test]
    fn host_update_never_follows_a_leftover_temporary_symlink() {
        let home = unique_test_dir("ibus-safe-upgrade");
        let directory = home.join(".local/libexec/suzaku");
        fs::create_dir_all(&directory).unwrap();
        let unrelated = home.join("unrelated.txt");
        fs::write(&unrelated, b"keep this file").unwrap();
        let stale = directory.join(format!("linux_ime_host.new.{}", std::process::id()));
        std::os::unix::fs::symlink(&unrelated, &stale).unwrap();
        let source = home.join("source");
        fs::write(&source, b"new host").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o755)).unwrap();
        let installed = install_linux_ime_host_binary(&home, &source).unwrap();
        assert_eq!(fs::read(&installed).unwrap(), b"new host");
        assert_eq!(fs::read(&unrelated).unwrap(), b"keep this file");
        assert!(fs::symlink_metadata(stale).unwrap().is_symlink());
        assert_eq!(
            fs::metadata(&installed).unwrap().permissions().mode() & 0o777,
            0o755
        );
        // Failed preparation must retain the previous executable and clean its own temp file.
        assert!(install_linux_ime_host_binary(&home, &home.join("missing")).is_err());
        assert_eq!(fs::read(&installed).unwrap(), b"new host");
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 2);
        fs::remove_dir_all(home).unwrap();
    }
}
