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

use super::params::{
    AMPLITUDE_VALUES_PER_SLOT, DENSITY_VALUES_PER_SLOT, MAX_AMPLITUDE_OUTCOMES,
    MAX_PROBABILITY_OUTCOMES,
};

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
pub(crate) struct ProbabilityGpuHandle {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) output_buffer: wgpu::Buffer,
}

#[derive(Clone)]
pub(crate) struct AmplitudeGpuHandle {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) output_buffer: wgpu::Buffer,
    pub(crate) meta_buffer: wgpu::Buffer,
}

#[derive(Clone)]
pub(crate) struct DensityGpuHandle {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) output_buffer: wgpu::Buffer,
    pub(crate) meta_buffer: wgpu::Buffer,
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
    /// Same pattern for Probability displays. Test-only APIs may read this on
    /// demand; production rendering samples `probability_output` in a
    /// fragment shader.
    pub(crate) static PROBABILITY_GPU_HANDLE: RefCell<Option<ProbabilityGpuHandle>> =
        const { RefCell::new(None) };
    pub(crate) static PROBABILITY_SLOT_MAP: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    /// Same pattern for Amplitude displays. Production shaders sample
    /// `amplitude_output` + `amplitude_meta`; tests can ask for one cell.
    pub(crate) static AMPLITUDE_GPU_HANDLE: RefCell<Option<AmplitudeGpuHandle>> =
        const { RefCell::new(None) };
    pub(crate) static AMPLITUDE_SLOT_MAP: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    /// Same pattern for Density Matrix displays. Production shaders sample
    /// `density_output` + `density_meta`; tests can ask for one cell.
    pub(crate) static DENSITY_GPU_HANDLE: RefCell<Option<DensityGpuHandle>> =
        const { RefCell::new(None) };
    pub(crate) static DENSITY_SLOT_MAP: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
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
pub(crate) async fn read_probability_distributions_impl() -> Result<js_sys::Float32Array, JsValue> {
    let Some(handle) = PROBABILITY_GPU_HANDLE.with(|slot| slot.borrow().clone()) else {
        return Ok(js_sys::Float32Array::new_with_length(0));
    };
    let slot_map = PROBABILITY_SLOT_MAP.with(|cell| cell.borrow().clone());
    if slot_map.is_empty() {
        return Ok(js_sys::Float32Array::new_with_length(0));
    }
    let values_per_slot = MAX_PROBABILITY_OUTCOMES;
    let copy_bytes = slot_map.len() * values_per_slot * std::mem::size_of::<f32>();
    let staging = handle.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("probability_readback"),
        size: copy_bytes as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = handle
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("probability_readback_encoder"),
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

#[cfg(target_arch = "wasm32")]
pub(crate) async fn read_amplitude_cell_impl(
    gate_id: u32,
    outcome: u32,
) -> Result<js_sys::Float64Array, JsValue> {
    let Some(handle) = AMPLITUDE_GPU_HANDLE.with(|slot| slot.borrow().clone()) else {
        return Ok(js_sys::Float64Array::new_with_length(0));
    };
    let slot_map = AMPLITUDE_SLOT_MAP.with(|cell| cell.borrow().clone());
    let Some(slot) = slot_map.iter().position(|id| *id == gate_id) else {
        return Ok(js_sys::Float64Array::new_with_length(0));
    };
    if outcome as usize >= MAX_AMPLITUDE_OUTCOMES {
        return Ok(js_sys::Float64Array::new_with_length(0));
    }

    let data_base = slot * AMPLITUDE_VALUES_PER_SLOT;
    let reim_offset = (data_base + outcome as usize * 2) * std::mem::size_of::<f32>();
    let incoherent_offset =
        (data_base + 2 * MAX_AMPLITUDE_OUTCOMES + outcome as usize) * std::mem::size_of::<f32>();
    let meta_offset = slot * std::mem::size_of::<[f32; 4]>();
    let staging = handle.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("amplitude_cell_readback"),
        size: 32,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = handle
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("amplitude_cell_readback_encoder"),
        });
    encoder.copy_buffer_to_buffer(
        &handle.output_buffer,
        reim_offset as wgpu::BufferAddress,
        &staging,
        0,
        8,
    );
    encoder.copy_buffer_to_buffer(
        &handle.output_buffer,
        incoherent_offset as wgpu::BufferAddress,
        &staging,
        8,
        4,
    );
    encoder.copy_buffer_to_buffer(
        &handle.meta_buffer,
        meta_offset as wgpu::BufferAddress,
        &staging,
        16,
        16,
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
    let phase = if floats[5] < 0.0 {
        u32::MAX as f64
    } else {
        f64::from(floats[5])
    };
    let output = js_sys::Float64Array::new_with_length(8);
    output.set_index(0, f64::from(gate_id));
    output.set_index(1, f64::from(outcome));
    output.set_index(2, f64::from(floats[0]));
    output.set_index(3, f64::from(floats[1]));
    output.set_index(4, f64::from(floats[2]));
    output.set_index(5, f64::from(floats[4]));
    output.set_index(6, phase);
    output.set_index(7, f64::from(floats[6]));
    drop(data);
    staging.unmap();
    Ok(output)
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn read_density_matrix_cell_impl(
    gate_id: u32,
    row: u32,
    col: u32,
) -> Result<js_sys::Float64Array, JsValue> {
    let Some(handle) = DENSITY_GPU_HANDLE.with(|slot| slot.borrow().clone()) else {
        return Ok(js_sys::Float64Array::new_with_length(0));
    };
    let slot_map = DENSITY_SLOT_MAP.with(|cell| cell.borrow().clone());
    let Some(slot) = slot_map.iter().position(|id| *id == gate_id) else {
        return Ok(js_sys::Float64Array::new_with_length(0));
    };

    let meta_offset = slot * std::mem::size_of::<[f32; 4]>();
    let staging = handle.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("density_cell_readback"),
        size: 24,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = handle
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("density_cell_readback_encoder"),
        });
    encoder.copy_buffer_to_buffer(
        &handle.meta_buffer,
        meta_offset as wgpu::BufferAddress,
        &staging,
        8,
        16,
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
    let meta_floats: &[f32] = bytemuck::cast_slice(&data[8..24]);
    let span = meta_floats[1].max(0.0) as u32;
    let dim = if span < usize::BITS {
        1usize << span
    } else {
        0
    };
    let unity = meta_floats[0].max(1.0e-12);
    drop(data);
    staging.unmap();

    if dim == 0 || row as usize >= dim || col as usize >= dim {
        return Ok(js_sys::Float64Array::new_with_length(0));
    }
    let cell = row as usize * dim + col as usize;
    if cell >= DENSITY_VALUES_PER_SLOT {
        return Ok(js_sys::Float64Array::new_with_length(0));
    }
    let value_offset = (slot * DENSITY_VALUES_PER_SLOT + cell) * std::mem::size_of::<[f32; 2]>();
    let value_staging = handle.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("density_value_readback"),
        size: 8,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut value_encoder = handle
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("density_value_readback_encoder"),
        });
    value_encoder.copy_buffer_to_buffer(
        &handle.output_buffer,
        value_offset as wgpu::BufferAddress,
        &value_staging,
        0,
        8,
    );
    handle.queue.submit(Some(value_encoder.finish()));

    let value_slice = value_staging.slice(..);
    let (value_sender, value_receiver) = oneshot::channel();
    value_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = value_sender.send(result);
    });
    value_receiver
        .await
        .map_err(|_| JsValue::from_str("readback dropped"))?
        .map_err(|err| JsValue::from_str(&format!("map_async failed: {err:?}")))?;
    let value_data = value_slice.get_mapped_range();
    let value_floats: &[f32] = bytemuck::cast_slice(&value_data);
    let output = js_sys::Float64Array::new_with_length(7);
    output.set_index(0, f64::from(gate_id));
    output.set_index(1, f64::from(row));
    output.set_index(2, f64::from(col));
    output.set_index(3, f64::from(value_floats[0] / unity));
    output.set_index(4, f64::from(value_floats[1] / unity));
    output.set_index(5, f64::from(unity));
    output.set_index(6, f64::from(span));
    drop(value_data);
    value_staging.unmap();
    Ok(output)
}
