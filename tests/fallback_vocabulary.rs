//! Authored fallback scenarios, not a model-quality or corpus-accuracy benchmark.
use std::sync::Arc;
use std::time::{Duration, Instant};
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};
use suzaku_map::languages::llm::{
    LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
};

const ENGLISH: &[(&str, &str, &str)] = &[
    (
        "could you speak more slo",
        "could you speak more slowly",
        "could you speak more slowly?",
    ),
    (
        "please resend the atta",
        "please resend the attachment",
        "please resend the attachment.",
    ),
    (
        "the network is temp",
        "the network is temporarily",
        "the network is temporarily unavailable.",
    ),
    (
        "i haven't received the conf",
        "i haven't received the confirmation",
        "i haven't received the confirmation yet.",
    ),
    (
        "where is the nearest rest",
        "where is the nearest restroom",
        "where is the nearest restroom?",
    ),
    (
        "please do not over",
        "please do not overwrite",
        "please do not overwrite this file.",
    ),
    (
        "i'll be there sho",
        "i'll be there shortly",
        "i'll be there shortly.",
    ),
    (
        "the file has not been sa",
        "the file has not been saved",
        "the file has not been saved.",
    ),
    (
        "could i have a rece",
        "could i have a receipt",
        "could i have a receipt?",
    ),
    (
        "could you confirm the loc",
        "could you confirm the location",
        "could you confirm the location?",
    ),
    (
        "sorry for the mis",
        "sorry for the misunderstanding",
        "sorry for the misunderstanding.",
    ),
    (
        "i'd like to collect my par",
        "i'd like to collect my parcel",
        "i'd like to collect my parcel.",
    ),
];

const CHINESE: &[(&str, &str, &str)] = &[
    ("qing'shao'deng", "请稍等", "请稍等，我查一下。"),
    ("mei'shou'dao", "没收到", "没收到，可以重新发一下吗？"),
    ("zai'shuo'yi'bian", "再说一遍", "再说一遍可以吗？"),
    ("bu'tai'que'ding", "不太确定", "不太确定，我再核对一下。"),
    ("li'xian'mo'shi", "离线模式", "离线模式下仍然可以输入。"),
    ("fu'wu'bu'ke'yong", "服务不可用", "服务不可用，请稍后再试。"),
    ("bao'cun'shi'bai", "保存失败", "保存失败，请先保留草稿。"),
    ("xi'shou'jian", "洗手间", "洗手间在哪里？"),
    ("chong'dian'qi", "充电器", "充电器可以借用一下吗？"),
    ("tui'kuan", "退款", "退款大概需要多久？"),
    ("bao'liu'yuan'wen", "保留原文", "保留原文，不要自动替换。"),
    ("ben'di'ci'ku", "本地词库", "本地词库提供基础词句候选。"),
];

const CONVERSATION_ENGLISH: &[(&str, &str, &str)] = &[
    (
        "would you mind checking the ava",
        "would you mind checking the availability",
        "would you mind checking the availability?",
    ),
    (
        "are you available tom",
        "are you available tomorrow",
        "are you available tomorrow afternoon?",
    ),
    (
        "the meeting has been resc",
        "the meeting has been rescheduled",
        "the meeting has been rescheduled for tomorrow.",
    ),
    (
        "please grant me acc",
        "please grant me access",
        "please grant me access to this document.",
    ),
    (
        "please preserve the forma",
        "please preserve the formatting",
        "please preserve the formatting.",
    ),
    (
        "i've attached the rev",
        "i've attached the revised",
        "i've attached the revised document.",
    ),
    (
        "my flight has been del",
        "my flight has been delayed",
        "my flight has been delayed.",
    ),
    (
        "i'd prefer an ais",
        "i'd prefer an aisle",
        "i'd prefer an aisle seat.",
    ),
    (
        "where can i collect my bag",
        "where can i collect my baggage",
        "where can i collect my baggage?",
    ),
    (
        "does this contain dai",
        "does this contain dairy",
        "does this contain dairy?",
    ),
    (
        "please put the leftovers in the ref",
        "please put the leftovers in the refrigerator",
        "please put the leftovers in the refrigerator.",
    ),
    (
        "let's make a shop",
        "let's make a shopping",
        "let's make a shopping list.",
    ),
];

const CONVERSATION_CHINESE: &[(&str, &str, &str)] = &[
    ("shi'jian'an'pai", "时间安排", "时间安排可以再调整一下。"),
    ("hui'yi'lian'jie", "会议链接", "会议链接已经发到群里了。"),
    (
        "fang'wen'quan'xian",
        "访问权限",
        "访问权限不足，麻烦帮我开通。",
    ),
    ("dai'shen'he", "待审核", "待审核的内容我已经提交了。"),
    ("cuo'wu'ti'shi", "错误提示", "错误提示能发个截图吗？"),
    (
        "wang'luo'bu'wen'ding",
        "网络不稳定",
        "网络不稳定，我重新连接一下。",
    ),
    ("sui'shen'xing'li", "随身行李", "随身行李有重量限制吗？"),
    ("huan'cheng'shi'jian", "换乘时间", "换乘时间够用吗？"),
    ("fa'ding'wei", "发定位", "发定位给我，我跟着导航走。"),
    ("bu'yao'xiang'cai", "不要香菜", "不要香菜，谢谢。"),
    ("gou'wu'qing'dan", "购物清单", "购物清单我已经列好了。"),
    ("shui'dian'fei", "水电费", "水电费这个月已经交了。"),
];

