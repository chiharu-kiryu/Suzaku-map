#![cfg(feature = "gpu")]

use std::error::Error;
use std::sync::Arc;

#[path = "panel/app_state.rs"]
mod app_state;
#[path = "panel/composition.rs"]
mod composition;
#[path = "panel/controller.rs"]
mod controller;
#[path = "panel/handwriting.rs"]
mod handwriting;
#[path = "panel/input.rs"]
mod input;
#[path = "panel/render.rs"]
mod render;
#[path = "panel/voice.rs"]
mod voice;

use crate::app_state::{VoiceInputController, apply_display_settings, load_display_settings};
use crate::input::handle_panel_window_event;
use crate::render::{FontAtlas, PanelVertex, TextVertex, create_font_atlas};
use suzaku_map::ime::gpu::{
    CandidateDensity, CandidateQuad, DisplayTextScale, FontFaceChoice, InputMode, LlmModelPreset,
    LlmTemperaturePreset, PanelChromeState, PreviewStyle, RenderScene, TextSmoothing, TextSpacing,
    VoiceCaptureState, VoicePermissionState, WgpuCandidateRenderer,
};
use suzaku_map::ime::{EngineConfig, InputSource, SignalState, XRTabletImeEngine};
use suzaku_map::platform::gpu_host::{
    configure_event_loop_builder, decorate_main_window_attributes,
    decorate_settings_window_attributes,
};
use suzaku_map::platform::{host_platform, support_for};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::dpi::PhysicalPosition;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowAttributes, WindowId};

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
    let attrs = WindowAttributes::default()
        .with_title("Suzaku XR Candidate Panel")
        .with_inner_size(LogicalSize::new(420.0, 520.0))
        .with_min_inner_size(LogicalSize::new(620.0, 560.0))
        .with_resizable(true);
    decorate_main_window_attributes(attrs)
}

fn settings_window_attributes() -> WindowAttributes {
    let attrs = WindowAttributes::default()
        .with_title("Suzaku Panel Settings")
        .with_inner_size(LogicalSize::new(520.0, 340.0))
        .with_resizable(false);
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

#[derive(Default)]
struct PanelApp {
    panel: Option<PanelState>,
    settings: Option<PanelState>,
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
            panel.poll_voice_bridge();
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
    handwriting_dragging: bool,
    compact_hovered: bool,
    compact_dragging: bool,
    compact_drag_moved: bool,
    compact_drag_start_cursor: Option<(f32, f32)>,
    compact_drag_start_window_pos: Option<PhysicalPosition<i32>>,
    touch_tap_pending: bool,
    touch_start_position: Option<(f32, f32)>,
    voice_stability_ticks: u8,
    last_polled_voice_transcript: String,
    last_handwriting_summary: Option<String>,
    composition_base_seed: String,
    selected_next_tokens: Vec<String>,
    expanded_window_size: Option<LogicalSize<f64>>,
    expanded_window_pos: Option<PhysicalPosition<i32>>,
    compact_dock_edge: Option<DockEdge>,
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
        let had_initial_chrome = initial_chrome.is_some();
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
            font_face: FontFaceChoice::Auto,
            text_spacing: TextSpacing::Normal,
            text_smoothing: TextSmoothing::Smooth,
            voice_state: VoiceCaptureState::Idle,
            voice_permission: VoicePermissionState::Unknown,
            voice_backend_label: "Unknown Voice Host".to_string(),
            voice_supports_live_capture: false,
            voice_transcript: String::new(),
            voice_visual_phase: 0,
            voice_auto_insert: true,
            llm_enabled: false,
            llm_model: LlmModelPreset::Llama32_3b,
            llm_temperature: LlmTemperaturePreset::Balanced,
            composed_tokens: Vec::new(),
            next_token_candidates: Vec::new(),
            sentence_candidates: Vec::new(),
            handwriting_strokes: Vec::new(),
            handwriting_candidates: Vec::new(),
            handwriting_hint: "Draw a seed word with mouse or touch.".to_string(),
        });
        if !had_initial_chrome {
            if let Some(saved) = load_display_settings() {
                apply_display_settings(&mut chrome, &saved);
            }
        } else if let Some(saved) = load_display_settings() {
            apply_display_settings(&mut chrome, &saved);
        }

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
            handwriting_dragging: false,
            compact_hovered: false,
            compact_dragging: false,
            compact_drag_moved: false,
            compact_drag_start_cursor: None,
            compact_drag_start_window_pos: None,
            touch_tap_pending: false,
            touch_start_position: None,
            voice_stability_ticks: 0,
            last_polled_voice_transcript: String::new(),
            last_handwriting_summary: None,
            composition_base_seed,
            selected_next_tokens: Vec::new(),
            expanded_window_size: None,
            expanded_window_pos: None,
            compact_dock_edge: None,
        };
        if kind == PanelWindowKind::Main {
            state.reconfigure_llama_plugin();
        }

        Ok(state)
    }
}

