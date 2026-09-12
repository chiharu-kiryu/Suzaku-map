//! Explicit local-only smoke check. Never reads a draft or permits cloud fallback.
use suzaku_map::{
    ime::settings::ImeSettings,
    languages::{
        model::{HttpModelProvider, ModelScope},
        translation::{TranslationLanguage, TranslationProvider, TranslationRequest},
    },
};

#[test]
#[ignore = "opt-in SUZAKU_TRANSLATION_LOCAL_QA=1; uses the configured local model with synthetic text"]
fn configured_local_model_translates_eight_synthetic_targets() {
    assert_eq!(
        std::env::var("SUZAKU_TRANSLATION_LOCAL_QA").as_deref(),
        Ok("1")
    );
    let config = ImeSettings::load().expect("model configuration").provider;
    assert_eq!(
        config.scope,
        ModelScope::Local,
        "live QA must never use a cloud model"
    );
    let provider = HttpModelProvider::new(config);
    for target in TranslationLanguage::ALL {
        let text = provider
            .translate(&TranslationRequest {
                text: "Hello, how are you? Thank you for your help.".into(),
                source: Some(TranslationLanguage::English),
                target,
            })
            .unwrap_or_else(|error| panic!("{}: {error}", target.name()));
        assert!(!text.trim().is_empty());
        println!("{}: {}", target.name(), text);
    }
}
