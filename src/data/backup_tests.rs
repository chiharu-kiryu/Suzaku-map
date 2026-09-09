use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture {
    root: PathBuf,
    paths: DataPaths,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "suzaku-data-test-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self {
            paths: DataPaths {
                ime: root.join("config/ime.json"),
                panel: root.join("panel/settings.toml"),
                backups: root.join("backups"),
            },
            root,
        }
    }
    fn write_settings(&self) {
        files::atomic_write(&self.paths.ime, br#"{"language":"ja","llm_model":"llama3.2:1b","llm_temperature_tenths":3,"history":"synthetic-never-export"}"#).unwrap();
        files::atomic_write(
            &self.paths.panel,
            b"theme_preset=solarized\nllm_temperature=custom:3\nwindow_scale=1.2\n",
        )
        .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn snapshot_contains_configuration_only_and_round_trips_custom_values() {
    let fixture = Fixture::new();
    fixture.write_settings();
    fs::write(fixture.root.join("input-history.txt"), "do-not-export").unwrap();
    let backup = Backup::collect(&fixture.paths).unwrap();
    let raw = backup.to_json().to_string();
    assert!(!raw.contains("synthetic-never-export"));
    assert!(!raw.contains("history"));
    assert!(!raw.contains(&fixture.root.display().to_string()));
    let parsed = Backup::parse(&raw).unwrap();
    assert_eq!(parsed.ime.as_ref().unwrap()["language"], "ja");
    assert_eq!(parsed.ime.as_ref().unwrap()["llm_temperature_tenths"], 3);
    assert_eq!(parsed.panel.unwrap()["llm_temperature"], "custom:3");
}

#[test]
fn missing_settings_are_backed_up_without_materializing_defaults() {
    let fixture = Fixture::new();
    let backup = Backup::collect(&fixture.paths).unwrap();
    assert!(backup.ime.is_none() && backup.panel.is_none());
    assert!(!fixture.paths.ime.exists());
    assert!(!fixture.paths.panel.exists());
    Backup::parse(&backup.to_json().to_string()).unwrap();
}

#[test]
fn backup_never_overwrites_and_is_private() {
    let fixture = Fixture::new();
    fixture.write_settings();
    let backup = Backup::collect(&fixture.paths).unwrap();
    let destination = fixture.paths.backups.join("test.json");
    backup.write_new(&destination).unwrap();
    let before = fs::read(&destination).unwrap();
    assert!(backup.write_new(&destination).is_err());
    assert_eq!(fs::read(&destination).unwrap(), before);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&destination).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&fixture.paths.backups)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
    assert_eq!(fs::read_dir(&fixture.paths.backups).unwrap().count(), 1);
}

#[test]
fn corrupt_unknown_remote_and_path_injection_backups_are_rejected() {
    let fixture = Fixture::new();
    let good = Backup::collect(&fixture.paths).unwrap().to_json();
    let mut cases = vec![json!(null), json!([])];
    for (key, value) in [
        ("schema_version", json!(2)),
        ("format", json!("zip")),
        ("files", json!({"../../victim": "x"})),
        ("app_version", json!("bad\nversion")),
        ("panel", json!({"theme_preset": "malicious\ntext=x"})),
        ("panel", json!({"window_scale": "NaN"})),
        ("panel", json!({"theme_preset": true})),
        (
            "ime",
            json!({"llm_endpoint": "https://remote.invalid/api/chat"}),
        ),
        ("ime", json!({"history": "secret"})),
    ] {
        let mut value_to_test = good.clone();
        value_to_test[key] = value;
        cases.push(value_to_test);
    }
    for value in cases {
        assert!(Backup::parse(&value.to_string()).is_err(), "{value}");
    }
    assert!(Backup::parse("{broken").is_err());
    assert!(Backup::parse(&" ".repeat(BACKUP_LIMIT + 1)).is_err());
    assert!(!fixture.paths.ime.exists());
}

#[test]
fn panel_unknown_duplicate_or_invalid_values_are_not_exported() {
    for raw in [
        "history=secret",
        "window_scale=inf",
        "theme_preset=unknown",
        "text_scale=large\ntext_scale=small",
        "not-a-key",
    ] {
        assert!(panel_from_text(raw).is_err(), "{raw}");
    }
    assert!(panel_from_text("# ignored comment\nllm_temperature=custom:10\n").is_ok());
}

#[test]
fn preview_is_read_only_and_restore_can_be_undone() {
    let fixture = Fixture::new();
    fixture.write_settings();
    let previous = Backup::collect(&fixture.paths).unwrap();
    let desired = Backup {
        ime: Some(ImeSettings::default().to_json()),
        panel: Some(panel_from_text("theme_preset=device_dark\n").unwrap()),
    };
    let raw_before = fs::read(&fixture.paths.ime).unwrap();
    let report = preview(&fixture.paths, &desired).unwrap();
    assert_eq!(report.len(), 2);
    assert_eq!(fs::read(&fixture.paths.ime).unwrap(), raw_before);
    assert!(!fixture.paths.backups.exists());
    let safety = restore_locked(&fixture.paths, &desired).unwrap().unwrap();
    assert_eq!(Backup::collect(&fixture.paths).unwrap().ime, desired.ime);
    restore_locked(&fixture.paths, &Backup::read(&safety).unwrap()).unwrap();
    let restored = Backup::collect(&fixture.paths).unwrap();
    assert_eq!(restored.ime, previous.ime);
    assert_eq!(restored.panel, previous.panel);
}

