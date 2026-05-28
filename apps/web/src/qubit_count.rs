use std::num::NonZeroUsize;

use crate::constants::{GPU_MAX_QUBITS, LOCAL_MAX_QUBITS};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) struct QubitCount(NonZeroUsize);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) struct QubitCapacity(NonZeroUsize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QubitCountError {
    Zero,
    AboveCapacity { count: usize, capacity: usize },
}

impl QubitCount {
    pub(crate) fn try_new(value: usize) -> Result<Self, QubitCountError> {
        NonZeroUsize::new(value)
            .map(Self)
            .ok_or(QubitCountError::Zero)
    }

    pub(crate) fn try_for_capacity(
        value: usize,
        capacity: QubitCapacity,
    ) -> Result<Self, QubitCountError> {
        let count = Self::try_new(value)?;
        if capacity.contains(count) {
            Ok(count)
        } else {
            Err(QubitCountError::AboveCapacity {
                count: count.get(),
                capacity: capacity.get(),
            })
        }
    }

    pub(crate) fn get(self) -> usize {
        self.0.get()
    }

    /// ローカル実行の状態ベクトル長 `2^n`。ローカル容量を超える `n` は
    /// `QubitCountError::AboveCapacity`。外部 GPU では有効な量子ビット数でも
    /// ローカル状態ベクトルは確保できないため、名前に `local` を残す。
    pub(crate) fn local_state_count(self) -> Result<usize, QubitCountError> {
        let capacity = QubitCapacity::local();
        if capacity.contains(self) {
            Ok(1usize << self.get())
        } else {
            Err(QubitCountError::AboveCapacity {
                count: self.get(),
                capacity: capacity.get(),
            })
        }
    }

    /// GPU バッファ / `GateParams` 用の `u32` 版。同じくローカル容量で検証する。
    pub(crate) fn local_state_count_u32(self) -> Result<u32, QubitCountError> {
        self.local_state_count().map(|count| count as u32)
    }
}

impl QubitCapacity {
    pub(crate) const fn local() -> Self {
        Self(nonzero_const(LOCAL_MAX_QUBITS))
    }

    pub(crate) const fn external_gpu() -> Self {
        Self(nonzero_const(GPU_MAX_QUBITS))
    }

    pub(crate) fn get(self) -> usize {
        self.0.get()
    }

    pub(crate) fn contains(self, count: QubitCount) -> bool {
        count.get() <= self.get()
    }
}

const fn nonzero_const(value: usize) -> NonZeroUsize {
    match NonZeroUsize::new(value) {
        Some(value) => value,
        None => panic!("qubit count constants must be non-zero"),
    }
}

#[cfg(test)]
mod tests {
    use super::{QubitCapacity, QubitCount, QubitCountError};

    #[test]
    fn zero_is_rejected() {
        assert_eq!(QubitCount::try_new(0), Err(QubitCountError::Zero));
    }

    #[test]
    fn one_is_accepted() {
        assert_eq!(QubitCount::try_new(1).map(QubitCount::get), Ok(1));
    }

    #[test]
    fn local_capacity_rejects_seventeen_qubits() {
        assert_eq!(
            QubitCount::try_for_capacity(17, QubitCapacity::local()),
            Err(QubitCountError::AboveCapacity {
                count: 17,
                capacity: 16,
            })
        );
    }

    #[test]
    fn external_gpu_capacity_accepts_thirty_two_qubits() {
        assert_eq!(
            QubitCount::try_for_capacity(32, QubitCapacity::external_gpu()).map(QubitCount::get),
            Ok(32)
        );
    }

    #[test]
    fn external_gpu_capacity_rejects_thirty_three_qubits() {
        assert_eq!(
            QubitCount::try_for_capacity(33, QubitCapacity::external_gpu()),
            Err(QubitCountError::AboveCapacity {
                count: 33,
                capacity: 32,
            })
        );
    }

    #[test]
    fn local_state_count_is_available_inside_local_capacity() {
        let count = QubitCount::try_for_capacity(16, QubitCapacity::local()).unwrap();

        assert_eq!(count.local_state_count(), Ok(65_536));
    }

    #[test]
    fn local_state_count_u32_is_available_inside_local_capacity() {
        let count = QubitCount::try_for_capacity(16, QubitCapacity::local()).unwrap();

        assert_eq!(count.local_state_count_u32(), Ok(65_536u32));
    }

    #[test]
    fn local_state_count_rejects_external_only_count() {
        let count = QubitCount::try_for_capacity(32, QubitCapacity::external_gpu()).unwrap();

        assert_eq!(
            count.local_state_count(),
            Err(QubitCountError::AboveCapacity {
                count: 32,
                capacity: 16,
            })
        );
    }
}
