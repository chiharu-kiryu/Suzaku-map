//! Data-only offline packs. Installation is transactional; activation is a
//! bounded, immutable startup snapshot, independent of hosts and model APIs.
use super::{Lexicon, builtin, check_text};
use crate::data::{files, paths};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::{
    collections::{BTreeMap, HashSet},
    env,
    path::{Path, PathBuf},
    sync::OnceLock,
};

pub const MAX_PACK_BYTES: usize = 256 * 1024;
pub const MAX_PACK_UNITS: usize = 2048;
pub const MAX_LANGUAGE_UNITS: usize = 4096;
pub const MAX_PACKS: usize = 16;
const MAX_REGISTRY_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub id: String,
    pub version: String,
    pub name: String,
    pub description: String,
    pub language: String,
    pub topics: Vec<String>,
    pub license: String,
    pub authors: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OfflinePack {
    pub format_version: u32,
    pub manifest: Manifest,
    pub lexicon: Lexicon,
}

impl OfflinePack {
    pub fn from_json(raw: &str) -> Result<Self, String> {
        if raw.len() > MAX_PACK_BYTES {
            return Err("offline pack exceeds 256 KiB".into());
        }
        let pack: Self = serde_json::from_str(raw).map_err(|e| e.to_string())?;
        pack.validate()?;
        Ok(pack)
    }

    pub fn read(path: &Path) -> Result<Self, String> {
        let raw = files::read_optional(path, MAX_PACK_BYTES)
            .map_err(|e| e.to_string())?
            .ok_or("offline pack file does not exist")?;
        Self::from_json(&raw)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format_version != 1 {
            return Err("unsupported offline pack format_version".into());
        }
        let m = &self.manifest;
        identifier(&m.id, 64)?;
        if m.version.split('.').count() != 3
            || !m.version.split('.').all(|part| {
                !part.is_empty()
                    && part.len() <= 6
                    && part.bytes().all(|b| b.is_ascii_digit())
                    && (part == "0" || !part.starts_with('0'))
            })
        {
            return Err("version must be MAJOR.MINOR.PATCH without leading zeroes".into());
        }
        for (value, limit) in [(&m.name, 128), (&m.description, 512), (&m.license, 128)] {
            metadata(value, limit)?;
        }
        if m.authors.is_empty() || m.authors.len() > 8 {
            return Err("provide 1..8 authors".into());
        }
        for author in &m.authors {
            metadata(author, 80)?;
        }
        if m.topics.is_empty() || m.topics.len() > 8 {
            return Err("provide 1..8 topics".into());
        }
        let mut topics = HashSet::new();
        for topic in &m.topics {
            identifier(topic, 32)?;
            if !topics.insert(topic) {
                return Err("duplicate topic".into());
            }
        }
        if builtin(&m.language).is_none() || m.language != self.lexicon.language() {
            return Err(
                "pack/resource language must match a supported decoder: en, zh-Hans, ja".into(),
            );
        }
        self.lexicon.validate()?;
        self.validate_decoder_fields()?;
        if self.lexicon.word_layers.len() > 8 {
            return Err("a pack may contain at most 8 layers".into());
        }
        if !(1..=MAX_PACK_UNITS).contains(&self.units()) {
            return Err("a pack must contain 1..2048 indexing units".into());
        }
        if serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > MAX_PACK_BYTES {
            return Err("offline pack exceeds 256 KiB".into());
        }
        Ok(())
    }

    // Data remains decoder-independent; these are explicit v1 compatibility
    // profiles so unsupported fields are rejected rather than silently ignored.
    fn validate_decoder_fields(&self) -> Result<(), String> {
        let lex = &self.lexicon;
        if lex.language == "en" {
            if !lex.readings.is_empty() || !lex.continuations.is_empty() {
                return Err(
                    "English uses word_layers and phrase_endings, not readings/continuations"
                        .into(),
                );
            }
            for word in lex.word_layers.iter().flat_map(|layer| {
                layer
                    .words
                    .iter()
                    .chain(layer.next_words.iter().flat_map(|(_, words)| words))
            }) {
                if !word
                    .chars()
                    .all(|ch| ch.is_ascii_alphabetic() || matches!(ch, '\'' | '’'))
                    || !word.chars().any(|ch| ch.is_ascii_alphabetic())
                {
                    return Err(format!("unsupported English word form: {word}"));
                }
            }
        } else {
            if !lex.word_layers.is_empty() || !lex.phrase_endings.is_empty() {
                return Err("Chinese/Japanese use readings and continuations, not word_layers/phrase_endings".into());
            }
            for entry in &lex.readings {
                let valid = if lex.language == "zh-Hans" {
                    entry.reading.split('\'').all(|syllable| {
                        !syllable.is_empty() && syllable.bytes().all(|ch| ch.is_ascii_lowercase())
                    })
                } else {
                    !entry.require_separators
                        && entry
                            .reading
                            .chars()
                            .all(|ch| matches!(ch, 'ぁ'..='ゖ' | 'ー'))
                };
                if !valid {
                    return Err(format!(
                        "invalid {} reading: {}",
                        lex.language, entry.reading
                    ));
                }
            }
        }
        Ok(())
    }

