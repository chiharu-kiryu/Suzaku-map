//! Authored pack examples, not an independent corpus or live-model benchmark.
use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    languages::llm::{
        LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
    },
    lexicon::packs::{self, PackStore},
};

const ENGLISH: &[(&str, &str, &str)] = &[
    (
        "please annotate this para",
        "please annotate this paragraph",
        "please annotate this paragraph before our discussion.",
    ),
    (
        "could you proofread my dra",
        "could you proofread my draft",
        "could you proofread my draft before i submit it?",
    ),
    (
        "please check each cit",
        "please check each citation",
        "please check each citation against the original source.",
    ),
    (
        "can you paraphrase this sen",
        "can you paraphrase this sentence",
        "can you paraphrase this sentence in simpler language?",
    ),
    (
        "let's make some flash",
        "let's make some flashcards",
        "let's make some flashcards for the new vocabulary.",
    ),
    (
        "please follow the rub",
        "please follow the rubric",
        "please follow the rubric when revising your draft.",
    ),
    (
        "please number the foot",
        "please number the footnotes",
        "please number the footnotes in order.",
    ),
    (
        "please upload the completed work",
        "please upload the completed worksheet",
        "please upload the completed worksheet before class.",
    ),
    (
        "please pass me the col",
        "please pass me the colander",
        "please pass me the colander from the cupboard.",
    ),
    (
        "could you chop the oni",
        "could you chop the onions",
        "could you chop the onions while i get the bowl?",
    ),
    (
        "please whisk the eg",
        "please whisk the eggs",
        "please whisk the eggs in a separate bowl.",
    ),
    (
        "we need a larger sauce",
        "we need a larger saucepan",
        "we need a larger saucepan for this recipe.",
    ),
    (
        "let's prepare the mari",
        "let's prepare the marinade",
        "let's prepare the marinade in a separate bowl.",
    ),
    (
        "please knead the dou",
        "please knead the dough",
        "please knead the dough on the floured surface.",
    ),
    (
        "could you grate some che",
        "could you grate some cheese",
        "could you grate some cheese into this bowl?",
    ),
    (
        "could you bring the measuring spo",
        "could you bring the measuring spoons",
        "could you bring the measuring spoons from the drawer?",
    ),
    (
        "could you store our lugg",
        "could you store our luggage",
        "could you store our luggage until this afternoon?",
    ),
    (
        "is there a quieter ro",
        "is there a quieter room",
        "is there a quieter room away from the lift?",
    ),
    (
        "could you replace my keyc",
        "could you replace my keycard",
        "could you replace my keycard at the front desk?",
    ),
    (
        "does the room have a kit",
        "does the room have a kitchenette",
        "does the room have a kitchenette with a fridge?",
    ),
    (
        "where does the airport shutt",
        "where does the airport shuttle",
        "where does the airport shuttle pick up passengers?",
    ),
    (
        "could you send our updated itin",
        "could you send our updated itinerary",
        "could you send our updated itinerary by email?",
    ),
    (
        "can we extend our st",
        "can we extend our stay",
        "can we extend our stay by one night?",
    ),
    (
        "could you check the accessible ent",
        "could you check the accessible entrance",
        "could you check the accessible entrance before we arrive?",
    ),
    (
        "please update the project road",
        "please update the project roadmap",
        "please update the project roadmap before our next meeting.",
    ),
    (
        "could you confirm the task ow",
        "could you confirm the task owner",
        "could you confirm the task owner before we begin?",
    ),
    (
        "let's document the remaining block",
        "let's document the remaining blockers",
        "let's document the remaining blockers in the handover notes.",
    ),
    (
        "please share the meeting min",
        "please share the meeting minutes",
        "please share the meeting minutes with everyone.",
    ),
    (
        "could you review the acceptance crit",
        "could you review the acceptance criteria",
        "could you review the acceptance criteria with the team?",
    ),
    (
        "let's agree on the next mile",
        "let's agree on the next milestone",
        "let's agree on the next milestone before we finish.",
    ),
    (
        "please prepare the handover no",
        "please prepare the handover notes",
        "please prepare the handover notes before you go on leave.",
    ),
    (
        "could you check the latest deliv",
        "could you check the latest deliverable",
        "could you check the latest deliverable against the checklist?",
    ),
];

