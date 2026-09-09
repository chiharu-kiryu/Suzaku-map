#[cfg(target_os = "linux")]
fn main() {
    let _data_lease = match suzaku_map::data::files::DataLease::current_shared() {
        Ok(lease) => lease,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
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
