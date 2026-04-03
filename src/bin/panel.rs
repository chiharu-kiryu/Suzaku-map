#![cfg(feature = "gpu")]

use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use fontdue::Font;
use suzaku_map::ime::gpu::{
    AtlasGlyph, CandidateDensity, CandidateQuad, DisplayTextScale, FontFaceChoice, InputMode,
    InteractionKind, LlmModelPreset, LlmTemperaturePreset, PanelChromeState, PreviewStyle,
    RenderScene, TextSmoothing, TextSpacing, VirtualKeyboardKey, VoiceCaptureState,
    VoicePermissionState, WgpuCandidateRenderer,
};
use suzaku_map::ime::{CommitOptions, EngineConfig, InputSource, SignalState, XRTabletImeEngine};
use suzaku_map::languages::llama::{LlamaProviderConfig, llama_english_plugin_with_config};
use suzaku_map::platform::gpu_host::{
    configure_event_loop_builder, decorate_main_window_attributes,
    decorate_settings_window_attributes, is_quit_shortcut, preferred_font_paths,
};
use suzaku_map::platform::settings_host::display_settings_path;
use suzaku_map::platform::voice_host::HostSpeechRecognizer;
use suzaku_map::platform::{host_platform, support_for};
use wgpu::SurfaceError;
use wgpu::util::DeviceExt;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
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

        if panel.chrome.settings_open && self.settings.is_none() {
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
            panel.window.request_redraw();
            if let Some(settings) = self.settings.as_mut() {
                settings.chrome = panel.chrome.clone();
                settings.window.request_redraw();
            }
        }
    }
}

fn handle_panel_window_event(
    state: &mut PanelState,
    event_loop: &ActiveEventLoop,
    event: WindowEvent,
    allow_exit: bool,
) {
    match event {
        WindowEvent::CloseRequested => {
            if allow_exit {
                event_loop.exit();
            }
        }
        WindowEvent::Resized(size) => state.resize(size.width, size.height),
        WindowEvent::ScaleFactorChanged { .. } => state.window.request_redraw(),
        WindowEvent::CursorMoved { position, .. } => {
            state.cursor_position = Some((position.x as f32, position.y as f32));
            if state.kind == PanelWindowKind::Main {
                state.extend_handwriting_stroke();
            }
            state.window.request_redraw();
        }
        WindowEvent::ModifiersChanged(modifiers) => {
            state.modifiers = modifiers.state();
        }
        WindowEvent::Touch(touch) => {
            state.cursor_position = Some((touch.location.x as f32, touch.location.y as f32));
            if state.kind == PanelWindowKind::Main {
                match touch.phase {
                    TouchPhase::Started => {
                        let _ = state.try_begin_handwriting_stroke();
                    }
                    TouchPhase::Moved => state.extend_handwriting_stroke(),
                    TouchPhase::Ended | TouchPhase::Cancelled => state.finish_handwriting_stroke(),
                }
            }
            state.window.request_redraw();
        }
        WindowEvent::MouseInput {
            state: ElementState::Pressed,
            button: MouseButton::Left,
            ..
        } => {
            if !(state.kind == PanelWindowKind::Main && state.try_begin_handwriting_stroke()) {
                state.select_at_cursor();
            }
            state.window.request_redraw();
        }
        WindowEvent::MouseInput {
            state: ElementState::Released,
            button: MouseButton::Left,
            ..
        } => {
            if state.kind == PanelWindowKind::Main {
                state.finish_handwriting_stroke();
            }
            state.window.request_redraw();
        }
        WindowEvent::KeyboardInput { event, .. } => {
            if event.state == ElementState::Pressed {
                if allow_exit && state.is_quit_shortcut(&event.physical_key) {
                    event_loop.exit();
                    return;
                }
                if state.kind == PanelWindowKind::Settings {
                    if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
                        state.chrome.settings_open = false;
                    }
                } else {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::ArrowLeft) => state.chrome.move_caret_left(),
                        PhysicalKey::Code(KeyCode::ArrowRight) => state.chrome.move_caret_right(),
                        PhysicalKey::Code(KeyCode::ArrowDown) => {
                            if state.chrome.input_focused {
                                state.chrome.blur_input();
                            } else {
                                state.engine.move_selection(1);
                            }
                        }
                        PhysicalKey::Code(KeyCode::ArrowUp) => {
                            state.engine.move_selection(-1);
                        }
                        PhysicalKey::Code(KeyCode::Digit1) => {
                            if !(state.chrome.input_focused
                                && state.chrome.active_input_mode == InputMode::VirtualKeyboard)
                            {
                                state.chrome.active_input_mode = InputMode::VirtualKeyboard;
                            }
                        }
                        PhysicalKey::Code(KeyCode::Digit2) => {
                            if !(state.chrome.input_focused
                                && state.chrome.active_input_mode == InputMode::VirtualKeyboard)
                            {
                                state.chrome.active_input_mode = InputMode::Dictation;
                            }
                        }
                        PhysicalKey::Code(KeyCode::Digit3) => {
                            if !(state.chrome.input_focused
                                && state.chrome.active_input_mode == InputMode::VirtualKeyboard)
                            {
                                state.chrome.active_input_mode = InputMode::Handwriting;
                            }
                        }
                        PhysicalKey::Code(KeyCode::Tab) => {
                            state.chrome.input_modes_expanded = !state.chrome.input_modes_expanded;
                        }
                        PhysicalKey::Code(KeyCode::KeyD) => {
                            if !state.chrome.input_focused {
                                state.engine.update_signal(SignalState {
                                    pointer_precision: 0.2,
                                    gaze_stability: 0.2,
                                    host_intent_weight: 0.4,
                                    source_confidence: 0.4,
                                });
                            }
                        }
                        PhysicalKey::Code(KeyCode::KeyV) => {
                            if state.chrome.active_input_mode == InputMode::Dictation {
                                if state.chrome.voice_state == VoiceCaptureState::Listening {
                                    state.stop_voice_capture();
                                } else {
                                    state.start_voice_capture();
                                }
                            }
                        }
                        PhysicalKey::Code(KeyCode::KeyN) => {
                            if state.chrome.active_input_mode == InputMode::Dictation {
                                state.advance_voice_sample();
                            }
                        }
                        PhysicalKey::Code(KeyCode::KeyI) => {
                            if state.chrome.active_input_mode == InputMode::Dictation {
                                state.insert_voice_transcript();
                            }
                        }
                        PhysicalKey::Code(KeyCode::KeyR) => {
                            if !state.chrome.input_focused {
                                state.reset_signal();
                            }
                        }
                        PhysicalKey::Code(KeyCode::Backspace) => {
                            if state.chrome.input_focused {
                                state.backspace_seed();
                            }
                        }
                        PhysicalKey::Code(KeyCode::Space) => {
                            if state.chrome.active_input_mode == InputMode::VirtualKeyboard
                                && state.chrome.input_focused
                            {
                                state.handle_text_input(" ");
                            } else {
                                let _ = state.engine.commit(CommitOptions { force: true });
                            }
                        }
                        PhysicalKey::Code(KeyCode::Enter) => {
                            if state.chrome.input_focused {
                                state.chrome.blur_input();
                            } else {
                                let _ = state.engine.commit(CommitOptions { force: true });
                            }
                        }
                        _ => {}
                    }
                    if let Some(text) = event.text.as_deref() {
                        state.handle_text_input(text);
                    }
                }
                state.window.request_redraw();
            }
        }
        WindowEvent::RedrawRequested => {
            if let Err(err) = state.render() {
                match err {
                    SurfaceError::Lost | SurfaceError::Outdated => {
                        state.resize(state.size.width, state.size.height);
                    }
                    SurfaceError::OutOfMemory => {
                        if allow_exit {
                            event_loop.exit();
                        }
                    }
                    SurfaceError::Timeout | SurfaceError::Other => {}
                }
            }
        }
        _ => {}
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct PanelVertex {
    position: [f32; 2],
    color: [f32; 4],
}

