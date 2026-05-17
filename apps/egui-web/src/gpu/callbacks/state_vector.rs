use eframe::egui;
use eframe::{egui_wgpu, wgpu};

use crate::simulation_plan::SimulationOp;

use super::super::params::RenderParams;
use super::super::readback::{
    BlochGpuHandle, ChanceGpuHandle, GpuReadbackState, MeasurementGpuHandle, BLOCH_GPU_HANDLE,
    CHANCE_GPU_HANDLE, GPU_READBACK, MEASUREMENT_GPU_HANDLE,
};
use super::super::recompute::recompute_state_vector;
use super::super::resources::StateVectorResources;

pub(crate) struct StateVectorCallback {
    /// Linearised simulation ops for the GPU pipeline. Includes gate
    /// application, readout captures, measurement collapse, and optional
    /// state-panel snapshots. The GPU dispatches them in order; ping-pong of
    /// the state buffers happens for any op that mutates state (gates and
    /// `MeasureCollapse`).
    pub(crate) sim_ops: Vec<SimulationOp>,
    pub(crate) state_count: usize,
    pub(crate) recompute: bool,
    pub(crate) target_format: wgpu::TextureFormat,
    /// Pre-built render params describing the panel geometry, cell pitch,
    /// circle radii, and palette. Built by `render.rs` from
    /// `StatePanelLayout` and passed straight through to the uniform
    /// buffer; the fragment shader uses every field to figure out which
    /// cell each pixel belongs to and how to draw its state circle.
    pub(crate) render_params: RenderParams,
}

impl egui_wgpu::CallbackTrait for StateVectorCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let resources = if callback_resources.contains::<StateVectorResources>() {
            callback_resources
                .get_mut::<StateVectorResources>()
                .expect("StateVectorResources missing")
        } else {
            callback_resources.insert(StateVectorResources::new(device, queue, self.target_format));
            callback_resources
                .get_mut::<StateVectorResources>()
                .expect("StateVectorResources just inserted")
        };

        resources.update_target_format(device, self.target_format);

        if resources.state.last_render_params != Some(self.render_params) {
            queue.write_buffer(
                &resources.state.render_params_buffer,
                0,
                bytemuck::bytes_of(&self.render_params),
            );
            resources.state.last_render_params = Some(self.render_params);
        }

        if (self.recompute || resources.state_count != self.state_count)
            && !recompute_state_vector(device, queue, resources, &self.sim_ops, self.state_count)
        {
            return Vec::new();
        }

        GPU_READBACK.with(|slot| {
            *slot.borrow_mut() = Some(GpuReadbackState {
                device: device.clone(),
                queue: queue.clone(),
                state_buffers: [
                    resources.common.state_buffers[0].clone(),
                    resources.common.state_buffers[1].clone(),
                    resources.common.state_preview_buffer.clone(),
                ],
                state_count: resources.state_count,
                active_state: resources.active_state,
            });
        });
        BLOCH_GPU_HANDLE.with(|slot| {
            *slot.borrow_mut() = Some(BlochGpuHandle {
                device: device.clone(),
                queue: queue.clone(),
                output_buffer: resources.bloch.output_buffer.clone(),
            });
        });
        MEASUREMENT_GPU_HANDLE.with(|slot| {
            *slot.borrow_mut() = Some(MeasurementGpuHandle {
                device: device.clone(),
                queue: queue.clone(),
                aux_buffer: resources.measure.aux_buffer.clone(),
            });
        });
        CHANCE_GPU_HANDLE.with(|slot| {
            *slot.borrow_mut() = Some(ChanceGpuHandle {
                device: device.clone(),
                queue: queue.clone(),
                output_buffer: resources.chance.output_buffer.clone(),
            });
        });

        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(resources) = callback_resources.get::<StateVectorResources>() else {
            return;
        };
        if self.render_params.cols == 0 || self.render_params.rows == 0 {
            return;
        }
        render_pass.set_pipeline(&resources.state.render_pipeline);
        render_pass.set_bind_group(
            0,
            &resources.state.render_bind_groups[resources.active_state],
            &[],
        );
        render_pass.set_vertex_buffer(0, resources.common.unit_quad_vertex_buffer.slice(..));
        render_pass.set_index_buffer(
            resources.common.unit_quad_index_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        // One instanced draw of the panel quad — the fragment shader splits
        // it into per-cell circles. Replaces the previous N-instance loop
        // (one quad per cell). See gpu/shaders.rs::STATE_RENDER_SHADER.
        render_pass.draw_indexed(0..resources.common.unit_quad_index_count, 0, 0..1);
    }
}
