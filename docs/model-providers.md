# 模型服务：本地优先，与具体模型解耦

中、英、日文共用模型无关的 `prediction::PredictionProvider` 候选协议。
`HttpModelProvider` 根据服务协议封装请求，不根据模型名称判断行为。
旧 `LlmCompletionProvider`、`languages::llama` 与 `suzaku_tool llama` 保留兼容入口；
新增适配器实现统一预测协议，不必改动键盘事件、离线词库或语言转换。

## 输入预测协议 v1（开发版）

实际调用链为：引擎 → 有界异步 worker → `PredictionProvider` → 后端适配器。
Linux 宿主和独立面板都走此入口；本地发现、云端授权及传输仍由 HTTP 适配器负责。
这是应用内可序列化的任务协议，**不是新增网络监听服务，也不是要求模型直接输出协议包**。
它与配置中的 `llm_protocol` 不同：后者选择 Ollama 或 compatible 的 HTTP 外层格式。

公开类型位于 [`src/prediction/mod.rs`](../src/prediction/mod.rs)：

| 类型 | 契约 |
| --- | --- |
| `PredictionRequest` | `version`、`request_id`、`input`、`output`、`limits` |
| `PredictionInput` | 语言 ID、精确原始草稿、本地转换、会话提交上下文、置信度与降级状态 |
| `PredictionOutput` | v1 只接受 `complete_draft_replacement`：完整替换当前可编辑草稿，不是追加 token 或提交命令 |
| `PredictionResponse` | 对应版本和请求编号，以及候选列表；编号由适配器回传，不让语言模型生成 |
| `PredictionCandidate` | 纯正文、相对排序提示、可选 `word` / `sentence` 类型；没有 UI 标签或原样输入类型 |
| `PredictionCancellation` / `PredictionError` | 协作取消、结构错误和后端失败；失败保持离线输入可用 |

`PredictionRequest::new(id, input)` 使用 v1 默认预算：最多 6 条候选，原始草稿最多
256 个 Unicode 字符、会话提交上下文最多 160 字符；本地转换传入上限为 416 字符。
候选默认最多新增 160 字符，只有精确匹配且不超过 256 字符的本地转换前缀才不计入
新增预算；其他替换正文整体限 160 字符。调用方可缩小候选数和新增字符预算，不能扩张
全局上限。HTTP 适配器过滤超限项而不截断正文。权重不是概率，其影响仍由引擎限幅。

请求和响应支持 Serde JSON 往返，保留 Unicode、缩进及连续空格。worker 在调用前
验证请求、调用后验证版本/编号/数量/字符预算/控制字符/有限权重，同时保留自身的
revision 屏障，不信任响应编号来替代本地过期判断。结构校验不是语言质量认证；
语言前缀与补全规则、词句混排分别属于任务策略和引擎。

2026-10-06 补齐了所有后端共用的草稿保护：结构校验后，worker 通过
[`prediction/policy.rs`](../src/prediction/policy.rs) 逐项过滤丢失英文前文/缩进、
仅回显原稿或生成请求字段的候选，再交给面板/IBus 限额与混排。坏候选不会遮住同批
有效候选，全无效时保留完整本地候选并报告 `NoCandidates`。
英文以原始草稿为准，不强制采用本地首选词；有本地补全证据的半词如 `hel` 不能
变成 `hel there`，但仅仅未收录本地词库的词不会因此禁止独立后端继续预测。
拼音、罗马音与同音转换不受英文前缀规则约束。字段回显判断只检查生成段，保留用户
已输入的 JSON、Unicode 前缀以及合法的中日文转换。HTTP 原有的转换前缀和短句策略仍
独立保留；这些检查不代表对任意语言的正确性保证。

