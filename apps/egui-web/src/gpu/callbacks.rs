//! `egui_wgpu::CallbackTrait` implementations.
//!
//! Three callbacks: the Bloch overlay (renders arrow + tip dot from
//! `bloch_output_buffer`), the measurement digit overlay (renders 0/1
//! glyphs sampled from `measurement_aux_buffer`), and the state vector
//! callback (drives the entire recompute — gate dispatches, Bloch
//! captures, measurement reduce/collapse, and the state circle render
//! pass). All three pull resources from the shared
//! `StateVectorResources` slot inside `egui_wgpu::CallbackResources`.

use std::sync::Arc;

use eframe::egui;
use eframe::{egui_wgpu, wgpu};

use crate::bloch::{validate_simulation_plan_capacity, SimulationOp, SimulationPlanLimits};
use crate::gates::GateParams;

use super::params::{
    BlochOverlayInstance, BlochOverlayParams, BlochParams, MeasureCollapseParams,
    MeasureReduceParams, MeasurementDigitInstance, MeasurementDigitParams, PopupValueParams,
    RenderParams, MAX_BLOCH_SLOTS, MAX_MEASUREMENT_SLOTS, MAX_OPS_PER_RECOMPUTE,
    STATE_WORKGROUP_SIZE,
};
use super::readback::{
    BlochGpuHandle, GpuReadbackState, MeasurementGpuHandle, BLOCH_GPU_HANDLE, BLOCH_SLOT_MAP,
    GPU_READBACK, MEASUREMENT_GPU_HANDLE, MEASUREMENT_SLOT_MAP,
};
use super::resources::StateVectorResources;

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
    pub(crate) tip_color: [f32; 4],
    pub(crate) zero_color: [f32; 4],
}

impl egui_wgpu::CallbackTrait for BlochOverlayCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
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
        let params = BlochOverlayParams {
            viewport_min: self.viewport_min,
            viewport_size: self.viewport_size,
            line_color: self.line_color,
            tip_color: self.tip_color,
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

/// Renders the 0/1 measurement digit straight from
/// `measurement_aux_buffer.z` (the GPU-sampled outcome). Static meter icon
/// (purple or zinc-200 ring) is still painted by egui — only the digit is
/// quantum-state-derived.
pub(crate) struct MeasurementDigitCallback {
    pub(crate) instances: Arc<[MeasurementDigitInstance]>,
    /// See `BlochOverlayCallback::viewport_min`.
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) zero_color: [f32; 4],
    pub(crate) one_color: [f32; 4],
}

impl egui_wgpu::CallbackTrait for MeasurementDigitCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
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
        let params = MeasurementDigitParams {
            viewport_min: self.viewport_min,
            viewport_size: self.viewport_size,
            zero_color: self.zero_color,
            one_color: self.one_color,
        };
        if resources.digit.last_params != Some(params) {
            queue.write_buffer(
                &resources.digit.params_buffer,
                0,
                bytemuck::bytes_of(&params),
            );
            resources.digit.last_params = Some(params);
        }
        queue.write_buffer(
            &resources.digit.instance_buffer,
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
        render_pass.set_pipeline(&resources.digit.pipeline);
        render_pass.set_bind_group(0, &resources.digit.bind_group, &[]);
        // Reuse the bloch overlay's quad geometry — both render full-rect
        // quads with `[-1..1]` corners.
        render_pass.set_vertex_buffer(0, resources.common.unit_quad_vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, resources.digit.instance_buffer.slice(..));
        render_pass.set_index_buffer(
            resources.common.unit_quad_index_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        render_pass.draw_indexed(0..6, 0, 0..self.instances.len() as u32);
    }
}

/// Renders the amplitude / probability / phase numeric rows of the
/// state-cell hover popup straight from the live state buffer. Three
/// instances (one per row), no vertex buffer — the shader uses
/// `@builtin(vertex_index)` to lay out the row quad and reads the
/// hovered cell's amplitude directly from `state_buffers[active]`.
/// The chrome / icons / labels are still painted by egui on the CPU
/// side; only the digit text is GPU-rendered.
pub(crate) struct PopupValueCallback {
    /// Egui callback viewport — see `BlochOverlayCallback::viewport_min`.
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    /// Top-left of the row-0 (amplitude) value text in egui pixels.
    pub(crate) value_anchor: [f32; 2],
    /// Vertical pitch between value rows (same `ROW_H` constant used by
    /// the popup chrome).
    pub(crate) row_pitch: f32,
    /// Atlas cell size in egui pixels — should match
    /// `(POPUP_GLYPH_CELL_W, POPUP_GLYPH_CELL_H)`.
    pub(crate) char_size: [f32; 2],
    pub(crate) text_color: [f32; 4],
    pub(crate) hovered_display_index: u32,
    pub(crate) qubits: u32,
}

