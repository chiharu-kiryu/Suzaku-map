//! Curated vocabulary checks, not a general language benchmark. No provider or
//! personal history is involved; every accepted result remains a full draft.
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE, merge_model};
use suzaku_map::ime::{CommitOptions, EngineConfig, XRTabletImeEngine};
use suzaku_map::languages::llm::LlmCompletion;

fn engine(language: &str, seed: &str) -> XRTabletImeEngine {
    let mut engine = XRTabletImeEngine::new(EngineConfig {
        default_language: language.into(),
        ..Default::default()
    });
    engine.enable_ibus_candidate_mix();
    engine.seed(seed);
    engine
}

fn assert_on_first_page(engine: &XRTabletImeEngine, expected: &str, kind: CandidateKind) {
    assert!(
        engine
            .candidates()
            .iter()
            .take(PAGE_SIZE)
            .any(|c| c.text == expected && c.kind == kind),
        "missing {expected:?} ({kind:?}): {:?}",
        engine.candidates()
    );
}

fn select_and_commit(engine: &mut XRTabletImeEngine, expected: &str) {
    let index = engine
        .candidates()
        .iter()
        .position(|c| c.text == expected)
        .unwrap();
    engine.select_candidate(index);
    assert!(engine.snapshot().committed_text.is_empty());
    assert_eq!(
        engine.commit(CommitOptions { force: true }).text.as_deref(),
        Some(expected)
    );
}

#[test]
fn everyday_english_vocabulary_includes_common_nouns_and_explicit_word_forms() {
    for (seed, expected) in [
        ("breakf", "breakfast"),
        ("groc", "groceries"),
        ("restau", "restaurant"),
        ("receip", "receipt"),
        ("reservat", "reservation"),
        ("subscr", "subscription"),
        ("arrivi", "arriving"),
        ("brou", "brought"),
        ("bough", "bought"),
        ("choos", "choose"),
        ("choos", "choosing"),
        ("scheduli", "scheduling"),
        ("debuggi", "debugging"),
        ("deplo", "deployment"),
        ("autocomp", "autocomplete"),
        ("authenti", "authentication"),
        ("concurr", "concurrency"),
        ("serializ", "serialization"),
        ("regress", "regression"),
        ("transact", "transaction"),
        ("backp", "backpack"),
        ("pharma", "pharmacy"),
        ("kitch", "kitchen"),
        ("lugg", "luggage"),
    ] {
        let engine = engine("en", seed);
        assert_eq!(engine.candidates()[0].text, seed);
        assert_on_first_page(&engine, expected, CandidateKind::Word);
        assert!(
            engine
                .candidates()
                .iter()
                .all(|c| c.text.starts_with(seed) && c.source == CandidateSource::Local)
        );
    }
}

#[test]
fn new_english_collocations_offer_both_next_words_and_complete_sentences() {
    for (seed, word, sentence) in [
        (
            "could we res",
            "could we reschedule",
            "could we reschedule the meeting?",
        ),
        (
            "please attach the inv",
            "please attach the invoice",
            "please attach the invoice.",
        ),
        (
            "I'll bring the groc",
            "I'll bring the groceries",
            "I'll bring the groceries home.",
        ),
        (
            "I’ll bring the groc",
            "I’ll bring the groceries",
            "I’ll bring the groceries home.",
        ),
        (
            "please restart the ser",
            "please restart the server",
            "please restart the server.",
        ),
        (
            "  Please  restart  the  ser",
            "  Please  restart  the  server",
            "  Please  restart  the  server.",
        ),
        (
            "please restart ",
            "please restart the",
            "please restart the server.",
        ),
        (
            "please restart the ",
            "please restart the server",
            "please restart the server.",
        ),
        (
            "the deployment is r",
            "the deployment is ready",
            "the deployment is ready.",
        ),
        (
            "let's grab some lu",
            "let's grab some lunch",
            "let's grab some lunch.",
        ),
        (
            "I need a rece",
            "I need a receipt",
            "I need a receipt, please.",
        ),
        (
            "please run the reg",
            "please run the regression",
            "please run the regression tests.",
        ),
    ] {
        let mut engine = engine("en", seed);
        assert_on_first_page(&engine, word, CandidateKind::Word);
        assert_on_first_page(&engine, sentence, CandidateKind::Sentence);
        assert!(engine.candidates().iter().all(|c| c.text.starts_with(seed)));
        select_and_commit(&mut engine, sentence);
    }
}

