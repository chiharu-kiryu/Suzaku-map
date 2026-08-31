use std::env;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

const DEFAULT_ANDROID_SDK_ROOT: &str = "/opt/homebrew/share/android-commandlinetools";
const DEFAULT_JAVA_HOME: &str = "/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home";
const REQUIRED_ANDROID_TARGETS: [&str; 4] = [
    "aarch64-linux-android",
    "armv7-linux-androideabi",
    "i686-linux-android",
    "x86_64-linux-android",
];

fn detect_ndk_home(sdk_root: &Path) -> String {
    let ndk_root = sdk_root.join("ndk");
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

fn configured_sdk_root() -> PathBuf {
    env::var_os("ANDROID_SDK_ROOT")
        .or_else(|| env::var_os("ANDROID_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_ANDROID_SDK_ROOT))
}

fn configured_java_home() -> PathBuf {
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
        .and_then(|path| std::fs::canonicalize(path).ok())
        .and_then(|path| path.parent()?.parent().map(Path::to_path_buf))
}

fn is_jdk_home(path: &Path) -> bool {
    is_executable_file(&path.join("bin/java")) && is_executable_file(&path.join("bin/javac"))
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
    command(cmd)?.args(args).output().ok().and_then(|output| {
        if output.status.success() {
            Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            None
        }
    })
}

fn main() {
    let sdk_root = configured_sdk_root();
    let java_home = configured_java_home();
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
    println!("resolved_sdk_root={}", sdk_root.display());
    println!("resolved_java_home={}", java_home.display());
    println!("java-home-has-compiler={}", is_jdk_home(&java_home));
    println!("detected_ndk_home={}", detect_ndk_home(&sdk_root));
    println!("java={}", command_exists("java"));
    println!("javac={}", command_exists("javac"));
    println!("adb={}", command_exists("adb"));
    println!("sdkmanager={}", command_exists("sdkmanager"));
    println!("gradle={}", command_exists("gradle"));
    println!("gradle-wrapper={}", Path::new("android/gradlew").is_file());

    let installed_targets =
        run_and_capture("rustup", &["target", "list", "--installed"]).unwrap_or_default();
    println!("rust-targets={}", installed_targets.replace('\n', ", "));
    let missing_targets = REQUIRED_ANDROID_TARGETS
        .iter()
        .copied()
        .filter(|target| !installed_targets.lines().any(|line| line == *target))
        .collect::<Vec<_>>();
    println!(
        "missing-rust-targets={}",
        if missing_targets.is_empty() {
            "<none>".to_string()
        } else {
            missing_targets.join(", ")
        }
    );
}
