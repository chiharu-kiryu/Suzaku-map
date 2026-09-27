//! Curated offline vocabulary, not a replacement for a full Pinyin dictionary.
//! Unknown input is always preserved; an optional LLM can enrich these candidates.

use super::ranked_typed_candidates;
use crate::ime::candidate_mix::CandidateKind;
use crate::ime::{Candidate, LanguagePlugin};
use std::sync::OnceLock;

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
        let mut consumed = 0;
        for expected in self.reading.bytes() {
            if consumed == input.len() {
                return Some(ReadingMatch::Prefix);
            }
            if expected == b'\'' {
                let separators = separator_bytes(&input[consumed..]);
                if separators > 0 {
                    consumed += separators;
                } else if self.require_separators {
                    return None;
                }
            } else if input.as_bytes()[consumed] == expected {
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
    // Project-authored expansion. Keep existing entries above in their original
    // order; new homophones are alternatives, not automatic spelling changes.
    // Daily life, communication and time.
    PinyinEntry::new("zao'fan", "早饭"),
    PinyinEntry::new("wu'fan", "午饭"),
    PinyinEntry::new("wan'fan", "晚饭"),
    PinyinEntry::new("zao'can", "早餐"),
    PinyinEntry::new("wu'can", "午餐"),
    PinyinEntry::new("wan'can", "晚餐"),
    PinyinEntry::new("chi'fan", "吃饭"),
    PinyinEntry::new("he'shui", "喝水"),
    PinyinEntry::new("can'ting", "餐厅"),
    PinyinEntry::new("fan'dian", "饭店"),
    PinyinEntry::new("cai'dan", "菜单"),
    PinyinEntry::new("dian'can", "点餐"),
    PinyinEntry::new("wai'mai", "外卖"),
    PinyinEntry::new("mi'fan", "米饭"),
    PinyinEntry::new("mian'tiao", "面条"),
    PinyinEntry::new("mian'bao", "面包"),
    PinyinEntry::new("niu'nai", "牛奶"),
    PinyinEntry::new("ji'dan", "鸡蛋"),
    PinyinEntry::new("shui'guo", "水果"),
    PinyinEntry::new("shu'cai", "蔬菜"),
    PinyinEntry::new("ping'guo", "苹果"),
    PinyinEntry::new("xiang'jiao", "香蕉"),
    PinyinEntry::new("ka'fei", "咖啡"),
    PinyinEntry::new("nai'cha", "奶茶"),
    PinyinEntry::new("yin'liao", "饮料"),
    PinyinEntry::new("hao'chi", "好吃"),
    PinyinEntry::new("hao'he", "好喝"),
    PinyinEntry::new("kou'wei", "口味"),
    PinyinEntry::new("jian'kang", "健康"),
    PinyinEntry::new("xiu'xi", "休息"),
    PinyinEntry::new("shui'jiao", "睡觉"),
    PinyinEntry::new("qi'chuang", "起床"),
    PinyinEntry::new("duan'lian", "锻炼"),
    PinyinEntry::new("yun'dong", "运动"),
    PinyinEntry::new("san'bu", "散步"),
    PinyinEntry::new("pao'bu", "跑步"),
    PinyinEntry::new("you'yong", "游泳"),
    PinyinEntry::new("yi'yuan", "医院"),
    PinyinEntry::new("yi'sheng", "医生"),
    PinyinEntry::new("hu'shi", "护士"),
    PinyinEntry::new("yao'dian", "药店"),
    PinyinEntry::new("yu'yue", "预约"),
    PinyinEntry::new("sheng'ri", "生日"),
    PinyinEntry::new("kuai'le", "快乐"),
    PinyinEntry::new("gao'xing", "高兴"),
    PinyinEntry::new("kai'xin", "开心"),
    PinyinEntry::new("gan'xie", "感谢"),
    PinyinEntry::new("gan'jue", "感觉"),
    PinyinEntry::new("dan'xin", "担心"),
    PinyinEntry::new("fang'xin", "放心"),
    PinyinEntry::new("bao'qian", "抱歉"),
    PinyinEntry::new("xin'ku", "辛苦"),
    PinyinEntry::new("ma'fan", "麻烦"),
    PinyinEntry::new("fang'bian", "方便"),
    PinyinEntry::new("bu'yong", "不用"),
    PinyinEntry::new("bu'ke'qi", "不客气"),
    PinyinEntry::new("mei'wen'ti", "没问题"),
    PinyinEntry::new("deng'yi'xia", "等一下"),
    PinyinEntry::new("shao'deng", "稍等"),
    PinyinEntry::new("shao'hou", "稍后"),
    PinyinEntry::new("ma'shang", "马上"),
    PinyinEntry::new("yi'hui", "一会"),
    PinyinEntry::new("xian'deng'yi'xia", "先等一下"),
    PinyinEntry::new("mei'shi", "没事"),
    PinyinEntry::new("hao'de", "好的"),
    PinyinEntry::new("xing'de", "行的"),
    PinyinEntry::new("que'shi", "确实"),
    PinyinEntry::new("dang'ran", "当然"),
    PinyinEntry::new("zhen'de", "真的"),
    PinyinEntry::new("yi'jing", "已经"),
    PinyinEntry::new("hai'mei", "还没"),
    PinyinEntry::new("yi'qi", "一起"),
    PinyinEntry::new("yi'xia", "一下"),
    PinyinEntry::new("yi'zhi", "一直"),
    PinyinEntry::new("yi'yang", "一样"),
    PinyinEntry::new("yi'dian", "一点"),
    PinyinEntry::new("yi'ban", "一般"),
    PinyinEntry::new("bi'jiao", "比较"),
    PinyinEntry::new("fei'chang", "非常"),
    PinyinEntry::new("te'bie", "特别"),
    PinyinEntry::new("zui'jin", "最近"),
    PinyinEntry::new("xian'qian", "先前"),
    PinyinEntry::new("zhi'qian", "之前"),
    PinyinEntry::new("zhi'hou", "之后"),
    PinyinEntry::new("yi'qian", "以前"),
    PinyinEntry::new("yi'hou", "以后"),
    PinyinEntry::new("gang'cai", "刚才"),
    PinyinEntry::new("ran'hou", "然后"),
    PinyinEntry::new("suo'yi", "所以"),
    PinyinEntry::new("yin'wei", "因为"),
    PinyinEntry::new("ru'guo", "如果"),
    PinyinEntry::new("dan'shi", "但是"),
    PinyinEntry::new("sui'ran", "虽然"),
    PinyinEntry::new("huo'zhe", "或者"),
    PinyinEntry::new("hai'shi", "还是"),
    PinyinEntry::new("er'qie", "而且"),
    PinyinEntry::new("bing'qie", "并且"),
    PinyinEntry::new("ling'wai", "另外"),
    PinyinEntry::new("qi'shi", "其实"),
    PinyinEntry::new("ying'gai", "应该"),
    PinyinEntry::new("ke'neng", "可能"),
    PinyinEntry::new("xu'yao", "需要"),
    PinyinEntry::new("bi'xu", "必须"),
    PinyinEntry::new("xi'wang", "希望"),
    PinyinEntry::new("jue'de", "觉得"),
    PinyinEntry::new("zhi'dao", "知道"),
    PinyinEntry::new("ming'bai", "明白"),
    PinyinEntry::new("liao'jie", "了解"),
    PinyinEntry::new("ren'wei", "认为"),
    PinyinEntry::new("ren'shi", "认识"),
    PinyinEntry::new("jue'ding", "决定"),
    PinyinEntry::new("tong'yi", "同意"),
    PinyinEntry::new("yi'jian", "意见"),
    PinyinEntry::new("jian'yi", "建议"),
    PinyinEntry::new("xiang'fa", "想法"),
    PinyinEntry::new("jie'shao", "介绍"),
    PinyinEntry::new("shuo'ming", "说明"),
    PinyinEntry::new("jie'shi", "解释"),
    PinyinEntry::new("lian'xi", "联系"),
    PinyinEntry::new("lian'xi", "练习"),
    PinyinEntry::new("xiao'xi", "消息"),
    PinyinEntry::new("you'jian", "邮件"),
    PinyinEntry::new("duan'xin", "短信"),
    PinyinEntry::new("dian'hua", "电话"),
    PinyinEntry::new("hui'fu", "回复"),
    PinyinEntry::new("hui'fu", "恢复"),
    PinyinEntry::new("shou'dao", "收到"),
    PinyinEntry::new("fa'song", "发送"),
    PinyinEntry::new("tong'zhi", "通知"),
    PinyinEntry::new("ti'xing", "提醒"),
    PinyinEntry::new("yao'qing", "邀请"),
    PinyinEntry::new("fen'xiang", "分享"),
    PinyinEntry::new("jiao'liu", "交流"),
    PinyinEntry::new("gou'tong", "沟通"),
    PinyinEntry::new("tao'lun", "讨论"),
    PinyinEntry::new("hui'yi", "会议"),
    PinyinEntry::new("kai'hui", "开会"),
    PinyinEntry::new("tong'shi", "同事"),
    PinyinEntry::new("tong'shi", "同时"),
    PinyinEntry::new("ke'hu", "客户"),
    PinyinEntry::new("gong'si", "公司"),
    PinyinEntry::new("tuan'dui", "团队"),
    PinyinEntry::new("bu'men", "部门"),
    PinyinEntry::new("ling'dao", "领导"),
    PinyinEntry::new("jing'li", "经理"),
    PinyinEntry::new("jing'li", "经历"),
    PinyinEntry::new("xiang'mu", "项目"),
    PinyinEntry::new("ren'wu", "任务"),
    PinyinEntry::new("ji'hua", "计划"),
    PinyinEntry::new("an'pai", "安排"),
    PinyinEntry::new("jin'du", "进度"),
    PinyinEntry::new("wan'cheng", "完成"),
    PinyinEntry::new("kai'shi", "开始"),
    PinyinEntry::new("jie'shu", "结束"),
    PinyinEntry::new("qu'xiao", "取消"),
    PinyinEntry::new("que'ren", "确认"),
    PinyinEntry::new("jian'cha", "检查"),
    PinyinEntry::new("chu'li", "处理"),
    PinyinEntry::new("jie'jue", "解决"),
    PinyinEntry::new("fu'ze", "负责"),
    PinyinEntry::new("can'jia", "参加"),
    PinyinEntry::new("zhun'bei", "准备"),
    PinyinEntry::new("zi'liao", "资料"),
    PinyinEntry::new("nei'rong", "内容"),
    PinyinEntry::new("yao'qiu", "要求"),
    PinyinEntry::new("xu'qiu", "需求"),
    PinyinEntry::new("biao'zhun", "标准"),
    PinyinEntry::new("jie'guo", "结果"),
    PinyinEntry::new("yuan'yin", "原因"),
    PinyinEntry::new("fang'fa", "方法"),
    PinyinEntry::new("fang'an", "方案"),
    PinyinEntry::new("mu'biao", "目标"),
    PinyinEntry::new("zhong'dian", "重点"),
    PinyinEntry::new("you'xian", "优先"),
    PinyinEntry::new("you'xian'ji", "优先级"),
    PinyinEntry::new("bao'gao", "报告"),
    PinyinEntry::new("fan'kui", "反馈"),
    PinyinEntry::new("zi'yuan", "资源"),
    PinyinEntry::new("yi'ju", "依据"),
    PinyinEntry::new("jing'yan", "经验"),
    PinyinEntry::new("xiao'lv", "效率"),
    PinyinEntry::new("zhi'liang", "质量"),
    PinyinEntry::new("fu'jian", "附件"),
    PinyinEntry::new("ri'cheng", "日程"),
    PinyinEntry::new("ri'li", "日历"),
    PinyinEntry::new("shang'wu", "上午"),
    PinyinEntry::new("zhong'wu", "中午"),
    PinyinEntry::new("xia'wu", "下午"),
    PinyinEntry::new("zao'shang", "早上"),
    PinyinEntry::new("wan'shang", "晚上"),
    PinyinEntry::new("jin'wan", "今晚"),
    PinyinEntry::new("hou'tian", "后天"),
    PinyinEntry::new("zhou'mo", "周末"),
    PinyinEntry::new("xing'qi", "星期"),
    PinyinEntry::new("xing'qi'yi", "星期一"),
    PinyinEntry::new("xing'qi'er", "星期二"),
    PinyinEntry::new("xing'qi'san", "星期三"),
    PinyinEntry::new("xing'qi'si", "星期四"),
    PinyinEntry::new("xing'qi'wu", "星期五"),
    PinyinEntry::new("xing'qi'liu", "星期六"),
    PinyinEntry::new("xing'qi'tian", "星期天"),
    PinyinEntry::new("xia'zhou", "下周"),
    PinyinEntry::new("shang'zhou", "上周"),
    PinyinEntry::new("ben'zhou", "本周"),
    PinyinEntry::new("yue'fen", "月份"),
    PinyinEntry::new("jin'nian", "今年"),
    PinyinEntry::new("ming'nian", "明年"),
    PinyinEntry::new("qu'nian", "去年"),
    PinyinEntry::new("jia'qi", "假期"),
    PinyinEntry::new("jie'ri", "节日"),
    PinyinEntry::new("chu'fa", "出发"),
    PinyinEntry::new("dao'da", "到达"),
    PinyinEntry::new("chu'men", "出门"),
    PinyinEntry::new("hui'jia", "回家"),
    PinyinEntry::new("shang'ban", "上班"),
    PinyinEntry::new("xia'ban", "下班"),
    PinyinEntry::new("lu'shang", "路上"),
    PinyinEntry::new("di'tie", "地铁"),
    PinyinEntry::new("gong'jiao", "公交"),
    PinyinEntry::new("chu'zu'che", "出租车"),
    PinyinEntry::new("huo'che", "火车"),
    PinyinEntry::new("gao'tie", "高铁"),
    PinyinEntry::new("fei'ji", "飞机"),
    PinyinEntry::new("ji'chang", "机场"),
    PinyinEntry::new("che'zhan", "车站"),
    PinyinEntry::new("che'piao", "车票"),
    PinyinEntry::new("hang'ban", "航班"),
    PinyinEntry::new("jiu'dian", "酒店"),
    PinyinEntry::new("fang'jian", "房间"),
    PinyinEntry::new("di'zhi", "地址"),
    PinyinEntry::new("di'zhi", "地质"),
    PinyinEntry::new("wei'zhi", "位置"),
    PinyinEntry::new("fu'jin", "附近"),
    PinyinEntry::new("di'tu", "地图"),
    PinyinEntry::new("dao'hang", "导航"),
    PinyinEntry::new("lv'xing", "旅行"),
    PinyinEntry::new("lv'you", "旅游"),
    PinyinEntry::new("jing'dian", "景点"),
    PinyinEntry::new("feng'jing", "风景"),
    PinyinEntry::new("zhao'pian", "照片"),
    PinyinEntry::new("shi'pin", "视频"),
    PinyinEntry::new("yin'yue", "音乐"),
    PinyinEntry::new("dian'ying", "电影"),
    PinyinEntry::new("gou'wu", "购物"),
    PinyinEntry::new("chao'shi", "超市"),
    PinyinEntry::new("shang'dian", "商店"),
    PinyinEntry::new("shang'pin", "商品"),
    PinyinEntry::new("jia'ge", "价格"),
    PinyinEntry::new("you'hui", "优惠"),
    PinyinEntry::new("ding'dan", "订单"),
    PinyinEntry::new("ding'yue", "订阅"),
    PinyinEntry::new("kuai'di", "快递"),
    PinyinEntry::new("wu'liu", "物流"),
    PinyinEntry::new("pei'song", "配送"),
    PinyinEntry::new("shou'huo", "收货"),
    PinyinEntry::new("fu'kuan", "付款"),
    PinyinEntry::new("zhi'fu", "支付"),
    PinyinEntry::new("tui'kuan", "退款"),
    PinyinEntry::new("fa'piao", "发票"),
    PinyinEntry::new("bao'xiu", "保修"),
    PinyinEntry::new("shou'hou", "售后"),
    PinyinEntry::new("yin'hang", "银行"),
    PinyinEntry::new("xue'xiao", "学校"),
    PinyinEntry::new("lao'shi", "老师"),
    PinyinEntry::new("xue'sheng", "学生"),
    PinyinEntry::new("ke'cheng", "课程"),
    PinyinEntry::new("kao'shi", "考试"),
    PinyinEntry::new("zuo'ye", "作业"),
    PinyinEntry::new("zhi'shi", "知识"),
    PinyinEntry::new("shu'ji", "书籍"),
    PinyinEntry::new("yue'du", "阅读"),
    PinyinEntry::new("bi'ji", "笔记"),
    // Writing, interface and software development.
    PinyinEntry::new("ci'ku", "词库"),
    PinyinEntry::new("ci'yu", "词语"),
    PinyinEntry::new("ci'zu", "词组"),
    PinyinEntry::new("ci'dian", "词典"),
    PinyinEntry::new("dan'ci", "单词"),
    PinyinEntry::new("ju'zi", "句子"),
    PinyinEntry::new("chang'ju", "长句"),
    PinyinEntry::new("duan'luo", "段落"),
    PinyinEntry::new("wen'ben", "文本"),
    PinyinEntry::new("wen'zi", "文字"),
    PinyinEntry::new("zi'ti", "字体"),
    PinyinEntry::new("zi'fu", "字符"),
    PinyinEntry::new("zi'hao", "字号"),
    PinyinEntry::new("pin'yin", "拼音"),
    PinyinEntry::new("luo'ma'yin", "罗马音"),
    PinyinEntry::new("ying'yu", "英语"),
    PinyinEntry::new("han'yu", "汉语"),
    PinyinEntry::new("ri'yu", "日语"),
    PinyinEntry::new("fan'yi", "翻译"),
    PinyinEntry::new("hou'xuan'ci", "候选词"),
    PinyinEntry::new("bu'quan", "补全"),
    PinyinEntry::new("zi'dong'bu'quan", "自动补全"),
    PinyinEntry::new("ci'pin", "词频"),
    PinyinEntry::new("quan'zhong", "权重"),
    PinyinEntry::new("pai'xu", "排序"),
    PinyinEntry::new("tong'yin'ci", "同音词"),
    PinyinEntry::new("jian'pan", "键盘"),
    PinyinEntry::new("shu'biao", "鼠标"),
    PinyinEntry::new("ping'mu", "屏幕"),
    PinyinEntry::new("chuang'kou", "窗口"),
    PinyinEntry::new("an'niu", "按钮"),
    PinyinEntry::new("tu'biao", "图标"),
    PinyinEntry::new("jie'mian", "界面"),
    PinyinEntry::new("mian'ban", "面板"),
    PinyinEntry::new("tuo'pan", "托盘"),
    PinyinEntry::new("xuan'xiang", "选项"),
    PinyinEntry::new("xuan'xiang'ka", "选项卡"),
    PinyinEntry::new("kuai'jie'jian", "快捷键"),
    PinyinEntry::new("shu'ru'kuang", "输入框"),
    PinyinEntry::new("guang'biao", "光标"),
    PinyinEntry::new("tuo'dong", "拖动"),
    PinyinEntry::new("suo'fang", "缩放"),
    PinyinEntry::new("gun'dong", "滚动"),
    PinyinEntry::new("dian'ji", "点击"),
    PinyinEntry::new("shuang'ji", "双击"),
    PinyinEntry::new("xuan'ze", "选择"),
    PinyinEntry::new("xuan'zhong", "选中"),
    PinyinEntry::new("fu'zhi", "复制"),
    PinyinEntry::new("zhan'tie", "粘贴"),
    PinyinEntry::new("jian'qie", "剪切"),
    PinyinEntry::new("che'xiao", "撤销"),
    PinyinEntry::new("hui'tui", "回退"),
    PinyinEntry::new("bao'cun", "保存"),
    PinyinEntry::new("shan'chu", "删除"),
    PinyinEntry::new("tian'jia", "添加"),
    PinyinEntry::new("xiu'gai", "修改"),
    PinyinEntry::new("ti'huan", "替换"),
    PinyinEntry::new("cha'ru", "插入"),
    PinyinEntry::new("da'kai", "打开"),
    PinyinEntry::new("guan'bi", "关闭"),
    PinyinEntry::new("tui'chu", "退出"),
    PinyinEntry::new("qi'dong", "启动"),
    PinyinEntry::new("chong'qi", "重启"),
    PinyinEntry::new("chong'xin'qi'dong", "重新启动"),
    PinyinEntry::new("ji'huo", "激活"),
    PinyinEntry::new("shi'fang", "释放"),
    PinyinEntry::new("qi'yong", "启用"),
    PinyinEntry::new("jin'yong", "禁用"),
    PinyinEntry::new("chong'zhi", "重置"),
    PinyinEntry::new("mo'ren", "默认"),
    PinyinEntry::new("zi'ding'yi", "自定义"),
    PinyinEntry::new("zi'dong", "自动"),
    PinyinEntry::new("shou'dong", "手动"),
    PinyinEntry::new("zhu'ti", "主题"),
    PinyinEntry::new("bu'ju", "布局"),
    PinyinEntry::new("yang'shi", "样式"),
    PinyinEntry::new("yan'se", "颜色"),
    PinyinEntry::new("chi'cun", "尺寸"),
    PinyinEntry::new("xian'shi", "显示"),
    PinyinEntry::new("yin'cang", "隐藏"),
    PinyinEntry::new("zhan'kai", "展开"),
    PinyinEntry::new("shou'qi", "收起"),
    PinyinEntry::new("jiao'dian", "焦点"),
    PinyinEntry::new("shu'ju", "数据"),
    PinyinEntry::new("shu'ju'ku", "数据库"),
    PinyinEntry::new("wen'jian", "文件"),
    PinyinEntry::new("wen'jian'jia", "文件夹"),
    PinyinEntry::new("wen'dang", "文档"),
    PinyinEntry::new("mu'lu", "目录"),
    PinyinEntry::new("lu'jing", "路径"),
    PinyinEntry::new("wang'luo", "网络"),
    PinyinEntry::new("lian'jie", "连接"),
    PinyinEntry::new("lian'jie", "链接"),
    PinyinEntry::new("wang'zhan", "网站"),
    PinyinEntry::new("wang'ye", "网页"),
    PinyinEntry::new("liu'lan'qi", "浏览器"),
    PinyinEntry::new("liu'lan", "浏览"),
    PinyinEntry::new("sou'suo", "搜索"),
    PinyinEntry::new("cha'xun", "查询"),
    PinyinEntry::new("guo'lv", "过滤"),
    PinyinEntry::new("zhang'hao", "账号"),
    PinyinEntry::new("zhang'hao", "帐号"),
    PinyinEntry::new("mi'ma", "密码"),
    PinyinEntry::new("deng'lu", "登录"),
    PinyinEntry::new("zhu'ce", "注册"),
    PinyinEntry::new("quan'xian", "权限"),
    PinyinEntry::new("shou'quan", "授权"),
    PinyinEntry::new("an'quan", "安全"),
    PinyinEntry::new("yin'si", "隐私"),
    PinyinEntry::new("yan'zheng", "验证"),
    PinyinEntry::new("fu'wu", "服务"),
    PinyinEntry::new("fu'wu'qi", "服务器"),
    PinyinEntry::new("ke'hu'duan", "客户端"),
    PinyinEntry::new("jie'kou", "接口"),
    PinyinEntry::new("xie'yi", "协议"),
    PinyinEntry::new("qing'qiu", "请求"),
    PinyinEntry::new("xiang'ying", "响应"),
    PinyinEntry::new("chao'shi", "超时"),
    PinyinEntry::new("yan'chi", "延迟"),
    PinyinEntry::new("yi'bu", "异步"),
    PinyinEntry::new("tong'bu", "同步"),
    PinyinEntry::new("xian'cheng", "线程"),
    PinyinEntry::new("jin'cheng", "进程"),
    PinyinEntry::new("bing'fa", "并发"),
    PinyinEntry::new("huan'cun", "缓存"),
    PinyinEntry::new("nei'cun", "内存"),
    PinyinEntry::new("ci'pan", "磁盘"),
    PinyinEntry::new("kong'jian", "空间"),
    PinyinEntry::new("xing'neng", "性能"),
    PinyinEntry::new("you'hua", "优化"),
    PinyinEntry::new("wen'ding", "稳定"),
    PinyinEntry::new("liu'chang", "流畅"),
    PinyinEntry::new("jian'rong", "兼容"),
    PinyinEntry::new("cao'zuo", "操作"),
    PinyinEntry::new("xi'tong", "系统"),
    PinyinEntry::new("ping'tai", "平台"),
    PinyinEntry::new("kua'ping'tai", "跨平台"),
    PinyinEntry::new("ruan'jian", "软件"),
    PinyinEntry::new("ying'jian", "硬件"),
    PinyinEntry::new("ying'yong", "应用"),
    PinyinEntry::new("cheng'xu", "程序"),
    PinyinEntry::new("dai'ma", "代码"),
    PinyinEntry::new("yuan'ma", "源码"),
    PinyinEntry::new("kai'yuan", "开源"),
    PinyinEntry::new("bian'cheng", "编程"),
    PinyinEntry::new("bian'yi", "编译"),
    PinyinEntry::new("gou'jian", "构建"),
    PinyinEntry::new("yun'xing", "运行"),
    PinyinEntry::new("diao'shi", "调试"),
    PinyinEntry::new("ri'zhi", "日志"),
    PinyinEntry::new("cuo'wu", "错误"),
    PinyinEntry::new("yi'chang", "异常"),
    PinyinEntry::new("beng'kui", "崩溃"),
    PinyinEntry::new("xiu'fu", "修复"),
    PinyinEntry::new("bu'ding", "补丁"),
    PinyinEntry::new("ban'ben", "版本"),
    PinyinEntry::new("geng'xin", "更新"),
    PinyinEntry::new("sheng'ji", "升级"),
    PinyinEntry::new("fa'bu", "发布"),
    PinyinEntry::new("an'zhuang", "安装"),
    PinyinEntry::new("xie'zai", "卸载"),
    PinyinEntry::new("da'bao", "打包"),
    PinyinEntry::new("bu'shu", "部署"),
    PinyinEntry::new("pei'zhi", "配置"),
    PinyinEntry::new("yi'lai", "依赖"),
    PinyinEntry::new("cang'ku", "仓库"),
    PinyinEntry::new("fen'zhi", "分支"),
    PinyinEntry::new("he'bing", "合并"),
    PinyinEntry::new("chong'tu", "冲突"),
    PinyinEntry::new("ti'jiao", "提交"),
    PinyinEntry::new("tui'song", "推送"),
    PinyinEntry::new("la'qu", "拉取"),
    PinyinEntry::new("bei'fen", "备份"),
    PinyinEntry::new("dao'ru", "导入"),
    PinyinEntry::new("dao'chu", "导出"),
    PinyinEntry::new("shang'chuan", "上传"),
    PinyinEntry::new("xia'zai", "下载"),
    PinyinEntry::new("ben'di", "本地"),
    PinyinEntry::new("yuan'cheng", "远程"),
    PinyinEntry::new("yun'duan", "云端"),
    PinyinEntry::new("li'xian", "离线"),
    PinyinEntry::new("zai'xian", "在线"),
    PinyinEntry::new("ben'di'mo'xing", "本地模型"),
    PinyinEntry::new("ren'gong'zhi'neng", "人工智能"),
    PinyinEntry::new("ji'qi'xue'xi", "机器学习"),
    PinyinEntry::new("sheng'cheng", "生成"),
    PinyinEntry::new("yu'ce", "预测"),
    PinyinEntry::new("tui'jian", "推荐"),
    PinyinEntry::new("shang'xia'wen", "上下文"),
    PinyinEntry::new("yu'liao", "语料"),
    PinyinEntry::new("yu'liao'ku", "语料库"),
    PinyinEntry::new("can'shu", "参数"),
    PinyinEntry::new("xun'lian", "训练"),
    PinyinEntry::new("jing'du", "精度"),
    PinyinEntry::new("zhun'que", "准确"),
    PinyinEntry::new("jie'ou", "解耦"),
    PinyinEntry::new("mo'kuai", "模块"),
    PinyinEntry::new("jia'gou", "架构"),
    PinyinEntry::new("kuang'jia", "框架"),
    PinyinEntry::new("cha'jian", "插件"),
    PinyinEntry::new("zu'jian", "组件"),
    PinyinEntry::new("lei'xing", "类型"),
    PinyinEntry::new("jie'gou", "结构"),
    PinyinEntry::new("han'shu", "函数"),
    PinyinEntry::new("bian'liang", "变量"),
    PinyinEntry::new("chang'liang", "常量"),
    PinyinEntry::new("bian'ma", "编码"),
    PinyinEntry::new("jie'ma", "解码"),
    PinyinEntry::new("zi'jie", "字节"),
    PinyinEntry::new("zi'fu'chuan", "字符串"),
    PinyinEntry::new("shu'zu", "数组"),
    PinyinEntry::new("lie'biao", "列表"),
    PinyinEntry::new("dui'lie", "队列"),
    PinyinEntry::new("ji'he", "集合"),
    PinyinEntry::new("suan'fa", "算法"),
    PinyinEntry::new("suo'yin", "索引"),
    PinyinEntry::new("xun'huan", "循环"),
    PinyinEntry::new("di'gui", "递归"),
    PinyinEntry::new("tiao'jian", "条件"),
    PinyinEntry::new("luo'ji", "逻辑"),
    PinyinEntry::new("bian'jie", "边界"),
    PinyinEntry::new("yuan'wen", "原文"),
    PinyinEntry::new("cao'gao", "草稿"),
    PinyinEntry::new("hui'gui", "回归"),
    PinyinEntry::new("hui'gui'ce'shi", "回归测试"),
    PinyinEntry::new("dan'yuan'ce'shi", "单元测试"),
    PinyinEntry::new("ji'cheng'ce'shi", "集成测试"),
    PinyinEntry::new("shi'jian", "事件"),
    PinyinEntry::new("shi'shi", "事实"),
    PinyinEntry::new("shi'shi", "实施"),
    PinyinEntry::new("shi'shi", "实时"),
    PinyinEntry::new("shi'shi", "试试"),
];

// Bucket by the first ASCII letter, retaining the authored order within each
// bucket. Expanding the lexicon must not make every beam path scan all entries
// or change how exact readings and homophones are ranked.
fn pinyin_starting_with(input: &str) -> &'static [&'static PinyinEntry] {
    static INDEX: OnceLock<[Vec<&'static PinyinEntry>; 26]> = OnceLock::new();
    let Some(initial) = input.bytes().next().filter(u8::is_ascii_lowercase) else {
        return &[];
    };
    let index = INDEX.get_or_init(|| {
        let mut buckets: [Vec<&'static PinyinEntry>; 26] = std::array::from_fn(|_| Vec::new());
        for entry in PINYIN {
            buckets[usize::from(entry.reading.as_bytes()[0] - b'a')].push(entry);
        }
        buckets
    });
    &index[usize::from(initial - b'a')]
}

fn is_pinyin_spacing(ch: char) -> bool {
    // Horizontal separators can join syllables; line/paragraph boundaries are
    // literal text, even when both neighbouring runs happen to be Pinyin.
    ch == '\t' || (ch.is_whitespace() && !ch.is_control() && !matches!(ch, '\u{2028}' | '\u{2029}'))
}

fn is_pinyin_separator(ch: char) -> bool {
    ch == '\'' || is_pinyin_spacing(ch)
}

fn separator_bytes(text: &str) -> usize {
    text.len() - text.trim_start_matches(is_pinyin_separator).len()
}

fn is_tone_terminator(tail: &[char]) -> bool {
    let Some((&next, rest)) = tail.split_first() else {
        return true;
    };
    if next.is_ascii_alphabetic() || next.is_whitespace() || next == '\'' {
        return true;
    }
    match next {
        // A decimal, clock value, dotted identifier or path is not a sentence
        // boundary. Commas immediately before digits may be numeric grouping.
        '.' | ':' => rest
            .first()
            .is_none_or(|ch| !ch.is_alphanumeric() && !matches!(ch, '/' | '\\' | '_' | '-')),
        ',' => !rest.first().is_some_and(|ch| ch.is_numeric()),
        '!' | '?' | ';' | ')' | ']' | '}' | '"' | '”' | '’' | '，' | '。' | '！' | '？' | '；'
        | '：' | '、' | '…' | '）' | '】' | '》' | '」' | '』' => true,
        _ => false,
    }
}

fn normalize_pinyin(seed: &str) -> String {
    // Normalize only letters used by this reading system. Unicode lowercasing
    // the entire draft corrupts literal Greek/Cyrillic/accented text. Preserve
    // separators here so the decoder can distinguish readings from literal spans.
    let lowered = seed
        .to_ascii_lowercase()
        .replace("u:", "v")
        .replace(['ü', 'Ü'], "v");
    let chars: Vec<_> = lowered.chars().collect();
    chars
        .iter()
        .enumerate()
        .filter_map(|(index, &ch)| {
            let tone = matches!(ch, '1'..='5')
                && index > 0
                && chars[index - 1].is_ascii_alphabetic()
                && is_tone_terminator(&chars[index + 1..]);
            (!tone).then_some(ch)
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
        ("早饭", &["早饭吃什么？", "早饭已经准备好了。"]),
        ("午饭", &["午饭一起吃吧。", "午饭想吃什么？"]),
        ("晚饭", &["晚饭一起吃吧。", "晚饭后出去散步吧。"]),
        ("周末", &["周末有什么安排？", "周末一起出去走走吧。"]),
        ("预约", &["预约已经确认了。", "预约时间可以修改吗？"]),
        ("订单", &["订单已经确认了。", "订单什么时候发货？"]),
        ("快递", &["快递已经到了。", "快递放在门口就可以。"]),
        ("地址", &["地址已经发给你了。", "地址需要修改一下。"]),
        ("会议", &["会议什么时候开始？", "会议时间已经确认了。"]),
        ("文件", &["文件已经保存了。", "文件已经发给你了。"]),
        ("保存", &["保存一下当前的修改。", "保存后再关闭窗口。"]),
        ("确认", &["确认一下时间和地点。", "确认后我再回复你。"]),
        ("安排", &["安排一个合适的时间吧。", "安排已经确认了。"]),
        ("收到", &["收到，谢谢。", "收到，我稍后处理。"]),
        ("稍等", &["稍等，我确认一下。", "稍等，我马上就来。"]),
        ("没问题", &["没问题，我来处理。", "没问题，稍后联系。"]),
        ("数据库", &["数据库连接正常。", "数据库需要先备份。"]),
        ("服务器", &["服务器已经启动了。", "服务器连接超时了。"]),
        ("部署", &["部署已经完成了。", "部署前先运行测试。"]),
        ("更新", &["更新已经完成了。", "更新后请重新启动。"]),
        ("修复", &["修复后再测试一下。", "修复已经完成了。"]),
        (
            "回归测试",
            &["回归测试已经通过了。", "回归测试还需要补充。"],
        ),
        ("词库", &["词库还需要继续扩充。", "词库已经更新了。"]),
        (
            "本地模型",
            &["本地模型已经加载了。", "本地模型还在加载中。"],
        ),
    ];
    let code = normalize_pinyin(seed);
    let mut output = Vec::new();
    if code.bytes().filter(u8::is_ascii_lowercase).count() >= 2 {
        for word in pinyin_starting_with(&code)
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
        && let phrase = primary.trim_matches(is_pinyin_spacing)
        && let Some((_, values)) = CONTINUATIONS.iter().find(|(word, _)| *word == phrase)
    {
        output.extend(values.iter().map(|text| {
            // Keep literal padding around adopted Han text, including a
            // newly typed Space. Only the authored continuation is appended.
            (
                format!("{primary}{}", &text[phrase.len()..]),
                CandidateKind::Sentence,
            )
        }));
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
    let input_chars = seed.chars().count();
    if input_chars > 256 {
        return vec![(seed.into(), CandidateKind::Literal)];
    }
    let normalized = normalize_pinyin(seed);
    let mut paths = vec![(0usize, String::new(), 0usize)];
    // Bounded beam search: never exponential in the number of ambiguous syllables.
    let mut finished = Vec::new();
    let mut completions = Vec::new();
    // Every live path consumes at least one Unicode scalar per round. Allow
    // the already bounded input to finish, plus one round to collect its end;
    // a fixed 128 rounds loses Pinyin after long adopted/literal prefixes.
    // Normalization never increases the input's scalar count, and the beam
    // width remains capped below, independently of the number of syllables.
    for _ in 0..=input_chars {
        let mut next = Vec::new();
        for (offset, text, segments) in paths {
            let rest = &normalized[offset..];
            if rest.is_empty() {
                finished.push((text, segments));
                continue;
            }
            let separators = separator_bytes(rest);
            if separators > 0 {
                // Only separators within a decoded phonetic span may disappear.
                // A trailing space can still end a reading; a closing quote or
                // spacing before literal Han text must not disappear with it.
                let after_reading = normalized[..offset]
                    .chars()
                    .next_back()
                    .is_some_and(|ch| ch.is_ascii_lowercase());
                let tail = &rest[separators..];
                let within_readings = tail.starts_with(|ch: char| ch.is_ascii_lowercase());
                let trailing_spacing = tail.is_empty() && rest.chars().all(is_pinyin_spacing);
                let text = if after_reading && (within_readings || trailing_spacing) {
                    text
                } else {
                    format!("{text}{}", &rest[..separators])
                };
                next.push((offset + separators, text, segments));
                continue;
            }
            if let Some(ch) = rest.chars().next().filter(|ch| !ch.is_ascii_alphabetic()) {
                next.push((offset + ch.len_utf8(), format!("{text}{ch}"), segments));
                continue;
            }
            let can_complete = rest.bytes().filter(u8::is_ascii_lowercase).count() >= 2;
            let mut completed = 0;
            for word in pinyin_starting_with(rest) {
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
            } else if is_dictionary_word(text.trim_matches(is_pinyin_spacing)) {
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
    fn reading_separators_use_utf8_boundaries_without_consuming_newlines() {
        let entry = PinyinEntry::new("shu'ru'fa", "输入法");
        for input in [
            "shu ru fa",
            "shu\tru\tfa",
            "shu\u{3000}ru\u{a0}fa",
            "shu''ru'fa",
        ] {
            assert!(entry.match_input(input) == Some(ReadingMatch::Exact(input.len())));
        }
        for input in [
            "shu\nru fa",
            "shu\r\nru fa",
            "shu\u{2028}ru fa",
            "sh u ru fa",
        ] {
            assert!(entry.match_input(input).is_none(), "{input:?}");
        }
        assert_eq!(pinyin_candidates("ni hao ")[0], "你好");
        assert_eq!(pinyin_candidates("nÜ")[0], "女");
        assert_eq!(pinyin_candidates("NU:")[0], "女");
    }

    #[test]
    fn authored_syllables_match_joined_and_separated_readings_without_inferred_splits() {
        let mut entries = std::collections::HashSet::new();
        for entry in PINYIN {
            assert!(entries.insert((entry.reading, entry.text)));
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
    fn prefix_index_retains_every_entry_and_its_authored_rank() {
        let mut count = 0;
        for initial in b'a'..=b'z' {
            let prefix = char::from(initial).to_string();
            let indexed = pinyin_starting_with(&prefix);
            let exhaustive: Vec<_> = PINYIN
                .iter()
                .filter(|entry| entry.reading.starts_with(char::from(initial)))
                .collect();
            assert_eq!(indexed.len(), exhaustive.len());
            for (actual, expected) in indexed.iter().zip(exhaustive) {
                // PINYIN is const: each use may promote a separate allocation.
                // Ordering concerns entry contents, not those storage addresses.
                assert_eq!(
                    (actual.reading, actual.text, actual.require_separators),
                    (expected.reading, expected.text, expected.require_separators)
                );
            }
            count += indexed.len();
        }
        assert_eq!(count, PINYIN.len());
        for input in ["", "A", "1", "'", " ", "中", "ü"] {
            assert!(pinyin_starting_with(input).is_empty(), "{input}");
        }
    }

    #[test]
    fn every_authored_entry_reaches_the_decoder_with_valid_syllable_boundaries() {
        for entry in PINYIN {
            let mut spellings = vec![entry.reading.to_owned(), entry.reading.replace('\'', " ")];
            if !entry.require_separators {
                spellings.push(entry.reading.replace('\'', ""));
            }
            for seed in spellings {
                let candidates = pinyin_candidates(&seed);
                assert!(
                    candidates.iter().any(|text| text == entry.text),
                    "{seed} -> {}: {candidates:?}",
                    entry.text
                );
                assert!(candidates.contains(&seed));
            }
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
    fn tone_terminators_preserve_punctuation_and_numeric_literal_boundaries() {
        for punctuation in [
            ".", ":", ",", "!", "?", ";", ")", "]", "}", "\"", "”", "’", "，", "。", "！", "？",
            "；", "：", "、", "…", "）", "】", "》", "」", "』", "?!", "...", ".)",
        ] {
            let seed = format!("ni3hao3{punctuation}");
            assert_eq!(normalize_pinyin(&seed), format!("nihao{punctuation}"));
            assert_eq!(pinyin_candidates(&seed)[0], format!("你好{punctuation}"));
            assert!(pinyin_candidates(&seed).contains(&seed));
        }
        for suffix in [
            "3.5",
            "3:30",
            "3,000",
            "3,４００",
            "30",
            "36",
            "0!",
            "6!",
            "3/4",
            "3-4",
            "3_4",
            "3.rs",
            "3://",
            "3@home",
            "3%",
            "3+4",
            "3=4",
        ] {
            let seed = format!("hao{suffix}");
            assert_eq!(normalize_pinyin(&seed), seed, "{seed:?}");
        }
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