const ESSENTIALS_ENGLISH: &[(&str, &str, &str)] = &[
    (
        "would tomorrow mor",
        "would tomorrow morning",
        "would tomorrow morning work for you?",
    ),
    (
        "please confirm the exact da",
        "please confirm the exact date",
        "please confirm the exact date.",
    ),
    (
        "when does the subscription exp",
        "when does the subscription expire",
        "when does the subscription expire?",
    ),
    (
        "i was charged tw",
        "i was charged twice",
        "i was charged twice for this order.",
    ),
    (
        "the delivery address is inco",
        "the delivery address is incorrect",
        "the delivery address is incorrect.",
    ),
    (
        "the parcel arrived dam",
        "the parcel arrived damaged",
        "the parcel arrived damaged.",
    ),
    (
        "could you send me a rep",
        "could you send me a replacement",
        "could you send me a replacement?",
    ),
    (
        "please email me the inv",
        "please email me the invoice",
        "please email me the invoice.",
    ),
    (
        "is this still under warr",
        "is this still under warranty",
        "is this still under warranty?",
    ),
    (
        "i hope you have a rel",
        "i hope you have a relaxing",
        "i hope you have a relaxing evening.",
    ),
    (
        "i completely understand your con",
        "i completely understand your concern",
        "i completely understand your concern.",
    ),
    (
        "could you give me an exa",
        "could you give me an example",
        "could you give me an example?",
    ),
];

const ESSENTIALS_CHINESE: &[(&str, &str, &str)] = &[
    ("ming'tian'xia'wu", "明天下午", "明天下午可以安排。"),
    ("xia'zhou'yi", "下周一", "下周一我们再联系。"),
    ("ri'cheng'an'pai", "日程安排", "日程安排已经更新了。"),
    ("zi'dong'xu'fei", "自动续费", "自动续费可以关闭吗？"),
    ("chong'fu'kou'kuan", "重复扣款", "重复扣款了，请帮我核对。"),
    ("ding'dan'zhuang'tai", "订单状态", "订单状态一直没有更新。"),
    ("wu'liu'xin'xi", "物流信息", "物流信息暂时没有更新。"),
    ("dian'zi'fa'piao", "电子发票", "电子发票请发到我的邮箱。"),
    ("tui'kuan'shen'qing", "退款申请", "退款申请已经提交了。"),
    (
        "shang'pin'po'sun",
        "商品破损",
        "商品破损了，我把照片发给你。",
    ),
    ("xie'xie'guan'xin", "谢谢关心", "谢谢关心，我这边一切都好。"),
    ("lu'shang'xiao'xin", "路上小心", "路上小心，不用着急。"),
];

const CLARITY_ENGLISH: &[(&str, &str, &str)] = &[
    (
        "what does this me",
        "what does this mean",
        "what does this mean in this context?",
    ),
    (
        "how do you pron",
        "how do you pronounce",
        "how do you pronounce this word?",
    ),
    (
        "could you be more spec",
        "could you be more specific",
        "could you be more specific?",
    ),
    (
        "could you check the gram",
        "could you check the grammar",
        "could you check the grammar?",
    ),
    (
        "let me put it an",
        "let me put it another",
        "let me put it another way.",
    ),
    (
        "if i understand corr",
        "if i understand correctly",
        "if i understand correctly, we should wait.",
    ),
    (
        "thanks for pointing th",
        "thanks for pointing that",
        "thanks for pointing that out.",
    ),
    (
        "please summarize the ma",
        "please summarize the main",
        "please summarize the main points.",
    ),
];

const CLARITY_CHINESE: &[(&str, &str, &str)] = &[
    ("zhe'ge'ci", "这个词", "这个词是什么意思？"),
    ("fa'yin", "发音", "发音可以再示范一下吗？"),
    ("yu'fa'cuo'wu", "语法错误", "语法错误已经改好了。"),
    ("ju'ge'li'zi", "举个例子", "举个例子会更容易理解。"),
    (
        "huan'ju'hua'shuo",
        "换句话说",
        "换句话说，我们还需要一些时间。",
    ),
    (
        "wo'xiang'que'ren'yi'xia",
        "我想确认一下",
        "我想确认一下具体要求。",
    ),
    ("gen'jin'yi'xia", "跟进一下", "跟进一下这个问题的进展。"),
    ("shao'hou'hui'fu", "稍后回复", "稍后回复你具体结果。"),
];

