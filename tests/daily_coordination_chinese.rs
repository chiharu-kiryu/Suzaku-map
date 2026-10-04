//! Authored daily clarification, polite requests and coordination phrases.
//! Fixed expectations do not read the resource, use a model or learn from user
//! history. Native numeric adoption and physical Space have separate GTK gates.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

const WORDS: &[(&str, &str)] = &[
    ("zai'jiang'yi'bian", "再讲一遍"),
    ("shuo'man'yi'dian", "说慢一点"),
    ("kan'bu'dong", "看不懂"),
    ("kan'dong'le", "看懂了"),
    ("ting'dong'le", "听懂了"),
    ("mei'kan'qing", "没看清"),
    ("wo'mei'ting'dong", "我没听懂"),
    ("wo'mei'kan'dong", "我没看懂"),
    ("wo'shi'shuo", "我是说"),
    ("shi'zhe'yang'ma", "是这样吗"),
    ("shi'zhe'ge'ma", "是这个吗"),
    ("zhe'yang'dui'ma", "这样对吗"),
    ("mei'nong'ming'bai", "没弄明白"),
    ("mei", "没"),
    ("zai'que'ren'yi'xia", "再确认一下"),
    ("mei'you", "没有"),
    ("wo'cha'yi'xia", "我查一下"),
    ("bang'wo'kan'kan", "帮我看看"),
    ("bang'wo'yi'xia", "帮我一下"),
    ("jie'wo'yi'xia", "借我一下"),
    ("bang'ge'mang", "帮个忙"),
    ("da'ba'shou", "搭把手"),
    ("qing'rang'yi'xia", "请让一下"),
    ("jie'guo'yi'xia", "借过一下"),
    ("bie'ke'qi", "别客气"),
    ("ji'cuo'le", "记错了"),
    ("wo'wang'le", "我忘了"),
    ("nong'cuo'le", "弄错了"),
    ("gao'cuo'le", "搞错了"),
    ("fa'cuo'le", "发错了"),
    ("kan'cuo'le", "看错了"),
    ("ting'cuo'le", "听错了"),
    ("an'cuo'le", "按错了"),
    ("wo'lai'ba", "我来吧"),
    ("jiao'gei'wo'ba", "交给我吧"),
    ("ni'xian'lai", "你先来"),
    ("wo'xian'lai", "我先来"),
    ("yi'qi'zuo", "一起做"),
    ("fen'tou'xing'dong", "分头行动"),
    ("lun'dao'ni'le", "轮到你了"),
    ("lun'dao'wo'le", "轮到我了"),
    ("wo'shi'shi'kan", "我试试看"),
    ("wan'dian'hui'fu", "晚点回复"),
    ("hui'fu'ni'le", "回复你了"),
    ("fa'gei'ni'le", "发给你了"),
    ("you'mei'you", "有没有"),
    ("fa'wo'yi'xia", "发我一下"),
    ("zhuan'fa'yi'xia", "转发一下"),
    ("da'ge'dian'hua", "打个电话"),
    ("jie'ge'dian'hua", "接个电话"),
    ("bu'yong'hui'fu", "不用回复"),
    ("bao'chi'lian'xi", "保持联系"),
    ("sui'shi'zhao'wo", "随时找我"),
    ("you'shi'zhao'wo", "有事找我"),
    ("wo'ji'xia'le", "我记下了"),
    ("ji'xia'lai", "记下来"),
    ("bang'wo'zhuan'gao", "帮我转告"),
    ("fang'bian'de'shi'hou", "方便的时候"),
    ("you'kong'zai'hui", "有空再回"),
    ("xian'mang'ni'de", "先忙你的"),
    ("shuo'ding'le", "说定了"),
    ("mei'wen'ti'de", "没问题的"),
    ("wo'chu'li'yi'xia", "我处理一下"),
    ("wo'wen'yi'xia", "我问一下"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "再讲一遍",
        ["再讲一遍可以吗，我刚才没听清。", "再讲一遍，我想记下来。"],
    ),
    (
        "说慢一点",
        ["说慢一点可以吗，我还没记好。", "说慢一点，我想听清楚。"],
    ),
    (
        "看不懂",
        ["看不懂这句话是什么意思。", "看不懂的地方我标出来了。"],
    ),
    ("看懂了", ["看懂了，谢谢你解释。", "看懂了，我再试一下。"]),
    (
        "听懂了",
        ["听懂了，我来试试看。", "听懂了，谢谢你说得这么清楚。"],
    ),
    (
        "没看清",
        ["没看清，可以再发一张吗？", "没看清，我放大一点再看看。"],
    ),
    (
        "是这样吗",
        ["是这样吗，你帮我确认一下。", "是这样吗，我再核对一遍。"],
    ),
    (
        "这样对吗",
        ["这样对吗，你帮我看看。", "这样对吗，我怕自己弄错了。"],
    ),
    (
        "再确认一下",
        ["再确认一下时间和地点吧。", "再确认一下，有变化及时告诉我。"],
    ),
    (
        "我查一下",
        ["我查一下，稍后告诉你。", "我查一下有没有其他时间。"],
    ),
    (
        "帮我看看",
        ["帮我看看有没有漏掉什么。", "帮我看看这个怎么用。"],
    ),
    (
        "帮个忙",
        ["帮个忙，替我拿一下这个。", "帮个忙，把门扶一下好吗？"],
    ),
    (
        "请让一下",
        ["请让一下，我想从这里过去。", "请让一下，谢谢。"],
    ),
    (
        "别客气",
        ["别客气，能帮上忙就好。", "别客气，有需要随时找我。"],
    ),
    (
        "我忘了",
        ["我忘了，刚才说到哪里了？", "我忘了带钥匙，得回去拿一下。"],
    ),
    ("发错了", ["发错了，不好意思。", "发错了，我重新发给你。"]),
    (
        "我来吧",
        ["我来吧，你先休息一下。", "我来吧，这个我比较熟悉。"],
    ),
    (
        "交给我吧",
        ["交给我吧，弄好了告诉你。", "交给我吧，你先忙别的。"],
    ),
    (
        "发给你了",
        ["发给你了，看看有没有收到。", "发给你了，有空再看就好。"],
    ),
    (
        "发我一下",
        [
            "发我一下地址，我找找怎么过去。",
            "发我一下照片，我确认一下。",
        ],
    ),
    (
        "打个电话",
        ["打个电话说吧，这样比较清楚。", "打个电话问一下，再决定吧。"],
    ),
    (
        "先忙你的",
        ["先忙你的，有空再回复就好。", "先忙你的，我这边不着急。"],
    ),
    (
        "我记下了",
        ["我记下了，谢谢你提醒。", "我记下了，到时候再联系你。"],
    ),
    (
        "帮我转告",
        ["帮我转告他，我晚一点到。", "帮我转告大家，时间改到下午了。"],
    ),
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
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
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

fn commit_once_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let index = first_page_index(ime, text, kind);
    ime.select_candidate(index);
    assert_eq!(ime.selected_completion_text(true), Some(text));
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
    assert!(ime.undo().unwrap().committed_text.is_empty());
}

