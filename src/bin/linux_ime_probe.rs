#[cfg(target_os = "linux")]
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let via_ipc = args.first().is_some_and(|argument| argument == "--ipc");
    let seed = args.get(usize::from(via_ipc)).cloned().unwrap_or_else(|| {
        if via_ipc {
            "Suzaku panel IPC roundtrip".to_string()
        } else {
            "ni".to_string()
        }
    });
    let result = if via_ipc {
        suzaku_map::platform::linux_ibus_host::probe_ipc_roundtrip(&seed)
    } else {
        suzaku_map::platform::linux_ibus_host::probe_roundtrip(&seed)
    };
    match result {
        Ok(committed) => println!(
            "roundtrip: mode={} seed={seed:?} committed={committed:?}",
            if via_ipc { "panel-ipc" } else { "keyboard" }
        ),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("The native IBus probe is only available on Linux.");
    std::process::exit(1);
}