impl egui_wgpu::CallbackTrait for PopupValueCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(resources) = callback_resources.get_mut::<StateVectorResources>() else {
            return Vec::new();
        };
        let params = PopupValueParams {
            viewport_min: self.viewport_min,
            viewport_size: self.viewport_size,
            value_anchor: self.value_anchor,
            row_pitch: self.row_pitch,
            _pad_row: 0.0,
            char_size: self.char_size,
            _pad_char: [0.0; 2],
            text_color: self.text_color,
            hovered_display_index: self.hovered_display_index,
            qubits: self.qubits,
            _pad: [0; 2],
        };
        if resources.popup_value.last_params != Some(params) {
            queue.write_buffer(
                &resources.popup_value.params_buffer,
                0,
                bytemuck::bytes_of(&params),
            );
            resources.popup_value.last_params = Some(params);
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
        let active = resources.active_state;
        render_pass.set_pipeline(&resources.popup_value.pipeline);
        render_pass.set_bind_group(0, &resources.popup_value.bind_groups[active], &[]);
        // 6 verts × 3 instances (one quad per row, no vertex buffer —
        // verts come from `@builtin(vertex_index)`).
        render_pass.draw(0..6, 0..3);
    }
}

pub(crate) struct StateVectorCallback {
    /// Linearised simulation ops for the GPU pipeline. Includes all four
    /// op kinds: `ApplyGate`, `CaptureBloch`, `MeasureReduceSample`, and
    /// `MeasureCollapse`. The GPU dispatches them in order; ping-pong of
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