impl PanelVertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<PanelVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 2]>() as u64,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct TextVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

impl TextVertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<TextVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 2]>() as u64,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as u64,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

struct FontAtlas {
    bind_group: wgpu::BindGroup,
    uv_map: HashMap<char, [f32; 4]>,
    uses_runtime_font: bool,
    font_label: String,
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
    last_handwriting_summary: Option<String>,
    composition_base_seed: String,
    selected_next_tokens: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PersistedDisplaySettings {
    text_scale: DisplayTextScale,
    candidate_density: CandidateDensity,
    preview_style: PreviewStyle,
    font_face: FontFaceChoice,
    text_spacing: TextSpacing,
    text_smoothing: TextSmoothing,
    llm_enabled: bool,
    llm_model: LlmModelPreset,
    llm_temperature: LlmTemperaturePreset,
}

struct VoiceInputController {
    samples: Vec<&'static str>,
    next_index: usize,
    bridge: Option<HostSpeechRecognizer>,
}

impl VoiceInputController {
    fn new() -> Self {
        Self {
            samples: vec![
                "hello xr panel",
                "tablet ime voice seed",
                "continue this sentence by tap",
                "spatial input feels lighter with speech",
            ],
            next_index: 0,
            bridge: HostSpeechRecognizer::new(),
        }
    }

