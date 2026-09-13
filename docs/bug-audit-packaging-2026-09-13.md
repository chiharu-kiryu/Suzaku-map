# Linux 分发与注册审计 · 第五轮（2026-09-13）

沿 [功能链路网络](functional-network.md) 检查 **F50–F53**，并回查与注册共用的 F07
诊断入口。基线为 `efc6aac641cd0d80997e3d0796c9eb8fc06f1336`（`v0.5.6`）加工作区
已有 N05–N10 修复。**N11–N14 已在工作区修复，待发布**。以下 N11–N14 的复现与根因
保留修复前记录；修复措施和当前验证入口见后文。

重新构建了四个发布程序和 `.tar.gz` / `.deb` 测试包；安装、升级、启动与卸载只在一次性
Ubuntu 24.04 / amd64 容器中执行。用户注册调用真实 `suzaku_tool`，但其 HOME、XDG 目录、
服务管理器、IBus、GNOME 设置和进程查询均使用合成夹具，不连接真实桌面。
没有安装到本机、启停用户服务、切换输入法、读写个人配置，未提交、推送或发布。

## 修复前审计结论

| ID | 优先级 | 链路 | 结果 |
| --- | --- | --- | --- |
| N11 | P2 | F52 注册失败 → 重试 | 首次注册在 reload / enable 阶段失败后，重试成功但漏掉自启 |
| N12 | P2 | F07 `diag` | 诊断名称的入口实际安装、启用、重启服务并添加输入源，帮助未说明副作用 |
| N13 | P2 | F07 / F52 桌面命令等待 | 状态、验证、注册和注销仍有不受程序超时约束的外部命令 |
| N14 | P3 | F50 / F51 包内文档 | 功能网络引用的三份新审计文档未打包，现有包验收仍通过 |

常规包结构和容器安装链路通过，并不抵消以上异常路径的问题。

## N11：首次注册失败后的残留文件，使成功重试漏掉自启

[`install_ibus_user_service`](../src/bin/suzaku_tool.rs) 仅用 `!unit_path.exists()` 判断
是否首次安装。它先写服务文件，再 reload、首次 enable、restart。reload 或 enable
失败后直接返回，保留刚写的服务文件；下一次调用因此跳过 enable。

隔离复现分别让 reload 和 enable **只失败一次**，随后恢复所有桌面命令：

```text
stage=daemon-reload: failed_install_left_unit=true, retry_success=true, active=true, enabled=false
stage=enable:        failed_install_left_unit=true, retry_success=true, active=true, enabled=false
```

用户没有主动禁用过 Suzaku，第二次注册也返回成功，但并未建立文档承诺的首次注册自启。
当前服务已启动，问题会延迟到下次登录或其他启动路径才显现；不表示打开面板后也一定
无法使用，因为面板有独立的服务启动逻辑。

与此不同，已成功安装后用户主动关闭自启，再注册时应继续保留关闭状态；原有对照测试
通过，修复不能简单改成“每次都 enable”。建议区分完整注册与半完成状态，或可靠撤销
首次失败留下的文件与服务状态，并覆盖重试及已有禁用偏好两条分支。

## N12：`diag` 并不是只读诊断

[`linux_register`](../src/bin/suzaku_tool.rs) 的 `diag` 分支直接运行 `linux_install`，
成功后才运行 `linux_verify`。这不是推测系统工具可能有副作用；真实 CLI 的隔离调用
确实产生了以下行为：

```text
diag_success=true
host_created=true, unit_created=true, marker_created=true, enabled=true
calls: daemon-reload → enable → restart → restore previous engine → gsettings set
```

此前未注册的环境变为已注册，添加了 GNOME 输入源。旧安装也会经过重写与 restart 路径。
根命令和子命令帮助只并列列出 `status|verify|diag`，没有提示 `diag` 是安装后验证，
功能网络又把它放在诊断链路；用户仅排查故障时可能意外改变运行状态。

当前活动输入法为 Suzaku 时，既有预检仍会拒绝重装；本轮没有发现这项保护被绕过，
也没有在真实输入中验证草稿丢失。问题是诊断入口的未告知写入，不应夸大为必然打断输入。
建议 `diag` 只读；需要修复时由显式 `install` 或明确命名的修复动作承担。

