#[cfg(target_os = "linux")]
fn main() {
    if let Err(error) = suzaku_map::platform::linux_ibus_host::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("The native IBus host is only available on Linux.");
    std::process::exit(1);
}
