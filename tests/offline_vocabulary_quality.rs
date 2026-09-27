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

const WRITING_ENGLISH: &[(&str, &str, &str, &str)] = &[
    (
        "please acknowledge",
        "rece",
        "receipt",
        "please acknowledge receipt of this message.",
    ),
    (
        "could we postpone the",
        "mee",
        "meeting",
        "could we postpone the meeting until tomorrow?",
    ),
    (
        "please keep me",
        "po",
        "posted",
        "please keep me posted on your progress.",
    ),
    (
        "the deadline has been",
        "ext",
        "extended",
        "the deadline has been extended until Friday.",
    ),
    (
        "could you walk me",
        "thr",
        "through",
        "could you walk me through the process?",
    ),
    (
        "please check the tracking",
        "num",
        "number",
        "please check the tracking number.",
    ),
    (
        "we have reached a",
        "cons",
        "consensus",
        "we have reached a consensus on the proposal.",
    ),
    (
        "i'm waiting for",
        "conf",
        "confirmation",
        "i'm waiting for confirmation from the hotel.",
    ),
    ("please take your", "ti", "time", "please take your time."),
    (
        "could you pick up the",
        "par",
        "parcel",
        "could you pick up the parcel?",
    ),
    (
        "i'll be available after",
        "lu",
        "lunch",
        "i'll be available after lunch.",
    ),
    (
        "please review the handover",
        "check",
        "checklist",
        "please review the handover checklist.",
    ),
];

const WRITING_CHINESE: &[(&str, &str, &str)] = &[
    ("gai'qi", "改期", "改期后的时间我再确认一下。"),
    ("jiao'jie", "交接", "交接材料已经准备好了。"),
    ("cha'shou", "查收", "查收后请回复确认。"),
    ("qian'shou", "签收", "签收前请检查包裹。"),
    ("xing'li", "行李", "行李可以寄存在哪里？"),
    ("jin'zhan", "进展", "进展我会及时同步。"),
    ("bao'xiao", "报销", "报销需要提供发票。"),
    ("yu'ji", "预计", "预计明天下午可以完成。"),
    ("ti'qian", "提前", "提前十分钟到达就可以。"),
    ("bei'wang'lu", "备忘录", "备忘录已经更新了。"),
    ("dai'ban", "待办", "待办事项已经整理好了。"),
    ("lian'xi'fang'shi", "联系方式", "联系方式已经发给你了。"),
    ("shou'jian'ren", "收件人", "收件人的电话需要确认一下。"),
    ("yuan'yin'fen'xi", "原因分析", "原因分析已经完成了。"),
    ("hui'yi'ji'yao", "会议纪要", "会议纪要已经发到群里了。"),
    ("huan'cheng", "换乘", "换乘需要预留一些时间。"),
];

#[test]
fn writing_english_words_include_real_inflections_and_regional_spellings() {
    for (seed, word) in [
        ("acknowledgm", "acknowledgment"),
        ("acknowledgem", "acknowledgement"),
        ("reimbursem", "reimbursement"),
        ("postponem", "postponement"),
        ("clarificat", "clarification"),
        ("resubmitt", "resubmitted"),
        ("commuti", "commuting"),
        ("itinerar", "itineraries"),
        ("handov", "handover"),
        ("actionab", "actionable"),
        ("overd", "overdue"),
        ("unavailab", "unavailability"),
        ("consens", "consensus"),
        ("paraphras", "paraphrase"),
        ("bibliograph", "bibliography"),
        ("refurbish", "refurbished"),
        ("receip", "receipts"),
        ("detache", "detached"),
        ("legibil", "legibility"),
        ("infeas", "infeasible"),
    ] {
        let engine = engine("en", seed);
        assert_eq!(engine.candidates()[0].text, seed);
        assert_on_first_page(&engine, word, CandidateKind::Word);
        assert!(engine.candidates().iter().all(|c| c.text.starts_with(seed)));
    }
}

