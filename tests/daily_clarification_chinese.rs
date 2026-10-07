//! Independent authored fallback examples for clarification and confirmation.
//! The fixed word/sentence expectations do not come from production JSON or a
//! model. Historical content hashes were captured before this batch was added.
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    languages::llm::{
        LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
    },
    lexicon::{self, EntryKind},
};

const WORDS: &[(&str, &str)] = &[
    ("shuo'ju'ti'yi'dian", "说具体一点"),
    ("wo'li'jie'de'shi", "我理解的是"),
    ("shi'bu'shi'zhe'ge'yi'si", "是不是这个意思"),
    ("wo'que'ren'yi'xia", "我确认一下"),
    ("jie'shi'qing'chu'yi'dian", "解释清楚一点"),
    ("zai'ju'ge'li'zi", "再举个例子"),
    ("ju'ge'jian'dan'li'zi", "举个简单例子"),
    ("huan'ge'jiao'du'shuo", "换个角度说"),
    ("huan'ju'hua'zai'shuo", "换句话再说"),
    ("wo'huan'ge'shuo'fa", "我换个说法"),
    ("yong'jian'dan'de'hua'shuo", "用简单的话说"),
    ("shuo'de'zhi'bai'yi'dian", "说得直白一点"),
    ("ni'zhi'de'shi", "你指的是"),
    ("wo'zhi'de'shi", "我指的是"),
    ("zhe'li'zhi'de'shi", "这里指的是"),
    ("ni'shi'shuo'zhe'ge'ma", "你是说这个吗"),
    ("ni'de'yi'si'shi'shuo", "你的意思是说"),
    ("wo'xiang'biao'da'de'shi", "我想表达的是"),
    ("shi'bu'shi'zhe'yang'li'jie", "是不是这样理解"),
    ("zhe'yang'li'jie'dui'ma", "这样理解对吗"),
    ("wo'li'jie'de'dui'ma", "我理解得对吗"),
    ("zhe'yang'shuo'qing'chu'ma", "这样说清楚吗"),
    ("neng'shuo'ju'ti'dian'ma", "能说具体点吗"),
    ("neng'zai'jie'shi'yi'xia'ma", "能再解释一下吗"),
    ("ru'he'li'jie'zhe'ju'hua", "如何理解这句话"),
    ("zhe'ju'hua'shi'shen'me'yi'si", "这句话是什么意思"),
    ("wo'hai'bu'tai'ming'bai", "我还不太明白"),
    ("wo'you'dian'mei'ming'bai", "我有点没明白"),
    ("you'dian'mei'ting'dong", "有点没听懂"),
    ("wo'zai'que'ren'yi'bian", "我再确认一遍"),
    ("wo'xian'que'ren'yi'xia", "我先确认一下"),
    ("que'ren'qing'chu'zai'shuo", "确认清楚再说"),
    ("xian'he'dui'yi'xia", "先核对一下"),
    ("wo'zai'he'shi'yi'xia", "我再核实一下"),
    ("gei'wo'dian'ti'shi", "给我点提示"),
    ("neng'ju'ge'li'zi'ma", "能举个例子吗"),
    ("na'zhe'ge'ju'li", "拿这个举例"),
    ("bi'ru'shuo'zhe'ge", "比如说这个"),
    ("wo'da'gai'ming'bai'le", "我大概明白了"),
    ("zhe'xia'ming'bai'le", "这下明白了"),
    ("zhe'yang'jiu'qing'chu'le", "这样就清楚了"),
    ("ming'bai'ni'de'yi'si'le", "明白你的意思了"),
    ("wo'zhi'dao'ni'de'yi'si'le", "我知道你的意思了"),
    ("wo'shuo'qing'chu'le'ma", "我说清楚了吗"),
    ("zai'shuo'de'xiang'xi'yi'dian", "再说得详细一点"),
    ("shuo'de'jian'dan'yi'dian", "说得简单一点"),
    ("jie'shi'yi'xia'yuan'yin", "解释一下原因"),
    ("neng'bu'neng'xiang'xi'shuo'shuo", "能不能详细说说"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "说具体一点",
        [
            "说具体一点，我想知道是哪一步。",
            "说具体一点，我们再一起确认。",
        ],
    ),
    (
        "我理解的是",
        [
            "我理解的是这个意思，你看看对不对。",
            "我理解的是先确认，再往下继续。",
        ],
    ),
    (
        "是不是这个意思",
        [
            "是不是这个意思，你帮我确认一下。",
            "是不是这个意思，我怕理解错了。",
        ],
    ),
    (
        "我确认一下",
        ["我确认一下时间，再回复你。", "我确认一下，免得漏掉什么。"],
    ),
    (
        "再举个例子",
        [
            "再举个例子，我就更容易理解了。",
            "再举个例子，最好是日常会遇到的。",
        ],
    ),
    (
        "换个角度说",
        [
            "换个角度说，也许会更容易明白。",
            "换个角度说，我们关注的是同一个问题。",
        ],
    ),
    (
        "我换个说法",
        [
            "我换个说法，看看这样清不清楚。",
            "我换个说法，不是要改变原来的意思。",
        ],
    ),
    (
        "用简单的话说",
        [
            "用简单的话说，就是先做好这一步。",
            "用简单的话说，让大家都容易理解。",
        ],
    ),
    ("你指的是", ["你指的是这个部分吗？", "你指的是哪一个例子？"]),
    (
        "我想表达的是",
        [
            "我想表达的是这个意思，不是另一个。",
            "我想表达的是先商量，再决定。",
        ],
    ),
    (
        "这样理解对吗",
        [
            "这样理解对吗，你帮我核对一下。",
            "这样理解对吗，我再试着说一遍。",
        ],
    ),
    (
        "能再解释一下吗",
        [
            "能再解释一下吗，我还没完全明白。",
            "能再解释一下吗，尤其是后面那一步。",
        ],
    ),
    (
        "我还不太明白",
        [
            "我还不太明白，可以说具体一点吗？",
            "我还不太明白，我们慢慢看。",
        ],
    ),
    (
        "先核对一下",
        [
            "先核对一下这部分，再继续吧。",
            "先核对一下，确定没问题再说。",
        ],
    ),
    (
        "我大概明白了",
        [
            "我大概明白了，再试一遍看看。",
            "我大概明白了，还有一个地方想确认。",
        ],
    ),
    (
        "我说清楚了吗",
        [
            "我说清楚了吗，要不要再举个例子？",
            "我说清楚了吗，你可以用自己的话说说。",
        ],
    ),
];