        if self.recompute || resources.state_count != self.state_count {
            resources.state_count = self.state_count;
            if self.state_count > 0 {
                // Initialize to |0…0⟩ then dispatch each op on the GPU.
                // The init itself happens on the GPU: `clear_buffer` zeros
                // the state range, then `copy_buffer_to_buffer` writes the
                // ground-state amplitude (1.0, 0.0) into slot 0. Both are
                // encoded into the recompute encoder below — no CPU
                // allocation, no `queue.write_buffer` upload (Issue C).
                resources.active_state = 0;
                let pair_count = (self.state_count / 2) as u32;
                let dispatch_x = pair_count.div_ceil(STATE_WORKGROUP_SIZE);

                if validate_simulation_plan_capacity(
                    &self.sim_ops,
                    SimulationPlanLimits {
                        max_ops_per_variant: MAX_OPS_PER_RECOMPUTE,
                        max_bloch_slots: MAX_BLOCH_SLOTS,
                        max_measurement_slots: MAX_MEASUREMENT_SLOTS,
                    },
                )
                .is_err()
                {
                    return Vec::new();
                }

                // ─── Issue A pre-pass ─────────────────────────────────────
                // Classify every op by variant and pack their params
                // contiguously into the per-variant staging buffers via a
                // single `queue.write_buffer` per variant. The dispatch loop
                // below will then source each op's params via
                // `encoder.copy_buffer_to_buffer` from these staging buffers
                // instead of re-uploading per gate, so all dispatches can
                // live in one encoder + one submit.
                let mut packed_gate_params: Vec<GateParams> =
                    Vec::with_capacity(self.sim_ops.len());
                let mut packed_bloch_params: Vec<BlochParams> =
                    Vec::with_capacity(self.sim_ops.len());
                let mut packed_measure_reduce_params: Vec<MeasureReduceParams> =
                    Vec::with_capacity(self.sim_ops.len());
                let mut packed_measure_collapse_params: Vec<MeasureCollapseParams> =
                    Vec::with_capacity(self.sim_ops.len());
                for op in &self.sim_ops {
                    match op {
                        SimulationOp::ApplyGate(params) => {
                            packed_gate_params.push(*params);
                        }
                        SimulationOp::CaptureBloch {
                            qubit_bit,
                            output_slot,
                            ..
                        } => {
                            packed_bloch_params.push(BlochParams {
                                qubit_bit: *qubit_bit,
                                state_count: self.state_count as u32,
                                output_slot: *output_slot,
                                _pad: 0,
                            });
                        }
                        SimulationOp::MeasureReduceSample {
                            gate_id,
                            qubit_bit,
                            output_slot,
                        } => {
                            packed_measure_reduce_params.push(MeasureReduceParams {
                                qubit_bit: *qubit_bit,
                                state_count: self.state_count as u32,
                                output_slot: *output_slot,
                                seed: *gate_id,
                            });
                        }
                        SimulationOp::MeasureCollapse {
                            qubit_bit,
                            aux_slot,
                        } => {
                            if pair_count == 0 {
                                continue;
                            }
                            packed_measure_collapse_params.push(MeasureCollapseParams {
                                qubit_bit: *qubit_bit,
                                state_count: self.state_count as u32,
                                aux_slot: *aux_slot,
                                _pad: 0,
                            });
                        }
                    }
                }
                if !packed_gate_params.is_empty() {
                    queue.write_buffer(
                        &resources.state.gate_params_staging_buffer,
                        0,
                        bytemuck::cast_slice(&packed_gate_params),
                    );
                }
                if !packed_bloch_params.is_empty() {
                    queue.write_buffer(
                        &resources.bloch.params_staging_buffer,
                        0,
                        bytemuck::cast_slice(&packed_bloch_params),
                    );
                }
                if !packed_measure_reduce_params.is_empty() {
                    queue.write_buffer(
                        &resources.measure.reduce_params_staging_buffer,
                        0,
                        bytemuck::cast_slice(&packed_measure_reduce_params),
                    );
                }
                if !packed_measure_collapse_params.is_empty() {
                    queue.write_buffer(
                        &resources.measure.collapse_params_staging_buffer,
                        0,
                        bytemuck::cast_slice(&packed_measure_collapse_params),
                    );
                }
                // ──────────────────────────────────────────────────────────

                let mut in_index = 0usize;
                let mut bloch_capture_count: u32 = 0;
                let mut bloch_slot_to_gate_id: Vec<u32> = Vec::with_capacity(MAX_BLOCH_SLOTS);
                let mut measurement_count: u32 = 0;
                let mut measurement_slot_to_gate_id: Vec<u32> =
                    Vec::with_capacity(MAX_MEASUREMENT_SLOTS);
                // Single encoder for the entire recompute. Each per-op param
                // update is encoded as `copy_buffer_to_buffer` from the
                // matching staging slot into the existing tiny uniform
                // buffer, immediately followed by the dispatch that reads it.
                // WebGPU guarantees in-order execution within one encoder,
                // and inserts the necessary memory barriers automatically,
                // so each dispatch sees its own params even though all
                // dispatches share `gate_params_buffer` etc. Issue A: this
                // replaces N per-op `queue.submit` round trips with one.
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("recompute_batched_encoder"),
                });

                // Issue C: GPU-only |0…0⟩ initialization. clear_buffer zeros
                // the active state range, then copy_buffer_to_buffer writes
                // the ground-state amplitude into slot 0. Both live in the
                // same encoder as the gate dispatches, so the auto-inserted
                // memory barriers make the first ApplyGate read this fresh
                // |0…0⟩ vector.
                let state_active_bytes =
                    (self.state_count * std::mem::size_of::<[f32; 2]>()) as wgpu::BufferAddress;
                encoder.clear_buffer(
                    &resources.common.state_buffers[0],
                    0,
                    Some(state_active_bytes),
                );
                encoder.copy_buffer_to_buffer(
                    &resources.common.state_seed_buffer,
                    0,
                    &resources.common.state_buffers[0],
                    0,
                    std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                );

