# Linux 打包与数据管理

这一版以 Linux / IBus 为核心。macOS 和 Windows 的原有构建入口保留；
`.deb` 只针对构建它的 Debian/Ubuntu 系列及相容 ABI，压缩包也不是静态万能包。
Fcitx 目前仍不是完整的原生输入法后端。

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

压缩包内还有逐文件 `SHA256SUMS`、版本/架构/源码提交/工作区是否有改动的清单、
项目许可证、锁定依赖的许可证声明和其源码目录提供的 LICENSE/COPYING/NOTICE 文件。
校验和用于完整性检查，不是签名；不要把来源不明的包当成可信发布。
`SOURCE_DATE_EPOCH` 控制归档时间；这里不宣称跨工具链的二进制可重现。

## 安装和启动

Debian/Ubuntu，先在包所在目录检查校验和，再明确安装：

```bash
sha256sum -c suzaku_VERSION_ARCH.deb.sha256
sudo apt install ./suzaku_VERSION_ARCH.deb
suzaku-panel
# 需要正式作为输入法使用时，在当前桌面用户会话里执行，不要 sudo：
suzaku-tool linux-register install
suzaku-tool linux-register verify
```

系统应用菜单会出现 Suzaku。安装 `.deb` 本身不注册输入源、不启动用户服务、不启用
开机自启，也不切换当前输入法。注册命令会安装用户服务、添加 Suzaku 输入源，并保留
原活动输入法。打包版本的用户服务直接引用 `/usr/lib/suzaku/linux_ime_host`，不复制
一个会在升级后过期的宿主。升级前从托盘完整退出 Suzaku，更新后重新打开面板即可；
有输入正在进行时请先完成或取消组合。

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
| 内置词库 | 编译在程序内，暂无个人词库文件 |
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
timeout --kill-after=3s 25s dbus-run-session -- env SUZAKU_DATA_QA=1 /usr/bin/python3 scripts/test-data-folders.py
```

包装测试仅解压到临时目录并运行无副作用命令，不在本机安装包或连接真实桌面。
桌面文件按 [Desktop Entry Specification](https://specifications.freedesktop.org/desktop-entry/latest/)
验证；Debian 依赖遵循 [dpkg-shlibdeps](https://manpages.debian.org/bookworm/dpkg-dev/dpkg-shlibdeps.1.en.html)。
