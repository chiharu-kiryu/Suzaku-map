//! Linux path/lease regressions; every operation stays inside a fresh temporary fixture.
#![cfg(target_os = "linux")]

use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use suzaku_map::data::{
    files::{DataLease, atomic_write, private_directory},
    paths::lock_for_settings,
};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "suzaku-lock-regression-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }

    fn file(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        atomic_write(&path, b"{}").unwrap();
        path
    }

    fn link(&self, name: &str, target: &Path) -> PathBuf {
        let path = self.0.join(name);
        private_directory(path.parent().unwrap()).unwrap();
        symlink(target, &path).unwrap();
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn restore_gate(path: &Path) -> Result<DataLease, String> {
    DataLease::acquire(&lock_for_settings(path).unwrap(), true)
}

#[test]
fn a_lifetime_lease_keeps_every_link_gate_after_intermediate_and_head_replacements() {
    let fixture = Fixture::new();
    let target = fixture.file("target/settings.json");
    let middle = fixture.link("middle/settings.json", Path::new("../target/settings.json"));
    let head = fixture.link("head/settings.json", Path::new("../middle/settings.json"));
    let lease = DataLease::settings_shared(&head).unwrap();
    for path in [&head, &middle, &target] {
        assert!(restore_gate(path).is_err());
    }
    atomic_write(&middle, br#"{"language":"ja"}"#).unwrap();
    atomic_write(&head, br#"{"language":"en"}"#).unwrap();
    assert!(!head.is_symlink() && !middle.is_symlink());
    assert_eq!(fs::read(&target).unwrap(), b"{}");
    for path in [&head, &middle, &target] {
        assert!(
            restore_gate(path).is_err(),
            "replacing a link must not remove its gate"
        );
    }
    drop(lease);
    for path in [&head, &middle, &target] {
        assert!(restore_gate(path).is_ok());
    }
}

#[test]
fn aliases_share_writer_gates_and_directory_aliases_do_not_self_deadlock() {
    let fixture = Fixture::new();
    let target = fixture.file("z-target/settings.json");
    let alias = fixture.link("a-alias/settings.json", &target);
    let directory = fixture.link("directory-alias", target.parent().unwrap());
    let directory_alias = directory.join("settings.json");
    let writer = DataLease::settings_writer(&target).unwrap();
    assert!(DataLease::settings_writer(&alias).is_err());
    assert!(DataLease::settings_writer(&directory_alias).is_err());
    // The failed attempt above must have released any earlier alias-directory gate.
    let alias_writer = lock_for_settings(&alias)
        .unwrap()
        .with_file_name(".suzaku-settings-write.lock");
    assert!(DataLease::acquire(&alias_writer, true).is_ok());
    drop(writer);
    let writer = DataLease::settings_writer(&alias).unwrap();
    assert!(DataLease::settings_writer(&target).is_err());
    drop(writer);
    assert!(DataLease::settings_writer(&directory_alias).is_ok());
    let same_directory_alias = fixture.link("z-target/alias.json", Path::new("settings.json"));
    assert!(DataLease::settings_writer(&same_directory_alias).is_ok());
}

#[test]
fn an_exclusive_legacy_gate_blocks_alias_startup_and_partial_leases_are_released() {
    let fixture = Fixture::new();
    let target = fixture.file("z-target/settings.json");
    let alias = fixture.link("a-alias/settings.json", &target);
    // Use the original single-path gate, as existing restore/older processes do.
    let exclusive = restore_gate(&target).unwrap();
    assert!(DataLease::settings_shared(&alias).is_err());
    assert!(
        restore_gate(&alias).is_ok(),
        "do not leak the partially acquired alias lock"
    );
    drop(exclusive);
    assert!(DataLease::settings_shared(&alias).is_ok());
}

#[test]
fn missing_files_are_protected_without_creating_configuration_and_broken_parents_fail_closed() {
    let fixture = Fixture::new();
    let direct = fixture.0.join("fresh/nested/settings.json");
    let lease = DataLease::settings_shared(&direct).unwrap();
    assert!(!direct.exists());
    assert!(restore_gate(&direct).is_err());
    drop(lease);
    let alias = fixture.link("alias/settings.json", &direct);
    let lease = DataLease::settings_shared(&alias).unwrap();
    assert!(!direct.exists() && alias.is_symlink());
    assert!(restore_gate(&alias).is_err());
    assert!(restore_gate(&direct).is_err());
    drop(lease);
    let absent = fixture.0.join("missing-parent/settings.json");
    let broken = fixture.link("broken/settings.json", &absent);
    assert!(DataLease::settings_shared(&broken).is_err());
    assert!(!absent.parent().unwrap().exists());
}

#[test]
fn cycles_and_reserved_lock_targets_are_rejected_without_modifying_them() {
    let fixture = Fixture::new();
    let a = fixture.link("loop/a.json", Path::new("b.json"));
    fixture.link("loop/b.json", Path::new("a.json"));
    assert!(
        DataLease::settings_shared(&a)
            .err()
            .unwrap()
            .contains("循环")
    );
    assert!(DataLease::settings_writer(&a).is_err());
    for name in [".suzaku-data.lock", ".suzaku-settings-write.lock"] {
        let target = fixture.file(name);
        let alias = fixture.link(&format!("aliases/{name}.json"), &target);
        assert!(
            DataLease::settings_shared(&alias)
                .err()
                .unwrap()
                .contains("保留")
        );
        assert!(DataLease::settings_writer(&alias).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"{}");
    }
}
