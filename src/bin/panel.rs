#![cfg(feature = "gpu")]

use std::error::Error;
use std::sync::Arc;
use std::time::Instant;

#[path = "panel/app_state.rs"]
mod app_state;
#[path = "panel/composition.rs"]
mod composition;
#[path = "panel/controller.rs"]
mod controller;
#[path = "panel/controller_hints.rs"]
mod controller_hints;
#[path = "panel/handwriting.rs"]
mod handwriting;
#[path = "panel/helpers.rs"]
mod helpers;
#[path = "panel/input.rs"]
mod input;
#[path = "panel/render.rs"]
mod render;
#[cfg(test)]
#[path = "panel/tests.rs"]
mod tests;
#[path = "panel/voice.rs"]
mod voice;
#[path = "panel/windowing.rs"]
mod windowing;

use crate::app_state::{
    VoiceInputController, FIRST_LAUNCH_WINDOW_SCALE, apply_display_settings,
    load_display_settings, normalize_pointer_stability_settings,
};
use crate::input::handle_panel_window_event;
use crate::render::{FontAtlas, PanelVertex, TextVertex, create_font_atlas};
use suzaku_map::ime::gpu::{
    CandidateDensity, DisplayTextScale, FontFaceChoice, InputMode, LlmModelPreset,
    LlmTemperaturePreset, PANEL_SCALE_MAX, PANEL_SCALE_MIN, PANEL_SCALE_STEP, PanelChromeState,
    PreviewStyle, TextSmoothing, TextSpacing, VoiceCaptureState, VoicePermissionState,
    WgpuCandidateRenderer,
};
use suzaku_map::ime::{EngineConfig, InputSource, SignalState, XRTabletImeEngine};
use suzaku_map::platform::gpu_host::{
    configure_event_loop_builder, decorate_main_window_attributes,
    decorate_settings_window_attributes,
};
use suzaku_map::platform::panel_companion_dispatch::current_panel_companion_dispatch;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::dpi::PhysicalPosition;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowAttributes, WindowId};

const DEFAULT_PANEL_INNER_WIDTH: f64 = 900.0;
const DEFAULT_PANEL_INNER_HEIGHT: f64 = 520.0;
const MIN_PANEL_INNER_WIDTH: f64 = 420.0;
const MIN_PANEL_INNER_HEIGHT: f64 = 300.0;
const COMPACT_PANEL_INNER_WIDTH: f64 = 92.0;
const COMPACT_PANEL_INNER_HEIGHT: f64 = 92.0;

const SHADER: &str = r#"
struct VsIn {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    out.position = vec4<f32>(input.position, 0.0, 1.0);
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    return input.color;
}
"#;

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
    let event_loop = build_event_loop()?;
    let mut app = PanelApp::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}

fn build_event_loop() -> Result<EventLoop<()>, winit::error::EventLoopError> {
    let mut builder = EventLoop::builder();
    configure_event_loop_builder(&mut builder);
    builder.build()
}

