//! Authored everyday check-ins, invitations, supportive replies and polite
//! refusals. Expectations are independent of the data file and need no model,
//! learned history or real keyboard. Host/GTK adoption has a separate gate.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

const WORDS: &[(&str, &str)] = &[
    ("zui'jin'zen'me'yang", "最近怎么样"),
    ("zui'jin'mang'ma", "最近忙吗"),
    ("hai'shun'li'ma", "还顺利吗"),
    ("hai'xi'guan'ma", "还习惯吗"),
    ("shui'de'hao'ma", "睡得好吗"),
    ("xiu'xi'de'zen'me'yang", "休息得怎么样"),
    ("bie'tai'lei'le", "别太累了"),
    ("zhao'gu'hao'zi'ji", "照顾好自己"),
    ("ji'de'chi'fan", "记得吃饭"),
    ("zao'dian'shui'ba", "早点睡吧"),
    ("bie'ao'ye'le", "别熬夜了"),
    ("man'dian'zou", "慢点走"),
    ("dao'jia'shuo'yi'sheng", "到家说一声"),
    ("ji'de'bao'ping'an", "记得报平安"),
    ("wo'ting'zhe'ne", "我听着呢"),
    ("xiang'shuo'jiu'shuo", "想说就说"),
    ("wo'neng'li'jie", "我能理解"),
    ("que'shi'bu'rong'yi", "确实不容易"),
    ("nan'wei'ni'le", "难为你了"),
    ("bie'wang'xin'li'qu", "别往心里去"),
    ("bu'yong'mian'qiang", "不用勉强"),
    ("liang'li'er'xing", "量力而行"),
    ("kai'xin'jiu'hao", "开心就好"),
    ("ni'kai'xin'jiu'hao", "你开心就好"),
    ("ti'ni'gao'xing", "替你高兴"),
    ("gong'xi'ni'ya", "恭喜你呀"),
    ("zhi'de'qing'zhu", "值得庆祝"),
    ("zhen'wei'ni'gao'xing", "真为你高兴"),
    ("yi'qi'chi'fan'ba", "一起吃饭吧"),
    ("chu'lai'zou'zou", "出来走走"),
    ("yi'qi'san'bu", "一起散步"),
    ("he'bei'ka'fei", "喝杯咖啡"),
    ("zhao'ge'di'fang'zuo'zuo", "找个地方坐坐"),
    ("na'tian'fang'bian", "哪天方便"),
    ("zhou'mo'jian'ba", "周末见吧"),
    ("wo'men'zai'na'jian", "我们在哪见"),
    ("wo'qu'jie'ni", "我去接你"),
    ("xu'yao'wo'jie'ma", "需要我接吗"),
    ("wo'lai'an'pai", "我来安排"),
    ("ni'lai'jue'ding", "你来决定"),
    ("ting'ni'an'pai", "听你安排"),
    ("dou'ting'ni'de", "都听你的"),
    ("wo'dou'ke'yi", "我都可以"),
    ("kan'ni'fang'bian", "看你方便"),
    ("dao'shi'hou'zai'shuo", "到时候再说"),
    ("dao'shi'lian'xi", "到时联系"),
    ("lin'shi'you'an'pai", "临时有安排"),
    ("jin'tian'zou'bu'kai", "今天走不开"),
    ("zhe'ci'qu'bu'liao", "这次去不了"),
    ("xia'ci'yi'ding", "下次一定"),
    ("xie'xie'yao'qing", "谢谢邀请"),
    ("xin'yi'ling'le", "心意领了"),
    ("zhen'de'bu'yong", "真的不用"),
    ("bu'yong'te'yi", "不用特意"),
    ("bie'po'fei'le", "别破费了"),
    ("bie'zhe'me'ke'qi", "别这么客气"),
    ("wo'xian'bu'qu'le", "我先不去了"),
    ("gai'ge'shi'jian'ba", "改个时间吧"),
    ("ti'qian'shuo'yi'sheng", "提前说一声"),
    ("dao'le'gao'su'wo", "到了告诉我"),
    ("man'man'liao", "慢慢聊"),
    ("liao'de'hen'kai'xin", "聊得很开心"),
    ("jin'tian'hen'kai'xin", "今天很开心"),
    ("xia'ci'zai'liao", "下次再聊"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "最近怎么样",
        ["最近怎么样，有空聊聊吗？", "最近怎么样，工作还顺利吗？"],
    ),
    (
        "最近忙吗",
        [
            "最近忙吗，记得给自己留点休息时间。",
            "最近忙吗，有空一起吃顿饭吧。",
        ],
    ),
    (
        "还习惯吗",
        [
            "还习惯吗，有不方便的地方就告诉我。",
            "还习惯吗，慢慢适应就好。",
        ],
    ),
    (
        "别太累了",
        ["别太累了，忙完早点休息。", "别太累了，有需要就叫我。"],
    ),
    (
        "照顾好自己",
        [
            "照顾好自己，记得按时吃饭。",
            "照顾好自己，有空给我发个消息。",
        ],
    ),
    (
        "到家说一声",
        ["到家说一声，我好放心。", "到家说一声，路上慢点。"],
    ),
    (
        "我听着呢",
        ["我听着呢，你慢慢说。", "我听着呢，想聊什么都可以。"],
    ),
    (
        "我能理解",
        [
            "我能理解，你也有自己的难处。",
            "我能理解，我们再一起想想办法。",
        ],
    ),
    (
        "确实不容易",
        [
            "确实不容易，辛苦你了。",
            "确实不容易，能做到这样已经很好了。",
        ],
    ),
    (
        "别往心里去",
        ["别往心里去，大家没有怪你。", "别往心里去，先好好休息一下。"],
    ),
    (
        "不用勉强",
        ["不用勉强，按你自己的想法来。", "不用勉强，不方便就下次吧。"],
    ),
    (
        "替你高兴",
        [
            "替你高兴，这次的努力没有白费。",
            "替你高兴，找个时间庆祝一下吧。",
        ],
    ),
    (
        "一起吃饭吧",
        [
            "一起吃饭吧，你想吃什么？",
            "一起吃饭吧，我们找个安静点的地方。",
        ],
    ),
    (
        "出来走走",
        ["出来走走吧，换换心情。", "出来走走，我们边走边聊。"],
    ),
    (
        "一起散步",
        [
            "一起散步吧，饭后走一会儿。",
            "一起散步怎么样，去附近的公园？",
        ],
    ),
    (
        "哪天方便",
        ["哪天方便，我们约个时间见面。", "哪天方便，提前告诉我一声。"],
    ),
    (
        "我去接你",
        ["我去接你，把位置发给我。", "我去接你之前会先给你发消息。"],
    ),
    (
        "我来安排",
        [
            "我来安排，你告诉我大概几点有空。",
            "我来安排，确定以后再跟你说。",
        ],
    ),
    (
        "我都可以",
        [
            "我都可以，按你方便的时间来。",
            "我都可以，你选个想去的地方吧。",
        ],
    ),
    (
        "今天走不开",
        [
            "今天走不开，改天再约好吗？",
            "今天走不开，不好意思让你久等了。",
        ],
    ),
    (
        "这次去不了",
        ["这次去不了，谢谢你邀请我。", "这次去不了，你们玩得开心。"],
    ),
    (
        "谢谢邀请",
        [
            "谢谢邀请，我看看时间再回复你。",
            "谢谢邀请，这次没空，下次再一起吧。",
        ],
    ),
    (
        "别破费了",
        [
            "别破费了，大家随意一点就好。",
            "别破费了，你能来我就很开心。",
        ],
    ),
    (
        "聊得很开心",
        [
            "聊得很开心，下次有空再聊。",
            "聊得很开心，谢谢你愿意听我说。",
        ],
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
fn social_words_are_on_page_one_in_all_four_spellings_without_a_model() {
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
fn explicit_social_readings_offer_words_and_both_sentences_on_page_one() {
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
fn unfinished_social_readings_keep_word_and_sentence_choices() {
    for &(word, sentences) in SENTENCES {
        let reading = reading_for(word);
        let partial = &reading[..reading.len() - 1];
        for seed in spellings(partial) {
            let ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            first_page_index(&ime, word, CandidateKind::Word);
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
fn adopted_social_words_preserve_padding_and_stay_editable() {
    for &(word, sentences) in SENTENCES {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for sentence in sentences {
                let mut ime = engine(reading_for(word));
                let index = first_page_index(&ime, word, CandidateKind::Word);
                ime.select_candidate(index);
                let adopted = ime.selected_completion_text(true).unwrap().to_owned();
                assert_eq!(adopted, word);
                assert!(ime.snapshot().committed_text.is_empty());
                // This models an editable host draft update after adoption,
                // not physical Space dispatch or a desktop keyboard event.
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
fn social_sentences_continue_after_new_pinyin_and_preserve_long_literal_prefixes() {
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for next in ["之前", "zhi qian", "zhi'qian", "zhiqian", "ZHI QIAN"] {
                let seed = format!("{prefix}我去接你{padding}{next}");
                let expected = format!("{prefix}我去接你{padding}之前会先给你发消息。");
                let mut ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
            }
            for trailing in ["", " ", "  ", "\u{3000}"] {
                let seed = format!("{prefix}我去接你{padding}之前{trailing}");
                let expected = format!("{seed}会先给你发消息。");
                let mut ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
            }
        }
    }
}
