//! Independent everyday Japanese leisure vocabulary and bounded continuations.
//! These authored fixtures do not read production JSON, call a model, infer an
//! itinerary or claim general Japanese morphological analysis.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

const WORDS: &[(&str, &str, &str)] = &[
    ("sanpo", "さんぽ", "散歩"),
    ("dokusho", "どくしょ", "読書"),
    ("ongaku", "おんがく", "音楽"),
    ("eiga", "えいが", "映画"),
    ("shashin", "しゃしん", "写真"),
    ("ryouri", "りょうり", "料理"),
    ("shumi", "しゅみ", "趣味"),
    ("kouen", "こうえん", "公園"),
    ("bijutsukan", "びじゅつかん", "美術館"),
    ("toshokan", "としょかん", "図書館"),
    ("kyuujitsu", "きゅうじつ", "休日"),
    ("hanami", "はなみ", "花見"),
    ("kanshou", "かんしょう", "鑑賞"),
    ("higaeri", "ひがえり", "日帰り"),
    ("undou", "うんどう", "運動"),
    ("suiei", "すいえい", "水泳"),
    ("jitensha", "じてんしゃ", "自転車"),
    ("odori", "おどり", "踊り"),
    ("ensou", "えんそう", "演奏"),
    ("uta", "うた", "歌"),
    ("keshiki", "けしき", "景色"),
    ("sora", "そら", "空"),
    ("shokubutsu", "しょくぶつ", "植物"),
    ("heya", "へや", "部屋"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "散歩",
        ["散歩に行きませんか。", "散歩しながら話しましょう。"],
    ),
    (
        "読書",
        ["読書をして過ごしたいです。", "読書の時間を作りたいです。"],
    ),
    (
        "音楽",
        [
            "音楽を聴いてゆっくりしたいです。",
            "音楽のおすすめを教えてください。",
        ],
    ),
    (
        "映画",
        [
            "映画を一緒に見ませんか。",
            "映画を見たら感想を聞かせてください。",
        ],
    ),
    (
        "写真",
        ["写真を撮ってもいいですか。", "写真を後で送ります。"],
    ),
    (
        "料理",
        [
            "料理を一緒に作りませんか。",
            "料理ができたら呼んでください。",
        ],
    ),
    ("趣味", ["趣味は何ですか。", "趣味の話を聞かせてください。"]),
    (
        "公園",
        ["公園で少し休みませんか。", "公園まで歩いてみましょう。"],
    ),
    (
        "美術館",
        [
            "美術館に行ってみたいです。",
            "美術館で待ち合わせしませんか。",
        ],
    ),
    (
        "図書館",
        ["図書館で本を探したいです。", "図書館に寄ってから帰ります。"],
    ),
    (
        "休日",
        [
            "休日はゆっくり過ごしたいです。",
            "休日に何をするか決めましょう。",
        ],
    ),
    (
        "花見",
        ["花見に行きませんか。", "花見の予定を相談しましょう。"],
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
        .unwrap_or_else(|| {
            panic!(
                "missing {kind:?} {text:?}, seed={:?}: {:?}",
                ime.snapshot().seed_text,
                ime.candidates()
            )
        })
}

fn page_one_index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    let index = existing_index(ime, text, kind);
    assert!(
        index < PAGE_SIZE,
        "not on page one: {text:?}, seed={:?}, index={index}, candidates={:?}",
        ime.snapshot().seed_text,
        ime.candidates()
    );
    index
}