#[test]
fn writing_english_contexts_keep_word_and_sentence_choices_while_typing() {
    for &(context, partial, word, sentence) in WRITING_ENGLISH {
        for seed in [
            format!("{context} {partial}"),
            context.into(),
            format!("{context} "),
        ] {
            let mut engine = engine("en", &seed);
            assert_on_first_page(&engine, &format!("{context} {word}"), CandidateKind::Word);
            assert_on_first_page(&engine, sentence, CandidateKind::Sentence);
            assert!(
                engine
                    .candidates()
                    .iter()
                    .all(|c| c.text.starts_with(&seed))
            );
            select_and_commit(&mut engine, sentence);
        }
    }
}

#[test]
fn writing_chinese_words_have_first_page_sentences_in_all_reading_forms() {
    for &(reading, word, sentence) in WRITING_CHINESE {
        // Synthetic tone digits exercise syllable boundaries, not pronunciation.
        for seed in [
            reading.into(),
            reading.replace('\'', ""),
            reading.replace('\'', " "),
            reading.to_uppercase(),
            reading
                .split('\'')
                .map(|s| format!("{s}4"))
                .collect::<String>(),
        ] {
            let mut engine = engine("zh-Hans", &seed);
            assert_eq!(engine.candidates()[0].text, word, "{seed}");
            assert_on_first_page(&engine, word, CandidateKind::Word);
            assert_on_first_page(&engine, sentence, CandidateKind::Sentence);
            assert!(engine.candidates().iter().any(|c| c.text == seed));
            assert!(
                engine
                    .candidates()
                    .iter()
                    .all(|c| c.source == CandidateSource::Local)
            );
            select_and_commit(&mut engine, sentence);
        }
    }
}

#[test]
fn writing_chinese_homophones_do_not_replace_established_rankings() {
    for (seed, primary, other) in [
        ("beijing", "北京", "背景"),
        ("jinzhan", "进展", "进站"),
        ("gongshi", "共识", "公式"),
        ("xingcheng", "行程", "形成"),
        ("baoguan", "保管", "报关"),
        ("heshi", "核实", "合适"),
    ] {
        let mut engine = engine("zh-Hans", seed);
        assert_eq!(engine.candidates()[0].text, primary, "{seed}");
        assert_on_first_page(&engine, other, CandidateKind::Word);
        select_and_commit(&mut engine, other);
    }
}

#[test]
fn writing_adopted_words_continue_losslessly_in_short_and_long_drafts() {
    for &(reading, word, sentence) in WRITING_CHINESE {
        for prefix in [String::new(), "这段已写好的内容需要保留。".repeat(24)] {
            for padding in ["", " ", "  ", "\u{3000}"] {
                let seed = format!("{prefix}{word}{padding}");
                let expected = format!("{seed}{}", &sentence[word.len()..]);
                let mut engine = engine("zh-Hans", &seed);
                assert!(engine.candidates().iter().any(|c| c.text == seed));
                assert_on_first_page(&engine, &expected, CandidateKind::Sentence);
                assert!(
                    engine
                        .candidates()
                        .iter()
                        .all(|c| c.text.starts_with(&seed))
                );
                select_and_commit(&mut engine, &expected);
            }
            let seed = format!("{prefix}{}", reading.replace('\'', " "));
            let mut engine = engine("zh-Hans", &seed);
            select_and_commit(&mut engine, &format!("{prefix}{word}"));
        }
    }
    for &(context, partial, _, sentence) in WRITING_ENGLISH {
        let prefix = "This part must stay exactly as written. ".repeat(16);
        let mut engine = engine("en", &format!("{prefix}{context} {partial}"));
        assert!(
            engine
                .candidates()
                .iter()
                .all(|c| c.text.starts_with(&prefix))
        );
        select_and_commit(&mut engine, &format!("{prefix}{sentence}"));
    }
}

