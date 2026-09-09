use super::PanelUserEvent;
use winit::event_loop::EventLoopProxy;

#[cfg(target_os = "linux")]
mod platform {
    use super::{EventLoopProxy, PanelUserEvent};
    use crate::input_method::{IBusBackend, InputMethodController, InputMethodState};
    use ksni::blocking::{Handle, TrayMethods};
    use ksni::menu::{CheckmarkItem, StandardItem, SubMenu};
    use ksni::{Icon, MenuItem, ToolTip, Tray};
    use std::sync::mpsc::{self, Sender};
    use std::thread::{self, JoinHandle};
    use suzaku_map::ime::settings::PredictionSettingsPatch;
    use suzaku_map::languages::BuiltinLanguage;
    use suzaku_map::languages::llama::{LlamaProviderConfig, runtime as llama_runtime};
    use suzaku_map::platform::linux_ime_control::{self, NativeImeStatus};

    const TRAY_ICON_SIZES: [i32; 4] = [22, 32, 48, 64];

    pub(crate) struct SystemTray {
        control_tx: Sender<TrayControl>,
        worker: Option<JoinHandle<()>>,
    }

    enum TrayControl {
        SetPanelVisible(bool),
        RefreshInputMethod,
        ActivateInputMethod,
        ReleaseInputMethod,
        SetLanguage(BuiltinLanguage),
        SetPredictionEnabled(bool),
        SetPredictionSettings(PredictionSettingsPatch),
        ReloadImeSettings,
        CheckModel(LlamaProviderConfig),
        WarmModel(LlamaProviderConfig),
        ModelReport(LlamaProviderConfig, Result<String, String>),
        ManageData(DataAction),
        DataReport(Result<String, String>),
        Quit,
        Shutdown,
    }

    #[derive(Clone, Copy)]
    enum DataAction {
        Backup,
        OpenIme,
        OpenPanel,
        OpenBackups,
    }

    fn manage_data(action: DataAction) -> Result<String, String> {
        use suzaku_map::data::{backup, open_directory, paths::DataPaths};
        let paths = DataPaths::current()?;
        if matches!(action, DataAction::Backup) {
            let path = backup::backup_now(&paths)?;
            return Ok(format!(
                "已备份：{}（可在备份目录查看）",
                path.file_name().unwrap_or_default().to_string_lossy()
            ));
        }
        let path = match action {
            DataAction::OpenIme => paths.ime.parent().ok_or("配置路径无效")?,
            DataAction::OpenPanel => paths.panel.parent().ok_or("配置路径无效")?,
            _ => &paths.backups,
        };
        open_directory(path)?;
        Ok(format!("已打开：{}", path.display()))
    }

    #[derive(Default)]
    struct InputMethodMenuState {
        system: InputMethodState,
        busy: bool,
        status_error: Option<String>,
        operation_error: Option<String>,
        native: Option<NativeImeStatus>,
        model_busy: bool,
        model_status: Option<(LlamaProviderConfig, String)>,
    }

    impl InputMethodMenuState {
        fn can_activate(&self) -> bool {
            !self.busy && !self.system.active()
        }

        fn can_release(&self) -> bool {
            !self.busy && self.system.restore_engine.is_some()
        }

        fn description(&self) -> String {
            if self.busy {
                return "正在切换输入法…".into();
            }
            if let Some(error) = self.operation_error.as_ref().or(self.status_error.as_ref()) {
                return error.clone();
            }
            if self.system.active() {
                if self.system.restore_engine.is_some() {
                    "Suzaku 已激活，输入时自动显示候选窗".into()
                } else {
                    "Suzaku 由系统选中；请通过系统菜单切换回其他输入法".into()
                }
            } else if let Some(current) = self.system.current_engine.as_deref() {
                format!("当前输入法：{current}")
            } else {
                "正在读取输入法状态…".into()
            }
        }

        fn release_label(&self) -> String {
            match self.system.restore_engine.as_deref() {
                Some(_) if self.system.current_engine.is_some() && !self.system.active() => {
                    "结束激活（保留当前输入法）".into()
                }
                Some(previous) => format!("释放并恢复：{previous}"),
                None => "释放并恢复原输入法".into(),
            }
        }
    }

