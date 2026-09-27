//! Authored fallback scenarios, not a model-quality or corpus-accuracy benchmark.
use std::sync::Arc;
use std::time::{Duration, Instant};
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};
use suzaku_map::languages::llm::{
    LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
};

const ENGLISH: &[(&str, &str, &str)] = &[
    (
        "could you speak more slo",
        "could you speak more slowly",
        "could you speak more slowly?",
    ),
    (
        "please resend the atta",
        "please resend the attachment",
        "please resend the attachment.",
    ),
    (
        "the network is temp",
        "the network is temporarily",
        "the network is temporarily unavailable.",
    ),
    (
        "i haven't received the conf",
        "i haven't received the confirmation",
        "i haven't received the confirmation yet.",
    ),
    (
        "where is the nearest rest",
        "where is the nearest restroom",
        "where is the nearest restroom?",
    ),
    (
        "please do not over",
        "please do not overwrite",
        "please do not overwrite this file.",
    ),
    (
        "i'll be there sho",
        "i'll be there shortly",
        "i'll be there shortly.",
    ),
    (
        "the file has not been sa",
        "the file has not been saved",
        "the file has not been saved.",
    ),
    (
        "could i have a rece",
        "could i have a receipt",
        "could i have a receipt?",
    ),
    (
        "could you confirm the loc",
        "could you confirm the location",
        "could you confirm the location?",
    ),
    (
        "sorry for the mis",
        "sorry for the misunderstanding",
        "sorry for the misunderstanding.",
    ),
    (
        "i'd like to collect my par",
        "i'd like to collect my parcel",
        "i'd like to collect my parcel.",
    ),
];

const CHINESE: &[(&str, &str, &str)] = &[
    ("qing'shao'deng", "请稍等", "请稍等，我查一下。"),
    ("mei'shou'dao", "没收到", "没收到，可以重新发一下吗？"),
    ("zai'shuo'yi'bian", "再说一遍", "再说一遍可以吗？"),
    ("bu'tai'que'ding", "不太确定", "不太确定，我再核对一下。"),
    ("li'xian'mo'shi", "离线模式", "离线模式下仍然可以输入。"),
    ("fu'wu'bu'ke'yong", "服务不可用", "服务不可用，请稍后再试。"),
    ("bao'cun'shi'bai", "保存失败", "保存失败，请先保留草稿。"),
    ("xi'shou'jian", "洗手间", "洗手间在哪里？"),
    ("chong'dian'qi", "充电器", "充电器可以借用一下吗？"),
    ("tui'kuan", "退款", "退款大概需要多久？"),
    ("bao'liu'yuan'wen", "保留原文", "保留原文，不要自动替换。"),
    ("ben'di'ci'ku", "本地词库", "本地词库提供基础词句候选。"),
];

const CONVERSATION_ENGLISH: &[(&str, &str, &str)] = &[
    (
        "would you mind checking the ava",
        "would you mind checking the availability",
        "would you mind checking the availability?",
    ),
    (
        "are you available tom",
        "are you available tomorrow",
        "are you available tomorrow afternoon?",
    ),
    (
        "the meeting has been resc",
        "the meeting has been rescheduled",
        "the meeting has been rescheduled for tomorrow.",
    ),
    (
        "please grant me acc",
        "please grant me access",
        "please grant me access to this document.",
    ),
    (
        "please preserve the forma",
        "please preserve the formatting",
        "please preserve the formatting.",
    ),
    (
        "i've attached the rev",
        "i've attached the revised",
        "i've attached the revised document.",
    ),
    (
        "my flight has been del",
        "my flight has been delayed",
        "my flight has been delayed.",
    ),
    (
        "i'd prefer an ais",
        "i'd prefer an aisle",
        "i'd prefer an aisle seat.",
    ),
    (
        "where can i collect my bag",
        "where can i collect my baggage",
        "where can i collect my baggage?",
    ),
    (
        "does this contain dai",
        "does this contain dairy",
        "does this contain dairy?",
    ),
    (
        "please put the leftovers in the ref",
        "please put the leftovers in the refrigerator",
        "please put the leftovers in the refrigerator.",
    ),
    (
        "let's make a shop",
        "let's make a shopping",
        "let's make a shopping list.",
    ),
];

