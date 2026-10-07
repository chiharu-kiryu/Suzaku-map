//! Authored progress replies, clarification, pauses and trying again.
//! Expectations are fixed independently of the data resource. These tests use
//! the offline fallback only; native physical-key routing has separate gates.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

const WORDS: &[(&str, &str)] = &[
    ("wo'zhe'bian'hao'le", "我这边好了"),
    ("zhe'bian'nong'hao'le", "这边弄好了"),
    ("yi'jing'nong'hao'le", "已经弄好了"),
    ("hai'zai'chu'li'zhong", "还在处理中"),
    ("hai'cha'yi'dian'dian", "还差一点点"),
    ("kuai'yao'zuo'hao'le", "快要做好了"),
    ("ma'shang'chu'li'hao", "马上处理好"),
    ("zuo'hao'le", "做好了"),
    ("nong'hao'le", "弄好了"),
    ("wan'cheng'yi'ban", "完成一半"),
    ("zuo'dao'yi'ban", "做到一半"),
    ("xian'zuo'zhe'bu'fen", "先做这部分"),
    ("zhe'bu'zuo'hao'le", "这步做好了"),
    ("hou'mian'ji'xu'zuo", "后面继续做"),
    ("xian'ba'zhe'bu'zuo'wan", "先把这步做完"),
    ("gang'gang'zuo'wan", "刚刚做完"),
    ("ji'xu'wang'xia", "继续往下"),
    ("jie'zhe'wang'xia", "接着往下"),
    ("ji'xu'wang'qian", "继续往前"),
    ("xian'ji'xu'ba", "先继续吧"),
    ("ji'xu'jiu'hao", "继续就好"),
    ("ke'yi'jie'zhe'shuo", "可以接着说"),
    ("jie'zhe'shuo'ba", "接着说吧"),
    ("jie'zhe'lai'ba", "接着来吧"),
    ("wang'xia'kan'ba", "往下看吧"),
    ("xian'ting'yi'xia", "先停一下"),
    ("zan'ting'yi'xia", "暂停一下"),
    ("xian'dao'zhe'li", "先到这里"),
    ("dao'zhe'li'jiu'hao", "到这里就好"),
    ("deng'wo'yi'hui'er", "等我一会儿"),
    ("shao'deng'pian'ke", "稍等片刻"),
    ("yi'hui'er'jiu'hao", "一会儿就好"),
    ("bu'shi'zhe'ge'yi'si", "不是这个意思"),
    ("wo'bu'shi'zhe'ge'yi'si", "我不是这个意思"),
    ("wo'li'jie'cuo'le", "我理解错了"),
    ("gang'cai'li'jie'cuo'le", "刚才理解错了"),
    ("wo'shuo'de'shi'zhe'ge", "我说的是这个"),
    ("ni'shuo'de'mei'cuo", "你说得没错"),
    ("zhe'yang'jiu'dui'le", "这样就对了"),
    ("yuan'lai'shi'zhe'yang", "原来是这样"),
    ("zhe'ci'ming'bai'le", "这次明白了"),
    ("wo'zai'shuo'ming'yi'xia", "我再说明一下"),
    ("wo'chong'xin'shuo'yi'bian", "我重新说一遍"),
    ("shuo'ming'bai'yi'dian", "说明白一点"),
    ("jiang'ju'ti'yi'dian", "讲具体一点"),
    ("wo'mei'gen'shang", "我没跟上"),
    ("gang'cai'mei'gen'shang", "刚才没跟上"),
    ("mei'gen'shang", "没跟上"),
    ("chong'xin'lai'guo", "重新来过"),
    ("cong'tou'zai'lai", "从头再来"),
    ("chong'xin'shi'yi'ci", "重新试一次"),
    ("zai'lai'yi'bian", "再来一遍"),
    ("zai'zuo'yi'bian", "再做一遍"),
    ("wo'zai'shi'yi'ci", "我再试一次"),
    ("cong'zhe'yi'bu'kai'shi", "从这一步开始"),
    ("hui'dao'shang'yi'bu", "回到上一步"),
    ("cha'yi'bu'jiu'hao'le", "差一步就好了"),
    ("cha'zui'hou'yi'dian", "差最后一点"),
    ("hai'you'yi'dian'dian", "还有一点点"),
    ("zai'deng'yi'xiao'hui", "再等一小会"),
    ("hai'xu'yao'yi'dian'shi'jian", "还需要一点时间"),
    ("hai'zai'he'dui", "还在核对"),
    ("wo'zai'he'dui'yi'xia", "我再核对一下"),
    ("que'ren'hao'le'gao'su'ni", "确认好了告诉你"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "我这边好了",
        ["我这边好了，你那边怎么样？", "我这边好了，可以继续往下。"],
    ),
    (
        "已经弄好了",
        ["已经弄好了，你再看一下。", "已经弄好了，剩下的等你确认。"],
    ),
    (
        "还在处理中",
        ["还在处理中，稍后给你结果。", "还在处理中，还需要一点时间。"],
    ),
    (
        "还差一点点",
        ["还差一点点，马上就能完成。", "还差一点点，我再核对一下。"],
    ),
    (
        "快要做好了",
        ["快要做好了，你先稍等一会儿。", "快要做好了，完成后告诉你。"],
    ),
    (
        "做好了",
        ["做好了就告诉我一声。", "做好了，我们接着往下看。"],
    ),
    (
        "先做这部分",
        [
            "先做这部分，剩下的明天再处理。",
            "先做这部分，有问题再一起调整。",
        ],
    ),
    (
        "刚刚做完",
        ["刚刚做完，还没来得及检查。", "刚刚做完，正准备发给你。"],
    ),
    (
        "继续往下",
        ["继续往下看，有问题随时说。", "继续往下做，我们快完成了。"],
    ),
    (
        "接着说吧",
        ["接着说吧，我在认真听。", "接着说吧，刚才说到哪里了？"],
    ),
    (
        "先停一下",
        ["先停一下，我确认一个细节。", "先停一下，这一步还没弄清楚。"],
    ),
    (
        "先到这里",
        [
            "先到这里，剩下的下次接着做。",
            "先到这里，我整理一下再继续。",
        ],
    ),
    (
        "等我一会儿",
        ["等我一会儿，我马上回来。", "等我一会儿，还差最后一点。"],
    ),
    (
        "稍等片刻",
        [
            "稍等片刻，我找一下刚才的记录。",
            "稍等片刻，确认好了就回复你。",
        ],
    ),
    (
        "不是这个意思",
        [
            "不是这个意思，我再说明一下。",
            "不是这个意思，我说的是另一件事。",
        ],
    ),
    (
        "我理解错了",
        ["我理解错了，不好意思。", "我理解错了，你能再讲具体一点吗？"],
    ),
    (
        "原来是这样",
        [
            "原来是这样，这次我明白了。",
            "原来是这样，谢谢你说得这么清楚。",
        ],
    ),
    (
        "这次明白了",
        ["这次明白了，我再试一次。", "这次明白了，可以继续说下去。"],
    ),
    (
        "我再说明一下",
        [
            "我再说明一下，免得大家理解错了。",
            "我再说明一下刚才那句话的意思。",
        ],
    ),
    (
        "我没跟上",
        [
            "我没跟上，可以从这一步重新说吗？",
            "我没跟上，请稍微说慢一点。",
        ],
    ),
    (
        "重新来过",
        [
            "重新来过也没关系，我们一起试试。",
            "重新来过，这次把步骤记下来。",
        ],
    ),
    (
        "从头再来",
        ["从头再来吧，这次慢一点。", "从头再来，先确认第一步。"],
    ),
    (
        "回到上一步",
        [
            "回到上一步看看，可能漏了什么。",
            "回到上一步，再核对一下顺序。",
        ],
    ),
    (
        "还需要一点时间",
        [
            "还需要一点时间，我正在核对。",
            "还需要一点时间，做好了再告诉你。",
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
        // An ambiguous prefix may keep the raw spelling on a later page.
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
fn followup_words_are_on_page_one_in_all_four_spellings_without_a_model() {
    assert_eq!(WORDS.len(), 64);
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
fn explicit_followup_readings_offer_words_and_both_sentences_on_page_one() {
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
fn unfinished_followup_readings_keep_words_and_authored_sentence_choices() {
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
fn adopted_followup_words_preserve_space_padding_without_committing() {
    for &(word, sentences) in SENTENCES {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for sentence in sentences {
                let mut ime = engine(reading_for(word));
                ime.select_candidate(first_page_index(&ime, word, CandidateKind::Word));
                let adopted = ime.selected_completion_text(true).unwrap().to_owned();
                assert_eq!(adopted, word);
                assert!(ime.snapshot().committed_text.is_empty());
                // Editable host-style adoption followed by Space, not a
                // simulated physical key or a selection-triggered commit.
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
fn followup_sentences_continue_after_adopted_words_and_more_pinyin() {
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for (word, tail_reading, tail_word, sentence) in [
                (
                    "我再说明一下",
                    "gang'cai",
                    "刚才",
                    "我再说明一下刚才那句话的意思。",
                ),
                (
                    "重新来过",
                    "ye'mei'guan'xi",
                    "也没关系",
                    "重新来过也没关系，我们一起试试。",
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
                for tail in std::iter::once(tail_word.to_owned()).chain(spellings(tail_reading)) {
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
