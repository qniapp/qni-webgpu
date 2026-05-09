//! `StateVectorResources` — the kitchen-sink GPU resource bag.
//!
//! Owns every pipeline, bind group, and buffer the three CallbackTrait
//! impls in `callbacks.rs` need: state-vector compute + render, Bloch
//! reduction, measurement reduce + collapse, Bloch overlay, and the
//! measurement digit overlay. Constructed once via `new` (large but
//! straightforward setup) and reused for the lifetime of the canvas.
//! `update_render_pipeline` is a no-op unless the surface format
//! changes.

use eframe::wgpu;
use wgpu::util::DeviceExt as _;

use crate::constants::MAX_STATE_COUNT;
use crate::gates::GateParams;

use super::digit_atlas::{rasterize_digit_atlas, DIGIT_ATLAS_HEIGHT, DIGIT_ATLAS_WIDTH};
use super::params::{
    BlochOverlayInstance, BlochOverlayParams, BlochParams, MeasureCollapseParams,
    MeasureReduceParams, MeasurementDigitInstance, MeasurementDigitParams, RenderParams,
    StateInstance, MAX_BLOCH_SLOTS, MAX_MEASUREMENT_SLOTS, MAX_OPS_PER_RECOMPUTE,
};
use super::shaders::{
    BLOCH_OVERLAY_SHADER, BLOCH_REDUCE_SHADER, MEASUREMENT_DIGIT_SHADER, MEASURE_COLLAPSE_SHADER,
    MEASURE_REDUCE_SHADER, STATE_COMPUTE_SHADER, STATE_RENDER_SHADER,
};

