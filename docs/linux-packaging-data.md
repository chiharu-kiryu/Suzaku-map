# Linux 打包与数据管理

这一版以 Linux / IBus 为核心。macOS 和 Windows 的原有构建入口保留；
`.deb` 只针对构建它的 Debian/Ubuntu 系列及相容 ABI，压缩包也不是静态万能包。
Fcitx 目前仍不是完整的原生输入法后端。
当前验证基线是 Ubuntu 24.04 / amd64 / IBus，面板走 X11 或 GNOME 下的 XWayland。
其他发行版、ARM64 和完整原生 Wayland 会话尚不能视为通过同一安装验收。

## 从 0.8.0 开始的发布要求

从 **0.8.0（含）**起，每次版本发布都要在对应 GitHub Release 附上通过验收的 Linux
`.deb`、二进制 `.tar.gz` 和各自的 SHA-256 校验文件，同时提供安装/升级说明及已知限制。
仅推送源码标签或上传会过期的 CI artifact 不算完成发布。
先覆盖 Ubuntu 24.04 / amd64；其他平台和架构须单独验收，不因同步版本号而自动列为支持。
产物必须来自对应标签的干净源码，且同一提交的 CI、包内容校验、干净容器安装验收通过后
才能发布。这是后续发布门槛，不表示已实现自动上传，也不追溯宣称 0.7.x 已有 Release 附件。

## 构建分发包

```bash
bash scripts/package-linux.sh
# 只生成压缩包；不要求 dpkg-dev
bash scripts/package-linux.sh --format tar --output dist-tar
```

脚本构建 `panel`、`linux_ime_host`、`linux_ime_probe`、`suzaku_tool`，然后生成
`dist/suzaku-VERSION-TARGET.tar.gz`、`dist/suzaku_VERSION_ARCH.deb` 及各自 `.sha256`。
它不安装程序、不接入用户桌面，也不读取或打包个人配置、模型、缓存和输入数据。
已有同名产物不会被覆盖；重建时使用新的 `--output` 目录。
`--skip-build` 供 CI 在完成四个发布程序的构建后复用，不能用来打包过期二进制。

需要现有 Linux 构建依赖，以及 `jq`、`tar`、`gzip`、`binutils`；生成 `.deb` 另需
`dpkg-dev`。`.deb` 的链接库依赖由 `dpkg-shlibdeps` 从 ELF 推导，另外声明通过
运行时加载的图形库和 IBus 工具。CI 在 Ubuntu 24.04 上构建；在更新的系统本机构建
可能要求更新的 glibc。请检查 `manifest.json` 的 `build_libc` 和 `.deb` 的 `Depends`。
输入法必需的 CJK/拉丁字体、Fontconfig、systemd 用户服务工具以及 X11 键盘运行库均为
明确依赖；使用 `--no-install-recommends` 也不应出现缺字或缺失键盘库。

压缩包内还有逐文件 `SHA256SUMS`、版本/架构/源码提交/工作区是否有改动的清单、
项目许可证、锁定依赖的许可证声明和其源码目录提供的 LICENSE/COPYING/NOTICE 文件。
校验和用于完整性检查，不是签名；不要把来源不明的包当成可信发布。
`SOURCE_DATE_EPOCH` 控制归档时间；这里不宣称跨工具链的二进制可重现。

## 安装和启动

Debian/Ubuntu，先在包所在目录检查校验和，再明确安装：

```bash
sha256sum -c suzaku_VERSION_ARCH.deb.sha256
sudo apt install ./suzaku_VERSION_ARCH.deb
/usr/bin/suzaku-panel
# 需要正式作为输入法使用时，在当前桌面用户会话里执行，不要 sudo：
/usr/bin/suzaku-tool linux-register install
/usr/bin/suzaku-tool linux-register verify
```

系统应用菜单会出现 Suzaku。安装 `.deb` 本身不注册输入源、不启动用户服务、不启用
开机自启，也不切换当前输入法。注册命令会安装用户服务、添加 Suzaku 输入源，并保留
原活动输入法。打包版本的用户服务直接引用 `/usr/lib/suzaku/linux_ime_host`，不复制
一个会在升级后过期的宿主。升级前从托盘完整退出 Suzaku，更新后重新打开面板即可；
有输入正在进行时请先完成或取消组合。

