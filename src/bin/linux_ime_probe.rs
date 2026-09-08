#[cfg(target_os = "linux")]
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let via_ipc = args.first().is_some_and(|argument| argument == "--ipc");
    let via_llm = args.first().is_some_and(|argument| argument == "--llm");
    let complete_word = args
        .first()
        .is_some_and(|argument| argument == "--complete");
    let private = args.first().is_some_and(|argument| argument == "--private");
    let password = args
        .first()
        .is_some_and(|argument| argument == "--password");
    let seed = args
        .get(usize::from(
            via_ipc || via_llm || private || password || complete_word,
        ))
        .cloned()
        .unwrap_or_else(|| {
            if via_ipc {
                "Suzaku panel IPC roundtrip".to_string()
            } else {
                "ni".to_string()
            }
        });
    let result = if via_ipc {
        suzaku_map::platform::linux_ibus_host::probe_ipc_roundtrip_report(&seed)
    } else if via_llm {
        suzaku_map::platform::linux_ibus_host::probe_llm_roundtrip_report(&seed)
    } else if complete_word {
        suzaku_map::platform::linux_ibus_host::probe_completion_roundtrip_report(&seed)
    } else if private || password {
        suzaku_map::platform::linux_ibus_host::probe_private_roundtrip_report(&seed, password)
    } else {
        suzaku_map::platform::linux_ibus_host::probe_roundtrip_report(&seed)
    };
    match result {
        Ok(report) => {
            println!(
                "roundtrip: mode={} seed={seed:?} preedit={:?} candidates={} page-size={} selected={} primary={:?} committed={:?}",
                if via_ipc {
                    "panel-ipc"
                } else if via_llm {
                    "llm"
                } else if complete_word {
                    "word-completion"
                } else if password {
                    "password"
                } else if private {
                    "private"
                } else {
                    "keyboard"
                },
                report.preedit,
                report.candidate_count,
                report.page_size,
                report.selected_index,
                report.primary_candidate,
                report.committed,
            )
        }
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
