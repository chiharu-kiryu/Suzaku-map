//! Fixed Japanese comparisons, preferences, conditions and alternatives.
//! Authored expectations are independent of production JSON. These checks
//! exercise a bounded offline dictionary, not general morphological analysis.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

const WORDS: &[(&str, &str, &str)] = &[
    ("konomi", "このみ", "好み"),
    ("kibou", "きぼう", "希望"),
    ("jouken", "じょうけん", "条件"),
    ("yosan", "よさん", "予算"),
    ("sentaku", "せんたく", "選択"),
    ("hikaku", "ひかく", "比較"),
    ("riyuu", "りゆう", "理由"),
    ("chigai", "ちがい", "違い"),
    ("onaji", "おなじ", "同じ"),
    ("betsuno", "べつの", "別の"),
    ("dochira", "どちら", "どちら"),
    ("dochirademo", "どちらでも", "どちらでも"),
    ("kochira", "こちら", "こちら"),
    ("sochira", "そちら", "そちら"),
    ("osusume", "おすすめ", "おすすめ"),
    ("dekireba", "できれば", "できれば"),
    ("moshiyokereba", "もしよければ", "もしよければ"),
    ("sonokawari", "そのかわり", "その代わり"),
    ("nennotame", "ねんのため", "念のため"),
    ("murinara", "むりなら", "無理なら"),
    ("tsugou", "つごう", "都合"),
    ("benri", "べんり", "便利"),
    ("kantan", "かんたん", "簡単"),
    ("anshin", "あんしん", "安心"),
    ("takai", "たかい", "高い"),
    ("yasui", "やすい", "安い"),
    ("ookii", "おおきい", "大きい"),
    ("chiisai", "ちいさい", "小さい"),
    ("chikai", "ちかい", "近い"),
    ("tooi", "とおい", "遠い"),
    ("hayai", "はやい", "早い"),
    ("osoi", "おそい", "遅い"),
    ("atarashii", "あたらしい", "新しい"),
    ("furui", "ふるい", "古い"),
    ("karui", "かるい", "軽い"),
    ("omoi", "おもい", "重い"),
    ("ooi", "おおい", "多い"),
    ("sukunai", "すくない", "少ない"),
    ("hitsuyou", "ひつよう", "必要"),
    ("fuyou", "ふよう", "不要"),
    ("yuusen", "ゆうせん", "優先"),
    ("henkou", "へんこう", "変更"),
    ("daian", "だいあん", "代案"),
    ("ryouhou", "りょうほう", "両方"),
    ("katahou", "かたほう", "片方"),
    ("kimeru", "きめる", "決める"),
    ("erabu", "えらぶ", "選ぶ"),
    ("mayou", "まよう", "迷う"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "好み",
        ["好みに合わせて選んでください。", "好みは人それぞれですね。"],
    ),
    (
        "希望",
        [
            "希望があれば教えてください。",
            "希望に近いものを探しましょう。",
        ],
    ),
    (
        "条件",
        [
            "条件が合えば、こちらを選びます。",
            "条件をもう一度確認したいです。",
        ],
    ),
    (
        "予算",
        ["予算に合わせて選びましょう。", "予算を少し抑えたいです。"],
    ),
    (
        "選択",
        [
            "選択に迷ったら相談してください。",
            "選択する前に比べてみましょう。",
        ],
    ),
    (
        "比較",
        [
            "比較してから決めたいです。",
            "比較すると、こちらの方が便利です。",
        ],
    ),
    (
        "理由",
        ["理由を教えてもらえますか。", "理由が分かれば安心できます。"],
    ),
    (
        "違い",
        ["違いを教えてもらえますか。", "違いがよく分かりません。"],
    ),
    (
        "同じ",
        ["同じものでお願いします。", "同じ条件で比べてみましょう。"],
    ),
    (
        "別の",
        ["別の方法も試してみましょう。", "別の日でも大丈夫です。"],
    ),
    (
        "どちら",
        ["どちらがおすすめですか。", "どちらの方が使いやすいですか。"],
    ),
    (
        "どちらでも",
        [
            "どちらでも大丈夫です。",
            "どちらでも好きな方を選んでください。",
        ],
    ),
    (
        "こちら",
        ["こちらの方が使いやすいです。", "こちらにしてもいいですか。"],
    ),
    (
        "そちら",
        ["そちらの方が便利そうですね。", "そちらに合わせます。"],
    ),
    (
        "おすすめ",
        [
            "おすすめを教えてください。",
            "おすすめの中から選びたいです。",
        ],
    ),
    (
        "できれば",
        [
            "できれば、もう少し早い時間がいいです。",
            "できれば、簡単な方法にしたいです。",
        ],
    ),
    (
        "もしよければ",
        [
            "もしよければ、一緒に選んでもらえますか。",
            "もしよければ、別の日にしませんか。",
        ],
    ),
    (
        "その代わり",
        [
            "その代わり、明日は早めに始めましょう。",
            "その代わり、別の方法を考えましょう。",
        ],
    ),
    (
        "念のため",
        [
            "念のため、もう一度確認しましょう。",
            "念のため、別の案も用意しておきます。",
        ],
    ),
    (
        "無理なら",
        [
            "無理なら、別の日にしましょう。",
            "無理なら、後でも大丈夫です。",
        ],
    ),
    (
        "都合",
        [
            "都合がよければ、明日にしませんか。",
            "都合に合わせて変更できます。",
        ],
    ),
    (
        "便利",
        ["便利な方を選びましょう。", "便利ですが、少し高いですね。"],
    ),
    (
        "簡単",
        [
            "簡単な方法から試しましょう。",
            "簡単に使えるものがいいです。",
        ],
    ),
    (
        "安心",
        [
            "安心して使えるものを選びたいです。",
            "安心できる方にしましょう。",
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
            .all(|candidate| { candidate.source == CandidateSource::Local })
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

fn word_kind(seed: &str, word: &str) -> CandidateKind {
    if seed == word {
        // A kana spelling already entered verbatim is the literal choice.
        CandidateKind::Literal
    } else {
        CandidateKind::Word
    }
}

fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    let index = ime
        .candidates()
        .iter()
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()));
    // Raw spelling remains reachable even if useful completions fill page one.
    if kind != CandidateKind::Literal {
        assert!(index < PAGE_SIZE, "not on page one: {text:?}");
    }
    index
}