fn panel_window_attributes() -> WindowAttributes {
    let scale = load_display_settings()
        .as_ref()
        .map(|settings| settings.window_scale)
        .unwrap_or(FIRST_LAUNCH_WINDOW_SCALE)
        .clamp(PANEL_SCALE_MIN, PANEL_SCALE_MAX);
    let panel_dispatch = current_panel_companion_dispatch();
    let attrs = WindowAttributes::default()
        .with_title(panel_dispatch.default_panel_title())
        .with_inner_size(LogicalSize::new(
            DEFAULT_PANEL_INNER_WIDTH * scale as f64,
            DEFAULT_PANEL_INNER_HEIGHT * scale as f64,
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
        .with_inner_size(LogicalSize::new(520.0, 340.0))
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

#[derive(Default)]
struct PanelApp {
    panel: Option<PanelState>,
    settings: Option<PanelState>,
}

#[derive(Default)]
struct PanelInteractionState {
    hovered_interaction: Option<suzaku_map::ime::gpu::InteractionKind>,
    pressed_interaction: Option<suzaku_map::ime::gpu::InteractionKind>,
    press_target_rect: Option<[f32; 4]>,
    press_start_cursor: Option<(f32, f32)>,
    press_start_instant: Option<Instant>,
    touch_tap_pending: bool,
    touch_start_position: Option<(f32, f32)>,
    handwriting_dragging: bool,
    compact_hovered: bool,
    compact_dragging: bool,
    compact_drag_moved: bool,
    compact_drag_start_cursor: Option<(f32, f32)>,
    compact_drag_start_instant: Option<Instant>,
    compact_drag_start_window_pos: Option<PhysicalPosition<i32>>,
    scale_dragging: bool,
    scale_drag_start_cursor_x: Option<f32>,
    scale_drag_start_scale: f32,
    last_input_was_touch: bool,
}

impl ApplicationHandler for PanelApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.panel.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(panel_window_attributes())
                .expect("create panel window"),
        );
        let state = pollster::block_on(PanelState::new(window)).expect("initialize panel state");
        self.panel = Some(state);
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

        if let Some(settings) = self.settings.as_mut() {
            if settings.window.id() == window_id {
                match event {
                    WindowEvent::CloseRequested => {
                        panel.chrome.settings_open = false;
                        self.settings = None;
                    }
                    _ => {
                        handle_panel_window_event(settings, event_loop, event, false);
                        panel.chrome.settings_open = settings.chrome.settings_open;
                        panel.adopt_settings_from(&settings.chrome);
                        panel.window.request_redraw();
                        if panel.chrome.settings_open {
                            settings.chrome = panel.chrome.clone();
                            settings.window.request_redraw();
                        } else {
                            self.settings = None;
                        }
                    }
                }
                return;
            }
        }

        if panel.window.id() != window_id {
            return;
        }

        let should_exit = matches!(event, WindowEvent::CloseRequested);
        handle_panel_window_event(panel, event_loop, event, true);
        if should_exit {
            event_loop.exit();
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
            self.settings = Some(settings);
        } else if !panel.chrome.settings_open {
            self.settings = None;
        }
        if let Some(settings) = self.settings.as_mut() {
            settings.chrome = panel.chrome.clone();
            settings.window.request_redraw();
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(panel) = self.panel.as_mut() {
            if panel.is_focused {
                panel.poll_voice_bridge();
            }
            if panel.commit_feedback_ticks > 0 {
                panel.commit_feedback_ticks -= 1;
                if panel.commit_feedback_ticks == 0 {
                    panel.last_commit_feedback = None;
                }
            }
            if panel.chrome.active_input_mode == InputMode::Dictation
                && panel.chrome.voice_state == VoiceCaptureState::Listening
            {
                panel.chrome.voice_visual_phase =
                    panel.chrome.voice_visual_phase.wrapping_add(1) % 24;
            } else {
                panel.chrome.voice_visual_phase = 0;
            }
            panel.window.request_redraw();
            if let Some(settings) = self.settings.as_mut() {
                settings.chrome = panel.chrome.clone();
                settings.window.request_redraw();
            }
        }
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
    font_atlas: FontAtlas,
    size: winit::dpi::PhysicalSize<u32>,
    renderer: WgpuCandidateRenderer,
    engine: XRTabletImeEngine,
    chrome: PanelChromeState,
    voice: VoiceInputController,
    cursor_position: Option<(f32, f32)>,
    modifiers: ModifiersState,
    interaction: PanelInteractionState,
    voice_stability_ticks: u8,
    last_polled_voice_transcript: String,
    last_handwriting_summary: Option<String>,
    last_commit_feedback: Option<String>,
    commit_feedback_ticks: u8,
    composition_base_seed: String,
    selected_next_tokens: Vec<String>,
    expanded_window_size: Option<LogicalSize<f64>>,
    expanded_window_base_size: Option<LogicalSize<f64>>,
    expanded_window_pos: Option<PhysicalPosition<i32>>,
    window_scale: f32,
    compact_dock_edge: Option<DockEdge>,
    last_compact_toggle: Option<Instant>,
    is_focused: bool,
    input_dispatch_guard: bool,
    last_commit_attempt: Option<CommitAttempt>,
    last_interaction_action: Option<(suzaku_map::ime::gpu::InteractionKind, Instant)>,
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
            Some(size.to_logical::<f64>(window.scale_factor()))
        } else {
            None
        };
        let is_focused = window.has_focus();
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
            alpha_mode: caps.alpha_modes[0],
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
            seed_text: "ni hao".into(),
            compact_mode: false,
            input_modes_expanded: true,
            active_input_mode: InputMode::VirtualKeyboard,
            input_focused: true,
            caret_index: "ni hao".chars().count(),
            keyboard_shifted: false,
            keyboard_numeric: false,
            settings_open: false,
            text_scale: DisplayTextScale::Medium,
            candidate_density: CandidateDensity::Cozy,
            preview_style: PreviewStyle::Compact,
            font_face: FontFaceChoice::Monaco,
            text_spacing: TextSpacing::Normal,
            text_smoothing: TextSmoothing::Sharp,
            theme_preset: suzaku_map::ime::gpu::ThemePreset::Daylight,
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
            llm_model: LlmModelPreset::Llama32_3b,
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
        engine.seed("ni hao");

        let composition_base_seed = chrome.seed_text.clone();
        let mut state = Self {
            kind,
            window,
            surface,
            device,
            queue,
            config: config.clone(),
            shape_pipeline,
            text_pipeline,
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
            voice_stability_ticks: 0,
            last_polled_voice_transcript: String::new(),
            last_handwriting_summary: None,
            last_commit_feedback: None,
            commit_feedback_ticks: 0,
            composition_base_seed,
            selected_next_tokens: Vec::new(),
            expanded_window_size: None,
            expanded_window_base_size: initial_base_size,
            expanded_window_pos: None,
            window_scale: initial_window_scale,
            compact_dock_edge: None,
            last_compact_toggle: None,
            is_focused,
            input_dispatch_guard: false,
            last_commit_attempt: None,
            last_interaction_action: None,
        };
        if kind == PanelWindowKind::Main {
            if (initial_window_scale - 1.0).abs() > f32::EPSILON {
                state.set_window_scale(initial_window_scale);
            } else {
                state.window_scale = 1.0;
            }
            state.reconfigure_llama_plugin();
        }

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