    fn next_sample(&mut self) -> String {
        let sample = self.samples[self.next_index % self.samples.len()].to_string();
        self.next_index = (self.next_index + 1) % self.samples.len();
        sample
    }
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
        let voice = VoiceInputController::new();
        let had_initial_chrome = initial_chrome.is_some();
        let mut chrome = initial_chrome.unwrap_or(PanelChromeState {
            seed_text: "ni hao".into(),
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

        if let Some(bridge) = &voice.bridge {
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
            last_handwriting_summary: None,
            composition_base_seed,
            selected_next_tokens: Vec::new(),
        };
        if kind == PanelWindowKind::Main {
            state.reconfigure_llama_plugin();
        }

        Ok(state)
    }

    fn reset_signal(&mut self) {
        self.engine.update_signal(SignalState {
            pointer_precision: 0.42,
            gaze_stability: 0.50,
            host_intent_weight: 0.80,
            source_confidence: 0.70,
        });
        self.refresh_seed();
    }

    fn current_llama_config(&self) -> LlamaProviderConfig {
        LlamaProviderConfig {
            endpoint: "http://127.0.0.1:11434/v1/chat/completions".to_string(),
            model: match self.chrome.llm_model {
                LlmModelPreset::Llama32_3b => "llama3.2:3b".to_string(),
            },
            system_prompt: "You are a sentence-completion engine for an XR and tablet IME. Expand the user's seed into 3 short, tap-friendly English sentence candidates. Return plain text only, one candidate per line, no numbering.".to_string(),
            max_tokens: 96,
            temperature_tenths: match self.chrome.llm_temperature {
                LlmTemperaturePreset::Focused => 2,
                LlmTemperaturePreset::Balanced => 4,
                LlmTemperaturePreset::Expressive => 7,
            },
            timeout_ms: 1200,
            handwriting_hint: self.last_handwriting_summary.clone(),
        }
    }

    fn reconfigure_llama_plugin(&mut self) {
        if self.chrome.llm_enabled {
            self.engine
                .register_language_plugin(llama_english_plugin_with_config(
                    self.current_llama_config(),
                ));
            self.engine.set_language("llama-en");
        } else {
            self.engine.set_language("en");
        }
        self.refresh_seed();
    }

    fn sync_manual_seed_base(&mut self) {
        self.selected_next_tokens.clear();
        self.composition_base_seed = self
            .chrome
            .seed_text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
    }

    fn full_composed_seed(&self) -> String {
        let mut parts = Vec::new();
        if !self.composition_base_seed.is_empty() {
            parts.push(self.composition_base_seed.clone());
        }
        if !self.selected_next_tokens.is_empty() {
            parts.push(self.selected_next_tokens.join(" "));
        }
        parts.join(" ").trim().to_string()
    }

    fn refresh_composition_candidates(&mut self) {
        let snapshot = self.engine.snapshot();
        let normalized_seed = snapshot.seed_text.trim().to_string();
        self.chrome.composed_tokens = self.selected_next_tokens.clone();
        self.chrome.next_token_candidates =
            derive_next_token_candidates(&normalized_seed, &snapshot.candidate_labels, 6);
        self.chrome.sentence_candidates = if normalized_seed.split_whitespace().count() >= 2 {
            snapshot.candidate_labels.iter().take(4).cloned().collect()
        } else {
            Vec::new()
        };
    }

    fn select_next_token(&mut self, index: usize) {
        let Some(token) = self.chrome.next_token_candidates.get(index).cloned() else {
            return;
        };
        if self.selected_next_tokens.is_empty() {
            self.composition_base_seed = self
                .chrome
                .seed_text
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
        }
        self.selected_next_tokens.push(token);
        let full_seed = self.full_composed_seed();
        self.chrome.set_seed_text(full_seed.clone());
        self.chrome.move_caret_to_end();
        self.engine.seed(&full_seed);
        self.refresh_composition_candidates();
    }

    fn rewind_next_token(&mut self) {
        if self.selected_next_tokens.pop().is_none() {
            return;
        }
        let full_seed = self.full_composed_seed();
        self.chrome.set_seed_text(full_seed.clone());
        self.chrome.move_caret_to_end();
        self.engine.seed(&full_seed);
        self.refresh_composition_candidates();
    }

    fn rebuild_font_atlas(&mut self) {
        let text_bind_group_layout = self.text_pipeline.get_bind_group_layout(0);
        self.font_atlas = create_font_atlas(
            &self.device,
            &self.queue,
            &text_bind_group_layout,
            self.chrome.font_face,
            self.chrome.text_smoothing,
        );
    }

    fn persist_display_settings(&self) {
        let _ = save_display_settings(&PersistedDisplaySettings::from(&self.chrome));
    }

    fn advance_voice_sample(&mut self) {
        self.chrome.voice_transcript = self.voice.next_sample();
    }

    fn voice_fallback_allowed(permission: VoicePermissionState, bridge_available: bool) -> bool {
        !bridge_available || permission == VoicePermissionState::Unavailable
    }

    fn poll_voice_bridge(&mut self) {
        if let Some(bridge) = &self.voice.bridge {
            self.chrome.voice_permission = bridge.permission_state();
            self.chrome.voice_backend_label = bridge.source_label().to_string();
            self.chrome.voice_supports_live_capture = bridge.supports_live_capture();
            if self.chrome.voice_permission != VoicePermissionState::Ready
                && self.chrome.voice_state == VoiceCaptureState::Listening
            {
                self.chrome.voice_state = VoiceCaptureState::Idle;
            }
            if let Some(transcript) = bridge.poll_transcript() {
                self.chrome.voice_transcript = transcript;
            }
        } else {
            self.chrome.voice_permission = VoicePermissionState::Unavailable;
            self.chrome.voice_backend_label = "Fallback Samples".to_string();
            self.chrome.voice_supports_live_capture = false;
        }
    }

    fn start_voice_capture(&mut self) {
        let Some(bridge) = &self.voice.bridge else {
            self.chrome.voice_permission = VoicePermissionState::Unavailable;
            self.chrome.voice_state = VoiceCaptureState::Idle;
            self.advance_voice_sample();
            return;
        };

        bridge.request_permissions();
        let permission = bridge.permission_state();
        self.chrome.voice_permission = permission;

        if permission != VoicePermissionState::Ready {
            self.chrome.voice_state = VoiceCaptureState::Idle;
            if Self::voice_fallback_allowed(permission, true) {
                self.advance_voice_sample();
            }
            return;
        }

        if bridge.start() {
            bridge.seed_debug_transcript_from_env();
            self.chrome.voice_state = VoiceCaptureState::Listening;
        } else {
            let permission = bridge.permission_state();
            self.chrome.voice_permission = permission;
            self.chrome.voice_state = VoiceCaptureState::Idle;
            if Self::voice_fallback_allowed(permission, true) {
                self.advance_voice_sample();
            }
        }
    }

    fn stop_voice_capture(&mut self) {
        if let Some(bridge) = &self.voice.bridge {
            bridge.stop();
            self.chrome.voice_permission = bridge.permission_state();
        } else {
            self.chrome.voice_permission = VoicePermissionState::Unavailable;
        }
        self.chrome.voice_state = VoiceCaptureState::Idle;
    }

    fn insert_voice_transcript(&mut self) {
        if self.chrome.voice_transcript.is_empty() {
            return;
        }

        if !self.chrome.seed_text.is_empty() && !self.chrome.seed_text.ends_with(' ') {
            self.chrome.insert_text(" ");
        }
        let transcript = self.chrome.voice_transcript.clone();
        self.chrome.insert_text(&transcript);
        self.chrome.active_input_mode = InputMode::VirtualKeyboard;
        self.chrome.input_focused = true;
        self.chrome.move_caret_to_end();
        self.sync_manual_seed_base();
        self.refresh_seed();
    }

    fn current_scene(&self) -> RenderScene {
        match self.kind {
            PanelWindowKind::Main => {
                let mut chrome = self.chrome.clone();
                chrome.settings_open = false;
                self.renderer
                    .build_panel_scene(&self.engine.snapshot(), &chrome)
            }
            PanelWindowKind::Settings => self.renderer.build_settings_scene(&self.chrome),
        }
    }

    fn adopt_settings_from(&mut self, other: &PanelChromeState) {
        let needs_font_rebuild = self.chrome.font_face != other.font_face
            || self.chrome.text_smoothing != other.text_smoothing;
        let needs_llm_reconfigure = self.chrome.llm_enabled != other.llm_enabled
            || self.chrome.llm_model != other.llm_model
            || self.chrome.llm_temperature != other.llm_temperature;

        self.chrome.text_scale = other.text_scale;
        self.chrome.candidate_density = other.candidate_density;
        self.chrome.preview_style = other.preview_style;
        self.chrome.font_face = other.font_face;
        self.chrome.text_spacing = other.text_spacing;
        self.chrome.text_smoothing = other.text_smoothing;
        self.chrome.llm_enabled = other.llm_enabled;
        self.chrome.llm_model = other.llm_model;
        self.chrome.llm_temperature = other.llm_temperature;
        self.persist_display_settings();

        if needs_font_rebuild {
            self.rebuild_font_atlas();
        }
        if self.kind == PanelWindowKind::Main && needs_llm_reconfigure {
            self.reconfigure_llama_plugin();
        }
    }

    fn handwriting_canvas_rect(&self) -> Option<[f32; 4]> {
        self.current_scene()
            .interactive_targets
            .iter()
            .find(|target| matches!(target.kind, InteractionKind::HandwritingCanvas))
            .map(|target| target.rect)
    }

    fn try_begin_handwriting_stroke(&mut self) -> bool {
        if self.chrome.active_input_mode != InputMode::Handwriting {
            return false;
        }

        let Some((x, y)) = self.cursor_position else {
            return false;
        };
        let Some(rect) = self.handwriting_canvas_rect() else {
            return false;
        };
        if !point_in_rect(x, y, rect) {
            return false;
        }

        self.chrome.blur_input();
        self.handwriting_dragging = true;
        self.chrome.handwriting_strokes.push(vec![[x, y]]);
        self.chrome.handwriting_hint = "Tracing… release to recognize".to_string();
        true
    }

    fn extend_handwriting_stroke(&mut self) {
        if !self.handwriting_dragging || self.chrome.active_input_mode != InputMode::Handwriting {
            return;
        }

        let Some((x, y)) = self.cursor_position else {
            return;
        };
        let Some(rect) = self.handwriting_canvas_rect() else {
            return;
        };
        let clamped = [
            x.clamp(rect[0] + 4.0, rect[0] + rect[2] - 4.0),
            y.clamp(rect[1] + 4.0, rect[1] + rect[3] - 4.0),
        ];
        let Some(stroke) = self.chrome.handwriting_strokes.last_mut() else {
            return;
        };
        if stroke
            .last()
            .map(|last| (last[0] - clamped[0]).hypot(last[1] - clamped[1]) >= 3.0)
            .unwrap_or(true)
        {
            stroke.push(clamped);
        }
    }

    fn finish_handwriting_stroke(&mut self) {
        if !self.handwriting_dragging {
            return;
        }
        self.handwriting_dragging = false;
        self.last_handwriting_summary = Some(summarize_handwriting_strokes(
            &self.chrome.handwriting_strokes,
        ));
        self.chrome.handwriting_candidates =
            recognize_handwriting_candidates(&self.chrome.handwriting_strokes);
        self.reconfigure_llama_plugin();
        self.chrome.handwriting_hint = if self.chrome.handwriting_candidates.is_empty() {
            "Try a clearer trace, then tap a recognized seed.".to_string()
        } else {
            "Tap a recognized seed to insert it.".to_string()
        };
    }

    fn clear_handwriting(&mut self) {
        self.handwriting_dragging = false;
        self.chrome.handwriting_strokes.clear();
        self.chrome.handwriting_candidates.clear();
        self.chrome.handwriting_hint = "Draw a seed word with mouse or touch.".to_string();
        self.last_handwriting_summary = None;
        self.reconfigure_llama_plugin();
    }

    fn insert_handwriting_candidate(&mut self, index: usize) {
        let Some(candidate) = self.chrome.handwriting_candidates.get(index).cloned() else {
            return;
        };
        if !self.chrome.seed_text.is_empty() && !self.chrome.seed_text.ends_with(' ') {
            self.chrome.insert_text(" ");
        }
        self.chrome.insert_text(&candidate);
        self.chrome.active_input_mode = InputMode::VirtualKeyboard;
        self.chrome.focus_input();
        self.chrome.move_caret_to_end();
        self.sync_manual_seed_base();
        self.refresh_seed();
        self.clear_handwriting();
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        self.size.width = width;
        self.size.height = height;
        self.config.width = width;
        self.config.height = height;
        self.renderer = WgpuCandidateRenderer::new(width as f32, height as f32);
        self.surface.configure(&self.device, &self.config);
    }

    fn render(&mut self) -> Result<(), SurfaceError> {
        let snapshot = self.engine.snapshot();
        let scene = self.current_scene();
        match self.kind {
            PanelWindowKind::Main => self.window.set_title(&window_title(
                &scene,
                &snapshot.committed_text,
                self.font_atlas.uses_runtime_font,
                &self.font_atlas.font_label,
            )),
            PanelWindowKind::Settings => self.window.set_title("Suzaku Panel Settings"),
        }

        let shape_vertices =
            build_shape_vertices(&scene, self.config.width as f32, self.config.height as f32);
        let shape_vertex_buffer =
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("suzaku-panel-shape-vertices"),
                    contents: bytemuck::cast_slice(&shape_vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
        let text_vertices = build_text_vertices(
            &scene.atlas_glyphs,
            &self.font_atlas,
            self.config.width as f32,
            self.config.height as f32,
        );
        let text_vertex_buffer =
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("suzaku-panel-text-vertices"),
                    contents: bytemuck::cast_slice(&text_vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let background = if snapshot.degraded {
            wgpu::Color {
                r: 0.07,
                g: 0.07,
                b: 0.09,
                a: 1.0,
            }
        } else {
            wgpu::Color {
                r: 0.03,
                g: 0.05,
                b: 0.08,
                a: 1.0,
            }
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("suzaku-panel-encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("suzaku-panel-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(background),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            pass.set_pipeline(&self.shape_pipeline);
            pass.set_vertex_buffer(0, shape_vertex_buffer.slice(..));
            pass.draw(0..shape_vertices.len() as u32, 0..1);
            if !text_vertices.is_empty() {
                pass.set_pipeline(&self.text_pipeline);
                pass.set_bind_group(0, &self.font_atlas.bind_group, &[]);
                pass.set_vertex_buffer(0, text_vertex_buffer.slice(..));
                pass.draw(0..text_vertices.len() as u32, 0..1);
            }
        }

        self.queue.submit([encoder.finish()]);
        output.present();
        Ok(())
    }

    fn select_at_cursor(&mut self) {
        let Some((x, y)) = self.cursor_position else {
            return;
        };

        let scene = self.current_scene();
        if let Some(kind) = scene.hit_interaction(x, y) {
            match kind {
                InteractionKind::SeedInput => {
                    self.chrome.focus_input();
                    self.chrome.move_caret_to_end();
                }
                InteractionKind::InputModesToggle => {
                    self.chrome.blur_input();
                    self.chrome.input_modes_expanded = !self.chrome.input_modes_expanded;
                }
                InteractionKind::InputModeButton(mode) => {
                    self.chrome.blur_input();
                    self.chrome.active_input_mode = mode;
                    if mode == InputMode::Dictation {
                        if let Some(bridge) = &self.voice.bridge {
                            bridge.request_permissions();
                            self.chrome.voice_permission = bridge.permission_state();
                        } else {
                            self.chrome.voice_permission = VoicePermissionState::Unavailable;
                        }
                    }
                }
                InteractionKind::SettingsToggle => {
                    self.chrome.settings_open = !self.chrome.settings_open;
                }
                InteractionKind::SetTextScale(scale) => {
                    self.chrome.text_scale = scale;
                    self.persist_display_settings();
                }
                InteractionKind::SetCandidateDensity(density) => {
                    self.chrome.candidate_density = density;
                    self.persist_display_settings();
                }
                InteractionKind::SetPreviewStyle(style) => {
                    self.chrome.preview_style = style;
                    self.persist_display_settings();
                }
                InteractionKind::SetFontFace(font_face) => {
                    self.chrome.font_face = font_face;
                    self.rebuild_font_atlas();
                    self.persist_display_settings();
                }
                InteractionKind::SetTextSpacing(spacing) => {
                    self.chrome.text_spacing = spacing;
                    self.persist_display_settings();
                }
                InteractionKind::SetTextSmoothing(smoothing) => {
                    self.chrome.text_smoothing = smoothing;
                    self.rebuild_font_atlas();
                    self.persist_display_settings();
                }
                InteractionKind::SetLlmEnabled(enabled) => {
                    self.chrome.llm_enabled = enabled;
                    self.reconfigure_llama_plugin();
                    self.persist_display_settings();
                }
                InteractionKind::SetLlmModel(model) => {
                    self.chrome.llm_model = model;
                    self.reconfigure_llama_plugin();
                    self.persist_display_settings();
                }
                InteractionKind::SetLlmTemperature(temp) => {
                    self.chrome.llm_temperature = temp;
                    self.reconfigure_llama_plugin();
                    self.persist_display_settings();
                }
                InteractionKind::SelectNextToken(index) => {
                    self.select_next_token(index);
                }
                InteractionKind::RewindNextToken => {
                    self.rewind_next_token();
                }
                InteractionKind::ToggleVoiceCapture => {
                    if self.chrome.voice_state == VoiceCaptureState::Listening {
                        self.stop_voice_capture();
                    } else {
                        self.start_voice_capture();
                    }
                }
                InteractionKind::CycleVoiceSample => {
                    self.advance_voice_sample();
                }
                InteractionKind::InsertVoiceTranscript => {
                    self.insert_voice_transcript();
                }
                InteractionKind::ClearVoiceTranscript => {
                    self.chrome.voice_transcript.clear();
                    self.chrome.voice_state = VoiceCaptureState::Idle;
                }
                InteractionKind::HandwritingCanvas => {}
                InteractionKind::ClearHandwriting => {
                    self.clear_handwriting();
                }
                InteractionKind::UseHandwritingCandidate(index) => {
                    self.insert_handwriting_candidate(index);
                }
                InteractionKind::VirtualKeyboardKey(key) => {
                    self.chrome.active_input_mode = InputMode::VirtualKeyboard;
                    self.chrome.focus_input();
                    match key {
                        VirtualKeyboardKey::Character(ch) => {
                            let mut text = String::new();
                            text.push(ch);
                            self.handle_text_input(&text);
                            if self.chrome.keyboard_shifted && ch.is_ascii_alphabetic() {
                                self.chrome.keyboard_shifted = false;
                            }
                        }
                        VirtualKeyboardKey::Text(text) => self.handle_text_input(text),
                        VirtualKeyboardKey::Space => self.handle_text_input(" "),
                        VirtualKeyboardKey::Backspace => self.backspace_seed(),
                        VirtualKeyboardKey::Shift => self.chrome.toggle_shift(),
                        VirtualKeyboardKey::ToggleNumeric => self.chrome.use_numeric_keyboard(),
                        VirtualKeyboardKey::ToggleAlphabetic => self.chrome.use_alpha_keyboard(),
                    }
                }
                InteractionKind::Candidate(index) => {
                    self.chrome.blur_input();
                    self.engine.select_candidate(index);
                }
            }
        }
    }

    fn handle_text_input(&mut self, text: &str) {
        if self.chrome.active_input_mode != InputMode::VirtualKeyboard || !self.chrome.input_focused
        {
            return;
        }

        let mut accepted = String::new();
        for ch in text.chars() {
            if ch.is_ascii_alphanumeric()
                || ch == ' '
                || matches!(
                    ch,
                    '.' | ','
                        | '?'
                        | '!'
                        | '\''
                        | '-'
                        | '/'
                        | ':'
                        | ';'
                        | '('
                        | ')'
                        | '$'
                        | '&'
                        | '@'
                )
            {
                accepted.push(ch);
            }
        }

        if !accepted.is_empty() {
            self.chrome.insert_text(&accepted);
            self.sync_manual_seed_base();
            self.refresh_seed();
        }
    }

    fn backspace_seed(&mut self) {
        self.chrome.backspace();
        self.sync_manual_seed_base();
        self.refresh_seed();
    }

    fn refresh_seed(&mut self) {
        let normalized = self
            .chrome
            .seed_text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        self.engine.seed(&normalized);
        self.refresh_composition_candidates();
    }

    fn is_quit_shortcut(&self, key: &PhysicalKey) -> bool {
        matches!(key, PhysicalKey::Code(KeyCode::KeyQ)) && is_quit_shortcut(self.modifiers)
    }
}

fn point_in_rect(x: f32, y: f32, rect: [f32; 4]) -> bool {
    let [rx, ry, rw, rh] = rect;
    x >= rx && x <= rx + rw && y >= ry && y <= ry + rh
}

fn derive_next_token_candidates(
    seed_text: &str,
    sentence_candidates: &[String],
    limit: usize,
) -> Vec<String> {
    let seed_tokens: Vec<String> = seed_text
        .split_whitespace()
        .map(|token| token.to_ascii_lowercase())
        .collect();
    let mut next = Vec::new();
    let mut later = Vec::new();

    for sentence in sentence_candidates {
        let words: Vec<&str> = sentence.split_whitespace().collect();
        let mut prefix_len = 0;
        while prefix_len < seed_tokens.len()
            && prefix_len < words.len()
            && seed_tokens[prefix_len].eq_ignore_ascii_case(words[prefix_len])
        {
            prefix_len += 1;
        }

        if prefix_len >= words.len() {
            continue;
        }

        let token = words[prefix_len]
            .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '\'')
            .to_ascii_lowercase();
        if !token.is_empty()
            && !seed_tokens.iter().any(|existing| existing == &token)
            && !next.iter().any(|existing| existing == &token)
        {
            next.push(token);
        }
        if next.len() >= limit {
            return next;
        }

        for candidate in words.iter().skip(prefix_len + 1) {
            let token = candidate
                .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '\'')
                .to_ascii_lowercase();
            if token.is_empty()
                || seed_tokens.iter().any(|existing| existing == &token)
                || next.iter().any(|existing| existing == &token)
                || later.iter().any(|existing| existing == &token)
            {
                continue;
            }
            later.push(token);
        }
    }

    for token in later {
        next.push(token);
        if next.len() >= limit {
            return next;
        }
    }

    for fallback in ["is", "can", "will", "for", "with", "next"] {
        if !seed_tokens.iter().any(|existing| existing == fallback)
            && !next.iter().any(|existing| existing == fallback)
        {
            next.push(fallback.to_string());
        }
        if next.len() >= limit {
            break;
        }
    }
    next
}

fn recognize_handwriting_candidates(strokes: &[Vec<[f32; 2]>]) -> Vec<String> {
    let points: Vec<[f32; 2]> = strokes
        .iter()
        .flat_map(|stroke| stroke.iter().copied())
        .collect();
    if points.len() < 2 {
        return Vec::new();
    }

    let min_x = points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
    let max_x = points
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
    let max_y = points
        .iter()
        .map(|p| p[1])
        .fold(f32::NEG_INFINITY, f32::max);
    let width = (max_x - min_x).max(1.0);
    let height = (max_y - min_y).max(1.0);
    let aspect = width / height;
    let start = points[0];
    let end = *points.last().unwrap_or(&start);
    let end_distance = (end[0] - start[0]).hypot(end[1] - start[1]);
    let path_length: f32 = points
        .windows(2)
        .map(|window| (window[1][0] - window[0][0]).hypot(window[1][1] - window[0][1]))
        .sum();
    let straightness = end_distance / path_length.max(1.0);
    let closure = end_distance / width.max(height);
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];

    if strokes.len() >= 2 && strokes.iter().all(|stroke| stroke.len() >= 2) {
        return vec!["t".into(), "tap".into(), "text".into()];
    }

    if closure < 0.35 && path_length > (width + height) * 1.2 {
        return vec!["o".into(), "open".into(), "okay".into()];
    }

    if straightness > 0.9 {
        if aspect < 0.55 {
            return vec!["i".into(), "line".into(), "input".into()];
        }
        if aspect > 1.8 {
            return vec!["to".into(), "go".into(), "next".into()];
        }
        if dx.abs() > dy.abs() {
            return vec!["hi".into(), "hello".into(), "hand".into()];
        }
    }

    if dx.abs() > dy.abs() && aspect > 1.25 && path_length > width * 1.4 {
        return vec!["wave".into(), "write".into(), "word".into()];
    }

    vec!["apple".into(), "hello".into(), "input".into()]
}

fn summarize_handwriting_strokes(strokes: &[Vec<[f32; 2]>]) -> String {
    let points: Vec<[f32; 2]> = strokes
        .iter()
        .flat_map(|stroke| stroke.iter().copied())
        .collect();
    if points.len() < 2 {
        return "very short trace".to_string();
    }

    let min_x = points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
    let max_x = points
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
    let max_y = points
        .iter()
        .map(|p| p[1])
        .fold(f32::NEG_INFINITY, f32::max);
    let width = (max_x - min_x).max(1.0);
    let height = (max_y - min_y).max(1.0);
    let aspect = width / height;
    let start = points[0];
    let end = *points.last().unwrap_or(&start);
    let closure = (end[0] - start[0]).hypot(end[1] - start[1]) / width.max(height);

    let shape = if closure < 0.35 {
        "looped"
    } else if aspect > 1.6 {
        "wide"
    } else if aspect < 0.65 {
        "tall"
    } else {
        "balanced"
    };

    format!(
        "{} handwriting trace with {} stroke(s) and {} points",
        shape,
        strokes.len(),
        points.len()
    )
}

fn build_shape_vertices(scene: &RenderScene, width: f32, height: f32) -> Vec<PanelVertex> {
    let mut vertices = Vec::with_capacity(scene.quads.len() * 6);

    for quad in &scene.quads {
        push_quad_vertices(&mut vertices, quad, width, height);
    }

    vertices
}

fn push_quad_vertices(
    vertices: &mut Vec<PanelVertex>,
    quad: &CandidateQuad,
    width: f32,
    height: f32,
) {
    let [x, y, w, h] = quad.rect;
    let color = quad.color;
    let x1 = px_to_ndc_x(x, width);
    let x2 = px_to_ndc_x(x + w, width);
    let y1 = px_to_ndc_y(y, height);
    let y2 = px_to_ndc_y(y + h, height);

    vertices.extend_from_slice(&[
        PanelVertex {
            position: [x1, y1],
            color,
        },
        PanelVertex {
            position: [x2, y1],
            color,
        },
        PanelVertex {
            position: [x2, y2],
            color,
        },
        PanelVertex {
            position: [x1, y1],
            color,
        },
        PanelVertex {
            position: [x2, y2],
            color,
        },
        PanelVertex {
            position: [x1, y2],
            color,
        },
    ]);
}

fn px_to_ndc_x(x: f32, width: f32) -> f32 {
    (x / width) * 2.0 - 1.0
}

fn px_to_ndc_y(y: f32, height: f32) -> f32 {
    1.0 - (y / height) * 2.0
}

fn build_text_vertices(
    glyphs: &[AtlasGlyph],
    atlas: &FontAtlas,
    width: f32,
    height: f32,
) -> Vec<TextVertex> {
    let mut vertices = Vec::with_capacity(glyphs.len() * 6);
    for glyph in glyphs {
        let uv = atlas.uv_for(glyph.ch);
        push_text_quad_vertices(&mut vertices, glyph, uv, width, height);
    }
    vertices
}

fn push_text_quad_vertices(
    vertices: &mut Vec<TextVertex>,
    glyph: &AtlasGlyph,
    uv: [f32; 4],
    width: f32,
    height: f32,
) {
    let [x, y, w, h] = glyph.rect;
    let color = glyph.color;
    let x1 = px_to_ndc_x(x, width);
    let x2 = px_to_ndc_x(x + w, width);
    let y1 = px_to_ndc_y(y, height);
    let y2 = px_to_ndc_y(y + h, height);
    let [u1, v1, u2, v2] = uv;

    vertices.extend_from_slice(&[
        TextVertex {
            position: [x1, y1],
            uv: [u1, v1],
            color,
        },
        TextVertex {
            position: [x2, y1],
            uv: [u2, v1],
            color,
        },
        TextVertex {
            position: [x2, y2],
            uv: [u2, v2],
            color,
        },
        TextVertex {
            position: [x1, y1],
            uv: [u1, v1],
            color,
        },
        TextVertex {
            position: [x2, y2],
            uv: [u2, v2],
            color,
        },
        TextVertex {
            position: [x1, y2],
            uv: [u1, v2],
            color,
        },
    ]);
}

impl FontAtlas {
    fn uv_for(&self, ch: char) -> [f32; 4] {
        let key = atlas_lookup_char(ch);
        self.uv_map
            .get(&key)
            .copied()
            .or_else(|| self.uv_map.get(&'?').copied())
            .expect("font atlas must contain fallback glyph")
    }
}

fn create_font_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bind_group_layout: &wgpu::BindGroupLayout,
    font_face: FontFaceChoice,
    smoothing: TextSmoothing,
) -> FontAtlas {
    if let Some((runtime_font, font_label)) = load_runtime_font(font_face) {
        return create_runtime_font_atlas(
            device,
            queue,
            bind_group_layout,
            &runtime_font,
            smoothing,
            font_label,
        );
    }

    create_bitmap_font_atlas(device, queue, bind_group_layout, smoothing)
}

fn create_runtime_font_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bind_group_layout: &wgpu::BindGroupLayout,
    font: &Font,
    smoothing: TextSmoothing,
    font_label: String,
) -> FontAtlas {
    const GLYPH_SIZE: f32 = 28.0;
    let glyphs = atlas_charset();
    let mut rendered = Vec::with_capacity(glyphs.len());
    let mut max_w = 0u32;
    let mut max_h = 0u32;

    for ch in &glyphs {
        let (metrics, bitmap) = font.rasterize(*ch, GLYPH_SIZE);
        max_w = max_w.max(metrics.width as u32);
        max_h = max_h.max(metrics.height as u32);
        rendered.push((*ch, metrics, bitmap));
    }

    let cell_w = max_w.max(12) + 4;
    let cell_h = max_h.max(16) + 4;
    let cols = 16u32;
    let rows = (glyphs.len() as u32).div_ceil(cols);
    let atlas_w = cols * cell_w;
    let atlas_h = rows * cell_h;
    let mut bytes = vec![0u8; (atlas_w * atlas_h) as usize];
    let mut uv_map = HashMap::new();

    for (index, (ch, metrics, bitmap)) in rendered.iter().enumerate() {
        let col = index as u32 % cols;
        let row = index as u32 / cols;
        let origin_x = col * cell_w + ((cell_w - metrics.width as u32) / 2);
        let origin_y = row * cell_h + ((cell_h - metrics.height as u32) / 2);

        for y in 0..metrics.height as u32 {
            for x in 0..metrics.width as u32 {
                let src = bitmap[(y * metrics.width as u32 + x) as usize];
                let dst_x = origin_x + x;
                let dst_y = origin_y + y;
                bytes[(dst_y * atlas_w + dst_x) as usize] = src;
            }
        }

        uv_map.insert(
            *ch,
            [
                (col * cell_w) as f32 / atlas_w as f32,
                (row * cell_h) as f32 / atlas_h as f32,
                ((col + 1) * cell_w) as f32 / atlas_w as f32,
                ((row + 1) * cell_h) as f32 / atlas_h as f32,
            ],
        );
    }

    create_font_atlas_resources(
        device,
        queue,
        bind_group_layout,
        atlas_w,
        atlas_h,
        bytes,
        uv_map,
        true,
        smoothing,
        font_label,
    )
}

