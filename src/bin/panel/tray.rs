use super::PanelUserEvent;
use winit::event_loop::EventLoopProxy;

#[cfg(target_os = "linux")]
mod platform {
    use super::{EventLoopProxy, PanelUserEvent};
    use ksni::blocking::{Handle, TrayMethods};
    use ksni::menu::StandardItem;
    use ksni::{Icon, MenuItem, ToolTip, Tray};
    use std::sync::mpsc::{self, Sender};
    use std::thread::{self, JoinHandle};

    const TRAY_ICON_SIZES: [i32; 4] = [22, 32, 48, 64];

    pub(crate) struct SystemTray {
        control_tx: Sender<TrayControl>,
        worker: Option<JoinHandle<()>>,
    }

    enum TrayControl {
        SetPanelVisible(bool),
        Shutdown,
    }

    impl SystemTray {
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
        icons: Vec<Icon>,
        panel_visible: bool,
    }

    impl SuzakuTray {
        fn send(&self, event: PanelUserEvent) {
            let _ = self.proxy.send_event(event);
        }
    }

    impl Tray for SuzakuTray {
        const MENU_ON_ACTIVATE: bool = false;

        fn id(&self) -> String {
            "suzaku-panel".into()
        }

        fn title(&self) -> String {
            "Suzaku Input Panel".into()
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
                description: "Click to show or hide the input panel".into(),
                ..Default::default()
            }
        }

        fn menu(&self) -> Vec<MenuItem<Self>> {
            vec![
                StandardItem {
                    label: if self.panel_visible {
                        "Hide Panel".into()
                    } else {
                        "Show Panel".into()
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
                    label: "Reset Position".into(),
                    icon_name: "view-restore".into(),
                    activate: Box::new(|tray: &mut Self| {
                        tray.panel_visible = true;
                        tray.send(PanelUserEvent::ResetPanelPosition);
                    }),
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                StandardItem {
                    label: "Quit Suzaku".into(),
                    icon_name: "application-exit".into(),
                    activate: Box::new(|tray: &mut Self| {
                        tray.send(PanelUserEvent::Quit);
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
        match (SuzakuTray {
            proxy,
            icons,
            panel_visible,
        })
        .spawn()
        {
            Ok(handle) => start_tray_control_worker(handle),
            Err(error) => {
                eprintln!("Suzaku system tray unavailable: {error}");
                None
            }
        }
    }

    fn start_tray_control_worker(handle: Handle<SuzakuTray>) -> Option<SystemTray> {
        let (control_tx, control_rx) = mpsc::channel();
        let worker = match thread::Builder::new()
            .name("suzaku-tray-control".into())
            .spawn(move || {
                while let Ok(command) = control_rx.recv() {
                    match command {
                        TrayControl::SetPanelVisible(visible) => {
                            let _ = handle.update(|tray| tray.panel_visible = visible);
                        }
                        TrayControl::Shutdown => {
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
        use super::{TRAY_ICON_SIZES, suzaku_icon};

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
