#[cfg(target_os = "macos")]
fn main() {
    use suzaku_map::platform::TargetPlatform;
    use suzaku_map::platform::ime_host_runtime::runtime_report_for;

    println!("{}", runtime_report_for(TargetPlatform::MacOs, true));
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("macOS IME host skeleton is only available on macOS.");
}