#[test]
fn clarification_preserves_all_preexisting_chinese_readings_and_contexts() {
    // Freeze the full leisure-era prefix without replacing any older snapshot.
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 2544);
    assert!(vocabulary.continuations().count() >= 555);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(2544) {
        let kind = match entry.kind {
            EntryKind::Word => "word",
            EntryKind::Sentence => "sentence",
        };
        for byte in entry
            .reading
            .bytes()
            .chain([0])
            .chain(entry.text.bytes())
            .chain([0])
            .chain(kind.bytes())
            .chain([u8::from(entry.require_separators)])
        {
            reading_hash = (reading_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    assert_eq!(reading_hash, 0xb326f817cb1721f4);
    let mut context_hash = 0xcbf29ce484222325_u64;
    for (context, values) in vocabulary.continuations().take(555) {
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(context_hash, 0xe13d83a4ea2adf00);
}

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
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
    commit_index_once_and_undo(ime, index, text, kind);
}

fn commit_index_once_and_undo(
    ime: &mut XRTabletImeEngine,
    index: usize,
    text: &str,
    kind: CandidateKind,
) {
    assert_eq!(ime.candidates()[index].text, text);
    assert_eq!(ime.candidates()[index].kind, kind);
    assert_eq!(ime.candidates()[index].source, CandidateSource::Local);
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
fn clarification_words_are_on_page_one_in_all_four_spellings_without_a_model() {
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
fn explicit_clarification_readings_offer_words_and_both_sentences_on_page_one() {
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
fn unfinished_clarification_readings_keep_words_and_authored_sentence_choices() {
    for &(reading, word) in WORDS {
        for seed in spellings(&reading[..reading.len() - 1]) {
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
                    "missing {sentence:?} for {seed:?}: {:?}",
                    ime.candidates()
                );
            }
        }
    }
}

#[test]
fn adopted_clarification_words_preserve_space_padding_without_committing() {
    for &(word, sentences) in SENTENCES {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for sentence in sentences {
                let mut ime = engine(reading_for(word));
                ime.select_candidate(first_page_index(&ime, word, CandidateKind::Word));
                let adopted = ime.selected_completion_text(true).unwrap().to_owned();
                assert_eq!(adopted, word);
                assert!(ime.snapshot().committed_text.is_empty());
                // Host-style editable adoption/Space, not physical-key simulation.
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
fn clarification_sentences_continue_after_adoption_and_more_pinyin() {
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for (word, tail_reading, tail_word, sentence) in [
                (
                    "我理解的是",
                    "zhe'ge",
                    "这个",
                    "我理解的是这个意思，你看看对不对。",
                ),
                (
                    "我确认一下",
                    "shi'jian",
                    "时间",
                    "我确认一下时间，再回复你。",
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
                    if word == "我理解的是" && spellings(partial).contains(&tail) {
                        // `zheg` also completes old 这个词/这个更合适. Preserve
                        // those branches and prove exact later-page reachability
                        // instead of requiring every ambiguity on page one.
                        let older = [
                            ("这个词", CandidateKind::Word),
                            ("这个意思", CandidateKind::Word),
                            ("这个更合适", CandidateKind::Word),
                            ("这个", CandidateKind::Word),
                            ("这个词是什么意思？", CandidateKind::Sentence),
                            ("这个词应该怎么用？", CandidateKind::Sentence),
                        ];
                        for (candidate, (suffix, kind)) in ime.candidates().iter().zip(older) {
                            assert_eq!(
                                candidate.text,
                                format!("{prefix}{adopted}{padding}{suffix}")
                            );
                            assert_eq!(candidate.kind, kind);
                        }
                        assert_eq!(ime.candidates().len(), 10);
                        assert_eq!(PAGE_SIZE, 6);
                        commit_index_once_and_undo(
                            &mut ime,
                            PAGE_SIZE,
                            &expected,
                            CandidateKind::Sentence,
                        );
                    } else {
                        commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                    }
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

struct UnavailableProvider(LlmProviderError);

impl LlmCompletionProvider for UnavailableProvider {
    fn provider_id(&self) -> &str {
        "clarification-chinese-synthetic-error"
    }
    fn generate(&self, _: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        Vec::new()
    }
    fn generate_checked(
        &self,
        _: &LlmCompletionRequest,
    ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
        Err(self.0.clone())
    }
}

#[test]
fn clarification_candidates_survive_synthetic_provider_failures() {
    // These typed failures do not send network requests or evaluate any model.
    for error in [
        LlmProviderError::NoLocalModel,
        LlmProviderError::Unavailable,
        LlmProviderError::Timeout,
        LlmProviderError::HttpStatus(503),
    ] {
        for (seed, word, sentence) in [
            (
                "shuo ju ti yi dian",
                "说具体一点",
                "说具体一点，我想知道是哪一步。",
            ),
            (
                "wo li jie de shi",
                "我理解的是",
                "我理解的是这个意思，你看看对不对。",
            ),
            (
                "wo que ren yi xi",
                "我确认一下",
                "我确认一下时间，再回复你。",
            ),
            (
                "wo da gai ming bai le",
                "我大概明白了",
                "我大概明白了，再试一遍看看。",
            ),
        ] {
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(UnavailableProvider(error.clone()))));
            ime.seed(seed);
            assert_local_draft(&ime, seed);
            first_page_index(&ime, word, CandidateKind::Word);
            first_page_index(&ime, sentence, CandidateKind::Sentence);
            let before = ime.candidates().to_vec();
            let deadline = Instant::now() + Duration::from_secs(2);
            while ime.prediction_pending() && Instant::now() < deadline {
                ime.poll_prediction();
                std::thread::sleep(Duration::from_millis(2));
            }
            assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
            assert_eq!(ime.prediction_error(), Some(&error));
            assert_eq!(ime.candidates(), before);
            ime.configure_prediction(None);
            assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
            assert_local_draft(&ime, seed);
            commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn clarification_unknown_text_and_boundaries_keep_the_exact_literal_choice() {
    for seed in [
        "zzqvjy",
        "qqzzv  note",
        "“raw”  123\t???",
        "clarify@example.invalid",
        "https://example.invalid/clarify?q=raw",
        "🙂\u{3000}zzqvjy",
        "我理解的是??  zzqvjy",
        "我确认一下\nzzqvjy",
        "我理解的是。\tqqzzv",
    ] {
        let mut ime = engine(seed);
        assert_local_draft(&ime, seed);
        commit_once_and_undo(&mut ime, seed, CandidateKind::Literal);
    }
}
