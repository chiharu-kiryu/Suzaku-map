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
            last_handwriting_summary: None,
            composition_base_seed,
            selected_next_tokens: Vec::new(),
        };
        if kind == PanelWindowKind::Main {
            state.reconfigure_llama_plugin();
        }

        Ok(state)
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
}
