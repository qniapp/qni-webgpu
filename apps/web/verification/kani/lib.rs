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
