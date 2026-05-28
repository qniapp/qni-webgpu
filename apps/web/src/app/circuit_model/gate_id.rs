/// 配置済みゲートの同一性。
///
/// ゲートは位置・スパン・角度といった可変な属性を持つエンティティで、その属性が
/// 変わっても同一性は保たれる。等価性・順序・ハッシュは内部値で定める。一意な採番は
/// [`GateIdAllocator`] へ集約する。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub(crate) struct GateId {
    value: u32,
}

impl GateId {
    pub(crate) const fn from_u32(value: u32) -> Self {
        Self { value }
    }

    pub(crate) const fn as_u32(self) -> u32 {
        self.value
    }
}

/// 一意な [`GateId`] を順に発行するカウンタ。`next_gate_id += 1` をここに閉じ込める。
#[derive(Clone, Copy, Debug)]
pub(crate) struct GateIdAllocator {
    next: u32,
}

impl GateIdAllocator {
    /// 採番の起点。最初に発行する GateId は `1`（URL 復元の採番と同じ起点）。
    pub(crate) const fn new() -> Self {
        Self { next: 1 }
    }

    /// 一意な GateId を 1 つ発行し、カウンタを進める。
    pub(crate) fn allocate(&mut self) -> GateId {
        let id = GateId::from_u32(self.next);
        self.next += 1;
        id
    }
}

impl Default for GateIdAllocator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{GateId, GateIdAllocator};

    #[test]
    fn keeps_identity_value() {
        assert_eq!(GateId::from_u32(3).as_u32(), 3);
    }

    #[test]
    fn equal_ids_compare_by_value() {
        assert_eq!(GateId::from_u32(1), GateId::from_u32(1));
    }

    #[test]
    fn different_ids_are_not_equal() {
        assert_ne!(GateId::from_u32(1), GateId::from_u32(2));
    }

    #[test]
    fn id_order_matches_numeric_order() {
        assert!(GateId::from_u32(1) < GateId::from_u32(2));
    }

    #[test]
    fn allocator_starts_at_one() {
        assert_eq!(GateIdAllocator::new().allocate(), GateId::from_u32(1));
    }

    #[test]
    fn allocator_issues_distinct_ids() {
        let mut allocator = GateIdAllocator::new();
        let first = allocator.allocate();
        assert_ne!(first, allocator.allocate());
    }

    #[test]
    fn allocator_is_monotonic() {
        let mut allocator = GateIdAllocator::new();
        let first = allocator.allocate();
        assert!(first < allocator.allocate());
    }

    #[test]
    fn default_allocator_matches_new() {
        assert_eq!(
            GateIdAllocator::default().allocate(),
            GateIdAllocator::new().allocate()
        );
    }

    #[test]
    fn id_round_trips_through_hash_map() {
        let mut map = std::collections::HashMap::new();
        map.insert(GateId::from_u32(7), "slot");
        assert_eq!(map.get(&GateId::from_u32(7)).copied(), Some("slot"));
    }
}
