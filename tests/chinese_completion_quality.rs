//! Project-authored Pinyin acceptance cases, not general Chinese accuracy scores.
//! No model, network service or personal input history is required.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, XRTabletImeEngine};

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut engine = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    engine.enable_ibus_candidate_mix();
    engine.seed(seed);
    engine
}

#[test]
fn known_chinese_words_do_not_disappear_when_syllables_are_separated() {
    let cases = [
        ("nihao", "ni hao", "你好"),
        ("xiexie", "xie xie", "谢谢"),
        ("zaijian", "zai jian", "再见"),
        ("zaoshanghao", "zao shang hao", "早上好"),
        ("wanshanghao", "wan shang hao", "晚上好"),
        ("meiguanxi", "mei guan xi", "没关系"),
        ("duibuqi", "dui bu qi", "对不起"),
        ("qingwen", "qing wen", "请问"),
        ("zhongwen", "zhong wen", "中文"),
        ("yingwen", "ying wen", "英文"),
        ("shurufa", "shu ru fa", "输入法"),
        ("houxuan", "hou xuan", "候选"),
        ("lianxiang", "lian xiang", "联想"),
        ("moxing", "mo xing", "模型"),
        ("yuyan", "yu yan", "语言"),
        ("zhichi", "zhi chi", "支持"),
        ("gongneng", "gong neng", "功能"),
        ("jixu", "ji xu", "继续"),
        ("kaifa", "kai fa", "开发"),
        ("ceshi", "ce shi", "测试"),
        ("shezhi", "she zhi", "设置"),
        ("jintian", "jin tian", "今天"),
        ("mingtian", "ming tian", "明天"),
        ("zuotian", "zuo tian", "昨天"),
        ("xianzai", "xian zai", "现在"),
        ("gongzuo", "gong zuo", "工作"),
        ("xuexi", "xue xi", "学习"),
        ("xihuan", "xi huan", "喜欢"),
        ("pengyou", "peng you", "朋友"),
        ("shijian", "shi jian", "时间"),
        ("wenti", "wen ti", "问题"),
        ("bangzhu", "bang zhu", "帮助"),
        ("diannao", "dian nao", "电脑"),
        ("shouji", "shou ji", "手机"),
        ("beijing", "bei jing", "北京"),
        ("shanghai", "shang hai", "上海"),
        ("shenzhen", "shen zhen", "深圳"),
        ("guangzhou", "guang zhou", "广州"),
        ("woxihuanbeijing", "wo xi huan bei jing", "我喜欢北京"),
        (
            "woxiangxuexizhongwen",
            "wo xiang xue xi zhong wen",
            "我想学习中文",
        ),
    ];
    let mut hits = 0;
    let mut misses = Vec::new();
    for (joined, separated, expected) in cases {
        for seed in [
            joined.to_owned(),
            separated.to_owned(),
            separated.replace(' ', "'"),
            separated.to_ascii_uppercase(),
        ] {
            let engine = engine(&seed);
            let candidates = engine.candidates();
            let hit = candidates
                .first()
                .is_some_and(|candidate| candidate.text == expected);
            hits += usize::from(hit);
            if !hit {
                misses.push(format!("{seed:?} -> {candidates:?}"));
            }
            assert!(candidates.iter().any(|candidate| candidate.text == seed));
            assert!(
                candidates
                    .iter()
                    .all(|candidate| candidate.source == CandidateSource::Local)
            );
            assert_eq!(
                candidates.len(),
                candidates
                    .iter()
                    .map(|c| &c.text)
                    .collect::<HashSet<_>>()
                    .len()
            );
        }
    }
    println!(
        "Chinese curated spellings: primary conversion {hits}/{}",
        cases.len() * 4
    );
    assert!(misses.is_empty(), "{}", misses.join("\n"));
}

#[test]
fn separated_pinyin_keeps_words_and_sentences_on_the_first_page() {
    for (seed, word, sentence) in [
        ("shu ru fa", "输入法", "输入法支持多种语言。"),
        ("ji xu", "继续", "继续完善这个功能。"),
        ("xue xi", "学习", "学习一门新的语言。"),
        ("qing wen", "请问", "请问现在方便吗？"),
        ("wo xi huan bei j", "我喜欢北京", "我喜欢北京的文化。"),
        (
            "我想学习zhong w",
            "我想学习中文",
            "我想学习中文，请多指教。",
        ),
    ] {
        let engine = engine(seed);
        let first_page: Vec<_> = engine.candidates().iter().take(PAGE_SIZE).collect();
        assert!(
            first_page
                .iter()
                .any(|c| c.text == word && c.kind == CandidateKind::Word),
            "{seed}: {first_page:?}"
        );
        assert!(
            first_page
                .iter()
                .any(|c| c.text == sentence && c.kind == CandidateKind::Sentence),
            "{seed}: {first_page:?}"
        );
    }
}

#[test]
fn explicit_syllable_boundaries_are_not_erased_to_force_a_match() {
    for (seed, primary) in [
        ("xi an", "西安"),
        ("xi'an", "西安"),
        ("xian", "先"),
        ("xi'an shi", "西安是"),
    ] {
        assert_eq!(engine(seed).candidates()[0].text, primary);
    }
    for (seed, forbidden) in [
        ("z hongwen", "中文"),
        ("zh ongwen", "中文"),
        ("s hurufa", "输入法"),
        ("bei j ing", "北京"),
    ] {
        assert!(
            !engine(seed)
                .candidates()
                .iter()
                .any(|c| c.text == forbidden),
            "{seed}"
        );
    }
    for seed in [
        "Rust2026",
        "unknownbei",
        "https://bei",
        "user_name",
        "src/shuru.rs",
    ] {
        assert_eq!(
            engine(seed)
                .candidates()
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>(),
            [seed]
        );
    }
}

#[test]
fn selected_chinese_completion_remains_editable_until_explicit_commit() {
    let mut engine = engine("wo xi huan bei j");
    let index = engine
        .candidates()
        .iter()
        .position(|c| c.text == "我喜欢北京")
        .unwrap();
    engine.select_candidate(index);
    assert!(engine.snapshot().committed_text.is_empty());
    // Continue from the visible choice, as the host's adoption operation does.
    engine.seed("我喜欢北京de");
    let index = engine
        .candidates()
        .iter()
        .position(|c| c.text == "我喜欢北京的")
        .unwrap();
    engine.select_candidate(index);
    assert_eq!(
        engine.commit(CommitOptions { force: true }).text.as_deref(),
        Some("我喜欢北京的")
    );
    assert!(engine.candidates().is_empty());
}
