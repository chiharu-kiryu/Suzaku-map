//! Small offline bootstrap lexicon, not a replacement for a full Pinyin dictionary.
//! Unknown input is always preserved; an optional LLM can enrich these candidates.

use super::ranked_typed_candidates;
use crate::ime::candidate_mix::CandidateKind;
use crate::ime::{Candidate, LanguagePlugin};

#[derive(Default)]
pub struct ChineseLanguagePlugin;

impl LanguagePlugin for ChineseLanguagePlugin {
    fn id(&self) -> &str {
        "zh-Hans"
    }
    fn display_name(&self) -> &str {
        "简体中文"
    }
    fn normalize_seed(&self, input: &str) -> String {
        // Separators belong to the preedit and the lossless literal candidate.
        input.to_owned()
    }
    fn expand_token(&self, token: &str, _degraded: bool) -> Vec<String> {
        vec![token.into()]
    }
    fn build_candidates(&self, _parts: &[String], seed: &str, confidence: f32) -> Vec<Candidate> {
        ranked_typed_candidates(pinyin_choices(seed), confidence)
    }
    fn direct_candidates(&self, seed: &str, confidence: f32) -> Option<Vec<Candidate>> {
        Some(self.build_candidates(&[], seed, confidence))
    }
    fn commit_separator(&self) -> &str {
        ""
    }
}

// Original, deliberately small seed vocabulary. Longer phrases rank before syllable paths.
const PINYIN: &[(&str, &str)] = &[
    ("nihao", "你好"),
    ("nihaoma", "你好吗"),
    ("xiexie", "谢谢"),
    ("zaijian", "再见"),
    ("zaoshanghao", "早上好"),
    ("wanshanghao", "晚上好"),
    ("meiguanxi", "没关系"),
    ("duibuqi", "对不起"),
    ("qingwen", "请问"),
    ("keyi", "可以"),
    ("bukeyi", "不可以"),
    ("women", "我们"),
    ("nimen", "你们"),
    ("tamen", "他们"),
    ("zhongwen", "中文"),
    ("yingwen", "英文"),
    ("riwen", "日文"),
    ("zhongguo", "中国"),
    ("shijie", "世界"),
    ("shurufa", "输入法"),
    ("shuru", "输入"),
    ("houxuan", "候选"),
    ("lianxiang", "联想"),
    ("moxing", "模型"),
    ("yuyan", "语言"),
    ("zhichi", "支持"),
    ("gongneng", "功能"),
    ("jixu", "继续"),
    ("kaifa", "开发"),
    ("ceshi", "测试"),
    ("shezhi", "设置"),
    ("jintian", "今天"),
    ("mingtian", "明天"),
    ("zuotian", "昨天"),
    ("xianzai", "现在"),
    ("tianqi", "天气"),
    ("gongzuo", "工作"),
    ("xuexi", "学习"),
    ("xihuan", "喜欢"),
    ("pengyou", "朋友"),
    ("shijian", "时间"),
    ("wenti", "问题"),
    ("bangzhu", "帮助"),
    ("diannao", "电脑"),
    ("shouji", "手机"),
    ("beijing", "北京"),
    ("shanghai", "上海"),
    ("shenzhen", "深圳"),
    ("guangzhou", "广州"),
    ("xi'an", "西安"),
    ("wo", "我"),
    ("ni", "你"),
    ("ni", "呢"),
    ("ta", "他"),
    ("ta", "她"),
    ("men", "们"),
    ("hao", "好"),
    ("hao", "号"),
    ("shi", "是"),
    ("shi", "时"),
    ("de", "的"),
    ("le", "了"),
    ("ma", "吗"),
    ("ne", "呢"),
    ("bu", "不"),
    ("you", "有"),
    ("you", "又"),
    ("zai", "在"),
    ("ai", "爱"),
    ("hen", "很"),
    ("xiang", "想"),
    ("yao", "要"),
    ("qu", "去"),
    ("lai", "来"),
    ("kan", "看"),
    ("ting", "听"),
    ("shuo", "说"),
    ("qing", "请"),
    ("he", "和"),
    ("ye", "也"),
    ("dou", "都"),
    ("ke", "可"),
    ("yi", "以"),
    ("zhong", "中"),
    ("guo", "国"),
    ("ren", "人"),
    ("tian", "天"),
    ("qi", "气"),
    ("xin", "新"),
    ("da", "大"),
    ("xiao", "小"),
    ("duo", "多"),
    ("shao", "少"),
    ("lv", "绿"),
    ("nv", "女"),
    ("xian", "先"),
    ("xi", "西"),
    ("an", "安"),
];

