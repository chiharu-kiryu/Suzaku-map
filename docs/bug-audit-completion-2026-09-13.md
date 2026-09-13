# 候选采用、回退与继续输入审计 · 第七轮（2026-09-13）

沿 [功能链路网络](functional-network.md) 的 F11/F12/F13、独立面板 F31 与内容布局 F36
检查连续输入。
基线为 `0.5.6`，提交 `efc6aac641cd0d80997e3d0796c9eb8fc06f1336`，叠加前六轮
尚未发布的修复。本轮确认 **N17：快速连续点选/回退被错误去重**、**N18：词候选耗尽
后回退入口消失**，均已在工作区修复并通过本轮验收，待发布。
没有安装、提交、推送或改变个人输入法配置；窗口和原生验证只在私有测试环境进行。

## N17：按钮位置被当作编辑身份

优先级 P2，影响独立编辑模式的词候选按钮及 Back 回退按钮。
不是 IBus 数字选词失效，也不是补全模型没有返回结果。

[`select_next_token` / `rewind_next_token`](../src/bin/panel/composition.rs) 原先复用了
260 ms 的通用操作去重：只比较 `InteractionKind`（包括按钮索引），不区分草稿与编辑内容。
同一位置已显示下一个词时，新点击仍被看作前一次点击；Back 的操作标识始终相同，也会
丢弃这个时间窗口内的第二次回退。

隔离窗口中的实际事件链：

1. 输入 `hel`，按下/释放第一个词按钮，得到 `hello`。
2. 第一个词按钮更新成 `world`，再次按下/释放同一槽位。
3. 期望草稿为 `hello world`；修复前仍是 `hello`，没有报错。

新增回归在修复前失败：

```text
N17: new word in the same slot was mistaken for a duplicate; touch=false, step=1
left:  "hello"
right: "hello world"
```

测试通过当前场景的命中区域驱动真实按下/释放处理；第二次释放前将旧操作的时间设为当前
时刻，使“260 ms 内”的条件确定成立，避免依赖 CI 的绘制速度。没有修改候选或跳过点击流程。

## N18：没有下一个词时，可撤销历史也一起被隐藏

优先级 P2，影响独立面板 F31/F36。

N17 修复后，上面的真实事件链终于能到达 `hello world`。此时离线词候选为空，历史仍有
`hello`、`world` 两条，但下一次查找 Back 命中区域失败，用户无法通过面板撤回刚才的补全。

原因是三处布局判断都只看 `next_token_candidates` 是否为空：

- [高度测量](../src/ime/gpu/theme_metrics.rs) 不再为词区保留任何高度。
- [词区绘制](../src/ime/gpu/panel_scene_next_tokens_block.rs) 跳过整个区域，连历史标题与
  Back 按钮一起跳过。
- [句候选定位](../src/ime/gpu/panel_scene_sentence_candidates_block.rs) 同样把词区高度当成零。

新增独立布局回归在修复前直接失败：

```text
N18: missing Back with pending history at 420, Small, expanded=false, VirtualKeyboard, sentences=0
```

现在共用同一份“候选或待撤销历史”高度测量。没有词候选但仍有历史时保留单行回退区，
展开模式不再保留空词按钮行；字号放大时标题仍在卡片边界内。句候选从该行之后开始，
回退清空历史后窗口重新收紧。展开的翻译工具中仍隐藏输入候选与回退，不占多余高度。

## 修复与保留的保护

- 只移除词采用和回退路径上的按槽位/时间去重；新的按下/释放可以立即应用新的编辑。
- [释放处理](../src/bin/panel/controller.rs) 仍一次性消费按下状态，没有新按下的重复释放
  不会继续选词或多退一步；不会改变窗口按钮、设置按钮或真正提交的去重规则。
- [`CompletionHistory`](../src/panel_support.rs) 仍核对原始草稿，拒绝不属于当前草稿的旧
  编辑；撤回后新的手势可以重新采用同一个词。手工修改后的旧回退记录仍会被清理。
- 候选在按下后被异步结果替换时，仍取消这次手势，不能按旧索引采用新内容。
- 操作只改变未提交草稿，不产生外部提交或新提交上下文。

独立面板的 Back 保留最多 32 次词采用记录，普通手工编辑会清掉这段记录；这不是任意编辑
历史。IBus 的即时 Backspace 补全撤销仍是原有一步语义，两种模式没有被混成一套行为。

## 回归与重跑

[私有窗口候选回归](../src/bin/panel/candidates_native_test.rs) 增加鼠标和触摸两组路径：
快速 `hel → hello → hello world`、连续两次回退、重复释放不多做一步、回退后重新采用；
同时检查界面草稿、引擎草稿、历史标签和提交上下文。原有异步候选变更取消点击、词/句
提交确认/失败保留继续执行。该测试由现有 Linux UI CI 强制运行，不新增可漏跑的手动入口。

新增普通单元测试检查旧编辑载荷重复应用被拒绝、连续回退与重新采用、手工修改使旧历史失效。

[普通布局回归](../tests/ime_panel_content_fit.rs) 覆盖 240 个组合：5 种窗口宽度
（420/630/900/1600/2500）、3 种字号、展开/折叠、键盘/语音/手写/翻译、0/3 个句候选。
检查回退与句候选的实际命中、标题及按钮文字留在卡片内、不侵入句候选卡片、没有空词按钮、
高度重算稳定、历史耗尽后收紧，以及翻译模式不显示这段历史。

```bash
export LC_ALL=C CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
cargo test --locked --all-features --lib \
  panel_support::tests::completion_history_rejects_stale_payloads_but_allows_reapply_after_undo -- --exact
cargo test --locked --all-features --test ime_panel_content_fit --test ime_engine_ui_alignment
LIBGL_ALWAYS_SOFTWARE=1 \
__EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json \
bash scripts/test-linux-ci.sh ui
bash scripts/test-linux-ci.sh ibus
cargo test --quiet --locked --all-features -- --test-threads=1
cargo clippy --locked --all-features --all-targets -- -D warnings
cargo clippy --locked --features gpu --all-targets -- -D warnings
```

## 验证结果

- 常规全功能测试：**818 通过、0 失败、19 项隔离测试按默认规则忽略**；不把忽略项
  算作已通过。新增历史单元测试和 240 组合的布局回归均进入普通套件。
- 私有 Xvfb 窗口套件：**12 组全部通过**，包括鼠标/触摸快速采用与回退、重复释放、
  异步候选替换保护、提交交付，以及既有窗口/设置/工具/翻译/字体回归。
- 私有 IBus 全链通过：英中日空格不断流、数字选词、采用后撤销、继续输入、显式提交、
  焦点/隐私边界和宿主重启等；本轮没有改动原生一步 Backspace 撤销语义。
- `--all-features` 和 `--features gpu` 两套全目标严格 Clippy 均通过，无警告。
- 格式检查、`git diff --check`、相关 Linux 脚本 ShellCheck 通过；源码目录下的
  14 个审计文档链接校验通过，新报告同步加入明确的打包清单。

以上为本轮实际重跑的结果，不把前六轮或旧二进制结果计作本轮通过。没有重跑容器安装/
升级/卸载，也没有构建新发布包、安装、提交或推送；源码文档链接校验不等于成品包验收。
