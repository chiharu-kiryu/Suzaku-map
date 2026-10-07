use suzaku_map::lexicon::{EntryKind, Lexicon, builtin};

#[test]
fn independent_resources_do_not_need_a_language_plugin_host_or_model() {
    let lexicon = Lexicon::from_json(
        r#"{
        "format_version":1,"language":"fr",
        "word_layers":[{"id":"personal-example","words":["bonjour","salut"],
            "next_words":[["bonjour",["tout"]]],"sentences":["bonjour tout le monde."]}],
        "readings":[{"reading":"bonjour","text":"Bonjour","kind":"word"}],
        "continuations":[["Bonjour",["Bonjour tout le monde."]]]
    }"#,
    )
    .unwrap();
    assert_eq!(lexicon.language(), "fr");
    assert_eq!(lexicon.word_layers()[0].words, ["bonjour", "salut"]);
    assert_eq!(lexicon.next_words().next().unwrap().0, "bonjour");
    assert_eq!(
        lexicon.sentences().collect::<Vec<_>>(),
        ["bonjour tout le monde."]
    );
    assert_eq!(lexicon.readings()[0].kind, EntryKind::Word);
    assert!(
        builtin("fr").is_none(),
        "parsing is not implicit engine installation"
    );
}

#[test]
fn builtin_resources_are_cached_and_have_matching_exact_identifiers() {
    for language in ["en", "zh-Hans", "ja"] {
        let first = builtin(language).unwrap();
        assert_eq!(first.language(), language);
        assert!(std::ptr::eq(first, builtin(language).unwrap()));
    }
    for unsupported in ["", "EN", "unknown", "../../en.json"] {
        assert!(builtin(unsupported).is_none());
    }
}

#[test]
fn layer_order_and_duplicate_priority_positions_are_not_normalized_away() {
    let lexicon = Lexicon::from_json(
        r#"{
        "format_version":1,"language":"en",
        "word_layers":[
            {"id":"first","words":["world","hello","world"]},
            {"id":"later","words":["again","hello"]}
        ]
    }"#,
    )
    .unwrap();
    let layers = lexicon.word_layers();
    assert_eq!(layers[0].id, "first");
    assert_eq!(layers[1].id, "later");
    assert_eq!(layers[0].words, ["world", "hello", "world"]);
    assert_eq!(layers[1].words, ["again", "hello"]);
}

#[test]
fn homophones_separator_policy_and_kind_are_explicit_data() {
    let lexicon = Lexicon::from_json(
        r#"{
        "format_version":1,"language":"custom",
        "readings":[
            {"reading":"same","text":"甲","kind":"word"},
            {"reading":"same","text":"乙。","kind":"sentence","require_separators":true}
        ]
    }"#,
    )
    .unwrap();
    assert_eq!(lexicon.readings()[0].text, "甲");
    assert_eq!(lexicon.readings()[1].text, "乙。");
    assert_eq!(lexicon.readings()[1].kind, EntryKind::Sentence);
    assert!(!lexicon.readings()[0].require_separators);
    assert!(lexicon.readings()[1].require_separators);
    let chinese = builtin("zh-Hans").unwrap();
    assert_eq!(
        chinese
            .readings()
            .iter()
            .find(|entry| entry.text == "你好吗")
            .unwrap()
            .kind,
        EntryKind::Sentence
    );
    assert!(
        chinese
            .readings()
            .iter()
            .find(|entry| entry.text == "西安")
            .unwrap()
            .require_separators
    );
}