impl PanelState {
    fn apply_compact_mode(&mut self, compact: bool) {
        if self.kind != PanelWindowKind::Main || self.chrome.compact_mode == compact {
            return;
        }
        if compact {
            self.expanded_window_size = Some(LogicalSize::new(
                self.size.width as f64,
                self.size.height as f64,
            ));
            self.expanded_window_pos = self.window.outer_position().ok();
            self.chrome.settings_open = false;
            self.window.set_decorations(false);
            self.window.set_resizable(false);
            let _ = self.window.request_inner_size(LogicalSize::new(96.0, 96.0));
        } else {
            let restored = self
                .expanded_window_size
                .unwrap_or_else(|| LogicalSize::new(420.0, 520.0));
            self.window.set_decorations(true);
            self.window.set_resizable(true);
            let _ = self.window.request_inner_size(restored);
            self.restore_expanded_window_position(restored);
        }
        self.chrome.compact_mode = compact;
        self.compact_hovered = false;
        self.compact_dragging = false;
        self.compact_drag_moved = false;
        self.compact_drag_start_cursor = None;
        self.compact_drag_start_window_pos = None;
        self.touch_tap_pending = false;
        self.touch_start_position = None;
    }

    fn begin_compact_drag(&mut self) {
        if self.kind != PanelWindowKind::Main || !self.chrome.compact_mode {
            return;
        }
        self.compact_dragging = true;
        self.compact_drag_moved = false;
        self.compact_drag_start_cursor = self.cursor_position;
        self.compact_drag_start_window_pos = self.window.outer_position().ok();
        let _ = self.window.drag_window();
    }

    fn update_compact_hover(&mut self) {
        self.compact_hovered = if self.kind == PanelWindowKind::Main && self.chrome.compact_mode {
            match self.cursor_position {
                Some((x, y)) => {
                    let snapshot = self.engine.snapshot();
                    self.renderer
                        .build_compact_scene(&snapshot, &self.chrome, false, self.compact_dragging)
                        .hit_interaction(x, y)
                        == Some(suzaku_map::ime::gpu::InteractionKind::ToggleCompactMode)
                }
                None => false,
            }
        } else {
            false
        };
    }

    fn end_compact_drag(&mut self) -> bool {
        if !self.compact_dragging {
            return false;
        }
        let moved = match (
            self.compact_drag_start_window_pos,
            self.window.outer_position().ok(),
        ) {
            (Some(start), Some(end)) => (end.x - start.x).abs() > 3 || (end.y - start.y).abs() > 3,
            _ => self.compact_drag_moved,
        };
        self.compact_dragging = false;
        self.compact_drag_start_cursor = None;
        self.compact_drag_start_window_pos = None;
        self.compact_drag_moved = false;
        if moved {
            self.snap_compact_window_to_edge();
        }
        self.update_compact_hover();
        moved
    }

