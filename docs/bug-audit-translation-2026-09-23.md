# 翻译请求与取消边界审计 · 第十一轮（2026-09-23）

沿 [功能链路网络](functional-network.md) 的 F34/F35，并检查它们与 F29 原生快照同步的
边界。基线为 `0.5.8`，提交 `14984b9aa14f6d1b5d016e7ef1c06812f929c549`。
确认 **N24：旧的 Translate 按压在目标/原文变化后仍发送新草稿**，已在工作区修复，
尚未提交、发布或安装。本轮不推送，不改变个人桌面输入法、模型或用户配置。

## N24：结果可以丢弃，但请求已把另一份原文送出

优先级 P1，请求来源隔离。与 N23 的“工具结果送入新输入框”不同，这次错误发生在
**点击翻译、向模型发出原文之前**，不是翻译质量或已发送结果的采用问题。

复现使用私有 Xvfb/D-Bus/XDG、真实面板状态与请求实现，以及仅监听随机本机回环端口的
`ModelRecorder`。所有原文、配置和回复都是合成数据；没有连接实际模型、云服务、
麦克风或目标应用。因此下述记录不是声称发生过真实隐私文本泄露。

1. 输入框 A 的草稿为 `alpha draft`，展开翻译工具。
2. 在 Translate 上按下鼠标或触摸，尚未松开。
3. 收到输入框 B 的新快照，草稿改为 `beta draft`，按钮仍在原位置。
4. 松开旧手势。旧实现启动一次新的翻译，模拟 HTTP 服务实际收到 `beta draft`。

修复前的回归输出：

```text
AUDIT: N24 touch=false, new context: stale release sent "beta draft" to the synthetic model
AUDIT: N24 touch=true, new context: stale release sent "beta draft" to the synthetic model
AUDIT: N24 touch=false, local edit: stale release sent "edited draft" to the synthetic model
```

换宿主、断连后返回、已观察到的隐私/失焦变化后返回，以及本地原文修改后又恢复，也有
同类问题。原文或输入目标变化时尚未创建翻译任务，因此已有的“原文/上下文绑定检查”
没有旧任务可取消；释放时才按新原文建立绑定，不能阻止这次错误请求。

### 根因与修复

- [`receive_native_frame`](../src/bin/panel/native_sync.rs) 已保护候选、屏幕键盘和
  语音/手写插入按压，但遗漏 `TranslateText`。现在翻译按压除要求原生宿主、上下文、
  输入语言及公开焦点连续一致，还要求原文相同；目标或原文变化即取消旧按压及待执行触摸。
  相同输入框的纯版本/元数据刷新不会无故取消翻译按钮。
- [`cancel_translation`](../src/bin/panel/translation.rs) 原先只作废 worker 任务和结果。
  现在即使还没有任务，也撤销尚未释放的 Translate 手势。本地编辑、源/目标语选择与
  既有取消/重载路径因此不会留下可重新触发请求的旧按压。
- 不发送替代请求，不改变草稿。用户重新点击后仍能翻译当前原文；既有结果确认、
  原生回填、云端授权、已发送调用的超时/取消能力均不改变。

翻译与语音/手写追加的规则刻意不同：追加操作可以基于同一输入框的最新草稿，翻译会把
整份原文送往模型，原文变化后必须重新点击。此次没有改变候选刷新或连续输入语义。

## 回归入口与范围

[新增隔离回归](../src/bin/panel/functional_network_audit_test.rs)
`translation_requests_reject_stale_press_targets` 共 **17 种情形 × 鼠标/触摸 = 34 组**：

- 正常原生目标、仅版本刷新、正常本地编辑三种对照。
- 同框原文变化、换框、宿主重启、输入语言变化。
- 断连、隐私、失焦、上下文离开后返回。
- 本地修改、本地修改再恢复、从本地转入原生视图。
- 取消、修改翻译源语言、修改翻译目标语言。

使用真实布局命中区域和按下/释放处理，并读取模拟服务收到的 HTTP 原文。
同时确认重新点击可发送当前草稿、重复释放不重复请求、原草稿与提交状态不变。
从本地切入原生会折叠工具，因此该情形独立检查旧按压已被清理，不把偶然命中失败算作
正确保护。所有网络流量均来自测试拥有的回环服务，不使用默认模型端口。

该测试已加入 [Linux UI CI](../scripts/test-linux-ci.sh) 的精确必跑名单，防止改名后
零测试通过。报告已加入 [Linux 文档打包清单](../scripts/package-linux.sh)。

```bash
export LC_ALL=C CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
LIBGL_ALWAYS_SOFTWARE=1 \
__EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json \
bash scripts/test-linux-ci.sh ui
cargo test --quiet --locked --all-features -- --test-threads=1
bash scripts/test-linux-ci.sh ibus
cargo clippy --locked --all-features --all-targets -- -D warnings
cargo clippy --locked --features gpu --all-targets -- -D warnings
python3 scripts/check-package-audit-links.py .
```

## 本轮验证结果

- 常规全功能测试：**823 通过、0 失败、23 项隔离测试按默认规则忽略**；不把忽略项
  计作已通过，本轮显式隔离验证见以下结果。
- 新回归先在旧逻辑复现错误 HTTP 原文，再在修复后通过全部 34 组组合。
- 私有 Xvfb 的 **15 组面板测试和 1 组程序入口测试全部通过**；原有八语翻译、
  确认顺序、迟到结果、模型配置/授权、自动语音回填与手势隔离保护继续通过。
- 私有 IBus 全链通过，覆盖英中日连续输入、选词/撤销/提交、焦点/隐私、激活/释放、
  通道与宿主重启；不操作个人桌面会话。
- `--all-features`、`--features gpu` 两套全目标严格 Clippy 通过，无警告。
- 格式检查、`git diff --check`、两项相关脚本 ShellCheck 通过；源码目录下
  **28 个审计文档链接**校验通过，新报告已加入明确打包清单。

本轮没有重跑真实 GNOME/Wayland、其他平台、真实模型语言质量或容器安装/升级/卸载，
没有构建分发包。源码链接检查不是成品包验收；不将合成故障时序等同于全面桌面验收。
