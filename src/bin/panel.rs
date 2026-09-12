#![cfg(feature = "gpu")]

use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[path = "panel/app_state.rs"]
mod app_state;
#[cfg(all(test, target_os = "linux"))]
#[path = "panel/candidates_native_test.rs"]
mod candidates_native_test;
#[path = "panel/composition.rs"]
mod composition;
#[path = "panel/controller.rs"]
mod controller;
#[path = "panel/controller_hints.rs"]
mod controller_hints;
#[path = "panel/font_atlas.rs"]
mod font_atlas;
#[path = "panel/handwriting.rs"]
mod handwriting;
#[path = "panel/helpers.rs"]
mod helpers;
#[path = "panel/input.rs"]
mod input;
#[cfg(target_os = "linux")]
#[path = "panel/input_method.rs"]
mod input_method;
#[path = "panel/instance.rs"]
mod instance;
#[path = "panel/keyboard.rs"]
mod keyboard;
#[cfg(all(test, target_os = "linux"))]
#[path = "panel/keyboard_native_test.rs"]
mod keyboard_native_test;
#[path = "panel/native_sync.rs"]
mod native_sync;
#[path = "panel/prediction_settings.rs"]
mod prediction_settings;
#[path = "panel/render.rs"]
mod render;
#[cfg(all(test, target_os = "linux"))]
#[path = "panel/settings_native_test.rs"]
mod settings_native_test;
#[cfg(all(test, target_os = "linux"))]
#[path = "panel/status_native_test.rs"]
mod status_native_test;
#[cfg(test)]
#[path = "panel/tests.rs"]
mod tests;
#[path = "panel/tray.rs"]
mod tray;
#[path = "panel/voice.rs"]
mod voice;
#[path = "panel/windowing.rs"]
mod windowing;
#[cfg(all(test, target_os = "linux"))]
#[path = "panel/windowing_native_test.rs"]
mod windowing_native_test;

use crate::app_state::{
    FIRST_LAUNCH_WINDOW_SCALE, VoiceInputController, apply_display_settings, load_display_settings,
    normalize_pointer_stability_settings,
};
use crate::input::handle_panel_window_event;
use crate::instance::{InstanceLaunch, SingleInstanceGuard, claim_single_instance};
use crate::render::{FontAtlas, PanelVertex, TextVertex, create_font_atlas};
use crate::tray::{SystemTray, start_system_tray};
use suzaku_map::ime::gpu::{
    CandidateDensity, DisplayTextScale, FontFaceChoice, InputMode, InteractionKind, LlmModelPreset,
    LlmTemperaturePreset, PANEL_SCALE_MAX, PANEL_SCALE_MIN, PANEL_SCALE_STEP, PanelChromeState,
    PreviewStyle, TextSmoothing, TextSpacing, VoiceCaptureState, VoicePermissionState,
    WgpuCandidateRenderer,
};
use suzaku_map::ime::{EngineConfig, InputSource, SignalState, XRTabletImeEngine};
use suzaku_map::platform::gpu_host::{
    configure_event_loop_builder, decorate_main_window_attributes,
    decorate_settings_window_attributes, finish_main_window_creation,
    finish_settings_window_creation, main_window_runs_without_focus,
};
use suzaku_map::platform::panel_companion_dispatch::current_panel_companion_dispatch;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::dpi::PhysicalPosition;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowAttributes, WindowId};

const DEFAULT_PANEL_INNER_WIDTH: f64 = 900.0;
const DEFAULT_PANEL_INNER_HEIGHT: f64 = 480.0;
const MIN_PANEL_INNER_WIDTH: f64 = 420.0;
const MIN_PANEL_INNER_HEIGHT: f64 = 72.0;
const MAX_PANEL_INNER_WIDTH: f64 = 1395.0;
const MAX_PANEL_INNER_HEIGHT: f64 = 806.0;
const COMPACT_PANEL_INNER_WIDTH: f64 = 92.0;
const COMPACT_PANEL_INNER_HEIGHT: f64 = 92.0;
const UI_FRAME_INTERVAL: Duration = Duration::from_millis(16);
const MIN_STREAMING_VERTEX_BUFFER_CAPACITY: usize = 256;

fn periodic_frame_schedule(now: Instant, scheduled: Option<Instant>) -> (bool, Instant) {
    let deadline = scheduled.unwrap_or(now);
    if now >= deadline {
        (true, now + UI_FRAME_INTERVAL)
    } else {
        (false, deadline)
    }
}

fn next_vertex_buffer_capacity(current: usize, required: usize) -> usize {
    if required <= current {
        current
    } else {
        required
            .max(MIN_STREAMING_VERTEX_BUFFER_CAPACITY)
            .next_power_of_two()
    }
}

