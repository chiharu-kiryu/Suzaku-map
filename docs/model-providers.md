# 模型服务：本地优先，与具体模型解耦

中、英、日文共用 `LlmCompletionProvider` 候选接口。`HttpModelProvider` 根据协议封装请求，
不根据模型名称判断行为。`languages::llama` 与 `suzaku_tool llama` 保留兼容入口；
新增适配器可以实现该候选接口，不必改动键盘事件、离线词库或语言转换。

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

请求携带 `candidate_groups: {"word": 3, "sentence": 3}`，响应建议采用：

```json
{"candidates":[{"text":"hello","kind":"word"},{"text":"hello world","kind":"sentence"}]}
```

Ollama 使用对应 JSON schema；compatible 服务通过系统提示请求相同 JSON，也兼容旧版
逐行纯文本候选。每次最多 6 项，默认输出预算 256 tokens；拒绝截断
JSON、未知类型和控制字符。开发版统一了长草稿的长度预算：完整候选可保留最多 256 字符的
精确输入/转换前缀，再新增最多 160 字符（总长最多 416）；没有匹配前缀的转换结果仍限 160 字符。
解析与引擎混排使用同一预算，不截断前文，不放宽响应体大小或控制字符校验；超过 256 字符的
输入仍走有界本地回退。旧接口的字符串数组仍接受最多 3 项，缺少类型时由语言层回退
分类。模型的排序提示经本地限幅，不能覆盖输入首项或把展示注解写进正文。
具体混排与按键见 [IBus 候选体验](ibus-candidates.md)。
英文候选从响应解析、混排到提交都保留已输入的精确前缀，包括缩进、连续空格和列表标记；
仅清除前缀以外的响应留白，不补造模型漏掉的前缀。原样输入仍不会作为新联想接受。

开发版英文请求额外区分三种状态：未完成词 `complete_word`、已输入分隔空格等续写场景
`next_word`，以及 `a` / `can` 等既可能是完整词又可能是词头的 `complete_or_continue`。
`word_prefix` 和 `text_before_word` 明确当前词及其前缀；候选仍必须是完整草稿替换文本，
不是仅返回末词。这个规则同时用于 Ollama 和 compatible 协议，不依据具体模型名称分支。
IBus 混排还能从有效英文句子中提取独立词候选，细节与回归范围见
[英文补全质量回归](ibus-candidates.md#英文补全质量回归开发版)。

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
会发送当前输入片段、本地转换和本次焦点会话内最近最多 160 字符提交上下文；
不采集桌面周边文本、不落盘输入历史。服务商如何保留和处理数据取决于你选择的服务。
未授权、断网、超时、证书错误或无有效候选时仍可用离线候选。

HTTPS 使用 Rustls 验证证书。不跟随重定向、不使用系统代理、不自动重试，响应上限 128 KiB。
`llm_timeout_ms` 可设为 20–5000 ms。云端接口的超时/格式兼容不代表模型语言质量已验证。
CLI 更换端点、模型、协议、部署位置或密钥引用时撤销旧授权，需要再次明确授权。
手动编辑 JSON 等同直接修改配置，务必同时核对 `llm_cloud_consent`。

回到本地：`suzaku_tool model configure`，然后从托盘重载。
撤销云端授权：`suzaku_tool model configure --cloud-consent false`，然后从托盘重载。
禁用联想或重载不会撤回已开始发送的请求；旧结果不会覆盖新的输入状态。
重载模型配置保留当前 IBus 草稿；更换模型、端点、协议、部署位置或密钥引用时，清除
旧服务的已提交上下文与撤销历史，新服务只接收当前草稿。语言切换仍会清空原组合。

## 备份与迁移

旧配置未设置新字段时按本地协议处理，显式模型名称不变；新安装默认自动发现。
数据备份支持新配置字段，仅含密钥环境变量名，不含值。恢复预览和实际应用都会将
云端授权关闭，避免把旧机器的发送许可带到新机器。离线配置与面板偏好照常恢复。

协议参考：[Ollama 模型清单](https://docs.ollama.com/api/tags)、
[llama.cpp server](https://github.com/ggml-org/llama.cpp/tree/master/tools/server)。