const DIGITAL_ENGLISH: &[(&str, &str, &str)] = &[
    (
        "i forgot my pass",
        "i forgot my password",
        "i forgot my password.",
    ),
    (
        "i'll send you a scre",
        "i'll send you a screenshot",
        "i'll send you a screenshot.",
    ),
    (
        "please save a co",
        "please save a copy",
        "please save a copy before closing.",
    ),
    (
        "please turn on the cap",
        "please turn on the captions",
        "please turn on the captions.",
    ),
    (
        "could you make the text lar",
        "could you make the text larger",
        "could you make the text larger?",
    ),
    (
        "i'm running out of st",
        "i'm running out of storage",
        "i'm running out of storage.",
    ),
    (
        "the verification code has exp",
        "the verification code has expired",
        "the verification code has expired.",
    ),
    (
        "i can't sign i",
        "i can't sign in",
        "i can't sign in to my account.",
    ),
];

const DIGITAL_CHINESE: &[(&str, &str, &str)] = &[
    ("ping'mu'jie'tu", "屏幕截图", "屏幕截图稍后发给你。"),
    ("chong'zhi'mi'ma", "重置密码", "重置密码的邮件还没收到。"),
    ("yan'zheng'ma", "验证码", "验证码已经过期了。"),
    ("wen'jian'fu'jian", "文件附件", "文件附件已经上传了。"),
    ("fu'zhi'lian'jie", "复制链接", "复制链接后发给我就好。"),
    (
        "cun'chu'kong'jian",
        "存储空间",
        "存储空间不足，请先检查一下。",
    ),
    ("zi'ti'da'xiao", "字体大小", "字体大小可以在设置里调整。"),
    ("li'xian'shi'yong", "离线使用", "离线使用时不需要连接网络。"),
];

const HOME_ENGLISH: &[(&str, &str, &str)] = &[
    (
        "where did i leave my ke",
        "where did i leave my keys",
        "where did i leave my keys?",
    ),
    (
        "please close the win",
        "please close the window",
        "please close the window before leaving.",
    ),
    (
        "please take out the rub",
        "please take out the rubbish",
        "please take out the rubbish before you leave.",
    ),
    (
        "the kitchen tap is lea",
        "the kitchen tap is leaking",
        "the kitchen tap is leaking.",
    ),
    (
        "could you lend me a scre",
        "could you lend me a screwdriver",
        "could you lend me a screwdriver?",
    ),
    (
        "i need to replace the lightb",
        "i need to replace the lightbulb",
        "i need to replace the lightbulb.",
    ),
    (
        "please hang up the lau",
        "please hang up the laundry",
        "please hang up the laundry outside.",
    ),
    (
        "the spare key is un",
        "the spare key is under",
        "the spare key is under the doormat.",
    ),
];

const HOME_CHINESE: &[(&str, &str, &str)] = &[
    ("bei'yong'yao'shi", "备用钥匙", "备用钥匙放在抽屉里。"),
    (
        "wang'dai'yu'san",
        "忘带雨伞",
        "忘带雨伞了，可以借我一把吗？",
    ),
    ("guan'hao'men'chuang", "关好门窗", "关好门窗再出门。"),
    (
        "shui'long'tou'lou'shui",
        "水龙头漏水",
        "水龙头漏水了，需要预约维修。",
    ),
    (
        "xi'yi'ji'lou'shui",
        "洗衣机漏水",
        "洗衣机漏水了，需要请人检查。",
    ),
    ("jie'ge'luo'si'dao", "借个螺丝刀", "借个螺丝刀可以吗？"),
    (
        "men'ling'huai'le",
        "门铃坏了",
        "门铃坏了，到了请给我打电话。",
    ),
    ("yu'yue'wei'xiu", "预约维修", "预约维修的时间已经确认了。"),
];

const ERRANDS_ENGLISH: &[(&str, &str, &str)] = &[
    (
        "where is the nearest post off",
        "where is the nearest post office",
        "where is the nearest post office?",
    ),
    (
        "the pickup point is clo",
        "the pickup point is closed",
        "the pickup point is closed today.",
    ),
    (
        "i'd like to renew this bo",
        "i'd like to renew this book",
        "i'd like to renew this book.",
    ),
    (
        "when is this book du",
        "when is this book due",
        "when is this book due?",
    ),
    (
        "could you print this fo",
        "could you print this form",
        "could you print this form?",
    ),
    (
        "please print it on bo",
        "please print it on both",
        "please print it on both sides.",
    ),
    (
        "can i collect it tom",
        "can i collect it tomorrow",
        "can i collect it tomorrow afternoon?",
    ),
    (
        "where is the lost pro",
        "where is the lost property",
        "where is the lost property office?",
    ),
];

const ERRANDS_CHINESE: &[(&str, &str, &str)] = &[
    ("ji'ge'kuai'di", "寄个快递", "寄个快递，需要填哪些信息？"),
    ("qu'jian'ma", "取件码", "取件码还没有收到。"),
    (
        "chong'xin'pai'song",
        "重新派送",
        "重新派送可以安排在明天吗？",
    ),
    ("xu'jie'tu'shu", "续借图书", "续借图书可以在网上办理吗？"),
    ("gui'huan'tu'shu", "归还图书", "归还图书可以放进还书箱。"),
    ("jie'shu'zheng", "借书证", "借书证忘带了，可以用电子版吗？"),
    ("shuang'mian'da'yin", "双面打印", "双面打印可以节省纸张。"),
    ("ren'ling'shi'wu", "认领失物", "认领失物需要提供哪些信息？"),
];

