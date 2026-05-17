//! Test-only on-demand readback APIs.
//!
//! Production rendering never touches CPU readback — render shaders sample
//! storage buffers directly. These async functions exist solely so
//! the JS test harness (`read_state_vector` etc. in `lib.rs`) can verify
//! GPU outputs after a recompute. Each issues a fresh staging buffer +
//! `map_async` against the latest GPU handle that `StateVectorCallback::
//! prepare` parked in the thread-local slots below.

use std::cell::RefCell;

use eframe::wgpu;

use super::params::MAX_CHANCE_OUTCOMES;

#[cfg(target_arch = "wasm32")]
use futures_channel::oneshot;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;

#[derive(Clone)]
pub(crate) struct GpuReadbackState {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) state_buffers: [wgpu::Buffer; 3],
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

#[derive(Clone)]
pub(crate) struct ChanceGpuHandle {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) output_buffer: wgpu::Buffer,
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
    /// Same pattern for Chance displays. Test-only APIs may read this on
    /// demand; production rendering samples `chance_probability_output` in a
    /// fragment shader.
    pub(crate) static CHANCE_GPU_HANDLE: RefCell<Option<ChanceGpuHandle>> =
        const { RefCell::new(None) };
    pub(crate) static CHANCE_SLOT_MAP: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
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

#[cfg(target_arch = "wasm32")]
pub(crate) async fn read_chance_probabilities_impl() -> Result<js_sys::Float32Array, JsValue> {
    let Some(handle) = CHANCE_GPU_HANDLE.with(|slot| slot.borrow().clone()) else {
        return Ok(js_sys::Float32Array::new_with_length(0));
    };
    let slot_map = CHANCE_SLOT_MAP.with(|cell| cell.borrow().clone());
    if slot_map.is_empty() {
        return Ok(js_sys::Float32Array::new_with_length(0));
    }
    let values_per_slot = MAX_CHANCE_OUTCOMES;
    let copy_bytes = slot_map.len() * values_per_slot * std::mem::size_of::<f32>();
    let staging = handle.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("chance_readback"),
        size: copy_bytes as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = handle
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("chance_readback_encoder"),
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
    let output =
        js_sys::Float32Array::new_with_length((slot_map.len() * (values_per_slot + 1)) as u32);
    for (slot, gate_id) in slot_map.iter().enumerate() {
        let out_base = slot * (values_per_slot + 1);
        let in_base = slot * values_per_slot;
        output.set_index(out_base as u32, *gate_id as f32);
        for i in 0..values_per_slot {
            output.set_index((out_base + 1 + i) as u32, floats[in_base + i]);
        }
    }
    drop(data);
    staging.unmap();
    Ok(output)
}