fn create_bitmap_font_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bind_group_layout: &wgpu::BindGroupLayout,
    smoothing: TextSmoothing,
) -> FontAtlas {
    const CELL_W: u32 = 8;
    const CELL_H: u32 = 10;
    const COLS: u32 = 16;
    let glyphs = atlas_charset();
    let rows = (glyphs.len() as u32).div_ceil(COLS);
    let atlas_w = COLS * CELL_W;
    let atlas_h = rows * CELL_H;
    let mut bytes = vec![0u8; (atlas_w * atlas_h) as usize];
    let mut uv_map = HashMap::new();

    for (index, ch) in glyphs.iter().enumerate() {
        let col = index as u32 % COLS;
        let row = index as u32 / COLS;
        let origin_x = col * CELL_W + 1;
        let origin_y = row * CELL_H + 1;
        for (bitmap_row, pattern) in suzaku_map::ime::gpu::glyph_bitmap(*ch).iter().enumerate() {
            for bitmap_col in 0..5 {
                if (pattern >> (4 - bitmap_col)) & 1 == 1 {
                    let x = origin_x + bitmap_col;
                    let y = origin_y + bitmap_row as u32;
                    bytes[(y * atlas_w + x) as usize] = 255;
                }
            }
        }
        uv_map.insert(
            *ch,
            [
                (col * CELL_W) as f32 / atlas_w as f32,
                (row * CELL_H) as f32 / atlas_h as f32,
                ((col + 1) * CELL_W) as f32 / atlas_w as f32,
                ((row + 1) * CELL_H) as f32 / atlas_h as f32,
            ],
        );
    }

    create_font_atlas_resources(
        device,
        queue,
        bind_group_layout,
        atlas_w,
        atlas_h,
        bytes,
        uv_map,
        false,
        smoothing,
        "Fallback".to_string(),
    )
}

