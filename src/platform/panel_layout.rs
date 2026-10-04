//! Conservative, startup-only defaults for the panel's automatic layout.
//!
//! This probes session/chassis metadata, not input events or touchscreen presence:
//! a touchscreen laptop is still a desktop unless a mobile session is explicit.

use std::fs::File;
use std::io::Read;

use crate::ime::gpu::PanelLayoutMode;

const MAX_METADATA_BYTES: usize = 4096;
const MAX_SESSION_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelLayoutDetection {
    pub mode: PanelLayoutMode,
    /// Stable diagnostic identifier; deliberately contains no device/user data.
    pub reason: &'static str,
}

impl PanelLayoutDetection {
    const fn follow(reason: &'static str) -> Self {
        Self {
            mode: PanelLayoutMode::FollowCaret,
            reason,
        }
    }

    const fn bottom(reason: &'static str) -> Self {
        Self {
            mode: PanelLayoutMode::BottomDock,
            reason,
        }
    }
}

#[derive(Default)]
struct LinuxHints<'a> {
    session_desktop: Option<&'a str>,
    machine_info: Option<&'a str>,
    dmi_chassis: Option<&'a str>,
    device_tree_chassis: Option<&'a str>,
}

#[derive(Clone, Copy)]
enum Chassis {
    Mobile,
    Other,
}

/// Resolve Auto once when creating the main panel; callers retain the result.
///
/// Only fixed, bounded Linux metadata files and the session name are inspected.
/// There are no subprocesses, network requests, device-event reads or monitors.
/// Android's actual IME view remains positioned by its InputMethodService host.
pub fn detect() -> PanelLayoutDetection {
    let os = std::env::consts::OS;
    if os != "linux" {
        return resolve(os, &LinuxHints::default());
    }

    let session_desktop = std::env::var("XDG_SESSION_DESKTOP").ok();
    let machine_info = read_metadata("/etc/machine-info");
    let dmi_chassis = read_metadata("/sys/class/dmi/id/chassis_type");
    let device_tree_chassis = read_metadata("/sys/firmware/devicetree/base/chassis-type")
        .or_else(|| read_metadata("/proc/device-tree/chassis-type"));
    resolve(
        os,
        &LinuxHints {
            session_desktop: session_desktop.as_deref(),
            machine_info: machine_info.as_deref(),
            dmi_chassis: dmi_chassis.as_deref(),
            device_tree_chassis: device_tree_chassis.as_deref(),
        },
    )
}

fn resolve(os: &str, hints: &LinuxHints<'_>) -> PanelLayoutDetection {
    match os {
        "android" => return PanelLayoutDetection::bottom("android-mobile"),
        "macos" => return PanelLayoutDetection::follow("macos-desktop"),
        "windows" => return PanelLayoutDetection::follow("windows-desktop"),
        "linux" => {}
        _ => return PanelLayoutDetection::follow("unknown-platform"),
    }

    // gamescope also sets CURRENT_DESKTOP inside nested desktop sessions, so
    // only an explicit SESSION_DESKTOP token is authoritative here.
    if let Some(session) = hints
        .session_desktop
        .filter(|value| value.len() <= MAX_SESSION_BYTES)
        .map(str::trim)
    {
        if session.eq_ignore_ascii_case("gamescope")
            || session.eq_ignore_ascii_case("gamescope-session")
        {
            return PanelLayoutDetection::bottom("linux-session-gamescope");
        }
        if session.eq_ignore_ascii_case("phosh") {
            return PanelLayoutDetection::bottom("linux-session-phosh");
        }
        if session.eq_ignore_ascii_case("plasma-mobile") {
            return PanelLayoutDetection::bottom("linux-session-plasma-mobile");
        }
    }

    // An administrator's valid machine-info assignment overrides firmware.
    if let Some(chassis) = hints.machine_info.and_then(machine_info_chassis) {
        return match chassis {
            Chassis::Mobile => PanelLayoutDetection::bottom("linux-machine-info-mobile"),
            Chassis::Other => PanelLayoutDetection::follow("linux-machine-info-non-mobile"),
        };
    }
    if let Some(chassis) = hints.dmi_chassis.and_then(dmi_chassis) {
        return match chassis {
            Chassis::Mobile => PanelLayoutDetection::bottom("linux-dmi-mobile"),
            Chassis::Other => PanelLayoutDetection::follow("linux-dmi-non-mobile"),
        };
    }
    if let Some(chassis) = hints.device_tree_chassis.and_then(device_tree_chassis) {
        return match chassis {
            Chassis::Mobile => PanelLayoutDetection::bottom("linux-device-tree-mobile"),
            Chassis::Other => PanelLayoutDetection::follow("linux-device-tree-non-mobile"),
        };
    }
    PanelLayoutDetection::follow("linux-default-desktop")
}

