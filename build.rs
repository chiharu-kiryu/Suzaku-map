fn main() {
    #[cfg(target_os = "macos")]
    {
        cc::Build::new()
            .file("src/macos/speech_bridge.m")
            .flag("-fobjc-arc")
            .compile("suzaku_speech_bridge");

        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=Speech");
        println!("cargo:rustc-link-lib=framework=AVFoundation");
        println!("cargo:rerun-if-changed=src/macos/speech_bridge.m");
        println!("cargo:rerun-if-changed=src/macos/SuzakuPanel-Info.plist");
        println!(
            "cargo:rustc-link-arg-bin=panel=-Wl,-sectcreate,__TEXT,__info_plist,src/macos/SuzakuPanel-Info.plist"
        );
    }

    #[cfg(target_os = "windows")]
    {
        cc::Build::new()
            .cpp(true)
            .flag_if_supported("/std:c++17")
            .flag_if_supported("-std=c++17")
            .file("src/windows/speech_bridge.cpp")
            .compile("suzaku_windows_speech_bridge");

        println!("cargo:rerun-if-changed=src/windows/speech_bridge.cpp");
    }

    #[cfg(target_os = "linux")]
    {
        cc::Build::new()
            .file("src/linux/speech_bridge.c")
            .compile("suzaku_linux_speech_bridge");

        println!("cargo:rerun-if-changed=src/linux/speech_bridge.c");
    }
}
