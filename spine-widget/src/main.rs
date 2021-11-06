use std::{
    collections::HashSet,
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::Result;
use config::Config;
use image::{io::Reader as ImageReader, GenericImageView};
use spine::{
    atlas::AtlasPage, spine_init, AnimationState, AnimationStateData, Atlas, AttachmentType,
    Skeleton, SkeletonData, SpineCallbacks,
};
use texture::{Texture, TextureConfig};
use wgpu::{util::DeviceExt, IndexFormat};
use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, GWL_STYLE, WINDOW_EX_STYLE,
        WS_EX_LAYERED, WS_EX_TRANSPARENT,
    },
};
use winit::{
    dpi::{LogicalPosition, LogicalSize},
    event::*,
    event_loop::{ControlFlow, EventLoop},
    platform::windows::WindowExtWindows,
    window::{Window, WindowBuilder},
};

mod config;
mod texture;

struct SpineCb;
impl SpineCallbacks for SpineCb {
    type Texture = Texture;

    type LoadError = anyhow::Error;

    fn load_texture(path: &str, atlas: &AtlasPage) -> Result<(Texture, u32, u32), Self::LoadError> {
        let mut img = ImageReader::open(path)?.decode()?;

        let mask_path = PathBuf::from(path.replace(".png", "[alpha].png").as_str());
        if mask_path.is_file() {
            let mask_img = ImageReader::open(&mask_path)?.decode()?;

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
}
spine_init!(SpineCb);

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable, Default)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
    tint: [f32; 4],
}

impl Vertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: (mem::size_of::<[f32; 2]>() + mem::size_of::<[f32; 2]>())
                        as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ScalingUniform {
    window_width: f32,
    window_height: f32,
    scale: f32,
}

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    scale_factor: f64,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    texture_bind_group_layout: wgpu::BindGroupLayout,

    scaling_uniform: ScalingUniform,
    scaling_buffer: wgpu::Buffer,
    scaling_bind_group: wgpu::BindGroup,

    spine: SpineState,
    world_vertices: Vec<[f32; 2]>,
    scratch_vertex_buffer: Vec<Vertex>,
    scratch_index_buffer: Vec<u16>,

    pressed_keys: HashSet<VirtualKeyCode>,
    modifiers_state: ModifiersState,
    passthrough: bool,
}

