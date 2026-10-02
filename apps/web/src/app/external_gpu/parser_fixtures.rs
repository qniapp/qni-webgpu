//! ホストでは JavaScript の解析器を呼べないため、解析境界で検証済みバッチを注入する。
//! JSON 解析の代替実装ではなく、アプリの一括反映を検証するためのテスト専用の準備。

use std::cell::RefCell;

use crate::gpu::{
    ExternalAmplitudeUploadBatch, ExternalBlochUploadBatch, ExternalDensityUploadBatch,
    ExternalProbabilityUploadBatch,
};

macro_rules! parser_fixture {
    ($storage:ident, $inject:ident, $take:ident, $batch:ty) => {
        thread_local! {
            static $storage: RefCell<Option<(String, $batch)>> = const { RefCell::new(None) };
        }

        pub(super) fn $inject(fixture: Option<(String, $batch)>) {
            $storage.with(|slot| *slot.borrow_mut() = fixture);
        }

        pub(super) fn $take(message: &str, generation: u64, slots: &[u32]) -> Option<$batch> {
            $storage.with(|slot| {
                let (expected_message, mut batch) = slot.borrow_mut().take()?;
                if message != expected_message || slots != batch.slot_to_gate_id.as_ref() {
                    panic!("parser fixture does not match the response or requested slots");
                }
                batch.generation = generation;
                Some(batch)
            })
        }
    };
}

parser_fixture!(
    AMPLITUDE,
    inject_amplitude,
    take_amplitude,
    ExternalAmplitudeUploadBatch
);
parser_fixture!(BLOCH, inject_bloch, take_bloch, ExternalBlochUploadBatch);
parser_fixture!(
    PROBABILITY,
    inject_probability,
    take_probability,
    ExternalProbabilityUploadBatch
);
parser_fixture!(
    DENSITY,
    inject_density,
    take_density,
    ExternalDensityUploadBatch
);

pub(super) fn reset() {
    inject_amplitude(None);
    inject_bloch(None);
    inject_probability(None);
    inject_density(None);
}
