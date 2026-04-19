use eframe::egui;

#[cfg(target_arch = "wasm32")]
pub(crate) fn now_seconds() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map(|performance| performance.now() / 1000.0)
        .unwrap_or(0.0)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn now_seconds() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;

    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

pub(crate) fn display_index_to_state_index(mut display_index: usize, qubits: usize) -> usize {
    let mut value = 0usize;
    for _ in 0..qubits {
        value = (value << 1) | (display_index & 1);
        display_index >>= 1;
    }
    value
}

pub(crate) fn amplitude_qubits(len: usize) -> usize {
    let mut qubits = 0;
    let mut size = 1usize;
    if len == 0 {
        return 1;
    }
    while size < len {
        size <<= 1;
        qubits += 1;
    }
    qubits.max(1)
}

pub(crate) fn color_rgba(r: f32, g: f32, b: f32, a: f32) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
        (a * 255.0).round() as u8,
    )
}
