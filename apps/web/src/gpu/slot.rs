use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

pub(crate) struct SlotIndex<F> {
    value: u32,
    _marker: PhantomData<F>,
}

impl<F> SlotIndex<F> {
    pub(crate) const fn new(value: u32) -> Self {
        Self {
            value,
            _marker: PhantomData,
        }
    }

    pub(crate) const fn as_u32(self) -> u32 {
        self.value
    }
}

impl<F> Clone for SlotIndex<F> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<F> Copy for SlotIndex<F> {}

impl<F> PartialEq for SlotIndex<F> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<F> Eq for SlotIndex<F> {}

impl<F> Hash for SlotIndex<F> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl<F> std::fmt::Debug for SlotIndex<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SlotIndex").field(&self.value).finish()
    }
}

pub(crate) struct SlotAllocator<F> {
    next: u32,
    _marker: PhantomData<F>,
}

impl<F> SlotAllocator<F> {
    pub(crate) const fn new() -> Self {
        Self {
            next: 0,
            _marker: PhantomData,
        }
    }

    pub(crate) fn allocate(&mut self) -> SlotIndex<F> {
        let slot = SlotIndex::new(self.next);
        self.next += 1;
        slot
    }
}

impl<F> Default for SlotAllocator<F> {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) enum Snapshot {}
pub(crate) enum Bloch {}
pub(crate) enum Measurement {}
pub(crate) enum Postselection {}
pub(crate) enum Probability {}
pub(crate) enum Amplitude {}
pub(crate) enum Density {}

#[cfg(test)]
mod tests {
    use super::{Bloch, SlotAllocator, SlotIndex};

    #[test]
    fn keeps_slot_value() {
        assert_eq!(SlotIndex::<Bloch>::new(2).as_u32(), 2);
    }

    #[test]
    fn allocator_first_slot_is_zero() {
        assert_eq!(SlotAllocator::<Bloch>::new().allocate().as_u32(), 0);
    }

    #[test]
    fn allocator_second_slot_is_one() {
        let mut allocator = SlotAllocator::<Bloch>::new();
        allocator.allocate();
        assert_eq!(allocator.allocate().as_u32(), 1);
    }

    #[test]
    fn default_allocator_matches_new() {
        assert_eq!(
            SlotAllocator::<Bloch>::default().allocate().as_u32(),
            SlotAllocator::<Bloch>::new().allocate().as_u32()
        );
    }

    #[test]
    fn equal_slots_compare_by_value() {
        assert_eq!(SlotIndex::<Bloch>::new(3), SlotIndex::<Bloch>::new(3));
    }

    #[test]
    fn different_slots_are_not_equal() {
        assert_ne!(SlotIndex::<Bloch>::new(3), SlotIndex::<Bloch>::new(4));
    }

    #[test]
    fn slot_round_trips_as_hash_map_key() {
        let mut map = std::collections::HashMap::new();
        map.insert(SlotIndex::<Bloch>::new(5), "five");
        assert_eq!(map.get(&SlotIndex::<Bloch>::new(5)).copied(), Some("five"));
    }
}
