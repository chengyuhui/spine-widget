// #![cfg_attr(
//     all(target_os = "windows", not(debug_assertions)),
//     windows_subsystem = "windows"
// )]

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use anyhow::Result;
use image::GenericImageView;
use spine::{atlas::AtlasPage, spine_init, AttachmentType, SpineCallbacks};
use texture::{Texture, TextureConfig};

use trayicon::{MenuBuilder, MenuItem, TrayIcon, TrayIconBuilder};
use wgpu::IndexFormat;
use window_ext::SpineWidgetWindowExt;
use winit::{
    dpi::{LogicalSize, PhysicalPosition},
    event::*,
    event_loop::{ControlFlow, EventLoop},
    platform::windows::{WindowBuilderExtWindows, WindowExtWindows},
    window::{Window, WindowBuilder},
};

mod buffer;
mod config;
mod display;
mod hook;
mod scaling;
mod spine_state;
mod texture;
mod utils;
mod vertex;
mod window_ext;

use crate::hook::KeyboardHook;
use buffer::ScratchBuffers;
use config::Config;
use display::Display;
use scaling::ScalingState;
use spine_state::SpineState;
use utils::*;
use vertex::Vertex;

struct SpineCb;
impl SpineCallbacks for SpineCb {
    type Texture = Texture;

    type LoadTextureError = anyhow::Error;
    type LoadFileError = anyhow::Error;

    fn load_texture(
        path: &str,
        atlas: &AtlasPage,
    ) -> Result<(Texture, u32, u32), Self::LoadTextureError> {
        let mut img = image::load_from_memory(&load_file_packed(path)?)?;

        let mask_path = PathBuf::from(path.replace(".png", "[alpha].png").as_str());
        if let Ok(mask_buf) = load_file_packed(mask_path.to_str().unwrap()) {
            let mask_img = image::load_from_memory(&mask_buf)?;

            let base = img.as_mut_rgba8().unwrap();
            let mask = mask_img.as_rgba8().unwrap();

            for (b, m) in base.pixels_mut().zip(mask.pixels()) {
                b[3] = m[0];
            }
        }

        let width = img.width();
        let height = img.height();

        Ok((
            Texture::new(
                img,
                TextureConfig {
                    mag_filter: atlas.mag_filter(),
                    min_filter: atlas.min_filter(),
                    u_wrap: atlas.u_wrap(),
                    v_wrap: atlas.v_wrap(),
                },
            ),
            width,
            height,
        ))
    }

    fn load_file(path: &str) -> Result<Vec<u8>, Self::LoadFileError> {
        load_file_packed(path)
    }
}
spine_init!(SpineCb);

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum UserEvent {
    ToggleWindowed,
    ToggleClickPassthrough,
    SetOpacity(u8),
    About,
    Exit,
    GlobalKey {
        state: ElementState,
        vk_code: u32,
        modifiers: ModifiersState,
    },
}

struct State {
    window: Window,

    display: Display,
    size: winit::dpi::PhysicalSize<u32>,
    scale_factor: f64,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    texture_bind_group_layout: wgpu::BindGroupLayout,

    scaling_state: ScalingState,
    /// Opacity value from 0 to 100.
    opacity: u8,

    spine: SpineState,
    world_vertices: Vec<[f32; 2]>,
    scratch_buffers: ScratchBuffers,

    pressed_keys: HashSet<VirtualKeyCode>,
    modifiers_state: ModifiersState,

    windowed: bool,
    click_passthrough: bool,

    tray: TrayIcon<UserEvent>,
}

