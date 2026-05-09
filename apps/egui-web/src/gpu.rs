use ab_glyph::{Font as _, ScaleFont as _};
use eframe::egui;
use eframe::{egui_wgpu, wgpu};
use std::cell::RefCell;
use std::sync::Arc;
use wgpu::util::DeviceExt as _;

#[cfg(target_arch = "wasm32")]
use futures_channel::oneshot;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;

use crate::bloch::SimulationOp;
use crate::colors::Colors;
use crate::constants::MAX_STATE_COUNT;
use crate::gates::GateParams;

mod shaders;
use shaders::{
    BLOCH_OVERLAY_SHADER, BLOCH_REDUCE_SHADER, MEASUREMENT_DIGIT_SHADER, MEASURE_COLLAPSE_SHADER,
    MEASURE_REDUCE_SHADER, STATE_COMPUTE_SHADER, STATE_RENDER_SHADER,
};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct StateInstance {
    pub(crate) center: [f32; 2],
    pub(crate) radius: f32,
    pub(crate) inner_radius: f32,
    pub(crate) stroke: f32,
    pub(crate) state_index: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct RenderParams {
    /// See `BlochOverlayParams::viewport_min`. NDC -1..1 maps to the egui
    /// callback viewport, not the full canvas.
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) surface: [f32; 4],
    pub(crate) fill: [f32; 4],
    pub(crate) outline: [f32; 4],
    pub(crate) outline_zero: [f32; 4],
    pub(crate) needle: [f32; 4],
}

#[derive(Clone, Copy)]
pub(crate) struct RenderColors {
    pub(crate) surface: [f32; 4],
    pub(crate) fill: [f32; 4],
    pub(crate) outline: [f32; 4],
    pub(crate) outline_zero: [f32; 4],
    pub(crate) needle: [f32; 4],
}

impl RenderColors {
    pub(crate) fn new(colors: &Colors) -> Self {
        Self {
            surface: egui::Rgba::from(colors.surface).to_array(),
            fill: egui::Rgba::from(colors.state_fill).to_array(),
            outline: egui::Rgba::from(colors.state_outline).to_array(),
            outline_zero: egui::Rgba::from(colors.state_outline_zero).to_array(),
            needle: egui::Rgba::from(colors.state_needle).to_array(),
        }
    }
}

pub(crate) const STATE_WORKGROUP_SIZE: u32 = 64;

/// Upper bound on the number of `SimulationOp`s of any single variant we can
/// batch into one recompute encoder. Issue A's fix packs all per-op params
/// into staging buffers up-front, then issues `copy_buffer_to_buffer` from
/// staging slots into the existing small uniform buffers between dispatches
/// inside a single encoder. Each variant has its own staging buffer sized to
/// `MAX_OPS_PER_RECOMPUTE` slots; if a circuit ever exceeds this, the
/// `debug_assert!` in the prepare pass will trip.
pub(crate) const MAX_OPS_PER_RECOMPUTE: usize = 256;


/// Maximum number of Bloch displays whose vectors can be captured in a single
/// recompute. Each placed `BlochDisplay` occupies one slot in the GPU's
/// `bloch_output_buffer` (a vec4 per slot, .xyz used).
pub(crate) const MAX_BLOCH_SLOTS: usize = 64;

// Workgroup size for the Bloch reduction shader is hard-coded to 64 (matches
// `@workgroup_size(64)` in `BLOCH_REDUCE_SHADER`). One workgroup processes the
// entire state vector for a single qubit and reduces (ρ_00, ρ_11, Re(ρ_01),
// Im(ρ_01)) via shared memory.


/// Maximum number of measurement gates whose outcomes can be captured in a
/// single recompute. Each placed `Measurement` occupies one slot in the GPU's
/// `measurement_aux_buffer` (a vec4 per slot — pZero, r, outcome, √p_kept).
pub(crate) const MAX_MEASUREMENT_SLOTS: usize = 64;

// MEASURE_REDUCE_SAMPLE — workgroup reduces pZero across the state vector,
// samples a deterministic [0, 1) value with a PCG-style hash seeded by the
// placed gate's id, and writes `(pZero, r, outcome, sqrt_p_kept)` into
// `aux_out[output_slot]`.  qni reference: `simulator.ts:measure`.

// MEASURE_COLLAPSE — per-pair shader that reads the previously-sampled
// outcome + sqrt_p_kept from the aux buffer, zeroes the unobserved branch,
// and renormalizes the surviving amplitudes.