fn advance_commit_feedback_state(ticks: u8) -> (u8, bool) {
    if ticks == 0 {
        return (0, false);
    }
    let next = ticks - 1;
    (next, next == 0)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PanelUserEvent {
    ShowPanel,
    HidePanel,
    OpenSettings,
    ResetPanelPosition,
    InputMethodSettingsChanged(suzaku_map::ime::settings::ImeSettings),
    InputMethodError(String),
    PredictionSettingsApplied(Option<suzaku_map::ime::settings::ImeSettings>),
    NativeCompositionReady,
    NativeActionFinished {
        host: String,
        revision: u64,
        result: Result<bool, String>,
    },
    Quit,
}

const SHADER: &str = include_str!("panel/shapes.wgsl");

const TEXT_SHADER: &str = r#"
struct VsIn {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@group(0) @binding(0) var atlas_texture: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    out.position = vec4<f32>(input.position, 0.0, 1.0);
    out.uv = input.uv;
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    let alpha = textureSample(atlas_texture, atlas_sampler, input.uv).r;
    return vec4<f32>(input.color.rgb, input.color.a * alpha);
}
"#;

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(target_os = "linux")]
    let _data_lease =
        suzaku_map::data::files::DataLease::current_shared().map_err(std::io::Error::other)?;
    let event_loop = build_event_loop()?;
    let event_proxy = event_loop.create_proxy();
    let instance = match claim_single_instance(event_proxy.clone()) {
        Ok(InstanceLaunch::Primary(instance)) => Some(instance),
        Ok(InstanceLaunch::ExistingSignaled) => return Ok(()),
        Err(error) => {
            eprintln!("Suzaku single-instance control unavailable: {error}");
            None
        }
    };
    let mut app = PanelApp::new(event_proxy, instance);
    event_loop.run_app(&mut app)?;
    Ok(())
}

fn build_event_loop() -> Result<EventLoop<PanelUserEvent>, winit::error::EventLoopError> {
    let mut builder = EventLoop::<PanelUserEvent>::with_user_event();
    configure_event_loop_builder(&mut builder);
    builder.build()
}

fn panel_window_icon() -> Option<winit::window::Icon> {
    let mut rgba = suzaku_map::ime::gpu::suzaku_icon_argb(64);
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.rotate_left(1);
    }
    winit::window::Icon::from_rgba(rgba, 64, 64).ok()
}

fn panel_window_attributes() -> WindowAttributes {
    let settings = load_display_settings();
    let scale = settings
        .as_ref()
        .map(|settings| settings.window_scale)
        .unwrap_or(FIRST_LAUNCH_WINDOW_SCALE)
        .clamp(PANEL_SCALE_MIN, PANEL_SCALE_MAX);
    let mut chrome = PanelChromeState {
        input_modes_expanded: true,
        ..Default::default()
    };
    if let Some(settings) = settings.as_ref() {
        apply_display_settings(&mut chrome, settings);
    }
    let width = DEFAULT_PANEL_INNER_WIDTH * scale as f64;
    let height =
        WgpuCandidateRenderer::new(width as f32, 1.0).preferred_input_panel_height(&chrome) as f64;
    let panel_dispatch = current_panel_companion_dispatch();
    let attrs = WindowAttributes::default()
        .with_title(panel_dispatch.default_panel_title())
        .with_window_icon(panel_window_icon())
        .with_transparent(true)
        .with_inner_size(LogicalSize::new(
            width,
            height.clamp(MIN_PANEL_INNER_HEIGHT, MAX_PANEL_INNER_HEIGHT),
        ))
        .with_max_inner_size(LogicalSize::new(
            MAX_PANEL_INNER_WIDTH,
            MAX_PANEL_INNER_HEIGHT,
        ))
        .with_min_inner_size(LogicalSize::new(
            MIN_PANEL_INNER_WIDTH,
            MIN_PANEL_INNER_HEIGHT,
        ))
        .with_resizable(true);
    decorate_main_window_attributes(attrs)
}

fn settings_window_attributes() -> WindowAttributes {
    let attrs = WindowAttributes::default()
        .with_title("Suzaku Panel Settings")
        .with_window_icon(panel_window_icon())
        .with_inner_size(LogicalSize::new(520.0, 340.0))
        .with_visible(false)
        .with_resizable(true);
    decorate_settings_window_attributes(attrs)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PanelWindowKind {
    Main,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DockEdge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Clone, Debug)]
struct CommitAttempt {
    selected_index: usize,
    text: String,
    timestamp: Instant,
}

