use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const HOMEBREW_ANDROID_SDK_ROOT: &str = "/opt/homebrew/share/android-commandlinetools";
const HOMEBREW_JAVA_HOME: &str = "/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home";

fn detect_ndk_home(sdk_root: &str) -> String {
    let ndk_root = Path::new(sdk_root).join("ndk");
    std::fs::read_dir(&ndk_root)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.flatten())
        .filter_map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                Some(path.display().to_string())
            } else {
                None
            }
        })
        .max()
        .unwrap_or_else(|| "<missing>".to_string())
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
        let mode = std::fs::metadata(path)
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

fn run_and_capture(cmd: &str, args: &[&str]) -> Option<String> {
    command(cmd)?
        .args(args)
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
}

fn main() {
    println!("Suzaku Android Doctor");
    println!(
        "ANDROID_HOME={}",
        env::var("ANDROID_HOME").unwrap_or_else(|_| "<unset>".to_string())
    );
    println!(
        "ANDROID_SDK_ROOT={}",
        env::var("ANDROID_SDK_ROOT").unwrap_or_else(|_| "<unset>".to_string())
    );
    println!(
        "ANDROID_NDK_HOME={}",
        env::var("ANDROID_NDK_HOME").unwrap_or_else(|_| "<unset>".to_string())
    );
    println!("default_sdk_root={}", HOMEBREW_ANDROID_SDK_ROOT);
    println!("default_java_home={}", HOMEBREW_JAVA_HOME);
    println!(
        "detected_ndk_home={}",
        detect_ndk_home(HOMEBREW_ANDROID_SDK_ROOT)
    );
    println!("adb={}", command_exists("adb"));
    println!("sdkmanager={}", command_exists("sdkmanager"));
    println!("gradle={}", command_exists("gradle"));

    let installed_targets =
        run_and_capture("rustup", &["target", "list", "--installed"]).unwrap_or_default();
    println!("rust-targets={}", installed_targets.replace('\n', ", "));
}
