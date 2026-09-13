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
    use suzaku_map::ime::gpu::ThemePreset;
    use suzaku_map::ime::settings::PredictionSettingsPatch;
    use suzaku_map::languages::BuiltinLanguage;
    use suzaku_map::languages::model::{ModelProviderConfig, ModelScope, runtime as model_runtime};
    use suzaku_map::platform::linux_ime_control::{self, NativeImeStatus};
    use suzaku_map::ui::UiLanguage;

    const TRAY_ICON_SIZES: [i32; 4] = [22, 32, 48, 64];

    pub(crate) struct SystemTray {
        control_tx: Sender<TrayControl>,
        worker: Option<JoinHandle<()>>,
        theme: ThemePreset,
        ui_language: UiLanguage,
        has_icon: bool,
    }

    enum TrayControl {
        SetPanelVisible(bool),
        SetTheme(ThemePreset),
        SetUiLanguage(UiLanguage),
        RefreshInputMethod,
        ActivateInputMethod,
        ReleaseInputMethod,
        SetLanguage(BuiltinLanguage),
        SetPredictionEnabled(bool),
        SetPredictionSettings(PredictionSettingsPatch),
        ReloadImeSettings,
        CheckModel(ModelProviderConfig),
        WarmModel(ModelProviderConfig),
        ModelReport(ModelProviderConfig, Result<String, String>),
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
                "Backup saved: {} (see backup folder)",
                path.file_name().unwrap_or_default().to_string_lossy()
            ));
        }
        let path = match action {
            DataAction::OpenIme => paths.ime.parent().ok_or("配置路径无效")?,
            DataAction::OpenPanel => paths.panel.parent().ok_or("配置路径无效")?,
            _ => &paths.backups,
        };
        open_directory(path)?;
        Ok(format!("Opened: {}", path.display()))
    }

    #[derive(Default)]
    struct InputMethodMenuState {
        system: InputMethodState,
        busy: bool,
        status_error: Option<String>,
        operation_error: Option<String>,
        native: Option<NativeImeStatus>,
        native_error: Option<String>,
        model_busy: bool,
        model_status: Option<(ModelProviderConfig, String)>,
    }

    impl InputMethodMenuState {
        fn can_activate(&self) -> bool {
            !self.busy && (!self.system.active() || self.native_error.is_some())
        }

        fn can_release(&self) -> bool {
            !self.busy && self.system.restore_engine.is_some()
        }

        fn description(&self) -> String {
            if self.busy {
                return "Updating input method…".into();
            }
            if let Some(error) = self
                .operation_error
                .as_ref()
                .or(self.status_error.as_ref())
                .or(self.native_error.as_ref())
            {
                return error.clone();
            }
            if self.system.active() {
                if self.system.restore_engine.is_some() {
                    "Suzaku active; candidates appear while typing".into()
                } else {
                    "System-selected; switch back using the system input menu".into()
                }
            } else if let Some(current) = self.system.current_engine.as_deref() {
                format!("Current input method: {current}")
            } else {
                "Reading input method status…".into()
            }
        }

        fn release_label(&self) -> String {
            match self.system.restore_engine.as_deref() {
                Some(_) if self.system.current_engine.is_some() && !self.system.active() => {
                    "End activation (keep current input method)".into()
                }
                Some(previous) => format!("Release and restore: {previous}"),
                None => "Release and restore previous input method".into(),
            }
        }

        fn native_unavailable_hint(&self) -> &str {
            self.native_error
                .as_deref()
                .unwrap_or("Input service not ready; activation starts it automatically")
        }
    }

    fn input_language_label(language: BuiltinLanguage, ui: UiLanguage) -> &'static str {
        ui.tr(match language {
            BuiltinLanguage::English => "English",
            BuiltinLanguage::ChineseSimplified => "Chinese · Pinyin",
            BuiltinLanguage::Japanese => "Japanese · Romaji",
        })
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
        pub(crate) fn has_icon(&self) -> bool {
            self.has_icon
        }

        pub(crate) fn request_quit(&self) -> bool {
            self.control_tx.send(TrayControl::Quit).is_ok()
        }

        pub(crate) fn set_prediction_settings(&self, patch: PredictionSettingsPatch) -> bool {
            self.control_tx
                .send(TrayControl::SetPredictionSettings(patch))
                .is_ok()
        }
        pub(crate) fn set_panel_visible(&self, visible: bool) {
            let _ = self.control_tx.send(TrayControl::SetPanelVisible(visible));
        }

        pub(crate) fn set_ui_language(&mut self, language: UiLanguage) {
            if self.ui_language != language
                && self
                    .control_tx
                    .send(TrayControl::SetUiLanguage(language))
                    .is_ok()
            {
                self.ui_language = language;
            }
        }

        pub(crate) fn set_theme(&mut self, theme: ThemePreset) {
            // The app can sync this at idle; only actual changes enqueue raster work.
            if self.theme != theme && self.control_tx.send(TrayControl::SetTheme(theme)).is_ok() {
                self.theme = theme;
            }
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
        ui_language: UiLanguage,
        input_method: InputMethodMenuState,
        data_busy: bool,
        data_report: Option<String>,
    }

    // Service lifetime must not depend on a desktop tray extension being installed.
    // The fallback still processes lifecycle commands and sends UI acknowledgements.
    struct TrayHandle {
        live: Option<Handle<SuzakuTray>>,
        fallback: std::sync::Mutex<SuzakuTray>,
    }

    impl TrayHandle {
        fn update(&self, update: impl FnOnce(&mut SuzakuTray)) -> Option<()> {
            if let Some(live) = &self.live {
                live.update(update)
            } else {
                update(&mut self.fallback.lock().unwrap());
                Some(())
            }
        }

        fn shutdown(&self) {
            if let Some(live) = &self.live {
                live.shutdown().wait();
            }
        }
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
                        Some("Tray controller unavailable; restart Suzaku".into());
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
                    self.data_report = Some("Data manager unavailable".into());
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
            self.ui_language.tr("Suzaku Input Method").into()
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
                    "{}\n{}",
                    self.ui_language.message(&self.input_method.description()),
                    self.ui_language
                        .tr("Right-click for input options; left-click to show or hide")
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
            let mut menu = vec![
                CheckmarkItem {
                    label: "Activate Suzaku".into(),
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
                    label: self.input_method.release_label(),
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
                    label: self.input_method.description(),
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
                        .map(|state| {
                            format!(
                                "Input language: {}",
                                input_language_label(state.settings.language, self.ui_language)
                            )
                        })
                        .unwrap_or_else(|| "Input language (host disconnected)".into()),
                    enabled: !self.input_method.busy && self.input_method.native.is_some(),
                    submenu: BuiltinLanguage::ALL
                        .into_iter()
                        .map(|language| {
                            CheckmarkItem {
                                label: input_language_label(language, self.ui_language).into(),
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
                    label: self
                        .input_method
                        .native
                        .as_ref()
                        .map(|state| {
                            if state.settings.provider.scope == ModelScope::Cloud {
                                "LLM suggestions (cloud, consent required)"
                            } else {
                                "LLM suggestions (local)"
                            }
                        })
                        .unwrap_or("LLM suggestions")
                        .into(),
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
                            "Pending" => "Generating; local candidates remain available".into(),
                            "Ready" => "Model candidates ready (marked AI)".into(),
                            "Unavailable" => state.prediction_error.clone().unwrap_or_else(|| {
                                "Model unavailable; local candidates retained".into()
                            }),
                            _ if state.settings.llm_enabled => {
                                if state.settings.provider.scope == ModelScope::Cloud
                                    && !state.settings.provider.cloud_consent
                                {
                                    "Cloud consent required; no input text is sent".into()
                                } else {
                                    "LLM enabled; waiting for input".into()
                                }
                            }
                            _ => "Using local candidates; no model request".into(),
                        })
                        .unwrap_or_else(|| self.input_method.native_unavailable_hint().into()),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: if self.input_method.model_busy {
                        "Checking / warming model; typing remains available…".into()
                    } else if let Some(state) = &self.input_method.native {
                        self.input_method
                            .model_status
                            .as_ref()
                            .filter(|(config, _)| config == &state.settings.provider)
                            .map(|(_, status)| status.clone())
                            .unwrap_or_else(|| {
                                if state.settings.provider.scope == ModelScope::Cloud {
                                    format!(
                                        "Cloud model: {} (not checked online)",
                                        state.settings.provider.model
                                    )
                                } else if state.settings.provider.model == "auto" {
                                    "Model: local discovery, preferring LLaMA".into()
                                } else {
                                    format!(
                                        "Model: {} (not checked)",
                                        state.settings.provider.model
                                    )
                                }
                            })
                    } else {
                        "Model status unknown".into()
                    },
                    enabled: false,
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: "Discover / check local models".into(),
                    enabled: !self.input_method.model_busy
                        && self.input_method.native.as_ref().is_some_and(|state| {
                            state.settings.provider.scope == ModelScope::Local
                        }),
                    activate: Box::new(|tray: &mut Self| tray.request_model(false)),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: "Warm Ollama model (unload after 5 minutes idle)".into(),
                    enabled: !self.input_method.model_busy
                        && self.input_method.native.as_ref().is_some_and(|state| {
                            state.settings.provider.scope == ModelScope::Local
                                && state.settings.provider.uses_ollama_api()
                        }),
                    activate: Box::new(|tray: &mut Self| tray.request_model(true)),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: "Reload model settings".into(),
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
                        "Hide panel".into()
                    } else {
                        "Show panel".into()
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
                    label: "Settings".into(),
                    icon_name: "preferences-system".into(),
                    activate: Box::new(|tray: &mut Self| {
                        tray.panel_visible = true;
                        tray.send(PanelUserEvent::OpenSettings);
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: "Reset window position".into(),
                    icon_name: "view-restore".into(),
                    activate: Box::new(|tray: &mut Self| {
                        tray.panel_visible = true;
                        tray.send(PanelUserEvent::ResetPanelPosition);
                    }),
                    ..Default::default()
                }
                .into(),
                SubMenu {
                    label: "Data management".into(),
                    submenu: vec![
                        StandardItem {
                            label: if self.data_busy {
                                "Managing data…"
                            } else {
                                self.data_report
                                    .as_deref()
                                    .unwrap_or("Settings only; no input history or model weights")
                            }
                            .into(),
                            enabled: false,
                            ..Default::default()
                        }
                        .into(),
                        StandardItem {
                            label: "Back up settings now".into(),
                            enabled: !self.data_busy,
                            activate: Box::new(|tray: &mut Self| {
                                tray.request_data(DataAction::Backup)
                            }),
                            ..Default::default()
                        }
                        .into(),
                        StandardItem {
                            label: "Open backup folder".into(),
                            enabled: !self.data_busy,
                            activate: Box::new(|tray: &mut Self| {
                                tray.request_data(DataAction::OpenBackups)
                            }),
                            ..Default::default()
                        }
                        .into(),
                        StandardItem {
                            label: "Open input method settings folder".into(),
                            enabled: !self.data_busy,
                            activate: Box::new(|tray: &mut Self| {
                                tray.request_data(DataAction::OpenIme)
                            }),
                            ..Default::default()
                        }
                        .into(),
                        StandardItem {
                            label: "Open panel settings folder".into(),
                            enabled: !self.data_busy,
                            activate: Box::new(|tray: &mut Self| {
                                tray.request_data(DataAction::OpenPanel)
                            }),
                            ..Default::default()
                        }
                        .into(),
                        StandardItem {
                            label: "Restore after quitting: suzaku-tool data restore".into(),
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
                    label: "Quit Suzaku".into(),
                    enabled: !self.input_method.busy,
                    icon_name: "application-exit".into(),
                    activate: Box::new(|tray: &mut Self| {
                        tray.request_input_method(TrayControl::Quit);
                    }),
                    ..Default::default()
                }
                .into(),
            ];
            localize_menu(&mut menu, self.ui_language);
            menu
        }
    }

    // Localize the complete message before truncation, otherwise a long payload
    // can remove a template's suffix and prevent localization. Escape access-key
    // underscores exactly once here, including external prediction diagnostics.
    fn localize_menu<T>(menu: &mut [MenuItem<T>], language: UiLanguage) {
        for item in menu {
            let label = match item {
                MenuItem::Standard(item) => &mut item.label,
                MenuItem::Checkmark(item) => &mut item.label,
                MenuItem::SubMenu(item) => {
                    localize_menu(&mut item.submenu, language);
                    &mut item.label
                }
                _ => continue,
            };
            *label = menu_label(&language.message(label));
        }
    }

    pub(crate) fn start_system_tray(
        proxy: EventLoopProxy<PanelUserEvent>,
        panel_visible: bool,
        theme: ThemePreset,
        ui_language: UiLanguage,
    ) -> Option<SystemTray> {
        let icons: Vec<Icon> = TRAY_ICON_SIZES
            .into_iter()
            .map(|size| themed_icon(theme, size))
            .collect();
        let (control_tx, control_rx) = mpsc::channel();
        let make_tray = || SuzakuTray {
            proxy: proxy.clone(),
            control_tx: control_tx.clone(),
            icons: icons.clone(),
            panel_visible,
            ui_language,
            input_method: InputMethodMenuState {
                busy: true,
                ..Default::default()
            },
            data_busy: false,
            data_report: None,
        };
        let live = match make_tray().spawn() {
            Ok(handle) => Some(handle),
            Err(error) => {
                eprintln!("Suzaku system tray unavailable: {error}");
                None
            }
        };
        let handle = TrayHandle {
            live,
            fallback: std::sync::Mutex::new(make_tray()),
        };
        start_tray_control_worker(handle, control_tx, control_rx, theme, ui_language)
    }

    fn start_tray_control_worker(
        handle: TrayHandle,
        control_tx: Sender<TrayControl>,
        control_rx: mpsc::Receiver<TrayControl>,
        theme: ThemePreset,
        ui_language: UiLanguage,
    ) -> Option<SystemTray> {
        let model_report_tx = control_tx.clone();
        let has_icon = handle.live.is_some();
        let worker = match thread::Builder::new()
            .name("suzaku-tray-control".into())
            .spawn(move || {
                let mut input_method = InputMethodController::new(IBusBackend::default());
                let startup = input_method.start();
                refresh_input_method(&handle, &mut input_method);
                let _ = handle.update(|tray| {
                    tray.input_method.busy = false;
                    tray.input_method.operation_error = startup.err();
                    if let Some(error) = &tray.input_method.operation_error {
                        tray.send(PanelUserEvent::InputMethodError(error.clone()));
                    }
                });
                while let Ok(command) = control_rx.recv() {
                    match command {
                        TrayControl::SetPanelVisible(visible) => {
                            let _ = handle.update(|tray| tray.panel_visible = visible);
                        }
                        TrayControl::SetUiLanguage(language) => {
                            let _ = handle.update(|tray| tray.ui_language = language);
                        }
                        TrayControl::SetTheme(theme) => {
                            let icons = TRAY_ICON_SIZES
                                .into_iter()
                                .map(|size| themed_icon(theme, size))
                                .collect();
                            let _ = handle.update(|tray| tray.icons = icons);
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
                                        model_runtime::warm_up(&config).map(|()| {
                                            format!(
                                                "Model warmed: {} (no input text sent)",
                                                config.model
                                            )
                                        })
                                    } else {
                                        model_runtime::inspect(&config)
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
                                        .unwrap_or_else(|_| {
                                            Err("Data operation failed; please retry".into())
                                        });
                                    let _ = report_tx.send(TrayControl::DataReport(result));
                                })
                                .is_err()
                            {
                                let _ = handle.update(|tray| {
                                    tray.data_busy = false;
                                    tray.data_report =
                                        Some("Could not start data operation".into());
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
                            let _ = handle.update(|tray| tray.input_method.busy = true);
                            let result = if activating {
                                input_method.activate()
                            } else if quitting {
                                input_method.shutdown()
                            } else {
                                input_method.release()
                            };
                            let succeeded = result.is_ok();
                            if !quitting || !succeeded {
                                refresh_input_method(&handle, &mut input_method);
                            }
                            let _ = handle.update(|tray| {
                                tray.input_method.system = input_method.state().clone();
                                tray.input_method.busy = false;
                                tray.input_method.status_error = None;
                                tray.input_method.operation_error = result.err();
                                if let Some(error) = &tray.input_method.operation_error {
                                    tray.send(PanelUserEvent::InputMethodError(error.clone()));
                                }
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
                            if let Err(error) = input_method.shutdown() {
                                eprintln!(
                                    "Suzaku could not safely finish input-service shutdown: {error}"
                                );
                            }
                            handle.shutdown();
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
            theme,
            ui_language,
            has_icon,
        })
    }

    fn refresh_input_method(
        handle: &TrayHandle,
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
            match native {
                Ok(state) => {
                    tray.input_method.native = Some(state);
                    tray.input_method.native_error = None;
                }
                Err(error) => {
                    tray.input_method.native = None;
                    tray.input_method.native_error = Some(error);
                }
            }
            if let Some(state) = &tray.input_method.native {
                tray.send(PanelUserEvent::InputMethodSettingsChanged(
                    state.settings.clone(),
                ));
            }
        });
    }

    fn publish_native_settings(handle: &TrayHandle, result: Result<NativeImeStatus, String>) {
        publish_settings_result(handle, result, false);
    }

    fn publish_settings_result(
        handle: &TrayHandle,
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
                tray.input_method.native_error = None;
            } else {
                tray.input_method.native = None;
                tray.input_method.native_error = error.clone();
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
            if panel_write_finished && let Some(error) = &tray.input_method.operation_error {
                tray.send(PanelUserEvent::InputMethodError(error.clone()));
            }
        });
    }

    #[cfg(test)]
    fn suzaku_icon(size: i32) -> Icon {
        themed_icon(ThemePreset::Suzaku, size)
    }

    fn themed_icon(theme: ThemePreset, size: i32) -> Icon {
        let size = size.clamp(1, 256);
        Icon {
            width: size,
            height: size,
            data: suzaku_map::ime::gpu::theme_icon_argb(theme, size as u32),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            InputMethodMenuState, InputMethodState, TRAY_ICON_SIZES, menu_label, suzaku_icon,
        };
        use crate::input_method::SUZAKU_ENGINE;

        #[test]
        fn language_updates_are_deduplicated_and_nested_menus_keep_actions_and_ids() {
            use super::*;
            let (control_tx, control_rx) = std::sync::mpsc::channel();
            let mut tray = SystemTray {
                control_tx,
                worker: None,
                theme: ThemePreset::Suzaku,
                ui_language: UiLanguage::English,
                has_icon: true,
            };
            tray.set_ui_language(UiLanguage::English);
            assert!(control_rx.try_recv().is_err());
            for language in UiLanguage::ALL {
                tray.set_ui_language(language);
                assert!(
                    matches!(control_rx.try_recv(), Ok(TrayControl::SetUiLanguage(actual)) if actual == language)
                );
                tray.set_ui_language(language);
                assert!(control_rx.try_recv().is_err());
                let mut menu: Vec<MenuItem<()>> = vec![
                    SubMenu {
                        label: "Settings".into(),
                        submenu: vec![
                            StandardItem {
                                label: "Current input method: some_ime".into(),
                                enabled: false,
                                ..Default::default()
                            }
                            .into(),
                        ],
                        ..Default::default()
                    }
                    .into(),
                ];
                localize_menu(&mut menu, language);
                let MenuItem::SubMenu(item) = &menu[0] else {
                    panic!("submenu retained")
                };
                assert_eq!(item.label, language.tr("Settings"));
                let MenuItem::Standard(child) = &item.submenu[0] else {
                    panic!("item retained")
                };
                assert!(!child.enabled);
                assert!(child.label.ends_with("some__ime"));
            }
        }

        #[test]
        fn unchanged_theme_does_not_enqueue_tray_raster_work() {
            use super::{SystemTray, ThemePreset, TrayControl};
            let (control_tx, control_rx) = std::sync::mpsc::channel();
            let mut tray = SystemTray {
                control_tx,
                worker: None,
                theme: ThemePreset::Suzaku,
                ui_language: Default::default(),
                has_icon: true,
            };
            tray.set_theme(ThemePreset::Suzaku);
            assert!(control_rx.try_recv().is_err());
            for theme in [
                ThemePreset::Baihu,
                ThemePreset::Qinglong,
                ThemePreset::Xuanwu,
                ThemePreset::Suzaku,
            ] {
                tray.set_theme(theme);
                assert!(
                    matches!(control_rx.try_recv(), Ok(TrayControl::SetTheme(actual)) if actual == theme)
                );
                for _ in 0..20 {
                    tray.set_theme(theme);
                }
                assert!(control_rx.try_recv().is_err());
            }
        }

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
            assert!(
                state
                    .description()
                    .contains("candidates appear while typing")
            );
            assert_eq!(state.release_label(), "Release and restore: rime");
            state.system.current_engine = Some("mozc-jp".into());
            assert!(state.can_activate());
            assert!(state.can_release());
            assert!(state.release_label().contains("keep current input method"));
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
            assert!(state.description().contains("Updating input method"));
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
        fn disconnected_host_reports_connection_failure_not_a_required_upgrade() {
            let mut state = InputMethodMenuState {
                native_error: Some("Suzaku 输入法服务未运行或连接已断开".into()),
                ..Default::default()
            };
            assert_eq!(state.description(), state.native_unavailable_hint());
            assert!(!state.native_unavailable_hint().contains("新版"));
            state.system.current_engine = Some(SUZAKU_ENGINE.into());
            assert!(
                state.can_activate(),
                "a disconnected active engine must be restartable"
            );
            state.busy = true;
            assert!(!state.can_activate());
            assert!(
                InputMethodMenuState::default()
                    .native_unavailable_hint()
                    .contains("starts it automatically")
            );
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
            assert!(state.description().contains("System-selected"));
        }

        #[test]
        fn input_method_menu_labels_preserve_underscores_and_bound_long_errors() {
            assert_eq!(menu_label("some_ime"), "some__ime");
            let label = menu_label(&"错".repeat(100));
            assert_eq!(label.chars().count(), 73);
            assert!(label.ends_with('…'));
        }

        #[test]
        fn final_menu_labels_localize_before_bounding_and_escape_external_identifiers_once() {
            use super::*;
            for language in UiLanguage::ALL {
                for (key, payload) in [
                    ("Model: {} (not checked)", "long_model_name_".repeat(12)),
                    (
                        "Backup saved: {} (see backup folder)",
                        "备份_file_".repeat(12),
                    ),
                    ("Current input method: {}", "some_ime".into()),
                    ("{}", "external_error_".repeat(12)),
                ] {
                    let raw = key.replace("{}", &payload);
                    let expected = menu_label(&language.message(&raw));
                    let mut menu: Vec<MenuItem<()>> = vec![
                        StandardItem {
                            label: raw,
                            enabled: false,
                            ..Default::default()
                        }
                        .into(),
                    ];
                    localize_menu(&mut menu, language);
                    let MenuItem::Standard(item) = &menu[0] else {
                        panic!("status retained")
                    };
                    assert_eq!(item.label, expected, "{language:?} {key}");
                    assert!(!item.enabled);
                }
            }
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
        pub(crate) fn has_icon(&self) -> bool {
            false
        }
        pub(crate) fn request_quit(&self) -> bool {
            false
        }
        pub(crate) fn set_panel_visible(&self, _visible: bool) {}
        pub(crate) fn set_theme(&mut self, _theme: suzaku_map::ime::gpu::ThemePreset) {}
        pub(crate) fn set_ui_language(&mut self, _language: suzaku_map::ui::UiLanguage) {}
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
        _theme: suzaku_map::ime::gpu::ThemePreset,
        _language: suzaku_map::ui::UiLanguage,
    ) -> Option<SystemTray> {
        None
    }
}

pub(super) use platform::{SystemTray, start_system_tray};
