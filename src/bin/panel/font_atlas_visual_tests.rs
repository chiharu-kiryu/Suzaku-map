//! Opt-in GPU readback of synthetic candidates through the actual panel shaders and atlas.
//! Does not capture the desktop, activate an IME, read live text or request a model.
use super::*;
use crate::render::{PanelVertex, TextVertex, build_frame_vertices};
use suzaku_map::ime::gpu::{InputMode, PanelChromeState, ThemePreset, WgpuCandidateRenderer};
use suzaku_map::ime::{EngineConfig, XRTabletImeEngine};
use wgpu::util::DeviceExt;

#[derive(Clone, Copy, Debug, PartialEq)]
enum PreviewKind {
    Panel,
    Orb,
    Settings,
}

fn write_bitmap(path: &std::path::Path, width: u32, height: u32, stride: u32, rgba: &[u8]) {
    let mut bytes = vec![0; 54 + (width * height * 4) as usize];
    bytes[..2].copy_from_slice(b"BM");
    let length = bytes.len() as u32;
    bytes[2..6].copy_from_slice(&length.to_le_bytes());
    bytes[10..14].copy_from_slice(&54u32.to_le_bytes());
    bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&(width as i32).to_le_bytes());
    bytes[22..26].copy_from_slice(&(-(height as i32)).to_le_bytes());
    bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&32u16.to_le_bytes());
    let preview_background = suzaku_map::ime::gpu::srgb_color(0xF5EEE7);
    for y in 0..height as usize {
        for x in 0..width as usize {
            let source = y * stride as usize + x * 4;
            let dest = 54 + (y * width as usize + x) * 4;
            // BMP previews use a warm backdrop. The raw GPU alpha is asserted separately.
            let alpha = rgba[source + 3] as f32 / 255.0;
            for channel in 0..3 {
                let value = rgba[source + channel] as f32 / 255.0;
                let linear = if value <= 0.04045 {
                    value / 12.92
                } else {
                    ((value + 0.055) / 1.055).powf(2.4)
                };
                let composite = linear + preview_background[channel] * (1.0 - alpha);
                let encoded = if composite <= 0.0031308 {
                    composite * 12.92
                } else {
                    1.055 * composite.powf(1.0 / 2.4) - 0.055
                };
                bytes[dest + 2 - channel] = (encoded.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
            bytes[dest + 3] = 255;
        }
    }
    std::fs::write(path, bytes).unwrap();
}

#[test]
#[ignore = "requires a GPU adapter and an installed CJK font; optional SUZAKU_GLYPH_QA_DIR writes synthetic BMPs"]
fn multilingual_candidates_render_through_the_gpu_without_question_mark_fallback() {
    pollster::block_on(async {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .expect("GPU adapter");
        println!("Rendering adapter: {:?}", adapter.get_info());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .unwrap();
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let bindings = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
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
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let pipelines: Vec<_> = [(crate::SHADER, false), (crate::TEXT_SHADER, true)]
            .into_iter()
            .map(|(source, text)| {
                let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: None,
                    source: wgpu::ShaderSource::Wgsl(source.into()),
                });
                let bind_groups = if text { vec![&bindings] } else { vec![] };
                let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &bind_groups,
                    push_constant_ranges: &[],
                });
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: None,
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        buffers: &[if text {
                            TextVertex::desc()
                        } else {
                            PanelVertex::desc()
                        }],
                        compilation_options: Default::default(),
                    },
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
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
                })
            })
            .collect();
        let keyboard_cases = [
            ("zh-Hans", "nihao", FontFaceChoice::Auto, 1.0, true),
            ("en", "hel", FontFaceChoice::Auto, 1.0, true),
            ("en", "hel", FontFaceChoice::Auto, 1.0, false),
            ("en", "hel", FontFaceChoice::Auto, 0.75, false),
            ("en", "hel", FontFaceChoice::Auto, 1.25, true),
            ("en", "hel", FontFaceChoice::Auto, 1.5, false),
            ("ja", "nihongo", FontFaceChoice::Monaco, 1.0, true),
            ("zh-Hans", "nihaoshijie", FontFaceChoice::Monaco, 2.5, true),
            ("ja", "nihongo", FontFaceChoice::Monaco, 1.0, false),
        ]
        .into_iter()
        .map(|(language, seed, face, scale, expanded)| {
            (
                language,
                seed,
                face,
                scale,
                expanded,
                InputMode::VirtualKeyboard,
                suzaku_map::ime::gpu::DisplayTextScale::Medium,
            )
        });
        let handwriting_cases = [0.42, 0.9, 1.5, 2.5].into_iter().flat_map(|scale| {
            [
                suzaku_map::ime::gpu::DisplayTextScale::Medium,
                suzaku_map::ime::gpu::DisplayTextScale::Large,
            ]
            .into_iter()
            .map(move |text_scale| {
                (
                    "en",
                    "hel",
                    FontFaceChoice::Auto,
                    scale,
                    true,
                    InputMode::Handwriting,
                    text_scale,
                )
            })
        });
        let editor_cases = [0.42, 0.85].into_iter().map(|scale| {
            (
                "en",
                "Wi  hello  你好 日本語  a long editable phrase with repeated spaces  at the end",
                FontFaceChoice::Auto,
                scale,
                false,
                InputMode::VirtualKeyboard,
                suzaku_map::ime::gpu::DisplayTextScale::Large,
            )
        });
        let panel_cases = keyboard_cases
            .chain(handwriting_cases)
            .chain(editor_cases)
            .map(|case| (case, PreviewKind::Panel, ThemePreset::Suzaku));
        let extra_cases = [
            (PreviewKind::Orb, 0.096, ThemePreset::Suzaku),
            (PreviewKind::Orb, 0.144, ThemePreset::Suzaku),
            (PreviewKind::Orb, 0.096, ThemePreset::DeviceDark),
            (PreviewKind::Orb, 0.096, ThemePreset::HighContrast),
            (PreviewKind::Settings, 0.64, ThemePreset::Suzaku),
            (PreviewKind::Panel, 0.75, ThemePreset::Suzaku),
            (PreviewKind::Orb, 0.096, ThemePreset::Baihu),
            (PreviewKind::Orb, 0.144, ThemePreset::Baihu),
            (PreviewKind::Settings, 0.64, ThemePreset::Baihu),
            (PreviewKind::Panel, 0.75, ThemePreset::Baihu),
            (PreviewKind::Orb, 0.096, ThemePreset::Qinglong),
            (PreviewKind::Orb, 0.144, ThemePreset::Qinglong),
            (PreviewKind::Settings, 0.64, ThemePreset::Qinglong),
            (PreviewKind::Panel, 0.75, ThemePreset::Qinglong),
            (PreviewKind::Orb, 0.096, ThemePreset::Xuanwu),
            (PreviewKind::Orb, 0.144, ThemePreset::Xuanwu),
            (PreviewKind::Settings, 0.64, ThemePreset::Xuanwu),
            (PreviewKind::Panel, 0.75, ThemePreset::Xuanwu),
        ]
        .into_iter()
        .map(|(kind, scale, theme)| {
            (
                (
                    "en",
                    "hello",
                    FontFaceChoice::Auto,
                    scale,
                    true,
                    InputMode::Dictation,
                    suzaku_map::ime::gpu::DisplayTextScale::Medium,
                ),
                kind,
                theme,
            )
        });
        let guardian_cases = [
            ThemePreset::Baihu,
            ThemePreset::Qinglong,
            ThemePreset::Xuanwu,
        ]
        .into_iter()
        .flat_map(|theme| {
            [
                ("en", "hello", false, InputMode::VirtualKeyboard),
                ("en", "hello", true, InputMode::VirtualKeyboard),
                ("zh", "nihao", false, InputMode::VirtualKeyboard),
                ("ja", "konnichiha", false, InputMode::VirtualKeyboard),
                ("en", "hello", true, InputMode::Handwriting),
            ]
            .into_iter()
            .map(move |(language, seed, expanded, mode)| {
                (
                    (
                        language,
                        seed,
                        FontFaceChoice::Auto,
                        0.52,
                        expanded,
                        mode,
                        suzaku_map::ime::gpu::DisplayTextScale::Medium,
                    ),
                    PreviewKind::Panel,
                    theme,
                )
            })
        });
        for ((language, seed, face, scale, expanded, mode, text_scale), preview_kind, theme) in
            panel_cases.chain(extra_cases).chain(guardian_cases)
        {
            let width = (1000.0 * scale) as u32;
            let mut engine = XRTabletImeEngine::new(EngineConfig {
                default_language: language.into(),
                ..Default::default()
            });
            let snapshot = engine.seed(seed);
            if language != "en" {
                assert!(
                    snapshot.candidate_labels[0]
                        .chars()
                        .any(|ch| unicode_width::UnicodeWidthChar::width(ch) == Some(2)),
                    "CJK QA must exercise actual converted candidates"
                );
            }
            let previews = suzaku_map::panel_support::composition_candidate_previews(
                seed,
                language,
                engine.candidates(),
                6,
                4,
            );
            let (source_indices, candidates) = previews.sentences.into_iter().unzip();
            let chrome = PanelChromeState {
                compact_mode: false,
                input_modes_expanded: expanded,
                active_input_mode: mode,
                text_scale,
                theme_preset: theme,
                settings_open: preview_kind == PreviewKind::Settings,
                voice_backend_label: "Local speech".into(),
                voice_transcript: "Hello, welcome to Suzaku.".into(),
                handwriting_candidates: vec!["O".into(), "A".into()],
                handwriting_hint: "Try a clearer trace, undo a stroke, or tap Clear.".into(),
                seed_text: seed.into(),
                input_focused: seed.chars().count() > 40,
                caret_index: seed.chars().count(),
                sentence_candidates: candidates,
                sentence_candidate_source_indices: source_indices,
                next_token_candidates: previews
                    .next_tokens
                    .into_iter()
                    .map(|edit| edit.label)
                    .collect(),
                font_face: face,
                window_scale: scale,
                ..Default::default()
            };
            let height = match preview_kind {
                PreviewKind::Orb => width,
                PreviewKind::Settings => 760,
                PreviewKind::Panel => WgpuCandidateRenderer::new(width as f32, 1.0)
                    .preferred_input_panel_height(&chrome)
                    as u32,
            };
            let renderer = WgpuCandidateRenderer::new(width as f32, height as f32);
            let mut scene = match preview_kind {
                PreviewKind::Panel => {
                    renderer.build_panel_scene(&snapshot, &chrome, None, None, None, None)
                }
                PreviewKind::Orb => renderer.build_compact_scene(&snapshot, &chrome, false, false),
                PreviewKind::Settings => renderer.build_settings_scene(&chrome, None),
            };
            let mut atlas = create_font_atlas(
                &device,
                &queue,
                &bindings,
                face,
                TextSmoothing::Smooth,
                scale,
            );
            let mut characters: Vec<_> = scene.atlas_glyphs.iter().map(|glyph| glyph.ch).collect();
            atlas.ensure_glyphs(&queue, language, characters.iter().copied());
            for _ in 0..3 {
                scene =
                    suzaku_map::ime::gpu::with_font_metrics(atlas.layout_metrics.clone(), || {
                        match preview_kind {
                            PreviewKind::Panel => renderer
                                .build_panel_scene(&snapshot, &chrome, None, None, None, None),
                            PreviewKind::Orb => {
                                renderer.build_compact_scene(&snapshot, &chrome, false, false)
                            }
                            PreviewKind::Settings => renderer.build_settings_scene(&chrome, None),
                        }
                    });
                characters = scene.atlas_glyphs.iter().map(|glyph| glyph.ch).collect();
                let revision = atlas.layout_revision;
                atlas.ensure_glyphs(&queue, language, characters.iter().copied());
                if revision == atlas.layout_revision {
                    break;
                }
            }
            assert!(
                atlas.missing.is_empty(),
                "visible glyphs lack font coverage for {language}"
            );
            for ch in snapshot
                .candidate_labels
                .iter()
                .flat_map(|text| text.chars())
            {
                if unicode_width::UnicodeWidthChar::width(ch) == Some(2) && characters.contains(&ch)
                {
                    assert_ne!(atlas.uv_for(ch), atlas.uv_for(MISSING));
                }
            }
            let slots = atlas.next_slot;
            let mapping = atlas.uv_map.clone();
            atlas.ensure_glyphs(&queue, language, characters);
            assert_eq!(
                slots, atlas.next_slot,
                "cached frame must not rasterize or allocate"
            );
            assert_eq!(mapping, atlas.uv_map, "cached UVs must remain stable");
            let vertices = build_frame_vertices(&scene, &[], width as f32, height as f32, |ch| {
                atlas.uv_for(ch)
            });
            let shape_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&vertices.shapes),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let text_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&vertices.text),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let output = device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = output.create_view(&Default::default());
            let stride = (width * 4).div_ceil(256) * 256;
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: u64::from(stride) * u64::from(height),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(if preview_kind == PreviewKind::Orb {
                                wgpu::Color::TRANSPARENT
                            } else {
                                wgpu::Color::WHITE
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                for layer in vertices.layers {
                    pass.set_pipeline(&pipelines[0]);
                    pass.set_vertex_buffer(0, shape_buffer.slice(..));
                    pass.draw(layer.shapes, 0..1);
                    if !layer.text.is_empty() {
                        pass.set_pipeline(&pipelines[1]);
                        pass.set_bind_group(0, &atlas.bind_group, &[]);
                        pass.set_vertex_buffer(0, text_buffer.slice(..));
                        pass.draw(layer.text, 0..1);
                    }
                }
            }
            encoder.copy_texture_to_buffer(
                output.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(stride),
                        rows_per_image: Some(height),
                    },
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            queue.submit([encoder.finish()]);
            let (tx, rx) = std::sync::mpsc::channel();
            readback
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    tx.send(result).unwrap();
                });
            device
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(std::time::Duration::from_secs(15)),
                })
                .unwrap();
            rx.recv_timeout(std::time::Duration::from_secs(1))
                .unwrap()
                .unwrap();
            let bytes = readback.slice(..).get_mapped_range();
            assert!(
                bytes.iter().any(|byte| *byte != 255),
                "GPU output must contain actual pixels"
            );
            if preview_kind == PreviewKind::Orb {
                assert_eq!(bytes[3], 0, "orb corners must stay transparent");
                assert!(
                    bytes
                        .chunks_exact(4)
                        .any(|pixel| pixel[3] > 0 && pixel[3] < 255),
                    "curves need antialiased edge coverage"
                );
            }
            if let Some(directory) = std::env::var_os("SUZAKU_GLYPH_QA_DIR") {
                let directory = PathBuf::from(directory);
                std::fs::create_dir_all(&directory).unwrap();
                write_bitmap(
                    &directory.join(format!(
                        "{language}-{scale}-{expanded}-{mode:?}-{text_scale:?}-{preview_kind:?}-{theme:?}.bmp"
                    )),
                    width,
                    height,
                    stride,
                    &bytes,
                );
            }
            println!(
                "{language} {scale}x {mode:?} {text_scale:?}: {} cached glyphs, {}, missing={}",
                atlas.next_slot,
                atlas.font_label,
                atlas.missing.len()
            );
            drop(bytes);
            readback.unmap();
            // Exercise bounded eviction and recovery to the actual scene.
            atlas.ensure_glyphs(
                &queue,
                language,
                (0x3400..0x3400 + 5000).filter_map(char::from_u32),
            );
            assert!(atlas.next_slot <= atlas.grid.capacity && atlas.uv_map.len() <= 4096);
            atlas.ensure_glyphs(
                &queue,
                language,
                scene.atlas_glyphs.iter().map(|glyph| glyph.ch),
            );
            assert!(
                scene
                    .atlas_glyphs
                    .iter()
                    .all(|glyph| !atlas.missing.contains(&glyph.ch))
            );
        }
        assert!(device.pop_error_scope().await.is_none());
    });
}