fn engine(language: &str) -> XRTabletImeEngine {
    let mut engine = XRTabletImeEngine::new(EngineConfig {
        default_language: language.into(),
        ..Default::default()
    });
    engine.enable_ibus_candidate_mix();
    engine
}

fn assert_fallback(ime: &XRTabletImeEngine, seed: &str, word: &str, sentence: &str) {
    assert!(ime.candidates().len() <= 12);
    assert!(ime.candidates().iter().any(|c| c.text == seed));
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local)
    );
    for (text, kind) in [
        (word, CandidateKind::Word),
        (sentence, CandidateKind::Sentence),
    ] {
        assert!(
            ime.candidates()
                .iter()
                .take(PAGE_SIZE)
                .any(|c| c.text == text && c.kind == kind),
            "missing {text:?} ({kind:?}) for {seed:?}: {:?}",
            ime.candidates()
        );
    }
}

fn confirm(ime: &mut XRTabletImeEngine, text: &str) {
    let index = ime
        .candidates()
        .iter()
        .position(|c| c.text == text)
        .unwrap();
    ime.select_candidate(index);
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
}

#[test]
fn offline_english_fallback_keeps_words_sentences_and_exact_commits() {
    for &(seed, word, sentence) in ENGLISH
        .iter()
        .chain(CONVERSATION_ENGLISH)
        .chain(ESSENTIALS_ENGLISH)
        .chain(CLARITY_ENGLISH)
        .chain(DIGITAL_ENGLISH)
        .chain(HOME_ENGLISH)
        .chain(ERRANDS_ENGLISH)
    {
        let mut ime = engine("en");
        ime.seed(seed);
        assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
        assert_fallback(&ime, seed, word, sentence);
        assert_eq!(ime.candidates()[0].text, seed);
        confirm(&mut ime, sentence);
    }
}

#[test]
fn offline_chinese_fallback_preserves_spelling_forms_and_exact_commits() {
    for &(reading, word, sentence) in CHINESE
        .iter()
        .chain(CONVERSATION_CHINESE)
        .chain(ESSENTIALS_CHINESE)
        .chain(CLARITY_CHINESE)
        .chain(DIGITAL_CHINESE)
        .chain(HOME_CHINESE)
        .chain(ERRANDS_CHINESE)
    {
        for seed in [
            reading.replace('\'', ""),
            reading.replace('\'', " "),
            reading.into(),
            reading.to_uppercase(),
        ] {
            let mut ime = engine("zh-Hans");
            ime.seed(&seed);
            assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
            assert_fallback(&ime, &seed, word, sentence);
            confirm(&mut ime, sentence);
        }
    }
}

struct UnavailableProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for UnavailableProvider {
    fn provider_id(&self) -> &str {
        "synthetic-fallback-error"
    }
    fn generate(&self, _: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        Vec::new()
    }
    fn generate_checked(
        &self,
        _: &LlmCompletionRequest,
    ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
        self.0.clone().map_or_else(|| Ok(Vec::new()), Err)
    }
}