const CONVERSATION_CHINESE: &[(&str, &str, &str)] = &[
    ("shi'jian'an'pai", "时间安排", "时间安排可以再调整一下。"),
    ("hui'yi'lian'jie", "会议链接", "会议链接已经发到群里了。"),
    (
        "fang'wen'quan'xian",
        "访问权限",
        "访问权限不足，麻烦帮我开通。",
    ),
    ("dai'shen'he", "待审核", "待审核的内容我已经提交了。"),
    ("cuo'wu'ti'shi", "错误提示", "错误提示能发个截图吗？"),
    (
        "wang'luo'bu'wen'ding",
        "网络不稳定",
        "网络不稳定，我重新连接一下。",
    ),
    ("sui'shen'xing'li", "随身行李", "随身行李有重量限制吗？"),
    ("huan'cheng'shi'jian", "换乘时间", "换乘时间够用吗？"),
    ("fa'ding'wei", "发定位", "发定位给我，我跟着导航走。"),
    ("bu'yao'xiang'cai", "不要香菜", "不要香菜，谢谢。"),
    ("gou'wu'qing'dan", "购物清单", "购物清单我已经列好了。"),
    ("shui'dian'fei", "水电费", "水电费这个月已经交了。"),
];

const ESSENTIALS_ENGLISH: &[(&str, &str, &str)] = &[
    (
        "would tomorrow mor",
        "would tomorrow morning",
        "would tomorrow morning work for you?",
    ),
    (
        "please confirm the exact da",
        "please confirm the exact date",
        "please confirm the exact date.",
    ),
    (
        "when does the subscription exp",
        "when does the subscription expire",
        "when does the subscription expire?",
    ),
    (
        "i was charged tw",
        "i was charged twice",
        "i was charged twice for this order.",
    ),
    (
        "the delivery address is inco",
        "the delivery address is incorrect",
        "the delivery address is incorrect.",
    ),
    (
        "the parcel arrived dam",
        "the parcel arrived damaged",
        "the parcel arrived damaged.",
    ),
    (
        "could you send me a rep",
        "could you send me a replacement",
        "could you send me a replacement?",
    ),
    (
        "please email me the inv",
        "please email me the invoice",
        "please email me the invoice.",
    ),
    (
        "is this still under warr",
        "is this still under warranty",
        "is this still under warranty?",
    ),
    (
        "i hope you have a rel",
        "i hope you have a relaxing",
        "i hope you have a relaxing evening.",
    ),
    (
        "i completely understand your con",
        "i completely understand your concern",
        "i completely understand your concern.",
    ),
    (
        "could you give me an exa",
        "could you give me an example",
        "could you give me an example?",
    ),
];

const ESSENTIALS_CHINESE: &[(&str, &str, &str)] = &[
    ("ming'tian'xia'wu", "明天下午", "明天下午可以安排。"),
    ("xia'zhou'yi", "下周一", "下周一我们再联系。"),
    ("ri'cheng'an'pai", "日程安排", "日程安排已经更新了。"),
    ("zi'dong'xu'fei", "自动续费", "自动续费可以关闭吗？"),
    ("chong'fu'kou'kuan", "重复扣款", "重复扣款了，请帮我核对。"),
    ("ding'dan'zhuang'tai", "订单状态", "订单状态一直没有更新。"),
    ("wu'liu'xin'xi", "物流信息", "物流信息暂时没有更新。"),
    ("dian'zi'fa'piao", "电子发票", "电子发票请发到我的邮箱。"),
    ("tui'kuan'shen'qing", "退款申请", "退款申请已经提交了。"),
    (
        "shang'pin'po'sun",
        "商品破损",
        "商品破损了，我把照片发给你。",
    ),
    ("xie'xie'guan'xin", "谢谢关心", "谢谢关心，我这边一切都好。"),
    ("lu'shang'xiao'xin", "路上小心", "路上小心，不用着急。"),
];

fn engine(language: &str) -> XRTabletImeEngine {
    let mut engine = XRTabletImeEngine::new(EngineConfig {
        default_language: language.into(),
        ..Default::default()
    });
    engine.enable_ibus_candidate_mix();
    engine
}