struct PanelApp {
    panel: Option<PanelState>,
    settings: Option<PanelState>,
    next_frame_at: Option<Instant>,
    event_proxy: EventLoopProxy<PanelUserEvent>,
    instance: Option<SingleInstanceGuard>,
    tray: Option<SystemTray>,
    panel_visible: bool,
    prediction_settings_sync: prediction_settings::PredictionSettingsSync,
    last_native_ime_settings: Option<suzaku_map::ime::settings::ImeSettings>,
    native_sync: Option<native_sync::NativeSync>,
    native_auto_shown: bool,
    native_hidden_context: Option<(String, u64)>,
}

impl PanelApp {
    fn request_quit(&self, event_loop: &ActiveEventLoop) {
        if !self.tray.as_ref().is_some_and(SystemTray::request_quit) {
            event_loop.exit();
        }
    }

    fn new(
        event_proxy: EventLoopProxy<PanelUserEvent>,
        instance: Option<SingleInstanceGuard>,
    ) -> Self {
        Self {
            panel: None,
            settings: None,
            next_frame_at: None,
            event_proxy,
            instance,
            tray: None,
            panel_visible: false,
            prediction_settings_sync: Default::default(),
            last_native_ime_settings: None,
            native_sync: None,
            native_auto_shown: false,
            native_hidden_context: None,
        }
    }

    fn set_panel_visible(&mut self, visible: bool) {
        self.native_auto_shown = false;
        let Some(panel) = self.panel.as_mut() else {
            return;
        };

        panel.close_requested = false;
        if visible {
            self.native_hidden_context = None;
        } else if let Some(frame) = panel.native.frame.as_ref().filter(|f| f.visible()) {
            self.native_hidden_context = Some((frame.host.clone(), frame.context));
        }
        if !visible {
            panel.finish_text_editing();
        }
        panel.window.set_visible(visible);
        self.panel_visible = visible;
        if visible {
            panel.constrain_expanded_window_position();
            panel.window.request_redraw();
        } else {
            panel.clear_pointer_hover();
            panel.chrome.settings_open = false;
            self.settings = None;
            self.next_frame_at = None;
        }
        if let Some(tray) = self.tray.as_ref() {
            tray.set_panel_visible(visible);
        }
    }

    fn apply_native_settings(
        &mut self,
        ime_settings: suzaku_map::ime::settings::ImeSettings,
        write_finished: bool,
    ) {
        let language = ime_settings.language;
        let configuration_changed = self.last_native_ime_settings.as_ref() != Some(&ime_settings);
        self.last_native_ime_settings = Some(ime_settings.clone());
        if let Some(panel) = self.panel.as_mut() {
            let previous = (panel.chrome.llm_enabled, panel.chrome.llm_temperature);
            if write_finished {
                self.prediction_settings_sync
                    .finish(Some(&ime_settings), &mut panel.chrome);
            } else {
                self.prediction_settings_sync
                    .observe(&ime_settings, &mut panel.chrome);
            }
            if configuration_changed
                || panel.engine.snapshot().active_language != language.id()
                || previous != (panel.chrome.llm_enabled, panel.chrome.llm_temperature)
            {
                panel.sync_manual_seed_base();
                panel.engine.set_language(language.id());
                panel.reconfigure_model_provider();
                panel.persist_display_settings();
                if self.panel_visible {
                    panel.window.request_redraw();
                }
            }
            if let Some(settings) = self.settings.as_mut() {
                settings.chrome.llm_enabled = panel.chrome.llm_enabled;
                settings.chrome.llm_temperature = panel.chrome.llm_temperature;
                settings.window.request_redraw();
            }
        }
    }

    fn finish_prediction_settings(
        &mut self,
        settings: Option<suzaku_map::ime::settings::ImeSettings>,
    ) {
        if let Some(settings) = settings.or_else(|| self.last_native_ime_settings.clone()) {
            self.apply_native_settings(settings, true);
        }
    }

    fn push_prediction_settings(&mut self) {
        if let (Some(panel), Some(tray)) = (&self.panel, &self.tray) {
            if let Some(patch) = self.prediction_settings_sync.next_patch(&panel.chrome) {
                if !tray.set_prediction_settings(patch) {
                    self.finish_prediction_settings(None);
                }
            }
        }
    }

    fn reset_panel_position(&mut self, event_loop: &ActiveEventLoop) {
        let Some(panel) = self.panel.as_mut() else {
            return;
        };

        panel.close_requested = false;
        panel.chrome.settings_open = false;
        self.settings = None;
        finish_main_window_creation(&panel.window, event_loop);
        panel.note_expanded_window_position();
        panel.window.set_visible(true);
        panel.window.request_redraw();
        self.panel_visible = true;
        if let Some(tray) = self.tray.as_ref() {
            tray.set_panel_visible(true);
        }
    }