#[test]
fn daily_expressions_stay_available_when_models_are_missing_failed_or_empty() {
    let cases = [
        (
            "en",
            "could you put that in si",
            "could you put that in simpler",
            "could you put that in simpler words?",
        ),
        (
            "en",
            "let me say it an",
            "let me say it another",
            "let me say it another way.",
        ),
        (
            "zh-Hans",
            "wo li jie de shi",
            "我理解的是",
            "我理解的是这个意思，你看看对不对。",
        ),
        (
            "zh-Hans",
            "wo que ren yi xi",
            "我确认一下",
            "我确认一下时间，再回复你。",
        ),
        ("ja", "shitsumon", "質問", "質問があれば教えてください。"),
        ("ja", "hosoku", "補足", "補足があれば教えてください。"),
        (
            "en",
            "what do you like to re",
            "what do you like to read",
            "what do you like to read in your free time?",
        ),
        (
            "en",
            "this weekend i'm planning to re",
            "this weekend i'm planning to relax",
            "this weekend i'm planning to relax at home.",
        ),
        (
            "zh-Hans",
            "zhou mo you kong ma",
            "周末有空吗",
            "周末有空吗，一起出去走走吧。",
        ),
        (
            "zh-Hans",
            "chu qu zou yi zou",
            "出去走一走",
            "出去走一走，换个心情也挺好。",
        ),
        ("ja", "sanpo", "散歩", "散歩に行きませんか。"),
        ("ja", "ryouri", "料理", "料理ができたら呼んでください。"),
        (
            "en",
            "i'm having trouble fi",
            "i'm having trouble finding",
            "i'm having trouble finding the right place.",
        ),
        (
            "en",
            "we're almost do",
            "we're almost done",
            "we're almost done, just one more step.",
        ),
        (
            "zh-Hans",
            "wo zhe bian hao le",
            "我这边好了",
            "我这边好了，你那边怎么样？",
        ),
        (
            "zh-Hans",
            "chong xin lai guo",
            "重新来过",
            "重新来过也没关系，我们一起试试。",
        ),
        ("ja", "henshin", "返信", "返信ありがとうございます。"),
        (
            "ja",
            "junbichuu",
            "準備中",
            "準備中です、少しお待ちください。",
        ),
        ("zh-Hans", "ZHONG W ", "中文", "中文输入很方便。"),
        ("zh-Hans", "shu ru f\t", "输入法", "输入法支持多种语言。"),
        (
            "zh-Hans",
            "发音 ke y\u{3000}",
            "发音 可以",
            "发音 可以再示范一下吗？",
        ),
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
            // A complete multiword conversion is Sentence; this longer known
            // word completion must coexist with the whole-draft continuation.
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
            "ja",
            "準備gadeki",
            "準備ができれば",
            "準備ができたら連絡します。",
        ),
        (
            "ja",
            "sumimasen",
            "すみません",
            "すみません、もう一度お願いします。",
        ),
        (
            "ja",
            "onegaishimasu",
            "お願いします",
            "お願いします。終わったら教えてください。",
        ),
        ("ja", "yotei", "予定", "予定が決まったら連絡します。"),
        ("ja", "JYUNBI", "準備", "準備ができたら連絡します。"),
        (
            "en",
            "have you seen my ke",
            "have you seen my keys",
            "have you seen my keys?",
        ),
        (
            "en",
            "can i borrow your ch",
            "can i borrow your charger",
            "can i borrow your charger?",
        ),
        (
            "en",
            "There's a spare ch",
            "There's a spare charger",
            "There's a spare charger on the table.",
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
        (
            "zh-Hans",
            "hai sheng duo shao",
            "还剩多少",
            "还剩多少，我们先数一下。",
        ),
        (
            "en",
            "do you want to grab co",
            "do you want to grab coffee",
            "do you want to grab coffee?",
        ),
        (
            "en",
            "i hope you're fe",
            "i hope you're feeling",
            "i hope you're feeling better.",
        ),
        (
            "en",
            "i'll have to pass to",
            "i'll have to pass tonight",
            "i'll have to pass tonight, but thank you.",
        ),
        (
            "zh-Hans",
            "zhaoguhaoziji",
            "照顾好自己",
            "照顾好自己，记得按时吃饭。",
        ),
        (
            "zh-Hans",
            "woqujieni",
            "我去接你",
            "我去接你之前会先给你发消息。",
        ),
        (
            "zh-Hans",
            "xiexieyaoqing",
            "谢谢邀请",
            "谢谢邀请，我看看时间再回复你。",
        ),
        (
            "zh-Hans",
            "bangwokankan",
            "帮我看看",
            "帮我看看有没有漏掉什么。",
        ),
        (
            "zh-Hans",
            "fageinile",
            "发给你了",
            "发给你了，看看有没有收到。",
        ),
        (
            "zh-Hans",
            "xianmangnide",
            "先忙你的",
            "先忙你的，有空再回复就好。",
        ),
        (
            "en",
            "did you get my me",
            "did you get my message",
            "did you get my message?",
        ),
        (
            "en",
            "could you give me a ha",
            "could you give me a hand",
            "could you give me a hand?",
        ),
        (
            "en",
            "thanks for getting ba",
            "thanks for getting back",
            "thanks for getting back to me.",
        ),
        (
            "en",
            "that's really imp",
            "that's really impressive",
            "that's really impressive.",
        ),
        (
            "en",
            "I'm almost th",
            "I'm almost there",
            "I'm almost there.",
        ),
        ("zh-Hans", "zhenbucuo", "真不错", "真不错，下次还想再来。"),
        ("zh-Hans", "daojiale", "到家了", "到家了，给你报个平安。"),
        (
            "en",
            "please make it mi",
            "please make it mild",
            "please make it mild.",
        ),
        (
            "en",
            "it's getting co",
            "it's getting cold",
            "it's getting cold outside.",
        ),
        (
            "en",
            "can we change the ti",
            "can we change the time",
            "can we change the time?",
        ),
        ("zh-Hans", "shaotang", "少糖", "少糖就好，谢谢。"),
        ("zh-Hans", "daisan", "带伞", "带伞出门，免得淋雨。"),
        (
            "zh-Hans",
            "linshiyoushi",
            "临时有事",
            "临时有事，可能要晚一点。",
        ),
    ];
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (language, seed, word, sentence) in cases {
            let mut ime = engine(language);
            ime.configure_prediction(Some(Arc::new(UnavailableProvider(error.clone()))));
            ime.seed(seed);
            assert_fallback(&ime, seed, word, sentence);
            let local = ime.candidates().to_vec();
            let deadline = Instant::now() + Duration::from_secs(2);
            while ime.prediction_pending() && Instant::now() < deadline {
                ime.poll_prediction();
                std::thread::sleep(Duration::from_millis(2));
            }
            assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
            assert_eq!(
                ime.prediction_error(),
                Some(&error.clone().unwrap_or(LlmProviderError::NoCandidates))
            );
            assert_eq!(ime.candidates(), local, "{language} {error:?}");
            ime.configure_prediction(None);
            assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
            assert_fallback(&ime, seed, word, sentence);
            confirm(&mut ime, sentence);
        }
    }
}