#[test]
fn writing_additions_leave_identifiers_and_unknown_contexts_literal() {
    for (language, seed) in [
        ("en", "https://example.org/acknowledg"),
        ("en", "draft_reimbursem"),
        ("en", "user@handover.example"),
        ("en", "unknowncontext "),
        ("zh-Hans", "https://example.org/gaiqi"),
        ("zh-Hans", "notes/jiaojie.txt"),
        ("zh-Hans", "user@chashou.example"),
    ] {
        let engine = engine(language, seed);
        assert_eq!(
            engine.candidates().len(),
            1,
            "{seed}: {:?}",
            engine.candidates()
        );
        assert_eq!(engine.candidates()[0].text, seed);
        assert_eq!(engine.candidates()[0].source, CandidateSource::Local);
        assert!(engine.snapshot().committed_text.is_empty());
    }
}

const DAILY_ENGLISH: &[(&str, &str, &str, &str)] = &[
    (
        "can we catch up",
        "tom",
        "tomorrow",
        "can we catch up tomorrow?",
    ),
    ("let's go for a", "wa", "walk", "let's go for a walk."),
    (
        "i might be a little",
        "la",
        "late",
        "i might be a little late.",
    ),
    (
        "i'll get back to you",
        "so",
        "soon",
        "i'll get back to you soon.",
    ),
    (
        "i should've",
        "call",
        "called",
        "i should've called earlier.",
    ),
    (
        "please add this to my",
        "wish",
        "wishlist",
        "please add this to my wishlist.",
    ),
    (
        "is this available in a",
        "larg",
        "larger",
        "is this available in a larger size?",
    ),
    (
        "i'd like to exchange this for a different",
        "si",
        "size",
        "i'd like to exchange this for a different size.",
    ),
    (
        "could you turn down the",
        "vol",
        "volume",
        "could you turn down the volume?",
    ),
    (
        "the battery needs",
        "rech",
        "recharging",
        "the battery needs recharging.",
    ),
    (
        "my headphones are not",
        "pair",
        "pairing",
        "my headphones are not pairing.",
    ),
    (
        "could you help me fold the",
        "lau",
        "laundry",
        "could you help me fold the laundry?",
    ),
];

const DAILY_CHINESE: &[(&str, &str, &str)] = &[
    ("zao'an", "早安", "早安，今天也要加油。"),
    ("wan'an", "晚安", "晚安，早点休息。"),
    ("ming'zao", "明早", "明早我再联系你。"),
    ("yi'hui'er", "一会儿", "一会儿见。"),
    ("ju'hui", "聚会", "聚会几点开始？"),
    ("shun'lu", "顺路", "顺路的话帮我带一下。"),
    ("nao'zhong", "闹钟", "闹钟已经设好了。"),
    ("gou'wu'che", "购物车", "购物车里的商品需要再确认一下。"),
    ("you'hui'quan", "优惠券", "优惠券可以一起使用吗？"),
    ("tui'huo", "退货", "退货需要保留包装吗？"),
    ("chi'ma", "尺码", "尺码不合适可以换吗？"),
    ("chong'dian'bao", "充电宝", "充电宝需要提前充电。"),
    ("lan'ya", "蓝牙", "蓝牙已经连接好了。"),
    ("chu'mo'ban", "触摸板", "触摸板的灵敏度可以调整。"),
    ("jing'yin", "静音", "静音已经取消了。"),
    ("xi'yi'fu", "洗衣服", "洗衣服之前先检查口袋。"),
];

#[test]
fn daily_english_words_and_contractions_preserve_spelling_and_case() {
    for (seed, expected) in [
        ("rechargi", "recharging"),
        ("rechargeab", "rechargeable"),
        ("unplugg", "unplugged"),
        ("picnick", "picnicked"),
        ("overslep", "overslept"),
        ("swep", "swept"),
        ("rearrangem", "rearrangement"),
        ("dishcl", "dishcloth"),
        ("pillowca", "pillowcase"),
        ("restocki", "restocking"),
        ("roomma", "roommate"),
        ("touchpa", "touchpad"),
        ("screensav", "screensaver"),
        ("could'v", "could've"),
        ("should’v", "should’ve"),
        ("Would’v", "Would’ve"),
        ("HE’L", "HE’LL"),
        ("She'l", "She'll"),
        ("who'd", "who'd"),
    ] {
        let engine = engine("en", seed);
        assert_on_first_page(&engine, expected, CandidateKind::Word);
        assert!(engine.candidates().iter().all(|c| c.text.starts_with(seed)));
    }
}