fn machine_info_chassis(text: &str) -> Option<Chassis> {
    if text.len() > MAX_METADATA_BYTES {
        return None;
    }
    // Parse data, never source shell code. The last CHASSIS assignment wins;
    // invalid/unsupported syntax remains unknown instead of guessing a value.
    let value = text
        .lines()
        .filter_map(|line| line.trim().split_once('='))
        .filter(|(key, _)| key.trim() == "CHASSIS")
        .map(|(_, value)| value.trim())
        .next_back()?;
    let value = assignment_value(value)?;
    match value {
        "tablet" | "handset" => Some(Chassis::Mobile),
        "desktop" | "laptop" | "convertible" | "server" | "watch" | "embedded" | "vm"
        | "container" => Some(Chassis::Other),
        _ => None,
    }
}

fn assignment_value(value: &str) -> Option<&str> {
    if let Some(quote) = value.chars().next().filter(|ch| matches!(ch, '\'' | '"')) {
        let (value, suffix) = value[1..].split_once(quote)?;
        let trailing = suffix.trim_start();
        let comment = suffix.starts_with(char::is_whitespace) && trailing.starts_with('#');
        return (trailing.is_empty() || comment).then_some(value);
    }
    let (value, suffix) = value.split_once(char::is_whitespace).unwrap_or((value, ""));
    let suffix = suffix.trim_start();
    (suffix.is_empty() || suffix.starts_with('#')).then_some(value)
}

fn dmi_chassis(value: &str) -> Option<Chassis> {
    if value.len() > MAX_METADATA_BYTES {
        return None;
    }
    // SMBIOS: 11 = Hand Held, 30 = Tablet. Convertible (31), detachable
    // (32), and notebook (10) do not establish the current tablet posture.
    match value.trim().parse::<u8>().ok()? {
        11 | 30 => Some(Chassis::Mobile),
        3..=36 => Some(Chassis::Other),
        _ => None,
    }
}

fn device_tree_chassis(value: &str) -> Option<Chassis> {
    if value.len() > MAX_METADATA_BYTES {
        return None;
    }
    match value.trim_end_matches('\0').trim() {
        "tablet" | "handheld" | "handset" => Some(Chassis::Mobile),
        "desktop" | "laptop" | "convertible" | "server" | "all-in-one" | "watch" | "embedded"
        | "television" | "spectacles" | "headset" => Some(Chassis::Other),
        _ => None,
    }
}

fn read_metadata(path: &str) -> Option<String> {
    // Reject accidental device/FIFO paths before opening; these fixed system
    // metadata files may be symlinks, but their targets must be regular files.
    if !std::fs::metadata(path).ok()?.is_file() {
        return None;
    }
    read_bounded_text(File::open(path).ok()?)
}

