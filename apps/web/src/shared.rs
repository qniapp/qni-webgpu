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

    #[cfg(test)]
    if let Some(now) = test_clock::take() {
        return now;
    }

    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) mod test_clock {
    use std::cell::RefCell;
    use std::collections::VecDeque;

    thread_local! {
        static TIMES: RefCell<Option<VecDeque<f64>>> = const { RefCell::new(None) };
    }

    pub(super) fn take() -> Option<f64> {
        TIMES.with(|times| {
            times
                .borrow_mut()
                .as_mut()
                .map(|queue| queue.pop_front().expect("unexpected now_seconds() call"))
        })
    }

    pub(crate) fn with_times<T>(times: &[f64], action: impl FnOnce() -> T) -> (T, usize) {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                TIMES.with(|times| *times.borrow_mut() = None);
            }
        }
        TIMES.with(|slot| *slot.borrow_mut() = Some(times.iter().copied().collect()));
        let _reset = Reset;
        let result = action();
        let remaining = TIMES.with(|slot| slot.borrow().as_ref().unwrap().len());
        (result, remaining)
    }
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
