//! Bloch reduction (compute) + dynamic Bloch arrow overlay (render).
//!
//! * `reduce_pipeline` — compute pass that reads the active state
//!   buffer and writes `(x, y, z, len)` per qubit into
//!   `output_buffer`. Two bind groups so we can read from either
//!   ping-pong state buffer.
//! * `overlay_pipeline` — render pass that samples `output_buffer`
//!   in the fragment shader and draws the arrow + tip dot directly,
//!   no CPU readback. Per-frame instance data lives in
//!   `overlay_instance_buffer`.
//!
//! `output_buffer` is the bridge between the two: written by the
//! compute pipeline, read by the overlay's fragment stage.
//!
//! Bug fix vs the previous monolithic `update_render_pipeline`: this
//! module's `update_target_format` *also* rebuilds the overlay
//! pipeline, not just the state-vector render pipeline. The earlier
//! code silently kept the overlay pipeline pinned to the original
//! surface format.

mod overlay;
mod pipeline;
mod reduce;

use eframe::wgpu;

use super::super::params::BlochOverlayParams;
use super::common::Common;

pub(crate) struct BlochResources {
    // --- reduce (compute) ---
    pub reduce_pipeline: wgpu::ComputePipeline,
    pub reduce_bind_groups: [wgpu::BindGroup; 2],
    pub params_buffer: wgpu::Buffer,
    /// See `state::gate_params_staging_buffer` — same staging pattern.
    pub params_staging_buffer: wgpu::Buffer,
    pub output_buffer: wgpu::Buffer,

    // --- overlay (render) ---
    pub overlay_pipeline: wgpu::RenderPipeline,
    pub overlay_bind_group: wgpu::BindGroup,
    pub overlay_bind_group_layout: wgpu::BindGroupLayout,
    pub overlay_params_buffer: wgpu::Buffer,
    pub overlay_instance_buffer: wgpu::Buffer,
    pub last_overlay_params: Option<BlochOverlayParams>,
}

impl BlochResources {
    pub(super) fn build(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        common: &Common,
    ) -> Self {
        let reduce = reduce::build(device, common);
        let overlay = overlay::build(device, target_format, &reduce.output_buffer);

        Self {
            reduce_pipeline: reduce.pipeline,
            reduce_bind_groups: reduce.bind_groups,
            params_buffer: reduce.params_buffer,
            params_staging_buffer: reduce.params_staging_buffer,
            output_buffer: reduce.output_buffer,
            overlay_pipeline: overlay.pipeline,
            overlay_bind_group: overlay.bind_group,
            overlay_bind_group_layout: overlay.bind_group_layout,
            overlay_params_buffer: overlay.params_buffer,
            overlay_instance_buffer: overlay.instance_buffer,
            last_overlay_params: None,
        }
    }

    pub(super) fn update_target_format(
        &mut self,
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
    ) {
        self.overlay_pipeline = pipeline::build_overlay_pipeline(
            device,
            target_format,
            &self.overlay_bind_group_layout,
        );
    }
}