    fn open_settings(&mut self, event_loop: &ActiveEventLoop) {
        self.set_panel_visible(true);
        let Some(panel) = self.panel.as_mut() else {
            return;
        };

        if panel.chrome.compact_mode {
            panel.apply_compact_mode(false);
        }
        panel.chrome.settings_open = true;
        panel.window.request_redraw();
        let chrome = panel.chrome.clone();
        if let Some(settings) = self.settings.as_mut() {
            if settings.chrome != chrome {
                settings.chrome = chrome;
                settings.window.request_redraw();
            }
            settings.window.set_visible(true);
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(settings_window_attributes())
                .expect("create settings window"),
        );
        let settings = pollster::block_on(PanelState::new_settings(window, chrome))
            .expect("initialize settings window");
        finish_settings_window_creation(&settings.window, &panel.window, event_loop);
        settings.window.request_redraw();
        self.settings = Some(settings);
    }
}

#[derive(Default)]
struct PanelInteractionState {
    hovered_interaction: Option<suzaku_map::ime::gpu::InteractionKind>,
    pointer_cursor: winit::window::CursorIcon,
    tooltip: controller_hints::HoverTooltipState,
    pressed_interaction: Option<suzaku_map::ime::gpu::InteractionKind>,
    press_target_rect: Option<[f32; 4]>,
    press_start_cursor: Option<(f32, f32)>,
    press_start_instant: Option<Instant>,
    touch_tap_pending: bool,
    touch_start_position: Option<(f32, f32)>,
    handwriting_dragging: bool,
    compact_hovered: bool,
    panel_dragging: bool,
    panel_drag_moved: bool,
    panel_drag_start_cursor: Option<(f32, f32)>,
    panel_drag_start_instant: Option<Instant>,
    panel_drag_start_window_pos: Option<PhysicalPosition<i32>>,
    scale_dragging: bool,
    scale_drag_start_cursor_x: Option<f32>,
    scale_drag_start_scale: f32,
    last_input_was_touch: bool,
    sentence_candidate_scroll_index: Option<usize>,
    sentence_candidate_scroll_started_at: Option<Instant>,
    next_token_candidate_scroll_index: Option<usize>,
    next_token_candidate_scroll_started_at: Option<Instant>,
    handwriting_candidate_scroll_index: Option<usize>,
    handwriting_candidate_scroll_started_at: Option<Instant>,
    handwriting_last_sample: Option<Instant>,
    handwriting_last_sample_position: Option<[f32; 2]>,
    settings_option_text_scroll_target: Option<InteractionKind>,
    settings_option_text_scroll_started_at: Option<Instant>,
    settings_scroll_dragging: bool,
    settings_scroll_drag_start_y: Option<f32>,
    settings_scroll_drag_start_offset: f32,
    settings_scroll_drag_range: f32,
    settings_scroll_track_rect: Option<[f32; 4]>,
    settings_scroll_handle_rect: Option<[f32; 4]>,
    settings_scroll_max_offset: f32,
    settings_scroll_visible_height: f32,
    settings_scroll_content_height: f32,
}