pub(crate) struct StateVectorResources {
    pub(crate) compute_pipeline: wgpu::ComputePipeline,
    pub(crate) render_pipeline: wgpu::RenderPipeline,
    pub(crate) compute_bind_groups: [wgpu::BindGroup; 2],
    pub(crate) render_bind_groups: [wgpu::BindGroup; 2],
    pub(crate) render_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) gate_params_buffer: wgpu::Buffer,
    /// Staging buffer holding all `GateParams` for a recompute, packed
    /// contiguously. Filled once via `queue.write_buffer` before the dispatch
    /// loop; each per-gate dispatch then sources its params via
    /// `encoder.copy_buffer_to_buffer` from the matching slot into
    /// `gate_params_buffer`. Lets us keep the existing uniform binding while
    /// collapsing N per-gate `queue.submit` round trips into a single submit.
    pub(crate) gate_params_staging_buffer: wgpu::Buffer,
    pub(crate) render_params_buffer: wgpu::Buffer,
    pub(crate) state_buffers: [wgpu::Buffer; 2],
    /// 8-byte read-only buffer holding the |0…0⟩ amplitude `(1.0, 0.0)`. At
    /// the start of every recompute we encode `clear_buffer(state_buffers[0])`
    /// followed by `copy_buffer_to_buffer(seed → state_buffers[0])` to
    /// initialize the state vector entirely on the GPU. Replaces the prior
    /// CPU-side `vec![[0.0; 2]; state_count]` + `queue.write_buffer` upload
    /// (Issue C — see docs/egui-web-perf-audit.html).
    pub(crate) state_seed_buffer: wgpu::Buffer,
    pub(crate) instance_buffer: wgpu::Buffer,
    pub(crate) vertex_buffer: wgpu::Buffer,
    pub(crate) index_buffer: wgpu::Buffer,
    pub(crate) index_count: u32,
    pub(crate) target_format: wgpu::TextureFormat,
    pub(crate) state_count: usize,
    pub(crate) active_state: usize,
    /// Bloch reduction pipeline + buffers. Two bind groups so we can read from
    /// either ping-pong state buffer (whichever holds the current state at
    /// capture time). The output buffer is GPU-only — readback uses a fresh
    /// MAP_READ staging buffer per dispatch so we never re-issue commands
    /// against a buffer that is still mapped from an earlier readback.
    pub(crate) bloch_pipeline: wgpu::ComputePipeline,
    pub(crate) bloch_bind_groups: [wgpu::BindGroup; 2],
    pub(crate) bloch_params_buffer: wgpu::Buffer,
    /// See `gate_params_staging_buffer`. Same pattern, holds packed
    /// `BlochParams` for every Bloch capture in the recompute.
    pub(crate) bloch_params_staging_buffer: wgpu::Buffer,
    pub(crate) bloch_output_buffer: wgpu::Buffer,
    /// Measurement reduce + sample shader and its bind groups (one per ping-
    /// pong state buffer). Writes `(pZero, r, outcome, sqrt_p_kept)` to
    /// `measurement_aux_buffer`.
    pub(crate) measure_reduce_pipeline: wgpu::ComputePipeline,
    pub(crate) measure_reduce_bind_groups: [wgpu::BindGroup; 2],
    pub(crate) measure_reduce_params_buffer: wgpu::Buffer,
    /// Packed `MeasureReduceParams` for every measurement-reduce in the
    /// recompute. See `gate_params_staging_buffer` for the rationale.
    pub(crate) measure_reduce_params_staging_buffer: wgpu::Buffer,
    /// Measurement collapse shader. Four bind groups: two ping-pong
    /// directions × ?, actually two bind groups (state_in side selects which
    /// buffer to read; the other is the write target).
    pub(crate) measure_collapse_pipeline: wgpu::ComputePipeline,
    pub(crate) measure_collapse_bind_groups: [wgpu::BindGroup; 2],
    pub(crate) measure_collapse_params_buffer: wgpu::Buffer,
    /// Packed `MeasureCollapseParams` for every measurement-collapse in the
    /// recompute. See `gate_params_staging_buffer` for the rationale.
    pub(crate) measure_collapse_params_staging_buffer: wgpu::Buffer,
    pub(crate) measurement_aux_buffer: wgpu::Buffer,
    /// GPU render pass that draws the dynamic Bloch arrow + tip dot directly
    /// from `bloch_output_buffer`. No CPU readback in production.
    pub(crate) bloch_overlay_pipeline: wgpu::RenderPipeline,
    pub(crate) bloch_overlay_bind_group: wgpu::BindGroup,
    pub(crate) bloch_overlay_params_buffer: wgpu::Buffer,
    pub(crate) bloch_overlay_instance_buffer: wgpu::Buffer,
    pub(crate) bloch_overlay_vertex_buffer: wgpu::Buffer,
    pub(crate) bloch_overlay_index_buffer: wgpu::Buffer,
    /// Renders the 0/1 measurement digit straight from
    /// `measurement_aux_buffer`. Static meter icon is still painted by egui.
    pub(crate) measurement_digit_pipeline: wgpu::RenderPipeline,
    pub(crate) measurement_digit_bind_group: wgpu::BindGroup,
    pub(crate) measurement_digit_params_buffer: wgpu::Buffer,
    pub(crate) measurement_digit_instance_buffer: wgpu::Buffer,
    /// Last params written to each `*_params_buffer`. Lets the per-frame
    /// `prepare()` code skip `queue.write_buffer` when nothing changed —
    /// viewport / colors are stable across most frames, so without these
    /// caches we'd issue 3 redundant uploads every frame even when the
    /// circuit is idle (Issue B).
    pub(crate) last_render_params: Option<RenderParams>,
    pub(crate) last_bloch_overlay_params: Option<BlochOverlayParams>,
    pub(crate) last_measurement_digit_params: Option<MeasurementDigitParams>,
}