#[test]
fn daily_english_collocations_survive_spaces_case_and_long_prefixes() {
    for &(context, partial, word, sentence) in DAILY_ENGLISH {
        for seed in [
            context.to_owned(),
            format!("{context} "),
            format!("{context} {partial}"),
        ] {
            let mut engine = engine("en", &seed);
            assert_on_first_page(&engine, &format!("{context} {word}"), CandidateKind::Word);
            assert_on_first_page(&engine, sentence, CandidateKind::Sentence);
            select_and_commit(&mut engine, sentence);
        }
        // Curly apostrophes and doubled separators belong to the literal draft.
        let prefix = "Keep this already written text exactly. ".repeat(16);
        let seed = format!("{prefix}{}  {partial}", context.replace('\'', "’"));
        let expected = format!(
            "{prefix}{}  {}",
            context.replace('\'', "’"),
            &sentence[context.len() + 1..]
        );
        let mut engine = engine("en", &seed);
        assert_on_first_page(&engine, &expected, CandidateKind::Sentence);
        assert!(
            engine
                .candidates()
                .iter()
                .all(|c| c.text.starts_with(&seed))
        );
        select_and_commit(&mut engine, &expected);
    }
}

#[test]
fn daily_chinese_readings_keep_words_and_sentences_on_the_first_page() {
    for &(reading, word, sentence) in DAILY_CHINESE {
        for seed in [
            reading.to_owned(),
            reading.replace('\'', ""),
            reading.replace('\'', " "),
            reading.to_uppercase(),
        ] {
            let mut engine = engine("zh-Hans", &seed);
            assert_eq!(engine.candidates()[0].text, word, "{seed}");
            assert_on_first_page(&engine, word, CandidateKind::Word);
            assert_on_first_page(&engine, sentence, CandidateKind::Sentence);
            assert!(engine.candidates().iter().any(|c| c.text == seed));
            assert!(
                engine
                    .candidates()
                    .iter()
                    .all(|c| c.source == CandidateSource::Local)
            );
            select_and_commit(&mut engine, sentence);
        }
    }
}

#[test]
fn daily_chinese_adoption_continues_without_losing_long_drafts_or_padding() {
    for &(_, word, sentence) in DAILY_CHINESE {
        for prefix in [String::new(), "先前写好的这一段不能丢失。".repeat(24)] {
            for padding in ["", " ", "  ", "\u{3000}"] {
                let seed = format!("{prefix}{word}{padding}");
                let expected = format!("{seed}{}", &sentence[word.len()..]);
                let mut engine = engine("zh-Hans", &seed);
                assert!(
                    engine
                        .candidates()
                        .iter()
                        .all(|c| c.text.starts_with(&seed))
                );
                assert_on_first_page(&engine, &expected, CandidateKind::Sentence);
                select_and_commit(&mut engine, &expected);
            }
        }
    }
}

#[test]
fn daily_homophones_leave_old_choices_and_literal_input_available() {
    for (seed, primary, alternatives) in [
        ("shijian", "时间", &["事件", "实践", "世间"][..]),
        ("shiyong", "使用", &["食用", "实用", "适用"][..]),
        ("anjian", "安检", &["按键"][..]),
        ("shoushi", "收拾", &["手势"][..]),
        ("dianliang", "电量", &["点亮"][..]),
    ] {
        let mut engine = engine("zh-Hans", seed);
        assert_eq!(engine.candidates()[0].text, primary, "{seed}");
        assert!(engine.candidates().iter().any(|c| c.text == seed));
        for expected in alternatives {
            assert_on_first_page(&engine, expected, CandidateKind::Word);
        }
        select_and_commit(&mut engine, alternatives.last().unwrap());
    }
}