fn assert_fallback(ime: &XRTabletImeEngine, seed: &str, word: &str, sentence: &str) {
    assert!(ime.candidates().len() <= 12);
    assert!(ime.candidates().iter().any(|c| c.text == seed));
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local)
    );
    for (text, kind) in [
        (word, CandidateKind::Word),
        (sentence, CandidateKind::Sentence),
    ] {
        assert!(
            ime.candidates()
                .iter()
                .take(PAGE_SIZE)
                .any(|c| c.text == text && c.kind == kind),
            "missing {text:?} ({kind:?}) for {seed:?}: {:?}",
            ime.candidates()
        );
    }
}

fn confirm(ime: &mut XRTabletImeEngine, text: &str) {
    let index = ime
        .candidates()
        .iter()
        .position(|c| c.text == text)
        .unwrap();
    ime.select_candidate(index);
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
}

#[test]
fn offline_english_fallback_keeps_words_sentences_and_exact_commits() {
    for &(seed, word, sentence) in ENGLISH
        .iter()
        .chain(CONVERSATION_ENGLISH)
        .chain(ESSENTIALS_ENGLISH)
    {
        let mut ime = engine("en");
        ime.seed(seed);
        assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
        assert_fallback(&ime, seed, word, sentence);
        assert_eq!(ime.candidates()[0].text, seed);
        confirm(&mut ime, sentence);
    }
}

#[test]
fn offline_chinese_fallback_preserves_spelling_forms_and_exact_commits() {
    for &(reading, word, sentence) in CHINESE
        .iter()
        .chain(CONVERSATION_CHINESE)
        .chain(ESSENTIALS_CHINESE)
    {
        for seed in [
            reading.replace('\'', ""),
            reading.replace('\'', " "),
            reading.into(),
            reading.to_uppercase(),
        ] {
            let mut ime = engine("zh-Hans");
            ime.seed(&seed);
            assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
            assert_fallback(&ime, &seed, word, sentence);
            confirm(&mut ime, sentence);
        }
    }
}

struct UnavailableProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for UnavailableProvider {
    fn provider_id(&self) -> &str {
        "synthetic-fallback-error"
    }
    fn generate(&self, _: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        Vec::new()
    }
    fn generate_checked(
        &self,
        _: &LlmCompletionRequest,
    ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
        self.0.clone().map_or_else(|| Ok(Vec::new()), Err)
    }
}

#[test]
fn missing_failed_timed_out_and_empty_models_preserve_the_immediate_local_fallback() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (language, cases, added, latest) in [
            ("en", ENGLISH, CONVERSATION_ENGLISH, ESSENTIALS_ENGLISH),
            ("zh-Hans", CHINESE, CONVERSATION_CHINESE, ESSENTIALS_CHINESE),
        ] {
            for &(seed, word, sentence) in cases
                .iter()
                .take(2)
                .chain(added.iter().take(1))
                .chain(latest.iter().take(1))
            {
                let mut ime = engine(language);
                ime.configure_prediction(Some(Arc::new(UnavailableProvider(error.clone()))));
                ime.seed(seed);
                assert_fallback(&ime, seed, word, sentence);
                let local = ime.candidates().to_vec();
                let deadline = Instant::now() + Duration::from_secs(2);
                while ime.prediction_pending() && Instant::now() < deadline {
                    ime.poll_prediction();
                    std::thread::sleep(Duration::from_millis(2));
                }
                assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
                assert_eq!(
                    ime.prediction_error(),
                    Some(&error.clone().unwrap_or(LlmProviderError::NoCandidates))
                );
                assert_eq!(ime.candidates(), local, "{language} {error:?}");
                ime.configure_prediction(None);
                assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
                assert!(ime.prediction_error().is_none());
                assert_fallback(&ime, seed, word, sentence);
                confirm(&mut ime, sentence);
            }
        }
    }
}

