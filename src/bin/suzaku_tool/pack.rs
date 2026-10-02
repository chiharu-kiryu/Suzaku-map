use std::path::Path;
use suzaku_map::{
    data::files::StagedFile,
    ime::{EngineConfig, XRTabletImeEngine},
    lexicon::packs::{self, OfflinePack, PackStore},
};

pub fn run(args: &[String]) -> i32 {
    match dispatch(args) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn dispatch(args: &[String]) -> Result<(), String> {
    let args: Vec<_> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["help" | "--help" | "-h"] => println!(
            "suzaku_tool pack list\nsuzaku_tool pack catalog [--language en|zh-Hans|ja] [--topic TOPIC]\nsuzaku_tool pack export ID FILE\nsuzaku_tool pack validate FILE\nsuzaku_tool pack install FILE [--replace]\nsuzaku_tool pack enable ID\nsuzaku_tool pack disable ID\nsuzaku_tool pack remove ID\nsuzaku_tool pack preview LANGUAGE TEXT\n\nPure data, no executable plugins/downloads. Install enables a new pack; --replace preserves its position and enabled state. Changes take effect after restarting BOTH host and panel. list/preview describe a fresh process, not the running desktop. Pack files are not included in settings backups."
        ),
        ["catalog", filters @ ..] => {
            let (mut language, mut topic) = (None, None);
            if filters.len() % 2 != 0 {
                return Err("catalog expects --language LANG and/or --topic TOPIC".into());
            }
            for pair in filters.chunks_exact(2) {
                match pair[0] {
                    "--language" if language.is_none() => language = Some(pair[1]),
                    "--topic" if topic.is_none() => topic = Some(pair[1]),
                    _ => return Err("unknown or repeated catalog filter".into()),
                }
            }
            if language.is_some_and(|id| suzaku_map::lexicon::builtin(id).is_none()) {
                return Err("unsupported language; use en, zh-Hans or ja".into());
            }
            let packages: Vec<_> = packs::recommended()
                .into_iter()
                .filter(|pack| {
                    language.is_none_or(|id| id == pack.manifest.language)
                        && topic.is_none_or(|tag| pack.manifest.topics.iter().any(|t| t == tag))
                })
                .map(|pack| pack.manifest)
                .collect();
            print_json(&packages)?;
        }
        ["export", id, file] => {
            let pack = packs::recommended()
                .into_iter()
                .find(|p| p.manifest.id == *id)
                .ok_or("unknown recommended package id; use pack catalog")?;
            let bytes = serde_json::to_vec_pretty(&pack).map_err(|e| e.to_string())?;
            StagedFile::new(Path::new(file), &bytes)
                .and_then(StagedFile::create)
                .map_err(|e| e.to_string())?;
            println!("Exported {id} to {file}; not installed.");
        }
        ["validate", file] => {
            let pack = OfflinePack::read(Path::new(file))?;
            println!(
                "Valid data pack: {} {} ({}, {} indexing units).",
                pack.manifest.id,
                pack.manifest.version,
                pack.manifest.language,
                pack.units()
            );
            println!(
                "Schema/decoder validation only, not a signature or license verification. Installation also checks active-pack conflicts."
            );
        }
        [] | ["list" | "status"] => {
            let store = PackStore::current()?;
            let entries = store.list().map_err(|error| {
                format!(
                    "{}: {error}; preserve a copy before repairing the registry",
                    store.path().display()
                )
            })?;
            let catalog = packs::Catalog::from_installed(&entries);
            print_json(&serde_json::json!({
                "registry": store.path(),
                "activation": "next process startup; restart host and panel",
                "packages": entries.iter().map(|e| serde_json::json!({"enabled":e.enabled,"manifest":e.package.manifest,"units":e.package.units()})).collect::<Vec<_>>(),
                "next_startup": catalog.report,
            }))?;
        }
        ["preview", language, seed] => {
            if suzaku_map::lexicon::builtin(language).is_none() {
                return Err("unsupported language".into());
            }
            let report = packs::initialize(&PackStore::current()?)?;
            let mut engine = XRTabletImeEngine::new(EngineConfig {
                default_language: (*language).into(),
                ..Default::default()
            });
            engine.enable_ibus_candidate_mix();
            engine.seed(seed);
            print_json(&serde_json::json!({
                "report": report,
                "candidates": engine.candidates().iter().map(|c| serde_json::json!({"text":c.text,"kind":format!("{:?}",c.kind)})).collect::<Vec<_>>()
            }))?;
        }
        ["install", file] | ["install", file, "--replace"] => {
            PackStore::current()?.install(OfflinePack::read(Path::new(file))?, args.len() == 3)?;
            changed();
        }
        ["enable", id] | ["disable", id] => {
            PackStore::current()?.set_enabled(id, args[0] == "enable")?;
            changed();
        }
        ["remove", id] => {
            PackStore::current()?.remove(id)?;
            changed();
        }
        _ => return Err("invalid arguments; see suzaku_tool pack --help".into()),
    }
    Ok(())
}

fn changed() {
    println!(
        "Saved offline pack registry. Restart the input host AND panel to apply; the running session is unchanged."
    );
}

fn print_json(value: &impl serde::Serialize) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(|e| e.to_string())?
    );
    Ok(())
}