    /// Conservative work budget, including sentence token projections. It is
    /// not a unique-word count and does not change decoder candidate limits.
    pub fn units(&self) -> usize {
        let lex = &self.lexicon;
        lex.readings.len()
            + lex
                .word_layers
                .iter()
                .map(|layer| {
                    layer.words.len()
                        + layer
                            .next_words
                            .iter()
                            .map(|(_, words)| 1 + words.len())
                            .sum::<usize>()
                        + layer
                            .sentences
                            .iter()
                            .map(|s| {
                                1 + s
                                    .split(|ch: char| !ch.is_ascii_alphabetic() && ch != '\'')
                                    .filter(|word| !word.is_empty())
                                    .count()
                            })
                            .sum::<usize>()
                })
                .sum::<usize>()
            + lex
                .continuations
                .iter()
                .chain(&lex.phrase_endings)
                .map(|(_, values)| 1 + values.len())
                .sum::<usize>()
    }
}

fn metadata(value: &str, limit: usize) -> Result<(), String> {
    check_text(value)?;
    if value.chars().count() > limit {
        return Err("oversized package metadata".into());
    }
    Ok(())
}

fn identifier(value: &str, limit: usize) -> Result<(), String> {
    if value.is_empty()
        || value.len() > limit
        || !value
            .bytes()
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase())
        || !value
            .bytes()
            .last()
            .is_some_and(|ch| ch.is_ascii_alphanumeric())
        || !value
            .bytes()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, b'.' | b'-'))
        || value.contains("..")
    {
        return Err("invalid package/topic identifier (lowercase slug required)".into());
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InstalledPack {
    pub enabled: bool,
    pub package: OfflinePack,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    format_version: u32,
    // Individual records are decoded separately at startup: one damaged record
    // must not prevent independent valid packs from loading. Keep the original
    // JSON until strict InstalledPack deserialization: Value would silently
    // discard duplicate enabled/package fields and nested package fields.
    packages: Vec<Box<RawValue>>,
}

#[derive(Clone, Debug)]
pub struct PackStore {
    directory: PathBuf,
}

impl PackStore {
    pub fn new(directory: PathBuf) -> Result<Self, String> {
        if !directory.is_absolute() {
            return Err("offline pack directory must be absolute".into());
        }
        Ok(Self { directory })
    }

    pub fn current() -> Result<Self, String> {
        Self::new(match env::var_os("SUZAKU_LEXICON_DIR") {
            Some(path) => PathBuf::from(path),
            None => paths::data_home()
                .ok_or("cannot locate user data directory")?
                .join("suzaku/lexicons"),
        })
    }

    pub fn path(&self) -> PathBuf {
        self.directory.join("registry.json")
    }

    fn registry(&self) -> Result<Registry, String> {
        let raw =
            files::read_optional(&self.path(), MAX_REGISTRY_BYTES).map_err(|e| e.to_string())?;
        let registry: Registry = match raw {
            Some(raw) => serde_json::from_str(&raw)
                .map_err(|e| format!("invalid offline pack registry: {e}"))?,
            None => Registry {
                format_version: 1,
                packages: vec![],
            },
        };
        if registry.format_version != 1 || registry.packages.len() > MAX_PACKS {
            return Err("unsupported registry version or more than 16 installed packs".into());
        }
        Ok(registry)
    }

    /// Read-only: does not create directories, locks or default files.
    pub fn list(&self) -> Result<Vec<InstalledPack>, String> {
        let mut ids = HashSet::new();
        self.registry()?
            .packages
            .into_iter()
            .map(|raw| {
                let entry = decode_entry(&raw)?;
                if !ids.insert(entry.package.manifest.id.clone()) {
                    return Err("duplicate installed package id".into());
                }
                Ok(entry)
            })
            .collect()
    }

    pub fn install(&self, pack: OfflinePack, replace: bool) -> Result<(), String> {
        pack.validate()?;
        self.update(move |entries| {
            if let Some(entry) = entries
                .iter_mut()
                .find(|e| e.package.manifest.id == pack.manifest.id)
            {
                if !replace {
                    return Err("package already installed; use --replace explicitly".into());
                }
                if entry.package.manifest.language != pack.manifest.language {
                    return Err("replacement must keep the package language".into());
                }
                entry.package = pack;
            } else {
                entries.push(InstalledPack {
                    enabled: true,
                    package: pack,
                });
            }
            Ok(())
        })
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
        self.update(|entries| {
            entries
                .iter_mut()
                .find(|e| e.package.manifest.id == id)
                .ok_or("package is not installed")?
                .enabled = enabled;
            Ok(())
        })
    }

    pub fn remove(&self, id: &str) -> Result<(), String> {
        self.update(|entries| {
            let index = entries
                .iter()
                .position(|e| e.package.manifest.id == id)
                .ok_or("package is not installed")?;
            entries.remove(index);
            Ok(())
        })
    }

    fn update(
        &self,
        mutate: impl FnOnce(&mut Vec<InstalledPack>) -> Result<(), String>,
    ) -> Result<(), String> {
        let _lease = files::DataLease::acquire(&self.directory.join(".registry.lock"), true)
            .map_err(|_| "offline pack registry is busy or its lock is inaccessible".to_owned())?;
        let mut entries = self.list()?;
        mutate(&mut entries)?;
        if entries.len() > MAX_PACKS {
            return Err("at most 16 packs may be installed".into());
        }
        let catalog = Catalog::from_entries(entries.iter().cloned().map(Ok));
        if !catalog.report.errors.is_empty() {
            return Err(catalog.report.errors.join("\n"));
        }
        let registry = Registry {
            format_version: 1,
            packages: entries
                .iter()
                .map(serde_json::value::to_raw_value)
                .collect::<Result<_, _>>()
                .map_err(|e| e.to_string())?,
        };
        let bytes = serde_json::to_vec(&registry).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_REGISTRY_BYTES {
            return Err("offline pack registry exceeds 4 MiB".into());
        }
        files::atomic_write(&self.path(), &bytes).map_err(|e| e.to_string())
    }

    pub fn load(&self) -> Catalog {
        match self.registry() {
            Ok(registry) => {
                Catalog::from_entries(registry.packages.iter().map(|raw| decode_entry(raw)))
            }
            Err(error) => Catalog {
                report: LoadReport {
                    errors: vec![error],
                    ..Default::default()
                },
                ..Default::default()
            },
        }
    }
}

fn decode_entry(raw: &RawValue) -> Result<InstalledPack, String> {
    let entry: InstalledPack = serde_json::from_str(raw.get()).map_err(|e| e.to_string())?;
    entry
        .package
        .validate()
        .map_err(|e| format!("{}: {e}", entry.package.manifest.id))?;
    Ok(entry)
}

#[derive(Debug, Default, Serialize)]
pub struct LoadReport {
    pub loaded: Vec<String>,
    pub disabled: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Default)]