fn create_font_atlas_resources(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bind_group_layout: &wgpu::BindGroupLayout,
    atlas_w: u32,
    atlas_h: u32,
    bytes: Vec<u8>,
    uv_map: HashMap<char, [f32; 4]>,
    uses_runtime_font: bool,
    smoothing: TextSmoothing,
    font_label: String,
) -> FontAtlas {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("suzaku-font-atlas"),
        size: wgpu::Extent3d {
            width: atlas_w,
            height: atlas_h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        &bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(atlas_w),
            rows_per_image: Some(atlas_h),
        },
        wgpu::Extent3d {
            width: atlas_w,
            height: atlas_h,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("suzaku-font-atlas-sampler"),
        mag_filter: if smoothing == TextSmoothing::Smooth {
            wgpu::FilterMode::Linear
        } else {
            wgpu::FilterMode::Nearest
        },
        min_filter: if smoothing == TextSmoothing::Smooth {
            wgpu::FilterMode::Linear
        } else {
            wgpu::FilterMode::Nearest
        },
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("suzaku-font-atlas-bind-group"),
        layout: bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });

    FontAtlas {
        bind_group,
        uv_map,
        uses_runtime_font,
        font_label,
    }
}

fn atlas_charset() -> Vec<char> {
    let mut glyphs: Vec<char> = (32u8..=126u8).map(char::from).collect();
    glyphs.push('…');
    glyphs
}

