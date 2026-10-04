//! Authored everyday Japanese phrases through the bounded offline converter.
//! These fixed expectations do not imply morphological analysis or full IME
//! coverage, and do not use a live model, learned preferences or the desktop.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

const WORDS: &[(&str, &str, &str)] = &[
    ("sumimasen", "すみません", "すみません"),
    ("onegaishimasu", "おねがいします", "お願いします"),
    ("daijoubu", "だいじょうぶ", "大丈夫"),
    ("wakarimashita", "わかりました", "分かりました"),
    ("wakarimasen", "わかりません", "分かりません"),
    ("mouichido", "もういちど", "もう一度"),
    ("yukkuri", "ゆっくり", "ゆっくり"),
    ("mattekudasai", "まってください", "待ってください"),
    ("tasukarimashita", "たすかりました", "助かりました"),
    ("shitsureishimasu", "しつれいします", "失礼します"),
    ("otsukaresama", "おつかれさま", "お疲れさま"),
    ("mataashita", "またあした", "また明日"),
    ("hajimemashite", "はじめまして", "はじめまして"),
    ("yoroshiku", "よろしく", "よろしく"),
    ("ohisashiburi", "おひさしぶり", "お久しぶり"),
    ("ogenkidesuka", "おげんきですか", "お元気ですか"),
    ("oyasumi", "おやすみ", "おやすみ"),
    ("ittekimasu", "いってきます", "行ってきます"),
    ("itterasshai", "いってらっしゃい", "いってらっしゃい"),
    ("tadaima", "ただいま", "ただいま"),
    ("okaeri", "おかえり", "おかえり"),
    ("itadakimasu", "いただきます", "いただきます"),
    ("gochisousama", "ごちそうさま", "ごちそうさま"),
    ("tanoshimi", "たのしみ", "楽しみ"),
    ("yotei", "よてい", "予定"),
    ("yakusoku", "やくそく", "約束"),
    ("shuumatsu", "しゅうまつ", "週末"),
    ("raishuu", "らいしゅう", "来週"),
    ("asa", "あさ", "朝"),
    ("hiru", "ひる", "昼"),
    ("yoru", "よる", "夜"),
    ("atode", "あとで", "後で"),
    ("imakara", "いまから", "今から"),
    ("sukoshi", "すこし", "少し"),
    ("isshoni", "いっしょに", "一緒に"),
    ("kyuukei", "きゅうけい", "休憩"),
    ("denwa", "でんわ", "電話"),
    ("renraku", "れんらく", "連絡"),
    ("kakunin", "かくにん", "確認"),
    ("soudan", "そうだん", "相談"),
    ("kaimono", "かいもの", "買い物"),
    ("hirugohan", "ひるごはん", "昼ご飯"),
    ("bangohan", "ばんごはん", "晩ご飯"),
    ("kaigi", "かいぎ", "会議"),
    ("junbi", "じゅんび", "準備"),
    ("okuremasu", "おくれます", "遅れます"),
    ("tsukimashita", "つきました", "着きました"),
    ("owarimashita", "おわりました", "終わりました"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "すみません",
        [
            "すみません、もう一度お願いします。",
            "すみません、少し待ってください。",
        ],
    ),
    (
        "お願いします",
        [
            "お願いします。終わったら教えてください。",
            "お願いします。準備ができたら連絡してください。",
        ],
    ),
    (
        "大丈夫",
        [
            "大丈夫です、ありがとうございます。",
            "大丈夫です、ゆっくりでいいですよ。",
        ],
    ),
    (
        "分かりました",
        [
            "分かりました、確認しておきます。",
            "分かりました、後で連絡します。",
        ],
    ),
    (
        "分かりません",
        [
            "分かりません、もう一度教えてください。",
            "分かりません、一緒に確認してもらえますか。",
        ],
    ),
    (
        "もう一度",
        ["もう一度お願いします。", "もう一度確認してみます。"],
    ),
    (
        "準備",
        ["準備ができたら連絡します。", "準備を手伝ってもらえますか。"],
    ),
    (
        "待ってください",
        [
            "待ってください、今確認しています。",
            "待ってください、すぐに戻ります。",
        ],
    ),
    (
        "助かりました",
        [
            "助かりました、ありがとうございます。",
            "助かりました、またよろしくお願いします。",
        ],
    ),
    (
        "お疲れさま",
        [
            "お疲れさまです、今日はありがとうございました。",
            "お疲れさまです、ゆっくり休んでください。",
        ],
    ),
    (
        "よろしく",
        ["よろしくお願いします。", "よろしくお伝えください。"],
    ),
    (
        "おやすみ",
        [
            "おやすみなさい、また明日。",
            "おやすみなさい、ゆっくり休んでね。",
        ],
    ),
    (
        "楽しみ",
        [
            "楽しみにしています。",
            "楽しみですね、当日よろしくお願いします。",
        ],
    ),
    (
        "予定",
        [
            "予定が決まったら連絡します。",
            "予定を確認してから返事します。",
        ],
    ),
    (
        "約束",
        [
            "約束の時間を確認してもいいですか。",
            "約束どおり、明日伺います。",
        ],
    ),
    (
        "週末",
        ["週末は何をする予定ですか。", "週末に一緒に出かけませんか。"],
    ),
    ("後で", ["後で連絡します。", "後で一緒に確認しましょう。"]),
    (
        "一緒に",
        ["一緒に昼ご飯を食べませんか。", "一緒に考えてみましょう。"],
    ),
    (
        "休憩",
        [
            "休憩しましょう、少し疲れましたね。",
            "休憩が終わったら再開します。",
        ],
    ),
    (
        "連絡",
        ["連絡ありがとうございます。", "連絡が遅くなってすみません。"],
    ),
    ("確認", ["確認してから連絡します。", "確認をお願いします。"]),
    (
        "買い物",
        [
            "買い物に行ってきます。",
            "買い物のついでに何か買ってきましょうか。",
        ],
    ),
    (
        "遅れます",
        [
            "遅れます、先に始めていてください。",
            "遅れます、着いたら連絡します。",
        ],
    ),
    (
        "着きました",
        [
            "着きました、入り口で待っています。",
            "着きました、今からそちらに向かいます。",
        ],
    ),
];

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "ja".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn spellings(word: &str) -> [String; 3] {
    let &(romaji, kana, _) = WORDS.iter().find(|(_, _, text)| *text == word).unwrap();
    [romaji.into(), romaji.to_ascii_uppercase(), kana.into()]
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

fn word_kind(seed: &str, word: &str) -> CandidateKind {
    // A kana word already typed verbatim is the literal choice, not a second
    // duplicate candidate. Romanized or kanji conversions remain typed words.
    if seed == word {
        CandidateKind::Literal
    } else {
        CandidateKind::Word
    }
}

fn commit_once_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    // Short prefixes can fill page one with useful completions. The raw
    // spelling must remain selectable, but need not occupy the first page.
    if kind != CandidateKind::Literal {
        first_page_index(ime, text, kind);
    }
    commit_any_page_and_undo(ime, text, kind);
}