#[test]
fn missing_failed_timed_out_and_empty_models_preserve_the_immediate_local_fallback() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (language, cases, added, essentials, clarity, latest, home, errands) in [
            (
                "en",
                ENGLISH,
                CONVERSATION_ENGLISH,
                ESSENTIALS_ENGLISH,
                CLARITY_ENGLISH,
                DIGITAL_ENGLISH,
                HOME_ENGLISH,
                ERRANDS_ENGLISH,
            ),
            (
                "zh-Hans",
                CHINESE,
                CONVERSATION_CHINESE,
                ESSENTIALS_CHINESE,
                CLARITY_CHINESE,
                DIGITAL_CHINESE,
                HOME_CHINESE,
                ERRANDS_CHINESE,
            ),
        ] {
            for &(seed, word, sentence) in cases
                .iter()
                .take(2)
                .chain(added.iter().take(1))
                .chain(essentials.iter().take(1))
                .chain(clarity.iter().take(1))
                .chain(latest.iter().take(1))
                .chain(home.iter().take(1))
                .chain(errands.iter().take(1))
            {
                let mut ime = engine(language);
                ime.configure_prediction(Some(Arc::new(UnavailableProvider(error.clone()))));
                ime.seed(seed);
                assert_fallback(&ime, seed, word, sentence);
                let local = ime.candidates().to_vec();
                let deadline = Instant::now() + Duration::from_secs(2);
                while ime.prediction_pending() && Instant::now() < deadline {
                    ime.poll_prediction();
                    std::thread::sleep(Duration::from_millis(2));
                }
                assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
                assert_eq!(
                    ime.prediction_error(),
                    Some(&error.clone().unwrap_or(LlmProviderError::NoCandidates))
                );
                assert_eq!(ime.candidates(), local, "{language} {error:?}");
                ime.configure_prediction(None);
                assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
                assert!(ime.prediction_error().is_none());
                assert_fallback(&ime, seed, word, sentence);
                confirm(&mut ime, sentence);
            }
        }
    }
}

#[test]
fn long_fallback_drafts_keep_the_prefix_and_do_not_require_a_model() {
    for (language, prefix, cases, added, essentials, clarity, latest, home, errands) in [
        (
            "en",
            "note ".repeat(60),
            ENGLISH,
            CONVERSATION_ENGLISH,
            ESSENTIALS_ENGLISH,
            CLARITY_ENGLISH,
            DIGITAL_ENGLISH,
            HOME_ENGLISH,
            ERRANDS_ENGLISH,
        ),
        (
            "zh-Hans",
            "你".repeat(260),
            CHINESE,
            CONVERSATION_CHINESE,
            ESSENTIALS_CHINESE,
            CLARITY_CHINESE,
            DIGITAL_CHINESE,
            HOME_CHINESE,
            ERRANDS_CHINESE,
        ),
    ] {
        for &(seed, word, sentence) in cases
            .iter()
            .take(2)
            .chain(added.iter().take(2))
            .chain(essentials.iter().take(2))
            .chain(clarity.iter().take(2))
            .chain(latest.iter().take(2))
            .chain(home.iter().take(2))
            .chain(errands.iter().take(2))
        {
            let mut ime = engine(language);
            let seed = format!("{prefix}{seed}");
            let word = format!("{prefix}{word}");
            let sentence = format!("{prefix}{sentence}");
            assert!(seed.chars().count() > 256);
            ime.seed(&seed);
            assert_fallback(&ime, &seed, &word, &sentence);
            confirm(&mut ime, &sentence);
        }
    }
}

#[test]
fn standalone_fallback_word_forms_are_visible_without_sentence_context() {
    for (seed, word) in [
        ("apologet", "apologetic"),
        ("hesit", "hesitate"),
        ("confus", "confused"),
        ("contributor", "contributors"),
        ("reope", "reopened"),
        ("synci", "syncing"),
        ("rerout", "rerouted"),
        ("transferr", "transferred"),
        ("refrigerat", "refrigerated"),
        ("chopst", "chopsticks"),
        ("misspel", "misspelled"),
        ("formatti", "formatting"),
        ("edita", "editable"),
        ("piny", "pinyin"),
        ("weekl", "weekly"),
        ("monthl", "monthly"),
        ("anniversar", "anniversaries"),
        ("believ", "believing"),
        ("realize", "realized"),
        ("suppos", "supposed"),
        ("meani", "meaning"),
        ("wante", "wanted"),
        ("offer", "offered"),
        ("shari", "sharing"),
        ("contactl", "contactless"),
        ("redeem", "redeemed"),
        ("eligib", "eligible"),
        ("withdra", "withdrawn"),
        ("detachab", "detachable"),
        ("stripe", "striped"),
        ("fluentl", "fluently"),
        ("textbo", "textbook"),
        ("workbo", "workbook"),
        ("grammaticall", "grammatically"),
        ("alphabeti", "alphabetical"),
        ("comparati", "comparative"),
        ("literall", "literally"),
        ("misinterprete", "misinterpreted"),
        ("reassessm", "reassessment"),
        ("contextu", "contextual"),
        ("graysca", "grayscale"),
        ("greysca", "greyscale"),
        ("passk", "passkey"),
        ("thumbna", "thumbnail"),
        ("redownlo", "redownload"),
        ("livestrea", "livestream"),
        ("permali", "permalink"),
        ("spreadsh", "spreadsheet"),
        ("scrollab", "scrollable"),
        ("taskb", "taskbar"),
        ("lightbu", "lightbulb"),
        ("screwdr", "screwdriver"),
        ("clothespi", "clothespin"),
        ("clothesli", "clothesline"),
        ("unclogg", "unclogged"),
        ("keyri", "keyring"),
        ("postmarke", "postmarked"),
        ("postbo", "postbox"),
        ("mailroo", "mailroom"),
        ("photocopie", "photocopier"),
        ("printou", "printout"),
        ("laminati", "laminating"),
        ("refilla", "refillable"),
        ("redeliv", "redelivery"),
    ] {
        let mut ime = engine("en");
        ime.seed(seed);
        assert_eq!(ime.candidates()[0].text, seed);
        assert!(
            ime.candidates().iter().take(PAGE_SIZE).any(|c| {
                c.text == word
                    && c.kind == CandidateKind::Word
                    && c.source == CandidateSource::Local
            }),
            "{seed} -> {word}: {:?}",
            ime.candidates()
        );
        confirm(&mut ime, word);
    }
}