impl ApplicationHandler<PanelUserEvent> for PanelApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.panel.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(panel_window_attributes())
                .expect("create panel window"),
        );
        let mut state =
            pollster::block_on(PanelState::new(window)).expect("initialize panel state");
        finish_main_window_creation(&state.window, event_loop);
        self.native_sync = native_sync::start(self.event_proxy.clone());
        state.native.sender = self.native_sync.as_ref().map(|sync| sync.sender.clone());
        state.note_expanded_window_position();
        state.window.request_redraw();
        self.panel = Some(state);
        self.panel_visible = true;
        if self.tray.is_none() {
            let theme = self
                .panel
                .as_ref()
                .expect("initialized panel")
                .chrome
                .theme_preset;
            self.tray = start_system_tray(self.event_proxy.clone(), true, theme);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(panel) = self.panel.as_mut() else {
            return;
        };

        if let Some(settings) = self.settings.as_mut()
            && settings.window.id() == window_id
        {
            match event {
                WindowEvent::CloseRequested => {
                    panel.chrome.settings_open = false;
                    self.settings = None;
                    panel.window.request_redraw();
                }
                _ => {
                    handle_panel_window_event(settings, event_loop, event, false);
                    panel.chrome.settings_open = settings.chrome.settings_open;
                    if panel.adopt_settings_from(&settings.chrome) {
                        panel.window.request_redraw();
                    }
                    if panel.chrome.settings_open {
                        if settings.chrome != panel.chrome {
                            settings.chrome = panel.chrome.clone();
                            settings.window.request_redraw();
                        }
                    } else {
                        self.settings = None;
                    }
                }
            }
            return;
        }

        if panel.window.id() != window_id {
            return;
        }

        handle_panel_window_event(panel, event_loop, event, true);
        if panel.quit_requested {
            panel.quit_requested = false;
            self.request_quit(event_loop);
            return;
        }
        if panel.close_requested {
            panel.close_requested = false;
            if self.tray.as_ref().is_some_and(SystemTray::has_icon) {
                self.native_auto_shown = false;
                if let Some(frame) = panel.native.frame.as_ref().filter(|f| f.visible()) {
                    self.native_hidden_context = Some((frame.host.clone(), frame.context));
                }
                panel.finish_text_editing();
                panel.clear_pointer_hover();
                panel.chrome.settings_open = false;
                panel.window.set_visible(false);
                self.settings = None;
                self.next_frame_at = None;
                self.panel_visible = false;
                if let Some(tray) = self.tray.as_ref() {
                    tray.set_panel_visible(false);
                }
            } else {
                self.request_quit(event_loop);
            }
            return;
        }

        if panel.chrome.compact_mode {
            panel.chrome.settings_open = false;
            self.settings = None;
        } else if panel.chrome.settings_open && self.settings.is_none() {
            let window = Arc::new(
                event_loop
                    .create_window(settings_window_attributes())
                    .expect("create settings window"),
            );
            let settings =
                pollster::block_on(PanelState::new_settings(window, panel.chrome.clone()))
                    .expect("initialize settings window");
            finish_settings_window_creation(&settings.window, &panel.window, event_loop);
            settings.window.request_redraw();
            self.settings = Some(settings);
        } else if !panel.chrome.settings_open {
            self.settings = None;
        }
        if let Some(settings) = self.settings.as_mut()
            && settings.chrome != panel.chrome
        {
            settings.chrome = panel.chrome.clone();
            settings.window.request_redraw();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: PanelUserEvent) {
        match event {
            PanelUserEvent::ShowPanel => self.set_panel_visible(true),
            PanelUserEvent::HidePanel => self.set_panel_visible(false),
            PanelUserEvent::OpenSettings => self.open_settings(event_loop),
            PanelUserEvent::ResetPanelPosition => self.reset_panel_position(event_loop),
            PanelUserEvent::InputMethodSettingsChanged(ime_settings) => {
                self.apply_native_settings(ime_settings, false);
            }
            PanelUserEvent::InputMethodError(error) => {
                if let Some(panel) = self.panel.as_mut() {
                    panel.last_commit_feedback = Some(error);
                    panel.commit_feedback_ticks = 180;
                    panel.window.request_redraw();
                }
                self.set_panel_visible(true);
            }
            PanelUserEvent::PredictionSettingsApplied(settings) => {
                self.finish_prediction_settings(settings)
            }
            PanelUserEvent::NativeCompositionReady => {
                if let Some(update) = self
                    .native_sync
                    .as_ref()
                    .and_then(|sync| sync.take_update())
                {
                    if let Some(panel) = self.panel.as_mut() {
                        let was_visible = panel
                            .native
                            .frame
                            .as_ref()
                            .is_some_and(|frame| frame.visible());
                        panel.receive_native_frame(update);
                        let visible = panel.native.showing
                            && panel
                                .native
                                .frame
                                .as_ref()
                                .is_some_and(|frame| frame.visible());
                        let manually_hidden = panel.native.frame.as_ref().is_some_and(|frame| {
                            self.native_hidden_context.as_ref()
                                == Some(&(frame.host.clone(), frame.context))
                        });
                        let can_show = !manually_hidden
                            && panel.runs_without_window_focus
                            && !panel.is_focused
                            && !panel.chrome.settings_open;
                        if visible && !was_visible && !self.panel_visible && can_show {
                            self.set_panel_visible(true);
                            self.native_auto_shown = true;
                        } else if !visible && self.native_auto_shown {
                            self.set_panel_visible(false);
                        }
                    }
                }
            }
            PanelUserEvent::NativeActionFinished {
                host,
                revision,
                result,
            } => {
                if let Some(panel) = self.panel.as_mut() {
                    panel.native_action_finished(host, revision, result);
                }
            }
            PanelUserEvent::Quit => event_loop.exit(),
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.native_sync = None;
        if let Some(panel) = self.panel.as_mut() {
            panel.finish_text_editing();
        }
        if let Some(tray) = self.tray.take() {
            tray.shutdown();
        }
        if let Some(instance) = self.instance.take() {
            instance.shutdown();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        self.push_prediction_settings();
        if let (Some(tray), Some(panel)) = (self.tray.as_mut(), self.panel.as_ref()) {
            tray.set_theme(panel.chrome.theme_preset);
        }
        let mut tooltip_deadline = None;
        for state in self
            .panel
            .iter_mut()
            .filter(|_| self.panel_visible)
            .chain(self.settings.iter_mut())
        {
            if !state.native.showing && state.engine.poll_prediction() {
                state.refresh_composition_candidates();
                state.window.request_redraw();
            }
            if !state.native.showing && state.engine.prediction_pending() {
                tooltip_deadline = Some(now + Duration::from_millis(30));
            }
            state.fit_window_to_content();
            if state.interaction.tooltip.advance(now) {
                state.window.request_redraw();
            }
            if let Some(deadline) = state.interaction.tooltip.deadline() {
                tooltip_deadline = Some(
                    tooltip_deadline.map_or(deadline, |earlier: Instant| earlier.min(deadline)),
                );
            }
        }
        let panel_needs_frame = self.panel_visible
            && self
                .panel
                .as_ref()
                .is_some_and(PanelState::needs_periodic_frame);
        let settings_needs_frame = self
            .settings
            .as_ref()
            .is_some_and(PanelState::needs_periodic_frame);
        if !panel_needs_frame && !settings_needs_frame {
            self.next_frame_at = None;
            event_loop.set_control_flow(
                tooltip_deadline.map_or(ControlFlow::Wait, ControlFlow::WaitUntil),
            );
            return;
        }

        let (should_advance, next_frame_at) = periodic_frame_schedule(now, self.next_frame_at);
        if should_advance
            && panel_needs_frame
            && let Some(panel) = self.panel.as_mut()
        {
            panel.advance_periodic_frame();
            panel.window.request_redraw();
        }
        if should_advance
            && settings_needs_frame
            && let Some(settings) = self.settings.as_mut()
        {
            settings.advance_periodic_frame();
            settings.window.request_redraw();
        }
        self.next_frame_at = Some(next_frame_at);
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            tooltip_deadline.map_or(next_frame_at, |deadline| deadline.min(next_frame_at)),
        ));
    }
}