impl State {
    // Creating some of the wgpu types requires async code
    async fn new(window: &Window, config: &config::Config) -> Self {
        let size = window.inner_size();

        // The instance is a handle to our GPU
        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                    label: None,
                },
                None,
            )
            .await
            .unwrap();

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_preferred_format(&adapter).unwrap(),
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        };
        surface.configure(&device, &surface_config);

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

        let scaling_uniform = {
            let window_logical_size = size.to_logical::<f32>(window.scale_factor());
            ScalingUniform {
                window_width: window_logical_size.width,
                window_height: window_logical_size.height,
                scale: config.scale,
            }
        };

        let scaling_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Scaling Buffer"),
            contents: bytemuck::cast_slice(&[scaling_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let scaling_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("scaling_bind_group_layout"),
            });

        let scaling_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &scaling_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: scaling_buffer.as_entire_binding(),
            }],
            label: Some("scaling_bind_group"),
        });

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
                    format: surface_config.format,
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
            size: 1024 * 128, // 1M
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Index Buffer"),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            size: 1024 * 128, // 1M
            mapped_at_creation: false,
        });

        let spine = SpineState::new(config).unwrap();

        Self {
            surface,
            device,
            queue,
            config: surface_config,
            size,
            scale_factor: window.scale_factor(),
            render_pipeline,
            vertex_buffer,
            index_buffer,
            texture_bind_group_layout,

            scaling_uniform,
            scaling_buffer,
            scaling_bind_group,

            spine,
            world_vertices: Vec::new(),
            scratch_vertex_buffer: Vec::new(),
            scratch_index_buffer: Vec::new(),

            pressed_keys: HashSet::new(),
            modifiers_state: Default::default(),
            passthrough: true,
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);

            let window_logical_size = new_size.to_logical::<f32>(self.scale_factor);
            self.scaling_uniform.window_height = window_logical_size.height;
            self.scaling_uniform.window_width = window_logical_size.width;
        }
    }

    fn scale(&mut self, scale_factor: f64) {
        self.scale_factor = scale_factor;
    }

    fn input(&mut self, event: &WindowEvent, window: &Window, config: &Config) -> bool {
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
                        self.scaling_uniform.scale += 0.1;
                        return true;
                    }
                    (ModifiersState::CTRL, VirtualKeyCode::Minus) => {
                        // "-_" on main keyboard
                        self.scaling_uniform.scale -= 0.1;
                        return true;
                    }
                    (_, VirtualKeyCode::F12) => {
                        // "F12" on main keyboard
                        self.passthrough = !self.passthrough;
                        dbg!(self.passthrough);
                        window.set_decorations(!self.passthrough);
                        set_click_passthrough(&window, self.passthrough);
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
        self.queue.write_buffer(
            &self.scaling_buffer,
            0,
            bytemuck::cast_slice(&[self.scaling_uniform]),
        );
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.spine.prepare_render();

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
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
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });

        let mut current_tex_id = -1i64;

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
                skel_tint[3] * slot_tint[3],
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

                    if current_tex_id == -1 {
                        // Initialize texture
                        tex.initialize(
                            &self.device,
                            &self.queue,
                            &self.texture_bind_group_layout,
                            None,
                        )
                        .unwrap();
                        current_tex_id = tex.id() as i64;

                        render_pass.set_bind_group(0, &tex.get_texture().bind_group, &[]);
                    } else if current_tex_id != tex.id() as i64 {
                        unimplemented!();
                    }

                    let offset = self.scratch_vertex_buffer.len() as u16;
                    region.compute_world_vertices(&mut self.world_vertices);
                    let new_vectors = self
                        .world_vertices
                        .iter()
                        .enumerate()
                        .map(|(i, p)| {
                            let (u, v) = region.uv(i);
                            ([u, v], *p)
                        })
                        .map(to_vertex);
                    self.scratch_vertex_buffer.extend(new_vectors);

                    let new_indices = [0, 1, 2, 2, 3, 0].iter().map(|i| i + offset);
                    self.scratch_index_buffer.extend(new_indices);
                }
                AttachmentType::Mesh(mesh) => {
                    let tex = if let Some(tex) =
                        unsafe { mesh.atlas_region().page().render_object::<Texture>() }
                    {
                        tex
                    } else {
                        continue;
                    };

                    if current_tex_id == -1 {
                        // Initialize texture
                        tex.initialize(
                            &self.device,
                            &self.queue,
                            &self.texture_bind_group_layout,
                            None,
                        )
                        .unwrap();
                        current_tex_id = tex.id() as i64;

                        render_pass.set_bind_group(0, &tex.get_texture().bind_group, &[]);
                    } else if current_tex_id != tex.id() as i64 {
                        unimplemented!();
                    }

                    let offset = self.scratch_vertex_buffer.len() as u16;
                    mesh.compute_world_vertices(&mut self.world_vertices);
                    let new_vectors = self
                        .world_vertices
                        .iter()
                        .enumerate()
                        .map(|(i, p)| {
                            let (u, v) = mesh.uv(i);
                            ([u, v], *p)
                        })
                        .map(to_vertex);
                    self.scratch_vertex_buffer.extend(new_vectors);

                    let new_indices = mesh.indices().iter().map(|i| i + offset);
                    self.scratch_index_buffer.extend(new_indices);
                }
            }
        }

        {
            let len = self.scratch_vertex_buffer.len();
            let vb_pad = len % 4;
            if vb_pad != 0 {
                self.scratch_vertex_buffer.resize(
                    self.scratch_vertex_buffer.len() + 4 - vb_pad,
                    Default::default(),
                );
            }
        };

        let ib_len = {
            let len = self.scratch_index_buffer.len();
            let ib_pad = len % 4;
            if ib_pad != 0 {
                self.scratch_index_buffer
                    .resize(self.scratch_index_buffer.len() + 4 - ib_pad, 0);
            }
            len
        };

        self.queue.write_buffer(
            &self.vertex_buffer,
            0,
            bytemuck::cast_slice(&self.scratch_vertex_buffer),
        );
        self.queue.write_buffer(
            &self.index_buffer,
            0,
            bytemuck::cast_slice(&self.scratch_index_buffer),
        );

        render_pass.set_bind_group(1, &self.scaling_bind_group, &[]);
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint16);

        render_pass.draw_indexed(0..ib_len as u32, 0, 0..1);

        drop(render_pass);

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        self.scratch_vertex_buffer.clear();
        self.scratch_index_buffer.clear();

        Ok(())
    }
}