#[test]
fn long_fallback_drafts_keep_the_prefix_and_do_not_require_a_model() {
    for (language, prefix, cases, added, latest) in [
        (
            "en",
            "note ".repeat(60),
            ENGLISH,
            CONVERSATION_ENGLISH,
            ESSENTIALS_ENGLISH,
        ),
        (
            "zh-Hans",
            "你".repeat(260),
            CHINESE,
            CONVERSATION_CHINESE,
            ESSENTIALS_CHINESE,
        ),
    ] {
        for &(seed, word, sentence) in cases
            .iter()
            .take(2)
            .chain(added.iter().take(2))
            .chain(latest.iter().take(2))
        {
            let mut ime = engine(language);
            let seed = format!("{prefix}{seed}");
            let word = format!("{prefix}{word}");
            let sentence = format!("{prefix}{sentence}");
            assert!(seed.chars().count() > 256);
            ime.seed(&seed);
            assert_fallback(&ime, &seed, &word, &sentence);
            confirm(&mut ime, &sentence);
        }
    }
}

#[test]
fn standalone_fallback_word_forms_are_visible_without_sentence_context() {
    for (seed, word) in [
        ("apologet", "apologetic"),
        ("hesit", "hesitate"),
        ("confus", "confused"),
        ("contributor", "contributors"),
        ("reope", "reopened"),
        ("synci", "syncing"),
        ("rerout", "rerouted"),
        ("transferr", "transferred"),
        ("refrigerat", "refrigerated"),
        ("chopst", "chopsticks"),
        ("misspel", "misspelled"),
        ("formatti", "formatting"),
        ("edita", "editable"),
        ("piny", "pinyin"),
        ("weekl", "weekly"),
        ("monthl", "monthly"),
        ("anniversar", "anniversaries"),
        ("believ", "believing"),
        ("realize", "realized"),
        ("suppos", "supposed"),
        ("meani", "meaning"),
        ("wante", "wanted"),
        ("offer", "offered"),
        ("shari", "sharing"),
        ("contactl", "contactless"),
        ("redeem", "redeemed"),
        ("eligib", "eligible"),
        ("withdra", "withdrawn"),
        ("detachab", "detachable"),
        ("stripe", "striped"),
    ] {
        let mut ime = engine("en");
        ime.seed(seed);
        assert_eq!(ime.candidates()[0].text, seed);
        assert!(
            ime.candidates().iter().take(PAGE_SIZE).any(|c| {
                c.text == word
                    && c.kind == CandidateKind::Word
                    && c.source == CandidateSource::Local
            }),
            "{seed} -> {word}: {:?}",
            ime.candidates()
        );
        confirm(&mut ime, word);
    }
}

#[test]
fn conversation_fallback_retains_literal_spacing_case_and_apostrophes() {
    for (seed, word, sentence) in [
        (
            "  Please  grant  me  acc",
            "  Please  grant  me  access",
            "  Please  grant  me  access to this document.",
        ),
        (
            "I've attached the rev",
            "I've attached the revised",
            "I've attached the revised document.",
        ),
        (
            "I’ve attached the rev",
            "I’ve attached the revised",
            "I’ve attached the revised document.",
        ),
        (
            "I’d prefer an ais",
            "I’d prefer an aisle",
            "I’d prefer an aisle seat.",
        ),
        (
            "  Please  email  me  the  inv",
            "  Please  email  me  the  invoice",
            "  Please  email  me  the  invoice.",
        ),
        (
            "I’d like to change the del",
            "I’d like to change the delivery",
            "I’d like to change the delivery address.",
        ),
    ] {
        let mut ime = engine("en");
        ime.seed(seed);
        assert_fallback(&ime, seed, word, sentence);
        confirm(&mut ime, sentence);
    }
}

#[test]
fn everyday_chinese_unfinished_syllables_offer_words_and_sentences() {
    for (seed, word, sentence) in [
        ("ming tian xia w", "明天下午", "明天下午可以安排。"),
        ("dian zi fa p", "电子发票", "电子发票请发到我的邮箱。"),
        ("chong fu kou k", "重复扣款", "重复扣款了，请帮我核对。"),
        ("zi dong xu f", "自动续费", "自动续费可以关闭吗？"),
    ] {
        let mut ime = engine("zh-Hans");
        ime.seed(seed);
        assert_fallback(&ime, seed, word, sentence);
        confirm(&mut ime, sentence);
    }
}