impl State {
    // Creating some of the wgpu types requires async code
    async fn new(
        window: Window,
        event_loop: &EventLoop<UserEvent>,
        config: &config::Config,
    ) -> Self {
        let size = window.inner_size();

        let display = Display::new(&window).await;
        let device = &display.device;

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                        ty: wgpu::BindingType::Sampler {
                            // This is only for TextureSampleType::Depth
                            comparison: false,
                            // This should be true if the sample_type of the texture is:
                            //     TextureSampleType::Float { filterable: true }
                            // Otherwise you'll get an error.
                            filtering: true,
                        },
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let (scaling_state, scaling_bind_group_layout) = ScalingState::new(&window, device, config);

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&texture_bind_group_layout, &scaling_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "main",
                targets: &[wgpu::ColorTargetState {
                    format: display.config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                }],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList, // Three vertices -> triangle
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw, // 2.
                cull_mode: None,
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLAMPING
                clamp_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: None, // No depth/stencil buffer.
            multisample: wgpu::MultisampleState {
                count: 1,                         // 2.
                mask: !0,                         // All of them.
                alpha_to_coverage_enabled: false, // No anti-aliasing for now.
            },
        });

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vertex Buffer"),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            size: 1024 * 128,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Index Buffer"),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            size: 1024 * 128,
            mapped_at_creation: false,
        });

        let spine = SpineState::new(config).unwrap();

        let tray = TrayIconBuilder::new()
            .icon_from_buffer(include_bytes!("tray.ico"))
            .sender_winit(event_loop.create_proxy())
            .build()
            .unwrap();

        let scale_factor = window.scale_factor();

        let mut r = Self {
            window,

            display,
            size,
            scale_factor,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            texture_bind_group_layout,

            scaling_state,
            opacity: 100,

            spine,
            world_vertices: Vec::new(),
            scratch_buffers: ScratchBuffers::new(),

            pressed_keys: HashSet::new(),
            modifiers_state: Default::default(),

            windowed: false,
            click_passthrough: true,

            tray,
        };

        r.set_windowed(false);
        r.set_click_passthrough(true);

        r.update_tray();

        r
    }

    fn update_tray(&mut self) {
        let tray = &mut self.tray;
        let _ = tray.set_menu(
            &MenuBuilder::new()
                .checkable("窗口化/调整大小", self.windowed, UserEvent::ToggleWindowed)
                .checkable(
                    "鼠标点击穿透",
                    self.click_passthrough,
                    UserEvent::ToggleClickPassthrough,
                )
                .submenu("不透明度", {
                    let mut submenu = MenuBuilder::new();

                    for i in (10..=100).step_by(10) {
                        submenu = submenu.checkable(
                            &format!("{}%", i),
                            self.opacity == i,
                            UserEvent::SetOpacity(i as u8),
                        );
                    }

                    submenu
                })
                .separator()
                .with(MenuItem::Item {
                    id: UserEvent::About,
                    name: format!("Mon3tr-Widget {}", env!("VERGEN_GIT_SEMVER")),
                    disabled: true,
                    icon: None,
                })
                .item("退出", UserEvent::Exit),
        );
    }

    fn set_windowed(&mut self, windowed: bool) {
        self.window.set_decorations(windowed); // Hide window borders.

        self.windowed = windowed;
        self.update_tray();
    }

    fn toggle_windowed(&mut self) {
        self.set_windowed(!self.windowed);
    }

    fn set_click_passthrough(&mut self, click_passthrough: bool) {
        self.window.set_click_passthrough(click_passthrough);
        self.window.set_enable(!click_passthrough); // Also hides window from task switcher if disabled.

        self.click_passthrough = click_passthrough;
        self.update_tray();
    }

    fn toggle_click_passthrough(&mut self) {
        self.set_click_passthrough(!self.click_passthrough);
    }

    /// Set opacity of the model, from 0 to 100.
    fn set_opacity(&mut self, opacity: u8) {
        self.opacity = opacity;
        self.update_tray();
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;

            self.display.resize(new_size.width, new_size.height);

            self.scaling_state.resize(new_size, self.scale_factor);
        }
    }

    fn scale(&mut self, scale_factor: f64) {
        self.scale_factor = scale_factor;
        self.scaling_state.resize(self.size, scale_factor);
    }

    fn input(&mut self, event: &WindowEvent, config: &Config) -> bool {
        let window = &self.window;

        match event {
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(keycode),
                        ..
                    },
                ..
            } => {
                if self.pressed_keys.contains(keycode) {
                    return true;
                }
                self.pressed_keys.insert(*keycode);

                match (self.modifiers_state, keycode) {
                    (ModifiersState::CTRL, VirtualKeyCode::Equals) => {
                        // "=+" on main keyboard
                        *self.scaling_state.model_scaling_mut() += 0.1;
                        return true;
                    }
                    (ModifiersState::CTRL, VirtualKeyCode::Minus) => {
                        // "-_" on main keyboard
                        *self.scaling_state.model_scaling_mut() -= 0.1;
                        return true;
                    }
                    _ => {}
                }

                for action in &config.actions {
                    if action.trigger == *keycode {
                        let mut last_length = 0.0;
                        let mut is_first = true;
                        for item in &action.sequence {
                            if is_first {
                                is_first = false;
                                self.spine
                                    .anim
                                    .set_animation_by_name(0, &item.name, item.loop_);
                            } else {
                                self.spine.anim.add_animation_by_name(
                                    0,
                                    &item.name,
                                    item.loop_,
                                    last_length,
                                );
                            }
                            last_length = item.length.unwrap_or(0.0);
                        }

                        // Return to idle
                        if let (true, Some(idle_name)) =
                            (action.return_to_idle, &config.idle_animation)
                        {
                            self.spine
                                .anim
                                .add_animation_by_name(0, idle_name, true, last_length);
                        }
                    }
                }
                true
            }
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: ElementState::Released,
                        virtual_keycode: Some(keycode),
                        ..
                    },
                ..
            } => self.pressed_keys.remove(keycode),
            WindowEvent::ModifiersChanged(mod_state) => {
                self.modifiers_state = *mod_state;
                true
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state: ElementState::Pressed,
                ..
            } => {
                let _ = window.drag_window();
                true
            }
            _ => false,
        }
    }

    fn update(&mut self) {
        self.scaling_state.write_to_gpu(&self.display.queue);
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.spine.prepare_render();
        let opacity = self.opacity as f32 / 100.0;

        let queue = &self.display.queue;

        let output = self.display.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut textures = HashMap::new();

        let skel_tint = self.spine.skel.tint_color();
        for slot in self.spine.skel.slots() {
            let attachment = if let Some(a) = slot.attachment() {
                a
            } else {
                continue;
            };

            let slot_tint = slot.tint_color();
            let tint = [
                skel_tint[0] * slot_tint[0],
                skel_tint[1] * slot_tint[1],
                skel_tint[2] * slot_tint[2],
                skel_tint[3] * slot_tint[3] * opacity,
            ];

            let to_vertex = |(uv, pos): ([f32; 2], [f32; 2])| Vertex {
                position: pos,
                tex_coords: uv,
                tint,
            };

            match attachment.as_inner() {
                AttachmentType::Region(region) => {
                    let tex = if let Some(tex) =
                        unsafe { region.atlas_region().page().render_object::<Texture>() }
                    {
                        tex
                    } else {
                        continue;
                    };
                    let tex_id = tex.id();

                    tex.initialize(&self.display, &self.texture_bind_group_layout, None)
                        .unwrap();
                    textures.entry(tex_id).or_insert(tex);

                    let (scratch_vb, scratch_ib) = self.scratch_buffers.get_buffers_mut(tex_id);

                    let offset = scratch_vb.len() as u16;
                    region.compute_world_vertices(&mut self.world_vertices);
                    let new_vertices = self
                        .world_vertices
                        .iter()
                        .enumerate()
                        .map(|(i, p)| {
                            let (u, v) = region.uv(i);
                            ([u, v], *p)
                        })
                        .map(to_vertex);
                    scratch_vb.extend(new_vertices);

                    let new_indices = [0, 1, 2, 2, 3, 0].iter().map(|i| i + offset);
                    scratch_ib.extend(new_indices);
                }
                AttachmentType::Mesh(mesh) => {
                    let tex = if let Some(tex) =
                        unsafe { mesh.atlas_region().page().render_object::<Texture>() }
                    {
                        tex
                    } else {
                        continue;
                    };
                    let tex_id = tex.id();

                    tex.initialize(&self.display, &self.texture_bind_group_layout, None)
                        .unwrap();
                    textures.entry(tex_id).or_insert(tex);

                    let (scratch_vb, scratch_ib) = self.scratch_buffers.get_buffers_mut(tex_id);

                    let offset = scratch_vb.len() as u16;
                    mesh.compute_world_vertices(&mut self.world_vertices);
                    let new_vertices = self
                        .world_vertices
                        .iter()
                        .enumerate()
                        .map(|(i, p)| {
                            let (u, v) = mesh.uv(i);
                            ([u, v], *p)
                        })
                        .map(to_vertex);
                    scratch_vb.extend(new_vertices);

                    let new_indices = mesh.indices().iter().map(|i| i + offset);
                    scratch_ib.extend(new_indices);
                }
                _ => {}
            }
        }

        let mut cleared = false;

        for (tex_id, vb, ib) in self.scratch_buffers.iter_mut() {
            {
                let len = vb.len();
                let vb_pad = len % 4;
                if vb_pad != 0 {
                    vb.resize(vb.len() + 4 - vb_pad, Default::default());
                }
            }
            let ib_len = {
                let len = ib.len();
                let ib_pad = len % 4;
                if ib_pad != 0 {
                    ib.resize(ib.len() + 4 - ib_pad, 0);
                }
                len
            };

            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(vb));
            queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(ib));

            let mut encoder =
                self.display
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Render Encoder"),
                    });

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if cleared {
                            wgpu::LoadOp::Load
                        } else {
                            cleared = true;
                            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                        },
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(
                0,
                &textures.get(&tex_id).unwrap().get_texture().bind_group,
                &[],
            );
            render_pass.set_bind_group(1, self.scaling_state.bind_group(), &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint16);

            render_pass.draw_indexed(0..ib_len as u32, 0, 0..1);

            drop(render_pass);
            queue.submit(std::iter::once(encoder.finish()));
        }

        output.present();

        self.scratch_buffers.clear();

        Ok(())
    }

    fn request_redraw(&mut self) {
        self.window.request_redraw();
    }
}

