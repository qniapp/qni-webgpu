//! Resources shared across every GPU subsystem.
//!
//! * `state_buffers` — the two ping-pong state-vector buffers. Every
//!   pipeline (compute, render, bloch, measure, popup-value) reads or
//!   writes them, so they live here rather than inside any one
//!   subsystem.
//! * `state_preview_buffer` — state-panel snapshot currently selected by
//!   hover / breakpoint. It is populated by GPU copy from the step cache.
//! * `state_snapshot_cache_buffer` — GPU-resident per-column snapshots,
//!   qni-style. Hovering a column copies one cached slot into
//!   `state_preview_buffer` without rerunning the simulation.
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
    pub state_snapshot_cache_buffer: wgpu::Buffer,
    snapshot_cache_slots: usize,
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
        let snapshot_cache_slots = 1;
        let state_snapshot_cache_buffer =
            Self::create_snapshot_cache_buffer(device, snapshot_cache_slots, state_buffer_size);

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
            state_snapshot_cache_buffer,
            snapshot_cache_slots,
            state_seed_buffer,
            unit_quad_vertex_buffer,
            unit_quad_index_buffer,
            unit_quad_index_count: index_data.len() as u32,
        }
    }

    pub(crate) fn snapshot_cache_offset(&self, slot: usize) -> wgpu::BufferAddress {
        let state_buffer_size =
            (MAX_STATE_COUNT * std::mem::size_of::<[f32; 2]>()) as wgpu::BufferAddress;
        slot as wgpu::BufferAddress * state_buffer_size
    }

    pub(crate) fn snapshot_cache_slots(&self) -> usize {
        self.snapshot_cache_slots
    }

    pub(crate) fn ensure_snapshot_cache_slots(&mut self, device: &wgpu::Device, slots: usize) {
        let slots = slots.max(1);
        if slots <= self.snapshot_cache_slots {
            return;
        }
        let state_buffer_size =
            (MAX_STATE_COUNT * std::mem::size_of::<[f32; 2]>()) as wgpu::BufferAddress;
        self.state_snapshot_cache_buffer =
            Self::create_snapshot_cache_buffer(device, slots, state_buffer_size);
        self.snapshot_cache_slots = slots;
    }

    fn create_snapshot_cache_buffer(
        device: &wgpu::Device,
        slots: usize,
        state_buffer_size: wgpu::BufferAddress,
    ) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("state_vector_snapshot_cache_buffer"),
            size: state_buffer_size * slots.max(1) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }
}
