#![allow(dead_code)]

#[path = "../../src/qubit_bit.rs"]
mod qubit_bit;

mod controls {
    // 子モジュールから非公開フィールドを生成するため、実ソースをこの位置に取り込む。
    include!("../../src/gates/controls.rs");

    #[cfg(kani)]
    pub(crate) fn symbolic_controls() -> ColumnControls {
        let mask: u32 = kani::any();
        let value: u32 = kani::any();
        kani::assume(value & !mask == 0);
        ColumnControls { mask, value }
    }

    #[cfg(kani)]
    mod verification {
        use super::ColumnControls;

        struct RestrictedOutsideSpec;

        impl RestrictedOutsideSpec {
            fn holds(before: ColumnControls, after: ColumnControls, span_mask: u32) -> bool {
                (after.mask() & span_mask == 0)
                    && (after.value() & span_mask == 0)
                    && ((before.mask() ^ after.mask()) & !span_mask == 0)
                    && ((before.value() ^ after.value()) & !span_mask == 0)
            }
        }

        #[kani::proof]
        fn restricted_outside_removes_only_inside_controls() {
            let mask: u32 = kani::any();
            let value: u32 = kani::any();
            let span_mask: u32 = kani::any();
            kani::assume(value & !mask == 0);
            let before = ColumnControls { mask, value };
            let after = before.restricted_outside(span_mask);
            kani::assert(
                RestrictedOutsideSpec::holds(before, after, span_mask),
                "restricted_outside clears inside bits and preserves outside mask/value bits",
            );
        }
    }
}

mod gates {
    pub(crate) use crate::controls::ColumnControls;
}

#[path = "../../src/simulation_plan/arithmetic_field.rs"]
mod arithmetic_field;

#[cfg(kani)]
mod arithmetic_verification {
    use super::arithmetic_field::ArithmeticField;
    use super::controls::symbolic_controls;

    // ここでの 16 はローカル実行の上限。外部 GPU の上限 32 ではない。
    fn dimensions() -> (u32, u32, u32) {
        let n: u32 = kani::any();
        kani::assume(n >= 1 && n <= 16);
        let wire: u32 = kani::any();
        kani::assume(wire < n);
        let span: u32 = kani::any();
        kani::assume(span >= 1 && span <= n - wire);
        (n, wire, span)
    }

    #[kani::proof]
    fn arithmetic_plan_preconditions() {
        // コンストラクタがクランプするという証明ではない。呼び出し側の前提を置く。
        let (n, wire, span) = dimensions();
        let field = ArithmeticField::new(wire, span);
        let before = symbolic_controls();
        kani::assume(before.mask() < (1u32 << n));
        let after = field.external_controls(before);
        let shifted = field.mask() << field.shift();
        let increment: bool = kani::any();
        kani::assert(
            field.shift() == wire
                && field.mask() == (1u32 << span) - 1
                && shifted < (1u32 << n)
                && after.mask() & shifted == 0
                && after.value() & shifted == 0
                && after.value() & !after.mask() == 0
                && after.mask() == before.mask() & !shifted
                && after.value() == before.value() & !shifted
                && ArithmeticField::amount(increment) == if increment { 1 } else { u32::MAX },
            "arithmetic plan has exact mask fit disjoint controls and correct amount",
        );
    }
}

#[path = "../../src/gpu/arithmetic_index.rs"]
mod arithmetic_index;

#[cfg(kani)]
mod index_verification {
    use super::arithmetic_index::source_index;

    fn parameters() -> (u32, u32, u32, u32, u32) {
        let width: u32 = kani::any();
        kani::assume(width >= 1 && width <= 16);
        let shift: u32 = kani::any();
        kani::assume(shift <= 32 - width);
        let mask = (1u32 << width) - 1;
        let controls: u32 = kani::any();
        let value: u32 = kani::any();
        kani::assume(value & !controls == 0);
        kani::assume(controls & (mask << shift) == 0);
        let increment: bool = kani::any();
        (
            shift,
            mask,
            if increment { 1 } else { u32::MAX },
            controls,
            value,
        )
    }

    #[kani::proof]
    fn arithmetic_permutation() {
        let (shift, mask, amount, controls, value) = parameters();
        let i: u32 = kani::any();
        let j: u32 = kani::any();
        kani::assume(i != j);
        kani::assert(
            source_index(i, shift, mask, amount, controls, value)
                != source_index(j, shift, mask, amount, controls, value),
            "arithmetic source lookup is injective",
        );
    }

    #[kani::proof]
    fn arithmetic_outside_bits() {
        let (shift, mask, amount, controls, value) = parameters();
        let i: u32 = kani::any();
        let source = source_index(i, shift, mask, amount, controls, value);
        kani::assert(
            (source ^ i) & !(mask << shift) == 0,
            "arithmetic preserves outside bits",
        );
    }

    #[kani::proof]
    fn arithmetic_inverse() {
        let (shift, mask, amount, controls, value) = parameters();
        let i: u32 = kani::any();
        let source = source_index(i, shift, mask, amount, controls, value);
        kani::assert(
            source_index(
                source,
                shift,
                mask,
                0u32.wrapping_sub(amount),
                controls,
                value,
            ) == i,
            "arithmetic opposite amounts are inverse",
        );
    }

    #[kani::proof]
    fn arithmetic_direction() {
        let (shift, mask, amount, controls, value) = parameters();
        let i: u32 = kani::any();
        kani::assume(i & controls == value);
        let source = source_index(i, shift, mask, amount, controls, value);
        kani::assert(
            ((source >> shift).wrapping_add(amount) & mask) == ((i >> shift) & mask),
            "arithmetic source plus amount equals destination field",
        );
    }

    #[kani::proof]
    fn arithmetic_disabled_controls() {
        let (shift, mask, amount, controls, value) = parameters();
        let i: u32 = kani::any();
        kani::assume(i & controls != value);
        kani::assert(
            source_index(i, shift, mask, amount, controls, value) == i,
            "arithmetic disabled controls preserve index",
        );
    }
}
