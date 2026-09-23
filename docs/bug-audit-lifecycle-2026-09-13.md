# 启动、退出与失败重试审计 · 第八轮（2026-09-13）

版本归档：下文保留本轮检查时的工作区状态；N19/N20 修复现已纳入
[0.5.8 源码](releases/0.5.8.md)，不表示已推送、安装或提供分发包。

沿 [功能链路网络](functional-network.md) 的 F02/F03/F04/F06，检查面板/托盘与受管
IBus 宿主的启动、激活、释放、退出，以及失败后再次操作。
基线为 `0.5.7`，提交 `803d9fe75b0ec3cf8a2c89302b0a37350bd13fba`。
本轮确认 **N19：停止服务后的恢复失败被第二次退出跳过**、**N20：未启动完成的服务
丢失清理责任**，已在工作区修复，尚未发布。
没有安装、提交、推送或改动个人输入法配置。

## N19：服务已停，不代表输入法已恢复

优先级 P2，影响托盘完整退出的失败重试。

[`InputMethodController::shutdown`](../src/bin/panel/input_method.rs) 原先只在一次调用
里记录停止后的恢复任务。如果移除宿主后 IBus 没有全局引擎，控制器会尝试恢复停止前
观察到的输入法；但恢复失败后，没有保存这项任务仍未完成的状态。第二次退出发现服务
已经停止，直接返回成功，[托盘工作线程](../src/bin/panel/tray.rs) 据此发送退出事件。

故障回归使用合成后端注入“停止后没有全局引擎”，再分别令恢复命令失败、或命令成功但
实际上没有切换。修复前，第二次退出都错误返回成功：

```text
N19: a second Quit reported success with no restored input method
N19: late service stop completion must retain the observed recovery target
```

现在停止前将已观察到的回退输入法存入控制器的未完成恢复状态，直到有效读回或恢复并
确认成功才清除。停止命令丢失确认、但请求稍后实际完成时，也保留这个状态。
若服务仍保留待确认的清理责任，重试可以先修复缺失引擎，再完成停止确认，不会卡在
“必须先读到当前引擎、却不再尝试恢复”的循环。

保护边界：

- 用户手动切到有效的其他输入法时，保留该选择，不强制恢复旧目标。
- 命令返回成功但读回失败/目标不匹配，仍视为恢复未确认，不放行退出。
- 没有曾观察到的安全回退目标时，不猜测默认输入法、不盲目停止宿主。
- 已成功完成的退出重试不重复切换或停止；单独“释放”仍不停止服务。

## N20：inactive 状态仍可能有排队中的启动请求

优先级 P2，影响启动失败/超时后退出。

[`HostService`](../src/bin/panel/input_method_service.rs) 使用非阻塞的服务启动请求。
启动命令超时也可能已经把任务交给服务管理器，因此原实现正确地在发命令前记住清理
责任；但 `needs_stop` 又在看到非运行状态时清掉它。

服务启动等待依赖时，单元可以仍为 `inactive`。此时就绪探测超时后点击退出，原实现
不再发送停止/取消请求，排队的启动任务可能在面板退出后才执行。
本机 systemd 接口文档将 `ActiveState` 与当前调度任务 `Job` 分别定义，任务的
`waiting` 状态不等于服务已经开始运行；单看运行状态不足以证明没有待启动任务。

合成服务传输层模拟“启动已排队、单元仍 inactive”，分别覆盖正常命令确认与确认丢失。
修复前两个新增测试失败：

```text
N20: inactive state discarded ownership of a queued start
a failed cancellation must be retryable
```

现在只有单元确实不存在，或显式停止/取消已确认后，才释放清理责任。已接管的 inactive
单元仍收到停止请求，从而取消排队的启动；取消失败继续保留重试机会。未接管的外部
宿主、自定义通道和私有测试服务仍不会触发桌面服务管理命令。

## 回归入口与验证边界

新增 5 项普通测试，无需真实桌面会话，随现有 Linux 测试套件执行：

- `shutdown_retry_must_finish_post_stop_recovery_before_reporting_success`：两类切换失败、
  多次失败重试、恢复后重复退出无副作用。
- `shutdown_recovery_retry_requires_readback_and_preserves_manual_switches`：读回失败仍
  拒绝退出，手动选择其他输入法不被覆盖。
- `shutdown_retry_recovers_when_an_unconfirmed_stop_finishes_later`：迟到的停止完成，
  分别覆盖清理责任已释放/仍需确认两种分支。
- `queued_start_is_cancelled_on_quit_even_while_the_unit_is_inactive`：排队启动、启动
  确认丢失、实际取消及重复取消无副作用。
- `failed_cancellation_remains_owned_but_a_removed_unit_can_be_released`：取消失败可重试，
  单元被移除后才允许释放责任。

```bash
export LC_ALL=C CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
cargo test --locked --all-features --bin panel input_method:: -- --test-threads=1
cargo test --quiet --locked --all-features -- --test-threads=1
bash scripts/test-linux-ci.sh ibus
LIBGL_ALWAYS_SOFTWARE=1 \
__EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json \
bash scripts/test-linux-ci.sh ui
cargo clippy --locked --all-features --all-targets -- -D warnings
cargo clippy --locked --features gpu --all-targets -- -D warnings
python3 scripts/check-package-audit-links.py .
```

## 本轮验证结果

- 定向控制器/服务回归：**28 通过、0 失败、1 项原生隔离测试默认忽略**。
- 常规全功能测试：**823 通过、0 失败、19 项隔离测试按默认规则忽略**，不将忽略项
  算作已通过；新增 5 项回归均在普通套件内。
- 私有 IBus 全链通过，包括真实激活、英中日连续草稿/数字选词/撤销/提交、重复释放与
  清理恢复，以及焦点/隐私/通道/宿主重启回归。
- 私有 Xvfb 窗口套件：**12 组全部通过**，包括窗口收放、键盘编辑、快速候选采用与
  回退、设置确认、模型重载边界、工具回填、翻译、状态查询与字体渲染。
- `--all-features` 和 `--features gpu` 两套全目标严格 Clippy 均通过，无警告。
- 格式检查、`git diff --check`、打包脚本 ShellCheck 通过；源码目录下 **21 个审计
  文档链接**校验通过，新报告已加入 Linux 明确打包清单。

N19/N20 的故障时序使用合成后端确定性复现，不宣称已在真实 GNOME/systemd 会话中
强制复现。私有 IBus 验证另走仓库已有隔离入口，不管理个人桌面服务；没有重跑
容器安装/升级/卸载，也没有构建或安装新分发包。源码文档链接校验不等于成品包验收。
