//! Resources shared across every GPU subsystem.
//!
//! * `state_buffers` — the two ping-pong state-vector buffers. Every
//!   pipeline (compute, render, bloch, measure, popup-value) reads or
//!   writes them, so they live here rather than inside any one
//!   subsystem.
//! * `state_preview_buffer` — optional state-panel snapshot used while a
//!   circuit column is selected. The simulation still runs to the end so
//!   readout gates after the selected column stay populated.
//! * `state_seed_buffer` — 8-byte (1.0, 0.0) read-only seed copied
//!   into `state_buffers[0]` at the start of every recompute to
//!   initialise the state vector to `|0…0⟩` entirely on the GPU.
//! * `unit_quad_*` — the `[-1, 1]²` quad geometry used by the state
//!   render, the bloch overlay, and the measurement digit overlay.
//!   Identical buffers were duplicated three times in the original
//!   monolithic file; this module owns one shared pair.
//!
//! Each pipeline submodule borrows what it needs through `&Common`
//! during its `build()` call.

use eframe::wgpu;
use wgpu::util::DeviceExt;

use crate::constants::MAX_STATE_COUNT;

pub(crate) struct Common {
    pub state_buffers: [wgpu::Buffer; 2],
    pub state_preview_buffer: wgpu::Buffer,
    pub state_seed_buffer: wgpu::Buffer,
    pub unit_quad_vertex_buffer: wgpu::Buffer,
    pub unit_quad_index_buffer: wgpu::Buffer,
    pub unit_quad_index_count: u32,
}

impl Common {
    pub(super) fn build(device: &wgpu::Device) -> Self {
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
        let state_preview_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("state_vector_preview_buffer"),
            size: state_buffer_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let state_seed_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("state_vector_ground_seed"),
            contents: bytemuck::cast_slice(&[1.0f32, 0.0f32]),
            usage: wgpu::BufferUsages::COPY_SRC,
        });

        let unit_quad_vertex_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("unit_quad_vertices"),
                contents: bytemuck::cast_slice(&[
                    [-1.0f32, -1.0],
                    [1.0, -1.0],
                    [1.0, 1.0],
                    [-1.0, 1.0],
                ]),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_data: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let unit_quad_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("unit_quad_indices"),
            contents: bytemuck::cast_slice(&index_data),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            state_buffers,
            state_preview_buffer,
            state_seed_buffer,
            unit_quad_vertex_buffer,
            unit_quad_index_buffer,
            unit_quad_index_count: index_data.len() as u32,
        }
    }
}