                let gate_param_size = std::mem::size_of::<GateParams>() as wgpu::BufferAddress;
                let bloch_param_size = std::mem::size_of::<BlochParams>() as wgpu::BufferAddress;
                let measure_reduce_param_size =
                    std::mem::size_of::<MeasureReduceParams>() as wgpu::BufferAddress;
                let measure_collapse_param_size =
                    std::mem::size_of::<MeasureCollapseParams>() as wgpu::BufferAddress;
                let mut gate_slot: u64 = 0;
                let mut bloch_slot: u64 = 0;
                let mut measure_reduce_slot: u64 = 0;
                let mut measure_collapse_slot: u64 = 0;
                for op in &self.sim_ops {
                    match op {
                        SimulationOp::ApplyGate(_) => {
                            if pair_count == 0 {
                                continue;
                            }
                            encoder.copy_buffer_to_buffer(
                                &resources.state.gate_params_staging_buffer,
                                gate_slot * gate_param_size,
                                &resources.state.gate_params_buffer,
                                0,
                                gate_param_size,
                            );
                            {
                                let mut pass =
                                    encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                                        label: Some("state_vector_compute_pass"),
                                        timestamp_writes: None,
                                    });
                                pass.set_pipeline(&resources.state.compute_pipeline);
                                pass.set_bind_group(
                                    0,
                                    &resources.state.compute_bind_groups[in_index],
                                    &[],
                                );
                                pass.dispatch_workgroups(dispatch_x, 1, 1);
                            }
                            gate_slot += 1;
                            in_index = 1 - in_index;
                        }
                        SimulationOp::CaptureBloch { gate_id, .. } => {
                            encoder.copy_buffer_to_buffer(
                                &resources.bloch.params_staging_buffer,
                                bloch_slot * bloch_param_size,
                                &resources.bloch.params_buffer,
                                0,
                                bloch_param_size,
                            );
                            {
                                let mut pass =
                                    encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                                        label: Some("bloch_reduce_pass"),
                                        timestamp_writes: None,
                                    });
                                pass.set_pipeline(&resources.bloch.reduce_pipeline);
                                // The current state lives in `state_buffers[in_index]`,
                                // which is the read side of the next gate dispatch.
                                pass.set_bind_group(
                                    0,
                                    &resources.bloch.reduce_bind_groups[in_index],
                                    &[],
                                );
                                pass.dispatch_workgroups(1, 1, 1);
                            }
                            bloch_slot += 1;
                            bloch_slot_to_gate_id.push(*gate_id);
                            bloch_capture_count += 1;
                        }
                        SimulationOp::MeasureReduceSample { gate_id, .. } => {
                            encoder.copy_buffer_to_buffer(
                                &resources.measure.reduce_params_staging_buffer,
                                measure_reduce_slot * measure_reduce_param_size,
                                &resources.measure.reduce_params_buffer,
                                0,
                                measure_reduce_param_size,
                            );
                            {
                                let mut pass =
                                    encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                                        label: Some("measure_reduce_pass"),
                                        timestamp_writes: None,
                                    });
                                pass.set_pipeline(&resources.measure.reduce_pipeline);
                                pass.set_bind_group(
                                    0,
                                    &resources.measure.reduce_bind_groups[in_index],
                                    &[],
                                );
                                pass.dispatch_workgroups(1, 1, 1);
                            }
                            measure_reduce_slot += 1;
                            measurement_slot_to_gate_id.push(*gate_id);
                            measurement_count += 1;
                        }
                        SimulationOp::MeasureCollapse { .. } => {
                            if pair_count == 0 {
                                continue;
                            }
                            encoder.copy_buffer_to_buffer(
                                &resources.measure.collapse_params_staging_buffer,
                                measure_collapse_slot * measure_collapse_param_size,
                                &resources.measure.collapse_params_buffer,
                                0,
                                measure_collapse_param_size,
                            );
                            {
                                let mut pass =
                                    encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                                        label: Some("measure_collapse_pass"),
                                        timestamp_writes: None,
                                    });
                                pass.set_pipeline(&resources.measure.collapse_pipeline);
                                pass.set_bind_group(
                                    0,
                                    &resources.measure.collapse_bind_groups[in_index],
                                    &[],
                                );
                                pass.dispatch_workgroups(dispatch_x, 1, 1);
                            }
                            measure_collapse_slot += 1;
                            in_index = 1 - in_index;
                        }
                    }
                }
                queue.submit(Some(encoder.finish()));
                resources.active_state = in_index;

                // Production path never reads back. The slot mappings are
                // stashed in thread-locals so the test-only on-demand
                // readback APIs (`read_bloch_vectors_impl` /
                // `read_measurement_outcomes_impl`) can copy + map the
                // GPU buffers when JS asks for them.
                BLOCH_SLOT_MAP.with(|cell| {
                    *cell.borrow_mut() = bloch_slot_to_gate_id;
                });
                MEASUREMENT_SLOT_MAP.with(|cell| {
                    *cell.borrow_mut() = measurement_slot_to_gate_id;
                });
                let _ = bloch_capture_count;
                let _ = measurement_count;
            } else {
                resources.active_state = 0;
            }
        }

        GPU_READBACK.with(|slot| {
            *slot.borrow_mut() = Some(GpuReadbackState {
                device: device.clone(),
                queue: queue.clone(),
                state_buffers: [
                    resources.common.state_buffers[0].clone(),
                    resources.common.state_buffers[1].clone(),
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