#[test]
fn conversation_fallback_retains_literal_spacing_case_and_apostrophes() {
    for (seed, word, sentence) in [
        (
            "  Please  grant  me  acc",
            "  Please  grant  me  access",
            "  Please  grant  me  access to this document.",
        ),
        (
            "I've attached the rev",
            "I've attached the revised",
            "I've attached the revised document.",
        ),
        (
            "I’ve attached the rev",
            "I’ve attached the revised",
            "I’ve attached the revised document.",
        ),
        (
            "I’d prefer an ais",
            "I’d prefer an aisle",
            "I’d prefer an aisle seat.",
        ),
        (
            "  Please  email  me  the  inv",
            "  Please  email  me  the  invoice",
            "  Please  email  me  the  invoice.",
        ),
        (
            "I’d like to change the del",
            "I’d like to change the delivery",
            "I’d like to change the delivery address.",
        ),
        (
            "  If  I  understand  corr",
            "  If  I  understand  correctly",
            "  If  I  understand  correctly, we should wait.",
        ),
        (
            "I’m not familiar with th",
            "I’m not familiar with that",
            "I’m not familiar with that term.",
        ),
        (
            "  Please  save  a  co",
            "  Please  save  a  copy",
            "  Please  save  a  copy before closing.",
        ),
        (
            "I’ll send you a scre",
            "I’ll send you a screenshot",
            "I’ll send you a screenshot.",
        ),
        (
            "I can’t sign i",
            "I can’t sign in",
            "I can’t sign in to my account.",
        ),
        (
            "  Please  close  the  win",
            "  Please  close  the  window",
            "  Please  close  the  window before leaving.",
        ),
        (
            "  Where  did  I  leave  my  ke",
            "  Where  did  I  leave  my  keys",
            "  Where  did  I  leave  my  keys?",
        ),
    ] {
        let mut ime = engine("en");
        ime.seed(seed);
        assert_fallback(&ime, seed, word, sentence);
        confirm(&mut ime, sentence);
    }
}

#[test]
fn everyday_chinese_unfinished_syllables_offer_words_and_sentences() {
    for (seed, word, sentence) in [
        ("ming tian xia w", "明天下午", "明天下午可以安排。"),
        ("dian zi fa p", "电子发票", "电子发票请发到我的邮箱。"),
        ("chong fu kou k", "重复扣款", "重复扣款了，请帮我核对。"),
        ("zi dong xu f", "自动续费", "自动续费可以关闭吗？"),
        ("zhe ge c", "这个词", "这个词是什么意思？"),
        ("yu fa cuo w", "语法错误", "语法错误已经改好了。"),
        ("ju ge li z", "举个例子", "举个例子会更容易理解。"),
        ("shao hou hui f", "稍后回复", "稍后回复你具体结果。"),
        ("ping mu jie t", "屏幕截图", "屏幕截图稍后发给你。"),
        ("chong zhi mi m", "重置密码", "重置密码的邮件还没收到。"),
        ("wen jian fu j", "文件附件", "文件附件已经上传了。"),
        ("zi ti da x", "字体大小", "字体大小可以在设置里调整。"),
        ("bei yong yao sh", "备用钥匙", "备用钥匙放在抽屉里。"),
        ("guan hao men ch", "关好门窗", "关好门窗再出门。"),
        (
            "shui long tou lou sh",
            "水龙头漏水",
            "水龙头漏水了，需要预约维修。",
        ),
        ("yu yue wei x", "预约维修", "预约维修的时间已经确认了。"),
        ("ji ge kuai d", "寄个快递", "寄个快递，需要填哪些信息？"),
        ("xu jie tu sh", "续借图书", "续借图书可以在网上办理吗？"),
        ("shuang mian da y", "双面打印", "双面打印可以节省纸张。"),
        ("ren ling shi w", "认领失物", "认领失物需要提供哪些信息？"),
    ] {
        let mut ime = engine("zh-Hans");
        ime.seed(seed);
        assert_fallback(&ime, seed, word, sentence);
        confirm(&mut ime, sentence);
    }
}