    fn snap_compact_window_to_edge(&mut self) {
        if self.kind != PanelWindowKind::Main || !self.chrome.compact_mode {
            return;
        }
        let Ok(current_pos) = self.window.outer_position() else {
            return;
        };
        let Some(monitor) = self.window.current_monitor() else {
            return;
        };
        let monitor_pos = monitor.position();
        let monitor_size = monitor.size();
        let margin = 12_i32;
        let win_w = self.size.width as i32;
        let win_h = self.size.height as i32;
        let min_x = monitor_pos.x + margin;
        let max_x = monitor_pos.x + monitor_size.width as i32 - win_w - margin;
        let min_y = monitor_pos.y + margin;
        let max_y = monitor_pos.y + monitor_size.height as i32 - win_h - margin;
        let clamped_y = current_pos.y.clamp(min_y, max_y.max(min_y));
        let clamped_x = current_pos.x.clamp(min_x, max_x.max(min_x));
        let current = PhysicalPosition::new(clamped_x, clamped_y);
        let threshold = 12_i32;
        let distances = [
            (
                (clamped_x - min_x).abs(),
                DockEdge::Left,
                PhysicalPosition::new(min_x, clamped_y),
            ),
            (
                (clamped_x - max_x).abs(),
                DockEdge::Right,
                PhysicalPosition::new(max_x.max(min_x), clamped_y),
            ),
            (
                (clamped_y - min_y).abs(),
                DockEdge::Top,
                PhysicalPosition::new(clamped_x, min_y),
            ),
            (
                (clamped_y - max_y).abs(),
                DockEdge::Bottom,
                PhysicalPosition::new(clamped_x, max_y.max(min_y)),
            ),
        ];
        if let Some((_, edge, snapped)) = distances
            .into_iter()
            .min_by_key(|(distance, _, _)| *distance)
        {
            let distance = match edge {
                DockEdge::Left => (current.x - min_x).abs(),
                DockEdge::Right => (current.x - max_x.max(min_x)).abs(),
                DockEdge::Top => (current.y - min_y).abs(),
                DockEdge::Bottom => (current.y - max_y.max(min_y)).abs(),
            };
            if distance <= threshold {
                self.window.set_outer_position(snapped);
                self.compact_dock_edge = Some(edge);
            } else {
                self.window.set_outer_position(current);
                self.compact_dock_edge = None;
            }
        }
    }

    fn restore_expanded_window_position(&mut self, restored_size: LogicalSize<f64>) {
        let compact_pos = self.window.outer_position().ok();
        let Some(monitor) = self.window.current_monitor() else {
            if let Some(saved) = self.expanded_window_pos {
                self.window.set_outer_position(saved);
            }
            return;
        };
        let monitor_pos = monitor.position();
        let monitor_size = monitor.size();
        let margin = 18_i32;
        let restored_w = restored_size.width.round() as i32;
        let restored_h = restored_size.height.round() as i32;
        let min_x = monitor_pos.x + margin;
        let max_x = monitor_pos.x + monitor_size.width as i32 - restored_w - margin;
        let min_y = monitor_pos.y + margin;
        let max_y = monitor_pos.y + monitor_size.height as i32 - restored_h - margin;

        let target = match (compact_pos, self.compact_dock_edge) {
            (Some(pos), Some(DockEdge::Left)) => {
                PhysicalPosition::new(min_x.max(pos.x), pos.y.clamp(min_y, max_y.max(min_y)))
            }
            (Some(pos), Some(DockEdge::Right)) => PhysicalPosition::new(
                (pos.x + self.size.width as i32 - restored_w).clamp(min_x, max_x.max(min_x)),
                pos.y.clamp(min_y, max_y.max(min_y)),
            ),
            (Some(pos), Some(DockEdge::Top)) => {
                PhysicalPosition::new(pos.x.clamp(min_x, max_x.max(min_x)), min_y)
            }
            (Some(pos), Some(DockEdge::Bottom)) => PhysicalPosition::new(
                pos.x.clamp(min_x, max_x.max(min_x)),
                (pos.y + self.size.height as i32 - restored_h).clamp(min_y, max_y.max(min_y)),
            ),
            _ => self.expanded_window_pos.unwrap_or_else(|| {
                PhysicalPosition::new(
                    min_x.max(monitor_pos.x + (monitor_size.width as i32 - restored_w) / 2),
                    min_y.max(monitor_pos.y + (monitor_size.height as i32 - restored_h) / 2),
                )
            }),
        };
        self.window.set_outer_position(PhysicalPosition::new(
            target.x.clamp(min_x, max_x.max(min_x)),
            target.y.clamp(min_y, max_y.max(min_y)),
        ));
        self.expanded_window_pos = Some(target);
    }
    fn note_expanded_window_position(&mut self) {
        if self.kind == PanelWindowKind::Main && !self.chrome.compact_mode {
            self.expanded_window_pos = self.window.outer_position().ok();
        }
    }
}

