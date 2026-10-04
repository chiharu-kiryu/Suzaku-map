//! Raw registry ambiguity must be rejected before serde_json::Value can discard
//! duplicate object fields. Fixtures are owned and never initialize runtime().
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use suzaku_map::lexicon::packs::{self, PackStore};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "suzaku-pack-edge-audit-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn store(&self) -> PackStore {
        PackStore::new(self.0.clone()).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn ambiguous_registry_fields_are_isolated_without_enabling_or_erasing_them() {
    let mut source = packs::recommended().remove(0);
    source.manifest.id = "example.ambiguous".into();
    let package = serde_json::to_string(&source).unwrap();
    let duplicate_id = package.replacen(
        "\"id\":\"example.ambiguous\"",
        "\"id\":\"example.discarded\",\"id\":\"example.ambiguous\"",
        1,
    );
    assert_ne!(duplicate_id, package);
    let duplicate_language = package.replacen(
        "\"language\":\"en\"",
        "\"language\":\"ja\",\"language\":\"en\"",
        1,
    );
    assert_ne!(duplicate_language, package);
    let duplicate_version = package.replacen(
        "\"format_version\":1",
        "\"format_version\":999,\"format_version\":1",
        1,
    );
    assert_ne!(duplicate_version, package);
    let layer_id = serde_json::to_string(&source.lexicon.word_layers()[0].id).unwrap();
    let duplicate_layer_id = package.replacen(
        &format!("\"id\":{layer_id}"),
        &format!("\"id\":\"discarded-layer\",\"id\":{layer_id}"),
        1,
    );
    assert_ne!(duplicate_layer_id, package);
    for raw in [
        &duplicate_id,
        &duplicate_language,
        &duplicate_version,
        &duplicate_layer_id,
    ] {
        assert!(
            packs::OfflinePack::from_json(raw).is_err(),
            "a direct package read already rejects duplicate known fields"
        );
    }

    let mut good = packs::recommended().remove(1);
    good.manifest.id = "example.independent".into();
    let good_entry = serde_json::json!({"enabled":true,"package":good}).to_string();
    for (label, ambiguous_entry) in [
        (
            "enabled flag",
            format!("{{\"enabled\":false,\"enabled\":true,\"package\":{package}}}"),
        ),
        (
            "escaped enabled flag",
            format!("{{\"enabled\":false,\"enab\\u006ced\":true,\"package\":{package}}}"),
        ),
        (
            "package object",
            format!(
                "{{\"enabled\":true,\"package\":{{\"unexpected\":true}},\"package\":{package}}}"
            ),
        ),
        (
            "manifest id",
            format!("{{\"enabled\":true,\"package\":{duplicate_id}}}"),
        ),
        (
            "manifest language",
            format!("{{\"enabled\":true,\"package\":{duplicate_language}}}"),
        ),
        (
            "package version",
            format!("{{\"enabled\":true,\"package\":{duplicate_version}}}"),
        ),
        (
            "nested layer id",
            format!("{{\"enabled\":true,\"package\":{duplicate_layer_id}}}"),
        ),
    ] {
        let fixture = Fixture::new();
        let store = fixture.store();
        let raw = format!("{{\"format_version\":1,\"packages\":[{ambiguous_entry},{good_entry}]}}");
        fs::write(store.path(), &raw).unwrap();

        let catalog = store.load();
        assert_eq!(
            catalog.report.loaded,
            ["example.independent"],
            "duplicate {label} must not load its ambiguous package: {:?}",
            catalog.report
        );
        assert_eq!(catalog.report.errors.len(), 1, "{label}");
        assert!(catalog.report.disabled.is_empty(), "{label}");
        assert!(store.list().is_err(), "{label}");
        assert!(store.remove("example.independent").is_err(), "{label}");
        assert_eq!(fs::read_to_string(store.path()).unwrap(), raw, "{label}");
    }
}

#[test]
fn registry_strict_parsing_preserves_valid_metadata_and_disabled_state() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let mut pack = packs::recommended().remove(0);
    pack.manifest.id = "example.valid-metadata".into();
    pack.manifest.description =
        r#"Example strings: "enabled":false,"enabled":true and {braces}."#.into();
    store.install(pack, false).unwrap();
    store.set_enabled("example.valid-metadata", false).unwrap();
    let before = fs::read(store.path()).unwrap();
    let entries = store.list().unwrap();
    assert_eq!(entries.len(), 1);
    assert!(!entries[0].enabled);
    assert_eq!(
        entries[0].package.manifest.description,
        r#"Example strings: "enabled":false,"enabled":true and {braces}."#
    );
    let catalog = store.load();
    assert!(catalog.report.loaded.is_empty());
    assert!(catalog.report.errors.is_empty());
    assert_eq!(catalog.report.disabled, ["example.valid-metadata"]);
    assert_eq!(fs::read(store.path()).unwrap(), before);
    store.set_enabled("example.valid-metadata", true).unwrap();
    assert_eq!(store.load().report.loaded, ["example.valid-metadata"]);
    store.remove("example.valid-metadata").unwrap();
    assert!(store.list().unwrap().is_empty());
}