struct PanelState {
    kind: PanelWindowKind,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    shape_pipeline: wgpu::RenderPipeline,
    text_pipeline: wgpu::RenderPipeline,
    shape_vertex_buffer: wgpu::Buffer,
    shape_vertex_capacity: usize,
    text_vertex_buffer: wgpu::Buffer,
    text_vertex_capacity: usize,
    font_atlas: FontAtlas,
    size: winit::dpi::PhysicalSize<u32>,
    renderer: WgpuCandidateRenderer,
    engine: XRTabletImeEngine,
    chrome: PanelChromeState,
    voice: VoiceInputController,
    cursor_position: Option<(f32, f32)>,
    modifiers: ModifiersState,
    interaction: PanelInteractionState,
    voice_progress: voice::VoiceTranscriptProgress,
    handwriting_generation: u64,
    last_handwriting_summary: Option<String>,
    last_commit_feedback: Option<String>,
    commit_feedback_ticks: u8,
    completion_history: suzaku_map::panel_support::CompletionHistory,
    next_token_completions: Vec<suzaku_map::panel_support::NextTokenCompletion>,
    expanded_window_size: Option<LogicalSize<f64>>,
    expanded_window_base_size: Option<LogicalSize<f64>>,
    expanded_window_pos: Option<PhysicalPosition<i32>>,
    expanded_window_decorations: bool,
    window_scale: f32,
    window_resize_state: windowing::WindowResizeState,
    compact_dock_edge: Option<DockEdge>,
    last_compact_toggle: Option<Instant>,
    is_focused: bool,
    runs_without_window_focus: bool,
    text_focus: suzaku_map::platform::panel_text_focus::PanelTextFocus,
    text_input: keyboard::TextInputState,
    native: native_sync::NativeView,
    input_dispatch_guard: bool,
    last_commit_attempt: Option<CommitAttempt>,
    last_interaction_action: Option<(suzaku_map::ime::gpu::InteractionKind, Instant)>,
    last_scene: Option<suzaku_map::ime::gpu::RenderScene>,
    last_window_title: String,
    close_requested: bool,
    quit_requested: bool,
}

impl PanelState {
    async fn new(window: Arc<Window>) -> Result<Self, Box<dyn Error>> {
        Self::new_with_kind(window, PanelWindowKind::Main, None).await
    }

