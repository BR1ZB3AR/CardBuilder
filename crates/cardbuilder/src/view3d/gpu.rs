//! GPU side of the 3D preview (wgpu, through egui's paint callbacks): a shadow map from the key
//! light, the card and a shadow-catching floor rendered off-screen with 4× MSAA, then blitted
//! into egui's render pass. Renders only when something changed.

use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu::{self, wgpu, CallbackResources, CallbackTrait, ScreenDescriptor};
use std::sync::{Arc, Mutex};

pub const MSAA: u32 = 4;
const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
pub const SHADOW: u32 = 2048;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default, PartialEq, Debug)]
pub struct Globals {
    pub view_proj: [[f32; 4]; 4],
    pub light_vp: [[f32; 4]; 4],
    pub cam_pos: [f32; 4],
    pub key_dir: [f32; 4],
    pub key_col: [f32; 4],
    pub fill_dir: [f32; 4],
    pub sky: [f32; 4],
    pub ground: [f32; 4],
    pub paper: [f32; 4],
    pub stock: [f32; 4],
    pub misc: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
    pub uv: [f32; 2],
    pub face: f32,
}

/// what one frame asks the GPU to do
#[derive(Clone, PartialEq, Debug)]
pub struct FrameParams {
    pub globals: Globals,
    /// render target size in pixels
    pub size: [u32; 2],
}

struct Mesh {
    vbuf: wgpu::Buffer,
    ibuf: wgpu::Buffer,
    count: u32,
}

struct Target {
    size: [u32; 2],
    msaa: wgpu::TextureView,
    resolve: wgpu::TextureView,
    depth: wgpu::TextureView,
    blit_bg: wgpu::BindGroup,
}

/// a texture of one side (colour with mips, or a mask)
struct SideTexture {
    size: [u32; 2],
    levels: u32,
    tex: wgpu::Texture,
    view: wgpu::TextureView,
}

pub struct Gpu3d {
    main: wgpu::RenderPipeline,
    floor: wgpu::RenderPipeline,
    shadow: wgpu::RenderPipeline,
    blit: wgpu::RenderPipeline,
    blit_bgl: wgpu::BindGroupLayout,
    tex_bgl: wgpu::BindGroupLayout,
    globals_buf: wgpu::Buffer,
    globals_bg: wgpu::BindGroup,
    shadow_map: wgpu::TextureView,
    shadow_globals_buf: wgpu::Buffer,
    shadow_bg: wgpu::BindGroup,
    lin_samp: wgpu::Sampler,
    tex_samp: wgpu::Sampler,
    /// front colour, back colour, front mask, back mask
    textures: [Option<SideTexture>; 4],
    tex_bg: Option<wgpu::BindGroup>,
    card: Option<Mesh>,
    floor_mesh: Option<Mesh>,
    target: Option<Target>,
    last: Option<FrameParams>,
    dirty: bool,
    pub snapshot: Arc<Mutex<Option<SnapshotState>>>,
}

pub enum SnapshotState {
    Requested { w: u32, h: u32, params: Box<FrameParams> },
    Done(Result<(u32, u32, Vec<u8>), String>),
}

fn buffer(device: &wgpu::Device, label: &str, size: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false })
}

fn tex_entry(binding: u32, sample: wgpu::TextureSampleType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: sample, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None }
}
fn samp_entry(binding: u32, ty: wgpu::SamplerBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(ty), count: None }
}
fn uniform_entry(binding: u32, vis: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility: vis, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None }
}

