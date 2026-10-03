//! Authored ordering, weather-chat and scheduling examples, not factual advice
//! or a general Chinese benchmark. No model, network or user history is used.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

// Fixed expectations intentionally do not read the implementation's JSON.
const WORDS: &[(&str, &str)] = &[
    ("shao'tang", "少糖"),
    ("re'de", "热的"),
    ("bing'de", "冰的"),
    ("bu'fang'la", "不放辣"),
    ("zhong'la", "中辣"),
    ("te'la", "特辣"),
    ("jia'la", "加辣"),
    ("xiao'fen", "小份"),
    ("da'fen", "大份"),
    ("jia'fan", "加饭"),
    ("bu'yao'can'ju", "不要餐具"),
    ("ji'kou", "忌口"),
    ("la'yi'dian", "辣一点"),
    ("tian'yi'dian", "甜一点"),
    ("dan'yi'dian", "淡一点"),
    ("shao'fang'yan", "少放盐"),
    ("shao'fang'you", "少放油"),
    ("bu'yong'jia'tang", "不用加糖"),
    ("mai'dan", "买单"),
    ("jie'zhang", "结账"),
    ("xia'yu'le", "下雨了"),
    ("xia'xue'le", "下雪了"),
    ("qi'feng'le", "起风了"),
    ("hao'leng", "好冷"),
    ("hao're", "好热"),
    ("dai'san", "带伞"),
    ("yu'ting'le", "雨停了"),
    ("tian'qing'le", "天晴了"),
    ("tian'hei'le", "天黑了"),
    ("chu'men'le", "出门了"),
    ("kuai'xia'yu'le", "快下雨了"),
    ("xia'xiao'yu", "下小雨"),
    ("xia'da'yu", "下大雨"),
    ("chuan'wai'tao", "穿外套"),
    ("wai'mian'leng", "外面冷"),
    ("wai'mian're", "外面热"),
    ("tian'qi'bu'cuo", "天气不错"),
    ("shai'tai'yang", "晒太阳"),
    ("dai'jian'wai'tao", "带件外套"),
    ("yin'tian", "阴天"),
    ("qing'tian", "晴天"),
    ("yu'tian", "雨天"),
    ("yue'hao'le", "约好了"),
    ("gai'shi'jian", "改时间"),
    ("lai'bu'ji'le", "来不及了"),
    ("gan'bu'shang'le", "赶不上了"),
    ("ti'qian'yi'dian", "提前一点"),
    ("gai'dao'ming'tian", "改到明天"),
    ("lin'shi'you'shi", "临时有事"),
    ("you'an'pai", "有安排"),
    ("mei'an'pai", "没安排"),
    ("you'kong'ma", "有空吗"),
    ("ji'dian'chu'fa", "几点出发"),
    ("ji'dian'hui'lai", "几点回来"),
    ("guo'yi'hui'er", "过一会儿"),
    ("deng'wo'yi'xia", "等我一下"),
    ("wo'dao'le", "我到了"),
    ("ni'dao'le'ma", "你到了吗"),
    ("gai'tian'zai'yue", "改天再约"),
    ("xian'zhe'yang'ba", "先这样吧"),
    ("wan'dian'dao", "晚点到"),
    ("zao'dian'dao", "早点到"),
    ("deng'ni'hui'fu", "等你回复"),
    ("shi'jian'he'shi", "时间合适"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    ("少糖", ["少糖就好，谢谢。", "少糖的可以做成热的吗？"]),
    (
        "不放辣",
        ["不放辣，清淡一点就好。", "不放辣的这份帮我打包。"],
    ),
    ("小份", ["小份就够了，谢谢。", "小份的可以换一种口味吗？"]),
    ("大份", ["大份可以两个人一起吃吗？", "大份的请帮我分开装。"]),
    ("加饭", ["加饭需要另外收费吗？", "加饭的话请再给我一小碗。"]),
    (
        "不要餐具",
        ["不要餐具，我自己带了。", "不要餐具和吸管，谢谢。"],
    ),
    ("少放盐", ["少放盐，谢谢。", "少放盐的这份是我的。"]),
    ("结账", ["结账可以扫码吗？", "结账的时候请帮我开张小票。"]),
    (
        "下雨了",
        ["下雨了，我带了伞。", "下雨了，我们在门口等一下吧。"],
    ),
    (
        "起风了",
        ["起风了，我去关窗。", "起风了，我们换个地方坐吧。"],
    ),
    (
        "带伞",
        ["带伞出门，免得淋雨。", "带伞了吗，我这里还有一把。"],
    ),
    ("雨停了", ["雨停了，我们再出发吧。", "雨停了，我现在过去。"]),
    (
        "天晴了",
        ["天晴了，出去走走吧。", "天晴了，我想去附近散步。"],
    ),
    (
        "出门了",
        ["出门了，晚点再联系。", "出门了，有事给我发消息。"],
    ),
    (
        "天气不错",
        [
            "天气不错，一起出去走走吧。",
            "天气不错，适合在外面坐一会儿。",
        ],
    ),
    (
        "带件外套",
        ["带件外套，晚上再回来。", "带件外套放在包里吧。"],
    ),
    (
        "约好了",
        ["约好了，那就到时候见。", "约好了，我们在门口碰面。"],
    ),
    (
        "改时间",
        ["改时间的话提前告诉我。", "改时间也可以，看你方便。"],
    ),
    (
        "来不及了",
        ["来不及了，我们改天再约吧。", "来不及了，你们先去吧。"],
    ),
    (
        "临时有事",
        ["临时有事，可能要晚一点。", "临时有事，今天就不过去了。"],
    ),
    (
        "有空吗",
        ["有空吗，想和你聊两句。", "有空吗，我们约个时间见面吧。"],
    ),
    (
        "几点出发",
        ["几点出发比较合适？", "几点出发，你定好了告诉我。"],
    ),
    (
        "等我一下",
        ["等我一下，我马上过来。", "等我一下，我还在收拾东西。"],
    ),
    ("我到了", ["我到了，你在哪儿？", "我到了，在门口等你。"]),
];

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn spellings(reading: &str) -> [String; 4] {
    [
        reading.replace('\'', ""),
        reading.replace('\'', " "),
        reading.to_owned(),
        reading.replace('\'', " ").to_ascii_uppercase(),
    ]
}

fn reading_for(word: &str) -> &'static str {
    WORDS.iter().find(|(_, text)| *text == word).unwrap().0
}