fn normalize_pinyin(seed: &str) -> String {
    let lowered = seed.to_lowercase().replace("u:", "v").replace('ü', "v");
    let chars: Vec<_> = lowered.chars().collect();
    chars
        .iter()
        .enumerate()
        .filter_map(|(index, &ch)| {
            let tone = matches!(ch, '1'..='5')
                && index > 0
                && chars[index - 1].is_ascii_alphabetic()
                && chars.get(index + 1).is_none_or(|next| {
                    next.is_ascii_alphabetic() || next.is_whitespace() || *next == '\''
                });
            (!tone).then_some(if ch.is_whitespace() { '\'' } else { ch })
        })
        .collect()
}

pub(crate) fn is_dictionary_word(text: &str) -> bool {
    PINYIN.iter().any(|(_, word)| *word == text) && text != "你好吗"
}

pub(crate) fn mixed_candidates(
    seed: &str,
) -> Vec<(String, crate::ime::candidate_mix::CandidateKind)> {
    const CONTINUATIONS: &[(&str, &[&str])] = &[
        (
            "你好",
            &["你好，很高兴认识你。", "你好，请问有什么可以帮忙？"],
        ),
        ("谢谢", &["谢谢你的帮助。", "谢谢，辛苦了。"]),
        ("请问", &["请问现在方便吗？", "请问可以帮我一下吗？"]),
        ("我", &["我想了解一下。", "我可以帮忙。"]),
        ("你", &["你现在方便吗？", "你有什么建议？"]),
        ("我们", &["我们一起试试看。", "我们可以稍后讨论。"]),
        ("今天", &["今天天气很好。", "今天有什么安排？"]),
        ("明天", &["明天见。", "明天再讨论吧。"]),
        ("中文", &["中文输入很方便。", "中文和英文都可以输入。"]),
        (
            "输入法",
            &["输入法支持多种语言。", "输入法可以提供词句候选。"],
        ),
        ("继续", &["继续完善这个功能。", "继续下一步吧。"]),
        ("可以", &["可以帮我看一下吗？", "可以继续了。"]),
        ("学习", &["学习一门新的语言。", "学习需要不断练习。"]),
        ("测试", &["测试一下输入效果。", "测试已经完成。"]),
        ("再见", &["再见，下次再聊。"]),
        ("我喜欢北京", &["我喜欢北京的文化。", "我喜欢北京的美食。"]),
        ("我想学习中文", &["我想学习中文，请多指教。"]),
    ];
    let code = normalize_pinyin(seed);
    let mut output = Vec::new();
    if code.len() >= 2 && code.bytes().all(|ch| ch.is_ascii_lowercase()) {
        for (_, word) in PINYIN
            .iter()
            .filter(|(key, _)| key.starts_with(&code) && *key != code)
            .take(5)
        {
            output.push((
                (*word).to_owned(),
                if is_dictionary_word(word) {
                    CandidateKind::Word
                } else {
                    CandidateKind::Sentence
                },
            ));
        }
    }
    if let Some(primary) = pinyin_candidates(seed).first()
        && let Some((_, values)) = CONTINUATIONS.iter().find(|(word, _)| *word == primary)
    {
        output.extend(
            values
                .iter()
                .map(|text| ((*text).into(), CandidateKind::Sentence)),
        );
    }
    output
}

pub fn pinyin_candidates(seed: &str) -> Vec<String> {
    pinyin_choices(seed)
        .into_iter()
        .map(|(text, _)| text)
        .collect()
}

