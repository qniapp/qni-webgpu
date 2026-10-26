use std::sync::Arc;

use eframe::egui;
use eframe::{egui_wgpu, wgpu};

use super::super::params::{
    ExternalProbabilityUploadBatch, ProbabilityInstance, ProbabilityRenderParams,
    MAX_PROBABILITY_AGGREGATE_ROWS, MAX_PROBABILITY_OUTCOMES, PROBABILITY_AGGREGATE_MIN_SPAN,
    PROBABILITY_RENDER_MODE_SAMPLE,
};
use super::super::readback::PROBABILITY_SLOT_MAP;
#[cfg(target_arch = "wasm32")]
use super::super::readback::{ProbabilityGpuHandle, PROBABILITY_GPU_HANDLE};
use super::super::resources::StateVectorResources;

pub(crate) struct ProbabilityDisplayCallback {
    pub(crate) instances: Arc<[ProbabilityInstance]>,
    pub(crate) use_drag_preview_buffer: bool,
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) background: [f32; 4],
    pub(crate) placeholder_background: [f32; 4],
    pub(crate) border: [f32; 4],
    pub(crate) log_hint: [f32; 4],
    pub(crate) bar: [f32; 4],
    pub(crate) sample_on_core: [f32; 4],
    pub(crate) sample_on_fill: [f32; 4],
    pub(crate) sample_on_glow: [f32; 4],
    pub(crate) sample_off_fill: [f32; 4],
    pub(crate) sample_off_edge: [f32; 4],
    pub(crate) bar_edge: [f32; 4],
    pub(crate) hover_border: [f32; 4],
    pub(crate) text_color: [f32; 4],
    pub(crate) external_uploads: Option<ExternalProbabilityUploadBatch>,
}

impl ProbabilityDisplayCallback {
    fn upload_external_probabilities(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &mut StateVectorResources,
    ) {
        let Some(batch) = &self.external_uploads else {
            return;
        };
        if resources.probability.last_external_upload_generation == Some(batch.generation) {
            return;
        }
        for upload in batch.uploads.iter() {
            let offset =
                upload.slot as usize * MAX_PROBABILITY_OUTCOMES * std::mem::size_of::<f32>();
            queue.write_buffer(
                &resources.probability.output_buffer,
                offset as wgpu::BufferAddress,
                bytemuck::cast_slice(upload.probabilities.as_slice()),
            );
        }
        resources.probability.last_external_upload_generation = Some(batch.generation);
        PROBABILITY_SLOT_MAP.with(|cell| {
            *cell.borrow_mut() = batch.slot_to_gate_id.to_vec();
        });
        #[cfg(target_arch = "wasm32")]
        PROBABILITY_GPU_HANDLE.with(|slot| {
            *slot.borrow_mut() = Some(ProbabilityGpuHandle {
                device: _device.clone(),
                queue: queue.clone(),
                output_buffer: resources.probability.output_buffer.clone(),
            });
        });
    }
}

impl egui_wgpu::CallbackTrait for ProbabilityDisplayCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(resources) = callback_resources.get_mut::<StateVectorResources>() else {
            return Vec::new();
        };
        if self.instances.is_empty() {
            return Vec::new();
        }
        if !self.use_drag_preview_buffer {
            self.upload_external_probabilities(device, queue, resources);
            #[cfg(target_arch = "wasm32")]
            super::super::readback::SAMPLE_DISPLAY_SPANS.with(|cell| {
                let mut spans = cell.borrow_mut();
                spans.clear();
                spans.extend(
                    self.instances
                        .iter()
                        .filter(|instance| {
                            instance.render_mode == ProbabilityInstance::SAMPLED_BITSTRING
                        })
                        .map(|instance| (instance.seed, instance.span)),
                );
            });
        }
        let params = ProbabilityRenderParams {
            viewport_min: self.viewport_min,
            viewport_size: self.viewport_size,
            pixels_per_point: screen_descriptor.pixels_per_point,
            _pad_ppp: [0.0; 3],
            background: self.background,
            placeholder_background: self.placeholder_background,
            border: self.border,
            log_hint: self.log_hint,
            bar: self.bar,
            sample_on_core: self.sample_on_core,
            sample_on_fill: self.sample_on_fill,
            sample_on_glow: self.sample_on_glow,
            sample_off_fill: self.sample_off_fill,
            sample_off_edge: self.sample_off_edge,
            bar_edge: self.bar_edge,
            hover_border: self.hover_border,
            text_color: self.text_color,
        };
        let probability = &mut resources.probability;
        let (params_buffer, instance_buffer, last_params) = if self.use_drag_preview_buffer {
            (
                &probability.drag_render_params_buffer,
                &probability.drag_render_instance_buffer,
                &mut probability.last_drag_render_params,
            )
        } else {
            (
                &probability.render_params_buffer,
                &probability.render_instance_buffer,
                &mut probability.last_render_params,
            )
        };
        if *last_params != Some(params) {
            queue.write_buffer(params_buffer, 0, bytemuck::bytes_of(&params));
            *last_params = Some(params);
        }
        queue.write_buffer(
            instance_buffer,
            0,
            bytemuck::cast_slice(self.instances.as_ref()),
        );
        if !self.use_drag_preview_buffer
            && self.instances.iter().any(|instance| {
                instance.render_mode == PROBABILITY_RENDER_MODE_SAMPLE
                    && instance.span >= PROBABILITY_AGGREGATE_MIN_SPAN
            })
        {
            let mut pass = egui_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("probability_aggregate_rows_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&resources.probability.aggregate_pipeline);
            pass.set_bind_group(0, &resources.probability.aggregate_bind_group, &[]);
            pass.dispatch_workgroups(
                (MAX_PROBABILITY_AGGREGATE_ROWS as u32).div_ceil(64),
                self.instances.len() as u32,
                1,
            );
        }
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
        if self.instances.is_empty() {
            return;
        }
        render_pass.set_pipeline(&resources.probability.render_pipeline);
        let (bind_group, instance_buffer) = if self.use_drag_preview_buffer {
            (
                &resources.probability.drag_render_bind_group,
                &resources.probability.drag_render_instance_buffer,
            )
        } else {
            (
                &resources.probability.render_bind_group,
                &resources.probability.render_instance_buffer,
            )
        };
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, resources.common.unit_quad_vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
        render_pass.set_index_buffer(
            resources.common.unit_quad_index_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        render_pass.draw_indexed(0..6, 0, 0..self.instances.len() as u32);
    }
}