    async fn new_settings(
        window: Arc<Window>,
        chrome: PanelChromeState,
    ) -> Result<Self, Box<dyn Error>> {
        Self::new_with_kind(window, PanelWindowKind::Settings, Some(chrome)).await
    }

    async fn new_with_kind(
        window: Arc<Window>,
        kind: PanelWindowKind,
        initial_chrome: Option<PanelChromeState>,
    ) -> Result<Self, Box<dyn Error>> {
        let size = window.inner_size();
        let persisted_settings = load_display_settings();
        let initial_window_scale = if kind == PanelWindowKind::Main {
            persisted_settings
                .as_ref()
                .map(|settings| settings.window_scale)
                .unwrap_or(FIRST_LAUNCH_WINDOW_SCALE)
        } else {
            1.0
        };
        let initial_base_size = if kind == PanelWindowKind::Main {
            let logical_size = size.to_logical::<f64>(window.scale_factor());
            Some(LogicalSize::new(
                logical_size.width / initial_window_scale as f64,
                logical_size.height / initial_window_scale as f64,
            ))
        } else {
            None
        };
        let is_focused = window.has_focus();
        let runs_without_window_focus =
            kind == PanelWindowKind::Main && main_window_runs_without_focus();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("suzaku-panel-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                experimental_features: Default::default(),
                trace: Default::default(),
            })
            .await?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            desired_maximum_frame_latency: 2,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: if kind == PanelWindowKind::Main
                && caps
                    .alpha_modes
                    .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
            {
                wgpu::CompositeAlphaMode::PreMultiplied
            } else {
                caps.alpha_modes[0]
            },
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let shape_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("suzaku-panel-shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let text_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("suzaku-panel-text-shader"),
            source: wgpu::ShaderSource::Wgsl(TEXT_SHADER.into()),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("suzaku-panel-layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let shape_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("suzaku-panel-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shape_shader,
                entry_point: Some("vs_main"),
                buffers: &[PanelVertex::desc()],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shape_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview: None,
            cache: None,
        });