    fn menu_label(text: &str) -> String {
        // D-Bus menus interpret underscores as access keys. Bound error/status rows.
        let mut characters = text.chars();
        let mut label: String = characters.by_ref().take(72).collect();
        if characters.next().is_some() {
            label.push('…');
        }
        label.replace('_', "__")
    }

    impl SystemTray {
        pub(crate) fn set_prediction_settings(&self, patch: PredictionSettingsPatch) -> bool {
            self.control_tx
                .send(TrayControl::SetPredictionSettings(patch))
                .is_ok()
        }
        pub(crate) fn set_panel_visible(&self, visible: bool) {
            let _ = self.control_tx.send(TrayControl::SetPanelVisible(visible));
        }

        pub(crate) fn shutdown(mut self) {
            let _ = self.control_tx.send(TrayControl::Shutdown);
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }

    struct SuzakuTray {
        proxy: EventLoopProxy<PanelUserEvent>,
        control_tx: Sender<TrayControl>,
        icons: Vec<Icon>,
        panel_visible: bool,
        input_method: InputMethodMenuState,
        data_busy: bool,
        data_report: Option<String>,
    }

    impl SuzakuTray {
        fn send(&self, event: PanelUserEvent) {
            let _ = self.proxy.send_event(event);
        }

        fn request_input_method(&mut self, command: TrayControl) {
            if !self.input_method.busy {
                self.input_method.busy = true;
                self.input_method.operation_error = None;
                if self.control_tx.send(command).is_err() {
                    self.input_method.busy = false;
                    self.input_method.operation_error =
                        Some("托盘控制服务不可用，请重新启动 Suzaku".into());
                }
            }
        }

        fn request_model(&mut self, warmup: bool) {
            if self.input_method.model_busy {
                return;
            }
            if let Some(state) = &self.input_method.native {
                let config = state.settings.provider.clone();
                let command = if warmup {
                    TrayControl::WarmModel(config)
                } else {
                    TrayControl::CheckModel(config)
                };
                self.input_method.model_busy = self.control_tx.send(command).is_ok();
            }
        }

        fn request_data(&mut self, action: DataAction) {
            if !self.data_busy {
                self.data_busy = self
                    .control_tx
                    .send(TrayControl::ManageData(action))
                    .is_ok();
                if !self.data_busy {
                    self.data_report = Some("数据管理服务不可用".into());
                }
            }
        }
    }

    impl Tray for SuzakuTray {
        const MENU_ON_ACTIVATE: bool = false;

        fn id(&self) -> String {
            "suzaku-panel".into()
        }

        fn title(&self) -> String {
            "Suzaku 输入法".into()
        }

        fn activate(&mut self, _x: i32, _y: i32) {
            self.panel_visible = !self.panel_visible;
            self.send(if self.panel_visible {
                PanelUserEvent::ShowPanel
            } else {
                PanelUserEvent::HidePanel
            });
        }

        fn icon_pixmap(&self) -> Vec<Icon> {
            self.icons.clone()
        }

        fn tool_tip(&self) -> ToolTip {
            ToolTip {
                icon_pixmap: self.icons.clone(),
                title: self.title(),
                description: format!(
                    "{}\n右键切换输入法，左键显示或隐藏面板",
                    self.input_method.description()
                ),
                ..Default::default()
            }
        }

        fn menu_about_to_show(&mut self) {
            // Refresh on demand, without polling or blocking the tray's D-Bus callbacks.
            if !self.input_method.busy {
                let _ = self.control_tx.send(TrayControl::RefreshInputMethod);
            }
        }