fn atlas_lookup_char(ch: char) -> char {
    if ch.is_ascii_alphabetic() {
        ch.to_ascii_lowercase()
    } else if ch == '…' {
        '…'
    } else if ch.is_ascii() {
        ch
    } else {
        '?'
    }
}

fn load_runtime_font(font_face: FontFaceChoice) -> Option<(Font, String)> {
    for (path, label) in preferred_font_paths(font_face) {
        if let Ok(bytes) = fs::read(path) {
            if let Ok(font) = Font::from_bytes(bytes, fontdue::FontSettings::default()) {
                return Some((font, label.to_string()));
            }
        }
    }
    None
}

impl From<&PanelChromeState> for PersistedDisplaySettings {
    fn from(chrome: &PanelChromeState) -> Self {
        Self {
            text_scale: chrome.text_scale,
            candidate_density: chrome.candidate_density,
            preview_style: chrome.preview_style,
            font_face: chrome.font_face,
            text_spacing: chrome.text_spacing,
            text_smoothing: chrome.text_smoothing,
            llm_enabled: chrome.llm_enabled,
            llm_model: chrome.llm_model,
            llm_temperature: chrome.llm_temperature,
        }
    }
}

fn apply_display_settings(chrome: &mut PanelChromeState, settings: &PersistedDisplaySettings) {
    chrome.text_scale = settings.text_scale;
    chrome.candidate_density = settings.candidate_density;
    chrome.preview_style = settings.preview_style;
    chrome.font_face = settings.font_face;
    chrome.text_spacing = settings.text_spacing;
    chrome.text_smoothing = settings.text_smoothing;
    chrome.llm_enabled = settings.llm_enabled;
    chrome.llm_model = settings.llm_model;
    chrome.llm_temperature = settings.llm_temperature;
}