impl Gpu3d {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target_format: wgpu::TextureFormat) -> Gpu3d {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("cb-card"), source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()) });
        let blit_module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("cb-blit"), source: wgpu::ShaderSource::Wgsl(include_str!("blit.wgsl").into()) });
        let globals_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cb-globals"),
            entries: &[uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT), tex_entry(1, wgpu::TextureSampleType::Depth), samp_entry(2, wgpu::SamplerBindingType::Comparison)],
        });
        // the shadow pass binds only the globals (its own copy, so the shadow map isn't bound while drawn)
        let shadow_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("cb-shadow-globals"), entries: &[uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT)] });
        let f = wgpu::TextureSampleType::Float { filterable: true };
        let tex_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cb-card-textures"),
            entries: &[tex_entry(0, f), tex_entry(1, f), tex_entry(2, f), tex_entry(3, f), samp_entry(4, wgpu::SamplerBindingType::Filtering)],
        });
        let blit_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("cb-blit"), entries: &[tex_entry(0, f), samp_entry(1, wgpu::SamplerBindingType::Filtering)] });
        let main_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("cb-main"), bind_group_layouts: &[Some(&globals_bgl), Some(&tex_bgl)], immediate_size: 0 });
        let shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("cb-shadow"), bind_group_layouts: &[Some(&shadow_bgl)], immediate_size: 0 });
        let blit_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("cb-blit"), bind_group_layouts: &[Some(&blit_bgl)], immediate_size: 0 });
        let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32];
        let vlayout = [Some(wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attrs })];
        let depth = |write: bool| wgpu::DepthStencilState { format: DEPTH, depth_write_enabled: Some(write), depth_compare: Some(wgpu::CompareFunction::LessEqual), stencil: Default::default(), bias: Default::default() };
        let pipe = |label: &str, fs: &str, cull: Option<wgpu::Face>, write: bool, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&main_layout),
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &vlayout },
                primitive: wgpu::PrimitiveState { cull_mode: cull, front_face: wgpu::FrontFace::Ccw, ..Default::default() },
                depth_stencil: Some(depth(write)),
                multisample: wgpu::MultisampleState { count: MSAA, mask: !0, alpha_to_coverage_enabled: false },
                fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some(fs), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format: COLOR, blend, write_mask: wgpu::ColorWrites::ALL })] }),
                multiview_mask: None,
                cache: None,
            })
        };
        let main = pipe("cb-card", "fs_main", Some(wgpu::Face::Back), true, None);
        let floor = pipe("cb-floor", "fs_floor", None, false, Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING));
        let shadow = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("cb-shadow"),
            layout: Some(&shadow_layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_shadow"), compilation_options: Default::default(), buffers: &vlayout },
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState { bias: wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 }, ..depth(true) }),
            multisample: Default::default(),
            fragment: None,
            multiview_mask: None,
            cache: None,
        });
        let srgb_target = target_format.is_srgb();
        let blit = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("cb-blit"),
            layout: Some(&blit_layout),
            vertex: wgpu::VertexState { module: &blit_module, entry_point: Some("vs_blit"), compilation_options: Default::default(), buffers: &[] },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &blit_module,
                entry_point: Some(if srgb_target { "fs_blit_srgb_target" } else { "fs_blit" }),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: target_format, blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = |aniso: u16, label: &str| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some(label),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                anisotropy_clamp: aniso,
                ..Default::default()
            })
        };
        let lin_samp = sampler(1, "cb-linear");
        let tex_samp = sampler(16, "cb-card-tex");
        let cmp_samp = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("cb-shadow"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let shadow_map = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("cb-shadow-map"),
                size: wgpu::Extent3d { width: SHADOW, height: SHADOW, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());
        let gsize = std::mem::size_of::<Globals>() as u64;
        let globals_buf = buffer(device, "cb-globals", gsize, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
        let shadow_globals_buf = buffer(device, "cb-shadow-globals", gsize, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
        let globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cb-globals"),
            layout: &globals_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&shadow_map) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&cmp_samp) },
            ],
        });
        let shadow_bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("cb-shadow-globals"), layout: &shadow_bgl, entries: &[wgpu::BindGroupEntry { binding: 0, resource: shadow_globals_buf.as_entire_binding() }] });
        let mut gpu = Gpu3d {
            main,
            floor,
            shadow,
            blit,
            blit_bgl,
            tex_bgl,
            globals_buf,
            globals_bg,
            shadow_map,
            shadow_globals_buf,
            shadow_bg,
            lin_samp,
            tex_samp,
            textures: [None, None, None, None],
            tex_bg: None,
            card: None,
            floor_mesh: None,
            target: None,
            last: None,
            dirty: true,
            snapshot: Default::default(),
        };
        // until the first render arrives: plain paper-white sides, no finishes
        let white = vec![vec![255u8, 255, 255, 255]];
        let black = vec![vec![0u8, 0, 0, 255]];
        for (i, data) in [&white, &white, &black, &black].into_iter().enumerate() {
            gpu.write_texture(device, queue, i, 1, 1, data);
        }
        gpu
    }

    /// (re)fill texture slot `i` (0 / 1 colour, 2 / 3 masks) with `levels` (mip chain)
    pub fn write_texture(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, i: usize, w: u32, h: u32, levels: &[Vec<u8>]) {
        let srgb = i < 2;
        let n = levels.len() as u32;
        let fits = self.textures[i].as_ref().is_some_and(|t| t.size == [w, h] && t.levels == n);
        if !fits {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("cb-side"),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: n,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: if srgb { wgpu::TextureFormat::Rgba8UnormSrgb } else { wgpu::TextureFormat::Rgba8Unorm },
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            self.textures[i] = Some(SideTexture { size: [w, h], levels: n, tex, view });
            self.tex_bg = None;
        }
        let t = self.textures[i].as_ref().unwrap();
        let (mut lw, mut lh) = (w, h);
        for (m, data) in levels.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &t.tex, mip_level: m as u32, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                data,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * lw), rows_per_image: Some(lh) },
                wgpu::Extent3d { width: lw, height: lh, depth_or_array_layers: 1 },
            );
            lw = (lw / 2).max(1);
            lh = (lh / 2).max(1);
        }
        if self.tex_bg.is_none() && self.textures.iter().all(Option::is_some) {
            let v = |k: usize| wgpu::BindingResource::TextureView(&self.textures[k].as_ref().unwrap().view);
            self.tex_bg = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("cb-card-textures"),
                layout: &self.tex_bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: v(0) },
                    wgpu::BindGroupEntry { binding: 1, resource: v(1) },
                    wgpu::BindGroupEntry { binding: 2, resource: v(2) },
                    wgpu::BindGroupEntry { binding: 3, resource: v(3) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&self.tex_samp) },
                ],
            }));
        }
        self.dirty = true;
    }

    fn mesh(device: &wgpu::Device, queue: &wgpu::Queue, v: &[Vertex], idx: &[u32], label: &str) -> Option<Mesh> {
        if v.is_empty() || idx.is_empty() {
            return None;
        }
        let vbuf = buffer(device, label, std::mem::size_of_val(v) as u64, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST);
        let ibuf = buffer(device, label, std::mem::size_of_val(idx) as u64, wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST);
        queue.write_buffer(&vbuf, 0, bytemuck::cast_slice(v));
        queue.write_buffer(&ibuf, 0, bytemuck::cast_slice(idx));
        Some(Mesh { vbuf, ibuf, count: idx.len() as u32 })
    }

    pub fn set_card(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, v: &[Vertex], idx: &[u32]) {
        self.card = Self::mesh(device, queue, v, idx, "cb-card-mesh");
        self.dirty = true;
    }

    pub fn set_floor(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, radius: f32) {
        let r = radius;
        let n = [0.0, 1.0, 0.0];
        let v = [[-r, 0.0, -r], [r, 0.0, -r], [r, 0.0, r], [-r, 0.0, r]].map(|pos| Vertex { pos, nrm: n, uv: [0.0, 0.0], face: 3.0 });
        self.floor_mesh = Self::mesh(device, queue, &v, &[0, 2, 1, 0, 3, 2], "cb-floor-mesh");
        self.dirty = true;
    }

    fn make_target(&self, device: &wgpu::Device, size: [u32; 2]) -> Target {
        let tex = |samples: u32, format: wgpu::TextureFormat, usage: wgpu::TextureUsages, label: &str| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        };
        let msaa = tex(MSAA, COLOR, wgpu::TextureUsages::RENDER_ATTACHMENT, "cb-msaa");
        let resolve = tex(1, COLOR, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC, "cb-resolve");
        let depth = tex(MSAA, DEPTH, wgpu::TextureUsages::RENDER_ATTACHMENT, "cb-depth");
        let blit_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cb-blit"),
            layout: &self.blit_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&resolve) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.lin_samp) }],
        });
        Target { size, msaa, resolve, depth, blit_bg }
    }

    /// the shadow pass and the main pass for `p`, into `t`
    fn record(&mut self, queue: &wgpu::Queue, enc: &mut wgpu::CommandEncoder, p: &FrameParams, t: &Target) {
        queue.write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&p.globals));
        queue.write_buffer(&self.shadow_globals_buf, 0, bytemuck::bytes_of(&p.globals));
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("cb-shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &self.shadow_map, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some(m) = &self.card {
                pass.set_pipeline(&self.shadow);
                pass.set_bind_group(0, &self.shadow_bg, &[]);
                pass.set_vertex_buffer(0, m.vbuf.slice(..));
                pass.set_index_buffer(m.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..m.count, 0, 0..1);
            }
        }
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("cb-scene"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &t.msaa,
                depth_slice: None,
                resolve_target: Some(&t.resolve),
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Discard },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &t.depth,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let Some(tex_bg) = &self.tex_bg else { return };
        pass.set_bind_group(0, &self.globals_bg, &[]);
        pass.set_bind_group(1, tex_bg, &[]);
        if let Some(m) = &self.card {
            pass.set_pipeline(&self.main);
            pass.set_vertex_buffer(0, m.vbuf.slice(..));
            pass.set_index_buffer(m.ibuf.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..m.count, 0, 0..1);
        }
        if let Some(m) = &self.floor_mesh {
            pass.set_pipeline(&self.floor);
            pass.set_vertex_buffer(0, m.vbuf.slice(..));
            pass.set_index_buffer(m.ibuf.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..m.count, 0, 0..1);
        }
    }

    /// render a still at `w` × `h` and read it back (straight RGBA, sRGB)
    fn snapshot(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, w: u32, h: u32, p: &FrameParams) {
        let t = self.make_target(device, [w, h]);
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("cb-snapshot") });
        self.record(queue, &mut enc, p, &t);
        let row = (4 * w).div_ceil(256) * 256;
        let buf = buffer(device, "cb-snapshot-read", (row * h) as u64, wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ);
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: t.resolve.texture(), mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        queue.submit([enc.finish()]);
        let slot = self.snapshot.clone();
        let buf2 = buf.clone();
        buf.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let res = r.map_err(|e| e.to_string()).map(|_| {
                let Ok(data) = buf2.slice(..).get_mapped_range() else { return (0, 0, Vec::new()) };
                let mut out = Vec::with_capacity((w * h * 4) as usize);
                for y in 0..h {
                    let s = (y * row) as usize;
                    out.extend_from_slice(&data[s..s + (w * 4) as usize]);
                }
                drop(data);
                buf2.unmap();
                cb_print::unpremultiply(&mut out);
                (w, h, out)
            });
            *slot.lock().unwrap() = Some(SnapshotState::Done(res));
        });
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        self.dirty = true;
    }
}

