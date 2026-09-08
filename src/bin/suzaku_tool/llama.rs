use std::time::Instant;
use suzaku_map::ime::{
    EngineConfig, XRTabletImeEngine,
    settings::{ImeSettings, settings_path},
};
use suzaku_map::languages::{
    BuiltinLanguage,
    llama::{DEFAULT_LLAMA_ENDPOINT, DEFAULT_LLAMA_MODEL, OpenAiCompatibleLlamaProvider, runtime},
    llm::{LlmCompletionProvider, LlmCompletionRequest},
};

pub(super) fn run(args: &[String]) -> i32 {
    match dispatch(args) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("Llama: {error}");
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
            let status = runtime::inspect(&settings.provider).map_err(|error| error.to_string())?;
            println!("{}", status.summary());
            for model in status.models {
                println!("  {} · {} · {:.2} GiB{}", model.name, model.family, model.size_bytes as f64 / 1_073_741_824.0, if model.remote { " (remote, not used)" } else { "" });
            }
            if !status.installed { return Err(format!("请先安装本机模型 {}；Suzaku 不会在输入期间自动下载模型", settings.provider.model)); }
            Ok(())
        }
        "configure" => {
            settings = configured_settings(settings, &args[1..])?;
            settings.save()?;
            println!("Saved Llama configuration: {} · {}\nLLM enabled remains {}. Choose 重新加载模型配置 in the tray to apply.", settings.provider.model, settings.provider.endpoint, settings.llm_enabled);
            Ok(())
        }
        "warmup" if args.len() == 1 => {
            let started = Instant::now();
            runtime::warm_up(&settings.provider).map_err(|error| error.to_string())?;
            println!("Llama preloaded in {} ms; no input text was sent. Idle residency: 5 minutes.", started.elapsed().as_millis());
            Ok(())
        }
        "probe" if args.len() <= 2 => {
            let languages = match args.get(1).map(String::as_str).unwrap_or("all") {
                "all" => BuiltinLanguage::ALL.to_vec(),
                value => vec![BuiltinLanguage::resolve(value).ok_or("不支持的测试语言")?],
            };
            let provider = OpenAiCompatibleLlamaProvider::new(settings.provider.clone());
            if provider.uses_ollama_api() {
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
                println!("{}: local={} µs, Llama={} ms, literal={:?}, local-conversion={:?}", language.id(), local_micros, started.elapsed().as_millis(), seed, request.normalized_phrase);
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
        _ => Err("用法：llama status | warmup | probe [all|en|zh-Hans|ja] | configure [--model NAME] [--endpoint URL] [--timeout-ms N]".into()),
    }
}

fn configured_settings(current: ImeSettings, args: &[String]) -> Result<ImeSettings, String> {
    let mut json = current.to_json();
    if args.is_empty() {
        json["llm_endpoint"] = DEFAULT_LLAMA_ENDPOINT.into();
        json["llm_model"] = DEFAULT_LLAMA_MODEL.into();
    }
    if !args.len().is_multiple_of(2) {
        return Err("配置参数需要对应的值".into());
    }
    for pair in args.chunks_exact(2) {
        match pair[0].as_str() {
            "--model" => json["llm_model"] = pair[1].clone().into(),
            "--endpoint" => json["llm_endpoint"] = pair[1].clone().into(),
            "--timeout-ms" => {
                json["llm_timeout_ms"] = pair[1]
                    .parse::<u64>()
                    .map_err(|_| "超时必须为整数毫秒")?
                    .into()
            }
            option => return Err(format!("未知参数：{option}")),
        }
    }
    ImeSettings::from_json(&json.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
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