fn assert_local_draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(
        ime.candidates()
            .iter()
            .any(|candidate| candidate.text == seed)
    );
    assert!(
        ime.candidates()
            .iter()
            .all(|candidate| candidate.source == CandidateSource::Local)
    );
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|candidate| &candidate.text)
            .collect::<HashSet<_>>()
            .len()
    );
}

fn first_page_index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .take(PAGE_SIZE)
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()))
}

fn commit_once(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let index = first_page_index(ime, text, kind);
    ime.select_candidate(index);
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let second = ime.commit(CommitOptions { force: true });
    assert!(!second.ok);
    assert_eq!(second.text, None);
    assert_eq!(ime.snapshot().committed_text, text);
}

#[test]
fn daily_needs_words_are_on_page_one_in_all_four_spellings_without_a_model() {
    assert_eq!(WORDS.len(), 64);
    for &(reading, word) in WORDS {
        for seed in spellings(reading) {
            let mut ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            commit_once(&mut ime, word, CandidateKind::Word);
        }
    }
}

#[test]
fn explicit_daily_needs_readings_keep_words_and_both_sentences_on_page_one() {
    assert_eq!(SENTENCES.len(), 24);
    for &(word, sentences) in SENTENCES {
        for seed in spellings(reading_for(word)) {
            let ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            first_page_index(&ime, word, CandidateKind::Word);
            for sentence in sentences {
                commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
            }
        }
    }
}

#[test]
fn daily_needs_partial_final_syllables_offer_both_a_word_and_a_sentence() {
    for &(word, sentences) in SENTENCES {
        let reading = reading_for(word);
        // Keep every explicit boundary and at least the final syllable onset.
        let partial = &reading[..reading.len() - 1];
        for seed in spellings(partial) {
            let ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            first_page_index(&ime, word, CandidateKind::Word);
            first_page_index(&ime, sentences[0], CandidateKind::Sentence);
            // Partial readings may also have older, valid decoded branches.
            // Require both authored alternatives, not all branches on page one.
            for sentence in sentences {
                assert!(
                    ime.candidates().iter().any(|candidate| {
                        candidate.text == sentence && candidate.kind == CandidateKind::Sentence
                    }),
                    "missing continuation {sentence:?} for {seed:?}: {:?}",
                    ime.candidates()
                );
            }
        }
    }
}

#[test]
fn adopted_daily_needs_words_keep_padding_and_commit_each_sentence_exactly_once() {
    for &(word, sentences) in SENTENCES {
        for padding in ["", " ", "  ", "\u{a0}", "\u{3000}"] {
            for sentence in sentences {
                let mut ime = engine(reading_for(word));
                let index = first_page_index(&ime, word, CandidateKind::Word);
                ime.select_candidate(index);
                let adopted = ime.selected_completion_text(true).unwrap().to_owned();
                assert_eq!(adopted, word);
                assert!(ime.snapshot().committed_text.is_empty());
                // Match the editable draft transition, not a word commit.
                // Physical numeric adoption is tested by the native host suite.
                let seed = format!("{adopted}{padding}");
                ime.seed(&seed);
                assert_local_draft(&ime, &seed);
                assert_eq!(ime.candidates()[0].text, seed);
                let expected = format!("{seed}{}", sentence.strip_prefix(word).unwrap());
                commit_once(&mut ime, &expected, CandidateKind::Sentence);
            }
        }
    }
}
