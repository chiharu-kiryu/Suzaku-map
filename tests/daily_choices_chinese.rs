//! Authored comparisons, preferences, conditions and alternative plans.
//! Expectations are independent of production JSON. This is bounded offline
//! fallback coverage, not a language-quality or live-model benchmark.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

const WORDS: &[(&str, &str)] = &[
    ("na'ge'geng'hao", "哪个更好"),
    ("na'ge'he'shi", "哪个合适"),
    ("you'shen'me'qu'bie", "有什么区别"),
    ("cha'bie'da'ma", "差别大吗"),
    ("cha'bu'tai'duo", "差不太多"),
    ("ge'you'hao'chu", "各有好处"),
    ("ge'you'you'dian", "各有优点"),
    ("bi'zhi'qian'hao", "比之前好"),
    ("bi'xiang'xiang'zhong'hao", "比想象中好"),
    ("bi'zhe'ge'pian'yi", "比这个便宜"),
    ("geng'fang'bian'yi'xie", "更方便一些"),
    ("geng'sheng'shi'jian", "更省时间"),
    ("geng'rong'yi'yi'xie", "更容易一些"),
    ("mei'na'me'fu'za", "没那么复杂"),
    ("cha'zai'na'li", "差在哪里"),
    ("xian'bi'jiao'yi'xia", "先比较一下"),
    ("wo'geng'xi'huan", "我更喜欢"),
    ("wo'bi'jiao'xi'huan", "我比较喜欢"),
    ("wo'qing'xiang'yu", "我倾向于"),
    ("wo'geng'yuan'yi", "我更愿意"),
    ("wo'xiang'xuan'zhe'ge", "我想选这个"),
    ("wo'xuan'zhe'ge", "我选这个"),
    ("ni'geng'xi'huan'na'ge", "你更喜欢哪个"),
    ("ni'xiang'xuan'na'ge", "你想选哪个"),
    ("an'ni'de'xi'hao", "按你的喜好"),
    ("an'shi'ji'xu'yao", "按实际需要"),
    ("zhe'ge'geng'he'shi", "这个更合适"),
    ("na'ge'ye'bu'cuo", "那个也不错"),
    ("mei'you'te'bie'pian'hao", "没有特别偏好"),
    ("wo'bu'tai'jie'yi", "我不太介意"),
    ("dou'ting'hao'de", "都挺好的"),
    ("xuan'ni'xi'huan'de", "选你喜欢的"),
    ("ru'guo'ke'yi'de'hua", "如果可以的话"),
    ("ru'guo'lai'de'ji", "如果来得及"),
    ("ru'guo'bu'fang'bian", "如果不方便"),
    ("yao'shi'you'kong", "要是有空"),
    ("yao'shi'xia'yu", "要是下雨"),
    ("you'xu'yao'de'hua", "有需要的话"),
    ("bu'zhao'ji'de'hua", "不着急的话"),
    ("shi'jian'yun'xu'de'hua", "时间允许的话"),
    ("kan'tian'qi'zai'ding", "看天气再定"),
    ("kan'qing'kuang'zai'shuo", "看情况再说"),
    ("dao'shi'hou'zai'jue'ding", "到时候再决定"),
    ("zhi'yao'he'shi", "只要合适"),
    ("zhi'yao'fang'bian", "只要方便"),
    ("bu'yi'ding'fei'yao", "不一定非要"),
    ("deng'que'ren'yi'hou", "等确认以后"),
    ("xian'bie'ji'zhe'jue'ding", "先别急着决定"),
    ("huan'ge'ban'fa", "换个办法"),
    ("huan'yi'zhong'fang'shi", "换一种方式"),
    ("hai'you'bie'de'ma", "还有别的吗"),
    ("you'mei'you'bie'de'xuan'ze", "有没有别的选择"),
    ("yao'bu'huan'yi'ge", "要不换一个"),
    ("yao'bu'gai'tian'ba", "要不改天吧"),
    ("xian'shi'shi'kan", "先试试看"),
    ("shi'shi'ling'yi'zhong", "试试另一种"),
    ("shi'zai'bu'xing", "实在不行"),
    ("bu'xing'zai'huan", "不行再换"),
    ("ye'ke'yi'zhe'yang", "也可以这样"),
    ("zhe'yang'ye'xing", "这样也行"),
    ("bu'ru'xian'zhe'yang", "不如先这样"),
    ("xian'an'zhe'ge'lai", "先按这个来"),
    ("liu'ge'bei'xuan", "留个备选"),
    ("zai'xiang'xiang'ban'fa", "再想想办法"),
    ("zhe'ge", "这个"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "哪个更好",
        ["哪个更好，我们一起看看。", "哪个更好，要看你更在意什么。"],
    ),
    (
        "有什么区别",
        [
            "有什么区别，可以具体说说吗？",
            "有什么区别，我们对照着看一下。",
        ],
    ),
    (
        "差别大吗",
        ["差别大吗，值得换一个吗？", "差别大吗，我想先了解一下。"],
    ),
    (
        "各有优点",
        ["各有优点，选适合自己的就好。", "各有优点，我们再比较一下。"],
    ),
    (
        "更省时间",
        [
            "更省时间的话，就按这个办法来。",
            "更省时间，也方便大家安排。",
        ],
    ),
    (
        "先比较一下",
        ["先比较一下价格和使用体验。", "先比较一下，再决定选哪一个。"],
    ),
    (
        "我更喜欢",
        ["我更喜欢这个，简单又方便。", "我更喜欢安静一点的地方。"],
    ),
    (
        "我倾向于",
        ["我倾向于先试一试，再做决定。", "我倾向于选更方便的那个。"],
    ),
    (
        "你更喜欢哪个",
        ["你更喜欢哪个，我们可以一起选。", "你更喜欢哪个颜色？"],
    ),
    (
        "这个更合适",
        ["这个更合适，就先用这个吧。", "这个更合适，带着也方便。"],
    ),
    (
        "没有特别偏好",
        ["没有特别偏好，你来选就好。", "没有特别偏好，只要方便就行。"],
    ),
    (
        "选你喜欢的",
        ["选你喜欢的，不用迁就我。", "选你喜欢的颜色就好。"],
    ),
    (
        "如果来得及",
        [
            "如果来得及，我们就一起过去。",
            "如果来得及，我想再检查一遍。",
        ],
    ),
    (
        "如果不方便",
        ["如果不方便，我们就换个时间。", "如果不方便，不用勉强。"],
    ),
    (
        "有需要的话",
        ["有需要的话，随时告诉我。", "有需要的话，我可以帮你看看。"],
    ),
    (
        "不着急的话",
        [
            "不着急的话，我们明天再处理。",
            "不着急的话，可以多比较几种。",
        ],
    ),
    (
        "看天气再定",
        [
            "看天气再定，下雨就换个安排。",
            "看天气再定，出门前再确认一下。",
        ],
    ),
    (
        "等确认以后",
        [
            "等确认以后再决定，先不用着急。",
            "等确认以后，我再告诉你结果。",
        ],
    ),
    (
        "换个办法",
        [
            "换个办法之前，我们先看看问题在哪里。",
            "换个办法试试，也许会更顺利。",
        ],
    ),
    (
        "还有别的吗",
        ["还有别的吗，我想再看看。", "还有别的吗，这个不太合适。"],
    ),
    (
        "要不换一个",
        [
            "要不换一个，试试别的颜色。",
            "要不换一个时间，大家都方便些。",
        ],
    ),
    (
        "先试试看",
        ["先试试看效果，不合适再调整。", "先试试看，不用现在就决定。"],
    ),
    (
        "实在不行",
        ["实在不行，我们就改天再试。", "实在不行，先用原来的办法。"],
    ),
    (
        "留个备选",
        ["留个备选，免得临时来不及。", "留个备选方案，会更安心一些。"],
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
        // Broad prefix competition may move raw input onto a later page.
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
fn choice_words_are_on_page_one_in_all_four_spellings() {
    assert_eq!(WORDS.len(), 65);
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
fn explicit_choice_readings_offer_words_and_both_sentences_on_page_one() {
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
fn unfinished_choice_readings_keep_words_and_authored_sentence_choices() {
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
        let partial = &reading[..reading.len() - 1];
        for seed in spellings(partial) {
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
fn adopted_choice_words_preserve_padding_and_remain_editable() {
    for &(word, sentences) in SENTENCES {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for sentence in sentences {
                let mut ime = engine(reading_for(word));
                let index = first_page_index(&ime, word, CandidateKind::Word);
                ime.select_candidate(index);
                let adopted = ime.selected_completion_text(true).unwrap().to_owned();
                assert_eq!(adopted, word);
                assert!(ime.snapshot().committed_text.is_empty());
                // Host-style editable draft update, not a physical key event.
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
fn choice_sentences_continue_after_new_pinyin_without_rewriting_literal_prefixes() {
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for next in ["这个", "zhe ge", "zhe'ge", "zhege", "ZHE GE"] {
                let seed = format!("{prefix}我更喜欢{padding}{next}");
                let expected = format!("{prefix}我更喜欢{padding}这个，简单又方便。");
                let mut ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
            for trailing in ["", " ", "  ", "\u{3000}"] {
                let seed = format!("{prefix}我更喜欢{padding}这个{trailing}");
                let expected = format!("{seed}，简单又方便。");
                let mut ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
        }
    }
}

#[test]
fn standalone_demonstrative_composes_after_different_adopted_words() {
    for seed in spellings("zhe'ge") {
        let mut ime = engine(&seed);
        assert_local_draft(&ime, &seed);
        first_page_index(&ime, "这个", CandidateKind::Word);
        // The shorter exact reading must coexist with old longer expressions,
        // not replace their entries or force their completions out of the list.
        for previous in ["这个词", "这个意思"] {
            assert!(
                ime.candidates().iter().any(|candidate| {
                    candidate.text == previous && candidate.kind == CandidateKind::Word
                }),
                "lost old completion {previous:?} for {seed:?}: {:?}",
                ime.candidates()
            );
        }
        commit_once_and_undo(&mut ime, "这个", CandidateKind::Word);
        commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
    }

    for (reading, prefix) in [
        ("xi'huan", "喜欢"),
        ("xuan'ze", "选择"),
        ("wo'geng'xi'huan", "我更喜欢"),
    ] {
        let mut ime = engine(reading);
        ime.select_candidate(first_page_index(&ime, prefix, CandidateKind::Word));
        let adopted = ime.selected_completion_text(true).unwrap().to_owned();
        assert_eq!(adopted, prefix);
        assert!(ime.snapshot().committed_text.is_empty());
        for padding in ["", " ", "\u{3000}"] {
            for tail in spellings("zhe'ge") {
                let seed = format!("{adopted}{padding}{tail}");
                let expected = format!("{adopted}{padding}这个");
                ime.seed(&seed);
                assert_local_draft(&ime, &seed);
                // Full multiword conversion uses the existing sentence kind;
                // this test does not invent a dictionary entry for each prefix.
                commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
        }
    }
}