fn point_in_rect(x: f32, y: f32, rect: [f32; 4]) -> bool {
    let [rx, ry, rw, rh] = rect;
    x >= rx && x <= rx + rw && y >= ry && y <= ry + rh
}

fn window_title(
    scene: &RenderScene,
    committed_text: &str,
    uses_runtime_font: bool,
    font_label: &str,
) -> String {
    let selected = scene.selected_label.as_deref().unwrap_or("no candidate");
    let host = support_for(host_platform());
    format!(
        "Suzaku XR Candidate Panel | host: {:?} ({:?}) | font: {}:{} | selected: {selected} | draft: {} | committed: {committed_text} | keys: 1/2/3 seed, arrows move, D degrade, R reset, Space commit",
        host.platform,
        host.tier,
        if uses_runtime_font {
            "system-atlas"
        } else {
            "bitmap-fallback"
        },
        font_label,
        scene.draft_text,
    )
}

#[allow(dead_code)]
fn _quad_debug(_quad: &CandidateQuad) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_chrome_defaults_to_llm_disabled() {
        let chrome = PanelChromeState::default();

        assert!(!chrome.llm_enabled);
    }

    #[test]
    fn voice_fallback_only_runs_when_bridge_is_unavailable() {
        assert!(!PanelState::voice_fallback_allowed(
            VoicePermissionState::Pending,
            true
        ));
        assert!(!PanelState::voice_fallback_allowed(
            VoicePermissionState::Denied,
            true
        ));
        assert!(PanelState::voice_fallback_allowed(
            VoicePermissionState::Unavailable,
            true
        ));
        assert!(PanelState::voice_fallback_allowed(
            VoicePermissionState::Unknown,
            false
        ));
    }

    #[test]
    fn suzaku_bird_svg_asset_exists() {
        assert!(
            std::path::Path::new(
                "/Users/Shared/chroot/dev/Suzaku-map/src/assets/icons/suzaku-bird.svg"
            )
            .exists()
        );
    }

    #[test]
    fn voice_transcript_normalization_collapses_whitespace() {
        assert_eq!(
            PanelState::normalize_voice_transcript("  hello   xr \n panel  "),
            "hello xr panel"
        );
    }

    #[test]
    fn voice_auto_insert_requires_stable_ready_transcript() {
        assert!(!PanelState::should_auto_insert_voice_transcript(
            "hello",
            2,
            VoiceCaptureState::Listening,
            VoicePermissionState::Ready,
        ));
        assert!(PanelState::should_auto_insert_voice_transcript(
            "hello xr",
            3,
            VoiceCaptureState::Listening,
            VoicePermissionState::Ready,
        ));
        assert!(!PanelState::should_auto_insert_voice_transcript(
            "hello xr",
            3,
            VoiceCaptureState::Idle,
            VoicePermissionState::Ready,
        ));
    }
}
