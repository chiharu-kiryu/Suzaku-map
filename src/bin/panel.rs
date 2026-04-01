#![cfg(feature = "gpu")]

use std::error::Error;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use suzaku_map::ime::gpu::{
    CandidateQuad, InputMode, InteractionKind, PanelChromeState, RenderScene, VirtualKeyboardKey,
    WgpuCandidateRenderer,
};
use suzaku_map::ime::{CommitOptions, EngineConfig, InputSource, SignalState, XRTabletImeEngine};
use wgpu::SurfaceError;
use wgpu::util::DeviceExt;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
#[cfg(target_os = "macos")]
use winit::platform::macos::{
    ActivationPolicy, EventLoopBuilderExtMacOS, WindowAttributesExtMacOS,
};
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

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = build_event_loop()?;
    let mut app = PanelApp::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}

fn build_event_loop() -> Result<EventLoop<()>, winit::error::EventLoopError> {
    let mut builder = EventLoop::builder();
    #[cfg(target_os = "macos")]
    {
        builder.with_activation_policy(ActivationPolicy::Regular);
        builder.with_default_menu(true);
        builder.with_activate_ignoring_other_apps(true);
    }
    builder.build()
}

fn panel_window_attributes() -> WindowAttributes {
    let attrs = WindowAttributes::default()
        .with_title("Suzaku XR Candidate Panel")
        .with_inner_size(LogicalSize::new(420.0, 520.0))
        .with_resizable(true);

    #[cfg(target_os = "macos")]
    let attrs = attrs
        .with_title_hidden(true)
        .with_titlebar_transparent(true)
        .with_fullsize_content_view(true)
        .with_movable_by_window_background(true)
        .with_accepts_first_mouse(true)
        .with_tabbing_identifier("suzaku.xr.panel");

    attrs
}

#[derive(Default)]
struct PanelApp {
    state: Option<PanelState>,
}

impl ApplicationHandler for PanelApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(panel_window_attributes())
                .expect("create panel window"),
        );
        let state = pollster::block_on(PanelState::new(window)).expect("initialize panel state");
        self.state = Some(state);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        if state.window.id() != window_id {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size.width, size.height),
            WindowEvent::ScaleFactorChanged { .. } => state.window.request_redraw(),
            WindowEvent::CursorMoved { position, .. } => {
                state.cursor_position = Some((position.x as f32, position.y as f32));
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                state.modifiers = modifiers.state();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                state.select_at_cursor();
                state.window.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    if state.is_quit_shortcut(&event.physical_key) {
                        event_loop.exit();
                        return;
                    }

                    if let Some(text) = event.text.as_deref() {
                        state.handle_text_input(text);
                    }

                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::ArrowLeft) => {
                            state.chrome.move_caret_left();
                        }
                        PhysicalKey::Code(KeyCode::ArrowRight) => {
                            state.chrome.move_caret_right();
                        }
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
                            state.chrome.active_input_mode = InputMode::VirtualKeyboard;
                        }
                        PhysicalKey::Code(KeyCode::Digit2) => {
                            state.chrome.active_input_mode = InputMode::Dictation;
                        }
                        PhysicalKey::Code(KeyCode::Digit3) => {
                            state.chrome.active_input_mode = InputMode::Handwriting;
                        }
                        PhysicalKey::Code(KeyCode::Tab) => {
                            state.chrome.input_modes_expanded = !state.chrome.input_modes_expanded;
                        }
                        PhysicalKey::Code(KeyCode::KeyD) => {
                            state.engine.update_signal(SignalState {
                                pointer_precision: 0.2,
                                gaze_stability: 0.2,
                                host_intent_weight: 0.4,
                                source_confidence: 0.4,
                            });
                        }
                        PhysicalKey::Code(KeyCode::KeyR) => state.reset_signal(),
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
                    state.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(err) = state.render() {
                    match err {
                        SurfaceError::Lost | SurfaceError::Outdated => {
                            state.resize(state.size.width, state.size.height);
                        }
                        SurfaceError::OutOfMemory => event_loop.exit(),
                        SurfaceError::Timeout => {}
                        SurfaceError::Other => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = self.state.as_ref() {
            state.window.request_redraw();
        }
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

struct PanelState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    size: winit::dpi::PhysicalSize<u32>,
    renderer: WgpuCandidateRenderer,
    engine: XRTabletImeEngine,
    chrome: PanelChromeState,
    cursor_position: Option<(f32, f32)>,
    modifiers: ModifiersState,
}

impl PanelState {
    async fn new(window: Arc<Window>) -> Result<Self, Box<dyn Error>> {
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

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("suzaku-panel-shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("suzaku-panel-layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("suzaku-panel-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
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
                module: &shader,
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

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config: config.clone(),
            pipeline,
            size,
            renderer: WgpuCandidateRenderer::new(config.width as f32, config.height as f32),
            engine,
            chrome: PanelChromeState {
                seed_text: "ni hao".into(),
                input_modes_expanded: true,
                active_input_mode: InputMode::VirtualKeyboard,
                input_focused: true,
                caret_index: "ni hao".chars().count(),
                keyboard_shifted: false,
                keyboard_numeric: false,
            },
            cursor_position: None,
            modifiers: ModifiersState::default(),
        })
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
        let scene = self.renderer.build_panel_scene(&snapshot, &self.chrome);
        self.window
            .set_title(&window_title(&scene, &snapshot.committed_text));

        let vertices = build_vertices(&scene, self.config.width as f32, self.config.height as f32);
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("suzaku-panel-vertices"),
                contents: bytemuck::cast_slice(&vertices),
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

            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.draw(0..vertices.len() as u32, 0..1);
        }

        self.queue.submit([encoder.finish()]);
        output.present();
        Ok(())
    }

    fn select_at_cursor(&mut self) {
        let Some((x, y)) = self.cursor_position else {
            return;
        };

        let scene = self
            .renderer
            .build_panel_scene(&self.engine.snapshot(), &self.chrome);
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
            self.refresh_seed();
        }
    }

    fn backspace_seed(&mut self) {
        self.chrome.backspace();
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
    }

    fn is_quit_shortcut(&self, key: &PhysicalKey) -> bool {
        matches!(key, PhysicalKey::Code(KeyCode::KeyQ))
            && if cfg!(target_os = "macos") {
                self.modifiers.super_key()
            } else {
                self.modifiers.control_key()
            }
    }
}

fn build_vertices(scene: &RenderScene, width: f32, height: f32) -> Vec<PanelVertex> {
    let total_quads = scene.quads.len() + scene.text_quads.len();
    let mut vertices = Vec::with_capacity(total_quads * 6);

    for quad in &scene.quads {
        push_quad_vertices(&mut vertices, quad, width, height);
    }

    for quad in &scene.text_quads {
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

fn window_title(scene: &RenderScene, committed_text: &str) -> String {
    let selected = scene.selected_label.as_deref().unwrap_or("no candidate");
    format!(
        "Suzaku XR Candidate Panel | selected: {selected} | draft: {} | committed: {committed_text} | keys: 1/2/3 seed, arrows move, D degrade, R reset, Space commit",
        scene.draft_text
    )
}

#[allow(dead_code)]
fn _quad_debug(_quad: &CandidateQuad) {}