新增后端只需实现 `PredictionProvider::predict`，并通过引擎的
`configure_prediction_provider` 接入；既可以适配聊天模型，也可以直接返回预测结构。
适配器应先检查取消和请求有效性，限制自己的 I/O，返回 `PredictionResponse::new`
关联的完整结果。端点、模型标识、认证、采样参数和发现缓存均不属于输入协议。
旧 `configure_prediction` 通过 `LegacyPredictionAdapter` 桥接；旧实现不需新增方法，
旧请求的字段与候选 struct literal 保留。旧候选列表的逐项过滤行为也保留。

[`prediction/prompt.rs`](../src/prediction/prompt.rs) 集中管理聊天模型的输入字段、
语言提示、精简 schema 和候选内容解析；HTTP 模块只组合供应商 envelope 并处理
传输及外层完成状态。当前仍发送原来的双字符串候选提示，不因分层改变小模型负担。
v1 不增加桌面周边文本采集、不扩大上下文、不支持 token 流式展示、任意编辑范围或
工具调用；这些需要后续明确协议扩展与独立验收，不能用此重构宣称预测质量已提升。

显式[文本翻译](translation.md)使用独立的 `TranslationProvider` 接口和翻译提示，共用同一个
`HttpModelProvider` 配置与安全传输。翻译总超时为 30 秒，不使用候选联想的 20–5000 ms
低延迟预算；翻译本身不要求联想开关开启，但云端授权和凭据检查完全相同。

## 三个独立选项

| 选项 | 可选值 | 默认值 |
| --- | --- | --- |
| `llm_scope` | `local` / `cloud` | `local` |
| `llm_protocol` | `auto` / `ollama` / `openai-compatible` | `auto` |
| `llm_model` | 服务公布的模型 ID；本地可用 `auto` | `auto` |

`auto` 协议将 `/api/chat` 识别为 Ollama，其他地址使用 chat completions。
可以显式指定协议以支持自定义生成路径。compatible 适配器使用 `messages`、`model`、
`temperature`、`max_tokens` 和非流式 `choices[].message.content`；
**不是所有厂商的专有 API 都受支持**，不同协议需新增适配器或使用兼容网关。

## 词与句子的统一输出

0.6.6 把默认请求精简为 **2 条完整草稿**：一个词补全和一个短句续写，减少小型 CPU 模型
反复生成字段名与分类标签的开销。Ollama 使用简单的双字符串数组 schema；compatible
服务通过提示请求同样格式：

```json
{"candidates":["hello","hello world"]}
```

响应仍兼容最多 6 项的旧 `{text, kind}` 对象、最多 3 项的字符串数组，以及 compatible
逐行纯文本；不要求现有服务同步换格式。分类、提取词候选和词句混排仍由语言层负责，
不保证模型每次都给出两种类型。默认输出上限仍是 256 tokens；拒绝截断
JSON、未知类型和控制字符。开发版统一了长草稿的长度预算：完整候选可保留最多 256 字符的
精确输入/转换前缀，再新增最多 160 字符（总长最多 416）；没有匹配前缀的转换结果仍限 160 字符。
解析与引擎混排使用同一预算，不截断前文，不放宽响应体大小或控制字符校验；超过 256 字符的
输入仍走有界本地回退。模型的排序提示经本地限幅，不能覆盖输入首项或把展示注解写进正文。
具体混排与按键见 [IBus 候选体验](ibus-candidates.md)。
英文候选从响应解析、混排到提交都保留已输入的精确前缀，包括缩进、连续空格和列表标记；
仅清除前缀以外的响应留白，不补造模型漏掉的前缀。原样输入仍不会作为新联想接受。
生成部分若包含至少两个不同的、带引号与冒号的请求字段键（例如 `local_conversion`
与 `raw_composition`），也作为请求数据回显丢弃。即使外层 JSON 正常，它也不是输入联想。
此保护只检查新增文本，不删除用户已经输入的字段名，也不充当通用语言质量判定器。