#[test]
fn expanded_words_keep_sentence_continuations_after_adoption_and_literal_spaces() {
    // Reseeding represents the editable draft after host adoption, not a commit.
    // Actual numeric adoption/Backspace/Space is covered by the private IBus/GTK gates.
    for (language, cases) in [
        ("en", CLARITY_ENGLISH),
        ("zh-Hans", CLARITY_CHINESE),
        ("en", DIGITAL_ENGLISH),
        ("zh-Hans", DIGITAL_CHINESE),
        ("en", HOME_ENGLISH),
        ("zh-Hans", HOME_CHINESE),
        ("en", ERRANDS_ENGLISH),
        ("zh-Hans", ERRANDS_CHINESE),
    ] {
        // An all-ASCII Chinese draft may still be one unconverted Pinyin span;
        // only a Han/CJK boundary safely freezes its long prefix (draft.rs).
        let long_prefix = if language == "en" {
            "note ".repeat(60)
        } else {
            "前文。".repeat(90)
        };
        for prefix in [String::new(), long_prefix, "前文 café 😀。  ".repeat(40)] {
            for &(reading, word, sentence) in cases {
                // English words immediately followed by punctuation have no next
                // sentence token after Space. Chinese keeps authored padding verbatim.
                if language == "en" && !sentence[word.len()..].starts_with(' ') {
                    continue;
                }
                for padding in [" ", "  ", "\u{3000}"] {
                    let mut ime = engine(language);
                    ime.seed(format!("{prefix}{reading}"));
                    let adopted = format!("{prefix}{word}");
                    assert!(
                        ime.candidates().iter().any(|c| c.text == adopted),
                        "{language} {reading:?} -> {adopted:?}: {:?}",
                        ime.candidates()
                    );
                    let seed = format!("{adopted}{padding}");
                    ime.seed(&seed);
                    assert!(ime.snapshot().committed_text.is_empty());
                    assert!(ime.candidates().len() <= 12);
                    assert!(ime.candidates().iter().any(|c| c.text == seed));
                    let suffix = &sentence[word.len()..];
                    let suffix = if language == "en" {
                        suffix.trim_start()
                    } else {
                        suffix
                    };
                    let expected = format!("{seed}{suffix}");
                    assert!(
                        ime.candidates().iter().take(PAGE_SIZE).any(|c| {
                            c.text == expected
                                && c.kind == CandidateKind::Sentence
                                && c.source == CandidateSource::Local
                        }),
                        "{language} {seed:?} -> {expected:?}: {:?}",
                        ime.candidates()
                    );
                    confirm(&mut ime, &expected);
                }
            }
        }
    }
}

#[test]
fn expanded_english_keeps_completing_the_next_word_in_an_uncommitted_draft() {
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for (reading, adopted, next, word, sentence) in [
            (
                "Please print it on bo",
                "Please print it on both",
                "  si",
                "Please print it on both  sides",
                "Please print it on both  sides.",
            ),
            (
                "can i collect it tom",
                "can i collect it tomorrow",
                " after",
                "can i collect it tomorrow afternoon",
                "can i collect it tomorrow afternoon?",
            ),
            (
                "Please close the win",
                "Please close the window",
                "  before lea",
                "Please close the window  before leaving",
                "Please close the window  before leaving.",
            ),
            (
                "the spare key is un",
                "the spare key is under",
                " the door",
                "the spare key is under the doormat",
                "the spare key is under the doormat.",
            ),
            (
                "Please save a co",
                "Please save a copy",
                "  before clo",
                "Please save a copy  before closing",
                "Please save a copy  before closing.",
            ),
            (
                "I can’t sign i",
                "I can’t sign in",
                " to my acc",
                "I can’t sign in to my account",
                "I can’t sign in to my account.",
            ),
            (
                "How do you pron",
                "How do you pronounce",
                " this w",
                "How do you pronounce this word",
                "How do you pronounce this word?",
            ),
            (
                "what does this me",
                "what does this mean",
                " in this cont",
                "what does this mean in this context",
                "what does this mean in this context?",
            ),
            (
                "let me put it an",
                "let me put it another",
                "  w",
                "let me put it another  way",
                "let me put it another  way.",
            ),
            (
                "please summarize the ma",
                "please summarize the main",
                " poi",
                "please summarize the main points",
                "please summarize the main points.",
            ),
        ] {
            let mut ime = engine("en");
            ime.seed(format!("{prefix}{reading}"));
            let adopted = format!("{prefix}{adopted}");
            assert!(
                ime.candidates()
                    .iter()
                    .take(PAGE_SIZE)
                    .any(|c| c.text == adopted)
            );
            let seed = format!("{adopted}{next}");
            ime.seed(&seed);
            assert!(ime.snapshot().committed_text.is_empty());
            let word = format!("{prefix}{word}");
            let sentence = format!("{prefix}{sentence}");
            assert_fallback(&ime, &seed, &word, &sentence);
            confirm(&mut ime, &sentence);
        }
    }
}