#[test]
fn coordination_words_are_on_page_one_in_all_four_spellings_without_a_model() {
    assert_eq!(WORDS.len(), 64);
    for &(reading, word) in WORDS {
        for seed in spellings(reading) {
            let mut ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            commit_once_and_undo(&mut ime, word, CandidateKind::Word);
        }
    }
}

#[test]
fn explicit_coordination_readings_keep_words_and_both_sentences_on_page_one() {
    assert_eq!(SENTENCES.len(), 24);
    for &(word, sentences) in SENTENCES {
        for seed in spellings(reading_for(word)) {
            let ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            first_page_index(&ime, word, CandidateKind::Word);
            for sentence in sentences {
                commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence);
            }
        }
    }
}

#[test]
fn unfinished_coordination_readings_keep_word_and_sentence_choices() {
    for &(word, sentences) in SENTENCES {
        let reading = reading_for(word);
        let partial = &reading[..reading.len() - 1];
        for seed in spellings(partial) {
            let ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            first_page_index(&ime, word, CandidateKind::Word);
            first_page_index(&ime, sentences[0], CandidateKind::Sentence);
            // Ambiguous unfinished readings retain older valid branches too.
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
fn adopted_coordination_words_keep_literal_padding_and_remain_uncommitted() {
    for &(word, sentences) in SENTENCES {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for sentence in sentences {
                let mut ime = engine(reading_for(word));
                let index = first_page_index(&ime, word, CandidateKind::Word);
                ime.select_candidate(index);
                let adopted = ime.selected_completion_text(true).unwrap().to_owned();
                assert_eq!(adopted, word);
                assert!(ime.snapshot().committed_text.is_empty());
                // This is the host's editable word-adoption/Space draft update,
                // not a claim to simulate a physical key in this engine test.
                let seed = format!("{adopted}{padding}");
                ime.seed(&seed);
                assert_local_draft(&ime, &seed);
                assert_eq!(ime.candidates()[0].text, seed);
                let expected = format!("{seed}{}", sentence.strip_prefix(word).unwrap());
                commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
            }
        }
    }
}

#[test]
fn coordination_sentences_continue_after_another_word_with_long_literal_prefixes() {
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for next in [
                "有没有",
                "you mei you",
                "you'mei'you",
                "youmeiyou",
                "YOU MEI YOU",
            ] {
                let seed = format!("{prefix}帮我看看{padding}{next}");
                let expected = format!("{prefix}帮我看看{padding}有没有漏掉什么。");
                let mut ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
            }
            for trailing in ["", " ", "  ", "\u{3000}"] {
                let seed = format!("{prefix}帮我看看{padding}有没有{trailing}");
                let expected = format!("{seed}漏掉什么。");
                let mut ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
            }
        }
    }
}