fn commit_once_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let selected = index(ime, text, kind);
    ime.select_candidate(selected);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let second = ime.commit(CommitOptions { force: true });
    assert!(!second.ok);
    assert!(second.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn choices_words_support_complete_romaji_case_and_hiragana() {
    assert_eq!(WORDS.len(), 48);
    for &(romaji, kana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
        ] {
            let mut ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            commit_once_and_undo(&mut ime, word, word_kind(&seed, word));
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn specific_partial_readings_keep_choices_words_on_page_one() {
    for &(romaji, kana, word) in WORDS {
        let last_kana = kana.char_indices().next_back().unwrap().0;
        for seed in [&romaji[..romaji.len() - 1], &kana[..last_kana]] {
            // These authored examples have specific multi-kana prefixes.
            // Do not reorder old entries or require every broad prefix to work.
            let mut ime = engine(seed);
            assert_local_draft(&ime, seed);
            commit_once_and_undo(&mut ime, word, CandidateKind::Word);
            commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn choices_offer_both_authored_sentences_alongside_complete_and_partial_words() {
    assert_eq!(SENTENCES.len(), 24);
    for &(word, sentences) in SENTENCES {
        let &(romaji, kana, _) = WORDS.iter().find(|(_, _, text)| *text == word).unwrap();
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
            romaji[..romaji.len() - 1].to_owned(),
        ] {
            let ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            index(&ime, word, word_kind(&seed, word));
            for sentence in sentences {
                commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence);
            }
        }
    }
}

#[test]
fn horizontal_reading_separators_preserve_raw_input_and_first_page_choices() {
    for (seed, word, sentence) in [
        ("KONOMI", "好み", "好みに合わせて選んでください。"),
        ("jouke", "条件", "条件が合えば、こちらを選びます。"),
        ("ひかく", "比較", "比較してから決めたいです。"),
        ("yo  san", "予算", "予算に合わせて選びましょう。"),
        ("  ko no mi  ", "好み", "好みに合わせて選んでください。"),
        (
            "\tjou\tken\u{3000}",
            "条件",
            "条件が合えば、こちらを選びます。",
        ),
        ("hi\u{3000}kaku", "比較", "比較してから決めたいです。"),
        ("よ さん", "予算", "予算に合わせて選びましょう。"),
        (
            "nen'notame",
            "念のため",
            "念のため、もう一度確認しましょう。",
        ),
        (
            "NEN’NOTAME",
            "念のため",
            "念のため、もう一度確認しましょう。",
        ),
    ] {
        let ime = engine(seed);
        assert_local_draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn adopted_choices_words_accept_particle_progress_without_implicit_commit() {
    for (reading, word, continued, sentence) in [
        ("KONOMI", "好み", "好みni", "好みに合わせて選んでください。"),
        (
            "jouke",
            "条件",
            "条件ga",
            "条件が合えば、こちらを選びます。",
        ),
        ("yosan", "予算", "予算 ni", "予算に合わせて選びましょう。"),
        ("kibou", "希望", "希望が", "希望があれば教えてください。"),
        ("riyuu", "理由", "理由wo", "理由を教えてもらえますか。"),
        (
            "tsugou",
            "都合",
            "都合 ga",
            "都合がよければ、明日にしませんか。",
        ),
    ] {
        let mut ime = engine(reading);
        assert_local_draft(&ime, reading);
        let selected = index(&ime, word, CandidateKind::Word);
        ime.select_candidate(selected);
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // Engine-level editable host draft replacement; private IBus key
        // events are tested separately. No new GTK test is claimed here.
        ime.seed(word);
        assert_local_draft(&ime, word);
        ime.seed(continued);
        assert_local_draft(&ime, continued);
        commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(continued), continued, CandidateKind::Literal);
    }
}

#[test]
fn choices_never_drop_unknown_text_or_join_line_boundaries() {
    for seed in [
        "xyzjouken",
        "🙂好みに",
        "前置き条件が",
        "https://yosan",
        "条件gaxyz",
        "好みにxyz",
        "条件をが",
        "予算に🙂",
    ] {
        let mut ime = engine(seed);
        assert_local_draft(&ime, seed);
        for candidate in ime.candidates() {
            for &(_, sentences) in SENTENCES {
                for sentence in sentences {
                    assert!(
                        !candidate.text.ends_with(sentence),
                        "{seed:?} must not manufacture {sentence:?}: {:?}",
                        ime.candidates()
                    );
                }
            }
        }
        commit_once_and_undo(&mut ime, seed, CandidateKind::Literal);
    }

    for separator in [
        "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (head, tail) in [("jouken", "ga"), ("好み", "ni"), ("yo", "san")] {
            let seed = format!("{head}{separator}{tail}");
            let mut ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            for candidate in ime.candidates() {
                assert!(
                    candidate.text.contains(separator),
                    "lost line boundary in {seed:?}: {candidate:?}"
                );
                for &(_, sentences) in SENTENCES {
                    for sentence in sentences {
                        assert!(!candidate.text.ends_with(sentence));
                    }
                }
            }
            commit_once_and_undo(&mut ime, &seed, CandidateKind::Literal);
        }
    }
}