const CHINESE: &[(&str, &str, &str)] = &[
    ("yue'du'pi'zhu", "阅读批注", "阅读批注请写在页边空白处。"),
    ("xie'zuo'ti'gang", "写作提纲", "写作提纲先列出主要观点。"),
    ("can'kao'wen'xian", "参考文献", "参考文献请放在文章末尾。"),
    ("yin'yong'ge'shi", "引用格式", "引用格式请按课程要求统一。"),
    ("duan'luo'da'yi", "段落大意", "段落大意可以用一句话概括。"),
    (
        "jiao'dui'biao'dian",
        "校对标点",
        "校对标点后再检查引用格式。",
    ),
    ("fu'xi'ka'pian", "复习卡片", "复习卡片可以按主题分类。"),
    (
        "wen'xian'jian'suo",
        "文献检索",
        "文献检索可以先确定关键词。",
    ),
    ("shi'cai'qing'dan", "食材清单", "食材清单先按菜谱整理一下。"),
    ("bei'cai'fen'gong", "备菜分工", "备菜分工我们先商量一下。"),
    ("liang'shao", "量勺", "量勺放在左边的抽屉里。"),
    ("da'dan'qi", "打蛋器", "打蛋器放在第二层抽屉里。"),
    ("rou'mian", "揉面", "揉面之前先准备好操作台。"),
    ("can'he'biao'qian", "餐盒标签", "餐盒标签上写好菜品名称。"),
    ("mei'zhou'cai'dan", "每周菜单", "每周菜单我们一起拟一下。"),
    ("pei'liao'fen'liang", "配料分量", "配料分量请按菜谱核对。"),
    ("xing'li'ji'cun", "行李寄存", "行李寄存可以到下午吗？"),
    (
        "an'jing'fang'jian",
        "安静房间",
        "安静房间可以安排在远离电梯的位置吗？",
    ),
    ("bu'ban'fang'ka", "补办房卡", "补办房卡需要到前台办理吗？"),
    ("ke'fang'she'shi", "客房设施", "客房设施里有小冰箱吗？"),
    ("jie'bo'che", "接驳车", "接驳车在哪里等候？"),
    ("xing'cheng'que'ren", "行程确认", "行程确认后请发我一份。"),
    ("xu'zhu'shen'qing", "续住申请", "续住申请可以延长一晚吗？"),
    (
        "wu'zhang'ai'tong'dao",
        "无障碍通道",
        "无障碍通道的入口在哪里？",
    ),
    (
        "xiang'mu'lu'xian'tu",
        "项目路线图",
        "项目路线图请在下次会议前更新。",
    ),
    (
        "ren'wu'fu'ze'ren",
        "任务负责人",
        "任务负责人能再确认一下吗？",
    ),
    (
        "dai'jie'shi'xiang",
        "待解事项",
        "待解事项先记录到交接文档里。",
    ),
    ("ji'yao'fu'he", "纪要复核", "纪要复核请在发送前完成。"),
    ("yan'shou'qing'dan", "验收清单", "验收清单请在开始前确认。"),
    ("jie'duan'mu'biao", "阶段目标", "阶段目标我们一起核对一下。"),
    (
        "jiao'jie'ji'lu",
        "交接记录",
        "交接记录请注明尚未完成的事项。",
    ),
    (
        "jiao'fu'cheng'guo",
        "交付成果",
        "交付成果请按清单逐项检查。",
    ),
];

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn engine(language: &str) -> XRTabletImeEngine {
    let mut engine = XRTabletImeEngine::new(EngineConfig {
        default_language: language.into(),
        ..Default::default()
    });
    engine.enable_ibus_candidate_mix();
    engine
}

fn has(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) {
    assert!(
        ime.candidates().iter().take(PAGE_SIZE).any(|candidate| {
            candidate.text == text
                && candidate.kind == kind
                && candidate.source == CandidateSource::Local
        }),
        "missing {text:?} ({kind:?}) on page one: {:?}",
        ime.candidates()
    );
}

fn check(ime: &XRTabletImeEngine, seed: &str, word: &str, sentence: &str) {
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
    assert!(ime.snapshot().committed_text.is_empty());
    has(ime, word, CandidateKind::Word);
    has(ime, sentence, CandidateKind::Sentence);
}

fn commit(ime: &mut XRTabletImeEngine, expected: &str) {
    let index = ime
        .candidates()
        .iter()
        .position(|c| c.text == expected)
        .unwrap();
    ime.select_candidate(index);
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(expected));
}

