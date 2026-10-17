use super::common::Common;
use eframe::wgpu;

pub(crate) struct StateStatusResources {
    pipeline: wgpu::ComputePipeline,
    bind_groups: [wgpu::BindGroup; 3],
    params_buffer: wgpu::Buffer,
}

impl StateStatusResources {
    pub(super) fn build(device: &wgpu::Device, common: &Common) -> Self {
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("selected_state_norm_params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let entries: Vec<_> = [
            wgpu::BufferBindingType::Storage { read_only: true },
            wgpu::BufferBindingType::Storage { read_only: false },
            wgpu::BufferBindingType::Uniform,
        ]
        .into_iter()
        .enumerate()
        .map(|(binding, ty)| wgpu::BindGroupLayoutEntry {
            binding: binding as u32,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        })
        .collect();
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("selected_state_norm"),
            entries: &entries,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("selected_state_norm"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/state_status.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("selected_state_norm"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("selected_state_norm"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let buffers = [
            &common.state_buffers[0],
            &common.state_buffers[1],
            &common.state_preview_buffer,
        ];
        let bind_groups = std::array::from_fn(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("selected_state_norm"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffers[i].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: common.selected_state_norm_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: params_buffer.as_entire_binding(),
                    },
                ],
            })
        });
        Self {
            pipeline,
            bind_groups,
            params_buffer,
        }
    }

    pub(crate) fn encode(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        active_state: usize,
        state_count: usize,
    ) {
        queue.write_buffer(
            &self.params_buffer,
            0,
            bytemuck::cast_slice(&[state_count as u32, 0, 0, 0]),
        );
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("selected_state_norm"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_groups[active_state], &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
}
