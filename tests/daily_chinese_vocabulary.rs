//! Finite, project-authored everyday phrases, not a general Chinese benchmark.
//! Expectations are independent of the resource file and require no model or
//! personal history. Native numeric adoption is covered by the IBus/GTK tests.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

const WORDS: &[(&str, &str)] = &[
    ("bu'cuo", "不错"),
    ("zhen'bu'cuo", "真不错"),
    ("ting'hao", "挺好"),
    ("hen'hao", "很好"),
    ("tai'hao'le", "太好了"),
    ("tai'bang'le", "太棒了"),
    ("hao'li'hai", "好厉害"),
    ("zhen'li'hai", "真厉害"),
    ("zhen'bang", "真棒"),
    ("hen'bang", "很棒"),
    ("hao'bang", "好棒"),
    ("kao'pu", "靠谱"),
    ("bu'kao'pu", "不靠谱"),
    ("you'yi'si", "有意思"),
    ("mei'yi'si", "没意思"),
    ("zhen'you'qu", "真有趣"),
    ("ting'you'qu", "挺有趣"),
    ("hao'kai'xin", "好开心"),
    ("hen'kai'xin", "很开心"),
    ("you'dian'lei", "有点累"),
    ("you'dian'kun", "有点困"),
    ("bu'shu'fu", "不舒服"),
    ("hao'duo'le", "好多了"),
    ("mei'shi'ba", "没事吧"),
    ("mei'shi'de", "没事的"),
    ("xin'ku'la", "辛苦啦"),
    ("ma'fan'le", "麻烦了"),
    ("ma'fan'ni'le", "麻烦你了"),
    ("xie'xie'la", "谢谢啦"),
    ("hao'ya", "好呀"),
    ("hao'lei", "好嘞"),
    ("xing'ba", "行吧"),
    ("ke'yi'ya", "可以呀"),
    ("liao'jie'le", "了解了"),
    ("ye'shi", "也是"),
    ("jiu'shi", "就是"),
    ("sui'bian", "随便"),
    ("zai'kan'kan", "再看看"),
    ("wo'kan'kan", "我看看"),
    ("ma'shang'lai", "马上来"),
    ("dao'jia'le", "到家了"),
    ("chu'fa'le", "出发了"),
    ("dao'na'li'le", "到哪里了"),
    ("ji'dian'jian", "几点见"),
    ("dai'hui'jian", "待会见"),
    ("wan'dian'liao", "晚点聊"),
    ("xia'ci'ba", "下次吧"),
    ("gai'tian'ba", "改天吧"),
    ("chi'fan'le'ma", "吃饭了吗"),
    ("chi'hao'le", "吃好了"),
    ("e'le", "饿了"),
    ("hao'e", "好饿"),
    ("ke'le", "渴了"),
    ("hao'kun", "好困"),
    ("xiu'xi'yi'xia", "休息一下"),
    ("zao'dian'shui", "早点睡"),
    ("shui'xing'le", "睡醒了"),
    ("qi'chuang'le", "起床了"),
    ("qu'san'bu", "去散步"),
    ("chu'qu'zou'zou", "出去走走"),
    ("he'dian'shui", "喝点水"),
    ("ji'de'xiu'xi", "记得休息"),
    ("zhu'yi'shen'ti", "注意身体"),
    ("hao'hao'xiu'xi", "好好休息"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    ("真不错", ["真不错，下次还想再来。", "真不错，值得试一试。"]),
    (
        "太好了",
        ["太好了，终于等到你了。", "太好了，我们一起去吧。"],
    ),
    ("太棒了", ["太棒了，这次很顺利。", "太棒了，继续保持！"]),
    ("靠谱", ["靠谱的话就试试看。", "靠谱的人让人放心。"]),
    (
        "有意思",
        ["有意思，下次再聊聊。", "有意思，我想多了解一点。"],
    ),
    (
        "很开心",
        ["很开心能和你一起出来。", "很开心，今天过得很充实。"],
    ),
    ("有点累", ["有点累，我先休息一下。", "有点累，晚点再聊吧。"]),
    ("有点困", ["有点困，我先去睡了。", "有点困，想休息一会儿。"]),
    (
        "不舒服",
        ["不舒服的话就先休息。", "不舒服，今天就不出门了。"],
    ),
    ("好多了", ["好多了，谢谢你的关心。", "好多了，不用担心我。"]),
    ("没事的", ["没事的，不用放在心上。", "没事的，我们慢慢来。"]),
    (
        "麻烦你了",
        ["麻烦你了，谢谢帮忙。", "麻烦你了，有空请你吃饭。"],
    ),
    ("谢谢啦", ["谢谢啦，帮了我大忙。", "谢谢啦，下次我来帮你。"]),
    ("好呀", ["好呀，那就这么说定了。", "好呀，我们一起去。"]),
    (
        "了解了",
        ["了解了，我再确认一下。", "了解了，谢谢你告诉我。"],
    ),
    (
        "我看看",
        ["我看看时间，再回复你。", "我看看有没有其他办法。"],
    ),
    (
        "马上来",
        ["马上来，请稍等一下。", "马上来，我拿好东西就走。"],
    ),
    ("到家了", ["到家了，给你报个平安。", "到家了，今天辛苦啦。"]),
    (
        "出发了",
        ["出发了，到了再联系你。", "出发了，路上大概半小时。"],
    ),
    ("几点见", ["几点见比较方便？", "几点见，我们提前约好吧。"]),
    (
        "晚点聊",
        ["晚点聊，我先忙一会儿。", "晚点聊，等你有空再说。"],
    ),
    (
        "吃饭了吗",
        ["吃饭了吗，要不要一起吃？", "吃饭了吗，别忙得忘了时间。"],
    ),
    (
        "休息一下",
        ["休息一下，等会儿再继续。", "休息一下，出去走走吧。"],
    ),
    ("早点睡", ["早点睡，明天还要早起。", "早点睡，晚安啦。"]),
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

fn reading_for(word: &str) -> &'static str {
    WORDS.iter().find(|(_, text)| *text == word).unwrap().0
}

fn spellings(reading: &str) -> [String; 4] {
    [
        reading.replace('\'', ""),
        reading.replace('\'', " "),
        reading.to_owned(),
        reading.replace('\'', " ").to_ascii_uppercase(),
    ]
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
    assert_eq!(
        ime.commit(CommitOptions { force: true }).text.as_deref(),
        Some(text)
    );
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let second = ime.commit(CommitOptions { force: true });
    assert!(!second.ok);
    assert_eq!(second.text, None);
    assert_eq!(ime.snapshot().committed_text, text);
}

#[test]
fn every_daily_word_converts_all_four_spellings_and_commits_without_a_model() {
    assert_eq!(WORDS.len(), 64);
    for &(reading, word) in WORDS {
        for seed in spellings(reading) {
            let mut ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            assert_eq!(ime.candidates()[0].text, word, "{seed:?}");
            commit_once(&mut ime, word, CandidateKind::Word);
        }
    }
}

#[test]
fn every_daily_context_keeps_words_and_both_sentences_on_the_first_page() {
    assert_eq!(SENTENCES.len(), 24);
    for &(word, sentences) in SENTENCES {
        for seed in spellings(reading_for(word)) {
            let ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            first_page_index(&ime, word, CandidateKind::Word);
            for sentence in sentences {
                first_page_index(&ime, sentence, CandidateKind::Sentence);
                commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
            }
        }
    }
}

#[test]
fn daily_contexts_complete_an_unfinished_final_syllable() {
    for &(word, sentences) in SENTENCES {
        let reading = reading_for(word);
        // All selected final syllables contain at least two ASCII letters.
        // Keep their onset and every explicit boundary; do not use initials.
        let partial = &reading[..reading.len() - 1];
        for seed in spellings(partial) {
            let ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            first_page_index(&ime, word, CandidateKind::Word);
            // Ambiguous partial readings retain their older decoded choices;
            // the first page must still offer both a word and a sentence.
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
fn adopted_daily_words_preserve_literal_padding_and_exact_sentence_commits() {
    for &(word, sentences) in SENTENCES {
        for padding in ["", " ", "  ", "\u{3000}"] {
            for sentence in sentences {
                let mut ime = engine(reading_for(word));
                let index = first_page_index(&ime, word, CandidateKind::Word);
                ime.select_candidate(index);
                let adopted = ime.selected_completion_text(true).unwrap().to_owned();
                assert_eq!(adopted, word);
                assert!(ime.snapshot().committed_text.is_empty());
                // The native host adopts a candidate into an editable draft.
                // Model that draft transition without committing the word.
                let seed = format!("{adopted}{padding}");
                ime.seed(&seed);
                assert_local_draft(&ime, &seed);
                assert_eq!(ime.candidates()[0].text, seed);
                let suffix = sentence.strip_prefix(word).unwrap();
                let expected = format!("{seed}{suffix}");
                commit_once(&mut ime, &expected, CandidateKind::Sentence);
            }
        }
    }
}
