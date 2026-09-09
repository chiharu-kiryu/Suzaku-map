use std::time::Instant;
use suzaku_map::ime::{
    EngineConfig, XRTabletImeEngine,
    settings::{ImeSettings, settings_path},
};
use suzaku_map::languages::{
    BuiltinLanguage,
    llm::{LlmCompletionProvider, LlmCompletionRequest},
    model::{DEFAULT_LOCAL_ENDPOINT, DEFAULT_MODEL, HttpModelProvider, ModelScope, runtime},
};

pub(super) fn run(args: &[String]) -> i32 {
    match dispatch(args) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("Model: {error}");
            1
        }
    }
}

fn dispatch(args: &[String]) -> Result<(), String> {
    let mut settings = ImeSettings::load()?;
    match args.first().map(String::as_str).unwrap_or("status") {
        "status" if args.len() <= 1 => {
            println!("settings: {}", settings_path().map(|path| path.display().to_string()).unwrap_or_default());
            println!("endpoint: {}\nmodel: {}\ninput-language: {}\nLLM enabled: {}", settings.provider.endpoint, settings.provider.model, settings.language.id(), settings.llm_enabled);
            println!("scope: {}\nprotocol: {}\ncloud consent: {}", settings.provider.scope.id(), settings.provider.protocol.id(), settings.provider.cloud_consent);
            if settings.provider.scope == ModelScope::Cloud {
                println!("Cloud configuration only; no remote request was made. Use probe explicitly for synthetic examples.");
                println!("credential environment configured: {}", settings.provider.api_key_env.is_some());
                return Ok(());
            }
            let status = runtime::inspect(&settings.provider).map_err(|error| error.to_string())?;
            println!("{}", status.summary());
            for model in status.models {
                println!("  {} · {} · {:.2} GiB{}", model.name, model.family, model.size_bytes as f64 / 1_073_741_824.0, if model.remote { " (remote, not used)" } else { "" });
            }
            if !status.installed { return Err(format!("请先安装本机模型 {}；Suzaku 不会在输入期间自动下载模型", settings.provider.model)); }
            Ok(())
        }
        "discover" if args.len() == 1 => {
            let resolved = runtime::discover(&settings.provider).map_err(|error| error.to_string())?;
            println!("Discovered local model: {}\nendpoint: {}\nprotocol: {}\nMetadata only; no model loaded, downloaded, or settings changed.", resolved.model, resolved.endpoint, if resolved.uses_ollama_api() { "ollama" } else { "openai-compatible" });
            Ok(())
        }
        "configure" => {
            settings = configured_settings(settings, &args[1..])?;
            settings.save()?;
            println!("Saved model configuration: {} · {} · {}\nLLM enabled remains {}. Choose 重新加载模型配置 in the tray to apply.", settings.provider.scope.id(), settings.provider.model, settings.provider.endpoint, settings.llm_enabled);
            if settings.provider.scope == ModelScope::Cloud {
                println!("Cloud consent: {}. When enabled AND authorized, composition and recent in-session commits are sent to this endpoint. Credentials are read by the IME host from its environment, never saved here.", settings.provider.cloud_consent);
            }
            Ok(())
        }
        "warmup" if args.len() == 1 => {
            let started = Instant::now();
            runtime::warm_up(&settings.provider).map_err(|error| error.to_string())?;
            println!("Model preloaded in {} ms; no input text was sent. Idle residency: 5 minutes.", started.elapsed().as_millis());
            Ok(())
        }
        "probe" if args.len() <= 2 => {
            let languages = match args.get(1).map(String::as_str).unwrap_or("all") {
                "all" => BuiltinLanguage::ALL.to_vec(),
                value => vec![BuiltinLanguage::resolve(value).ok_or("不支持的测试语言")?],
            };
            let provider = HttpModelProvider::new(settings.provider.clone());
            if settings.provider.scope == ModelScope::Local && provider.uses_ollama_api() && settings.provider.model != DEFAULT_MODEL {
                println!("Preloading configured model with an empty request…");
                runtime::warm_up(&settings.provider).map_err(|error| error.to_string())?;
            }
            println!("Synthetic IME examples only; no live input is read, no engine is switched.");
            let mut failures = 0;
            for language in languages {
                let seeds: &[&str] = match language { BuiltinLanguage::ChineseSimplified => &["nihao"], BuiltinLanguage::English => &["hello", "hel", "please sen", "good m", "thank you "], BuiltinLanguage::Japanese => &["nihongo"] };
                for &seed in seeds {
                let mut engine = XRTabletImeEngine::new(EngineConfig {default_language:language.id().into(), ..Default::default()});
                let started = Instant::now();
                engine.seed(seed);
                let local_micros = started.elapsed().as_micros();
                let request = LlmCompletionRequest {language_id:language.id().into(), seed_text:seed.into(), normalized_phrase:engine.candidates()[0].text.clone(), context_before_cursor:String::new(), confidence:1.0, degraded:false};
                let started = Instant::now();
                let result = provider.generate_checked(&request);
                println!("{}: local={} µs, model={} ms, literal={:?}, local-conversion={:?}", language.id(), local_micros, started.elapsed().as_millis(), seed, request.normalized_phrase);
                println!("  offline: {:?}", engine.candidates().iter().skip(1).map(|candidate| candidate.text.as_str()).collect::<Vec<_>>());
                match result {
                    Ok(candidates) => {
                        for candidate in candidates { println!("  {:?}", candidate.text); }
                    }
                    Err(error) => { failures += 1; println!("  unavailable: {error}"); }
                }
                }
            }
            println!("This checks transport and latency, not a comprehensive language-quality benchmark.");
            if failures > 0 { Err(format!("{failures} synthetic probe(s) returned no valid model candidates")) } else { Ok(()) }
        }
        _ => Err("用法：model status | discover | warmup | probe [all|en|zh-Hans|ja] | configure [--scope local|cloud] [--protocol auto|ollama|openai-compatible] [--model auto|NAME] [--endpoint URL] [--timeout-ms N] [--api-key-env NAME|none] [--cloud-consent true|false]".into()),
    }
}