struct UnavailableProvider(LlmProviderError);
impl LlmCompletionProvider for UnavailableProvider {
    fn provider_id(&self) -> &str {
        "pack-quality-synthetic-error"
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
fn recommended_collections_keep_word_sentence_and_fallback_workflows() {
    // One test/process owns the immutable catalog. Never initialize it from the
    // user's registry or set process-wide environment variables in a test.
    let path = std::env::temp_dir().join(format!("suzaku-pack-quality-{}", std::process::id()));
    fs::create_dir(&path).unwrap();
    // Only claim cleanup ownership after exclusive directory creation succeeds.
    let fixture = Fixture(path);
    let store = PackStore::new(fixture.0.join("packs")).unwrap();
    let collections = packs::recommended();
    for collection in &collections {
        store.install(collection.clone(), false).unwrap();
    }
    let report = packs::initialize(&store).unwrap();
    assert_eq!(report.loaded.len(), 14);
    assert!(report.errors.is_empty(), "{report:?}");

    // New built-in daily expressions must coexist with the old cooking pack,
    // including its longer context. Do not change a shipped pack to make a new
    // built-in key collision disappear; these expectations are independent.
    for (language, seed, word, sentence) in [
        (
            "en",
            "I’d rather st",
            "I’d rather stay",
            "I’d rather stay here.",
        ),
        (
            "en",
            "  i  prefer  the  se",
            "  i  prefer  the  second",
            "  i  prefer  the  second one.",
        ),
        (
            "zh-Hans",
            "WO GENG XI HUAN",
            "我更喜欢",
            "我更喜欢这个，简单又方便。",
        ),
        (
            "zh-Hans",
            "ru guo bu fang bia",
            "如果不方便",
            "如果不方便，我们就换个时间。",
        ),
        (
            "zh-Hans",
            "我更喜欢zhe ge",
            // Preserve the old longer Word alongside the new Sentence route.
            "我更喜欢这个词",
            "我更喜欢这个，简单又方便。",
        ),
        ("ja", "KONOMI", "好み", "好みに合わせて選んでください。"),
        ("ja", "jouke", "条件", "条件が合えば、こちらを選びます。"),
        ("ja", "ひかく", "比較", "比較してから決めたいです。"),
        ("ja", "yo  san", "予算", "予算に合わせて選びましょう。"),
        ("ja", "nihong ", "日本語", "日本語を勉強しています。"),
        ("ja", "JYUNB\t", "準備", "準備ができたら連絡します。"),
        (
            "ja",
            "jouk\u{a0}",
            "条件",
            "条件が合えば、こちらを選びます。",
        ),
        ("ja", "同じkanji", "同じ感じ", "同じ漢字"),
        (
            "en",
            "could you bring the cu",
            "could you bring the cups",
            "could you bring the cups?",
        ),
        (
            "en",
            "could you bring the measuring spo",
            "could you bring the measuring spoons",
            "could you bring the measuring spoons from the drawer?",
        ),
        (
            "en",
            "could you pass me the sa",
            "could you pass me the salt",
            "could you pass me the salt?",
        ),
        (
            "en",
            "please pass me the col",
            "please pass me the colander",
            "please pass me the colander from the cupboard.",
        ),
        (
            "zh-Hans",
            "fang hui yuan chu",
            "放回原处",
            "放回原处之前，记得擦干净。",
        ),
        (
            "zh-Hans",
            "gou'bu'gou",
            "够不够",
            "够不够，不够我再拿一点。",
        ),
        ("ja", "yotei", "予定", "予定が決まったら連絡します。"),
        ("ja", "JYUNBI", "準備", "準備ができたら連絡します。"),
        ("ja", "kaisatsu", "改札", "改札はどこですか。"),
    ] {
        for chosen in [word, sentence] {
            let mut ime = engine(language);
            ime.seed(seed);
            check(&ime, seed, word, sentence);
            commit(&mut ime, chosen);
            assert_eq!(ime.snapshot().committed_text, chosen);
            assert!(ime.undo().unwrap().committed_text.is_empty());
        }
    }

    // Sentence progress uses the active data catalog, not a built-in-only
    // trigger table. These rail/daily expectations stay fixed while all 14 packs
    // are enabled; a word followed by a particle is not mislabeled as a word here.
    for (seed, sentence) in [
        ("好みni", "好みに合わせて選んでください。"),
        ("条件ga", "条件が合えば、こちらを選びます。"),
        ("準備gadeki", "準備ができたら連絡します。"),
        ("JYUNBI GADEKI", "準備ができたら連絡します。"),
        ("kaisatsuha", "改札はどこですか。"),
        ("改札は", "改札はどこですか。"),
        ("かいさつは", "改札はどこですか。"),
        ("kippuwo", "切符を買いたいです。"),
        ("切符を", "切符を買いたいです。"),
        ("きっぷを", "切符を買いたいです。"),
        ("shuudennha", "終電は何時ですか。"),
        ("shuudenha", "終電は何時ですか。"),
        ("終電は", "終電は何時ですか。"),
        ("しゅうでんは", "終電は何時ですか。"),
    ] {
        for chosen in [sentence, seed] {
            let mut ime = engine("ja");
            ime.seed(seed);
            assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
            assert_eq!(ime.snapshot().seed_text, seed);
            assert!(ime.snapshot().committed_text.is_empty());
            assert!(ime.candidates().len() <= 12);
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
                    .collect::<std::collections::HashSet<_>>()
                    .len()
            );
            assert!(ime.candidates().iter().any(|candidate| {
                candidate.text == seed && candidate.kind == CandidateKind::Literal
            }));
            has(&ime, sentence, CandidateKind::Sentence);
            commit(&mut ime, chosen);
            assert_eq!(ime.snapshot().committed_text, chosen);
            assert!(ime.candidates().is_empty());
            let repeated = ime.commit(CommitOptions { force: true });
            assert!(!repeated.ok);
            assert!(repeated.text.is_none());
            assert_eq!(ime.snapshot().committed_text, chosen);
            assert!(ime.undo().unwrap().committed_text.is_empty());
            assert!(ime.undo().is_none());
        }
    }

    // Every authored form is usable, not just the examples advertised below.
    for collection in collections.iter().filter(|pack| {
        pack.manifest.topics.iter().any(|topic| {
            matches!(
                topic.as_str(),
                "study" | "cooking" | "lodging" | "collaboration"
            )
        })
    }) {
        for layer in collection.lexicon.word_layers() {
            for word in &layer.words {
                assert!(
                    suzaku_map::languages::english::is_known_english_word(word),
                    "{word}"
                );
            }
        }
        for entry in collection.lexicon.readings() {
            let mut ime = engine("zh-Hans");
            ime.seed(entry.reading.replace('\'', " "));
            assert!(
                ime.candidates()
                    .iter()
                    .any(|c| c.text == entry.text && c.kind == CandidateKind::Word),
                "{} -> {}: {:?}",
                entry.reading,
                entry.text,
                ime.candidates()
            );
        }
    }

    for (language, cases) in [("en", ENGLISH), ("zh-Hans", CHINESE)] {
        for &(reading, word, sentence) in cases {
            let spellings = if language == "zh-Hans" {
                vec![
                    reading.into(),
                    reading.replace('\'', " "),
                    reading.replace('\'', ""),
                    reading.to_uppercase(),
                ]
            } else {
                vec![reading.into()]
            };
            for seed in spellings {
                let mut ime = engine(language);
                ime.seed(&seed);
                assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
                check(&ime, &seed, word, sentence);
                commit(&mut ime, sentence);
            }
            // Editable adopted drafts, with preserved literal padding and long
            // prefixes. Actual numeric adoption/undo is tested in native IBus.
            for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
                for padding in [" ", "  ", "\u{3000}"] {
                    let mut ime = engine(language);
                    let seed = format!("{prefix}{word}{padding}");
                    ime.seed(&seed);
                    let suffix = &sentence[word.len()..];
                    let suffix = if language == "en" {
                        suffix.trim_start()
                    } else {
                        suffix
                    };
                    let expected = format!("{seed}{suffix}");
                    assert!(ime.snapshot().committed_text.is_empty());
                    assert!(ime.candidates().iter().any(|c| c.text == seed));
                    has(&ime, &expected, CandidateKind::Sentence);
                    commit(&mut ime, &expected);
                }
            }
        }
        // Errors are typed, synthetic provider failures, not network evidence.
        for error in [
            LlmProviderError::NoLocalModel,
            LlmProviderError::Unavailable,
            LlmProviderError::Timeout,
            LlmProviderError::HttpStatus(503),
        ] {
            // Each group of eight covers one independently selectable theme.
            for &(seed, word, sentence) in cases.iter().step_by(8) {
                let mut ime = engine(language);
                ime.configure_prediction(Some(Arc::new(UnavailableProvider(error.clone()))));
                ime.seed(seed);
                check(&ime, seed, word, sentence);
                let local = ime.candidates().to_vec();
                let deadline = Instant::now() + Duration::from_secs(2);
                while ime.prediction_pending() && Instant::now() < deadline {
                    ime.poll_prediction();
                    std::thread::sleep(Duration::from_millis(2));
                }
                assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
                assert_eq!(ime.prediction_error(), Some(&error));
                assert_eq!(ime.candidates(), local);
                commit(&mut ime, sentence);
            }
        }
    }

