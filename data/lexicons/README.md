# 内置词库数据

这里存放项目编写、MIT 许可的中英日词库，不是下载的第三方语料，也不包含个人输入历史。
词库与模型提供器、IBus、面板以及语言转换算法分离：

- [英文](en.json)：有序词形层、下一词搭配、完整短句、短语补尾。
- [中文](zh-Hans.json)：显式拼音音节、文本、词/句类型、强制分隔标记、汉字语境续句。
- [日文](ja.json)：假名读音、文字、词/句类型、语境续句；不是完整形态分析词典。
- [数据接口](../../src/lexicon.rs)只加载和校验数据，不导入输入引擎、语言插件、平台或模型。
  拼音分段、罗马音转假名、大小写/空白保真、索引、候选权重及数量上限仍归语言实现负责。

## 格式版本 1

UTF-8 JSON。必填 `format_version: 1` 和 `language`；其余顶层数组默认空。
未知字段、未知版本、类型错误、空白/控制字符污染、重复层标识或重复语境会报错，
不静默忽略错误条目。单资源上限 2 MiB、最多 64 个词层、单文本最多 4,096 个字符。

```json
{
  "format_version": 1,
  "language": "en",
  "word_layers": [{
    "id": "example",
    "words": ["hello", "world"],
    "next_words": [["hello", ["world"]]],
    "sentences": ["hello world."]
  }],
  "phrase_endings": [["hello", ["world"]]],
  "readings": [],
  "continuations": []
}
```

- `word_layers`：按数组顺序排序；`id` 只是标识，不对应引擎里的分支。每层的 `words`、
  `next_words`、`sentences` 都可以省略。`next_words` 是 `[语境, [下一词…]]`；
  `sentences` 是完整句子。英文实现按“词形 → 下一词 → 短句中的词”建立各层索引，
  首次出现的位置决定现有基础优先级；加载词库不会触发模型请求。
- `phrase_endings`：`[语境, [补尾…]]`，不是完整句子。
- `readings`：例如
  `{"reading":"xi'an","text":"西安","kind":"word","require_separators":true}`。
  `kind` 必填，只能为 `word` 或 `sentence`，不以硬编码特定文本判断类型；
  `require_separators` 默认 `false`，由读音实现解释。中文用 ASCII 撇号标明音节，
  不能靠任意猜测切分替代；日文使用完整假名读音。
- `continuations`：`[语境, [完整句子…]]`；完整句子必须保留语境前缀。
  与英文 `phrase_endings` 不同，不要只填写句尾。

## 排序与维护约束

数组顺序有意义，不按字母重排数据，不对历史重复词形自动去重。新增英文扩充请追加
一个层，避免插入旧层改变后续词的基础权重；中文/日文在原读音表末尾追加，保留同音顺序。
同一读音允许不同文本，同一读音/文本组合不允许重复；同一语境不应跨层重复声明。
真实词形显式收录，不任意拼接后缀。数据修改仍需维护质量基准，不能只更新数量断言。

扩充已有语言的词库不需要改语言算法，也不需要指定 LLaMA 或其他模型。新增一种语言的
数据并不自动提供该语言的输入法：仍须实现和注册相应的转换规则。

当前通过 `include_str!` 编译进程序，按语言在首次使用时解析、校验并缓存，之后不重复解析，
不在每次输入时读取磁盘或网络。普通安装包无需寻找外部词库路径；修改 JSON 仍需重编译和
重新安装，不是热更新、词库导入界面或个人学习功能。`Lexicon::from_json` 可独立解析另一份
资源，但不会替换活动输入法、写用户配置或自动连接任何模型。

## 验证

```sh
cargo test --locked --all-features --test lexicon_resources -- --test-threads=1
cargo test --locked --all-features languages:: -- --test-threads=1
cargo test --locked --all-features --test english_completion_quality --test chinese_completion_quality --test offline_vocabulary_quality --test long_draft_completion -- --test-threads=1
```

[数据契约回归](../../tests/lexicon_resources.rs)覆盖格式、边界、同音和层顺序；
迁移指纹从原 Rust 表生成，锁定原 5,251 个英文词的索引权重、1,143 条拼音以及 26 条日文
读音的完整顺序和标记。后续扩充不得以盲目重录指纹来掩盖旧词重排。
配合[安装版编辑器检查](../../scripts/test-linux-vocabulary.py)和
[原生 IBus 回归](../../scripts/test-native-sync.py)校验词句采用、撤回、续写与提交。
