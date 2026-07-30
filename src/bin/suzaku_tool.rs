use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, exit};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const CONNECTION_NAME: &str = "dev.suzaku.linux.ime";
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
    println!("  suzaku_tool linux-register [install|status|verify|uninstall|diag]");
    println!("  suzaku_tool linux-register-ime [install|status|verify|uninstall|diag]");
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
    let status = cmd.status().map_err(|e| format!("failed to spawn command: {e}"))?;
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
    let action = args.first().map(String::as_str).unwrap_or("install");
    let framework = LinuxFramework::from_env();
    let rc = match action {
        "install" => linux_install(framework),
        "status" => linux_status(framework),
        "verify" => linux_verify(framework),
        "uninstall" => linux_uninstall(framework),
        "diag" => {
            if let Err(err) = linux_install(framework) {
                eprintln!("{err}");
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
    };
    rc
}

fn linux_home_path() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME not set".to_string())
}

fn ibus_component_path(home: &Path) -> PathBuf {
    home.join(".local/share/ibus/component")
        .join(format!("{CONNECTION_NAME}.xml"))
}

fn fcitx_config_paths(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".local/share/fcitx5/inputmethod")
            .join(FCITX_CONFIG_NAME),
        home.join(".config/fcitx/inputmethod").join(FCITX_CONFIG_NAME),
        home.join(".config/fcitx5/inputmethod").join(FCITX_CONFIG_NAME),
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
        LinuxFramework::IBus => match write_ibus_marker(&home) {
            Ok(_) => {
                println!(
                    "Wrote IBus marker: {}",
                    ibus_component_path(&home).display()
                );
                if command_exists("ibus") {
                    println!("Attempting to restart IBus runtime...");
                    if let Some(mut cmd) = command("ibus") {
                        let _ = cmd.arg("restart").status();
                    }
                }
                0
            }
            Err(err) => {
                eprintln!("{err}");
                1
            }
        },
        LinuxFramework::Fcitx => {
            for path in fcitx_config_paths(&home) {
                if let Some(parent) = path.parent() {
                    if let Err(err) = fs::create_dir_all(parent) {
                        eprintln!("failed to create marker directory {}: {err}", parent.display());
                        return 1;
                    }
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
            if marker.exists() {
                println!("IBus: marker exists");
                println!("  {}", marker.display());
            } else {
                println!("IBus: marker not found at {}", marker.display());
            }
            if command_exists("ibus") {
                match command("ibus").and_then(|mut cmd| cmd.arg("list-engine").output().ok()) {
                    Ok(response) => {
                        if response.status.success() {
                            let stdout = String::from_utf8_lossy(&response.stdout);
                            if stdout
                                .lines()
                                .any(|line| line.trim().split_whitespace().next() == Some(CONNECTION_NAME))
                            {
                                println!("IBus runtime list contains {CONNECTION_NAME}.");
                            } else {
                                println!(
                                    "IBus runtime list does not include {CONNECTION_NAME} yet (file marker may still be enough)."
                                );
                            }
                        } else {
                            println!("ibus list-engine returned non-zero exit.");
                        }
                    }
                    Err(_) => println!("Failed to run ibus list-engine."),
                }
            } else {
                println!("ibus command unavailable; skipped runtime list check.");
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
            let marker_ok = marker_path.exists()
                && fs::read_to_string(&marker_path)
                    .map(|content| content.contains(CONNECTION_NAME))
                    .unwrap_or(false);
            if marker_ok {
                println!("✓ IBus marker file exists: {}", marker_path.display());
            } else {
                println!("⚠ IBus marker file missing or invalid: {}", marker_path.display());
            }

            let runtime_ok = if command_exists("ibus") {
                if let Some(output) = command("ibus").and_then(|mut cmd| cmd.arg("list-engine").output().ok()) {
                    if output.status.success() {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let found = stdout
                            .lines()
                            .any(|line| line.trim().split_whitespace().next() == Some(CONNECTION_NAME));
                        if found {
                            println!("✓ IBus runtime exposes {CONNECTION_NAME}.");
                        } else {
                            println!(
                                "⚠ ibus runtime does not expose {CONNECTION_NAME} yet (file marker may still be enough)."
                            );
                        }
                        found
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                println!("⚠ ibus command unavailable; cannot verify runtime list now.");
                false
            };

            if (marker_ok || runtime_ok) && daemon_ok {
                println!("Verdict: PASS (host registration ready).");
                0
            } else if marker_ok || runtime_ok {
                println!("Verdict: PENDING (registration detected, but IME daemon not running).");
                2
            } else {
                println!("Verdict: FAIL (no detectable registration marker/runtime).");
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
                println!("⚠ Fcitx daemon process not found (you may need to start it, e.g., fcitx5 -r).");
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

fn write_ibus_marker(home: &Path) -> Result<(), String> {
    let path = ibus_component_path(home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create dir {}: {e}", parent.display()))?;
    }
    let marker = format!(
        r#"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<component>
  <name>Suzaku IME</name>
  <description>Framework: IBus host marker for {CONNECTION_NAME}</description>
  <engines>
    <engine>
      <name>{CONNECTION_NAME}</name>
      <description>Suzaku IME</description>
      <language>all</language>
      <icon>input-keyboard</icon>
      <rank>80</rank>
    </engine>
  </engines>
</component>
"#,
    );
    fs::write(&path, marker).map_err(|e| format!("write {}: {e}", path.display()))
}

fn process_running(name: &str) -> bool {
    if !command_exists("pgrep") {
        return false;
    }
    if let Some(mut cmd) = command("pgrep") {
        cmd.arg("-x").arg(name).status().map(|status| status.success()).unwrap_or(false)
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
                if path.is_dir() {
                    Some(path)
                } else {
                    None
                }
            })
        })
        .collect();
    versions.sort();
    versions
        .pop()
        .ok_or_else(|| "no ndk folder found under $ANDROID_SDK_ROOT/ndk".to_string())
}

fn resolve_android_env() -> Result<AndroidEnv, String> {
    let android_sdk_root = env::var("ANDROID_SDK_ROOT")
        .or_else(|_| env::var("ANDROID_HOME"))
        .unwrap_or_else(|_| DEFAULT_ANDROID_SDK_ROOT.to_string());
    let android_home = env::var("ANDROID_HOME").unwrap_or_else(|_| android_sdk_root.clone());
    if !Path::new(&android_sdk_root).is_dir() {
        return Err(format!("ANDROID_SDK_ROOT not found: {android_sdk_root}"));
    }

    let java_home = env::var("JAVA_HOME").unwrap_or_else(|_| DEFAULT_JAVA_HOME.to_string());
    if !Path::new(&java_home).is_dir() {
        return Err(format!("JAVA_HOME not found: {java_home}"));
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
        java_home: PathBuf::from(java_home),
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
    println!("export SUZAKU_ANDROID_NDK_BIN={}", env.suzaku_android_ndk_bin.display());
    println!("export PATH={}:{}", env.java_home.join("bin").display(), env_var_path());
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
        ("x86", "i686-linux-android", "i686-linux-android29-clang", "CARGO_TARGET_I686_LINUX_ANDROID_LINKER"),
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

fn run_android_build_native(profile: Option<&str>) -> i32 {
    run_android_build_native_with_flags(profile == Some("release"))
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
    let target_path = root.join(format!("target/{rust_target}/{profile}")).join("libsuzaku_map.so");
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
    let mut status_cmd = build_cmd
        .env(linker_var, linker)
        .status()
        .map_err(|e| format!("cargo build failed for {rust_target}: {e}"))?;
    check_exit_status(&status_cmd, &format!("cargo build for {abi}"))?;

    if !target_path.exists() {
        return Err(format!("expected Rust library missing: {}", target_path.display()));
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
        None => Err(std::io::Error::new(std::io::ErrorKind::NotFound, "adb command not found")),
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
        if let Err(err) = run_status(cmd.arg("install").arg("-r").arg(apk_path.clone())) {
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
        return Err(format!("Invalid APK path (must be *.apk): {}", canonical.display()));
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
        None => Err(std::io::Error::new(std::io::ErrorKind::NotFound, "adb command not found")),
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
        if let Err(err) = run_status(cmd.arg("shell").arg("ime").arg("enable").arg(IME_ID)) {
            eprintln!("{err}");
            return 1;
        }
    } else {
        eprintln!("adb command not found.");
        return 1;
    }
    if let Some(mut cmd) = command("adb") {
        if let Err(err) = run_status(cmd.arg("shell").arg("ime").arg("set").arg(IME_ID)) {
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
        MacTarget::Panel => (
            "panel",
            "Suzaku Panel",
            "src/macos/SuzakuPanel-Info.plist",
        ),
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
    build.current_dir(&root).arg("build").arg("--bin").arg(binary);
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
