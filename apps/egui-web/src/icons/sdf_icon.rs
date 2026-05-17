//! WebGPU SDF renderer for SVG-derived gate icons.
//!
//! The source of truth is still `assets/icons/*.svg`; `build.rs` converts the
//! generated PNG alpha into a signed distance field. This module owns only the
//! GPU drawing path. `svg_icon` keeps the tiny alpha-texture fallback for
//! non-WebGPU contexts.

use eframe::egui;
use eframe::egui_wgpu;
use eframe::wgpu;
use std::cell::RefCell;
use std::collections::HashMap;

use wgpu::util::DeviceExt;

use super::svg_icon::{sdf_rle, GateGlyph, RASTER_SIZE, SDF_PX_RANGE};

const SDF_ICON_SHADER: &str = r#"
struct IconParams {
  color: vec4<f32>,
};

@group(0) @binding(0) var icon_sdf: texture_2d<f32>;
@group(0) @binding(1) var icon_sampler: sampler;
@group(0) @binding(2) var<uniform> params: IconParams;

struct VsIn {
  @location(0) clip: vec2<f32>,
  @location(1) uv: vec2<f32>,
};

struct VsOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
  var out: VsOut;
  // `Callback::new_paint_callback` maps NDC -1..1 to the icon rect.
  out.clip = vec4<f32>(input.clip, 0.0, 1.0);
  out.uv = input.uv;
  return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
  let distance = textureSample(icon_sdf, icon_sampler, input.uv).r;
  let dims_u = textureDimensions(icon_sdf);
  let dims = vec2<f32>(f32(dims_u.x), f32(dims_u.y));
  let uv_delta = max(fwidth(input.uv), vec2<f32>(1.0e-6));
  let screen_tex_size = vec2<f32>(1.0) / uv_delta;
  let unit_range = vec2<f32>(SDF_PX_RANGE_PLACEHOLDER) / dims;
  let screen_px_range = max(0.5 * dot(unit_range, screen_tex_size), 1.0);
  let screen_px_distance = screen_px_range * (distance - 0.5);
  let alpha = clamp(screen_px_distance + 0.5, 0.0, 1.0);
  if (alpha < 1.0e-3) {
    discard;
  }
  let out_alpha = alpha * params.color.a;
  return vec4<f32>(params.color.rgb * out_alpha, out_alpha);
}
"#;

const QUAD_VERTICES: [[f32; 4]; 4] = [
    [-1.0, -1.0, 0.0, 1.0],
    [1.0, -1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0, 0.0],
    [-1.0, 1.0, 0.0, 0.0],
];
const QUAD_INDICES: [u16; 6] = [0, 1, 2, 0, 2, 3];

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SdfIconParams {
    color: [f32; 4],
}

struct SdfTexture {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

struct SdfIconBindGroup {
    bind_group: wgpu::BindGroup,
    _params_buffer: wgpu::Buffer,
}

struct SdfIconResources {
    target_format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    textures: HashMap<GateGlyph, SdfTexture>,
    bind_groups: HashMap<(GateGlyph, u32), SdfIconBindGroup>,
}

struct SdfIconCallback {
    glyph: GateGlyph,
    color: egui::Color32,
    target_format: wgpu::TextureFormat,
}

thread_local! {
    static SDF_TARGET_FORMAT: RefCell<Option<wgpu::TextureFormat>> = const { RefCell::new(None) };
}

pub(crate) fn set_target_format(target_format: Option<wgpu::TextureFormat>) {
    SDF_TARGET_FORMAT.with(|slot| {
        *slot.borrow_mut() = target_format;
    });
}

fn current_target_format() -> Option<wgpu::TextureFormat> {
    SDF_TARGET_FORMAT.with(|slot| *slot.borrow())
}

fn sdf_texture_name(glyph: GateGlyph) -> &'static str {
    match glyph {
        GateGlyph::H => "gate-icon-h-sdf",
        GateGlyph::X => "gate-icon-x-sdf",
        GateGlyph::Y => "gate-icon-y-sdf",
        GateGlyph::Z => "gate-icon-z-sdf",
        GateGlyph::Plus => "gate-icon-plus-sdf",
        GateGlyph::SqrtX => "gate-icon-sqrtx-sdf",
        GateGlyph::S => "gate-icon-s-sdf",
        GateGlyph::SDagger => "gate-icon-sdagger-sdf",
    }
}

fn color_key(color: egui::Color32) -> u32 {
    let [r, g, b, a] = color.to_array();
    u32::from_be_bytes([r, g, b, a])
}

fn bytes_from_rle(rle: &[(u16, u8)]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(RASTER_SIZE * RASTER_SIZE);
    for &(run, value) in rle {
        bytes.extend(std::iter::repeat_n(value, run as usize));
    }
    debug_assert_eq!(bytes.len(), RASTER_SIZE * RASTER_SIZE);
    bytes
}

