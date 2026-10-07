//! Authored leisure, hobbies, weekend plans and relaxed outings.
//! The fixed expectations are independent of production JSON and model output.
//! Editable Space updates are checked here; physical IBus keys have own gates.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

const WORDS: &[(&str, &str)] = &[
    ("zhou'mo'you'kong'ma", "周末有空吗"),
    ("zhou'mo'xiang'qu'na", "周末想去哪"),
    ("zhou'mo'zen'me'guo", "周末怎么过"),
    ("zhou'mo'jian'yi'mian", "周末见一面"),
    ("zhou'mo'zai'jia'xiu'xi", "周末在家休息"),
    ("zhou'mo'bu'gan'shi'jian", "周末不赶时间"),
    ("chu'qu'zou'yi'zou", "出去走一走"),
    ("yi'qi'chu'qu'guang'guang", "一起出去逛逛"),
    ("sui'bian'guang'yi'guang", "随便逛一逛"),
    ("zhao'ge'an'jing'de'di'fang", "找个安静的地方"),
    ("qu'gong'yuan'zou'zou", "去公园走走"),
    ("qu'he'bian'san'bu", "去河边散步"),
    ("qu'kan'ge'zhan'lan", "去看个展览"),
    ("kan'chang'dian'ying", "看场电影"),
    ("kan'kan'xin'dian'ying", "看看新电影"),
    ("xiang'ting'dian'yin'yue", "想听点音乐"),
    ("ting'ting'xi'huan'de'ge", "听听喜欢的歌"),
    ("xiang'du'hui'er'shu", "想读会儿书"),
    ("zhao'ben'shu'kan'kan", "找本书看看"),
    ("shi'shi'xin'ai'hao", "试试新爱好"),
    ("xue'dian'xin'dong'xi", "学点新东西"),
    ("zui'jin'zai'xue'shen'me", "最近在学什么"),
    ("zui'jin'xi'huan'shen'me", "最近喜欢什么"),
    ("ping'shi'zen'me'fang'song", "平时怎么放松"),
    ("hua'dian'xiao'hua", "画点小画"),
    ("pai'dian'feng'jing", "拍点风景"),
    ("sui'shou'pai'ji'zhang", "随手拍几张"),
    ("lian'yi'hui'er'yue'qi", "练一会儿乐器"),
    ("zuo'dian'shou'gong", "做点手工"),
    ("man'man'xiang'shou", "慢慢享受"),
    ("fang'kong'yi'hui'er", "放空一会儿"),
    ("gei'zi'ji'fang'ge'jia", "给自己放个假"),
    ("bu'yong'an'pai'tai'man", "不用安排太满"),
    ("qing'song'yi'dian'jiu'hao", "轻松一点就好"),
    ("chu'lai'tou'tou'qi", "出来透透气"),
    ("huan'ge'xin'qing", "换个心情"),
    ("jin'tian'guo'de'ting'qing'song", "今天过得挺轻松"),
    ("jin'tian'wan'de'kai'xin", "今天玩得开心"),
    ("zhe'yang'hen'fang'song", "这样很放松"),
    ("gan'jue'you'dian'lei", "感觉有点累"),
    ("xiang'xiu'xi'yi'hui'er", "想休息一会儿"),
    ("bu'ji'zhe'hui'qu", "不急着回去"),
    ("san'san'bu'jiu'hao", "散散步就好"),
    ("zhao'dian'you'qu'de'shi", "找点有趣的事"),
    ("zhe'di'fang'ting'shu'fu", "这地方挺舒服"),
    ("zhou'wei'hen'an'jing", "周围很安静"),
    ("wai'mian'feng'jing'bu'cuo", "外面风景不错"),
    ("xia'ci'hai'xiang'zai'lai", "下次还想再来"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "周末有空吗",
        [
            "周末有空吗，一起出去走走吧。",
            "周末有空吗，我想约你看场电影。",
        ],
    ),
    (
        "周末怎么过",
        [
            "周末怎么过，想在家休息还是出门逛逛？",
            "周末怎么过，我们先看天气再决定。",
        ],
    ),
    (
        "周末在家休息",
        [
            "周末在家休息，听听音乐看看书。",
            "周末在家休息，给自己放个小假。",
        ],
    ),
    (
        "出去走一走",
        ["出去走一走，换个心情也挺好。", "出去走一走，回来再接着忙。"],
    ),
    (
        "去公园走走",
        [
            "去公园走走吧，顺便看看花。",
            "去公园走走，不用安排太多事情。",
        ],
    ),
    (
        "看场电影",
        ["看场电影放松一下吧。", "看场电影，结束后再找个地方吃饭。"],
    ),
    (
        "想听点音乐",
        [
            "想听点音乐，你有什么推荐吗？",
            "想听点音乐，安静一点的就好。",
        ],
    ),
    (
        "想读会儿书",
        [
            "想读会儿书，晚一点再联系你。",
            "想读会儿书，给自己留点安静时间。",
        ],
    ),
    (
        "试试新爱好",
        [
            "试试新爱好，慢慢学就好。",
            "试试新爱好，说不定会发现新的乐趣。",
        ],
    ),
    (
        "最近喜欢什么",
        [
            "最近喜欢什么，可以跟我聊聊吗？",
            "最近喜欢什么电影或者音乐？",
        ],
    ),
    (
        "平时怎么放松",
        [
            "平时怎么放松，我也想试试。",
            "平时怎么放松，有什么简单的办法吗？",
        ],
    ),
    (
        "拍点风景",
        ["拍点风景，留作今天的小纪念。", "拍点风景，不用急着赶路。"],
    ),
    (
        "放空一会儿",
        ["放空一会儿，等会儿再想别的。", "放空一会儿，什么都不安排。"],
    ),
    (
        "不用安排太满",
        [
            "不用安排太满，留点时间慢慢走。",
            "不用安排太满，轻松一点就好。",
        ],
    ),
    (
        "出来透透气",
        ["出来透透气，心情会轻松一些。", "出来透透气，顺便活动一下。"],
    ),
    (
        "下次还想再来",
        [
            "下次还想再来，这里待着很舒服。",
            "下次还想再来，到时候一起吧。",
        ],
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
        ime.candidates().iter().any(|candidate| {
            candidate.text == seed && candidate.kind == CandidateKind::Literal
        })
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
    let index = if kind == CandidateKind::Literal {
        ime.candidates()
            .iter()
            .position(|candidate| candidate.text == text && candidate.kind == kind)
            .unwrap_or_else(|| panic!("missing literal {text:?}: {:?}", ime.candidates()))
    } else {
        first_page_index(ime, text, kind)
    };
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
    assert!(ime.undo().is_none());
}

#[test]
fn leisure_words_are_on_page_one_in_all_four_spellings_without_a_model() {
    assert_eq!(WORDS.len(), 48);
    for &(reading, word) in WORDS {
        for seed in spellings(reading) {
            let mut ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            commit_once_and_undo(&mut ime, word, CandidateKind::Word);
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn explicit_leisure_readings_offer_words_and_both_sentences_on_page_one() {
    assert_eq!(SENTENCES.len(), 16);
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
fn unfinished_leisure_readings_keep_words_and_authored_sentence_choices() {
    for &(reading, word) in WORDS {
        let partial = &reading[..reading.len() - 1];
        for seed in spellings(partial) {
            let ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            first_page_index(&ime, word, CandidateKind::Word);
        }
    }
    for &(word, sentences) in SENTENCES {
        let reading = reading_for(word);
        for seed in spellings(&reading[..reading.len() - 1]) {
            let ime = engine(&seed);
            first_page_index(&ime, sentences[0], CandidateKind::Sentence);
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
fn adopted_leisure_words_preserve_space_padding_without_committing() {
    for &(word, sentences) in SENTENCES {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for sentence in sentences {
                let mut ime = engine(reading_for(word));
                ime.select_candidate(first_page_index(&ime, word, CandidateKind::Word));
                let adopted = ime.selected_completion_text(true).unwrap().to_owned();
                assert_eq!(adopted, word);
                assert!(ime.snapshot().committed_text.is_empty());
                // Host-style draft adoption and Space remain editable; this
                // does not manufacture a physical key or commit the choice.
                let seed = format!("{adopted}{padding}");
                ime.seed(&seed);
                assert_local_draft(&ime, &seed);
                assert_eq!(ime.candidates()[0].text, seed);
                let expected = format!("{seed}{}", sentence.strip_prefix(word).unwrap());
                commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
        }
    }
}

#[test]
fn leisure_sentences_continue_after_adoption_and_full_or_partial_pinyin() {
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for (word, tail_reading, tail_word, sentence) in [
                ("看场电影", "fang'song", "放松", "看场电影放松一下吧。"),
                (
                    "最近喜欢什么",
                    "dian'ying",
                    "电影",
                    "最近喜欢什么电影或者音乐？",
                ),
            ] {
                let mut adopted_ime = engine(reading_for(word));
                adopted_ime.select_candidate(first_page_index(
                    &adopted_ime,
                    word,
                    CandidateKind::Word,
                ));
                let adopted = adopted_ime
                    .selected_completion_text(true)
                    .unwrap()
                    .to_owned();
                assert_eq!(adopted, word);
                assert!(adopted_ime.snapshot().committed_text.is_empty());
                let partial = &tail_reading[..tail_reading.len() - 1];
                for tail in std::iter::once(tail_word.to_owned())
                    .chain(spellings(tail_reading))
                    .chain(spellings(partial))
                {
                    let seed = format!("{prefix}{adopted}{padding}{tail}");
                    let expected = format!(
                        "{prefix}{adopted}{padding}{}",
                        sentence.strip_prefix(word).unwrap()
                    );
                    let mut ime = engine(&seed);
                    assert_local_draft(&ime, &seed);
                    commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                    commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
                }
                for trailing in ["", " ", "  ", "\u{3000}"] {
                    let seed = format!("{prefix}{adopted}{padding}{tail_word}{trailing}");
                    let suffix = sentence
                        .strip_prefix(word)
                        .unwrap()
                        .strip_prefix(tail_word)
                        .unwrap();
                    let expected = format!("{seed}{suffix}");
                    let mut ime = engine(&seed);
                    assert_local_draft(&ime, &seed);
                    commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                    commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
                }
            }
        }
    }
}
