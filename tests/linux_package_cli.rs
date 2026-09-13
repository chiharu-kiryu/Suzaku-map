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

#[test]
fn packaged_audit_links_cover_references_inline_links_and_flattened_guides() {
    let f = Fixture::new();
    fs::create_dir(f.0.join("docs")).unwrap();
    f.write("docs/functional-network.md", "[first]: bug-audit-first.md#details\n\n[one](bug-audit-first.md) and [two](bug-audit-second.md)\n");
    f.write("docs/bug-audit-first.md", "first report");
    let check = || {
        Command::new("python3")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/scripts/check-package-audit-links.py"
            ))
            .arg(&f.0)
            .output()
            .unwrap()
    };
    let result = check();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("bug-audit-second.md"));
    f.write("docs/bug-audit-second.md", "");
    assert!(
        !check().status.success(),
        "empty reports must not count as present"
    );
    f.write("docs/bug-audit-second.md", "second report");
    assert!(check().status.success());
    f.write("README.md", "[flattened guide](bug-audit-first.md)\n");
    assert!(
        !check().status.success(),
        "a flat guide must not resolve from the docs directory"
    );
    f.write("README.md", "[flattened guide](docs/bug-audit-first.md)\n[remote](https://example.invalid/bug-audit-remote.md)\n```text\n[example](bug-audit-example.md)\n```\n");
    assert!(check().status.success());
    for guide in [
        "linux-packaging-data.md",
        "model-providers.md",
        "ibus-candidates.md",
        "translation.md",
        "interface-languages.md",
    ] {
        f.write(guide, "[audit](bug-audit-second.md)\n");
        let result = check();
        assert!(
            !result.status.success(),
            "must check {guide}, not just README"
        );
        assert!(String::from_utf8_lossy(&result.stderr).contains(guide));
        f.write(guide, "[audit](docs/bug-audit-second.md)\n");
        assert!(check().status.success());
    }
    // Tarball-level guides live outside the main documentation root and must
    // be passed to the checker explicitly, including guides other than README.
    fs::create_dir(f.0.join("tar-root")).unwrap();
    let extra = f.0.join("tar-root/model-providers.md");
    let check_extra = || {
        Command::new("python3")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/scripts/check-package-audit-links.py"
            ))
            .arg(&f.0)
            .arg(&extra)
            .output()
            .unwrap()
    };
    f.write(
        "tar-root/model-providers.md",
        "[audit](bug-audit-second.md)\n",
    );
    assert!(!check_extra().status.success());
    f.write(
        "tar-root/model-providers.md",
        "[audit](../docs/bug-audit-second.md)\n",
    );
    assert!(check_extra().status.success());
}
