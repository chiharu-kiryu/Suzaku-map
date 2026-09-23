# 原文候选选择与状态发布审计 · 第十四轮（2026-09-23）

沿 [功能链路网络](functional-network.md) 的 F11/F12/F13 → F29 检查 Linux / IBus
原生数字选词、续写/撤销、提交和伴随快照。基线为 `0.5.8`，提交
`14984b9aa14f6d1b5d016e7ef1c06812f929c549`；保留前几轮尚未提交的 N24–N27，
包括 [上轮原生候选与连续输入修复](bug-audit-native-actions-2026-09-23.md)。
本轮确认并修复 **N28**，不提交、推送、安装或改动个人桌面输入法配置。

## N28：选回原文后，候选栏仍显示旧选中项

优先级 P2，显示状态与实际提交目标不一致。以英文为例：

1. 输入 `hel`，按 Tab 高亮 `hello`。
2. 按 `1` 采用原文 `hel`，仍然留在草稿中。
3. 宿主内部已选中原文，但系统 IBus 候选栏和伴随订阅快照仍停在 `hello`，版本也未推进。
4. 回车实际提交 `hel`，与仍然高亮的 `hello` 不同。

中文第二页的原文、日文首页的原文也有同样问题。并非回车载荷被随意替换，
而是明确的选择已生效却没有对外发布；旧快照的版本也未因此失效。

### 复现证据

先在 [私有 IBus 夹具](../scripts/test-native-sync.py) 中加入回归，再修改生产代码。
使用实际 IBus 输入上下文、按键处理器、候选表事件及 Unix 订阅通道，不是仅调用引擎
内部方法。三种语言各四个后续操作，共 **12 组**均先复现。
下面的索引从 0 起；斜线两侧分别为伴随快照和 IBus 候选表：

```text
AUDIT: N28 en/commit: selected=1/1, expected=0; revision=1654, before=1654
AUDIT: N28 zh-Hans/commit: selected=6/6, expected=7; revision=1688, before=1688
AUDIT: N28 ja/commit: selected=0/0, expected=5; revision=1739, before=1739
```

修复前的回归同时确认：回车仍准确提交原文，空格仍接着原文写，退格仍删除原文末字。
因此不能把本问题描述成输入丢失、重复提交或跨输入框泄漏；它是候选选择的发布遗漏。

### 原因与修复

[`suzaku_ibus_engine_complete`](../src/linux/ibus_engine_bridge.c) 在“采用结果与现有草稿
相同且不追加空格”时直接返回。数字键此前已经调用选择操作，所以即使文字没变，
选中项也可能改变。该分支没有重绘原生候选表，也没有向伴随通道发布新版本。

现在在此分支调用已有的原生渲染/发布入口，使选中项和版本同步更新：

- **不重建草稿或候选列表**，不解除用户已锁定的选择，不额外发起模型请求。
- 不创建提交事件，不添加空格，也不为未变化的原文创建补全撤回记录。
- 保持当前页数字寻址；中文案例实际覆盖第二页，不假设原文必在首页。
- 新版本使选择前的伴随动作按原有规则被拒绝，协议格式和鉴权范围不变。

## 回归范围

新增 `check_literal_choice_publication` 已纳入现有 `scripts/test-linux-ci.sh ibus` 必跑链路。
三种语言 × 四种后续操作，共 **12 组**：

- 回车：原文精确提交一次并清稿。
- 空格：接着原文输入，不提交。
- 退格：删除末字符，不撤回到已经放弃的候选。
- 再次采用另一候选并立即退格：仍能恢复原拼写。

这四组分别使用无锁定修饰、Caps Lock、Num Lock、两者同时存在的事件状态；
不是四种操作与四种修饰键的全排列，也不改变真实键盘锁定状态或指示灯。
同时验证按键释放不处理、候选内容/顺序不变、两路选中项一致、版本前进、旧版本动作拒绝。
测试由固定本地词库生成候选，不依赖真实模型；完整夹具的模型测试仅连接自有回环模拟服务。

## 本轮验证结果

- 新增 **12 组专项回归全部通过**；三种语言均先观察到发布遗漏，再验证修复。
- 完整私有 IBus 链通过：中英日输入、候选翻页、采用/撤回/提交、Compose、异步候选、
  焦点/隐私、通道与激活/释放。未操作个人桌面会话。
- 常规全功能测试：**823 通过、0 失败、24 项隔离/显式启用测试按默认规则忽略**；
  忽略项不计作通过，隔离运行结果另列。
- 私有 Xvfb：**16 组面板测试和 1 组程序入口测试全部通过**，保留前几轮回归。
- `--all-features` 与 `--features gpu` 两套全目标严格 Clippy 通过，无警告。
- 格式、`git diff --check`、两项相关脚本 ShellCheck 通过；源码目录下
  **37 个审计文档链接**校验通过。

复核入口：

```bash
export LC_ALL=C CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
bash scripts/test-linux-ci.sh ibus
LIBGL_ALWAYS_SOFTWARE=1 \
__EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json \
bash scripts/test-linux-ci.sh ui
cargo test --quiet --locked --all-features -- --test-threads=1
cargo clippy --locked --all-features --all-targets -- -D warnings
cargo clippy --locked --features gpu --all-targets -- -D warnings
python3 scripts/check-package-audit-links.py .
```

本轮不构建分发包，不安装，不声称真实 GNOME/Wayland 桌面、其他平台或真实模型已完成验收。
报告加入 Linux 打包清单；源码文档链接校验不等于成品包验收。
