//! Authored object-location, small-action, quantity and size vocabulary.
//! Fixed expectations exercise the offline engine, not the real desktop or a
//! model. They intentionally do not derive expected text from the lexicon.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

const WORDS: &[(&str, &str)] = &[
    ("fang'na'le", "放哪了"),
    ("fang'zai'na'li", "放在哪里"),
    ("fang'hui'yuan'chu", "放回原处"),
    ("fang'dao'zhuo'shang", "放到桌上"),
    ("fang'jin'chou'ti", "放进抽屉"),
    ("zai'gui'zi'li", "在柜子里"),
    ("zai'bao'li'mian", "在包里面"),
    ("zai'ni'pang'bian", "在你旁边"),
    ("zai'men'hou'mian", "在门后面"),
    ("zai'zuo'shou'bian", "在左手边"),
    ("zai'you'shou'bian", "在右手边"),
    ("zhao'dao'le'ma", "找到了吗"),
    ("hai'mei'zhao'dao", "还没找到"),
    ("bang'wo'zhao'zhao", "帮我找找"),
    ("bie'nong'diu'le", "别弄丢了"),
    ("ji'de'dai'shang", "记得带上"),
    ("na'guo'lai", "拿过来"),
    ("na'guo'qu", "拿过去"),
    ("di'gei'wo", "递给我"),
    ("bang'wo'na'yi'xia", "帮我拿一下"),
    ("bang'wo'fu'yi'xia", "帮我扶一下"),
    ("bang'wo'ban'yi'xia", "帮我搬一下"),
    ("xian'fang'zhe'li", "先放这里"),
    ("qing'na'qing'fang", "轻拿轻放"),
    ("xiao'xin'tang'shou", "小心烫手"),
    ("xiao'xin'bie'shuai'le", "小心别摔了"),
    ("ba'men'guan'shang", "把门关上"),
    ("ba'chuang'da'kai", "把窗打开"),
    ("gei'wo'liu'yi'ge", "给我留一个"),
    ("bang'wo'shou'qi'lai", "帮我收起来"),
    ("yong'wan'huan'gei'wo", "用完还给我"),
    ("xian'jie'wo'yong'yong", "先借我用用"),
    ("gou'bu'gou", "够不够"),
    ("gou'yong'le", "够用了"),
    ("hai'bu'gou", "还不够"),
    ("hai'sheng'duo'shao", "还剩多少"),
    ("hai'you'ji'ge", "还有几个"),
    ("hai'cha'ji'ge", "还差几个"),
    ("duo'na'yi'ge", "多拿一个"),
    ("shao'na'yi'ge", "少拿一个"),
    ("mei'ren'yi'ge", "每人一个"),
    ("yi'ren'yi'fen", "一人一份"),
    ("zai'lai'yi'fen", "再来一份"),
    ("liu'dian'bei'yong", "留点备用"),
    ("yong'wan'le", "用完了"),
    ("kuai'yong'wan'le", "快用完了"),
    ("duo'mai'yi'dian", "多买一点"),
    ("shao'mai'yi'dian", "少买一点"),
    ("da'yi'dian", "大一点"),
    ("xiao'yi'dian", "小一点"),
    ("chang'yi'dian", "长一点"),
    ("duan'yi'dian", "短一点"),
    ("kuan'yi'dian", "宽一点"),
    ("zhai'yi'dian", "窄一点"),
    ("hou'yi'dian", "厚一点"),
    ("bao'yi'dian", "薄一点"),
    ("tai'jin'le", "太紧了"),
    ("tai'song'le", "太松了"),
    ("gang'hao'he'shi", "刚好合适"),
    ("huan'ge'da'de", "换个大的"),
    ("huan'ge'xiao'de", "换个小的"),
    ("zai'gao'yi'dian", "再高一点"),
    ("zai'di'yi'dian", "再低一点"),
    ("zhe'yang'zheng'hao", "这样正好"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "放哪了",
        ["放哪了，我刚才还看见了。", "放哪了，帮我一起找找吧。"],
    ),
    (
        "放回原处",
        ["放回原处之前，记得擦干净。", "放回原处，这样下次就好找了。"],
    ),
    (
        "放进抽屉",
        [
            "放进抽屉里，别放在桌子边上。",
            "放进抽屉之前，先把东西分类。",
        ],
    ),
    (
        "还没找到",
        [
            "还没找到，我再看看包里。",
            "还没找到，你记得最后放在哪了吗？",
        ],
    ),
    (
        "帮我找找",
        [
            "帮我找找钥匙，可能在沙发附近。",
            "帮我找找，我记得刚才还在这里。",
        ],
    ),
    (
        "记得带上",
        [
            "记得带上钥匙，出门前检查一下。",
            "记得带上雨伞，天气可能会变。",
        ],
    ),
    ("拿过来", ["拿过来吧，我帮你看看。", "拿过来放在这里就好。"]),
    (
        "帮我拿一下",
        ["帮我拿一下这个袋子，谢谢。", "帮我拿一下，我腾出手来开门。"],
    ),
    (
        "轻拿轻放",
        ["轻拿轻放，里面有玻璃杯。", "轻拿轻放，别碰到旁边的东西。"],
    ),
    (
        "小心烫手",
        ["小心烫手，先放一会儿再拿。", "小心烫手，记得用隔热手套。"],
    ),
    (
        "给我留一个",
        [
            "给我留一个，我等会儿过来拿。",
            "给我留一个就够了，其他的你们分吧。",
        ],
    ),
    (
        "用完还给我",
        ["用完还给我就行，不用着急。", "用完还给我，我明天还要用。"],
    ),
    (
        "够不够",
        ["够不够，不够我再拿一点。", "够不够用，你先试试看。"],
    ),
    (
        "还剩多少",
        ["还剩多少，我们先数一下。", "还剩多少，不够的话我去补一些。"],
    ),
    (
        "还有几个",
        ["还有几个，够大家分吗？", "还有几个没拿，帮我确认一下。"],
    ),
    (
        "每人一个",
        ["每人一个，剩下的先收起来。", "每人一个，不够的再跟我说。"],
    ),
    (
        "快用完了",
        [
            "快用完了，下次买东西时记得补上。",
            "快用完了，先看看还有没有备用的。",
        ],
    ),
    (
        "留点备用",
        ["留点备用，别一次全用掉。", "留点备用，放在容易找到的地方。"],
    ),
    (
        "大一点",
        ["大一点的袋子能装得下吗？", "大一点会更方便拿取。"],
    ),
    (
        "小一点",
        ["小一点的盒子更好收纳。", "小一点也可以，能放进去就行。"],
    ),
    (
        "长一点",
        [
            "长一点的线用起来更方便。",
            "长一点没关系，可以把多余的收起来。",
        ],
    ),
    (
        "薄一点",
        [
            "薄一点的外套比较适合今天的天气。",
            "薄一点会更轻便，出门好携带。",
        ],
    ),
    (
        "刚好合适",
        ["刚好合适，就用这个吧。", "刚好合适，不用再调整了。"],
    ),
    (
        "换个小的",
        ["换个小的吧，这个放不进去。", "换个小的就够了，带着也方便。"],
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

fn commit_once_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let index = first_page_index(ime, text, kind);
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
}

#[test]
fn everyday_object_words_are_on_page_one_in_all_four_spellings() {
    assert_eq!(WORDS.len(), 64);
    for &(reading, word) in WORDS {
        for seed in spellings(reading) {
            let mut ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            commit_once_and_undo(&mut ime, word, CandidateKind::Word);
        }
    }
}

#[test]
fn explicit_object_readings_offer_words_and_both_sentences_on_page_one() {
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
fn unfinished_object_readings_keep_word_and_sentence_choices() {
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
fn adopted_object_words_preserve_padding_and_stay_editable() {
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
            }
        }
    }
}

#[test]
fn object_sentences_continue_after_new_pinyin_and_preserve_long_literal_prefixes() {
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for next in ["之前", "zhi qian", "zhi'qian", "zhiqian", "ZHI QIAN"] {
                let seed = format!("{prefix}放回原处{padding}{next}");
                let expected = format!("{prefix}放回原处{padding}之前，记得擦干净。");
                let mut ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                // Choosing the literal keeps the user's mixed spelling intact,
                // including the long prefix and the exact whitespace.
                commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
            for trailing in ["", " ", "  ", "\u{3000}"] {
                let seed = format!("{prefix}放回原处{padding}之前{trailing}");
                let expected = format!("{seed}，记得擦干净。");
                let mut ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
        }
    }
}