fn commit_any_page_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let index = ime
        .candidates()
        .iter()
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()));
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
fn daily_japanese_words_support_romaji_case_and_hiragana_on_page_one() {
    assert_eq!(WORDS.len(), 48);
    for &(_, _, word) in WORDS {
        for seed in spellings(word) {
            let mut ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            commit_once_and_undo(&mut ime, word, word_kind(&seed, word));
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn unfinished_daily_readings_keep_specific_words_on_page_one_and_broad_kana_reachable() {
    for &(romaji, kana, word) in WORDS {
        let last_kana = kana.char_indices().next_back().unwrap().0;
        for (seed, is_kana) in [
            (&romaji[..romaji.len() - 1], false),
            (&kana[..last_kana], true),
        ] {
            let mut ime = engine(seed);
            assert_local_draft(&ime, seed);
            if is_kana && seed.chars().count() == 1 {
                // A one-kana prefix spans several unrelated words. Preserve
                // established priorities while requiring bounded reachability.
                commit_any_page_and_undo(&mut ime, word, CandidateKind::Word);
            } else {
                commit_once_and_undo(&mut ime, word, CandidateKind::Word);
            }
            commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn complete_daily_readings_offer_both_authored_sentences_on_page_one() {
    assert_eq!(SENTENCES.len(), 24);
    for &(word, sentences) in SENTENCES {
        for seed in spellings(word) {
            let ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            first_page_index(&ime, word, word_kind(&seed, word));
            for sentence in sentences {
                commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence);
            }
        }
    }
}

#[test]
fn unfinished_daily_romaji_keeps_both_authored_sentence_choices() {
    for &(word, sentences) in SENTENCES {
        let &(romaji, _, _) = WORDS.iter().find(|(_, _, text)| *text == word).unwrap();
        let seed = &romaji[..romaji.len() - 1];
        let ime = engine(seed);
        assert_local_draft(&ime, seed);
        first_page_index(&ime, word, CandidateKind::Word);
        for sentence in sentences {
            commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn adopted_daily_words_remain_editable_and_offer_exact_sentence_commits() {
    for &(word, sentences) in SENTENCES {
        let seed = spellings(word)[0].clone();
        let mut ime = engine(&seed);
        let index = first_page_index(&ime, word, CandidateKind::Word);
        ime.select_candidate(index);
        let adopted = ime.selected_completion_text(true).unwrap().to_owned();
        assert_eq!(adopted, word);
        assert!(ime.snapshot().committed_text.is_empty());
        // Model an editable host draft update, not a physical Space event.
        ime.seed(&adopted);
        assert_local_draft(&ime, &adopted);
        for sentence in sentences {
            commit_once_and_undo(&mut engine(&adopted), sentence, CandidateKind::Sentence);
        }
        commit_once_and_undo(&mut ime, &adopted, CandidateKind::Literal);
    }
}
