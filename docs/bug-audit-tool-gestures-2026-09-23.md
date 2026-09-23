# 工具采用手势与输入目标审计 · 第十轮（2026-09-23）

版本归档：下文保留本轮检查时的工作区状态；N23 修复现已纳入
[0.5.8 源码](releases/0.5.8.md)，不表示已推送、安装或提供分发包。

沿 [功能链路网络](functional-network.md) 的 F29/F30/F32/F33，继续检查面板接收原生
快照时，尚未完成的鼠标/触摸手势是否仍属于同一输入目标。基线为 `0.5.7`，提交
`803d9fe75b0ec3cf8a2c89302b0a37350bd13fba`，叠加第八、九轮工作区修复。
本轮确认 **N23：语音、手写的旧采用手势跨输入目标回填**，已在工作区修复，尚未发布。
没有安装、提交、推送或改变个人桌面输入法、服务和模型配置。

## N23：按下后换框，松开时采用到新目标

优先级 P2，影响工具结果的手动回填，不是最终提交。

复现序列：

1. 原生输入框 A 的草稿为 `alpha`，展开语音或手写工具，准备合成结果 `world`。
2. 在「插入转写」或手写候选上按下鼠标/触摸，尚未松开。
3. 面板接收到输入框 B 的新快照，草稿变为 `beta`，控件仍在原位置。
4. 松开旧手势。旧实现会向 B 排入替换预编辑的请求 `beta world`。

新增回归在修复前实际输出：

```text
AUDIT: N23 Dictation, touch=false, new context: old press survived
AUDIT: N23 Dictation, touch=false, new context: stale release sent A00000000-0000-0000-0000-000000000001 11 Tbeta world
AUDIT: N23 Handwriting, touch=true, new context: old press survived
AUDIT: N23 Handwriting, touch=true, new context: stale release sent A00000000-0000-0000-0000-000000000001 11 Tbeta world
```

请求中的 `T` 是替换草稿，不是提交。它使用 **新目标的最新版本**，因此宿主正常的
旧版本拒绝无法拦住；错误发生在面板把旧手势解释为对新目标的明确采用时。
验证使用私有 Xvfb/D-Bus、真实 `PanelState` 和内存动作队列：这是已复现的错误目标
请求，不是声称写入了个人应用、使用真实录音，或绕过了密码框保护。

### 根因与修复

[`receive_native_frame`](../src/bin/panel/native_sync.rs) 原先取消旧候选按压，并在目标
变化时取消屏幕键盘按压，却没有包含 `InsertVoiceTranscript` 和
`UseHandwritingCandidate`。[释放处理](../src/bin/panel/controller.rs) 校验控件、位置与
时间后，会按当前原生快照构造请求。语音采集目标绑定保护的是自动回填，来源代次保护
的是确认后清理，两者均不能替代手动采用手势的目标边界。

现在：

- 语音插入、手写候选和屏幕键盘共用原生目标连续性检查：宿主、输入框、语言必须一致，
  前后快照都须处于公开且聚焦状态，而且按下时已经在原生视图。
- 收到换框、宿主重启、语言切换、断连、隐私或失焦变化时，取消尚未完成的采用手势，
  同时取消待执行的触摸点击。从本地编辑进入原生镜像也不能沿用旧手势。
- 检查提前到面板焦点/设置的提前返回之前，避免已观察到的离开再返回使旧按压重新生效。
  即使从不可用目标开始按下，后来恢复就绪，也须重新点击。
- 保留转写、手写候选和笔迹，允许在新目标上明确重新点击；不自动重放，不改动提交规则。
- 同一公开输入框的候选/元数据刷新或草稿编辑不误取消追加操作，追加使用最新草稿；
  独立面板仍保持本地插入。句子和词候选继续使用更严格的快照变化取消规则。

这次不改变已发送动作的确认协议、来源清理或独立编辑器的词边界规则。

## 回归范围与入口

[工具链回归](../src/bin/panel/tool_chain_audit_test.rs) 新增
`tool_adoption_gestures_must_not_cross_native_targets`，共 **16 种情形 × 2 种工具 ×
鼠标/触摸 = 64 组组合**：

- 正常同框、元数据刷新、同框草稿编辑。
- 换输入框、换宿主、换语言、断连再连接。
- 隐私后恢复、失焦后恢复、输入框离开再返回、设置打开期间变化。
- 从隐私、失焦或断连状态开始按压，随后目标恢复。
- 从本地编辑切入原生视图，以及保持本地编辑的对照。

测试使用实际布局中的命中区域，经过真实按下/释放处理，检查旧手势取消、来源保留、
新点击仍可采用、重复释放不重复插入。不以窗口布局偶然变化导致命中失败作为修复依据。
目标状态由合成快照驱动，不向用户桌面注入鼠标、触摸或按键。

已加入 [工具专项入口](../scripts/test-tool-chain-audit.sh) 和
[Linux UI CI](../scripts/test-linux-ci.sh) 的必跑列表；两者核对精确测试名，防止改名后
零测试通过。新报告同时加入 [Linux 文档打包清单](../scripts/package-linux.sh)。

```bash
export LC_ALL=C CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export LIBGL_ALWAYS_SOFTWARE=1
export __EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json
bash scripts/test-tool-chain-audit.sh
bash scripts/test-linux-ci.sh ui
bash scripts/test-linux-ci.sh ibus
cargo test --quiet --locked --all-features -- --test-threads=1
cargo clippy --locked --all-features --all-targets -- -D warnings
cargo clippy --locked --features gpu --all-targets -- -D warnings
python3 scripts/check-package-audit-links.py .
```

## 本轮验证结果

- 常规全功能测试：**823 通过、0 失败、22 项隔离测试按默认规则忽略**。忽略项不算
  已通过；本轮显式运行的隔离链路见下列专项结果。
- 工具专项三项测试通过，包含 N05/N06 原有保护及 N23 的 64 组组合；新回归先在旧逻辑
  复现错误目标请求，再在修复后通过。
- 私有 Xvfb 的 **14 组面板窗口测试及 1 组程序入口测试全部通过**，覆盖原有候选、
  窗口、设置、翻译、模型绑定、异步来源保护和单实例入口。
- 私有 IBus 全链通过，包含英中日连续草稿、选词/撤销/提交、焦点/隐私、激活/释放、
  通道与宿主重启等；不连接个人桌面会话或实际模型。
- `--all-features`、`--features gpu` 两套全目标严格 Clippy 通过，无警告。
- 格式检查、`git diff --check` 和三个相关脚本的 ShellCheck 通过；源码目录下
  **23 个审计文档链接**校验通过，新报告已加入 Linux 明确打包清单。

这轮未重新验收真实 GNOME/Wayland 会话、其他平台、真实语音识别或模型质量；没有
构建新分发包或重跑安装/升级/卸载。源码文档链接检查不等于成品包验收。