开发版英文请求额外区分三种状态：未完成词 `complete_word`、已输入分隔空格等续写场景
`next_word`，以及 `a` / `can` 等既可能是完整词又可能是词头的 `complete_or_continue`。
请求保留 `raw_composition`、`local_conversion`、`committed_context` 和 `suggestion_mode`，
不再重复发送 `word_prefix` / `text_before_word` 或要求 3+3 条候选；默认置信度、空手写提示
也不再占用输入。非默认置信度、降级状态和显式手写提示仍保留。候选必须是完整草稿替换文本，
不是仅返回末词。这个规则同时用于 Ollama 和 compatible 协议，不依据具体模型名称分支，
也不覆盖调用方自定义系统提示、温度或生成预算。
IBus 混排还能从有效英文句子中提取独立词候选，细节与回归范围见
[英文补全质量回归](ibus-candidates.md)。

## 本地发现

```bash
suzaku_tool model configure
suzaku_tool model discover
suzaku_tool model status
```

以上 `configure` 恢复本地自动配置，保留输入语言、联想开关、超时和温度；不会开启联想。
启用联想后，后台第一次需要模型时也会自动发现。默认只检查三个回环地址：

- Ollama：`http://127.0.0.1:11434/api/chat`，读取 `/api/tags`。
- llama.cpp：`http://127.0.0.1:8080/v1/chat/completions`，读取 `/v1/models`。
- 其他兼容桌面服务：`http://127.0.0.1:1234/v1/chat/completions`，读取 `/v1/models`。

优先选择名称/元数据为 LLaMA 的已安装模型，其次选择其他本地模型。不会指定版本、
下载文件、启动服务、自动预热、遍历磁盘、扫描其他端口或访问云端。
本地请求只允许字面回环 IP 或 `localhost`（固定映射到回环），不走 DNS、代理或重定向。
发现最多 800 ms；生成受 `llm_timeout_ms` 总预算约束，发现也计入预算。
发现成功缓存 60 秒、失败缓存 5 秒，配置重载创建新缓存。慢模型不会阻塞离线候选。
本地生成遇到模型/接口不存在（404）或服务不可用时，会清除对应的成功缓存，让下一次
输入请求重新发现；不会自动重试刚才失败的生成。超时、限流和云端失败不触发重新发现。

指定其他模型或端口：

```bash
suzaku_tool model configure --model my-local-model --endpoint http://127.0.0.1:8080/v1/chat/completions --protocol openai-compatible
```

自定义端点不扫描其他服务。自动选择需要服务提供模型清单；显式指定 compatible 模型时，
可以只提供生成接口。Ollama 本地模型在生成前会检查清单，拒绝被标记为远程的模型，
包括自定义别名；清单结果会缓存，因此仍建议服务设置 `OLLAMA_NO_CLOUD=1`。
回环地址是本地服务边界，Suzaku 无法识别任意兼容网关内部是否继续代理到远端。
本地认证头暂不支持；不要把密钥写进 URL。

`model warmup` 仅用于本地 Ollama，必须显式执行；发送空消息，空闲 5 分钟后释放。
`model probe [all|en|zh-Hans|ja]` 使用固定合成示例，不读取真实输入；它会实际运行模型，
云端已授权时会发送这些示例，并可能产生服务费用。`status` 对云端只显示配置、不发请求。

## 云端配置与授权

以下域名和模型 ID 都是占位值，需要换成你选择的服务提供的 **完整 HTTPS 生成端点**。
`auto` 模型名不用于云端，不会自动枚举云端账号或选择服务。

```bash
suzaku_tool model configure \
  --scope cloud \
  --protocol openai-compatible \
  --endpoint https://models.example/v1/chat/completions \
  --model your-model-id \
  --api-key-env SUZAKU_MODEL_API_KEY
```

此时仍未授权发送输入。将密钥安全注入**运行宿主的进程环境**后，再明确开启：

```bash
suzaku_tool model configure --cloud-consent true
```

