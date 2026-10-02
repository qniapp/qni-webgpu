pub(crate) enum Acceptance {
    Closed,
    Awaiting(AcceptedRun),
}

pub(crate) struct AcceptedRun {
    pub(crate) id: u64,
    pub(crate) expected: DisplayExpectation,
}

pub(crate) enum DisplayExpectation {
    None,
    Requested(SlotLayout),
}

#[derive(Default)]
pub(crate) struct SlotLayout {
    pub(crate) amplitude: Vec<u32>,
    pub(crate) bloch: Vec<u32>,
    pub(crate) probability: Vec<u32>,
    pub(crate) density: Vec<u32>,
}

impl From<SlotLayout> for DisplayExpectation {
    fn from(slots: SlotLayout) -> Self {
        if slots.amplitude.is_empty()
            && slots.bloch.is_empty()
            && slots.probability.is_empty()
            && slots.density.is_empty()
        {
            Self::None
        } else {
            Self::Requested(slots)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DisplayExpectation, SlotLayout};

    #[test]
    fn display_expectation_is_requested_when_any_slot_kind_is_nonempty() {
        let layouts = [
            SlotLayout::default(),
            SlotLayout {
                amplitude: vec![11],
                ..Default::default()
            },
            SlotLayout {
                bloch: vec![22],
                ..Default::default()
            },
            SlotLayout {
                probability: vec![33],
                ..Default::default()
            },
            SlotLayout {
                density: vec![44],
                ..Default::default()
            },
        ];
        assert_eq!(
            layouts.map(|slots| matches!(slots.into(), DisplayExpectation::Requested(_))),
            [false, true, true, true, true]
        );
    }
}