#[test]
fn expanded_chinese_words_match_joined_separated_and_tone_readings() {
    for (reading, expected) in [
        ("zao'fan", "早饭"),
        ("wan'fan", "晚饭"),
        ("can'ting", "餐厅"),
        ("di'tie", "地铁"),
        ("gong'jiao", "公交"),
        ("ji'chang", "机场"),
        ("jiu'dian", "酒店"),
        ("yu'yue", "预约"),
        ("ding'dan", "订单"),
        ("kuai'di", "快递"),
        ("di'zhi", "地址"),
        ("zhou'mo", "周末"),
        ("hui'yi", "会议"),
        ("kai'hui", "开会"),
        ("tong'shi", "同事"),
        ("an'pai", "安排"),
        ("que'ren", "确认"),
        ("wan'cheng", "完成"),
        ("bao'cun", "保存"),
        ("wen'jian", "文件"),
        ("jian'pan", "键盘"),
        ("shu'biao", "鼠标"),
        ("ping'mu", "屏幕"),
        ("chuang'kou", "窗口"),
        ("shu'ju'ku", "数据库"),
        ("fu'wu'qi", "服务器"),
        ("huan'cun", "缓存"),
        ("diao'shi", "调试"),
        ("bu'shu", "部署"),
        ("yi'lai", "依赖"),
        ("jian'rong", "兼容"),
        ("zi'dong'bu'quan", "自动补全"),
        ("hui'gui'ce'shi", "回归测试"),
        ("ren'gong'zhi'neng", "人工智能"),
        ("ben'di'mo'xing", "本地模型"),
        ("yun'duan", "云端"),
        ("ti'jiao", "提交"),
        ("tui'song", "推送"),
        ("chong'xin'qi'dong", "重新启动"),
        ("chong'zhi", "重置"),
        ("ci'ku", "词库"),
    ] {
        // Tone values are ignored by the bootstrap decoder. These synthetic
        // digits test token boundaries, not the pronunciation of each word.
        for seed in [
            reading.replace('\'', ""),
            reading.replace('\'', " "),
            reading.to_uppercase(),
            reading
                .split('\'')
                .map(|syllable| format!("{syllable}4"))
                .collect::<String>(),
        ] {
            let engine = engine("zh-Hans", &seed);
            assert_eq!(engine.candidates()[0].text, expected, "{seed}");
            assert!(engine.candidates().iter().any(|c| c.text == seed));
            assert!(
                engine
                    .candidates()
                    .iter()
                    .all(|c| c.source == CandidateSource::Local)
            );
        }
    }
}

#[test]
fn chinese_homophones_are_choices_not_forced_replacements() {
    for (seed, primary, alternative) in [
        ("shijian", "时间", "事件"),
        ("dizhi", "地址", "地质"),
        ("tongshi", "同事", "同时"),
        ("zhanghao", "账号", "帐号"),
        ("shishi", "事实", "实施"),
    ] {
        let mut engine = engine("zh-Hans", seed);
        assert_eq!(engine.candidates()[0].text, primary, "{seed}");
        assert_on_first_page(&engine, alternative, CandidateKind::Word);
        select_and_commit(&mut engine, alternative);
    }
}

