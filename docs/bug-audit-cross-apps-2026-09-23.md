# 浏览器与 Qt 英文输入审计 · 第二十四轮（2026-09-23）

后续状态：本页保留最初失败与复验记录。两项问题已在
[第二十五轮](bug-audit-cross-app-fixes-2026-09-23.md) 的默认候选区草稿/空草稿数字直通
模式中修复，仍未发布；下文“未解决”和限制放行选项描述的是当时版本。

沿 [功能链路网络](functional-network.md) 的 F08/F10–F14/F29，将上一轮 GTK
检查扩展到 Chrome 和 Qt 5/6。基线仍为 `0.6.1`、提交
`b0300ee94a94e43d78632ef081adfd54b980eefa`，使用包含未发布 N38 修复的工作区程序。
**本轮新增可重复的应用测试，确认两类尚未解决的兼容性限制，没有新增产品修复。**
不能把正常用例通过等同于这些应用已经全面兼容。

## 环境与观测方式

- Chrome `153.0.8010.52`，X11，独立临时配置、本地合成网页；不打开个人浏览器资料。
- Qt `5.15.13` / PyQt `5.15.10`，Qt `6.4.2` / PyQt `6.6.1`；标准
  `QPlainTextEdit`、`QTextEdit`、`QLineEdit`，不是 Kdenlive 等完整应用的验收。
- IBus `1.5.29-2`；Qt 分别执行默认异步按键模式和显式同步模式；浏览器固定同步模式。
- 新建 Xvfb、私有会话总线/IBus、独立配置，模型联想关闭；不修改桌面输入法或系统安装。
  本机缺少的 PyQt 绑定只从发行版包解压到 `target/`，不作为 Suzaku 运行时依赖。

[运行入口](../scripts/test-linux-apps.sh) 共用上一轮的真实 XTest 按键与宿主快照；
[跨应用驱动](../scripts/test-linux-cross-apps.py) 启动
[网页](../scripts/fixtures/input-fields.html) 或
[Qt 窗口](../scripts/fixtures/qt-input-fields.py)。夹具仅用应用接口设焦点/选区，
正文必须经过真实键盘 → 应用输入模块 → Suzaku → 应用提交；不直接设置正文来假装输入。
应用通过带随机令牌的本地回环服务回传正文、预编辑与事件；服务不提供其他文件。

引擎 `CommitText` 旁观者只订阅本轮私有 IBus：先用正常 Enter 校验监听有效，
再检查切框阶段没有引擎提交，避免把客户端自行确认误归为 Suzaku 提交。
浏览器读取 `_NET_WM_NAME`，不依赖旧式窗口名；富文本框清空后浏览器自留的
单个 `<br>` 光标占位按空框处理，不对正文做 trim。

## 正常路径：每种运行配置 17 组

三种普通文本框分别检查四组，共 12 组：

1. 空格保留连续草稿、两次提交的字面正文准确。
2. 数字采用 `hello`、退格撤回 `hel`、再次采用并续写提交。
3. 替换真实选区和在正文光标中间插入，不重复或覆盖其他正文。
4. Escape 取消、单字删至空，应用预编辑同步清空。

另外 5 组：新框不继承旧草稿；切框前 Escape 可避免隐式确认；标准密码框的
合成实体按键不进入候选/伴随快照；Alt+数字后 Enter 可保全字面数值；返回普通
文本框后候选和提交恢复。**字面数字替代路径不代表数字框自动直通或隐私保护通过。**

Chrome 1 种、Qt 两版各 2 种按键模式，共 5 种配置；每种都另外记录下面两项限制。
密码测试只说明这些标准字段的实体按键路径，不覆盖自定义遮罩框、PRIVATE 普通框、
伴随面板注入或所有密码管理器。

## 未解决 A：客户端切框时自行确认草稿

在原框键入 `discardme`，不按 Enter，切到另一框：三类客户端都将该文本确认在
**原框**。Suzaku 草稿已清空、新框没有旧草稿，私有引擎也没有发出 `CommitText`。
这是客户端缓存预编辑的确认行为，不能宣称“只有 Enter/点击候选才会成为应用正文”。

