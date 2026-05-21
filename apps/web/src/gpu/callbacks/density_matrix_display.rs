use std::sync::Arc;

use eframe::egui;
use eframe::{egui_wgpu, wgpu};

use super::super::params::{
    DensityInstance, DensityRenderParams, ExternalDensityUploadBatch, DENSITY_VALUES_PER_SLOT,
};
use super::super::readback::{DensityGpuHandle, DENSITY_GPU_HANDLE, DENSITY_SLOT_MAP};
use super::super::resources::StateVectorResources;

/// Renders Density Matrix display cells straight from the GPU density buffer.
/// The CPU supplies only geometry, colours, render mode, and hovered cell index.
pub(crate) struct DensityMatrixDisplayCallback {
    pub(crate) instances: Arc<[DensityInstance]>,
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) background: [f32; 4],
    pub(crate) drag_background: [f32; 4],
    pub(crate) border: [f32; 4],
    pub(crate) disk: [f32; 4],
    pub(crate) disk_border: [f32; 4],
    pub(crate) outline: [f32; 4],
    pub(crate) outline_zero: [f32; 4],
    pub(crate) needle: [f32; 4],
    pub(crate) hover_border: [f32; 4],
    pub(crate) placeholder_background: [f32; 4],
    pub(crate) external_uploads: Option<ExternalDensityUploadBatch>,
}

impl DensityMatrixDisplayCallback {
    fn upload_external_density(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &mut StateVectorResources,
    ) {
        let Some(batch) = &self.external_uploads else {
            return;
        };
        if resources.density.last_external_upload_generation == Some(batch.generation) {
            return;
        }
        for upload in batch.uploads.iter() {
            let value_offset =
                upload.slot as usize * DENSITY_VALUES_PER_SLOT * std::mem::size_of::<[f32; 2]>();
            queue.write_buffer(
                &resources.density.output_buffer,
                value_offset as wgpu::BufferAddress,
                bytemuck::cast_slice(upload.cells.as_ref()),
            );
            let meta_offset = upload.slot as usize * std::mem::size_of::<[f32; 4]>();
            queue.write_buffer(
                &resources.density.meta_buffer,
                meta_offset as wgpu::BufferAddress,
                bytemuck::bytes_of(&upload.meta),
            );
        }
        resources.density.last_external_upload_generation = Some(batch.generation);
        DENSITY_SLOT_MAP.with(|cell| {
            *cell.borrow_mut() = batch.slot_to_gate_id.to_vec();
        });
        DENSITY_GPU_HANDLE.with(|slot| {
            *slot.borrow_mut() = Some(DensityGpuHandle {
                device: device.clone(),
                queue: queue.clone(),
                output_buffer: resources.density.output_buffer.clone(),
                meta_buffer: resources.density.meta_buffer.clone(),
            });
        });
    }
}

impl egui_wgpu::CallbackTrait for DensityMatrixDisplayCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(resources) = callback_resources.get_mut::<StateVectorResources>() else {
            return Vec::new();
        };
        if self.instances.is_empty() {
            return Vec::new();
        }
        self.upload_external_density(device, queue, resources);
        let params = DensityRenderParams {
            viewport_min: self.viewport_min,
            viewport_size: self.viewport_size,
            background: self.background,
            drag_background: self.drag_background,
            border: self.border,
            disk: self.disk,
            disk_border: self.disk_border,
            outline: self.outline,
            outline_zero: self.outline_zero,
            needle: self.needle,
            hover_border: self.hover_border,
            placeholder_background: self.placeholder_background,
        };
        if resources.density.last_render_params != Some(params) {
            queue.write_buffer(
                &resources.density.render_params_buffer,
                0,
                bytemuck::bytes_of(&params),
            );
            resources.density.last_render_params = Some(params);
        }
        queue.write_buffer(
            &resources.density.render_instance_buffer,
            0,
            bytemuck::cast_slice(self.instances.as_ref()),
        );
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
        render_pass.set_pipeline(&resources.density.render_pipeline);
        render_pass.set_bind_group(0, &resources.density.render_bind_group, &[]);
        render_pass.set_vertex_buffer(0, resources.common.unit_quad_vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, resources.density.render_instance_buffer.slice(..));
        render_pass.set_index_buffer(
            resources.common.unit_quad_index_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        render_pass.draw_indexed(0..6, 0, 0..self.instances.len() as u32);
    }
}