注册与注销必须显式指定动作；裸 `linux-register` 只显示帮助，多余参数会拒绝执行，
没有 `--dry-run` 之类的隐式选项。root/sudo、无效 HOME、缺失宿主、不可用的用户服务
管理器会被拒绝；当前正在使用 Suzaku 时也拒绝重装/注销，请先释放回原输入法。
初次注册启用宿主自启，重新注册保留已有的启用/禁用偏好。宿主文件采用独占临时文件和
原子替换，含空格、中文、引号、`$`、`%` 的用户路径经过专门的启动参数转义。
服务通过固定的 `/usr/bin/env --` 直接执行绝对路径，不经过 shell；这是为了兼容
[systemd 对可执行路径中引号和反斜杠的限制](https://github.com/systemd/systemd/blob/v255/src/core/load-fragment.c)，
并与 IBus 实际使用的 GLib 参数解析器分别验证。
已屏蔽或由符号链接提供的用户服务不会被重装命令擅自覆盖、解除屏蔽。

0.5.7 将首次注册尚未完成的自启步骤记录在服务文件注释里；reload/enable 失败后，
再次 `install` 会继续完成，而不会把残留文件误判为用户关闭自启。已经完成注册或没有该
标记的旧服务仍保留已有偏好，不自动猜测旧版半安装状态。`status`、`verify`、`diag` 均只读，
其中 `diag` 是 `verify` 的别名，不安装或重启服务。桌面查询与 GNOME 设置命令有 2 秒时限，
服务操作有 10 秒时限，恢复原输入法的查询/切换/读回共用 2 秒总预算；出错明确返回失败。
详情见 [注册与打包修复记录](bug-audit-packaging-2026-09-13.md)。

系统菜单启动器直接指向包内程序，不经过 PATH 中可能残留的旧用户安装。
如果曾手动安装到 `~/.local/bin` 或创建了用户级桌面快捷方式，请检查这些入口：
包管理器不会擅自删除它们；终端可用上面的 `/usr/bin/...` 明确运行 `.deb` 版本。

### 面板与输入法服务的生命周期

完成一次宿主注册后，无需每次手动执行 `systemctl`：

- 打开面板：在后台启动已注册的 `suzaku-ibus.service`，等待实际连接就绪，不自动切走原输入法。
- 托盘「激活 Suzaku 输入法」：服务停止时先启动，再切换；已经运行时复用，不重复拉起宿主。
- 「释放并恢复」：恢复原输入法，面板和服务仍保留，便于再次激活。
- 完整「退出 Suzaku」或退出快捷键：先确认原输入法恢复，再停止宿主服务，最后退出面板。
- 关闭窗口到托盘、隐藏面板或收缩为悬浮球：不停止输入法服务。

启动、就绪等待和退出清理都有时间上限，不在 UI 线程等待服务。没有托盘扩展时仍运行
生命周期控制器；关闭唯一的窗口会走完整退出。若恢复或停止失败，正常退出会保留界面
并显示错误供重试，不强行中断输入。系统菜单激活的 Suzaku 会回退到本次运行中实际观察
到的原输入法；如果从未观察到安全回退目标，会提示先从系统菜单切换，不猜测用户的默认项。

只管理当前桌面用户已注册的 Suzaku 服务，不重启 IBus 守护进程，不更改键盘锁定状态、
布局或既有自启设置，也不会自动安装/注册缺失的服务。自定义宿主 socket、私有 D-Bus
测试会话以及独立启动的非服务宿主仍归其启动器管理，不随面板被误停。
正常退出流程不等同于强杀进程；崩溃或强杀后的服务状态仍需由系统服务管理器检查。

0.6.5 为宿主服务关闭时间窗口内的启动次数配额，并保留 **2 秒重试间隔**，避免
IBus 暂时不可用时耗尽系统默认配额、恢复后服务却永久停在失败状态。显式停止仍取消
自动重试，不重新启用已关闭的自启偏好；该服务策略本身不会切回输入法或恢复丢失的草稿。
这会持续重试永久配置错误，因此仍需依据日志处理；它不是无限快速重启。
修改只在新版工具执行显式 `linux-register install` 时写入用户服务文件，构建或
替换二进制不会自动改动既有服务。重新注册前先完成输入并释放/退出 Suzaku。
隔离审计及随后明确授权的本机安装补验见 [N43 服务恢复审计](bug-audit-service-recovery-2026-09-24.md)。
首次本机补验确认服务可自动恢复，但 GNOME 的活动输入源可能仍为空，需要重新选择；
不要将宿主进程就绪视为原输入源和草稿都已恢复。
0.6.5 的 N44 修复在受管面板中观察同一 IBus 上的宿主替换：确认新宿主就绪且
全局输入源确实为空时，有限恢复最近观察到的输入源；有效的用户/桌面选择优先。
IBus 整体断开会清空恢复历史，主动退出会停止观察器，不重放草稿。N45 同时修正密码
探针的用途清理和恢复读回。这两项已补隔离回归，并按用户要求更新了本机用户安装：
GNOME 46/X11 的 TERM/KILL、活动草稿重启、两次密码探针及托盘退出/重开通过；配置
和原输入源保留。本机实测使用升版前含同样修复的 0.6.4 构建；准备 0.6.5 源码版本
不再改动安装，也未生成新分发包，不代表任意安装会自行升级。详见
[输入源与密码探针恢复审计](bug-audit-engine-recovery-2026-09-24.md)。

压缩包解压后可直接运行，无需 Cargo：

```bash
sha256sum -c suzaku-VERSION-TARGET.tar.gz.sha256
tar -xzf suzaku-VERSION-TARGET.tar.gz
cd suzaku-VERSION-TARGET
sha256sum --quiet -c SHA256SUMS
./bin/panel
# 显式注册宿主时使用以下命令，它会复制到用户 libexec：
./bin/suzaku_tool linux-register install
```

压缩包不会自动安装菜单项或设置 PATH。桌面启动器由 `.deb` 提供。
模型服务需另行配置，默认发现本机 LLaMA，也支持其他本地模型和显式授权的 HTTPS 云端服务。
包不附带模型，也不会自动下载权重。详见[模型服务配置](model-providers.md)。

## 数据位置和托盘入口

沿用既有目录，不强行迁移老用户配置：

| 内容 | Linux 默认位置 |
| --- | --- |
| 输入法语言、LLM 配置 | `~/.config/suzaku-ime/settings.json` |
| 面板显示、操作配置 | `~/.config/suzaku-panel/panel-settings.toml` |
| 配置备份 | `~/.local/share/suzaku/backups/` |
| 维护锁 | 输入法设置文件同目录的 `.suzaku-data.lock` |
| 输入、候选、语音/手写草稿 | 仅内存，不进入备份 |
| 内置词库 | 编译在程序内，始终保留兜底 |
| 可选离线包 | `~/.local/share/suzaku/lexicons/registry.json`；不进入配置备份 |
| 模型权重 | 由外部 Llama/Ollama 服务管理，不由 Suzaku 搬移或清理 |

遵循 [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir/latest/)：
支持绝对的 `XDG_CONFIG_HOME`、`XDG_DATA_HOME`；空值和相对值退回默认位置。
`SUZAKU_IME_CONFIG` 仍可单独指定输入法设置文件。

右键托盘 → **数据管理**：立即备份配置、打开备份目录、打开两个配置目录。
操作在后台运行，结果在子菜单里显示。打开目录使用桌面的
[FileManager1 服务](https://wiki.freedesktop.org/www/Specifications/file-manager-interface/)；
服务不可用会显示错误和路径，不会杀死或重启文件管理器。
新建配置目录/备份目录使用 0700，配置和备份使用 0600；现有目录权限不会被修改。
备份没有加密，可能包含模型服务地址和偏好设置，请自行保管。

命令行也可以管理：

```bash
suzaku-tool data status
suzaku-tool data backup
suzaku-tool data backup ./my-settings.json
suzaku-tool data validate ./my-settings.json
suzaku-tool data open backups
```

压缩包用户把 `suzaku-tool` 替换为 `./bin/suzaku_tool`。
备份只接受已知设置字段，不扫描目录或收集任意文件。原始输入法 JSON 中的未知字段
不会被导出；未知面板字段会使校验失败，避免把任意文本当配置备份。
配置上限 64 KiB、备份上限 256 KiB；备份/恢复操作拒绝特殊文件和最终路径为符号链接的备份/配置。
没有配置文件时仍可备份，其中 `null` 表示该项使用默认设置。

Linux 宿主和模型命令的运行时配置读取仍兼容指向普通 JSON 文件的符号链接，但会以
非阻塞方式打开，并按打开后的文件类型拒绝 FIFO、目录、套接字等特殊文件。
重载失败时保留内存中的配置和当前草稿，键盘输入仍可使用；不会自动重试或覆盖异常文件。

工作区修复版在进程生命周期内同时保留配置文件链接各层与最终目标目录的维护锁，
保存替换链接后也不提前释放；配置写锁同样覆盖这些别名，见 [N10 修复记录](bug-audit-data-2026-09-13.md)。
循环链接、目标父目录无法解析或无法建立锁时会拒绝启动/写入，请使用可保护的用户配置路径。
外部手工更改链接或父目录指向时，先停止 Suzaku，改完后重启，不以此进行运行期热切换。
备份和恢复仍使用真实路径，不跟随最终组件的软链接。

## 恢复：先预览，再应用

```bash
suzaku-tool data restore ./my-settings.json
# 以上只预览，没有写入；检查两项变化后：
# 1. 从托盘完整退出 Suzaku（恢复原输入法并停止受管宿主）
#    若另行手动启动过宿主，也需先通过对应启动器将其停止。
# 2. 明确应用：
suzaku-tool data restore ./my-settings.json --apply
# 3. 重新打开面板，服务会同步启动：
suzaku-panel
```

应用恢复目前仅支持 Linux。新版面板/宿主持有共享运行锁，恢复需要独占锁，
因此运行中的进程或并发启动不会覆盖恢复结果；旧版遗留运行时端点存在时也会拒绝恢复。
程序不会自动停止服务、删除运行时端点、夺走活动输入法或强制退出桌面程序。
若异常退出留下旧端点，请先确认旧进程已退出，正常重启再退出，或重新登录桌面会话。

恢复前先完整校验，预写新旧配置临时文件，创建 `before-restore-*.json`，再逐文件原子替换。
发生普通 I/O 错误会尝试回退，并报告恢复前备份的位置。两个配置文件不构成跨文件的
断电原子事务；崩溃后可对恢复前备份重新执行预览/恢复。
备份中 `null` 对应的项会删除那个设置文件并恢复默认值，预览会明确显示，其他文件不受影响。
原配置损坏、不可读取时会安全拒绝应用，避免丢掉唯一副本。
模型配置只备份密钥环境变量名、不备份密钥值；恢复会关闭云端发送授权，需重新明确授权。

## 离线词库包与制包 SDK

0.8.0 新增数据型插件包，不执行第三方脚本或加载动态库，不访问模型或自动下载。
第一版提供命令行管理，**尚无设置面板中的导入/启停列表，也不是热更新**。
推荐集合随程序提供：英文、中文的 `outdoors` / `astronomy` 观星集合，以及日文
`travel` / `rail` 铁路出行集合；它们是可选小型示例，不是完整专业词典。
按“一个语言 + 一组相关主题”制包，避免把无关领域塞进同一包；相同主题可发布不同语言包。

`.deb` 使用 `suzaku-tool`，源码/压缩包使用 `suzaku_tool` / `./bin/suzaku_tool`：

```bash
suzaku-tool pack catalog --language en --topic outdoors
suzaku-tool pack export org.suzaku.en.outdoors ./en-outdoors.json
suzaku-tool pack validate ./en-outdoors.json
suzaku-tool pack install ./en-outdoors.json
suzaku-tool pack list
suzaku-tool pack preview en "please bring your bino"
suzaku-tool pack disable org.suzaku.en.outdoors
suzaku-tool pack enable org.suzaku.en.outdoors
# 显式更新；保留包原有位置和启用状态，不允许更换语言：
suzaku-tool pack install ./en-outdoors.json --replace
suzaku-tool pack remove org.suzaku.en.outdoors
```

`export` 不覆盖已有文件，导出不等于安装；安装新包默认启用，ID 重复需要 `--replace`。
包版本采用三段非负数字，版本替换由使用者明确选择，不自动升级、降级或联网检查。
`list` 和 `preview` 显示**新进程将使用的**磁盘状态，不表示个人桌面已经加载新包。
包变更保存后，需要正常退出并重新启动面板及宿主。CLI 不擅自停止输入服务；只重启面板
不会让旧宿主的词表同步更新。安装、禁用和移除不改当前进程，`remove` 的包可从原文件重装。

注册表遵循 `XDG_DATA_HOME`；可用绝对路径 `SUZAKU_LEXICON_DIR` 覆盖，空/相对值报错。
面板、宿主和管理命令必须使用相同目录；自定义 systemd 环境需由使用者自行配置。
每次修改使用独立写锁和单文件原子替换；失败不覆盖已有注册表，不承诺跨文件或断电事务。
损坏记录在启动时隔离并写日志，其他独立包仍加载；整体 JSON/格式损坏则保留内置兜底。
管理写入遇损坏注册表拒绝修改，应先保存副本再人工修复，不会自动删除唯一的坏数据副本。
个人包及其注册表不进 `data backup`；保留原始分发 JSON 以便恢复，也不会随卸载被清除。

### 包格式与边界

一个 UTF-8 JSON 文件，顶层为 `format_version: 1`、`manifest`、`lexicon`。
`manifest` 必填 `id/version/name/description/language/topics/license/authors`。
ID 为最长 64 字节的小写字母开头 slug，只含小写字母、数字、点、连字符，不是文件路径。
语言与内部 `lexicon.language` 必须相同，当前只支持 `en`、`zh-Hans`、`ja`；增加语言数据
不会自动增加转换算法。主题是 1–8 个短 slug，作者 1–8 个，必须说明来源和许可。
署名和许可字段只是作者声明，**格式校验不等于签名验证、来源可信或许可审计**。

`lexicon` 复用内置资源 v1：

- 英文：`word_layers`，层内为 `id`、`words`、`next_words: [[语境,[下一词]]]`、
  `sentences`；可用 `phrase_endings: [[语境,[句尾]]]`。单词显式列出字母/撇号词形。
- 中文：`readings: [{reading,text,kind,require_separators}]`，读音用小写无调拼音，
  音节以 ASCII 撇号分隔，`v` 表示 ü；`kind` 为 `word` 或 `sentence`，分隔标记默认 false。
- 日文：同一 `readings` 结构，读音为平假名，不支持中文的强制分隔标记。
- 中日续句：`continuations: [[语境,[完整句子]]]`，每句必须以该语境开头。

只追加扩展层/读音，旧词库不被替换。已有语境如果值完全相同则跳过，不同则拒绝并提示
使用更具体的语境；同读音/文本的类型或分隔标记不能冲突。词序、同音顺序由数组顺序决定。
未知字段、版本及当前解码器不支持的字段拒绝；不静默忽略可能无效的数据。
单包上限 **256 KiB、8 层、2,048 索引单元**，每语言启用扩展上限 **4,096 单元**，
最多安装 **16 包**，注册表最多 **4 MiB**，合并后每语言最多 **64 层**。
单元按词、读音、语境/续句及英文句子的词投影保守计数，不是唯一词数；`validate` 显示单元。
只有安装/启用时才检查和其他活动包的组合冲突；候选窗口、混排数量和模型预算不变。

### 小型 SDK

Python 3.10+ 标准库即可；权威校验复用当前 Suzaku 的 `pack validate`，不另造一份易漂移的
Python 词典算法。源码位置 `sdk/offline-packs/`；打包脚本把 SDK 放在
`share/suzaku/offline-pack-sdk/`，三个示例 JSON 放在 `share/suzaku/offline-packs/`。
`.deb` 安装后相应位置为 `/usr/share/suzaku/…`，无网络依赖或额外 pip 依赖。

```bash
# 源码：制出园艺示例包；不会安装，也不允许覆盖已存在的输出。
python3 sdk/offline-packs/example.py ./garden.json ./target/debug/suzaku_tool
# 或校验、规范化自己编写的 JSON：
python3 sdk/offline-packs/suzaku_pack.py ./authored.json ./distribution.json --tool ./target/debug/suzaku_tool
# 安装版示例：
python3 /usr/share/suzaku/offline-pack-sdk/example.py ./garden.json /usr/bin/suzaku-tool
```

Python API 提供 `PackBuilder(...)`、`words(...)`、`reading(...)`、`continuation(...)`、
`phrase_ending(...)`、`build(path, tool=...)`，见随附 `example.py`。
输出先在同目录私有临时文件校验，再原子无覆盖发布；校验/工具失败不发布半成品。
分发的 JSON 只有数据，消费者无需 Python SDK；不要在包中附带个人输入记录或复制无权分发的语料。

### 开发阶段验证范围（2026-10-02）

源码全量回归 992 项通过、27 项 opt-in 跳过；其后新增三个负向边界用例，离线包专项
合计 15 项通过，严格全目标 Clippy 无警告。包专项原生 IBus 在默认/行内两种模式通过：
中英日词句、标签、数字采用、空格不断流、精确提交，以及启动后损坏注册表仍使用冻结快照。
未安装包的完整默认 IBus 流程也通过。首次包测试错误地假设日文转换候选保留读音分隔空格，
诊断确认其既有转换会合并分隔符；改为显式采用原文字面候选后验证精确空白提交，并另测
三语句子提交，没有放宽超时、断言或改动日文转换规则。

日志保留在本轮任务目录 `/tmp/szpacks.VFlaRa/`，包括原始失败和诊断。
本机是 Ubuntu 26.04，发布基线仍为 Ubuntu 24.04 amd64；打包脚本和 smoke 检查已纳入
SDK/示例，ShellCheck、语法、图源和文档审计链接检查通过。该开发阶段**没有重建 release 包、
运行容器安装、更新个人安装或验证 GUI 包管理**，不能据此宣称这些路径已经验收。
后续 0.8.0 发版必须另通过同提交的 Ubuntu 24.04 CI、包校验和容器安装门禁；
见 [0.8.0 发布说明](releases/0.8.0.md)。源码升版仍不会改动个人安装。

## 卸载与数据保留

需要完整移除输入法时，先退出面板，再在各自桌面用户会话中取消注册：

```bash
suzaku-tool linux-register uninstall
sudo apt remove suzaku
```

宿主注销不删除配置或备份；`.deb` 不拥有任何用户数据目录，因此 `remove`/`purge` 也
不会清理它们。多用户机器每个注册过的用户都需注销自己的服务。
压缩包注销时用 `./bin/suzaku_tool`；完成后再移走解压目录。
这一轮不提供一键清空个人数据、缓存清理、词库编辑或模型删除功能。

## 验证

```bash
cargo test --locked --all-features -- --test-threads=1
bash scripts/package-linux.sh --output dist-check
bash scripts/test-linux-package.sh dist-check
# 需要 Docker；联网仅用于测试容器安装 Ubuntu 软件源依赖：
bash scripts/test-linux-install.sh dist-check
timeout --kill-after=3s 25s dbus-run-session -- env SUZAKU_DATA_QA=1 /usr/bin/python3 scripts/test-data-folders.py
```

包装检查逐个要求包与校验文件配对，随后检查解压内容；不能只凭一个孤立校验文件通过。
打包包含功能网络引用的审计报告，验收同时检查这些报告，以及安装、模型、输入、翻译和
界面语言说明的所有扁平副本中的审计链接；这些副本的同级文档链接统一指向包内 `docs/`。
安装验收在全新的 Ubuntu 24.04 容器中进行：无 Recommends 安装、运行库/字体检查、
从合成旧包升级、启动已安装的面板、remove/purge，并逐字节检查用户设置和备份保留。
合成旧包用于验证包管理器的升级行为，不等于覆盖所有历史版本的数据迁移。
容器不挂载 HOME、会话总线、GPU 设备或 Docker socket；X11 显示与 D-Bus 会话都是私有的。
另有 CLI 隔离测试覆盖用户注册、重复安装、原输入源和自启保留、特殊路径与拒绝危险操作。
这些检查不在本机安装包，不连接真实桌面，也不强行重启用户服务；完整 GNOME 用户会话的
开机/注销行为仍需桌面验收，不能由容器中的服务命令替身证明。

默认测试网络为 Docker bridge。如本机代理只监听 localhost，可明确设置
`SUZAKU_PACKAGE_QA_NETWORK=host`，同时传入 `http_proxy` / `https_proxy`；只改变测试容器
网络，不修改宿主网络设置。`SUZAKU_PACKAGE_QA_CACHE` 可指定一个已有的 `.deb` 下载缓存目录，
以只读方式预填容器缓存；APT 仍会校验包。单轮安装验收上限 15 分钟，结束删除测试容器。
桌面文件按 [Desktop Entry Specification](https://specifications.freedesktop.org/desktop-entry/latest/)
验证；Debian 依赖遵循 [dpkg-shlibdeps](https://manpages.debian.org/bookworm/dpkg-dev/dpkg-shlibdeps.1.en.html)。
