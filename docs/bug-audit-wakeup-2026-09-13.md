# 单实例启动与候选面板唤出审计 · 第九轮（2026-09-13）

版本归档：下文保留本轮检查时的工作区状态；N21/N22 修复现已纳入
[0.5.8 源码](releases/0.5.8.md)，不表示已推送、安装或提供分发包。

沿 [功能链路网络](functional-network.md) 的 F01/F05/F29，检查重复启动、手动隐藏、
切换输入上下文后的自动显示。基线为 `0.5.7`，提交
`803d9fe75b0ec3cf8a2c89302b0a37350bd13fba`，叠加第八轮工作区修复。
本轮确认 **N21：单实例保护失败后继续启动额外面板**、**N22：合并快照后新输入框
无法重新唤出候选面板**，已在工作区修复，尚未发布。
没有安装、提交、推送、切换个人输入法或改动真实桌面服务。

## N21：已有实例无法唤出，却继续创建第二个面板

优先级 P2，影响面板启动入口。

[单实例控制器](../src/bin/panel/instance.rs) 检测到运行锁已被占用后，会向已有实例的
`control.sock` 发送 `show`，并对失败进行有限重试。控制器会正确返回通道缺失/不可用
等错误，但 [`main`](../src/bin/panel.rs) 原先只打印错误，随后以 `instance = None`
继续创建面板、托盘和工作线程。单实例内部的并发锁回归不能发现这个入口层的放行。

新增 [程序入口回归](../tests/linux_panel_instance_cli.rs) 直接运行当前编译的 `panel`：

1. 在私有 Xvfb/D-Bus/XDG 环境中，由夹具持有真实文件锁。
2. 保留接收通道时，确认程序成功退出且夹具收到 `show`，证明显示环境和启动入口可用。
3. 移除接收通道、继续持有文件锁，再次启动程序。
4. 修复前没有拒绝额外启动，程序进入未受保护的面板运行流程，超过 5 秒的测试时限。

```text
N21: held lock with missing Show socket did not reject the extra panel within the deadline:
Command deadline exceeded
```

现在单实例检查失败直接返回带上下文的错误，不启动额外窗口或后台控制器。正常的主实例
启动与重复启动转交不变。额外覆盖不可用的控制通道、不可打开的运行锁，并确认没有删除
或改写夹具的其他文件。Linux 单实例机制不可用时不再降级成无锁多开；其他平台仍沿用
原有的平台实现。

该回归只把 `show` 视为已递交请求，不将数据报发送成功等同于已有窗口已经完成绘制。
测试超时由现有有界命令执行器终止其拥有的子进程组并回收子进程，不遗留额外面板。

## N22：只等待“不可见变可见”，遗漏直接切换输入框

优先级 P2，影响原生伴随面板的自动唤出。

[原生同步](../src/bin/panel/native_sync.rs) 为保持 UI 响应只保留最新快照。旧输入框失焦、
新输入框产生草稿时，中间的空快照或断连通知可能被后来的完整快照替换。
窗口层原先要求 `visible && !was_visible` 才自动显示：手动隐藏 A 的候选后，即使最新
快照已属于 B，旧快照和新快照都可见，仍不会唤出。宿主重启后直接收到新草稿也有同样问题。

[新增隔离窗口回归](../src/bin/panel/native_sync_test.rs) 使用真实面板状态、更新邮箱和
`NativeCompositionReady` 事件处理，合并空通知与新输入框快照。修复前失败：

```text
N22: coalesced new context stayed hidden; restart=false
```

现在按最新上下文、窗口当前是否隐藏，以及既有显示保护决定唤出，不要求 UI 先观察到
中间的空状态。无需额外轮询，不改变草稿、候选或提交规则。

回归覆盖并保留：

- 同一输入框中用户手动隐藏后，候选刷新不能反复弹出。
- 新输入框及新宿主身份的完整快照可唤出，即使中间通知被合并。
- 面板正在编辑、设置打开、非无焦点后端时不抢显示；保护解除后新快照可正常唤出。
- 隐私、失焦和空草稿不自动显示。
- 显式 Show 清除手动隐藏标记，且不随草稿结束自动隐藏；自动显示的面板则随草稿结束收起。
- 通过与面板相连的合成动作通道确认：显示变化不发送输入操作。

测试检查私有 X 服务器中该测试窗口的实际映射状态，而不立即读取 winit 的缓存可见性：
后者需处理 `MapNotify` 才从待显示变为可见，不能在同一个 UI 回调内据此判定显示失败。
这一窗口夹具不注入真实桌面按键，也不启动宿主或模型服务。

## 回归入口

两项新增测试需要隔离图形环境，默认忽略，由现有 [Linux UI CI](../scripts/test-linux-ci.sh)
显式运行。脚本先核对精确测试名，避免改名后出现零测试却通过的情况：

- 13 项面板窗口测试，包括新增上下文唤出回归。
- 1 项真实程序入口回归，覆盖正常转交和三类启动保护失败。

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

- 常规全功能测试：**823 通过、0 失败、21 项隔离测试按默认规则忽略**，不把忽略项
  计作已通过。原有单实例并发、旧版兼容、崩溃重启及第八轮退出回归均通过。
- 私有 Xvfb：**13 组面板窗口测试和 1 组程序入口测试全部通过**。两项新增隔离回归
  分别先复现旧逻辑失败，再在修复后通过；此前窗口、候选、设置、工具和模型边界继续通过。
- 私有 IBus 全链通过：真实激活与释放、英中日连续草稿、数字选词/撤销/提交、焦点与
  隐私、通道和宿主重启等，不使用个人桌面会话或实际模型。
- `--all-features`、`--features gpu` 两套全目标严格 Clippy 通过，无警告。
- 格式检查、`git diff --check`、相关脚本 ShellCheck 通过；源码目录下 **22 个审计
  文档链接**校验通过，新报告已加入 Linux 明确打包清单。

本轮不宣称真实 GNOME 会话/托盘扩展、原生 Wayland 或其他平台已完整验收；没有重跑
容器安装/升级/卸载或构建新分发包，源码文档链接检查不等于成品包验收。