/// the per-frame paint callback
pub struct Paint3d {
    pub params: FrameParams,
}

impl CallbackTrait for Paint3d {
    fn prepare(&self, device: &wgpu::Device, queue: &wgpu::Queue, _sd: &ScreenDescriptor, enc: &mut wgpu::CommandEncoder, res: &mut CallbackResources) -> Vec<wgpu::CommandBuffer> {
        let Some(gpu) = res.get_mut::<Gpu3d>() else { return vec![] };
        let req = {
            let mut s = gpu.snapshot.lock().unwrap();
            match s.take() {
                Some(SnapshotState::Requested { w, h, params }) => Some((w, h, params)),
                other => {
                    *s = other;
                    None
                }
            }
        };
        if let Some((w, h, params)) = req {
            gpu.snapshot(device, queue, w, h, &params);
        }
        let size = [self.params.size[0].max(1), self.params.size[1].max(1)];
        if gpu.target.as_ref().map(|t| t.size) != Some(size) {
            gpu.target = Some(gpu.make_target(device, size));
            gpu.dirty = true;
        }
        if !gpu.dirty && gpu.last.as_ref() == Some(&self.params) {
            return vec![];
        }
        let t = gpu.target.take().unwrap();
        gpu.record(queue, enc, &self.params, &t);
        gpu.target = Some(t);
        gpu.last = Some(self.params.clone());
        gpu.dirty = false;
        vec![]
    }

    fn paint(&self, _info: egui::PaintCallbackInfo, pass: &mut wgpu::RenderPass<'static>, res: &CallbackResources) {
        let Some(gpu) = res.get::<Gpu3d>() else { return };
        let Some(t) = &gpu.target else { return };
        pass.set_pipeline(&gpu.blit);
        pass.set_bind_group(0, &t.blit_bg, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// put the GPU state into egui's renderer (once, at start-up)
pub fn install(rs: &egui_wgpu::RenderState) {
    let gpu = Gpu3d::new(&rs.device, &rs.queue, rs.target_format);
    rs.renderer.write().callback_resources.insert(gpu);
}
