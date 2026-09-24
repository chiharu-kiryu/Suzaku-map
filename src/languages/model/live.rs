//! Explicit local-only profiling with synthetic input; never a normal CI gate.
use super::*;
use crate::ime::settings::ImeSettings;

fn local_provider() -> HttpModelProvider {
    assert_eq!(std::env::var("SUZAKU_MODEL_LOCAL_QA").as_deref(), Ok("1"));
    let path = std::path::PathBuf::from(
        std::env::var_os("SUZAKU_IME_CONFIG").expect("explicit private QA configuration"),
    );
    let path = path.canonicalize().expect("existing QA configuration");
    assert!(
        path.starts_with(std::env::temp_dir().canonicalize().unwrap()),
        "use an isolated configuration in the temporary directory, not personal settings"
    );
    let config = ImeSettings::load()
        .expect("isolated local model configuration")
        .provider;
    assert_eq!(config.scope, ModelScope::Local);
    assert!(config.uses_ollama_api());
    let config = runtime::resolve_cached(
        &config,
        &Mutex::new(None),
        Instant::now() + Duration::from_secs(2),
    )
    .expect("installed, non-remote local Ollama model");
    runtime::warm_up(&config).expect("explicit synthetic warmup");
    HttpModelProvider::new(config)
}

fn measure(provider: &HttpModelProvider, label: &str, body: Value) -> Value {
    // Diagnostic only: a completed response exposes model-side timing counters.
    // This does not change the production 20–5000 ms candidate budget.
    let started = Instant::now();
    let response = http_request(
        &provider.config.endpoint,
        "POST",
        Some(&body.to_string()),
        Duration::from_secs(30),
    )
    .expect("complete synthetic profiling response");
    let response: Value = serde_json::from_str(&response).unwrap();
    println!(
        "{label}: wall={} ms, load={} ms, prompt={} tokens/{} ms, output={} tokens/{} ms, done={:?}, text={:?}",
        started.elapsed().as_millis(),
        response["load_duration"].as_u64().unwrap_or(0) / 1_000_000,
        response["prompt_eval_count"],
        response["prompt_eval_duration"].as_u64().unwrap_or(0) / 1_000_000,
        response["eval_count"],
        response["eval_duration"].as_u64().unwrap_or(0) / 1_000_000,
        response["done_reason"],
        response["message"]["content"]
    );
    response
}

#[test]
#[ignore = "explicit SUZAKU_MODEL_LOCAL_QA=1; synthetic local-only model profiling"]
fn profile_synthetic_candidate_request() {
    let provider = local_provider();
    let request = LlmCompletionRequest {
        language_id: "en".into(),
        seed_text: "hel".into(),
        normalized_phrase: "hel".into(),
        context_before_cursor: String::new(),
        confidence: 1.0,
        degraded: false,
    };
    for seed in [
        "hello",
        "hel",
        "please sen",
        "good m",
        "thank you ",
        "Could you sh",
        "see you tom",
    ] {
        let request = LlmCompletionRequest {
            seed_text: seed.into(),
            normalized_phrase: seed.into(),
            ..request.clone()
        };
        let body: Value = serde_json::from_str(&provider.request_body(&request)).unwrap();
        let response = measure(&provider, &format!("current {seed:?}"), body);
        let candidates =
            parse_ollama_candidates(&response.to_string(), completion_prefix(&request)).map(
                |texts| {
                    texts
                        .into_iter()
                        .filter(|text| useful_candidate(text, &request))
                        .collect::<Vec<_>>()
                },
            );
        println!(
            "accepted by strict parser/filter: {candidates:?}; diagnostic completion is NOT the production deadline gate"
        );
    }
}

#[test]
#[ignore = "explicit SUZAKU_MODEL_LOCAL_QA=1; synthetic local-only translation diagnostics"]
fn diagnose_synthetic_translation_holdouts() {
    use crate::languages::translation::{
        TranslationLanguage, TranslationProvider, TranslationRequest,
    };
    let provider = local_provider();
    for target in [
        TranslationLanguage::ChineseSimplified,
        TranslationLanguage::Japanese,
    ] {
        for text in [
            "Can you send the report by Friday? Do not change the numbers.",
            "Please keep order 42. Can we meet at 3 pm?",
        ] {
            let request = TranslationRequest {
                text: text.into(),
                source: Some(TranslationLanguage::English),
                target,
            };
            let started = Instant::now();
            let result = provider.translate(&request);
            println!(
                "{} {text:?}: {} ms, {result:?}; non-rejection is NOT semantic-quality approval",
                target.id(),
                started.elapsed().as_millis()
            );
            assert_eq!(request.text, text);
        }
    }
}
