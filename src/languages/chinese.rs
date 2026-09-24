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

// Authored bootstrap vocabulary, with explicit syllables rather than inferred
// splits. Optional typed separators may join these syllables, never split one.
struct PinyinEntry {
    reading: &'static str,
    text: &'static str,
    require_separators: bool,
}

impl PinyinEntry {
    const fn new(reading: &'static str, text: &'static str) -> Self {
        Self {
            reading,
            text,
            require_separators: false,
        }
    }

    const fn separated(reading: &'static str, text: &'static str) -> Self {
        Self {
            reading,
            text,
            require_separators: true,
        }
    }

    fn match_input(&self, input: &str) -> Option<ReadingMatch> {
        let input = input.as_bytes();
        let mut consumed = 0;
        for expected in self.reading.bytes() {
            if consumed == input.len() {
                return Some(ReadingMatch::Prefix);
            }
            if expected == b'\'' {
                if input[consumed] == b'\'' {
                    while input.get(consumed) == Some(&b'\'') {
                        consumed += 1;
                    }
                } else if self.require_separators {
                    return None;
                }
            } else if input[consumed] == expected {
                consumed += 1;
            } else {
                return None;
            }
        }
        Some(ReadingMatch::Exact(consumed))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReadingMatch {
    Exact(usize),
    Prefix,
}

// Longer phrases rank before syllable paths. Xi'an retains a mandatory boundary,
// so the joined reading xian still prefers 先 rather than treating it as xi + an.
const PINYIN: &[PinyinEntry] = &[
    PinyinEntry::new("ni'hao", "你好"),
    PinyinEntry::new("ni'hao'ma", "你好吗"),
    PinyinEntry::new("xie'xie", "谢谢"),
    PinyinEntry::new("zai'jian", "再见"),
    PinyinEntry::new("zao'shang'hao", "早上好"),
    PinyinEntry::new("wan'shang'hao", "晚上好"),
    PinyinEntry::new("mei'guan'xi", "没关系"),
    PinyinEntry::new("dui'bu'qi", "对不起"),
    PinyinEntry::new("qing'wen", "请问"),
    PinyinEntry::new("ke'yi", "可以"),
    PinyinEntry::new("bu'ke'yi", "不可以"),
    PinyinEntry::new("wo'men", "我们"),
    PinyinEntry::new("ni'men", "你们"),
    PinyinEntry::new("ta'men", "他们"),
    PinyinEntry::new("zhong'wen", "中文"),
    PinyinEntry::new("ying'wen", "英文"),
    PinyinEntry::new("ri'wen", "日文"),
    PinyinEntry::new("zhong'guo", "中国"),
    PinyinEntry::new("shi'jie", "世界"),
    PinyinEntry::new("shu'ru'fa", "输入法"),
    PinyinEntry::new("shu'ru", "输入"),
    PinyinEntry::new("hou'xuan", "候选"),
    PinyinEntry::new("lian'xiang", "联想"),
    PinyinEntry::new("mo'xing", "模型"),
    PinyinEntry::new("yu'yan", "语言"),
    PinyinEntry::new("zhi'chi", "支持"),
    PinyinEntry::new("gong'neng", "功能"),
    PinyinEntry::new("ji'xu", "继续"),
    PinyinEntry::new("kai'fa", "开发"),
    PinyinEntry::new("ce'shi", "测试"),
    PinyinEntry::new("she'zhi", "设置"),
    PinyinEntry::new("jin'tian", "今天"),
    PinyinEntry::new("ming'tian", "明天"),
    PinyinEntry::new("zuo'tian", "昨天"),
    PinyinEntry::new("xian'zai", "现在"),
    PinyinEntry::new("tian'qi", "天气"),
    PinyinEntry::new("gong'zuo", "工作"),
    PinyinEntry::new("xue'xi", "学习"),
    PinyinEntry::new("xi'huan", "喜欢"),
    PinyinEntry::new("peng'you", "朋友"),
    PinyinEntry::new("shi'jian", "时间"),
    PinyinEntry::new("wen'ti", "问题"),
    PinyinEntry::new("bang'zhu", "帮助"),
    PinyinEntry::new("dian'nao", "电脑"),
    PinyinEntry::new("shou'ji", "手机"),
    PinyinEntry::new("bei'jing", "北京"),
    PinyinEntry::new("shang'hai", "上海"),
    PinyinEntry::new("shen'zhen", "深圳"),
    PinyinEntry::new("guang'zhou", "广州"),
    PinyinEntry::separated("xi'an", "西安"),
    PinyinEntry::new("wo", "我"),
    PinyinEntry::new("ni", "你"),
    PinyinEntry::new("ni", "呢"),
    PinyinEntry::new("ta", "他"),
    PinyinEntry::new("ta", "她"),
    PinyinEntry::new("men", "们"),
    PinyinEntry::new("hao", "好"),
    PinyinEntry::new("hao", "号"),
    PinyinEntry::new("shi", "是"),
    PinyinEntry::new("shi", "时"),
    PinyinEntry::new("de", "的"),
    PinyinEntry::new("le", "了"),
    PinyinEntry::new("ma", "吗"),
    PinyinEntry::new("ne", "呢"),
    PinyinEntry::new("bu", "不"),
    PinyinEntry::new("you", "有"),
    PinyinEntry::new("you", "又"),
    PinyinEntry::new("zai", "在"),
    PinyinEntry::new("ai", "爱"),
    PinyinEntry::new("hen", "很"),
    PinyinEntry::new("xiang", "想"),
    PinyinEntry::new("yao", "要"),
    PinyinEntry::new("qu", "去"),
    PinyinEntry::new("lai", "来"),
    PinyinEntry::new("kan", "看"),
    PinyinEntry::new("ting", "听"),
    PinyinEntry::new("shuo", "说"),
    PinyinEntry::new("qing", "请"),
    PinyinEntry::new("he", "和"),
    PinyinEntry::new("ye", "也"),
    PinyinEntry::new("dou", "都"),
    PinyinEntry::new("ke", "可"),
    PinyinEntry::new("yi", "以"),
    PinyinEntry::new("zhong", "中"),
    PinyinEntry::new("guo", "国"),
    PinyinEntry::new("ren", "人"),
    PinyinEntry::new("tian", "天"),
    PinyinEntry::new("qi", "气"),
    PinyinEntry::new("xin", "新"),
    PinyinEntry::new("da", "大"),
    PinyinEntry::new("xiao", "小"),
    PinyinEntry::new("duo", "多"),
    PinyinEntry::new("shao", "少"),
    PinyinEntry::new("lv", "绿"),
    PinyinEntry::new("nv", "女"),
    PinyinEntry::new("xian", "先"),
    PinyinEntry::new("xi", "西"),
    PinyinEntry::new("an", "安"),
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
    PINYIN.iter().any(|word| word.text == text) && text != "你好吗"
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
    if code.bytes().filter(u8::is_ascii_lowercase).count() >= 2 {
        for word in PINYIN
            .iter()
            .filter(|word| word.match_input(&code) == Some(ReadingMatch::Prefix))
            .take(5)
        {
            output.push((
                word.text.to_owned(),
                if is_dictionary_word(word.text) {
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
            let can_complete = rest.bytes().filter(u8::is_ascii_lowercase).count() >= 2;
            let mut completed = 0;
            for word in PINYIN {
                match word.match_input(rest) {
                    Some(ReadingMatch::Exact(bytes)) => {
                        next.push((offset + bytes, format!("{text}{}", word.text), segments + 1))
                    }
                    // Complete only the final reading, including explicitly
                    // separated syllables. A separator inside a syllable fails
                    // the match; it is never blindly stripped to make a word.
                    Some(ReadingMatch::Prefix) if can_complete && completed < 4 => {
                        completed += 1;
                        completions.push((
                            format!("{text}{}", word.text),
                            segments + 1,
                            if is_dictionary_word(word.text) {
                                CandidateKind::Word
                            } else {
                                CandidateKind::Sentence
                            },
                        ));
                    }
                    _ => {}
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
    fn authored_syllables_match_joined_and_separated_readings_without_inferred_splits() {
        for entry in PINYIN {
            assert_eq!(
                entry.reading.split('\'').count(),
                entry.text.chars().count()
            );
            assert!(
                entry
                    .reading
                    .split('\'')
                    .all(|part| !part.is_empty() && part.bytes().all(|ch| ch.is_ascii_lowercase()))
            );
            assert!(
                entry.match_input(entry.reading) == Some(ReadingMatch::Exact(entry.reading.len()))
            );
            if !entry.require_separators {
                let joined = entry.reading.replace('\'', "");
                assert!(entry.match_input(&joined) == Some(ReadingMatch::Exact(joined.len())));
            }
            let repeated = entry.reading.replace('\'', "''");
            assert!(entry.match_input(&repeated) == Some(ReadingMatch::Exact(repeated.len())));
        }
        for (seed, expected) in [
            ("shu1 ru4 fa3", "输入法"),
            ("shu1ru4fa3", "输入法"),
            ("WO3  XI3  HUAN1  BEI3  JING1", "我喜欢北京"),
            ("我想xue2 xi2 zhong1 wen2", "我想学习中文"),
            ("xi'an zai", "西安在"),
            ("xian zai", "现在"),
        ] {
            assert_eq!(pinyin_candidates(seed)[0], expected, "{seed}");
            assert!(pinyin_candidates(seed).iter().any(|value| value == seed));
        }
    }

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