#[test]
fn restoring_absent_entries_resets_only_the_named_settings_and_is_reversible() {
    let fixture = Fixture::new();
    fixture.write_settings();
    let unrelated = fixture.paths.ime.parent().unwrap().join("notes.txt");
    fs::write(&unrelated, "keep").unwrap();
    let safety = restore_locked(
        &fixture.paths,
        &Backup {
            ime: None,
            panel: None,
        },
    )
    .unwrap()
    .unwrap();
    assert!(!fixture.paths.ime.exists() && !fixture.paths.panel.exists());
    assert_eq!(fs::read_to_string(&unrelated).unwrap(), "keep");
    restore_locked(&fixture.paths, &Backup::read(&safety).unwrap()).unwrap();
    assert!(fixture.paths.ime.is_file() && fixture.paths.panel.is_file());
}

#[test]
fn preflight_failure_does_not_partially_replace_settings() {
    let fixture = Fixture::new();
    fixture.write_settings();
    let before = fs::read(&fixture.paths.ime).unwrap();
    fs::remove_file(&fixture.paths.panel).unwrap();
    fs::create_dir(&fixture.paths.panel).unwrap();
    assert!(
        restore_locked(
            &fixture.paths,
            &Backup {
                ime: None,
                panel: None
            }
        )
        .is_err()
    );
    assert_eq!(fs::read(&fixture.paths.ime).unwrap(), before);
    assert!(!fixture.paths.backups.exists());
}

#[test]
fn a_mid_restore_failure_rolls_back_the_exact_original_bytes() {
    let fixture = Fixture::new();
    fixture.write_settings();
    let before_ime = fs::read(&fixture.paths.ime).unwrap();
    let before_panel = fs::read(&fixture.paths.panel).unwrap();
    let mut calls = 0;
    let error = restore_with_publish(
        &fixture.paths,
        &Backup {
            ime: None,
            panel: None,
        },
        |path, staged| {
            calls += 1;
            if calls == 2 {
                Err(std::io::Error::other("synthetic second-file failure"))
            } else {
                publish(path, staged)
            }
        },
    )
    .unwrap_err();
    assert!(error.contains("回退完成"), "{error}");
    assert_eq!(calls, 3);
    assert_eq!(fs::read(&fixture.paths.ime).unwrap(), before_ime);
    assert_eq!(fs::read(&fixture.paths.panel).unwrap(), before_panel);
    assert_eq!(fs::read_dir(&fixture.paths.backups).unwrap().count(), 1);
}

#[test]
fn rollback_failure_is_reported_with_a_usable_recovery_backup() {
    let fixture = Fixture::new();
    fixture.write_settings();
    let original = Backup::collect(&fixture.paths).unwrap();
    let mut calls = 0;
    let error = restore_with_publish(
        &fixture.paths,
        &Backup {
            ime: None,
            panel: None,
        },
        |path, staged| {
            calls += 1;
            if calls >= 2 {
                Err(std::io::Error::other("synthetic write/rollback failure"))
            } else {
                publish(path, staged)
            }
        },
    )
    .unwrap_err();
    assert!(error.contains("回退失败"), "{error}");
    let safety = fs::read_dir(&fixture.paths.backups)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let saved = Backup::read(&safety).unwrap();
    assert_eq!(saved.ime, original.ime);
    assert_eq!(saved.panel, original.panel);
    restore_locked(&fixture.paths, &saved).unwrap();
    assert_eq!(Backup::collect(&fixture.paths).unwrap().ime, original.ime);
}

#[test]
fn reserved_lock_name_is_never_treated_as_configuration() {
    let fixture = Fixture::new();
    assert!(
        super::super::paths::lock_for_settings(&fixture.root.join(".suzaku-data.lock")).is_err()
    );
}

#[test]
fn shared_process_leases_prevent_restore_and_exclusive_lease_prevents_new_writers() {
    let fixture = Fixture::new();
    let path = fixture.paths.lock_path().unwrap();
    let first = DataLease::acquire(&path, false).unwrap();
    let second = DataLease::acquire(&path, false).unwrap();
    assert!(DataLease::acquire(&path, true).is_err());
    drop(first);
    drop(second);
    let exclusive = DataLease::acquire(&path, true).unwrap();
    assert!(DataLease::acquire(&path, false).is_err());
    assert!(DataLease::acquire(&path, true).is_err());
    drop(exclusive);
    DataLease::acquire(&path, false).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn symlinks_fifos_and_oversized_files_are_rejected_without_following_or_blocking() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let path = fixture.root.join("untrusted");
    let target = fixture.root.join("target");
    fs::write(&target, "private").unwrap();
    symlink(&target, &path).unwrap();
    assert!(files::read_optional(&path, SETTINGS_LIMIT).is_err());
    assert!(DataLease::acquire(&path, true).is_err());
    fs::remove_file(&path).unwrap();
    let c_path = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
    assert!(files::read_optional(&path, SETTINGS_LIMIT).is_err());
    fs::remove_file(&path).unwrap();
    fs::write(&path, vec![b'x'; SETTINGS_LIMIT + 1]).unwrap();
    assert!(files::read_optional(&path, SETTINGS_LIMIT).is_err());
    assert_eq!(fs::read_to_string(target).unwrap(), "private");
}