fn read_bounded_text(reader: impl Read) -> Option<String> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_METADATA_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > MAX_METADATA_BYTES {
        return None;
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn operating_system_defaults_never_return_auto() {
        let mobile_hints = LinuxHints {
            session_desktop: Some("phosh"),
            machine_info: Some("CHASSIS=tablet"),
            ..LinuxHints::default()
        };
        for (os, mode, reason) in [
            ("android", PanelLayoutMode::BottomDock, "android-mobile"),
            ("macos", PanelLayoutMode::FollowCaret, "macos-desktop"),
            ("windows", PanelLayoutMode::FollowCaret, "windows-desktop"),
            ("freebsd", PanelLayoutMode::FollowCaret, "unknown-platform"),
            ("", PanelLayoutMode::FollowCaret, "unknown-platform"),
        ] {
            assert_eq!(
                resolve(os, &mobile_hints),
                PanelLayoutDetection { mode, reason }
            );
        }
        assert_eq!(
            resolve("linux", &LinuxHints::default()),
            PanelLayoutDetection::follow("linux-default-desktop")
        );
    }

    #[test]
    fn explicit_mobile_sessions_override_desktop_firmware() {
        for (session, reason) in [
            ("gamescope", "linux-session-gamescope"),
            ("gamescope-session", "linux-session-gamescope"),
            ("Phosh", "linux-session-phosh"),
            (" plasma-mobile ", "linux-session-plasma-mobile"),
        ] {
            assert_eq!(
                resolve(
                    "linux",
                    &LinuxHints {
                        session_desktop: Some(session),
                        machine_info: Some("CHASSIS=desktop"),
                        dmi_chassis: Some("10"),
                        ..LinuxHints::default()
                    }
                ),
                PanelLayoutDetection::bottom(reason)
            );
        }
    }

    #[test]
    fn desktop_or_ambiguous_session_names_do_not_identify_mobile_hardware() {
        for session in [
            "",
            "gnome",
            "ubuntu",
            "KDE",
            "plasma",
            "steamos",
            "SteamDeck",
            "gamescope-nested",
            "prefix-gamescope",
            "gamescope:GNOME",
            "phosh-debug",
            "GNOME:Phosh",
            "plasma-mobile-custom",
            "my-plasma-mobile",
            "tablet",
        ] {
            assert_eq!(
                resolve(
                    "linux",
                    &LinuxHints {
                        session_desktop: Some(session),
                        ..LinuxHints::default()
                    }
                )
                .mode,
                PanelLayoutMode::FollowCaret,
                "{session}"
            );
        }
        let overlong_session = format!("phosh{}", " ".repeat(MAX_SESSION_BYTES));
        assert_eq!(
            resolve(
                "linux",
                &LinuxHints {
                    session_desktop: Some(&overlong_session),
                    ..LinuxHints::default()
                }
            )
            .mode,
            PanelLayoutMode::FollowCaret
        );
    }

    #[test]
    fn valid_machine_info_overrides_firmware_in_both_directions() {
        for chassis in [
            "desktop",
            "laptop",
            "convertible",
            "server",
            "watch",
            "embedded",
            "vm",
            "container",
        ] {
            let machine_info = format!("CHASSIS={chassis}\n");
            assert_eq!(
                resolve(
                    "linux",
                    &LinuxHints {
                        machine_info: Some(&machine_info),
                        dmi_chassis: Some("30"),
                        device_tree_chassis: Some("tablet\0"),
                        ..LinuxHints::default()
                    }
                ),
                PanelLayoutDetection::follow("linux-machine-info-non-mobile")
            );
        }
        for text in [
            "CHASSIS=tablet",
            "CHASSIS='handset'",
            "CHASSIS=\"tablet\" # metadata",
            "# CHASSIS=desktop\r\nCHASSIS = tablet\r\n",
            "CHASSIS=desktop\nCHASSIS=tablet",
        ] {
            assert_eq!(
                resolve(
                    "linux",
                    &LinuxHints {
                        machine_info: Some(text),
                        dmi_chassis: Some("10"),
                        ..LinuxHints::default()
                    }
                ),
                PanelLayoutDetection::bottom("linux-machine-info-mobile")
            );
        }
    }

    #[test]
    fn malformed_machine_info_cannot_guess_a_mobile_chassis() {
        for text in [
            "",
            "# CHASSIS=tablet",
            "OTHER_CHASSIS=tablet",
            "CHASSIS=tabletpc",
            "CHASSIS=\"tablet",
            "CHASSIS=tablet;command",
            "CHASSIS=$(tablet)",
            "CHASSIS=tablet trailing",
            "CHASSIS=\"tablet\"trailing",
            "CHASSIS=\"tablet\"#suffix",
            "CHASSIS=TABLET",
            "CHASSIS=tablet\nCHASSIS=unknown",
            "CHASSIS=tablet\0",
            "CHASSIS=",
        ] {
            assert_eq!(
                resolve(
                    "linux",
                    &LinuxHints {
                        machine_info: Some(text),
                        dmi_chassis: Some("10"),
                        ..LinuxHints::default()
                    }
                ),
                PanelLayoutDetection::follow("linux-dmi-non-mobile"),
                "{text:?}"
            );
        }
    }

    #[test]
    fn dmi_uses_only_explicit_handheld_or_tablet_types() {
        for code in 0..=255 {
            let dmi = format!("{code}\n");
            assert_eq!(
                resolve(
                    "linux",
                    &LinuxHints {
                        dmi_chassis: Some(&dmi),
                        ..LinuxHints::default()
                    }
                )
                .mode,
                if matches!(code, 11 | 30) {
                    PanelLayoutMode::BottomDock
                } else {
                    PanelLayoutMode::FollowCaret
                },
                "DMI {code}"
            );
        }
        for value in ["10", "31", "32"] {
            assert_eq!(
                resolve(
                    "linux",
                    &LinuxHints {
                        dmi_chassis: Some(value),
                        device_tree_chassis: Some("tablet\0"),
                        ..LinuxHints::default()
                    }
                ),
                PanelLayoutDetection::follow("linux-dmi-non-mobile")
            );
        }
    }

    #[test]
    fn device_tree_is_a_fallback_for_missing_or_unknown_dmi() {
        for dmi in [None, Some(""), Some("2"), Some("not-a-number"), Some("300")] {
            for chassis in ["tablet\0", "handheld\0", "handset\0"] {
                assert_eq!(
                    resolve(
                        "linux",
                        &LinuxHints {
                            dmi_chassis: dmi,
                            device_tree_chassis: Some(chassis),
                            ..LinuxHints::default()
                        }
                    ),
                    PanelLayoutDetection::bottom("linux-device-tree-mobile")
                );
            }
        }
        for chassis in [
            "laptop\0",
            "convertible\0",
            "desktop\0",
            "tablet\0desktop",
            "tabletpc\0",
        ] {
            assert_eq!(
                resolve(
                    "linux",
                    &LinuxHints {
                        device_tree_chassis: Some(chassis),
                        ..LinuxHints::default()
                    }
                )
                .mode,
                PanelLayoutMode::FollowCaret
            );
        }
    }

    #[test]
    fn metadata_reads_are_bounded_and_invalid_utf8_is_unknown() {
        assert_eq!(
            read_bounded_text(Cursor::new(b"CHASSIS=tablet")),
            Some("CHASSIS=tablet".to_owned())
        );
        assert_eq!(read_bounded_text(Cursor::new([0xff])), None);
        assert_eq!(
            read_bounded_text(Cursor::new(vec![b' '; MAX_METADATA_BYTES]))
                .unwrap()
                .len(),
            MAX_METADATA_BYTES
        );
        let mut oversized = Cursor::new(vec![b' '; MAX_METADATA_BYTES * 4]);
        assert_eq!(read_bounded_text(&mut oversized), None);
        assert_eq!(oversized.position(), (MAX_METADATA_BYTES + 1) as u64);

        let overlong = format!("CHASSIS=tablet\n{}", " ".repeat(MAX_METADATA_BYTES));
        assert_eq!(
            resolve(
                "linux",
                &LinuxHints {
                    machine_info: Some(&overlong),
                    ..LinuxHints::default()
                }
            ),
            PanelLayoutDetection::follow("linux-default-desktop")
        );
        assert!(dmi_chassis(&" ".repeat(MAX_METADATA_BYTES + 1)).is_none());
        assert!(device_tree_chassis(&" ".repeat(MAX_METADATA_BYTES + 1)).is_none());
    }
}