    for (language, seed, word, sentence) in [
        (
            "en",
            "  Could  you  store  our  lugg",
            "  Could  you  store  our  luggage",
            "  Could  you  store  our  luggage until this afternoon?",
        ),
        (
            "en",
            "Let’s agree on the next mile",
            "Let’s agree on the next milestone",
            "Let’s agree on the next milestone before we finish.",
        ),
        (
            "en",
            "could you store our luggage  until this after",
            "could you store our luggage  until this afternoon",
            "could you store our luggage  until this afternoon?",
        ),
        (
            "en",
            "please update the project roadmap  before our next mee",
            "please update the project roadmap  before our next meeting",
            "please update the project roadmap  before our next meeting.",
        ),
        (
            "zh-Hans",
            "xing li ji c",
            "行李寄存",
            "行李寄存可以到下午吗？",
        ),
        (
            "zh-Hans",
            "XING LI JI C \t",
            "行李寄存",
            "行李寄存可以到下午吗？",
        ),
        ("zh-Hans", "zhong w\u{a0}", "中文", "中文输入很方便。"),
        (
            "zh-Hans",
            "发音 ke y\u{3000}",
            "发音 可以",
            "发音 可以再示范一下吗？",
        ),
        (
            "zh-Hans",
            "xiang mu lu xian t",
            "项目路线图",
            "项目路线图请在下次会议前更新。",
        ),
        // Coexistence must keep established built-in and 0.8.0 pack examples.
        (
            "zh-Hans",
            "hui yi ji yao",
            "会议纪要",
            "会议纪要已经发到群里了。",
        ),
        (
            "zh-Hans",
            "yan shou biao zhun",
            "验收标准",
            "验收标准已经补充到文档里了。",
        ),
        (
            "en",
            "please save a co",
            "please save a copy",
            "please save a copy before closing.",
        ),
        (
            "en",
            "please print it on bo",
            "please print it on both",
            "please print it on both sides.",
        ),
        (
            "zh-Hans",
            "ping mu jie tu",
            "屏幕截图",
            "屏幕截图稍后发给你。",
        ),
        (
            "zh-Hans",
            "xu jie tu shu",
            "续借图书",
            "续借图书可以在网上办理吗？",
        ),
        (
            "en",
            "please bring your bino",
            "please bring your binoculars",
            "please bring your binoculars.",
        ),
        (
            "zh-Hans",
            "xing kong guan ce",
            "星空观测",
            "星空观测安排在周末晚上。",
        ),
        ("ja", "kaisatsu", "改札", "改札はどこですか。"),
        (
            "en",
            "  Please  annotate  this  para",
            "  Please  annotate  this  paragraph",
            "  Please  annotate  this  paragraph before our discussion.",
        ),
        (
            "en",
            "Let’s prepare the mari",
            "Let’s prepare the marinade",
            "Let’s prepare the marinade in a separate bowl.",
        ),
        (
            "en",
            "please annotate this paragraph  before our disc",
            "please annotate this paragraph  before our discussion",
            "please annotate this paragraph  before our discussion.",
        ),
        (
            "en",
            "please pass me the colander  from the cup",
            "please pass me the colander  from the cupboard",
            "please pass me the colander  from the cupboard.",
        ),
        (
            "zh-Hans",
            "yue du pi zh",
            "阅读批注",
            "阅读批注请写在页边空白处。",
        ),
        (
            "zh-Hans",
            "shi cai qing d",
            "食材清单",
            "食材清单先按菜谱整理一下。",
        ),
    ] {
        let mut ime = engine(language);
        ime.seed(seed);
        check(&ime, seed, word, sentence);
        commit(&mut ime, sentence);
    }
}