fn configured_settings(current: ImeSettings, args: &[String]) -> Result<ImeSettings, String> {
    let mut json = current.to_json();
    if args.is_empty() {
        json["llm_endpoint"] = DEFAULT_LOCAL_ENDPOINT.into();
        json["llm_model"] = DEFAULT_MODEL.into();
        json["llm_scope"] = "local".into();
        json["llm_protocol"] = "auto".into();
        json["llm_api_key_env"] = serde_json::Value::Null;
        json["llm_cloud_consent"] = false.into();
    }
    if !args.len().is_multiple_of(2) {
        return Err("配置参数需要对应的值".into());
    }
    let mut consent = None;
    for pair in args.chunks_exact(2) {
        match pair[0].as_str() {
            "--model" => json["llm_model"] = pair[1].clone().into(),
            "--endpoint" => json["llm_endpoint"] = pair[1].clone().into(),
            "--scope" => json["llm_scope"] = pair[1].clone().into(),
            "--protocol" => json["llm_protocol"] = pair[1].clone().into(),
            "--api-key-env" => {
                json["llm_api_key_env"] = if pair[1] == "none" {
                    serde_json::Value::Null
                } else {
                    pair[1].clone().into()
                }
            }
            "--cloud-consent" => {
                consent = Some(
                    pair[1]
                        .parse::<bool>()
                        .map_err(|_| "云端授权必须为 true 或 false")?,
                )
            }
            "--timeout-ms" => {
                json["llm_timeout_ms"] = pair[1]
                    .parse::<u64>()
                    .map_err(|_| "超时必须为整数毫秒")?
                    .into()
            }
            option => return Err(format!("未知参数：{option}")),
        }
    }
    let previous = current.to_json();
    if [
        "llm_scope",
        "llm_protocol",
        "llm_model",
        "llm_endpoint",
        "llm_api_key_env",
    ]
    .iter()
    .any(|key| json[key] != previous[key])
    {
        json["llm_cloud_consent"] = false.into();
    }
    if let Some(consent) = consent {
        json["llm_cloud_consent"] = consent.into();
    }
    ImeSettings::from_json(&json.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).into()).collect()
    }

    #[test]
    fn cloud_configuration_and_target_changes_require_explicit_consent() {
        let configured = configured_settings(
            ImeSettings::default(),
            &args(&[
                "--scope",
                "cloud",
                "--endpoint",
                "https://models.example/v1/chat/completions",
                "--model",
                "custom-family",
                "--api-key-env",
                "SUZAKU_MODEL_KEY",
            ]),
        )
        .unwrap();
        assert!(!configured.provider.cloud_consent);
        assert!(!configured.llm_enabled);
        let authorized =
            configured_settings(configured, &args(&["--cloud-consent", "true"])).unwrap();
        assert!(authorized.provider.cloud_consent);
        let changed = configured_settings(
            authorized.clone(),
            &args(&["--endpoint", "https://another.example/v1/chat/completions"]),
        )
        .unwrap();
        assert!(!changed.provider.cloud_consent);
        let tuning =
            configured_settings(authorized.clone(), &args(&["--timeout-ms", "3000"])).unwrap();
        assert!(tuning.provider.cloud_consent);
        let reset = configured_settings(authorized, &[]).unwrap();
        assert_eq!(reset.provider.scope, ModelScope::Local);
        assert_eq!(reset.provider.model, "auto");
        assert_eq!(reset.provider.api_key_env, None);
        assert!(!reset.provider.cloud_consent);
    }
    #[test]
    fn llama_configuration_preserves_language_opt_in_and_validates_before_save() {
        let current = ImeSettings {
            language: BuiltinLanguage::Japanese,
            llm_enabled: false,
            ..Default::default()
        };
        let configured = configured_settings(
            current.clone(),
            &[
                "--model".into(),
                "llama3.2:1b".into(),
                "--timeout-ms".into(),
                "2400".into(),
            ],
        )
        .unwrap();
        assert_eq!(configured.language, BuiltinLanguage::Japanese);
        assert!(!configured.llm_enabled);
        assert_eq!(configured.provider.model, "llama3.2:1b");
        assert_eq!(configured.provider.timeout_ms, 2400);
        assert!(configured_settings(current.clone(), &["--model".into()]).is_err());
        assert!(
            configured_settings(current, &["--endpoint".into(), "http://example.com".into()])
                .is_err()
        );
    }
}
