use eframe::wgpu;

use super::super::params::{
    MeasurementDigitParams, SurvivalLabelInstance, MAX_OPS_PER_RECOMPUTE, MAX_POSTSELECTION_SLOTS,
};
use super::super::survival_glyph_atlas::{
    rasterize_survival_glyph_atlas, SURVIVAL_GLYPH_ATLAS_HEIGHT, SURVIVAL_GLYPH_ATLAS_WIDTH,
};
use super::common::Common;

mod pipeline;

pub(crate) struct PostselectionResources {
    pub(crate) reduce_pipeline: wgpu::ComputePipeline,
    pub(crate) normalize_pipeline: wgpu::ComputePipeline,
    pub(crate) reduce_bind_groups: [wgpu::BindGroup; 2],
    pub(crate) normalize_bind_groups: [wgpu::BindGroup; 2],
    pub(crate) params_buffer: wgpu::Buffer,
    pub(crate) params_staging_buffer: wgpu::Buffer,
    pub(crate) output_buffer: wgpu::Buffer,
    pub(crate) survival_buffer: wgpu::Buffer,
    pub(crate) pipeline: wgpu::RenderPipeline,
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) render_params_buffer: wgpu::Buffer,
    pub(crate) instance_buffer: wgpu::Buffer,
    pub(crate) last_params: Option<MeasurementDigitParams>,
}

fn buffer(device: &wgpu::Device, size: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("postselection"),
        size,
        usage,
        mapped_at_creation: false,
    })
}

fn entry(binding: u32, ty: wgpu::BufferBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn groups(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    common: &Common,
    buffers: &[&wgpu::Buffer],
    normalize: bool,
) -> [wgpu::BindGroup; 2] {
    std::array::from_fn(|i| {
        let mut resources = vec![&common.state_buffers[i]];
        if normalize {
            resources.push(&common.state_buffers[1 - i]);
        }
        resources.extend_from_slice(buffers);
        let entries: Vec<_> = resources
            .iter()
            .enumerate()
            .map(|(binding, b)| wgpu::BindGroupEntry {
                binding: binding as u32,
                resource: b.as_entire_binding(),
            })
            .collect();
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("postselection"),
            layout,
            entries: &entries,
        })
    })
}

fn compute(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    source: &str,
) -> wgpu::ComputePipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("postselection"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("postselection"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("postselection"),
        layout: Some(&layout),
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    })
}

impl PostselectionResources {
    pub(super) fn build(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        common: &Common,
    ) -> Self {
        use wgpu::BufferBindingType as T;
        use wgpu::BufferUsages as U;
        let output_buffer = buffer(
            device,
            (MAX_POSTSELECTION_SLOTS * 16) as u64,
            U::STORAGE | U::COPY_SRC | U::COPY_DST,
        );
        let survival_buffer = buffer(device, 4, U::STORAGE | U::COPY_DST);
        let params_buffer = buffer(device, 16, U::UNIFORM | U::COPY_DST);
        let params_staging_buffer = buffer(
            device,
            (MAX_OPS_PER_RECOMPUTE * 16) as u64,
            U::COPY_SRC | U::COPY_DST,
        );
        let reduce_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("postselection_reduce"),
            entries: &[
                entry(0, T::Storage { read_only: true }),
                entry(1, T::Storage { read_only: false }),
                entry(2, T::Storage { read_only: false }),
                entry(3, T::Uniform),
            ],
        });
        let normalize_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("postselection_normalize"),
            entries: &[
                entry(0, T::Storage { read_only: true }),
                entry(1, T::Storage { read_only: false }),
                entry(2, T::Storage { read_only: true }),
                entry(3, T::Uniform),
            ],
        });
        let reduce_pipeline = compute(
            device,
            &reduce_layout,
            include_str!("../shaders/postselection_reduce.wgsl"),
        );
        let normalize_pipeline = compute(
            device,
            &normalize_layout,
            include_str!("../shaders/postselection_normalize.wgsl"),
        );
        let reduce_bind_groups = groups(
            device,
            &reduce_layout,
            common,
            &[&output_buffer, &survival_buffer, &params_buffer],
            false,
        );
        let normalize_bind_groups = groups(
            device,
            &normalize_layout,
            common,
            &[&output_buffer, &params_buffer],
            true,
        );
        let mut entries = vec![
            entry(0, T::Storage { read_only: true }),
            entry(1, T::Uniform),
        ];
        entries
            .iter_mut()
            .for_each(|e| e.visibility = wgpu::ShaderStages::VERTEX_FRAGMENT);
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        });
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 3,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("postselection_render"),
            entries: &entries,
        });
        let render_params_buffer = buffer(
            device,
            std::mem::size_of::<MeasurementDigitParams>() as u64,
            U::UNIFORM | U::COPY_DST,
        );
        let instance_buffer = buffer(
            device,
            ((MAX_POSTSELECTION_SLOTS + 1) * std::mem::size_of::<SurvivalLabelInstance>()) as u64,
            U::VERTEX | U::COPY_DST,
        );
        let texture = wgpu::util::DeviceExt::create_texture_with_data(
            device,
            queue,
            &wgpu::TextureDescriptor {
                label: Some("postselection_atlas"),
                size: wgpu::Extent3d {
                    width: SURVIVAL_GLYPH_ATLAS_WIDTH,
                    height: SURVIVAL_GLYPH_ATLAS_HEIGHT,
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
            &rasterize_survival_glyph_atlas(),
        );
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("postselection_render"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: output_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: render_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let pipeline = pipeline::build_pipeline(device, target_format, &bind_group_layout);
        Self {
            reduce_pipeline,
            normalize_pipeline,
            reduce_bind_groups,
            normalize_bind_groups,
            params_buffer,
            params_staging_buffer,
            output_buffer,
            survival_buffer,
            pipeline,
            bind_group,
            bind_group_layout,
            render_params_buffer,
            instance_buffer,
            last_params: None,
        }
    }

    pub(super) fn update_target_format(
        &mut self,
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
    ) {
        self.pipeline = pipeline::build_pipeline(device, target_format, &self.bind_group_layout);
    }
}
