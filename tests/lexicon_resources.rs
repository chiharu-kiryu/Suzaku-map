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
        ("ja", 26, 0xf2e1aa0ec03a7e38_u64),
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
    assert_eq!(builtin("en").unwrap().next_words().count(), 313);
    assert_eq!(builtin("en").unwrap().sentences().count(), 590);
    assert_eq!(builtin("zh-Hans").unwrap().readings().len(), 1806);
    assert_eq!(builtin("zh-Hans").unwrap().continuations().count(), 305);
    assert_eq!(
        builtin("zh-Hans")
            .unwrap()
            .continuations()
            .map(|(_, values)| values.len())
            .sum::<usize>(),
        608
    );
    assert_eq!(builtin("ja").unwrap().readings().len(), 26);
    assert_eq!(builtin("ja").unwrap().continuations().count(), 14);
}