## N13：部分外部桌面命令没有程序自身的超时

注册预检已使用带 2 秒时限的 [`linux_registration_output`](../src/bin/suzaku_tool.rs)，
服务写操作也有 10 秒时限。但以下共用路径仍直接调用阻塞的 `.output()`：

- `ibus_runtime_has_engine` / `ibus_dynamic_registry_has_engine`：IBus 列表、地址和 D-Bus 查询。
- `ibus_current_engine` / `restore_ibus_engine`：原输入法查询、切换与读回。
- `read_gnome_input_sources` / `reconcile_gnome_ibus_input_source`：GNOME 设置读写。
- `ibus_user_service_active` / `process_running`：状态命令与进程查询。

实测选了两种外部调用，合成子命令记录“已进入”后等待 30 秒；审计父进程在 4 秒限额
终止**仅它自己创建的进程组**，避免真的长时间卡住：

| CLI 动作 | 不响应的合成命令 | 4 秒结果 |
| --- | --- | --- |
| `status` | `ibus list-engine` | 仍在等待，由审计终止 |
| `verify` | `ibus list-engine` | 仍在等待，由审计终止 |
| `install` | `gsettings get ... sources` | 仍在等待，由审计终止 |
| `uninstall` | `gsettings get ... sources` | 仍在等待，由审计终止 |

四例均确认已执行到指定命令，并非卡在测试编译或没走到目标分支。
代码没有施加自身时限；外部命令若自行结束，CLI 仍会继续，因此不把本次“超过 4 秒”
描述为实测了永久死锁，也不声称本机 GNOME 当前有这个故障。
修复应统一有界调用、错误反馈及子进程清理；恢复输入法的多次重试还需整体截止时间，
避免仅为单次调用加时限后，累计等待反而过长。

## N14：工作区新审计链接没有对应的包内文档

[`package-linux.sh`](../scripts/package-linux.sh) 的固定文档清单包含功能网络及第一轮
审计，但未包含第二、三、四轮报告。当前包内 `functional-network.md` 已引用：

- `bug-audit-tools-2026-09-13.md`
- `bug-audit-settings-2026-09-13.md`
- `bug-audit-data-2026-09-13.md`

实际构建的 tar 和 deb 都缺少这些文件；两种格式各有三条已确认断链。
[`test-linux-package.sh`](../scripts/test-linux-package.sh) 仍返回成功，因为它只检查
列出的固定文件，不检查已打包文档的这组引用。

这是工作区准备下一次发布时的资料遗漏，不是二进制损坏，也不能据此宣称此前发布的
0.5.6 包已经包含这些新链接。建议补齐打包清单，并让验收覆盖包内应离线可读的文档引用；
不要求将每个源码引用所指的整个源码树也塞进二进制包。

## 首次审计已通过的检查

- 当前工作区四个程序的 release / all-features 构建完成，未复用旧二进制。
- 新 tar/deb 成对产物、外部和逐文件校验、清单、许可证、ELF、桌面入口、无安装钩子等
  既有包装检查通过；依赖记录为 Ubuntu 24.04 ABI，未宣称跨发行版静态兼容。
- Ubuntu 24.04 干净容器按 `--no-install-recommends` 安装：必要工具、字体、运行库检查通过。
- 合成旧包升级后可执行文件正确替换；未自动注册用户输入法，合成配置与备份字节保持不变。
- 已安装面板在私有 X11 / D-Bus 中启动并渲染中文界面。
- remove / purge 清理包拥有的入口，合成用户设置与备份保留。
- 现有注册 CLI **8 项**、包 CLI **3 项**通过：特殊路径转义、原输入法和禁用自启偏好保留、
  活动 Suzaku 拒绝重装/注销、masked 服务拒绝覆盖、无效参数与前置条件拒绝等未回退。

容器中的合成旧包不等于遍历所有历史版本；服务命令替身也不能证明真实 GNOME 登录/
注销和所有 systemd 故障行为。此次没有重跑前轮全部输入与模型质量测试，也没有触发远程 CI。

## 已实施的修复