配置仅保存环境变量名 `SUZAKU_MODEL_API_KEY`，密钥值在请求时读取。没有 `--api-key`
参数，密钥不会进入配置、备份、托盘或错误消息。无需认证的服务可用 `--api-key-env none`。
单纯在终端 `export` 不会改变已运行宿主的环境；若由 systemd 用户服务启动，需通过自己的
安全环境配置方式注入，再重启宿主。不要把密钥贴进工单、命令行参数或提交到仓库。

配置后在托盘选 **重新加载模型配置**，再按需开启 **LLM 联想**。生成必须同时满足：
联想开启、云端明确授权、配置有效、所选密钥可用、非隐私输入字段。

0.5.7 面板的自动候选使用启动时/宿主确认的配置；温度或手写提示刷新不会偷偷采用磁盘上的
新模型目标。配置文件初始损坏时只保留离线输入，不改用默认服务；外观缓存的联想 On 也不能
覆盖输入法配置的 Off。确认重载切换提供器后，保留当前草稿、清旧提交上下文与未使用译文。
翻译发送前另外核对磁盘上的目标和授权，冲突或读取失败时不发请求，需先重新加载。
详见 [第六轮模型链路修复](bug-audit-model-2026-09-13.md)；保存配置不等于即时重载运行中的宿主。

compatible 完成状态只接受 `stop`，以及兼容旧接口的缺失字段/`null`；工具转交、未知状态、
错误类型、截断/过滤回复不能进入候选或翻译。失败仍保留本地输入，不会执行模型工具调用。
会发送当前输入片段、本地转换和本次焦点会话内最近最多 160 字符提交上下文；
不采集桌面周边文本、不落盘输入历史。服务商如何保留和处理数据取决于你选择的服务。
未授权、断网、超时、证书错误或无有效候选时仍可用离线候选。

HTTPS 使用 Rustls 验证证书。不跟随重定向、不使用系统代理、不自动重试，响应上限 128 KiB。
`llm_timeout_ms` 可设为 20–5000 ms。云端接口的超时/格式兼容不代表模型语言质量已验证。
CLI 更换端点、模型、协议、部署位置或密钥引用时撤销旧授权，需要再次明确授权。
手动编辑 JSON 等同直接修改配置，务必同时核对 `llm_cloud_consent`。

回到本地：`suzaku_tool model configure`，然后从托盘重载。
撤销云端授权：`suzaku_tool model configure --cloud-consent false`，然后从托盘重载。
编辑、提交、切框、禁用联想或重载会作废旧结果，并通知在途候选请求取消。当前本地 HTTP
提供器在已连接的读写过程中约每 50 ms 检查一次取消，关闭旧连接，让最新草稿不必等待旧
推理的完整期限；模型清单请求同样支持取消，取消不会写入“未找到模型”的缓存。
这不是 50 ms 端到端延迟保证：连接建立和共享发现锁仍可能等待；云端阻塞 HTTPS 与未实现
协作取消的自定义提供器仍等待原调用完成/超时，再丢弃旧结果。翻译仍使用自身的异步期限。
关闭连接不能撤回服务已收到的数据，也不能保证任意服务立即停止计算。完整响应校验、
120 ms 防抖和默认 1200 ms 候选期限不变，见 [本地取消审计](bug-audit-model-cancellation-2026-09-24.md)。
重载模型配置保留当前 IBus 草稿；更换模型、端点、协议、部署位置或密钥引用时，清除
旧服务的已提交上下文与撤销历史，新服务只接收当前草稿。语言切换仍会清空原组合。

## 备份与迁移

旧配置未设置新字段时按本地协议处理，显式模型名称不变；新安装默认自动发现。
数据备份支持新配置字段，仅含密钥环境变量名，不含值。恢复预览和实际应用都会将
云端授权关闭，避免把旧机器的发送许可带到新机器。离线配置与面板偏好照常恢复。

协议参考：[Ollama 模型清单](https://docs.ollama.com/api/tags)、
[llama.cpp server](https://github.com/ggml-org/llama.cpp/tree/master/tools/server)。