fn pinyin_choices(seed: &str) -> Vec<(String, CandidateKind)> {
    if seed.trim().is_empty() {
        return Vec::new();
    }
    if seed.chars().count() > 256 {
        return vec![(seed.into(), CandidateKind::Literal)];
    }
    let normalized = normalize_pinyin(seed);
    let mut paths = vec![(0usize, String::new(), 0usize)];
    // Bounded beam search: never exponential in the number of ambiguous syllables.
    let mut finished = Vec::new();
    let mut completions = Vec::new();
    for _ in 0..128 {
        let mut next = Vec::new();
        for (offset, text, segments) in paths {
            let rest = &normalized[offset..];
            if rest.is_empty() {
                finished.push((text, segments));
                continue;
            }
            if rest.starts_with('\'') {
                next.push((offset + 1, text, segments));
                continue;
            }
            if let Some(ch) = rest.chars().next().filter(|ch| !ch.is_ascii_alphabetic()) {
                next.push((offset + ch.len_utf8(), format!("{text}{ch}"), segments));
                continue;
            }
            // Finish only the final unfinished spelling, keeping the already converted prefix.
            // Unknown Latin prefixes never reach this state through a dictionary path.
            if rest.len() >= 2 && rest.bytes().all(|ch| ch.is_ascii_lowercase()) {
                for (_, word) in PINYIN
                    .iter()
                    .filter(|(key, _)| key.starts_with(rest) && *key != rest)
                    .take(4)
                {
                    completions.push((
                        format!("{text}{word}"),
                        segments + 1,
                        if is_dictionary_word(word) {
                            CandidateKind::Word
                        } else {
                            CandidateKind::Sentence
                        },
                    ));
                }
            }
            for (pinyin, hanzi) in PINYIN {
                if rest.starts_with(pinyin) {
                    next.push((
                        offset + pinyin.len(),
                        format!("{text}{hanzi}"),
                        segments + 1,
                    ));
                }
            }
        }
        next.sort_by(|left, right| right.0.cmp(&left.0).then(left.2.cmp(&right.2)));
        next.truncate(24);
        completions.sort_by_key(|(_, segments, _)| *segments);
        completions.truncate(64);
        if next.is_empty() {
            break;
        }
        paths = next;
    }
    finished.sort_by_key(|(_, segments)| *segments);
    let mut seen = std::collections::HashSet::new();
    let mut output: Vec<_> = finished
        .into_iter()
        .filter(|(text, _)| seen.insert(text.clone()))
        .take(5)
        .map(|(text, _)| {
            let kind = if text == seed {
                CandidateKind::Literal
            } else if is_dictionary_word(&text) {
                CandidateKind::Word
            } else {
                CandidateKind::Sentence
            };
            (text, kind)
        })
        .collect();
    output.extend(
        completions
            .into_iter()
            .filter(|(text, _, _)| seen.insert(text.clone()))
            .take(4)
            .map(|(text, _, kind)| (text, kind)),
    );
    if seen.insert(seed.to_owned()) {
        output.push((seed.into(), CandidateKind::Literal));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn converts_common_pinyin_and_tone_numbers() {
        for seed in ["nihao", "ni hao", "ni3 hao3", "NIHAO"] {
            assert_eq!(pinyin_candidates(seed)[0], "你好");
        }
        assert_eq!(pinyin_candidates("woaizhongguo")[0], "我爱中国");
    }
    #[test]
    fn apostrophe_keeps_syllable_boundaries_and_unknown_input_is_lossless() {
        assert_eq!(pinyin_candidates("xi'an")[0], "西安");
        assert_eq!(pinyin_candidates("xian")[0], "先");
        assert_eq!(pinyin_candidates("Rust2026"), ["Rust2026"]);
        assert!(pinyin_candidates("nihao").contains(&"nihao".into()));
        assert_eq!(pinyin_candidates("nü")[0], "女");
    }

    #[test]
    fn spaces_and_apostrophes_keep_pinyin_syllable_boundaries() {
        for seed in ["xi an", "xi  an", "XI AN", "xi1 an1", "xi'an"] {
            let candidates = pinyin_candidates(seed);
            assert_eq!(candidates[0], "西安", "{seed}: {candidates:?}");
            assert!(candidates.iter().any(|text| text == seed));
        }
        assert_eq!(pinyin_candidates("xian")[0], "先");
        assert_eq!(pinyin_candidates("xi'an")[0], "西安");
    }

    #[test]
    fn partial_last_syllables_and_typed_han_prefixes_have_bounded_lossless_completions() {
        for (seed, expected) in [
            ("woxihuanbeij", "我喜欢北京"),
            ("我喜欢bei", "我喜欢北京"),
            ("wo xihuan bei", "我喜欢北京"),
            ("我想学习zhongw", "我想学习中文"),
        ] {
            let candidates = pinyin_choices(seed);
            assert!(
                candidates
                    .iter()
                    .any(|(text, kind)| text == expected && *kind == CandidateKind::Word),
                "{seed}: {candidates:?}"
            );
            assert!(candidates.iter().any(|(text, _)| text == seed));
            assert!(candidates.len() <= 10);
        }
        for seed in [
            "unknownbei",
            "https://bei",
            "Rust2026",
            "字".repeat(300).as_str(),
        ] {
            assert_eq!(pinyin_candidates(seed), [seed]);
        }
    }
}