fn commit_once_and_undo(
    ime: &mut XRTabletImeEngine,
    text: &str,
    kind: CandidateKind,
    on_page_one: bool,
) {
    let index = if on_page_one {
        page_one_index(ime, text, kind)
    } else {
        existing_index(ime, text, kind)
    };
    ime.select_candidate(index);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let committed = ime.commit(CommitOptions { force: true });
    assert!(committed.ok);
    assert_eq!(committed.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let duplicate = ime.commit(CommitOptions { force: true });
    assert!(!duplicate.ok);
    assert!(duplicate.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn complete_leisure_words_support_romaji_case_and_hiragana_on_page_one() {
    assert_eq!(WORDS.len(), 24);
    for &(romaji, kana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
        ] {
            let mut ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            commit_once_and_undo(&mut ime, word, CandidateKind::Word, true);
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal, false);
        }
    }
}

#[test]
fn complete_leisure_readings_offer_words_and_both_authored_sentences() {
    assert_eq!(SENTENCES.len(), 12);
    for &(word, sentences) in SENTENCES {
        let &(romaji, kana, _) = WORDS.iter().find(|(_, _, text)| *text == word).unwrap();
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
        ] {
            let ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            page_one_index(&ime, word, CandidateKind::Word);
            for sentence in sentences {
                commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence, true);
            }
        }
    }
}

#[test]
fn specific_unfinished_leisure_readings_keep_words_and_both_sentences_on_page_one() {
    // Each fixed prefix narrows to the authored reading. Broad こう / う
    // are separately checked without displacing established completions.
    for (seed, word) in [
        ("sanp", "散歩"),
        ("dokush", "読書"),
        ("ongak", "音楽"),
        ("eig", "映画"),
        ("shashi", "写真"),
        ("ryour", "料理"),
        ("shum", "趣味"),
        ("koue", "公園"),
        ("bijutsuka", "美術館"),
        ("toshoka", "図書館"),
        ("kyuuji", "休日"),
        ("hanam", "花見"),
    ] {
        let ime = engine(seed);
        assert_local_draft(&ime, seed);
        page_one_index(&ime, word, CandidateKind::Word);
        let &(_, sentences) = SENTENCES.iter().find(|(text, _)| *text == word).unwrap();
        for sentence in sentences {
            commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence, true);
        }
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal, false);
    }
}

#[test]
fn reading_separators_preserve_raw_leisure_input_and_authored_choices() {
    for (seed, word, sentence) in [
        ("  san po  ", "散歩", "散歩に行きませんか。"),
        (
            "\ton\tgaku\u{3000}",
            "音楽",
            "音楽を聴いてゆっくりしたいです。",
        ),
        ("えい が", "映画", "映画を一緒に見ませんか。"),
        ("ryou ri", "料理", "料理を一緒に作りませんか。"),
        ("KYUU JITSU", "休日", "休日はゆっくり過ごしたいです。"),
        ("ha\u{3000}nami", "花見", "花見に行きませんか。"),
        ("doku sho", "読書", "読書をして過ごしたいです。"),
        ("びじゅつ かん", "美術館", "美術館に行ってみたいです。"),
        ("to sho kan", "図書館", "図書館で本を探したいです。"),
        ("ON’GAKU", "音楽", "音楽のおすすめを教えてください。"),
    ] {
        let ime = engine(seed);
        assert_local_draft(&ime, seed);
        page_one_index(&ime, word, CandidateKind::Word);
        commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence, true);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal, false);
    }
}