- **N11**：首次注册的服务文件带一条“自启尚未完成”注释。reload / enable 失败时保留
  注释，重试继续 enable；enable 成功后原子清除注释并刷新服务文件视图。已有完整安装、
  没有这条标记的旧服务继续保留原自启偏好。部分 enable 生效后报错也可重试；enable 已
  完成而 restart 失败时，不把随后用户主动禁用的偏好覆盖回去。
- **N12**：`diag` 改为只读 `verify` 的别名，两种命令入口的帮助均明确说明。未注册时
  返回未就绪，不安装；已注册或当前已激活 Suzaku 时也不重写文件、添加输入源或重启服务。
- **N13**：Linux 注册相关桌面命令复用同一个有界子进程执行器，查询和 GNOME 设置操作
  2 秒，服务写操作保留 10 秒。查询、切换、读回和重试延迟共享恢复原输入法的 **2 秒总预算**。
  超时终止本次命令的进程组并回收直接子进程，输出上限 64 KiB；不杀真实 IBus 守护进程。
  查询错误向上返回，诊断不能伪报 PASS。预检读到的原输入法直接用于恢复，不再因第二次
  查询失败而悄悄丢掉恢复目标；空值和 dummy 状态在任何注册写入前拒绝。
- **N14**：打包清单补齐第二至第五轮报告。顶层 tar 安装说明和扁平的已安装使用说明
  同步调整相对链接，统一指向包内规范文档目录。包验收新增
  [审计链接检查器](../scripts/check-package-audit-links.py)，覆盖引用式、行内、多链接、
  目录前缀、锚点及空文档，避免只补固定文件列表后再次遗漏。它不是通用 Markdown 排版
  校验器；源码链接仍需要源码仓库，远程链接不要求离线可达。

N11 的持久标记保护**新版开始的首次注册和重试**。旧版已留下、但没有标记的服务文件与
用户主动关闭自启无法可靠区分，因此不擅自启用旧服务；这类历史状态需用户显式选择。
注册仍不是跨文件和 systemd 的断电原子事务；服务或原输入法恢复失败时会保留错误供重试，
不能把命令超时理解为所有先前步骤已自动撤销。

## 当前回归与重跑

[注册回归](../tests/linux_registration_cli.rs) 和 [包 CLI 回归](../tests/linux_package_cli.rs)
已转为普通测试，不再需要环境开关或 `--ignored`。慢命令回归有独立的 4 秒外层时限，
验证指定合成命令被执行、CLI 报错、超时子进程已回收；所有 HOME、桌面命令和用户数据仍是
临时合成夹具。包链接的缺失、空文件、扁平目录错误均有负向测试。

```bash
cargo test --locked --all-features --test linux_registration_cli --test linux_package_cli -- --test-threads=1
suzaku_audit_output=$(mktemp -d "$PWD/target/packaging-chain-audit.XXXXXX")
bash scripts/package-linux.sh --output "$suzaku_audit_output/packages"
bash scripts/test-linux-package.sh "$suzaku_audit_output/packages"
# 仅在一次性 Ubuntu 容器内安装、升级、启动并卸载：
bash scripts/test-linux-install.sh "$suzaku_audit_output/packages"
```

普通测试和两种真实包的链接检查均由现有 [Linux CI](../.github/workflows/ci.yml) 必跑步骤
覆盖。包构建后若再修改报告，不会反向更改已有包；下一次构建使用新目录。本轮不触发远程
CI，不对本机安装执行注册命令。

当前工作区验证：全功能常规测试 **813 通过、0 失败、18 忽略**（其中注册 16 项、包 CLI
4 项均实际执行）；两种新包分别通过 15 / 13 处审计链接检查，修复前的旧测试包被新门禁
正确拒绝。全功能和 GPU 功能配置的全目标严格 Clippy、ShellCheck、格式及差异空白检查通过。
这里的 18 项忽略不计作已经执行，真实模型质量也没有在本轮评估。

修复后的新包在干净 Ubuntu 24.04 容器中再次通过无 Recommends 安装、依赖/字体检查、
合成旧包升级、中文面板启动及 remove/purge；合成用户设置和备份字节保留。私有 IBus
专项也通过，包括激活/释放、中英日连续输入、词句候选、异常配置和模型回退链路。
没有重新执行其余全部独立 UI/设置/数据专项；这些代码在本轮未改变，前轮记录继续保留。
