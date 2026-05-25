use std::sync::Arc;

use eframe::egui;
use eframe::{egui_wgpu, wgpu};

use super::super::params::{BlochOverlayInstance, BlochOverlayParams, ExternalBlochUploadBatch};
use super::super::readback::{BlochGpuHandle, BLOCH_GPU_HANDLE, BLOCH_SLOT_MAP};
use super::super::resources::StateVectorResources;

/// Renders the dynamic Bloch arrow + tip dot for every placed Bloch display
/// directly from `bloch_output_buffer`. No CPU readback in production —
/// `BlochOverlayInstance` carries (screen center, radius, output_slot) and
/// the fragment shader reads (x, y, z) straight from the GPU buffer the
/// reduction shader just wrote.
pub(crate) struct BlochOverlayCallback {
    pub(crate) instances: Arc<[BlochOverlayInstance]>,
    /// CSS-pixel rect of the egui callback viewport (= the rect we passed
    /// to `Callback::new_paint_callback`). NDC -1..1 maps to this, not the
    /// full canvas, so the shader needs both `min` and `size`.
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) line_color: [f32; 4],
    pub(crate) tip_0_color: [f32; 4],
    pub(crate) tip_mid_color: [f32; 4],
    pub(crate) tip_1_color: [f32; 4],
    pub(crate) tip_outline_color: [f32; 4],
    pub(crate) zero_color: [f32; 4],
    pub(crate) external_uploads: Option<ExternalBlochUploadBatch>,
}

impl BlochOverlayCallback {
    fn upload_external_bloch(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &mut StateVectorResources,
    ) {
        let Some(batch) = &self.external_uploads else {
            return;
        };
        if resources.bloch.last_external_upload_generation == Some(batch.generation) {
            return;
        }
        for upload in batch.uploads.iter() {
            let offset = upload.slot as usize * std::mem::size_of::<[f32; 4]>();
            queue.write_buffer(
                &resources.bloch.output_buffer,
                offset as wgpu::BufferAddress,
                bytemuck::bytes_of(&upload.vector),
            );
        }
        resources.bloch.last_external_upload_generation = Some(batch.generation);
        BLOCH_SLOT_MAP.with(|cell| {
            *cell.borrow_mut() = batch.slot_to_gate_id.to_vec();
        });
        BLOCH_GPU_HANDLE.with(|slot| {
            *slot.borrow_mut() = Some(BlochGpuHandle {
                device: device.clone(),
                queue: queue.clone(),
                output_buffer: resources.bloch.output_buffer.clone(),
            });
        });
    }
}

impl egui_wgpu::CallbackTrait for BlochOverlayCallback {
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
        self.upload_external_bloch(device, queue, resources);
        let params = BlochOverlayParams {
            viewport_min: self.viewport_min,
            viewport_size: self.viewport_size,
            line_color: self.line_color,
            tip_0_color: self.tip_0_color,
            tip_mid_color: self.tip_mid_color,
            tip_1_color: self.tip_1_color,
            tip_outline_color: self.tip_outline_color,
            zero_color: self.zero_color,
        };
        if resources.bloch.last_overlay_params != Some(params) {
            queue.write_buffer(
                &resources.bloch.overlay_params_buffer,
                0,
                bytemuck::bytes_of(&params),
            );
            resources.bloch.last_overlay_params = Some(params);
        }
        queue.write_buffer(
            &resources.bloch.overlay_instance_buffer,
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
        render_pass.set_pipeline(&resources.bloch.overlay_pipeline);
        render_pass.set_bind_group(0, &resources.bloch.overlay_bind_group, &[]);
        render_pass.set_vertex_buffer(0, resources.common.unit_quad_vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, resources.bloch.overlay_instance_buffer.slice(..));
        render_pass.set_index_buffer(
            resources.common.unit_quad_index_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        render_pass.draw_indexed(0..6, 0, 0..self.instances.len() as u32);
    }
}