impl SdfIconResources {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, target_format: wgpu::TextureFormat) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gate_icon_sdf_bind_group_layout"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline = build_sdf_pipeline(device, target_format, &bind_group_layout);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("gate_icon_sdf_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("gate_icon_sdf_quad_vertices"),
            contents: bytemuck::cast_slice(&QUAD_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("gate_icon_sdf_quad_indices"),
            contents: bytemuck::cast_slice(&QUAD_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        let textures = GateGlyph::ALL
            .iter()
            .map(|glyph| (*glyph, create_sdf_texture(device, queue, *glyph)))
            .collect::<HashMap<_, _>>();
        Self {
            target_format,
            pipeline,
            bind_group_layout,
            sampler,
            vertex_buffer,
            index_buffer,
            index_count: QUAD_INDICES.len() as u32,
            textures,
            bind_groups: HashMap::new(),
        }
    }

    fn update_target_format(&mut self, device: &wgpu::Device, target_format: wgpu::TextureFormat) {
        if self.target_format == target_format {
            return;
        }
        self.pipeline = build_sdf_pipeline(device, target_format, &self.bind_group_layout);
        self.target_format = target_format;
    }

    fn ensure_bind_group(&mut self, device: &wgpu::Device, glyph: GateGlyph, color: egui::Color32) {
        let key = (glyph, color_key(color));
        if self.bind_groups.contains_key(&key) {
            return;
        }
        let params = SdfIconParams {
            color: color.to_normalized_gamma_f32(),
        };
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("gate_icon_sdf_params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let texture = self
            .textures
            .get(&glyph)
            .expect("SDF texture must exist for every GateGlyph");
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gate_icon_sdf_bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });
        self.bind_groups.insert(
            key,
            SdfIconBindGroup {
                bind_group,
                _params_buffer: params_buffer,
            },
        );
    }

    fn bind_group(&self, glyph: GateGlyph, color: egui::Color32) -> Option<&wgpu::BindGroup> {
        self.bind_groups
            .get(&(glyph, color_key(color)))
            .map(|entry| &entry.bind_group)
    }
}

fn build_sdf_pipeline(
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
    bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader_source =
        SDF_ICON_SHADER.replace("SDF_PX_RANGE_PLACEHOLDER", &format!("{SDF_PX_RANGE:.1}"));
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("gate_icon_sdf_shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("gate_icon_sdf_pipeline_layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });
    let vertex_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 8,
                shader_location: 1,
            },
        ],
    };

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("gate_icon_sdf_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[vertex_layout],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

fn create_sdf_texture(device: &wgpu::Device, queue: &wgpu::Queue, glyph: GateGlyph) -> SdfTexture {
    let data = bytes_from_rle(sdf_rle(glyph));
    let texture = wgpu::util::DeviceExt::create_texture_with_data(
        device,
        queue,
        &wgpu::TextureDescriptor {
            label: Some(sdf_texture_name(glyph)),
            size: wgpu::Extent3d {
                width: RASTER_SIZE as u32,
                height: RASTER_SIZE as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::default(),
        &data,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    SdfTexture {
        _texture: texture,
        view,
    }
}

impl egui_wgpu::CallbackTrait for SdfIconCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let resources = if callback_resources.contains::<SdfIconResources>() {
            callback_resources
                .get_mut::<SdfIconResources>()
                .expect("SdfIconResources missing")
        } else {
            callback_resources.insert(SdfIconResources::new(device, queue, self.target_format));
            callback_resources
                .get_mut::<SdfIconResources>()
                .expect("SdfIconResources just inserted")
        };
        resources.update_target_format(device, self.target_format);
        resources.ensure_bind_group(device, self.glyph, self.color);
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(resources) = callback_resources.get::<SdfIconResources>() else {
            return;
        };
        let Some(bind_group) = resources.bind_group(self.glyph, self.color) else {
            return;
        };
        render_pass.set_pipeline(&resources.pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, resources.vertex_buffer.slice(..));
        render_pass.set_index_buffer(resources.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..resources.index_count, 0, 0..1);
    }
}

pub(super) fn draw_glyph(
    painter: &egui::Painter,
    rect: egui::Rect,
    color: egui::Color32,
    glyph: GateGlyph,
) -> bool {
    let Some(target_format) = current_target_format() else {
        return false;
    };
    let callback = SdfIconCallback {
        glyph,
        color,
        target_format,
    };
    let paint_callback = egui_wgpu::Callback::new_paint_callback(rect, callback);
    painter.add(egui::Shape::Callback(paint_callback));
    true
}