struct SpineState {
    _atlas: Atlas,
    _skel_data: SkeletonData,
    _anim_state_data: AnimationStateData,

    skel: Skeleton,
    anim: AnimationState,

    last_render: Option<Instant>,
}

impl SpineState {
    fn new(config: &Config) -> Result<Self> {
        let atlas = Atlas::new(&config.atlas)?;
        let skel_data = SkeletonData::new_binary(&atlas, &config.skel, 0.5)?;
        let anim_data = AnimationStateData::new(&skel_data, 0.5)?;

        let mut skel = Skeleton::new(&skel_data)?;
        skel.set_x(0.0);
        skel.set_y(0.0);

        let mut anim = AnimationState::new(&anim_data)?;
        if let Some(ref idle_name) = config.idle_animation {
            anim.set_animation_by_name(0, idle_name, true);
        }

        Ok(Self {
            _atlas: atlas,
            _skel_data: skel_data,
            _anim_state_data: anim_data,

            skel,
            anim,

            last_render: None,
        })
    }

    fn prepare_render(&mut self) {
        let now = Instant::now();
        let delta = if let Some(last_render) = self.last_render {
            now - last_render
        } else {
            Duration::from_millis(0)
        }
        .as_secs_f32();
        self.last_render = Some(now);

        self.anim.update(delta);
        self.skel.apply_animation(&self.anim);
        self.skel.update_world_transform();
    }
}

/// Make this window clickable or not (clicking passthrough)
fn set_click_passthrough(window: &Window, passthrough: bool) {
    unsafe {
        let hwnd: HWND = std::mem::transmute(window.hwnd());
        let window_styles: WINDOW_EX_STYLE = match GetWindowLongPtrW(hwnd, GWL_EXSTYLE) {
            0 => panic!("GetWindowLongPtrW failed"),
            n => WINDOW_EX_STYLE(n.try_into().unwrap()),
        };

        let window_styles = if passthrough {
            window_styles | WS_EX_TRANSPARENT | WS_EX_LAYERED
        } else {
            window_styles & !WS_EX_TRANSPARENT | WS_EX_LAYERED
        };

        if SetWindowLongPtrW(hwnd, GWL_EXSTYLE, window_styles.0.try_into().unwrap()) == 0 {
            panic!("SetWindowLongPtrW failed");
        }
    }
}

fn main() {
    env_logger::init();

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.yml".to_string());

    let mut config = config::load(&config_path).unwrap();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_decorations(false)
        .with_transparent(true)
        .with_inner_size(LogicalSize::new(config.window_size.0, config.window_size.1))
        .build(&event_loop)
        .unwrap();

    window.set_outer_position(LogicalPosition::new(
        config.window_position.0,
        config.window_position.1,
    ));
    window.set_title("spine-widget");
    window.set_always_on_top(true);
    set_click_passthrough(&window, true);

    let mut state = pollster::block_on(State::new(&window, &config));

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == window.id() => {
            if !state.input(event, &window, &config) {
                match event {
                    WindowEvent::CloseRequested => {
                        // Save window parameters
                        let logical_size =
                            window.inner_size().to_logical::<f64>(window.scale_factor());
                        config.window_size = (logical_size.width, logical_size.height);

                        let logical_pos = window
                            .outer_position()
                            .map(|p| p.to_logical::<f64>(window.scale_factor()));
                        if let Ok(pos) = logical_pos {
                            config.window_position = (pos.x, pos.y);
                        }

                        config.scale = state.scaling_uniform.scale;

                        let _ = config::save(&config, &config_path);
                        *control_flow = ControlFlow::Exit;
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
        Event::RedrawRequested(_) => {
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
            window.request_redraw();
        }
        _ => {}
    });
}
