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
}
