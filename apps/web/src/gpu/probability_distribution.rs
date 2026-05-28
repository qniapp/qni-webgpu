//! 外部 GPU（Qiskit）経路から受け取る測定確率分布の値オブジェクト。
//!
//! 確率の「計算」は GPU compute shader 側で完結する（GPU-only ルール）。CPU が触れるのは
//! 外部バックエンドが算出済みの確率分布を JSON で受け取り、GPU storage バッファへ転送する
//! 一瞬だけである。その境界で「分布として妥当か」を一度だけ検証して閉じ込めるのがこの型の
//! 役割で、確率値を CPU で再計算するものではない。
//!
//! 生の `Arc<[f32]>` のままだと次の不正がコンパイラにも実行時にも素通りしていた:
//! - NaN / ±Inf / 範囲外（負値・1 超過）の要素
//! - 長さが 2 のべき乗でない（= 2^span に対応しない）配列
//! - 上限 [`MAX_PROBABILITY_OUTCOMES`] を超える長さ（GPU バッファで隣スロットを破壊しうる）
//!
//! これらを [`ProbabilityDistribution::try_new`] が生成時に強制する。GPU への書き込みは
//! [`ProbabilityDistribution::as_slice`] が返す素の `&[f32]` を `bytemuck::cast_slice` に
//! 渡すだけなので、zero-copy 転送と GPU バッファレイアウトは従来どおり保たれる。

use std::sync::Arc;

use super::params::MAX_PROBABILITY_OUTCOMES;

/// 外部 GPU（Qiskit）から受け取った、確率表示スロット 1 つ分の測定確率分布。
///
/// 不変条件（生成時に強制）:
/// - 空でない
/// - 長さが 2 のべき乗（= ある span に対する 2^span 個の測定結果）
/// - 長さが [`MAX_PROBABILITY_OUTCOMES`] 以下
/// - 各要素が有限（NaN / ±Inf でない）かつ概ね `0.0..=1.0`
///   （f64→f32 丸めや外部バックエンドの誤差を吸収するため [`UNIT_TOLERANCE`] の余裕を許す）
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ProbabilityDistribution {
    values: Arc<[f32]>,
}

/// `0.0..=1.0` 判定の許容誤差。Qiskit の f64→f32 変換やヒストグラム近似で生じる
/// わずかな逸脱（例: `1.0000001`）を弾いて確率表示が空になる回帰を避けるための余裕。
const UNIT_TOLERANCE: f32 = 1e-3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ProbabilityDistributionError {
    /// 要素が 1 つも無い。
    Empty,
    /// 長さが 2 のべき乗でない（2^span に対応しない）。
    NotPowerOfTwo { len: usize },
    /// 長さが GPU バッファの 1 スロット枠を超える。
    TooManyOutcomes { len: usize, max: usize },
    /// 非有限、または許容誤差を超えて `0.0..=1.0` を外れた要素。
    OutOfUnitRange { index: usize, value: f32 },
}

impl ProbabilityDistribution {
    pub(crate) fn try_new(values: Arc<[f32]>) -> Result<Self, ProbabilityDistributionError> {
        let len = values.len();
        if len == 0 {
            return Err(ProbabilityDistributionError::Empty);
        }
        if !len.is_power_of_two() {
            return Err(ProbabilityDistributionError::NotPowerOfTwo { len });
        }
        if len > MAX_PROBABILITY_OUTCOMES {
            return Err(ProbabilityDistributionError::TooManyOutcomes {
                len,
                max: MAX_PROBABILITY_OUTCOMES,
            });
        }
        for (index, &value) in values.iter().enumerate() {
            if !value.is_finite() || !(-UNIT_TOLERANCE..=1.0 + UNIT_TOLERANCE).contains(&value) {
                return Err(ProbabilityDistributionError::OutOfUnitRange { index, value });
            }
        }
        Ok(Self { values })
    }

    /// GPU 書き込み用の素の確率値スライス。`bytemuck::cast_slice` にそのまま渡せる。
    pub(crate) fn as_slice(&self) -> &[f32] {
        &self.values
    }
}

#[cfg(test)]
mod tests {
    use super::{ProbabilityDistribution, ProbabilityDistributionError, UNIT_TOLERANCE};
    use std::sync::Arc;

    fn try_new(values: Vec<f32>) -> Result<ProbabilityDistribution, ProbabilityDistributionError> {
        ProbabilityDistribution::try_new(Arc::from(values.into_boxed_slice()))
    }

    #[test]
    fn accepts_valid_power_of_two_distribution() {
        assert!(try_new(vec![0.25, 0.75]).is_ok());
    }

    #[test]
    fn accepts_single_outcome_distribution() {
        // len==1 (2^0) は 2 のべき乗の下限。Empty(len==0) 拒否の直隣の境界。
        assert!(try_new(vec![1.0]).is_ok());
    }

    #[test]
    fn as_slice_round_trips_values() {
        let distribution = try_new(vec![0.1, 0.2, 0.3, 0.4]).expect("valid distribution");
        assert_eq!(distribution.as_slice(), &[0.1, 0.2, 0.3, 0.4]);
    }

    #[test]
    fn rejects_empty_distribution() {
        assert_eq!(try_new(vec![]), Err(ProbabilityDistributionError::Empty));
    }

    #[test]
    fn rejects_non_power_of_two_length() {
        assert!(matches!(
            try_new(vec![0.2, 0.3, 0.5]),
            Err(ProbabilityDistributionError::NotPowerOfTwo { len: 3 })
        ));
    }

    #[test]
    fn rejects_length_above_buffer_slot() {
        let oversized = vec![0.0; super::MAX_PROBABILITY_OUTCOMES * 2];
        assert!(matches!(
            try_new(oversized),
            Err(ProbabilityDistributionError::TooManyOutcomes { .. })
        ));
    }

    #[test]
    fn accepts_max_probability_outcomes_length() {
        // ちょうど上限 (span=16 の Probability16)。`>` を `>=` に取り違える退行を捕捉する。
        assert!(try_new(vec![0.0; super::MAX_PROBABILITY_OUTCOMES]).is_ok());
    }

    #[test]
    fn rejects_nan_element() {
        assert!(matches!(
            try_new(vec![f32::NAN, 0.0]),
            Err(ProbabilityDistributionError::OutOfUnitRange { index: 0, .. })
        ));
    }

    #[test]
    fn rejects_element_above_one() {
        assert!(matches!(
            try_new(vec![0.0, 1.5]),
            Err(ProbabilityDistributionError::OutOfUnitRange { index: 1, .. })
        ));
    }

    #[test]
    fn rejects_negative_element() {
        assert!(matches!(
            try_new(vec![-0.5, 0.5]),
            Err(ProbabilityDistributionError::OutOfUnitRange { index: 0, .. })
        ));
    }

    #[test]
    fn accepts_value_within_unit_tolerance() {
        assert!(try_new(vec![1.0 + UNIT_TOLERANCE / 2.0, 0.0]).is_ok());
    }

    #[test]
    fn accepts_negative_value_within_unit_tolerance() {
        // 許容帯は 0.0/1.0 の両側対称。下限 -UNIT_TOLERANCE が 0.0 に退行しないよう固定する。
        assert!(try_new(vec![-UNIT_TOLERANCE / 2.0, 1.0]).is_ok());
    }
}