#[test]
fn leisure_adoptions_can_be_restored_then_continue_without_implicit_commit() {
    for (reading, word, continued, sentence) in [
        ("sanpo", "散歩", "散歩ni", "散歩に行きませんか。"),
        ("dokusho", "読書", "読書wo", "読書をして過ごしたいです。"),
        (
            "ongaku",
            "音楽",
            "音楽WO",
            "音楽を聴いてゆっくりしたいです。",
        ),
        ("eiga", "映画", "映画 wo", "映画を一緒に見ませんか。"),
        ("shashin", "写真", "写真を", "写真を撮ってもいいですか。"),
        ("ryouri", "料理", "料理wo", "料理を一緒に作りませんか。"),
        ("shumi", "趣味", "趣味は", "趣味は何ですか。"),
        ("kouen", "公園", "公園de", "公園で少し休みませんか。"),
        (
            "bijutsukan",
            "美術館",
            "美術館 ni",
            "美術館に行ってみたいです。",
        ),
        (
            "toshokan",
            "図書館",
            "図書館de",
            "図書館で本を探したいです。",
        ),
        (
            "kyuujitsu",
            "休日",
            "休日ha",
            "休日はゆっくり過ごしたいです。",
        ),
        ("hanami", "花見", "花見 ni", "花見に行きませんか。"),
    ] {
        let mut ime = engine(reading);
        let selected = page_one_index(&ime, word, CandidateKind::Word);
        ime.select_candidate(selected);
        let adopted = ime.selected_completion_text(true).unwrap().to_owned();
        assert_eq!(adopted, word);
        assert!(ime.snapshot().committed_text.is_empty());
        // Editable host-style adoption / restoration is modeled by seed
        // updates; this is not desktop key injection or host cancellation QA.
        ime.seed(&adopted);
        assert_local_draft(&ime, word);
        ime.seed(reading);
        assert_local_draft(&ime, reading);
        page_one_index(&ime, word, CandidateKind::Word);
        ime.seed(word);
        ime.seed(continued);
        assert_local_draft(&ime, continued);
        commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence, true);
        commit_once_and_undo(
            &mut engine(continued),
            continued,
            CandidateKind::Literal,
            false,
        );
    }
}

#[test]
fn broad_leisure_prefixes_preserve_old_words_and_exact_later_page_selection() {
    let kou = engine("こう");
    assert_local_draft(&kou, "こう");
    let candidate = page_one_index(&kou, "候補", CandidateKind::Word);
    let update = page_one_index(&kou, "更新", CandidateKind::Word);
    let park = existing_index(&kou, "公園", CandidateKind::Word);
    assert!(candidate < update && update < park);
    assert!((PAGE_SIZE..12).contains(&park));
    page_one_index(&kou, "公園で少し休みませんか。", CandidateKind::Sentence);
    page_one_index(&kou, "公園まで歩いてみましょう。", CandidateKind::Sentence);
    let u = engine("う");
    assert_local_draft(&u, "う");
    let receipt = page_one_index(&u, "受け取り", CandidateKind::Word);
    let reception = page_one_index(&u, "受付", CandidateKind::Word);
    let exercise = existing_index(&u, "運動", CandidateKind::Word);
    let song = existing_index(&u, "歌", CandidateKind::Word);
    assert!(receipt < reception && reception < exercise && exercise < song);
    assert!((PAGE_SIZE..12).contains(&exercise));
    assert!((PAGE_SIZE..12).contains(&song));
    for (seed, word) in [("こう", "公園"), ("う", "運動"), ("う", "歌")] {
        let mut ime = engine(seed);
        assert_local_draft(&ime, seed);
        let selected = existing_index(&ime, word, CandidateKind::Word);
        assert!((PAGE_SIZE..12).contains(&selected));
        ime.select_candidate(selected);
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        ime.seed(word);
        assert_local_draft(&ime, word);
        ime.seed(seed);
        assert_local_draft(&ime, seed);
        commit_once_and_undo(&mut ime, word, CandidateKind::Word, false);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal, false);
    }
}

#[test]
fn leisure_suggestions_never_remove_unknown_text_or_line_boundaries() {
    for seed in [
        "xyzsanpo",
        "🙂音楽を",
        "前置き映画を",
        "https://dokusho",
        "公園dexyz",
        "花見には🙂",
        "休日をは",
    ] {
        let mut ime = engine(seed);
        assert_local_draft(&ime, seed);
        for candidate in ime.candidates() {
            for &(_, sentences) in SENTENCES {
                for sentence in sentences {
                    assert!(
                        !candidate.text.ends_with(sentence),
                        "manufactured {sentence:?} from {seed:?}: {:?}",
                        ime.candidates()
                    );
                }
            }
        }
        commit_once_and_undo(&mut ime, seed, CandidateKind::Literal, false);
    }
    for separator in [
        "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (head, tail) in [("san", "po"), ("音楽", "wo"), ("doku", "sho")] {
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
            commit_once_and_undo(&mut ime, &seed, CandidateKind::Literal, false);
        }
    }
}
