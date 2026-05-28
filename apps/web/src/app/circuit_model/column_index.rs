#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub(crate) struct CircuitColumnIndex {
    value: usize,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CircuitColumnIndexError {
    Negative,
    Overflow,
}

impl CircuitColumnIndex {
    pub(crate) const ZERO: Self = Self { value: 0 };

    pub(crate) const fn new(value: usize) -> Self {
        Self { value }
    }

    #[allow(dead_code)]
    pub(crate) fn try_from_i64(value: i64) -> Result<Self, CircuitColumnIndexError> {
        let value = usize::try_from(value).map_err(|_| {
            if value < 0 {
                CircuitColumnIndexError::Negative
            } else {
                CircuitColumnIndexError::Overflow
            }
        })?;
        Ok(Self::new(value))
    }

    pub(crate) const fn as_usize(self) -> usize {
        self.value
    }

    pub(crate) fn checked_add(self, delta: usize) -> Option<Self> {
        self.value.checked_add(delta).map(Self::new)
    }

    pub(crate) fn saturating_sub(self, delta: usize) -> Self {
        Self::new(self.value.saturating_sub(delta))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_first_column() {
        assert_eq!(CircuitColumnIndex::ZERO.as_usize(), 0);
    }

    #[test]
    fn new_accepts_usize_value() {
        assert_eq!(CircuitColumnIndex::new(3).as_usize(), 3);
    }

    #[test]
    fn try_from_i64_rejects_negative_value() {
        assert_eq!(
            CircuitColumnIndex::try_from_i64(-1),
            Err(CircuitColumnIndexError::Negative)
        );
    }

    #[test]
    fn equal_columns_compare_by_value() {
        assert_eq!(CircuitColumnIndex::new(2), CircuitColumnIndex::new(2));
    }

    #[test]
    fn different_columns_are_not_equal() {
        assert_ne!(CircuitColumnIndex::new(1), CircuitColumnIndex::new(2));
    }

    #[test]
    fn column_order_matches_numeric_order() {
        assert!(CircuitColumnIndex::new(1) < CircuitColumnIndex::new(2));
    }

    #[test]
    fn checked_add_detects_overflow() {
        assert_eq!(CircuitColumnIndex::new(usize::MAX).checked_add(1), None);
    }

    #[test]
    fn saturating_sub_never_goes_below_zero() {
        assert_eq!(
            CircuitColumnIndex::ZERO.saturating_sub(1),
            CircuitColumnIndex::ZERO
        );
    }
}