#[test]
fn new_chinese_words_have_sentences_before_and_after_an_adopted_space() {
    for (reading, word, sentence) in [
        ("zhou mo", "周末", "周末有什么安排？"),
        ("hui yi", "会议", "会议什么时候开始？"),
        ("wen jian", "文件", "文件已经保存了。"),
        ("bao cun", "保存", "保存一下当前的修改。"),
        ("fu wu qi", "服务器", "服务器已经启动了。"),
        ("shu ju ku", "数据库", "数据库连接正常。"),
        ("bu shu", "部署", "部署已经完成了。"),
        ("ci ku", "词库", "词库还需要继续扩充。"),
    ] {
        let mut engine = engine("zh-Hans", reading);
        assert_on_first_page(&engine, word, CandidateKind::Word);
        assert_on_first_page(&engine, sentence, CandidateKind::Sentence);
        let adopted = format!("{word} ");
        engine.seed(&adopted);
        let spaced_sentence = format!("{adopted}{}", &sentence[word.len()..]);
        assert_eq!(engine.candidates()[0].text, adopted);
        assert!(engine.snapshot().committed_text.is_empty());
        assert_on_first_page(&engine, &spaced_sentence, CandidateKind::Sentence);
        assert!(
            engine
                .candidates()
                .iter()
                .all(|c| c.text.starts_with(&adopted))
        );
    }
}

#[test]
fn expanded_chinese_branches_do_not_crowd_out_authored_sentences() {
    for (seed, primary, first_sentence, second_sentence) in [
        (
            "nihao",
            "你好",
            "你好，很高兴认识你。",
            "你好，请问有什么可以帮忙？",
        ),
        (
            "ni hao",
            "你好",
            "你好，很高兴认识你。",
            "你好，请问有什么可以帮忙？",
        ),
        (
            "ni3hao3",
            "你好",
            "你好，很高兴认识你。",
            "你好，请问有什么可以帮忙？",
        ),
        (
            "niha",
            "你好",
            "你好，很高兴认识你。",
            "你好，请问有什么可以帮忙？",
        ),
        (
            "你好 ",
            "你好 ",
            "你好 ，很高兴认识你。",
            "你好 ，请问有什么可以帮忙？",
        ),
    ] {
        let engine = engine("zh-Hans", seed);
        assert_eq!(engine.candidates()[0].text, primary);
        assert!(engine.candidates().iter().any(|c| c.text == seed));
        assert_on_first_page(&engine, first_sentence, CandidateKind::Sentence);
        assert_on_first_page(&engine, second_sentence, CandidateKind::Sentence);
    }
}

#[test]
fn expanded_chinese_vocabulary_leaves_room_for_valid_model_sentences() {
    for (seed, primary) in [("nihao", "你好"), ("  nihao", "  你好"), ("你好 ", "你好 ")] {
        let engine = engine("zh-Hans", seed);
        let model_sentence = format!("{primary}新的朋友。");
        let authored_sentence = format!("{primary}，很高兴认识你。");
        let (merged, accepted) = merge_model(
            "zh-Hans",
            seed,
            engine.candidates().to_vec(),
            vec![LlmCompletion {
                text: model_sentence.clone(),
                kind: Some(CandidateKind::Sentence),
                score_bias: 0.0,
            }],
            12,
        );
        assert!(accepted);
        assert_eq!(merged[0].text, primary);
        assert!(merged.iter().any(|c| c.text == seed));
        for (text, source) in [
            (&model_sentence, CandidateSource::Model),
            (&authored_sentence, CandidateSource::Local),
        ] {
            assert!(
                merged.iter().take(PAGE_SIZE).any(|c| {
                    &c.text == text && c.kind == CandidateKind::Sentence && c.source == source
                }),
                "{seed:?} -> {text:?}: {merged:?}"
            );
        }
    }
}

#[test]
fn new_vocabulary_completes_long_draft_tails_without_replacing_the_prefix() {
    for (language, prefix, tail, expected) in [
        (
            "en",
            "This part is already written. ".repeat(24),
            "please run the reg",
            "please run the regression tests.",
        ),
        (
            "zh-Hans",
            "这段已经写好的内容应该完整保留。".repeat(24),
            "shu ju k",
            "数据库",
        ),
    ] {
        let seed = format!("{prefix}{tail}");
        let expected = format!("{prefix}{expected}");
        let mut engine = engine(language, &seed);
        assert!(seed.chars().count() > 256);
        assert!(engine.candidates().iter().any(|c| c.text == seed));
        assert!(
            engine
                .candidates()
                .iter()
                .all(|c| c.text.starts_with(&prefix))
        );
        select_and_commit(&mut engine, &expected);
    }
}
