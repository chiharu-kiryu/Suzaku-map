#![cfg(target_os = "linux")]
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "suzaku-package-cli-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }

    fn write(&self, name: &str, bytes: impl AsRef<[u8]>) {
        fs::write(self.0.join(name), bytes).unwrap();
    }

    fn rejects(&self, message: &str) {
        let output = Command::new("bash")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/scripts/test-linux-package.sh"
            ))
            .arg(&self.0)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(message), "{stderr}");
    }

    fn checksum(&self, name: &str) -> Vec<u8> {
        let output = Command::new("sha256sum")
            .current_dir(&self.0)
            .arg(name)
            .output()
            .unwrap();
        assert!(output.status.success());
        output.stdout
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn an_empty_directory_or_orphaned_checksum_is_not_a_package() {
    let f = Fixture::new();
    f.rejects("No packages found");
    f.write("orphan.sha256", "not a package");
    f.rejects("No packages found");
    f.write("example.tar.gz", "dummy artifact");
    f.write("example.tar.gz.sha256", f.checksum("example.tar.gz"));
    f.rejects("Unexpected checksum without a package");
}

#[test]
fn every_artifact_needs_its_own_checksum_even_when_other_checksums_exist() {
    let f = Fixture::new();
    f.write("first.deb", "first dummy artifact");
    f.write("first.deb.sha256", f.checksum("first.deb"));
    f.write("second.deb", "second dummy artifact");
    f.rejects("Missing package checksum:");
}

#[test]
fn checksums_must_identify_the_exact_artifact_and_its_current_bytes() {
    let f = Fixture::new();
    f.write("expected.tar.gz", "dummy artifact");
    f.write("unrelated.txt", "dummy artifact");
    f.write("expected.tar.gz.sha256", f.checksum("unrelated.txt"));
    f.rejects("Checksum does not identify the expected artifact");
    f.write("expected.tar.gz.sha256", f.checksum("expected.tar.gz"));
    f.write("expected.tar.gz", "modified artifact");
    f.rejects("Checksum does not identify the expected artifact");
}