pub struct Catalog {
    lexicons: BTreeMap<String, Lexicon>,
    pub report: LoadReport,
}

impl Catalog {
    /// Inspect one already-read registry generation, without filesystem access.
    pub fn from_installed(entries: &[InstalledPack]) -> Self {
        Self::from_entries(entries.iter().map(|entry| {
            entry.package.validate()?;
            Ok(entry.clone())
        }))
    }

    pub fn get(&self, language: &str) -> Option<&Lexicon> {
        self.lexicons.get(language).or_else(|| builtin(language))
    }

    fn from_entries(entries: impl IntoIterator<Item = Result<InstalledPack, String>>) -> Self {
        let mut catalog = Self::default();
        let mut ids = HashSet::new();
        let mut budgets = BTreeMap::<String, usize>::new();
        for entry in entries {
            let result = entry.and_then(|entry| {
                let pack = entry.package;
                let id = &pack.manifest.id;
                if !ids.insert(id.clone()) {
                    return Err(format!("{id}: duplicate installed package id"));
                }
                if !entry.enabled {
                    catalog.report.disabled.push(id.clone());
                    return Ok(());
                }
                let language = &pack.manifest.language;
                let budget = budgets.entry(language.clone()).or_default();
                if *budget + pack.units() > MAX_LANGUAGE_UNITS {
                    return Err(format!(
                        "{id}: enabled packs exceed 4096 indexing units for {language}"
                    ));
                }
                let mut merged = catalog.get(language).ok_or("unsupported language")?.clone();
                append(&mut merged, &pack).map_err(|e| format!("{id}: {e}"))?;
                *budget += pack.units();
                catalog.lexicons.insert(language.clone(), merged);
                catalog.report.loaded.push(id.clone());
                Ok(())
            });
            if let Err(error) = result {
                catalog.report.errors.push(error);
            }
        }
        catalog
    }
}