#[test]
fn invalid_or_mistyped_metadata_is_rejected_without_partial_success() {
    for json in [
        r#"{}"#,
        r#"{"format_version":2,"language":"en"}"#,
        r#"{"format_version":1,"language":""}"#,
        r#"{"format_version":1,"language":"../en"}"#,
        r#"{"format_version":1,"language":"en","word_layer":[]}"#,
        r#"{"format_version":1,"language":"en","word_layers":[{"id":"a","word":[] }]}"#,
        r#"{"format_version":1,"language":"en","word_layers":[{"id":"a"},{"id":"a"}]}"#,
        r#"{"format_version":1,"language":"en","readings":[{"reading":"a","text":"A","kind":"literal"}]}"#,
        r#"{"format_version":1,"language":"en","readings":[{"reading":"a","text":"A","kind":"word","typo":true}]}"#,
    ] {
        assert!(Lexicon::from_json(json).is_err(), "accepted {json}");
    }
}

#[test]
fn malformed_content_and_ambiguous_duplicate_records_are_rejected() {
    for fragment in [
        r#""word_layers":[{"id":"a","words":["two words"]}]"#,
        r#""word_layers":[{"id":"a","words":[""]}]"#,
        r#""word_layers":[{"id":"a","sentences":["hidden\ncontrol"]}]"#,
        r#""readings":[{"reading":" a","text":"A","kind":"word"}]"#,
        r#""readings":[{"reading":"a","text":"A","kind":"word"},{"reading":"a","text":"A","kind":"sentence"}]"#,
        r#""continuations":[["a",[]]]"#,
        r#""continuations":[["a",["a one"]],["a",["a two"]]]"#,
        r#""continuations":[["a",["a one","a one"]]]"#,
        r#""continuations":[["a",["b different prefix"]]]"#,
        r#""word_layers":[{"id":"a","next_words":[["same",["one"]]]},{"id":"b","next_words":[["same",["two"]]]}]"#,
    ] {
        let json = format!(r#"{{"format_version":1,"language":"en",{fragment}}}"#);
        assert!(Lexicon::from_json(&json).is_err(), "accepted {json}");
    }
}

#[test]
fn resource_and_text_limits_are_checked_before_consumption() {
    assert!(
        Lexicon::from_json(&" ".repeat(2 * 1024 * 1024 + 1))
            .unwrap_err()
            .contains("2 MiB")
    );
    let long_word = "a".repeat(4097);
    let json = format!(
        r#"{{"format_version":1,"language":"en","word_layers":[{{"id":"a","words":["{long_word}"]}}]}}"#
    );
    assert!(Lexicon::from_json(&json).is_err());
    let layers: Vec<_> = (0..65).map(|i| format!(r#"{{"id":"layer{i}"}}"#)).collect();
    let json = format!(
        r#"{{"format_version":1,"language":"en","word_layers":[{}]}}"#,
        layers.join(",")
    );
    assert!(Lexicon::from_json(&json).unwrap_err().contains("layers"));
}

// Snapshot wording, not just counts/ranks. NUL/0xff cannot occur in valid entries.
fn hash_pairs<'a>(pairs: impl Iterator<Item = (&'a str, &'a [String])>) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for (context, values) in pairs {
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        hash = (hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    hash
}

#[test]
fn social_expansion_keeps_every_previous_context_and_sentence_unchanged() {
    let en = builtin("en").unwrap();
    let zh = builtin("zh-Hans").unwrap();
    assert_eq!(hash_pairs(en.next_words().take(449)), 0xfd567d3db4e0b922);
    assert_eq!(
        hash_pairs(en.sentences().take(862).map(|sentence| (sentence, &[][..]))),
        0xb512ac9b73b22b7d
    );
    assert_eq!(hash_pairs(zh.continuations().take(443)), 0xbc3c15e3a06455a1);
}

#[test]
fn objects_expansion_keeps_social_contexts_and_sentences_unchanged() {
    let en = builtin("en").unwrap();
    let zh = builtin("zh-Hans").unwrap();
    assert_eq!(hash_pairs(en.next_words().take(473)), 0x34af3fa752860cdf);
    assert_eq!(
        hash_pairs(en.sentences().take(910).map(|sentence| (sentence, &[][..]))),
        0x8a60dd1c50380604
    );
    assert_eq!(hash_pairs(zh.continuations().take(467)), 0x99b4be328206d04d);
}

#[test]
fn choices_expansion_keeps_all_released_contexts_and_sentences_unchanged() {
    // Captured from clean v0.8.2 before adding the choices vocabulary.
    let en = builtin("en").unwrap();
    let zh = builtin("zh-Hans").unwrap();
    assert_eq!(hash_pairs(en.next_words().take(497)), 0x7cb09ae2cc90c467);
    assert_eq!(
        hash_pairs(en.sentences().take(958).map(|sentence| (sentence, &[][..]))),
        0x18a3e1f0f4ad063f
    );
    assert_eq!(hash_pairs(zh.continuations().take(491)), 0xfe31248db8a31283);
}

#[test]
fn followup_expansion_preserves_every_pre_expansion_layer_and_context() {
    // Captured before the daily-followup/JP coordination data was appended.
    // Preserve complete content/order, not just unchanged resource totals.
    let en = builtin("en").unwrap();
    let zh = builtin("zh-Hans").unwrap();
    let ja = builtin("ja").unwrap();
    assert_eq!(
        hash_pairs(
            en.word_layers()
                .iter()
                .take(17)
                .map(|layer| (layer.id.as_str(), layer.words.as_slice()))
        ),
        0xd57607c4975244b6
    );
    assert_eq!(hash_pairs(en.next_words().take(521)), 0x110e8ca12a531a71);
    assert_eq!(
        hash_pairs(en.sentences().take(1006).map(|text| (text, &[][..]))),
        0x8dbf28d1bb7688b5
    );
    assert_eq!(hash_pairs(zh.continuations().take(515)), 0xd7dae0bce00b7a22);
    assert_eq!(hash_pairs(ja.continuations().take(62)), 0x2be8a0ecc8792ce5);
}

#[test]
fn leisure_expansion_preserves_every_followup_layer_and_context() {
    // Captured before the leisure vocabulary was appended. Older snapshots
    // remain independent guards for the historical prefixes.
    let en = builtin("en").unwrap();
    let zh = builtin("zh-Hans").unwrap();
    let ja = builtin("ja").unwrap();
    assert_eq!(
        hash_pairs(
            en.word_layers()
                .iter()
                .take(18)
                .map(|layer| (layer.id.as_str(), layer.words.as_slice()))
        ),
        0x9b66f8d032af2d48
    );
    assert_eq!(hash_pairs(en.next_words().take(545)), 0xaeafad06cf6cfec8);
    assert_eq!(
        hash_pairs(en.sentences().take(1054).map(|text| (text, &[][..]))),
        0xccebc462db57551b
    );
    assert_eq!(hash_pairs(zh.continuations().take(539)), 0xf84a19a37684cf7f);
    assert_eq!(hash_pairs(ja.continuations().take(78)), 0x08fef34c52d23710);
}

#[test]
fn japanese_daily_expansion_keeps_the_original_continuations_unchanged() {
    let ja = builtin("ja").unwrap();
    assert_eq!(hash_pairs(ja.continuations().take(14)), 0xdc98b98ae666f412);
    assert_eq!(
        ja.continuations()
            .map(|(_, values)| values.len())
            .sum::<usize>(),
        203
    );
}

#[test]
fn japanese_choices_keep_all_released_continuations_unchanged() {
    // Captured before appending comparison/preference vocabulary; retain the
    // earlier 14-context snapshot as well as this full v0.8.2 snapshot.
    let ja = builtin("ja").unwrap();
    assert_eq!(hash_pairs(ja.continuations().take(38)), 0x619160b447916797);
}

#[test]
fn migrated_readings_preserve_every_original_position_and_flag() {
    // FNV-1a snapshots taken from the original Rust tables, not the loader's
    // output. New readings can be appended; reordering the original homophones
    // or losing mandatory boundaries must still fail against the old hashes.
    for (language, count, expected) in [
        ("zh-Hans", 1143, 0x6e3c269b699b4a76_u64),
        // Also freeze the first fallback expansion before appending conversation data.
        ("zh-Hans", 1254, 0xfbccf5600288d524_u64),
        // And freeze conversation before appending the essentials expansion.
        ("zh-Hans", 1428, 0x07099ff30fb19027_u64),
        // Freeze essentials before adding explanation/learning/follow-up vocabulary.
        ("zh-Hans", 1631, 0xef71941ba6109809_u64),
        // Keep the clarity batch unchanged when appending the missing 再 homophone.
        ("zh-Hans", 1805, 0xd6baee17d78df81c_u64),
        // Freeze all 0.7.5 entries, including 再, before digital-life vocabulary.
        ("zh-Hans", 1806, 0x7af33f71531104b6_u64),
        // Freeze the digital-life entries before appending the home layer.
        ("zh-Hans", 1931, 0xa9f2649daf9ddc84_u64),
        // Freeze home-life readings before adding the errands layer.
        ("zh-Hans", 1984, 0xe19cee5e8babbc73_u64),
        // Freeze errands before restoring missing everyday adjective forms.
        ("zh-Hans", 2043, 0x7571270b8211ad24_u64),
        // Freeze the basic 很强/很厉害 repair before adding daily conversation.
        ("zh-Hans", 2047, 0x4f38a2847da90928_u64),
        // Freeze daily chat before appending food/weather/arrangement expressions.
        ("zh-Hans", 2111, 0x51d7947dfa3aed3a_u64),
        // Freeze the released daily-needs vocabulary before coordination phrases.
        ("zh-Hans", 2175, 0xe17ae047667a8cf5_u64),
        // Freeze coordination before the daily social fallback expansion.
        ("zh-Hans", 2239, 0x1b004562577bd470_u64),
        // Freeze social before everyday object, quantity and placement phrases.
        ("zh-Hans", 2303, 0x16edb6e92561eb5c_u64),
        // Freeze v0.8.2 before daily preferences, comparisons and alternatives.
        ("zh-Hans", 2367, 0x2593a87f84f4b2e4_u64),
        // Freeze the complete comparison layer before daily followup expressions.
        ("zh-Hans", 2432, 0xd84f6b0a49578ed6_u64),
        // Freeze progress/clarification before leisure and weekend expressions.
        ("zh-Hans", 2496, 0x2b8f323e232e082e_u64),
        ("ja", 26, 0xf2e1aa0ec03a7e38_u64),
        // Freeze all v0.8.2 Japanese readings and their original homophone order.
        ("ja", 74, 0x7cf9fb332d108ce3_u64),
        // Freeze Japanese comparisons before everyday messages/coordination.
        ("ja", 122, 0x051f413326359c03_u64),
        // Freeze coordination before leisure vocabulary and continuations.
        ("ja", 154, 0x6c9c733586d790c5_u64),
    ] {
        let entries = builtin(language).unwrap().readings();
        assert!(entries.len() >= count);
        let mut hash = 0xcbf29ce484222325_u64;
        for entry in entries.iter().take(count) {
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
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        assert_eq!(
            hash, expected,
            "{language} first {count} readings changed existing data/rank"
        );
    }
    assert_eq!(builtin("en").unwrap().next_words().count(), 577);
    assert_eq!(builtin("en").unwrap().sentences().count(), 1118);
    assert_eq!(builtin("zh-Hans").unwrap().readings().len(), 2592);
    assert_eq!(builtin("zh-Hans").unwrap().continuations().count(), 571);
    assert_eq!(
        builtin("zh-Hans")
            .unwrap()
            .continuations()
            .map(|(_, values)| values.len())
            .sum::<usize>(),
        1140
    );
    assert_eq!(builtin("ja").unwrap().readings().len(), 202);
    assert_eq!(builtin("ja").unwrap().continuations().count(), 102);
}