        let text_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("suzaku-font-atlas-bind-group-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let voice: VoiceInputController = VoiceInputController::new();
        let mut chrome = initial_chrome.unwrap_or(PanelChromeState {
            seed_text: String::new(),
            compact_mode: false,
            input_modes_expanded: true,
            active_input_mode: InputMode::VirtualKeyboard,
            input_focused: is_focused,
            caret_index: 0,
            keyboard_shifted: false,
            keyboard_numeric: false,
            settings_open: false,
            text_scale: DisplayTextScale::Medium,
            candidate_density: CandidateDensity::Cozy,
            preview_style: PreviewStyle::Compact,
            font_face: FontFaceChoice::Auto,
            text_spacing: TextSpacing::Normal,
            text_smoothing: TextSmoothing::Smooth,
            theme_preset: suzaku_map::ime::gpu::ThemePreset::Suzaku,
            voice_state: VoiceCaptureState::Idle,
            voice_permission: VoicePermissionState::Unknown,
            voice_backend_label: "Unknown Voice Host".to_string(),
            voice_supports_live_capture: false,
            hovered_interaction: None,
            pressed_interaction: None,
            voice_transcript: String::new(),
            voice_visual_phase: 0,
            voice_auto_insert: true,
            llm_enabled: false,
            llm_model: LlmModelPreset::Configured,
            llm_temperature: LlmTemperaturePreset::Balanced,
            pointer_tap_slop_tenths: 100,
            pointer_tap_max_ms: 420,
            pointer_target_slop_tenths: 50,
            window_scale: 1.0,
            composed_tokens: Vec::new(),
            next_token_candidates: Vec::new(),
            sentence_candidates: Vec::new(),
            sentence_candidate_source_indices: Vec::new(),
            handwriting_strokes: Vec::new(),
            handwriting_candidates: Vec::new(),
            handwriting_hint: "Draw a seed word with mouse or touch.".to_string(),
            settings_scroll_offset: 0.0,
            settings_search_query: String::new(),
            settings_search_focused: false,
            settings_collapsed_sections: Vec::new(),
        });
        if let Some(saved) = persisted_settings.as_ref() {
            apply_display_settings(&mut chrome, saved);
        }
        apply_pointer_stability_env_overrides(&mut chrome);

        if let Some(bridge) = voice.bridge.as_ref() {
            chrome.voice_backend_label = bridge.source_label().to_string();
            chrome.voice_supports_live_capture = bridge.supports_live_capture();
            chrome.voice_permission = bridge.permission_state();
        } else {
            chrome.voice_backend_label = "Fallback Samples".to_string();
            chrome.voice_supports_live_capture = false;
            chrome.voice_permission = VoicePermissionState::Unavailable;
        }

        let font_atlas = create_font_atlas(
            &device,
            &queue,
            &text_bind_group_layout,
            chrome.font_face,
            chrome.text_smoothing,
            chrome.window_scale,
        );
        let text_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("suzaku-panel-text-layout"),
            bind_group_layouts: &[&text_bind_group_layout],
            push_constant_ranges: &[],
        });
        let text_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("suzaku-panel-text-pipeline"),
            layout: Some(&text_layout),
            vertex: wgpu::VertexState {
                module: &text_shader,
                entry_point: Some("vs_main"),
                buffers: &[TextVertex::desc()],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &text_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview: None,
            cache: None,
        });

        let mut engine = XRTabletImeEngine::new(EngineConfig::default());
        engine.set_source(InputSource::GazeDwell);
        engine.update_signal(SignalState {
            pointer_precision: 0.42,
            gaze_stability: 0.50,
            host_intent_weight: 0.80,
            source_confidence: 0.70,
        });
        engine.seed(&chrome.seed_text);

        let shape_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("suzaku-panel-shape-vertices"),
            size: MIN_STREAMING_VERTEX_BUFFER_CAPACITY as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let text_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("suzaku-panel-text-vertices"),
            size: MIN_STREAMING_VERTEX_BUFFER_CAPACITY as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut state = Self {
            kind,
            window,
            surface,
            device,
            queue,
            config: config.clone(),
            shape_pipeline,
            text_pipeline,
            shape_vertex_buffer,
            shape_vertex_capacity: MIN_STREAMING_VERTEX_BUFFER_CAPACITY,
            text_vertex_buffer,
            text_vertex_capacity: MIN_STREAMING_VERTEX_BUFFER_CAPACITY,
            font_atlas,
            size,
            renderer: WgpuCandidateRenderer::new(config.width as f32, config.height as f32),
            engine,
            chrome,
            voice,
            cursor_position: None,
            modifiers: ModifiersState::default(),
            interaction: PanelInteractionState {
                scale_drag_start_scale: initial_window_scale,
                ..PanelInteractionState::default()
            },
            voice_progress: Default::default(),
            handwriting_generation: 0,
            last_handwriting_summary: None,
            last_commit_feedback: None,
            commit_feedback_ticks: 0,
            completion_history: Default::default(),
            next_token_completions: Vec::new(),
            expanded_window_size: None,
            expanded_window_base_size: initial_base_size,
            expanded_window_pos: None,
            expanded_window_decorations: true,
            window_scale: initial_window_scale,
            window_resize_state: Default::default(),
            compact_dock_edge: None,
            last_compact_toggle: None,
            is_focused,
            runs_without_window_focus,
            text_focus: Default::default(),
            text_input: Default::default(),
            native: Default::default(),
            input_dispatch_guard: false,
            last_commit_attempt: None,
            last_interaction_action: None,
            last_scene: None,
            last_window_title: String::new(),
            close_requested: false,
            quit_requested: false,
        };
        if kind == PanelWindowKind::Main {
            if (initial_window_scale - 1.0).abs() > f32::EPSILON {
                state.set_window_scale(initial_window_scale);
            } else {
                state.window_scale = 1.0;
            }
            state.reconfigure_model_provider();
        }

        state.sync_text_input_state();
        Ok(state)
    }
}

fn apply_pointer_stability_env_overrides(chrome: &mut PanelChromeState) {
    if let Ok(raw_tap_slop_px) = std::env::var("SUZAKU_POINTER_TAP_SLOP_PX") {
        if let Ok(px) = raw_tap_slop_px.parse::<f32>() {
            let clamped = (px * 10.0).round().clamp(20.0, 120.0) as u16;
            chrome.pointer_tap_slop_tenths = clamped;
        }
    }
    if let Ok(raw_tap_max_ms) = std::env::var("SUZAKU_POINTER_TAP_MAX_MS") {
        if let Ok(ms) = raw_tap_max_ms.parse::<u16>() {
            chrome.pointer_tap_max_ms = ms;
        }
    }
    if let Ok(raw_target_slop_px) = std::env::var("SUZAKU_POINTER_TARGET_SLOP_PX") {
        if let Ok(px) = raw_target_slop_px.parse::<f32>() {
            let clamped = (px * 10.0).round().clamp(10.0, 120.0) as u16;
            chrome.pointer_target_slop_tenths = clamped;
        }
    }

    normalize_pointer_stability_settings(chrome);
}