fn create_window<T>(event_loop: &EventLoop<T>, owner: &Window, config: &Config) -> Window {
    let window = WindowBuilder::new()
        .with_title("Mon3tr-Widget")
        .with_always_on_top(true)
        .with_decorations(false)
        .with_transparent(true)
        .with_inner_size(LogicalSize::new(config.window_size.0, config.window_size.1))
        .with_owner_window(owner.hwnd() as _)
        .build(event_loop)
        .unwrap();

    window.set_outer_position(PhysicalPosition::new(
        config.window_position.0,
        config.window_position.1,
    ));

    window
}

/// This window is required to hide the main window from the taskbar.
fn create_owner_window<Evt>(event_loop: &EventLoop<Evt>) -> Window {
    WindowBuilder::new()
        .with_visible(false)
        .build(event_loop)
        .unwrap()
}

fn init_logging() {
    use fern::colors::ColoredLevelConfig;

    let colors = ColoredLevelConfig::new();
    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                colors.color(record.level()),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(std::io::stdout())
        .apply()
        .unwrap();
}

fn main() {
    // #[cfg(debug_assertions)]
    init_logging();

    log::info!(
        "Mon3tr-Widget {} {} built {}",
        env!("VERGEN_GIT_SEMVER"),
        env!("VERGEN_CARGO_PROFILE"),
        env!("VERGEN_BUILD_TIMESTAMP")
    );
    log::info!(
        "Toolchain: {}@{}({}) target {}",
        env!("VERGEN_RUSTC_CHANNEL"),
        env!("VERGEN_RUSTC_SEMVER"),
        env!("VERGEN_RUSTC_COMMIT_DATE"),
        env!("VERGEN_CARGO_TARGET_TRIPLE")
    );

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.yml".to_string());
    let mut config = config::load(&config_path).unwrap();

    let event_loop = EventLoop::<UserEvent>::with_user_event();
    let owner_window = create_owner_window(&event_loop);
    let window = create_window(&event_loop, &owner_window, &config);
    let keyboard_hook = KeyboardHook::new(event_loop.create_proxy());

    let mut state = pollster::block_on(State::new(window, &event_loop, &config));

    let mut close_requested = false;

    event_loop.run(move |event, _, control_flow| {
        let _ = owner_window;
        let _ = keyboard_hook;

        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == state.window.id() => {
                if !state.input(event, &config) {
                    match event {
                        WindowEvent::CloseRequested => {
                            close_requested = true;
                        }
                        // Resize
                        WindowEvent::Resized(physical_size) => {
                            state.resize(*physical_size);
                        }
                        // Scale factor updated /  moved to another screen
                        WindowEvent::ScaleFactorChanged {
                            new_inner_size,
                            scale_factor,
                        } => {
                            // new_inner_size is &&mut so we have to dereference it twice
                            state.resize(**new_inner_size);
                            state.scale(*scale_factor);
                        }
                        _ => {}
                    }
                }
            }
            Event::RedrawRequested(window_id) if window_id == state.window.id() => {
                state.update();
                match state.render() {
                    Ok(_) => {}
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => eprintln!("{:?}", e),
                }
            }
            Event::MainEventsCleared => {
                state.request_redraw();

                if close_requested {
                    // Save window parameters
                    let logical_size = state
                        .window
                        .inner_size()
                        .to_logical::<f64>(state.window.scale_factor());
                    config.window_size = (logical_size.width, logical_size.height);

                    if let Ok(pos) = state.window.outer_position() {
                        let pos = pos.cast();
                        config.window_position = (pos.x, pos.y);
                    }

                    config.scale = state.scaling_state.model_scaling();

                    let _ = config::save(&config, &config_path);

                    *control_flow = ControlFlow::Exit;
                }
            }
            Event::UserEvent(e) => match e {
                UserEvent::ToggleWindowed => {
                    state.toggle_windowed();
                }
                UserEvent::ToggleClickPassthrough => {
                    state.toggle_click_passthrough();
                }
                UserEvent::SetOpacity(opacity) => {
                    state.set_opacity(opacity);
                }
                UserEvent::About => {}
                UserEvent::Exit => {
                    close_requested = true;
                }
                UserEvent::GlobalKey {
                    state: ElementState::Pressed,
                    vk_code,
                    modifiers,
                } => {
                    dbg!(vk_code);
                }
                _ => {}
            },
            _ => {}
        }
    });
}