impl StateVectorResources {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("state_vector_compute"),
            source: wgpu::ShaderSource::Wgsl(STATE_COMPUTE_SHADER.into()),
        });
        let bloch_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bloch_reduce"),
            source: wgpu::ShaderSource::Wgsl(BLOCH_REDUCE_SHADER.into()),
        });
        let measure_reduce_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("measure_reduce"),
            source: wgpu::ShaderSource::Wgsl(MEASURE_REDUCE_SHADER.into()),
        });
        let measure_collapse_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("measure_collapse"),
            source: wgpu::ShaderSource::Wgsl(MEASURE_COLLAPSE_SHADER.into()),
        });
        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("state_vector_render"),
            source: wgpu::ShaderSource::Wgsl(STATE_RENDER_SHADER.into()),
        });

        let compute_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("state_vector_compute_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("state_vector_compute_pipeline_layout"),
                bind_group_layouts: &[&compute_bind_group_layout],
                push_constant_ranges: &[],
            });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("state_vector_compute_pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let bloch_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("bloch_reduce_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let bloch_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bloch_reduce_pipeline_layout"),
            bind_group_layouts: &[&bloch_bind_group_layout],
            push_constant_ranges: &[],
        });
        let bloch_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("bloch_reduce_pipeline"),
            layout: Some(&bloch_pipeline_layout),
            module: &bloch_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Measurement reduce/sample shares the bloch bind-group layout shape
        // (state in, aux out, params uniform).
        let measure_reduce_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("measure_reduce_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
        let measure_reduce_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("measure_reduce_pipeline_layout"),
                bind_group_layouts: &[&measure_reduce_bind_group_layout],
                push_constant_ranges: &[],
            });
        let measure_reduce_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("measure_reduce_pipeline"),
                layout: Some(&measure_reduce_pipeline_layout),
                module: &measure_reduce_shader,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });

        // Measurement collapse needs a 4-binding layout: state_in (read),
        // state_out (read_write), aux (read), params (uniform).
        let measure_collapse_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("measure_collapse_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
        let measure_collapse_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("measure_collapse_pipeline_layout"),
                bind_group_layouts: &[&measure_collapse_bind_group_layout],
                push_constant_ranges: &[],
            });
        let measure_collapse_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("measure_collapse_pipeline"),
                layout: Some(&measure_collapse_pipeline_layout),
                module: &measure_collapse_shader,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });

        let bloch_overlay_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bloch_overlay"),
            source: wgpu::ShaderSource::Wgsl(BLOCH_OVERLAY_SHADER.into()),
        });
        let measurement_digit_shader =
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("measurement_digit"),
                source: wgpu::ShaderSource::Wgsl(MEASUREMENT_DIGIT_SHADER.into()),
            });
        let bloch_overlay_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("bloch_overlay_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
        let measurement_digit_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("measurement_digit_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // Bake the digit atlas (Hack "0" / "1") once and upload to a GPU
        // texture sampled by `MEASUREMENT_DIGIT_SHADER`.
        let digit_atlas_data = rasterize_digit_atlas();
        let digit_atlas_texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("measurement_digit_atlas"),
                size: wgpu::Extent3d {
                    width: DIGIT_ATLAS_WIDTH,
                    height: DIGIT_ATLAS_HEIGHT,
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
            &digit_atlas_data,
        );
        let digit_atlas_view =
            digit_atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let digit_atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("measurement_digit_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let render_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("state_vector_render_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("state_vector_render_pipeline_layout"),
                bind_group_layouts: &[&render_bind_group_layout],
                push_constant_ranges: &[],
            });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("state_vector_quad_vertices"),
            contents: bytemuck::cast_slice(&[
                [-1.0f32, -1.0],
                [1.0, -1.0],
                [1.0, 1.0],
                [-1.0, 1.0],
            ]),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_data: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("state_vector_quad_indices"),
            contents: bytemuck::cast_slice(&index_data),
            usage: wgpu::BufferUsages::INDEX,
        });

        let state_seed_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("state_vector_ground_seed"),
            contents: bytemuck::cast_slice(&[1.0f32, 0.0f32]),
            usage: wgpu::BufferUsages::COPY_SRC,
        });

        let state_buffer_size =
            (MAX_STATE_COUNT * std::mem::size_of::<[f32; 2]>()) as wgpu::BufferAddress;
        let state_buffers = [
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("state_vector_buffer_a"),
                size: state_buffer_size,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("state_vector_buffer_b"),
                size: state_buffer_size,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
        ];

        let gate_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("state_vector_gate_params"),
            size: std::mem::size_of::<GateParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let gate_params_staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("state_vector_gate_params_staging"),
            size: (MAX_OPS_PER_RECOMPUTE * std::mem::size_of::<GateParams>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let bloch_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bloch_reduce_params"),
            size: std::mem::size_of::<BlochParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let bloch_params_staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bloch_reduce_params_staging"),
            size: (MAX_OPS_PER_RECOMPUTE * std::mem::size_of::<BlochParams>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let bloch_buffer_size = (MAX_BLOCH_SLOTS * 4 * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
        let bloch_output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bloch_output"),
            size: bloch_buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let measure_reduce_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("measure_reduce_params"),
            size: std::mem::size_of::<MeasureReduceParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let measure_reduce_params_staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("measure_reduce_params_staging"),
            size: (MAX_OPS_PER_RECOMPUTE * std::mem::size_of::<MeasureReduceParams>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let measure_collapse_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("measure_collapse_params"),
            size: std::mem::size_of::<MeasureCollapseParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let measure_collapse_params_staging_buffer =
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("measure_collapse_params_staging"),
                size: (MAX_OPS_PER_RECOMPUTE * std::mem::size_of::<MeasureCollapseParams>())
                    as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
        let measurement_aux_size =
            (MAX_MEASUREMENT_SLOTS * 4 * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
        let measurement_aux_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("measurement_aux"),
            size: measurement_aux_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let bloch_overlay_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bloch_overlay_params"),
            size: std::mem::size_of::<BlochOverlayParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bloch_overlay_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bloch_overlay_instances"),
            size: (MAX_BLOCH_SLOTS * std::mem::size_of::<BlochOverlayInstance>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bloch_overlay_vertex_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("bloch_overlay_quad_vertices"),
                contents: bytemuck::cast_slice(&[
                    [-1.0f32, -1.0],
                    [1.0, -1.0],
                    [1.0, 1.0],
                    [-1.0, 1.0],
                ]),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let bloch_overlay_index_data: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let bloch_overlay_index_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("bloch_overlay_quad_indices"),
                contents: bytemuck::cast_slice(&bloch_overlay_index_data),
                usage: wgpu::BufferUsages::INDEX,
            });

        let measurement_digit_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("measurement_digit_params"),
            size: std::mem::size_of::<MeasurementDigitParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let measurement_digit_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("measurement_digit_instances"),
            size: (MAX_MEASUREMENT_SLOTS * std::mem::size_of::<MeasurementDigitInstance>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let render_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("state_vector_render_params"),
            size: std::mem::size_of::<RenderParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let compute_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("state_vector_compute_a_to_b"),
                layout: &compute_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: state_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: state_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: gate_params_buffer.as_entire_binding(),
                    },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("state_vector_compute_b_to_a"),
                layout: &compute_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: state_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: state_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: gate_params_buffer.as_entire_binding(),
                    },
                ],
            }),
        ];

        let bloch_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bloch_reduce_read_a"),
                layout: &bloch_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: state_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: bloch_output_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: bloch_params_buffer.as_entire_binding(),
                    },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bloch_reduce_read_b"),
                layout: &bloch_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: state_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: bloch_output_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: bloch_params_buffer.as_entire_binding(),
                    },
                ],
            }),
        ];

        let measure_reduce_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("measure_reduce_read_a"),
                layout: &measure_reduce_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: state_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: measurement_aux_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: measure_reduce_params_buffer.as_entire_binding(),
                    },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("measure_reduce_read_b"),
                layout: &measure_reduce_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: state_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: measurement_aux_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: measure_reduce_params_buffer.as_entire_binding(),
                    },
                ],
            }),
        ];

        let measure_collapse_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("measure_collapse_a_to_b"),
                layout: &measure_collapse_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: state_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: state_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: measurement_aux_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: measure_collapse_params_buffer.as_entire_binding(),
                    },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("measure_collapse_b_to_a"),
                layout: &measure_collapse_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: state_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: state_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: measurement_aux_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: measure_collapse_params_buffer.as_entire_binding(),
                    },
                ],
            }),
        ];

        let render_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("state_vector_render_a"),
                layout: &render_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: state_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: render_params_buffer.as_entire_binding(),
                    },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("state_vector_render_b"),
                layout: &render_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: state_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: render_params_buffer.as_entire_binding(),
                    },
                ],
            }),
        ];

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        };

        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<StateInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 8,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 12,
                    shader_location: 3,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 16,
                    shader_location: 4,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32,
                    offset: 20,
                    shader_location: 5,
                },
            ],
        };

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("state_vector_render_pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &render_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[vertex_layout, instance_layout],
            },
            fragment: Some(wgpu::FragmentState {
                module: &render_shader,
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
        });

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("state_vector_instances"),
            size: (MAX_STATE_COUNT * std::mem::size_of::<StateInstance>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bloch_overlay_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bloch_overlay_bind_group"),
                layout: &bloch_overlay_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: bloch_output_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: bloch_overlay_params_buffer.as_entire_binding(),
                    },
                ],
            });
        let bloch_overlay_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("bloch_overlay_pipeline_layout"),
                bind_group_layouts: &[&bloch_overlay_bind_group_layout],
                push_constant_ranges: &[],
            });
        let bloch_overlay_vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        };
        let bloch_overlay_instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<BlochOverlayInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 8,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 12,
                    shader_location: 3,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32,
                    offset: 16,
                    shader_location: 4,
                },
            ],
        };
        let bloch_overlay_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("bloch_overlay_pipeline"),
                layout: Some(&bloch_overlay_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &bloch_overlay_shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[bloch_overlay_vertex_layout, bloch_overlay_instance_layout],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &bloch_overlay_shader,
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
            });

        let measurement_digit_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("measurement_digit_bind_group"),
                layout: &measurement_digit_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: measurement_aux_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: measurement_digit_params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&digit_atlas_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&digit_atlas_sampler),
                    },
                ],
            });
        let measurement_digit_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("measurement_digit_pipeline_layout"),
                bind_group_layouts: &[&measurement_digit_bind_group_layout],
                push_constant_ranges: &[],
            });
        let measurement_digit_vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        };
        let measurement_digit_instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<MeasurementDigitInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 8,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32,
                    offset: 12,
                    shader_location: 3,
                },
            ],
        };
        let measurement_digit_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("measurement_digit_pipeline"),
                layout: Some(&measurement_digit_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &measurement_digit_shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[
                        measurement_digit_vertex_layout,
                        measurement_digit_instance_layout,
                    ],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &measurement_digit_shader,
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
            });

        Self {
            compute_pipeline,
            render_pipeline,
            compute_bind_groups,
            render_bind_groups,
            render_bind_group_layout,
            gate_params_buffer,
            gate_params_staging_buffer,
            render_params_buffer,
            state_buffers,
            state_seed_buffer,
            instance_buffer,
            vertex_buffer,
            index_buffer,
            index_count: index_data.len() as u32,
            target_format,
            state_count: 0,
            active_state: 0,
            bloch_pipeline,
            bloch_bind_groups,
            bloch_params_buffer,
            bloch_params_staging_buffer,
            bloch_output_buffer,
            measure_reduce_pipeline,
            measure_reduce_bind_groups,
            measure_reduce_params_buffer,
            measure_reduce_params_staging_buffer,
            measure_collapse_pipeline,
            measure_collapse_bind_groups,
            measure_collapse_params_buffer,
            measure_collapse_params_staging_buffer,
            measurement_aux_buffer,
            bloch_overlay_pipeline,
            bloch_overlay_bind_group,
            bloch_overlay_params_buffer,
            bloch_overlay_instance_buffer,
            bloch_overlay_vertex_buffer,
            bloch_overlay_index_buffer,
            measurement_digit_pipeline,
            measurement_digit_bind_group,
            measurement_digit_params_buffer,
            measurement_digit_instance_buffer,
            last_render_params: None,
            last_bloch_overlay_params: None,
            last_measurement_digit_params: None,
        }
    }

    pub(crate) fn update_render_pipeline(
        &mut self,
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
    ) {
        if self.target_format == target_format {
            return;
        }
        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("state_vector_render"),
            source: wgpu::ShaderSource::Wgsl(STATE_RENDER_SHADER.into()),
        });
        let render_bind_group_layout = &self.render_bind_group_layout;
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("state_vector_render_pipeline_layout"),
            bind_group_layouts: &[render_bind_group_layout],
            push_constant_ranges: &[],
        });
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        };
        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<StateInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 8,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 12,
                    shader_location: 3,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 16,
                    shader_location: 4,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32,
                    offset: 20,
                    shader_location: 5,
                },
            ],
        };
        self.render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("state_vector_render_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &render_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[vertex_layout, instance_layout],
            },
            fragment: Some(wgpu::FragmentState {
                module: &render_shader,
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
        });
        self.target_format = target_format;
    }
}