fn append(target: &mut Lexicon, pack: &OfflinePack) -> Result<(), String> {
    for layer in &pack.lexicon.word_layers {
        let mut layer = layer.clone();
        layer.id = format!("{}:{}", pack.manifest.id, layer.id);
        layer.next_words = novel_pairs(target.next_words(), &layer.next_words)?;
        target.word_layers.push(layer);
    }
    for entry in &pack.lexicon.readings {
        if let Some(existing) = target
            .readings
            .iter()
            .find(|e| e.reading == entry.reading && e.text == entry.text)
        {
            if existing != entry {
                return Err(format!("conflicting reading flags/type: {}", entry.reading));
            }
        } else {
            target.readings.push(entry.clone());
        }
    }
    target.continuations.extend(novel_pairs(
        target.continuations(),
        &pack.lexicon.continuations,
    )?);
    target.phrase_endings.extend(novel_pairs(
        target.phrase_endings(),
        &pack.lexicon.phrase_endings,
    )?);
    target.validate()
}

fn novel_pairs(
    existing: impl Iterator<Item = (impl AsRef<str>, impl AsRef<[String]>)>,
    incoming: &[(String, Vec<String>)],
) -> Result<Vec<(String, Vec<String>)>, String> {
    let existing: BTreeMap<String, Vec<String>> = existing
        .map(|(k, v)| (k.as_ref().into(), v.as_ref().into()))
        .collect();
    let mut output = Vec::new();
    for (key, values) in incoming {
        match existing.get(key) {
            Some(previous) if previous != values => {
                return Err(format!(
                    "context already exists with different values: {key}; use a more specific context"
                ));
            }
            Some(_) => {}
            None => output.push((key.clone(), values.clone())),
        }
    }
    Ok(output)
}

static ACTIVE: OnceLock<Catalog> = OnceLock::new();

/// Small authored collections shipped with the tool; browsing/exporting never
/// downloads, installs or enables anything. Topics are stable filter tags.
pub fn recommended() -> Vec<OfflinePack> {
    [
        include_str!("../../data/offline-packs/en-outdoors.json"),
        include_str!("../../data/offline-packs/zh-Hans-outdoors.json"),
        include_str!("../../data/offline-packs/ja-rail.json"),
        include_str!("../../data/offline-packs/en-study.json"),
        include_str!("../../data/offline-packs/zh-Hans-study.json"),
        include_str!("../../data/offline-packs/en-cooking.json"),
        include_str!("../../data/offline-packs/zh-Hans-cooking.json"),
        include_str!("../../data/offline-packs/en-travel.json"),
        include_str!("../../data/offline-packs/zh-Hans-travel.json"),
        include_str!("../../data/offline-packs/en-work.json"),
        include_str!("../../data/offline-packs/zh-Hans-work.json"),
    ]
    .into_iter()
    .map(|raw| OfflinePack::from_json(raw).expect("bundled offline pack must pass validation"))
    .collect()
}

pub fn runtime() -> &'static Catalog {
    ACTIVE.get_or_init(Catalog::default)
}

/// Call before constructing any engine. Late reinitialization is an error, not
/// a partial reload underneath already-created language indexes.
pub fn initialize(store: &PackStore) -> Result<&'static LoadReport, String> {
    if ACTIVE.get().is_some() {
        return Err("offline vocabulary is already frozen; restart the process".into());
    }
    ACTIVE
        .set(store.load())
        .map_err(|_| "offline vocabulary is already frozen; restart the process".to_owned())?;
    Ok(&runtime().report)
}

/// Host entry point: damaged/unavailable packs never stop built-in input.
pub fn initialize_current() {
    match PackStore::current().and_then(|store| initialize(&store)) {
        Ok(report) => {
            for error in &report.errors {
                eprintln!("Suzaku offline pack skipped: {error}");
            }
            if !report.loaded.is_empty() {
                eprintln!("Suzaku offline packs loaded: {}", report.loaded.join(", "));
            }
        }
        Err(error) => eprintln!("Suzaku offline packs unavailable: {error}"),
    }
}
