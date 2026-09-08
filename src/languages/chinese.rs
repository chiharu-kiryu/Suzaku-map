//! Small offline bootstrap lexicon, not a replacement for a full Pinyin dictionary.
//! Unknown input is always preserved; an optional LLM can enrich these candidates.

use super::ranked_candidates;
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
    fn expand_token(&self, token: &str, _degraded: bool) -> Vec<String> {
        vec![token.into()]
    }
    fn build_candidates(&self, _parts: &[String], seed: &str, confidence: f32) -> Vec<Candidate> {
        ranked_candidates(pinyin_candidates(seed), confidence)
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
            (!tone && !ch.is_whitespace()).then_some(ch)
        })
        .collect()
}

pub fn pinyin_candidates(seed: &str) -> Vec<String> {
    let seed = seed.trim();
    if seed.is_empty() {
        return Vec::new();
    }
    let normalized = normalize_pinyin(seed);
    let mut paths = vec![(0usize, String::new(), 0usize)];
    // Bounded beam search: never exponential in the number of ambiguous syllables.
    let mut finished = Vec::new();
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
        if next.is_empty() {
            break;
        }
        paths = next;
    }
    finished.sort_by_key(|(_, segments)| *segments);
    let mut output: Vec<String> = finished.into_iter().map(|(text, _)| text).take(5).collect();
    // Prefix completion only for a single unambiguous unfinished code, not arbitrary mixed text.
    if output.is_empty()
        && normalized.len() >= 2
        && normalized.bytes().all(|ch| ch.is_ascii_lowercase())
    {
        output.extend(
            PINYIN
                .iter()
                .filter(|(key, _)| key.starts_with(&normalized))
                .take(4)
                .map(|(_, text)| text.to_string()),
        );
    }
    output.push(seed.to_string());
    let mut seen = std::collections::HashSet::new();
    output.retain(|text| seen.insert(text.clone()));
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
}
