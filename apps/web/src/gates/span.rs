use std::num::NonZeroUsize;

use super::GateKind;

/// ゲートが縦方向に占めるワイヤ本数（スパン）。
///
/// 通常の 1 量子ビットゲートは [`GateSpan::SINGLE`]。確率・振幅・密度行列の表示ブロックと
/// QFT / QFT† はホバーで現れるハンドルで伸縮できる。内部値を `NonZeroUsize` で持ち
/// 「1 以上」を型で保証する。種類別の上限とワイヤ残数による切り詰めは [`GateSpan::clamped_for`]
/// に集約する。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct GateSpan {
    value: NonZeroUsize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GateSpanError {
    Zero,
}

impl GateSpan {
    /// 1 量子ビットゲートのスパン（= 1）。
    pub(crate) const SINGLE: Self = Self {
        value: match NonZeroUsize::new(1) {
            Some(value) => value,
            None => unreachable!(),
        },
    };

    pub(crate) fn try_new(value: usize) -> Result<Self, GateSpanError> {
        NonZeroUsize::new(value)
            .map(|value| Self { value })
            .ok_or(GateSpanError::Zero)
    }

    pub(crate) fn get(self) -> usize {
        self.value.get()
    }

    /// 種類別の上限（[`GateKind::max_resizable_span`]）と利用可能ワイヤ数で切り詰めた
    /// 新しいスパンを返す。`span.clamp(1, 16).min(qubits − wire)` 系をここに集約する。
    /// `available_wires` は、ステップ展開では `qubits − wire`、エディタ側では
    /// `capacity − wire`（いずれも飽和減算）を渡す。
    pub(crate) fn clamped_for(self, kind: GateKind, available_wires: usize) -> Self {
        let max_span = kind.max_resizable_span(available_wires);
        // `max_resizable_span` も `self.get()` も 1 以上なので min は 1 以上。
        let clamped = self.get().min(max_span);
        Self::try_new(clamped).expect("clamped span is at least 1")
    }
}

#[cfg(test)]
mod tests {
    use super::{GateSpan, GateSpanError};
    use crate::gates::GateKind;

    #[test]
    fn single_is_one() {
        assert_eq!(GateSpan::SINGLE.get(), 1);
    }

    #[test]
    fn try_new_accepts_positive() {
        assert_eq!(GateSpan::try_new(5).map(GateSpan::get), Ok(5));
    }

    #[test]
    fn try_new_rejects_zero() {
        assert_eq!(GateSpan::try_new(0), Err(GateSpanError::Zero));
    }

    #[test]
    fn equal_spans_compare_by_value() {
        assert_eq!(GateSpan::try_new(3), GateSpan::try_new(3));
    }

    #[test]
    fn probability_caps_at_sixteen() {
        let span = GateSpan::try_new(20).unwrap();
        assert_eq!(span.clamped_for(GateKind::ProbabilityDisplay, 32).get(), 16);
    }

    #[test]
    fn density_caps_at_eight() {
        let span = GateSpan::try_new(20).unwrap();
        assert_eq!(
            span.clamped_for(GateKind::DensityMatrixDisplay, 32).get(),
            8
        );
    }

    #[test]
    fn clamps_to_available_wires() {
        let span = GateSpan::try_new(10).unwrap();
        assert_eq!(span.clamped_for(GateKind::QftGate, 3).get(), 3);
    }

    #[test]
    fn does_not_grow_span_within_limits() {
        let span = GateSpan::try_new(2).unwrap();
        assert_eq!(span.clamped_for(GateKind::ProbabilityDisplay, 8).get(), 2);
    }
}