        fn menu(&self) -> Vec<MenuItem<Self>> {
            vec![
                CheckmarkItem {
                    label: "激活 Suzaku 输入法".into(),
                    checked: self.input_method.system.active(),
                    enabled: self.input_method.can_activate(),
                    activate: Box::new(|tray: &mut Self| {
                        if tray.input_method.can_activate() {
                            tray.request_input_method(TrayControl::ActivateInputMethod);
                        }
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: menu_label(&self.input_method.release_label()),
                    enabled: self.input_method.can_release(),
                    icon_name: "edit-undo".into(),
                    activate: Box::new(|tray: &mut Self| {
                        if tray.input_method.can_release() {
                            tray.request_input_method(TrayControl::ReleaseInputMethod);
                        }
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: menu_label(&self.input_method.description()),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                SubMenu {
                    label: self
                        .input_method
                        .native
                        .as_ref()
                        .map(|state| format!("输入语言：{}", state.settings.language.label()))
                        .unwrap_or_else(|| "输入语言（需更新输入法宿主）".into()),
                    enabled: !self.input_method.busy && self.input_method.native.is_some(),
                    submenu: BuiltinLanguage::ALL
                        .into_iter()
                        .map(|language| {
                            CheckmarkItem {
                                label: language.label().into(),
                                checked: self
                                    .input_method
                                    .native
                                    .as_ref()
                                    .is_some_and(|state| state.settings.language == language),
                                activate: Box::new(move |tray: &mut Self| {
                                    tray.request_input_method(TrayControl::SetLanguage(language))
                                }),
                                ..Default::default()
                            }
                            .into()
                        })
                        .collect(),
                    ..Default::default()
                }
                .into(),
                CheckmarkItem {
                    label: "本地 LLM 联想".into(),
                    enabled: !self.input_method.busy && self.input_method.native.is_some(),
                    checked: self
                        .input_method
                        .native
                        .as_ref()
                        .is_some_and(|state| state.settings.llm_enabled),
                    activate: Box::new(|tray: &mut Self| {
                        if let Some(status) = &tray.input_method.native {
                            tray.request_input_method(TrayControl::SetPredictionEnabled(
                                !status.settings.llm_enabled,
                            ));
                        }
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: self
                        .input_method
                        .native
                        .as_ref()
                        .map(|state| match state.prediction.as_str() {
                            "Pending" => "LLM 正在联想，本地候选可立即使用".into(),
                            "Ready" => "LLM 候选已就绪（标记 AI）".into(),
                            "Unavailable" => state
                                .prediction_error
                                .clone()
                                .unwrap_or_else(|| "模型暂无结果或不可用，已保留本地候选".into()),
                            _ if state.settings.llm_enabled => {
                                "LLM 已启用，等待输入（需要本机模型服务）".into()
                            }
                            _ => "使用本地基础候选，未请求模型".into(),
                        })
                        .unwrap_or_else(|| "请安装新版 Suzaku 输入法宿主".into()),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: menu_label(&if self.input_method.model_busy {
                        "正在检查 / 预热模型，本地输入可继续使用…".into()
                    } else if let Some(state) = &self.input_method.native {
                        self.input_method
                            .model_status
                            .as_ref()
                            .filter(|(config, _)| config == &state.settings.provider)
                            .map(|(_, status)| status.clone())
                            .unwrap_or_else(|| {
                                format!("模型：{}（尚未检查）", state.settings.provider.model)
                            })
                    } else {
                        "模型服务状态未知".into()
                    }),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: "检查本机 Llama 模型".into(),
                    enabled: !self.input_method.model_busy && self.input_method.native.is_some(),
                    activate: Box::new(|tray: &mut Self| tray.request_model(false)),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: "预热 Llama（空闲 5 分钟后释放）".into(),
                    enabled: !self.input_method.model_busy
                        && self.input_method.native.as_ref().is_some_and(|state| {
                            state.settings.provider.endpoint.ends_with("/api/chat")
                        }),
                    activate: Box::new(|tray: &mut Self| tray.request_model(true)),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: "重新加载模型配置".into(),
                    enabled: !self.input_method.busy && self.input_method.native.is_some(),
                    activate: Box::new(|tray: &mut Self| {
                        tray.request_input_method(TrayControl::ReloadImeSettings)
                    }),
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                StandardItem {
                    label: if self.panel_visible {
                        "隐藏面板".into()
                    } else {
                        "显示面板".into()
                    },
                    icon_name: if self.panel_visible {
                        "view-hidden".into()
                    } else {
                        "view-visible".into()
                    },
                    activate: Box::new(|tray: &mut Self| {
                        tray.panel_visible = !tray.panel_visible;
                        tray.send(if tray.panel_visible {
                            PanelUserEvent::ShowPanel
                        } else {
                            PanelUserEvent::HidePanel
                        });
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: "设置".into(),
                    icon_name: "preferences-system".into(),
                    activate: Box::new(|tray: &mut Self| {
                        tray.panel_visible = true;
                        tray.send(PanelUserEvent::OpenSettings);
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: "重置窗口位置".into(),
                    icon_name: "view-restore".into(),
                    activate: Box::new(|tray: &mut Self| {
                        tray.panel_visible = true;
                        tray.send(PanelUserEvent::ResetPanelPosition);
                    }),
                    ..Default::default()
                }
                .into(),
                SubMenu {
                    label: "数据管理".into(),
                    submenu: vec![
                        StandardItem {
                            label: menu_label(if self.data_busy {
                                "正在处理数据…"
                            } else {
                                self.data_report
                                    .as_deref()
                                    .unwrap_or("仅管理配置，不保存输入历史或模型权重")
                            }),
                            enabled: false,
                            ..Default::default()
                        }
                        .into(),
                        StandardItem {
                            label: "立即备份配置".into(),
                            enabled: !self.data_busy,
                            activate: Box::new(|tray: &mut Self| {
                                tray.request_data(DataAction::Backup)
                            }),
                            ..Default::default()
                        }
                        .into(),
                        StandardItem {
                            label: "打开备份目录".into(),
                            enabled: !self.data_busy,
                            activate: Box::new(|tray: &mut Self| {
                                tray.request_data(DataAction::OpenBackups)
                            }),
                            ..Default::default()
                        }
                        .into(),
                        StandardItem {
                            label: "打开输入法配置目录".into(),
                            enabled: !self.data_busy,
                            activate: Box::new(|tray: &mut Self| {
                                tray.request_data(DataAction::OpenIme)
                            }),
                            ..Default::default()
                        }
                        .into(),
                        StandardItem {
                            label: "打开面板配置目录".into(),
                            enabled: !self.data_busy,
                            activate: Box::new(|tray: &mut Self| {
                                tray.request_data(DataAction::OpenPanel)
                            }),
                            ..Default::default()
                        }
                        .into(),
                        StandardItem {
                            label: "恢复：退出后使用 suzaku-tool data restore".into(),
                            enabled: false,
                            ..Default::default()
                        }
                        .into(),
                    ],
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                StandardItem {
                    label: "退出 Suzaku".into(),
                    enabled: !self.input_method.busy,
                    icon_name: "application-exit".into(),
                    activate: Box::new(|tray: &mut Self| {
                        tray.request_input_method(TrayControl::Quit);
                    }),
                    ..Default::default()
                }
                .into(),
            ]
        }
    }

    pub(crate) fn start_system_tray(
        proxy: EventLoopProxy<PanelUserEvent>,
        panel_visible: bool,
    ) -> Option<SystemTray> {
        let icons = TRAY_ICON_SIZES.into_iter().map(suzaku_icon).collect();
        let (control_tx, control_rx) = mpsc::channel();
        match (SuzakuTray {
            proxy,
            control_tx: control_tx.clone(),
            icons,
            panel_visible,
            input_method: InputMethodMenuState::default(),
            data_busy: false,
            data_report: None,
        })
        .spawn()
        {
            Ok(handle) => start_tray_control_worker(handle, control_tx, control_rx),
            Err(error) => {
                eprintln!("Suzaku system tray unavailable: {error}");
                None
            }
        }
    }

    fn start_tray_control_worker(
        handle: Handle<SuzakuTray>,
        control_tx: Sender<TrayControl>,
        control_rx: mpsc::Receiver<TrayControl>,
    ) -> Option<SystemTray> {
        let model_report_tx = control_tx.clone();
        let worker = match thread::Builder::new()
            .name("suzaku-tray-control".into())
            .spawn(move || {
                let mut input_method = InputMethodController::new(IBusBackend);
                refresh_input_method(&handle, &mut input_method);
                while let Ok(command) = control_rx.recv() {
                    match command {
                        TrayControl::SetPanelVisible(visible) => {
                            let _ = handle.update(|tray| tray.panel_visible = visible);
                        }
                        TrayControl::RefreshInputMethod => {
                            refresh_input_method(&handle, &mut input_method);
                        }
                        TrayControl::SetLanguage(language) => {
                            publish_native_settings(
                                &handle,
                                linux_ime_control::set_language(language),
                            );
                        }
                        TrayControl::SetPredictionEnabled(enabled) => {
                            publish_native_settings(
                                &handle,
                                linux_ime_control::set_llm_enabled(enabled),
                            );
                        }
                        TrayControl::SetPredictionSettings(patch) => {
                            publish_settings_result(
                                &handle,
                                linux_ime_control::set_prediction_settings(patch),
                                true,
                            );
                        }
                        TrayControl::ReloadImeSettings => {
                            publish_native_settings(&handle, linux_ime_control::reload_settings());
                        }
                        TrayControl::CheckModel(ref config)
                        | TrayControl::WarmModel(ref config) => {
                            let warmup = matches!(command, TrayControl::WarmModel(_));
                            let config = config.clone();
                            let report_tx = model_report_tx.clone();
                            let task = thread::Builder::new()
                                .name("suzaku-model-check".into())
                                .spawn(move || {
                                    let result = if warmup {
                                        llama_runtime::warm_up(&config).map(|()| {
                                            format!(
                                                "模型已预热：{}（未发送输入文本）",
                                                config.model
                                            )
                                        })
                                    } else {
                                        llama_runtime::inspect(&config)
                                            .map(|status| status.summary())
                                    }
                                    .map_err(|error| error.to_string());
                                    let _ =
                                        report_tx.send(TrayControl::ModelReport(config, result));
                                });
                            if task.is_err() {
                                let _ = handle.update(|tray| tray.input_method.model_busy = false);
                            }
                        }
                        TrayControl::ModelReport(config, result) => {
                            let _ = handle.update(|tray| {
                                tray.input_method.model_busy = false;
                                tray.input_method.model_status =
                                    Some((config, result.unwrap_or_else(|error| error)));
                            });
                        }
                        TrayControl::ManageData(action) => {
                            let report_tx = model_report_tx.clone();
                            if thread::Builder::new()
                                .name("suzaku-data".into())
                                .spawn(move || {
                                    let result = std::panic::catch_unwind(|| manage_data(action))
                                        .unwrap_or_else(|_| Err("数据操作未完成，请重试".into()));
                                    let _ = report_tx.send(TrayControl::DataReport(result));
                                })
                                .is_err()
                            {
                                let _ = handle.update(|tray| {
                                    tray.data_busy = false;
                                    tray.data_report = Some("无法启动数据管理任务".into());
                                });
                            }
                        }
                        TrayControl::DataReport(result) => {
                            let _ = handle.update(|tray| {
                                tray.data_busy = false;
                                tray.data_report = Some(result.unwrap_or_else(|error| error));
                            });
                        }
                        TrayControl::ActivateInputMethod
                        | TrayControl::ReleaseInputMethod
                        | TrayControl::Quit => {
                            let activating = matches!(command, TrayControl::ActivateInputMethod);
                            let quitting = matches!(command, TrayControl::Quit);
                            let result = if activating {
                                input_method.activate()
                            } else {
                                input_method.release()
                            };
                            let succeeded = result.is_ok();
                            let _ = handle.update(|tray| {
                                tray.input_method.system = input_method.state().clone();
                                tray.input_method.busy = false;
                                tray.input_method.status_error = None;
                                tray.input_method.operation_error = result.err();
                                if succeeded && activating {
                                    // Native IBus candidates appear on input without taking focus.
                                    tray.panel_visible = false;
                                    tray.send(PanelUserEvent::HidePanel);
                                }
                                if succeeded && quitting {
                                    tray.send(PanelUserEvent::Quit);
                                }
                                // On restore failure, stay running so the user can retry.
                            });
                        }
                        TrayControl::Shutdown => {
                            if let Err(error) = input_method.release() {
                                eprintln!(
                                    "Suzaku could not restore the previous input method: {error}"
                                );
                            }
                            handle.shutdown().wait();
                            break;
                        }
                    }
                }
            }) {
            Ok(worker) => worker,
            Err(error) => {
                eprintln!("Suzaku tray control worker unavailable: {error}");
                return None;
            }
        };
        Some(SystemTray {
            control_tx,
            worker: Some(worker),
        })
    }

    fn refresh_input_method(
        handle: &Handle<SuzakuTray>,
        input_method: &mut InputMethodController<IBusBackend>,
    ) {
        let result = input_method.refresh();
        let _ = handle.update(|tray| {
            tray.input_method.system = input_method.state().clone();
            tray.input_method.status_error = result.err();
        });
        // This query runs only at startup/menu-open; it never polls input events.
        let native = linux_ime_control::status();
        let _ = handle.update(|tray| {
            tray.input_method.native = native.ok();
            if let Some(state) = &tray.input_method.native {
                tray.send(PanelUserEvent::InputMethodSettingsChanged(
                    state.settings.clone(),
                ));
            }
        });
    }

    fn publish_native_settings(
        handle: &Handle<SuzakuTray>,
        result: Result<NativeImeStatus, String>,
    ) {
        publish_settings_result(handle, result, false);
    }

    fn publish_settings_result(
        handle: &Handle<SuzakuTray>,
        result: Result<NativeImeStatus, String>,
        panel_write_finished: bool,
    ) {
        // A timed-out write may have succeeded. Reconcile before rolling the companion UI back.
        let (confirmed, error) = match result {
            Ok(state) => (Some(state), None),
            Err(error) => (linux_ime_control::status().ok(), Some(error)),
        };
        let _ = handle.update(|tray| {
            tray.input_method.busy = false;
            if let Some(state) = confirmed {
                tray.input_method.native = Some(state);
            }
            if panel_write_finished {
                tray.send(PanelUserEvent::PredictionSettingsApplied(
                    tray.input_method
                        .native
                        .as_ref()
                        .map(|state| state.settings.clone()),
                ));
            } else if let Some(state) = &tray.input_method.native {
                tray.send(PanelUserEvent::InputMethodSettingsChanged(
                    state.settings.clone(),
                ));
            }
            tray.input_method.operation_error = error;
        });
    }

    fn suzaku_icon(size: i32) -> Icon {
        let size = size.max(1);
        let mut data = vec![0; size as usize * size as usize * 4];
        let center = size as f32 / 2.0;
        let radius = size as f32 * 0.46;

        for y in 0..size {
            for x in 0..size {
                let nx = (x as f32 + 0.5 - center) / radius;
                let ny = (y as f32 + 0.5 - center) / radius;
                let distance = (nx * nx + ny * ny).sqrt();
                if distance > 1.04 {
                    continue;
                }

                let alpha = ((1.04 - distance) / 0.08).clamp(0.0, 1.0);
                let mut color = if distance > 0.87 {
                    [116, 18, 36]
                } else if ny < -0.2 {
                    [235, 54, 62]
                } else {
                    [205, 28, 52]
                };

                // A compact gold phoenix: raised wings, head, body, and split tail.
                let wing_y = 0.12 - nx.abs() * 0.54;
                let wing =
                    nx.abs() < 0.72 && ny > -0.34 && ny < 0.28 && (ny - wing_y).abs() < 0.105;
                let head = nx * nx + (ny + 0.31) * (ny + 0.31) < 0.15 * 0.15;
                let body = nx.abs() < 0.105 && (-0.24..=0.46).contains(&ny);
                let left_tail = nx < 0.0
                    && (-0.34..=-0.04).contains(&nx)
                    && (ny - (0.42 - nx * 0.75)).abs() < 0.085;
                let right_tail = nx >= 0.0
                    && (0.04..=0.34).contains(&nx)
                    && (ny - (0.42 + nx * 0.75)).abs() < 0.085;
                if wing || head || body || left_tail || right_tail {
                    color = [255, 205, 72];
                }

                let eye = (nx - 0.075) * (nx - 0.075) + (ny + 0.345) * (ny + 0.345) < 0.035 * 0.035;
                if eye {
                    color = [72, 17, 29];
                }

                let offset = ((y * size + x) * 4) as usize;
                data[offset] = (alpha * 255.0).round() as u8;
                data[offset + 1] = color[0];
                data[offset + 2] = color[1];
                data[offset + 3] = color[2];
            }
        }

        Icon {
            width: size,
            height: size,
            data,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            InputMethodMenuState, InputMethodState, TRAY_ICON_SIZES, menu_label, suzaku_icon,
        };
        use crate::input_method::SUZAKU_ENGINE;

        #[test]
        fn input_method_menu_tracks_owned_activation_and_manual_switches() {
            let mut state = InputMethodMenuState::default();
            assert!(state.can_activate());
            assert!(!state.can_release());
            state.system = InputMethodState {
                current_engine: Some(SUZAKU_ENGINE.into()),
                restore_engine: Some("rime".into()),
            };
            assert!(!state.can_activate());
            assert!(state.can_release());
            assert!(state.description().contains("自动显示候选窗"));
            assert_eq!(state.release_label(), "释放并恢复：rime");
            state.system.current_engine = Some("mozc-jp".into());
            assert!(state.can_activate());
            assert!(state.can_release());
            assert!(state.release_label().contains("保留当前输入法"));
        }

        #[test]
        fn pending_input_method_action_disables_both_actions() {
            let state = InputMethodMenuState {
                system: InputMethodState {
                    current_engine: Some("rime".into()),
                    restore_engine: Some("rime".into()),
                },
                busy: true,
                ..Default::default()
            };
            assert!(!state.can_activate());
            assert!(!state.can_release());
            assert!(state.description().contains("正在切换"));
        }

        #[test]
        fn restore_errors_are_visible_and_leave_the_release_action_enabled() {
            let state = InputMethodMenuState {
                system: InputMethodState {
                    current_engine: None,
                    restore_engine: Some("rime".into()),
                },
                operation_error: Some("恢复失败，请重试".into()),
                ..Default::default()
            };
            assert!(state.can_release());
            assert_eq!(state.description(), "恢复失败，请重试");
        }

        #[test]
        fn external_activation_explains_why_restore_is_unavailable() {
            let state = InputMethodMenuState {
                system: InputMethodState {
                    current_engine: Some(SUZAKU_ENGINE.into()),
                    restore_engine: None,
                },
                ..Default::default()
            };
            assert!(!state.can_release());
            assert!(state.description().contains("由系统选中"));
        }

        #[test]
        fn input_method_menu_labels_preserve_underscores_and_bound_long_errors() {
            assert_eq!(menu_label("some_ime"), "some__ime");
            let label = menu_label(&"错".repeat(100));
            assert_eq!(label.chars().count(), 73);
            assert!(label.ends_with('…'));
        }

        #[test]
        fn tray_icon_is_argb_at_each_advertised_size() {
            for size in TRAY_ICON_SIZES {
                let icon = suzaku_icon(size);
                assert_eq!(icon.width, size);
                assert_eq!(icon.height, size);
                assert_eq!(icon.data.len(), size as usize * size as usize * 4);
                assert!(icon.data.chunks_exact(4).any(|pixel| pixel[0] > 0));
                assert_eq!(icon.data[0], 0);
            }
        }

        #[test]
        fn tray_icon_contains_suzaku_red_and_gold() {
            let icon = suzaku_icon(64);
            let pixels = icon.data.chunks_exact(4).collect::<Vec<_>>();
            let red_pixels = pixels
                .iter()
                .filter(|pixel| pixel[0] > 0 && pixel[1] > 170 && pixel[2] < 90)
                .count();
            let gold_pixels = pixels
                .iter()
                .filter(|pixel| pixel[0] > 0 && pixel[1] > 240 && pixel[2] > 170)
                .count();

            assert!(red_pixels > 500);
            assert!(gold_pixels > 100);
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use super::{EventLoopProxy, PanelUserEvent};

    pub(crate) struct SystemTray;

    impl SystemTray {
        pub(crate) fn set_panel_visible(&self, _visible: bool) {}
        pub(crate) fn set_prediction_settings(
            &self,
            _patch: suzaku_map::ime::settings::PredictionSettingsPatch,
        ) -> bool {
            false
        }

        pub(crate) fn shutdown(self) {}
    }

    pub(crate) fn start_system_tray(
        _proxy: EventLoopProxy<PanelUserEvent>,
        _panel_visible: bool,
    ) -> Option<SystemTray> {
        None
    }
}

pub(super) use platform::{SystemTray, start_system_tray};