// BLOCH_OVERLAY_RENDER_SHADER renders the dynamic arrow + tip dot of every
// placed Bloch display directly from `bloch_output_buffer`. Static decoration
// (sphere bg, axis lines, equator/meridian ellipses) is still painted by
// egui — it doesn't depend on quantum state — but the part that does depend
// stays on the GPU end-to-end (no CPU readback). Projection mirrors
// `icons::bloch_project` (qni's `rotateY(phi) rotateX(-theta)` axis swap +
// `perspective: 4rem` pinhole at top-right).

// MEASUREMENT_DIGIT_SHADER renders the `0` / `1` digit of every placed
// measurement directly from `measurement_aux_buffer`. The aux layout is
// `(pZero, r, outcome, sqrt_p_kept)` per slot; we sample `.z` to pick the
// glyph and the colour. The digit pixels come from a single-channel atlas
// of two cells (`0` on top, `1` on bottom) baked at startup from Hack
// Regular at the same 16-px size egui uses for the |0> / |1> labels —
// keeps the measurement digit visually identical to the write gate digit
// without forcing a CPU readback of the outcome.


#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BlochParams {
    pub(crate) qubit_bit: u32,
    pub(crate) state_count: u32,
    pub(crate) output_slot: u32,
    pub(crate) _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MeasureReduceParams {
    pub(crate) qubit_bit: u32,
    pub(crate) state_count: u32,
    pub(crate) output_slot: u32,
    pub(crate) seed: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MeasureCollapseParams {
    pub(crate) qubit_bit: u32,
    pub(crate) state_count: u32,
    pub(crate) aux_slot: u32,
    pub(crate) _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BlochOverlayParams {
    /// Egui callback viewport in CSS pixels (= the rect we passed to
    /// `Callback::new_paint_callback`). NDC -1..1 maps to this viewport,
    /// NOT to the full canvas — using `screen_descriptor.size_in_pixels`
    /// here would shift everything by `viewport_min`.
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) line_color: [f32; 4],
    pub(crate) tip_color: [f32; 4],
    pub(crate) zero_color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BlochOverlayInstance {
    pub(crate) center: [f32; 2],
    pub(crate) radius: f32,
    pub(crate) outer: f32,
    pub(crate) slot: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MeasurementDigitParams {
    /// See `BlochOverlayParams::viewport_min` — same NDC story.
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) zero_color: [f32; 4],
    pub(crate) one_color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MeasurementDigitInstance {
    pub(crate) center: [f32; 2],
    pub(crate) half_extent: f32,
    pub(crate) slot: u32,
}

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
    /// collapsing N per-gate `queue.submit` round trips into a single submit
    /// (Issue A — see docs/perf-issue-a-fix-plan.html).
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
}

/// Atlas geometry for the measurement digit texture: a 1x2 grid of cells,
/// each holding a single rasterised glyph. Cell size matches the
/// `MeasurementDigitInstance::half_extent * 2` quad the shader draws into,
/// so UVs map identity-style.
const DIGIT_ATLAS_CELL: u32 = 22;
const DIGIT_ATLAS_WIDTH: u32 = DIGIT_ATLAS_CELL;
const DIGIT_ATLAS_HEIGHT: u32 = DIGIT_ATLAS_CELL * 2;

/// Rasterises the digits "0" and "1" using Hack Regular (the same font
/// egui's monospace family resolves to) so the measurement digits look
/// identical to the |0> / |1> labels egui paints. Done once at startup;
/// the result is uploaded to a GPU texture sampled by
/// `MEASUREMENT_DIGIT_SHADER`. The PxScale is calibrated so the rasterised
/// "0" matches the on-screen size of `FontId::monospace(16.0)`'s glyph
/// (egui internally upscales monospace ~1.2x past the raw em-size we'd
/// get from a plain ab_glyph rasterisation at PxScale(16)).
fn rasterize_digit_atlas() -> Vec<u8> {
    let font = ab_glyph::FontRef::try_from_slice(epaint_default_fonts::HACK_REGULAR)
        .expect("Hack Regular bytes should parse as a TTF");
    let scale = ab_glyph::PxScale::from(20.0);
    let scaled = font.as_scaled(scale);

    let mut atlas = vec![0u8; (DIGIT_ATLAS_WIDTH * DIGIT_ATLAS_HEIGHT) as usize];
    for (cell_index, ch) in ['0', '1'].iter().enumerate() {
        let glyph_id = font.glyph_id(*ch);
        let glyph =
            glyph_id.with_scale_and_position(scale, ab_glyph::Point { x: 0.0, y: scaled.ascent() });
        let Some(outlined) = font.outline_glyph(glyph) else {
            continue;
        };
        let bounds = outlined.px_bounds();
        let glyph_w = bounds.width().ceil() as u32;
        let glyph_h = bounds.height().ceil() as u32;
        let cell_origin_x = DIGIT_ATLAS_CELL.saturating_sub(glyph_w) / 2;
        let cell_origin_y =
            cell_index as u32 * DIGIT_ATLAS_CELL + DIGIT_ATLAS_CELL.saturating_sub(glyph_h) / 2;
        outlined.draw(|gx, gy, alpha| {
            let px = cell_origin_x + gx;
            let py = cell_origin_y + gy;
            if px < DIGIT_ATLAS_WIDTH && py < DIGIT_ATLAS_HEIGHT {
                atlas[(py * DIGIT_ATLAS_WIDTH + px) as usize] =
                    (alpha.clamp(0.0, 1.0) * 255.0) as u8;
            }
        });
    }
    atlas
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
        queue.write_buffer(
            &resources.bloch_overlay_params_buffer,
            0,
            bytemuck::bytes_of(&params),
        );
        queue.write_buffer(
            &resources.bloch_overlay_instance_buffer,
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
        render_pass.set_pipeline(&resources.bloch_overlay_pipeline);
        render_pass.set_bind_group(0, &resources.bloch_overlay_bind_group, &[]);
        render_pass.set_vertex_buffer(0, resources.bloch_overlay_vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, resources.bloch_overlay_instance_buffer.slice(..));
        render_pass.set_index_buffer(
            resources.bloch_overlay_index_buffer.slice(..),
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
        queue.write_buffer(
            &resources.measurement_digit_params_buffer,
            0,
            bytemuck::bytes_of(&params),
        );
        queue.write_buffer(
            &resources.measurement_digit_instance_buffer,
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
        render_pass.set_pipeline(&resources.measurement_digit_pipeline);
        render_pass.set_bind_group(0, &resources.measurement_digit_bind_group, &[]);
        // Reuse the bloch overlay's quad geometry — both render full-rect
        // quads with `[-1..1]` corners.
        render_pass.set_vertex_buffer(0, resources.bloch_overlay_vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, resources.measurement_digit_instance_buffer.slice(..));
        render_pass.set_index_buffer(
            resources.bloch_overlay_index_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        render_pass.draw_indexed(0..6, 0, 0..self.instances.len() as u32);
    }
}

pub(crate) struct StateVectorCallback {
    pub(crate) instances: Arc<[StateInstance]>,
    pub(crate) instances_dirty: bool,
    /// Linearised simulation ops for the GPU pipeline. Includes all four
    /// op kinds: `ApplyGate`, `CaptureBloch`, `MeasureReduceSample`, and
    /// `MeasureCollapse`. The GPU dispatches them in order; ping-pong of
    /// the state buffers happens for any op that mutates state (gates and
    /// `MeasureCollapse`).
    pub(crate) sim_ops: Vec<SimulationOp>,
    pub(crate) state_count: usize,
    pub(crate) recompute: bool,
    pub(crate) target_format: wgpu::TextureFormat,
    pub(crate) colors: RenderColors,
    /// See `BlochOverlayCallback::viewport_min`.
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
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
            callback_resources.insert(StateVectorResources::new(
                device,
                queue,
                self.target_format,
            ));
            callback_resources
                .get_mut::<StateVectorResources>()
                .expect("StateVectorResources just inserted")
        };

        resources.update_render_pipeline(device, self.target_format);

        let render_params = RenderParams {
            viewport_min: self.viewport_min,
            viewport_size: self.viewport_size,
            surface: self.colors.surface,
            fill: self.colors.fill,
            outline: self.colors.outline,
            outline_zero: self.colors.outline_zero,
            needle: self.colors.needle,
        };
        queue.write_buffer(
            &resources.render_params_buffer,
            0,
            bytemuck::bytes_of(&render_params),
        );

        let should_update_instances = self.instances_dirty || resources.state_count == 0;
        if should_update_instances && !self.instances.is_empty() {
            queue.write_buffer(
                &resources.instance_buffer,
                0,
                bytemuck::cast_slice(self.instances.as_ref()),
            );
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
                            if (*output_slot as usize) >= MAX_BLOCH_SLOTS {
                                continue;
                            }
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
                            if (*output_slot as usize) >= MAX_MEASUREMENT_SLOTS {
                                continue;
                            }
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
                debug_assert!(
                    packed_gate_params.len() <= MAX_OPS_PER_RECOMPUTE
                        && packed_bloch_params.len() <= MAX_OPS_PER_RECOMPUTE
                        && packed_measure_reduce_params.len() <= MAX_OPS_PER_RECOMPUTE
                        && packed_measure_collapse_params.len() <= MAX_OPS_PER_RECOMPUTE,
                    "sim_ops exceeds MAX_OPS_PER_RECOMPUTE; bump the constant in gpu.rs"
                );
                if !packed_gate_params.is_empty() {
                    queue.write_buffer(
                        &resources.gate_params_staging_buffer,
                        0,
                        bytemuck::cast_slice(&packed_gate_params),
                    );
                }
                if !packed_bloch_params.is_empty() {
                    queue.write_buffer(
                        &resources.bloch_params_staging_buffer,
                        0,
                        bytemuck::cast_slice(&packed_bloch_params),
                    );
                }
                if !packed_measure_reduce_params.is_empty() {
                    queue.write_buffer(
                        &resources.measure_reduce_params_staging_buffer,
                        0,
                        bytemuck::cast_slice(&packed_measure_reduce_params),
                    );
                }
                if !packed_measure_collapse_params.is_empty() {
                    queue.write_buffer(
                        &resources.measure_collapse_params_staging_buffer,
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
                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("recompute_batched_encoder"),
                    });

                // Issue C: GPU-only |0…0⟩ initialization. clear_buffer zeros
                // the active state range, then copy_buffer_to_buffer writes
                // the ground-state amplitude into slot 0. Both live in the
                // same encoder as the gate dispatches, so the auto-inserted
                // memory barriers make the first ApplyGate read this fresh
                // |0…0⟩ vector.
                let state_active_bytes = (self.state_count
                    * std::mem::size_of::<[f32; 2]>())
                    as wgpu::BufferAddress;
                encoder.clear_buffer(
                    &resources.state_buffers[0],
                    0,
                    Some(state_active_bytes),
                );
                encoder.copy_buffer_to_buffer(
                    &resources.state_seed_buffer,
                    0,
                    &resources.state_buffers[0],
                    0,
                    std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                );

                let gate_param_size =
                    std::mem::size_of::<GateParams>() as wgpu::BufferAddress;
                let bloch_param_size =
                    std::mem::size_of::<BlochParams>() as wgpu::BufferAddress;
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
                                &resources.gate_params_staging_buffer,
                                gate_slot * gate_param_size,
                                &resources.gate_params_buffer,
                                0,
                                gate_param_size,
                            );
                            {
                                let mut pass =
                                    encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                                        label: Some("state_vector_compute_pass"),
                                        timestamp_writes: None,
                                    });
                                pass.set_pipeline(&resources.compute_pipeline);
                                pass.set_bind_group(
                                    0,
                                    &resources.compute_bind_groups[in_index],
                                    &[],
                                );
                                pass.dispatch_workgroups(dispatch_x, 1, 1);
                            }
                            gate_slot += 1;
                            in_index = 1 - in_index;
                        }
                        SimulationOp::CaptureBloch {
                            gate_id,
                            output_slot,
                            ..
                        } => {
                            if (*output_slot as usize) >= MAX_BLOCH_SLOTS {
                                continue;
                            }
                            encoder.copy_buffer_to_buffer(
                                &resources.bloch_params_staging_buffer,
                                bloch_slot * bloch_param_size,
                                &resources.bloch_params_buffer,
                                0,
                                bloch_param_size,
                            );
                            {
                                let mut pass =
                                    encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                                        label: Some("bloch_reduce_pass"),
                                        timestamp_writes: None,
                                    });
                                pass.set_pipeline(&resources.bloch_pipeline);
                                // The current state lives in `state_buffers[in_index]`,
                                // which is the read side of the next gate dispatch.
                                pass.set_bind_group(
                                    0,
                                    &resources.bloch_bind_groups[in_index],
                                    &[],
                                );
                                pass.dispatch_workgroups(1, 1, 1);
                            }
                            bloch_slot += 1;
                            bloch_slot_to_gate_id.push(*gate_id);
                            bloch_capture_count += 1;
                        }
                        SimulationOp::MeasureReduceSample {
                            gate_id,
                            output_slot,
                            ..
                        } => {
                            if (*output_slot as usize) >= MAX_MEASUREMENT_SLOTS {
                                continue;
                            }
                            encoder.copy_buffer_to_buffer(
                                &resources.measure_reduce_params_staging_buffer,
                                measure_reduce_slot * measure_reduce_param_size,
                                &resources.measure_reduce_params_buffer,
                                0,
                                measure_reduce_param_size,
                            );
                            {
                                let mut pass =
                                    encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                                        label: Some("measure_reduce_pass"),
                                        timestamp_writes: None,
                                    });
                                pass.set_pipeline(&resources.measure_reduce_pipeline);
                                pass.set_bind_group(
                                    0,
                                    &resources.measure_reduce_bind_groups[in_index],
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
                                &resources.measure_collapse_params_staging_buffer,
                                measure_collapse_slot * measure_collapse_param_size,
                                &resources.measure_collapse_params_buffer,
                                0,
                                measure_collapse_param_size,
                            );
                            {
                                let mut pass =
                                    encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                                        label: Some("measure_collapse_pass"),
                                        timestamp_writes: None,
                                    });
                                pass.set_pipeline(&resources.measure_collapse_pipeline);
                                pass.set_bind_group(
                                    0,
                                    &resources.measure_collapse_bind_groups[in_index],
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
                    resources.state_buffers[0].clone(),
                    resources.state_buffers[1].clone(),
                ],
                state_count: resources.state_count,
                active_state: resources.active_state,
            });
        });
        BLOCH_GPU_HANDLE.with(|slot| {
            *slot.borrow_mut() = Some(BlochGpuHandle {
                device: device.clone(),
                queue: queue.clone(),
                output_buffer: resources.bloch_output_buffer.clone(),
            });
        });
        MEASUREMENT_GPU_HANDLE.with(|slot| {
            *slot.borrow_mut() = Some(MeasurementGpuHandle {
                device: device.clone(),
                queue: queue.clone(),
                aux_buffer: resources.measurement_aux_buffer.clone(),
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
        if self.instances.is_empty() {
            return;
        }
        render_pass.set_pipeline(&resources.render_pipeline);
        render_pass.set_bind_group(
            0,
            &resources.render_bind_groups[resources.active_state],
            &[],
        );
        render_pass.set_vertex_buffer(0, resources.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, resources.instance_buffer.slice(..));
        render_pass.set_index_buffer(resources.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..resources.index_count, 0, 0..self.instances.len() as u32);
    }
}

#[derive(Clone)]
pub(crate) struct GpuReadbackState {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) state_buffers: [wgpu::Buffer; 2],
    pub(crate) state_count: usize,
    pub(crate) active_state: usize,
}

#[derive(Clone)]
pub(crate) struct BlochGpuHandle {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) output_buffer: wgpu::Buffer,
}

