use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    match target_os.as_str() {
        "macos" => {
            cc::Build::new()
                .file("src/macos/speech_bridge.m")
                .file("src/macos/text_output_bridge.m")
                .file("src/macos/ime_host_bridge.m")
                .flag("-fobjc-arc")
                .compile("suzaku_speech_bridge");

            println!("cargo:rustc-link-lib=framework=Foundation");
            println!("cargo:rustc-link-lib=framework=AppKit");
            println!("cargo:rustc-link-lib=framework=ApplicationServices");
            println!("cargo:rustc-link-lib=framework=InputMethodKit");
            println!("cargo:rustc-link-lib=framework=Speech");
            println!("cargo:rustc-link-lib=framework=AVFoundation");
            println!("cargo:rerun-if-changed=src/macos/speech_bridge.m");
            println!("cargo:rerun-if-changed=src/macos/text_output_bridge.m");
            println!("cargo:rerun-if-changed=src/macos/ime_host_bridge.m");
            println!("cargo:rerun-if-changed=src/macos/ime_host_bridge/preamble.inc.m");
            println!("cargo:rerun-if-changed=src/macos/ime_host_bridge/controller.inc.m");
            println!("cargo:rerun-if-changed=src/macos/ime_host_bridge/bootstrap.inc.m");
            println!("cargo:rerun-if-changed=src/macos/SuzakuPanel-Info.plist");
            println!("cargo:rerun-if-changed=src/macos/SuzakuInputMethod-Info.plist");
            println!(
                "cargo:rustc-link-arg-bin=panel=-Wl,-sectcreate,__TEXT,__info_plist,src/macos/SuzakuPanel-Info.plist"
            );
        }
        "windows" => {
            cc::Build::new()
                .cpp(true)
                .flag_if_supported("/std:c++17")
                .flag_if_supported("-std=c++17")
                .file("src/windows/speech_bridge.cpp")
                .compile("suzaku_windows_speech_bridge");

            println!("cargo:rerun-if-changed=src/windows/speech_bridge.cpp");
        }
        "linux" => {
            cc::Build::new()
                .file("src/linux/speech_bridge.c")
                .compile("suzaku_linux_speech_bridge");

            println!("cargo:rerun-if-changed=src/linux/speech_bridge.c");

            if std::env::var_os("CARGO_FEATURE_LINUX_IBUS").is_some() {
                build_linux_ibus_bridge();
            }
        }
        _ => {}
    }
}

fn build_linux_ibus_bridge() {
    let mut build = cc::Build::new();
    build.file("src/linux/ibus_engine_bridge.c");

    let ibus_include = locate_ibus_include().unwrap_or_else(|| {
        panic!(
            "linux-ibus requires IBus headers; install libibus-1.0-dev or set SUZAKU_IBUS_INCLUDE_DIR"
        )
    });
    build.include(ibus_include);
    add_pkg_config_cflags(&mut build, &["glib-2.0", "gio-2.0"]);
    build.compile("suzaku_linux_ibus_bridge");

    // libibus keeps SONAME 5 on supported IBus 1.x installations. Linking the
    // SONAME also lets local builds use the runtime package when headers were
    // supplied separately through SUZAKU_IBUS_INCLUDE_DIR.
    println!("cargo:rustc-link-arg=-Wl,-l:libibus-1.0.so.5");
    println!("cargo:rustc-link-lib=gio-2.0");
    println!("cargo:rustc-link-lib=gobject-2.0");
    println!("cargo:rustc-link-lib=glib-2.0");
    println!("cargo:rerun-if-env-changed=SUZAKU_IBUS_INCLUDE_DIR");
    println!("cargo:rerun-if-changed=src/linux/ibus_engine_bridge.c");
    println!("cargo:rerun-if-changed=src/linux/ibus_companion.inc.c");
}

fn locate_ibus_include() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SUZAKU_IBUS_INCLUDE_DIR") {
        let path = PathBuf::from(path);
        if path.join("ibus.h").is_file() {
            return Some(path);
        }
    }

    let manifest_dir =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_else(|| ".".into()));
    [
        Path::new("/usr/include/ibus-1.0").to_path_buf(),
        manifest_dir.join("target/ibus-dev/usr/include/ibus-1.0"),
    ]
    .into_iter()
    .find(|path| path.join("ibus.h").is_file())
}

fn add_pkg_config_cflags(build: &mut cc::Build, packages: &[&str]) {
    let output = Command::new("pkg-config")
        .arg("--cflags")
        .args(packages)
        .output()
        .unwrap_or_else(|error| panic!("failed to run pkg-config for GLib headers: {error}"));
    if !output.status.success() {
        panic!(
            "pkg-config could not resolve GLib/GIO headers: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    for flag in String::from_utf8_lossy(&output.stdout).split_whitespace() {
        if let Some(include) = flag.strip_prefix("-I") {
            build.include(include);
        } else {
            build.flag(flag);
        }
    }
}