fn save_display_settings(settings: &PersistedDisplaySettings) -> std::io::Result<()> {
    let contents = format!(
        "text_scale={}\ncandidate_density={}\npreview_style={}\nfont_face={}\ntext_spacing={}\ntext_smoothing={}\nllm_enabled={}\nllm_model={}\nllm_temperature={}\n",
        encode_text_scale(settings.text_scale),
        encode_candidate_density(settings.candidate_density),
        encode_preview_style(settings.preview_style),
        encode_font_face(settings.font_face),
        encode_text_spacing(settings.text_spacing),
        encode_text_smoothing(settings.text_smoothing),
        if settings.llm_enabled {
            "true"
        } else {
            "false"
        },
        encode_llm_model(settings.llm_model),
        encode_llm_temperature(settings.llm_temperature),
    );
    let path = display_settings_path();
    ensure_settings_parent(&path)?;
    fs::write(path, contents)
}

fn load_display_settings() -> Option<PersistedDisplaySettings> {
    let contents = fs::read_to_string(display_settings_path()).ok()?;
    let mut settings = PersistedDisplaySettings {
        text_scale: DisplayTextScale::Medium,
        candidate_density: CandidateDensity::Cozy,
        preview_style: PreviewStyle::Compact,
        font_face: FontFaceChoice::Auto,
        text_spacing: TextSpacing::Normal,
        text_smoothing: TextSmoothing::Smooth,
        llm_enabled: false,
        llm_model: LlmModelPreset::Llama32_3b,
        llm_temperature: LlmTemperaturePreset::Balanced,
    };

    for line in contents.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "text_scale" => {
                if let Some(parsed) = decode_text_scale(value.trim()) {
                    settings.text_scale = parsed;
                }
            }
            "candidate_density" => {
                if let Some(parsed) = decode_candidate_density(value.trim()) {
                    settings.candidate_density = parsed;
                }
            }
            "preview_style" => {
                if let Some(parsed) = decode_preview_style(value.trim()) {
                    settings.preview_style = parsed;
                }
            }
            "font_face" => {
                if let Some(parsed) = decode_font_face(value.trim()) {
                    settings.font_face = parsed;
                }
            }
            "text_spacing" => {
                if let Some(parsed) = decode_text_spacing(value.trim()) {
                    settings.text_spacing = parsed;
                }
            }
            "text_smoothing" => {
                if let Some(parsed) = decode_text_smoothing(value.trim()) {
                    settings.text_smoothing = parsed;
                }
            }
            "llm_enabled" => {
                settings.llm_enabled = value.trim() == "true";
            }
            "llm_model" => {
                if let Some(parsed) = decode_llm_model(value.trim()) {
                    settings.llm_model = parsed;
                }
            }
            "llm_temperature" => {
                if let Some(parsed) = decode_llm_temperature(value.trim()) {
                    settings.llm_temperature = parsed;
                }
            }
            _ => {}
        }
    }

    Some(settings)
}

fn ensure_settings_parent(path: &PathBuf) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn encode_text_scale(value: DisplayTextScale) -> &'static str {
    match value {
        DisplayTextScale::Small => "small",
        DisplayTextScale::Medium => "medium",
        DisplayTextScale::Large => "large",
    }
}

fn decode_text_scale(value: &str) -> Option<DisplayTextScale> {
    match value {
        "small" => Some(DisplayTextScale::Small),
        "medium" => Some(DisplayTextScale::Medium),
        "large" => Some(DisplayTextScale::Large),
        _ => None,
    }
}

fn encode_candidate_density(value: CandidateDensity) -> &'static str {
    match value {
        CandidateDensity::Compact => "compact",
        CandidateDensity::Cozy => "cozy",
    }
}

fn decode_candidate_density(value: &str) -> Option<CandidateDensity> {
    match value {
        "compact" => Some(CandidateDensity::Compact),
        "cozy" => Some(CandidateDensity::Cozy),
        _ => None,
    }
}

fn encode_preview_style(value: PreviewStyle) -> &'static str {
    match value {
        PreviewStyle::Compact => "compact",
        PreviewStyle::Full => "full",
    }
}

fn decode_preview_style(value: &str) -> Option<PreviewStyle> {
    match value {
        "compact" => Some(PreviewStyle::Compact),
        "full" => Some(PreviewStyle::Full),
        _ => None,
    }
}