Qt 的对应版本实现会先向当前控件发送缓存预编辑的提交事件，再调用 Reset；见
[Qt 5.15.13](https://github.com/qt/qtbase/blob/v5.15.13-lts-lgpl/src/plugins/platforminputcontexts/ibus/qibusplatforminputcontext.cpp)
和 [Qt 6.4.2](https://github.com/qt/qtbase/blob/v6.4.2/src/plugins/platforminputcontexts/ibus/qibusplatforminputcontext.cpp)
的 `QIBusPlatformInputContext::commit()`。Chrome 的结论以本机 DOM 事件和引擎
提交旁观者实测为准，不将其他 Chromium 版本的实现直接当成本版本证据。

临时方式：**不要保留这段文字时，先按 Escape，再切框。** 本轮在五种配置中
验证此路径，不通过回删应用正文“修正”客户端已确认的文字。

## 未解决 B：数字用途没有传到 IBus

Qt 使用声明 `ImhFormattedNumbersOnly` 的 QLineEdit；网页使用
`<input type="number" step="any">`。直接输入 `12.5` 时，两者仍进入普通候选
路径：`2`、`5` 被当作不存在的候选槽位，草稿只剩 `1.`。Enter 后 Qt 正文为
`1.`，浏览器的数值字段为 `1`。伴随快照含该普通草稿，**数字用途的直通/隐藏不成立**。

Qt 对应版本的 IBus 插件没有转发这项用途提示；Chrome 对应版本的
[GTK 输入桥](https://github.com/chromium/chromium/blob/153.0.8010.52/ui/gtk/input_method_context_impl_gtk.cc)
也没有把 number 类型映射到 GTK input-purpose。该桥对密码使用简单输入上下文；
Qt 密码框则拒绝该 IBus 模块，因而密码按键通过不代表数字用途也已传达。
这些源码与实测一致，不推广为所有版本、后端和应用的结论。

临时方式：**Alt+1、Alt+2、`.`、Alt+5，再 Enter**，五种配置均保留 `12.5`。
此方法仍会形成公开草稿，敏感数值应先通过托盘释放 Suzaku，切到可信输入法。
没有按进程名/屏幕猜测输入用途，也没有悄悄更改普通文本框“数字优先选候选”的规则。

## 复验与验收语义

```bash
bash scripts/test-linux-apps.sh gtk
bash scripts/test-linux-apps.sh browser
bash scripts/test-linux-apps.sh qt5
bash scripts/test-linux-apps.sh qt6
bash scripts/test-linux-apps.sh cross
```

浏览器默认 `google-chrome`，可用 `SUZAKU_APP_QA_BROWSER` 指定兼容的 Chromium
可执行程序，但其他程序需要独立验证。Qt 需要发行版 `python3-pyqt5`、
`python3-pyqt6` 和对应 XCB/IBus 插件；`SUZAKU_QT_QA_SITE` 可指向只解压的绑定目录。
默认构建调试程序；`SUZAKU_APP_QA_BIN_DIR` 可指定已构建目录。

**默认只要上述限制仍出现，就返回失败，不给出完整兼容性绿灯。** 若仅回归正常
流程并复现已记录限制，可显式设置 `SUZAKU_APP_QA_ALLOW_KNOWN_LIMITS=1`；此时仍
逐项打印 `KNOWN LIMITATION (not acceptance)`。其他未知错误始终失败。
Linux CI 新增 Qt 两版的这种回归模式，明确不作为完整兼容性验收；不强制安装浏览器。
默认清理本轮合成数据；`SUZAKU_APP_QA_KEEP=1` 保留独立显示的失败截图和日志。

尚不覆盖 Firefox/Electron、原生 Wayland、GNOME 窗口管理器、网页框架编辑器、
Qt 完整业务应用、真实候选弹窗鼠标点击和长时间压力。没有提交、推送或更新个人安装。

## 本轮最终结果

- 发布程序的完整 `cross` 回归模式：**85 组正常工作流通过**，两项限制各在五种
  配置中复现，共 10 次限制观测；不是 95 组兼容性验收通过。
- 默认构建入口、调试程序的 Qt 6 同步/异步回归：34 组通过，4 次限制观测。
- 调试程序的浏览器默认验收：17 组正常路径通过后，因两项限制按预期返回失败，
  验证未显式允许限制时不会给出绿灯。
- 共用按键/窗口辅助代码重构后，GTK 实际应用 8 组重新通过。
- ShellCheck、Shell/Python/YAML 语法、差异空白检查，以及 75 条源码树审计链接通过。
- Qt 回归步骤已写入 CI，但未推送，不表示远端 CI 已执行。打包清单已包含本报告；
  本轮没有重新生成分发包或重跑安装，也未重复运行未改动的 Rust 全量测试。
