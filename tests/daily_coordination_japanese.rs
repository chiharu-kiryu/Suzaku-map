//! Fixed Japanese messages and everyday coordination through the offline IME.
//! The authored expectations are independent of production JSON and do not
//! imply general morphology, live communication or model-quality coverage.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

const WORDS: &[(&str, &str, &str)] = &[
    ("renrakusaki", "れんらくさき", "連絡先"),
    ("henshin", "へんしん", "返信"),
    ("dengon", "でんごん", "伝言"),
    ("kakuninzumi", "かくにんずみ", "確認済み"),
    ("junbichuu", "じゅんびちゅう", "準備中"),
    ("kanryou", "かんりょう", "完了"),
    ("saikai", "さいかい", "再開"),
    ("shimekiri", "しめきり", "締め切り"),
    ("uketori", "うけとり", "受け取り"),
    ("soushin", "そうしん", "送信"),
    ("jushin", "じゅしん", "受信"),
    ("kyouyuu", "きょうゆう", "共有"),
    ("tensou", "てんそう", "転送"),
    ("tenpu", "てんぷ", "添付"),
    ("houkoku", "ほうこく", "報告"),
    ("sagyou", "さぎょう", "作業"),
    ("fuzai", "ふざい", "不在"),
    ("taishutsu", "たいしゅつ", "退室"),
    ("nyuushitsu", "にゅうしつ", "入室"),
    ("tantou", "たんとう", "担当"),
    ("yakuwari", "やくわり", "役割"),
    ("buntan", "ぶんたん", "分担"),
    ("hikitsugi", "ひきつぎ", "引き継ぎ"),
    ("nittei", "にってい", "日程"),
    ("shinchoku", "しんちょく", "進捗"),
    ("koushin", "こうしん", "更新"),
    ("mikakunin", "みかくにん", "未確認"),
    ("misoushin", "みそうしん", "未送信"),
    ("yoyaku", "よやく", "予約"),
    ("uketsuke", "うけつけ", "受付"),
    ("chousei", "ちょうせい", "調整"),
    ("goannai", "ごあんない", "ご案内"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "連絡先",
        [
            "連絡先を教えてもらえますか。",
            "連絡先が変わったら教えてください。",
        ],
    ),
    (
        "返信",
        ["返信ありがとうございます。", "返信は後で大丈夫です。"],
    ),
    (
        "伝言",
        ["伝言をお願いできますか。", "伝言を受け取りました。"],
    ),
    (
        "確認済み",
        [
            "確認済みです、ありがとうございます。",
            "確認済みの内容を共有します。",
        ],
    ),
    (
        "準備中",
        [
            "準備中です、少しお待ちください。",
            "準備中なので、終わったら連絡します。",
        ],
    ),
    (
        "完了",
        [
            "完了しました、確認をお願いします。",
            "完了したら連絡してください。",
        ],
    ),
    (
        "再開",
        [
            "再開する時間を教えてください。",
            "再開したら、続きを進めましょう。",
        ],
    ),
    (
        "締め切り",
        ["締め切りはいつですか。", "締め切りまでに確認します。"],
    ),
    (
        "受け取り",
        [
            "受け取りました、ありがとうございます。",
            "受け取りの確認をお願いします。",
        ],
    ),
    (
        "送信",
        ["送信する前に確認します。", "送信が終わったら連絡します。"],
    ),
    (
        "受信",
        [
            "受信できました、ありがとうございます。",
            "受信できたら教えてください。",
        ],
    ),
    (
        "共有",
        ["共有ありがとうございます。", "共有する前に確認しましょう。"],
    ),
    (
        "転送",
        ["転送してもらえますか。", "転送する前に確認します。"],
    ),
    (
        "添付",
        [
            "添付した内容を確認してください。",
            "添付するものをもう一度確認します。",
        ],
    ),
    (
        "報告",
        ["報告ありがとうございます。", "報告は後でまとめて送ります。"],
    ),
    (
        "作業",
        [
            "作業が終わったら連絡します。",
            "作業を分担して進めましょう。",
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

fn existing_index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()))
}

fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    let index = existing_index(ime, text, kind);
    if kind != CandidateKind::Literal {
        assert!(
            index < PAGE_SIZE,
            "not on page one: {text:?}, seed={:?}, index={index}, candidates={:?}",
            ime.snapshot().seed_text,
            ime.candidates()
        );
    }
    index
}

fn commit_once_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let selected = index(ime, text, kind);
    commit_selected_once_and_undo(ime, selected, text);
}

fn commit_any_page_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let selected = existing_index(ime, text, kind);
    commit_selected_once_and_undo(ime, selected, text);
}

fn commit_selected_once_and_undo(ime: &mut XRTabletImeEngine, selected: usize, text: &str) {
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
fn messages_words_support_complete_romaji_case_and_hiragana() {
    assert_eq!(WORDS.len(), 32);
    for &(romaji, kana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
        ] {
            let mut ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            commit_once_and_undo(&mut ime, word, CandidateKind::Word);
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn partial_readings_keep_specific_words_on_page_one_and_ambiguous_words_reachable() {
    for &(romaji, kana, word) in WORDS {
        let last_kana = kana.char_indices().next_back().unwrap().0;
        for seed in [&romaji[..romaji.len() - 1], &kana[..last_kana]] {
            // Most are specific multi-kana prefixes. てん also prefixes the
            // older 天気 and new 転送: preserve their authored priorities
            // rather than claiming every broad prefix fits on page one.
            let mut ime = engine(seed);
            assert_local_draft(&ime, seed);
            if seed == "てん" && word == "添付" {
                commit_any_page_and_undo(&mut ime, word, CandidateKind::Word);
            } else {
                commit_once_and_undo(&mut ime, word, CandidateKind::Word);
            }
            commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn messages_offer_both_authored_sentences_alongside_complete_and_partial_words() {
    assert_eq!(SENTENCES.len(), 16);
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
            index(&ime, word, CandidateKind::Word);
            for sentence in sentences {
                // kyouyu also segments as 今日 + ゆ. The first authored
                // 共有 sentence remains on page one; the second competes
                // with established segment completions on the next page.
                // Complete spellings and all other partial cases stay strict.
                if seed == "kyouyu" && sentence == "共有する前に確認しましょう。" {
                    commit_any_page_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence);
                } else {
                    commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence);
                }
            }
        }
    }
}

#[test]
fn ambiguous_partial_readings_preserve_priority_and_exact_later_page_choices() {
    let ten = engine("てん");
    assert_local_draft(&ten, "てん");
    let weather = index(&ten, "天気", CandidateKind::Word);
    let forward = index(&ten, "転送", CandidateKind::Word);
    let attach = existing_index(&ten, "添付", CandidateKind::Word);
    assert!(weather < forward && forward < attach);
    assert!((PAGE_SIZE..12).contains(&attach));
    index(&ten, "天気がいいですね。", CandidateKind::Sentence);
    index(&ten, "天気はどうですか？", CandidateKind::Sentence);
    index(&ten, "てん", CandidateKind::Literal);

    let kyouyu = engine("kyouyu");
    assert_local_draft(&kyouyu, "kyouyu");
    assert_eq!(kyouyu.candidates()[0].text, "今日ゆ");
    index(&kyouyu, "共有", CandidateKind::Word);
    index(
        &kyouyu,
        "共有ありがとうございます。",
        CandidateKind::Sentence,
    );
    let second = existing_index(
        &kyouyu,
        "共有する前に確認しましょう。",
        CandidateKind::Sentence,
    );
    assert!((PAGE_SIZE..12).contains(&second));
    index(&kyouyu, "kyouyu", CandidateKind::Literal);

    for (seed, text, kind) in [
        ("てん", "添付", CandidateKind::Word),
        (
            "kyouyu",
            "共有する前に確認しましょう。",
            CandidateKind::Sentence,
        ),
    ] {
        let mut ime = engine(seed);
        let selected = existing_index(&ime, text, kind);
        assert!((PAGE_SIZE..12).contains(&selected));
        ime.select_candidate(selected);
        let adopted = ime.selected_completion_text(true).unwrap().to_owned();
        assert_eq!(adopted, text);
        assert!(ime.snapshot().committed_text.is_empty());
        // Host-style editable adoption and restoration, not physical keys or
        // a claim that Engine::undo handles host composition cancellation.
        ime.seed(&adopted);
        assert_local_draft(&ime, text);
        ime.seed(seed);
        assert_local_draft(&ime, seed);
        assert!((PAGE_SIZE..12).contains(&existing_index(&ime, text, kind)));
        commit_any_page_and_undo(&mut ime, text, kind);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn horizontal_reading_separators_preserve_raw_input_and_first_page_messages() {
    for (seed, word, sentence) in [
        ("RENRAKUSAKI", "連絡先", "連絡先を教えてもらえますか。"),
        ("shi me kiri", "締め切り", "締め切りはいつですか。"),
        ("  hen shin  ", "返信", "返信ありがとうございます。"),
        ("でん ごん", "伝言", "伝言をお願いできますか。"),
        (
            "\tkan\tryou\u{3000}",
            "完了",
            "完了しました、確認をお願いします。",
        ),
        ("kyo\u{3000}uyuu", "共有", "共有ありがとうございます。"),
        ("ten  pu", "添付", "添付した内容を確認してください。"),
        ("den'gon", "伝言", "伝言を受け取りました。"),
        ("DEN’GON", "伝言", "伝言をお願いできますか。"),
        ("junbi chuu", "準備中", "準備中です、少しお待ちください。"),
    ] {
        let ime = engine(seed);
        assert_local_draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn adopted_coordination_words_accept_particle_progress_without_implicit_commit() {
    for (reading, word, continued, sentence) in [
        (
            "RENRAKUSAKI",
            "連絡先",
            "連絡先wo",
            "連絡先を教えてもらえますか。",
        ),
        ("henshin", "返信", "返信 ha", "返信は後で大丈夫です。"),
        ("dengon", "伝言", "伝言を", "伝言をお願いできますか。"),
        (
            "kanryou",
            "完了",
            "完了shitara",
            "完了したら連絡してください。",
        ),
        (
            "shimekiri",
            "締め切り",
            "締め切りは",
            "締め切りはいつですか。",
        ),
        (
            "kyouyuu",
            "共有",
            "共有suru",
            "共有する前に確認しましょう。",
        ),
        ("houkoku", "報告", "報告 HA", "報告は後でまとめて送ります。"),
        ("sagyou", "作業", "作業ga", "作業が終わったら連絡します。"),
    ] {
        let mut ime = engine(reading);
        assert_local_draft(&ime, reading);
        let selected = index(&ime, word, CandidateKind::Word);
        ime.select_candidate(selected);
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // Engine-level editable host draft replacement, not desktop key
        // injection or an assertion about a private IBus event chain.
        ime.seed(word);
        assert_local_draft(&ime, word);
        ime.seed(continued);
        assert_local_draft(&ime, continued);
        commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(continued), continued, CandidateKind::Literal);
    }
}

#[test]
fn messages_never_drop_unknown_text_or_join_line_boundaries() {
    for seed in [
        "xyzrenrakusaki",
        "🙂連絡先を",
        "前置き完了したら",
        "https://henshin",
        "伝言woxyz",
        "締め切りはxyz",
        "共有をが",
        "報告は🙂",
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
        for (head, tail) in [("連絡先", "wo"), ("kanryou", "shitara"), ("hen", "shin")] {
            let seed = format!("{head}{separator}{tail}");
            let mut ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            for candidate in ime.candidates() {
                assert!(
                    candidate.text.contains(separator),
                    "lost line boundary: {candidate:?}"
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