fn encode_font_face(value: FontFaceChoice) -> &'static str {
    match value {
        FontFaceChoice::Auto => "auto",
        FontFaceChoice::Monaco => "monaco",
        FontFaceChoice::Geneva => "geneva",
        FontFaceChoice::ArialUnicode => "arial_unicode",
    }
}

fn decode_font_face(value: &str) -> Option<FontFaceChoice> {
    match value {
        "auto" => Some(FontFaceChoice::Auto),
        "monaco" => Some(FontFaceChoice::Monaco),
        "geneva" => Some(FontFaceChoice::Geneva),
        "arial_unicode" => Some(FontFaceChoice::ArialUnicode),
        _ => None,
    }
}

fn encode_text_spacing(value: TextSpacing) -> &'static str {
    match value {
        TextSpacing::Tight => "tight",
        TextSpacing::Normal => "normal",
        TextSpacing::Relaxed => "relaxed",
    }
}

fn decode_text_spacing(value: &str) -> Option<TextSpacing> {
    match value {
        "tight" => Some(TextSpacing::Tight),
        "normal" => Some(TextSpacing::Normal),
        "relaxed" => Some(TextSpacing::Relaxed),
        _ => None,
    }
}

fn encode_text_smoothing(value: TextSmoothing) -> &'static str {
    match value {
        TextSmoothing::Sharp => "sharp",
        TextSmoothing::Smooth => "smooth",
    }
}

fn decode_text_smoothing(value: &str) -> Option<TextSmoothing> {
    match value {
        "sharp" => Some(TextSmoothing::Sharp),
        "smooth" => Some(TextSmoothing::Smooth),
        _ => None,
    }
}

fn encode_llm_model(value: LlmModelPreset) -> &'static str {
    match value {
        LlmModelPreset::Llama32_3b => "llama32_3b",
    }
}

fn decode_llm_model(value: &str) -> Option<LlmModelPreset> {
    match value {
        "llama32_3b" => Some(LlmModelPreset::Llama32_3b),
        _ => None,
    }
}

fn encode_llm_temperature(value: LlmTemperaturePreset) -> &'static str {
    match value {
        LlmTemperaturePreset::Focused => "focused",
        LlmTemperaturePreset::Balanced => "balanced",
        LlmTemperaturePreset::Expressive => "expressive",
    }
}

fn decode_llm_temperature(value: &str) -> Option<LlmTemperaturePreset> {
    match value {
        "focused" => Some(LlmTemperaturePreset::Focused),
        "balanced" => Some(LlmTemperaturePreset::Balanced),
        "expressive" => Some(LlmTemperaturePreset::Expressive),
        _ => None,
    }
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
    fn display_settings_round_trip_codec() {
        let settings = PersistedDisplaySettings {
            text_scale: DisplayTextScale::Large,
            candidate_density: CandidateDensity::Compact,
            preview_style: PreviewStyle::Full,
            font_face: FontFaceChoice::Geneva,
            text_spacing: TextSpacing::Relaxed,
            text_smoothing: TextSmoothing::Sharp,
            llm_enabled: false,
            llm_model: LlmModelPreset::Llama32_3b,
            llm_temperature: LlmTemperaturePreset::Expressive,
        };

        let encoded = format!(
            "text_scale={}\ncandidate_density={}\npreview_style={}\nfont_face={}\ntext_spacing={}\ntext_smoothing={}\nllm_enabled={}\nllm_model={}\nllm_temperature={}\n",
            encode_text_scale(settings.text_scale),
            encode_candidate_density(settings.candidate_density),
            encode_preview_style(settings.preview_style),
            encode_font_face(settings.font_face),
            encode_text_spacing(settings.text_spacing),
            encode_text_smoothing(settings.text_smoothing),
            if settings.llm_enabled {
                "true"
            } else {
                "false"
            },
            encode_llm_model(settings.llm_model),
            encode_llm_temperature(settings.llm_temperature),
        );

        let mut decoded = PersistedDisplaySettings {
            text_scale: DisplayTextScale::Medium,
            candidate_density: CandidateDensity::Cozy,
            preview_style: PreviewStyle::Compact,
            font_face: FontFaceChoice::Auto,
            text_spacing: TextSpacing::Normal,
            text_smoothing: TextSmoothing::Smooth,
            llm_enabled: true,
            llm_model: LlmModelPreset::Llama32_3b,
            llm_temperature: LlmTemperaturePreset::Balanced,
        };

        for line in encoded.lines() {
            let (key, value) = line.split_once('=').expect("kv");
            match key {
                "text_scale" => decoded.text_scale = decode_text_scale(value).expect("scale"),
                "candidate_density" => {
                    decoded.candidate_density = decode_candidate_density(value).expect("density")
                }
                "preview_style" => {
                    decoded.preview_style = decode_preview_style(value).expect("preview")
                }
                "font_face" => decoded.font_face = decode_font_face(value).expect("font"),
                "text_spacing" => {
                    decoded.text_spacing = decode_text_spacing(value).expect("spacing")
                }
                "text_smoothing" => {
                    decoded.text_smoothing = decode_text_smoothing(value).expect("smooth")
                }
                "llm_enabled" => decoded.llm_enabled = value == "true",
                "llm_model" => decoded.llm_model = decode_llm_model(value).expect("llm model"),
                "llm_temperature" => {
                    decoded.llm_temperature = decode_llm_temperature(value).expect("llm temp")
                }
                _ => {}
            }
        }

        assert_eq!(decoded, settings);
    }

    #[test]
    fn panel_chrome_defaults_to_llm_disabled() {
        let chrome = PanelChromeState::default();

        assert!(!chrome.llm_enabled);
    }

    #[test]
    fn next_token_derivation_advances_after_selected_token() {
        let tokens = derive_next_token_candidates(
            "apple can",
            &[
                "apple can is ready as the next full sentence".into(),
                "apple can continue by tapping the next suggestion".into(),
                "apple can now expands into a complete candidate".into(),
            ],
            6,
        );

        assert!(
            tokens.starts_with(&["is".to_string(), "continue".to_string(), "now".to_string(),])
        );
        assert!(!tokens.iter().any(|token| token == "can"));
    }

    #[test]
    fn voice_controller_cycles_samples() {
        let mut voice = VoiceInputController::new();

        let first = voice.next_sample();
        let second = voice.next_sample();

        assert_ne!(first, second);
        assert!(!first.is_empty());
        assert!(!second.is_empty());
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
    fn handwriting_recognizer_returns_loop_candidates() {
        let stroke = vec![
            [10.0, 10.0],
            [20.0, 8.0],
            [28.0, 16.0],
            [26.0, 28.0],
            [16.0, 32.0],
            [8.0, 22.0],
            [10.0, 10.0],
        ];

        let candidates = recognize_handwriting_candidates(&[stroke]);

        assert!(candidates.iter().any(|candidate| candidate == "o"));
    }

    #[test]
    fn handwriting_recognizer_returns_cross_candidates_for_multi_stroke_input() {
        let candidates = recognize_handwriting_candidates(&[
            vec![[10.0, 10.0], [24.0, 24.0]],
            vec![[24.0, 10.0], [10.0, 24.0]],
        ]);

        assert_eq!(candidates.first().map(String::as_str), Some("t"));
    }
}