#[derive(Clone)]
pub(crate) struct MeasurementGpuHandle {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) aux_buffer: wgpu::Buffer,
}

thread_local! {
    pub(crate) static GPU_READBACK: RefCell<Option<GpuReadbackState>> = const { RefCell::new(None) };
    /// Latest GPU buffer + queue handle for the bloch overlay output. Set in
    /// `prepare()`; consumed by the test-only async API
    /// `read_bloch_vectors_impl`. No production code touches it — production
    /// rendering reads `bloch_output_buffer` directly inside the GPU shader.
    pub(crate) static BLOCH_GPU_HANDLE: RefCell<Option<BlochGpuHandle>> =
        const { RefCell::new(None) };
    /// gate_id list ordered by output_slot. Parallel to the contents of
    /// `bloch_output_buffer`; the test API joins this with the read-back
    /// floats to produce `[gate_id, x, y, z, …]`.
    pub(crate) static BLOCH_SLOT_MAP: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    /// Same as `BLOCH_GPU_HANDLE` for the measurement aux buffer.
    pub(crate) static MEASUREMENT_GPU_HANDLE: RefCell<Option<MeasurementGpuHandle>> =
        const { RefCell::new(None) };
    pub(crate) static MEASUREMENT_SLOT_MAP: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn read_state_vector_impl() -> Result<js_sys::Float32Array, JsValue> {
    let Some(state) = GPU_READBACK.with(|slot| slot.borrow().clone()) else {
        return Err(JsValue::from_str("state vector not ready"));
    };
    let byte_len = state.state_count * 2 * std::mem::size_of::<f32>();
    let staging = state.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("state_vector_readback"),
        size: byte_len as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = state
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("state_vector_readback_encoder"),
        });
    encoder.copy_buffer_to_buffer(
        &state.state_buffers[state.active_state],
        0,
        &staging,
        0,
        byte_len as wgpu::BufferAddress,
    );
    state.queue.submit(Some(encoder.finish()));

    let slice = staging.slice(..);
    let (sender, receiver) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    receiver
        .await
        .map_err(|_| JsValue::from_str("readback dropped"))?
        .map_err(|err| JsValue::from_str(&format!("map_async failed: {err:?}")))?;
    let data = slice.get_mapped_range();
    let floats: &[f32] = bytemuck::cast_slice(&data);
    let output = js_sys::Float32Array::new_with_length(floats.len() as u32);
    output.copy_from(floats);
    drop(data);
    staging.unmap();
    Ok(output)
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn read_bloch_vectors_impl() -> Result<js_sys::Float32Array, JsValue> {
    let Some(handle) = BLOCH_GPU_HANDLE.with(|slot| slot.borrow().clone()) else {
        return Ok(js_sys::Float32Array::new_with_length(0));
    };
    let slot_map = BLOCH_SLOT_MAP.with(|cell| cell.borrow().clone());
    if slot_map.is_empty() {
        return Ok(js_sys::Float32Array::new_with_length(0));
    }
    let copy_bytes = slot_map.len() * 4 * std::mem::size_of::<f32>();
    let staging = handle.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bloch_readback"),
        size: copy_bytes as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = handle
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("bloch_readback_encoder"),
        });
    encoder.copy_buffer_to_buffer(
        &handle.output_buffer,
        0,
        &staging,
        0,
        copy_bytes as wgpu::BufferAddress,
    );
    handle.queue.submit(Some(encoder.finish()));

    let slice = staging.slice(..);
    let (sender, receiver) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    receiver
        .await
        .map_err(|_| JsValue::from_str("readback dropped"))?
        .map_err(|err| JsValue::from_str(&format!("map_async failed: {err:?}")))?;
    let data = slice.get_mapped_range();
    let floats: &[f32] = bytemuck::cast_slice(&data);
    let output = js_sys::Float32Array::new_with_length((slot_map.len() * 4) as u32);
    for (slot, gate_id) in slot_map.iter().enumerate() {
        let base = slot * 4;
        if base + 2 >= floats.len() {
            break;
        }
        output.set_index((slot * 4) as u32, *gate_id as f32);
        output.set_index((slot * 4 + 1) as u32, floats[base]);
        output.set_index((slot * 4 + 2) as u32, floats[base + 1]);
        output.set_index((slot * 4 + 3) as u32, floats[base + 2]);
    }
    drop(data);
    staging.unmap();
    Ok(output)
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn read_measurement_outcomes_impl() -> Result<js_sys::Float32Array, JsValue> {
    let Some(handle) = MEASUREMENT_GPU_HANDLE.with(|slot| slot.borrow().clone()) else {
        return Ok(js_sys::Float32Array::new_with_length(0));
    };
    let slot_map = MEASUREMENT_SLOT_MAP.with(|cell| cell.borrow().clone());
    if slot_map.is_empty() {
        return Ok(js_sys::Float32Array::new_with_length(0));
    }
    let copy_bytes = slot_map.len() * 4 * std::mem::size_of::<f32>();
    let staging = handle.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("measurement_readback"),
        size: copy_bytes as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = handle
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("measurement_readback_encoder"),
        });
    encoder.copy_buffer_to_buffer(
        &handle.aux_buffer,
        0,
        &staging,
        0,
        copy_bytes as wgpu::BufferAddress,
    );
    handle.queue.submit(Some(encoder.finish()));

    let slice = staging.slice(..);
    let (sender, receiver) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    receiver
        .await
        .map_err(|_| JsValue::from_str("readback dropped"))?
        .map_err(|err| JsValue::from_str(&format!("map_async failed: {err:?}")))?;
    let data = slice.get_mapped_range();
    let floats: &[f32] = bytemuck::cast_slice(&data);
    let output = js_sys::Float32Array::new_with_length((slot_map.len() * 2) as u32);
    for (slot, gate_id) in slot_map.iter().enumerate() {
        // aux layout (.x, .y, .z, .w) = (pZero, r, outcome, sqrt_p_kept).
        let outcome_idx = slot * 4 + 2;
        if outcome_idx >= floats.len() {
            break;
        }
        output.set_index((slot * 2) as u32, *gate_id as f32);
        output.set_index((slot * 2 + 1) as u32, floats[outcome_idx]);
    }
    drop(data);
    staging.unmap();
    Ok(output)
}
