# 工具回填后的连续输入审计 · 第十二轮（2026-09-23）

沿 [功能链路网络](functional-network.md) 的 F35 → F30，检查采用译文后继续使用屏幕键盘，
并覆盖 F32/F33 手写、语音的共用回填路径。基线为 `0.5.8`，提交
`14984b9aa14f6d1b5d016e7ef1c06812f929c549`，保留第十一轮尚未提交的 N24 修复。
确认 **N25：成功确认先于新快照时，连续输入沿用旧原文和旧版本**，已在工作区修复，
尚未提交、发布或安装；不修改个人桌面输入法、服务、模型或配置。

## N25：采用成功后，下一次键入仍接在原文上

优先级 P2，连续输入工作流。动作确认与草稿快照由不同线程、不同通道传入面板，
即使宿主先发布快照，面板也不能依赖固定的到达顺序。

使用私有 Xvfb/D-Bus/XDG 中的真实面板、固定翻译提供器、内存动作队列及合成快照：

1. 原生草稿 `original draft`，版本 100；译文为 `translated draft`。
2. 点击采用，面板发出绑定版本 100 的替换请求。
3. 模拟成功确认先到，尚未交付版本 101 的新快照。
4. 重新展开屏幕键盘，点击 `x`。旧实现立即发送版本 100 的 `original draftx`。

先加入回归、尚未改生产代码时实际输出：

```text
AUDIT: N25 early keyboard write after "translated draft": A00000000-0000-0000-0000-000000000001 100 Toriginal draftx
assertion `left == right` failed: N25: keyboard must continue the acknowledged insertion
  left: "original draftx"
 right: "translated draftx"
```

宿主的版本检查会拒绝过期动作，不能把上述记录解释成目标应用被错误写入。
可确定的问题是面板继续编辑错误基准并发出必然过期的请求，随后进入拒绝/恢复流程，
打断正常续写。语音和手写采用共用同一确认路径，回归也覆盖它们。
本轮没有调用真实模型、录音、真实输入应用或个人 IBus 会话。

## 修复边界

[`native_sync.rs`](../src/bin/panel/native_sync.rs) 在发送带来源的替换时保留文本与输入目标：

- 成功确认先到且仍是同一原生版本时，保存已确认文本作为下一次编辑基准，**不伪造新版本**。
- 下一次屏幕键盘编辑继承这个已确认的在途状态，复用现有
  [`NativeTyping`](../src/bin/panel/native_typing.rs) 的合并和确认保护；匹配的新快照到达后，
  才用其真实版本发送合并后的文本。等待期间其他原生动作也不能复用旧版本。
- 如果用户还没继续编辑，下一份有效快照直接接管，包括已经合并掉中间状态的实体键盘修改。
  不把已确认文本重新发送，也不让旧文本覆盖最新快照。
- 已有后续编辑时，冲突、断连继续保留本地恢复文本；换宿主、输入框、语言、隐私或焦点
  边界不携带旧草稿。重复确认/旧快照不解除保护。
- 点击输入框显式转入独立编辑时，可以恢复已确认文本；不会把恢复视为再次发送。
- 迟到确认不覆盖已存在的本地编辑缓冲，也不改写新翻译或新工具的状态；失败/未确认的
  回填仍保留来源、不自动重试。原有来源代次和工具收起规则不变。

此次不改变宿主协议、物理键盘的输入规则、模型能力、翻译质量或提交语义。

## 回归入口

- [原生回填测试](../src/bin/panel/native_sync_test.rs)
  `native_source_insertions_preserve_drafts_until_acknowledged`：语音和手写各验证确认先到、
  快照先到后真实屏幕键盘连续点击，最终文本和发送版本准确，已确认文本不重放。
- [翻译测试](../src/bin/panel/translation.rs)
  `native_translation_preserves_drafts_and_rejects_stale_contexts`：使用相同续写检查验证译文，
  保留原有取消、新任务、新草稿、换框、隐私、另一工具及界面语言切换覆盖。
- 共用回填另外新增 **15 种边界 × 语音/手写 = 30 组**：正常/实体修改/同值回退的最新
  快照；有未发编辑的冲突、同值回退和断连；五种输入绑定变化；未编辑时的断连恢复；
  显式本地恢复；确认到达前已转入本地编辑或已有暂存键入。包含 Unicode 和退格。

两个测试均在 [Linux UI CI](../scripts/test-linux-ci.sh) 的精确必跑名单中，本轮扩展现有
测试，不把忽略项当作默认已通过。报告加入 [Linux 文档打包清单](../scripts/package-linux.sh)。

## 本轮验证结果

- 先在旧逻辑复现错误原文和过期版本，修复后 **三种来源 × 两种确认顺序 = 6 组**
  连续点击及 **30 组**共用边界场景全部通过。
- 常规全功能测试：**823 通过、0 失败、23 项隔离/显式启用测试按默认规则忽略**；
  未将忽略项计作通过，隔离运行结果另列。
- 私有 Xvfb：**15 组面板测试和 1 组程序入口测试全部通过**，包括上轮 N24 的
  34 组翻译请求手势回归，以及原有迟到确认、来源代次、同值回退和新任务保护。
- 私有 IBus 全链通过：英中日连续输入、选词/撤销/提交、配置重载、焦点/隐私、
  激活/释放、通道与宿主重启。不操作个人桌面会话。
- `--all-features`、`--features gpu` 两套全目标严格 Clippy 通过，无警告。
- 格式、`git diff --check`、两项相关脚本 ShellCheck 通过；源码目录下
  **30 个审计文档链接**校验通过，报告已加入明确打包清单。

复核入口：

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

本轮不构建分发包、不重跑容器安装升级，也不宣称完成真实 GNOME/Wayland、其他平台、
真实模型或麦克风验收。源码文档链接校验不等于成品包验收。
