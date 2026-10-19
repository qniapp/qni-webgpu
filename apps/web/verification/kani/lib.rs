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

    // 本番の添字計算ではなく、arithmetic.wgsl の main を転記した検証専用モデル。
    // スクリプトが main 全体を固定し、転記元の変更を検出する。
    struct ShaderArithmetic {
        state_count: u32,
        target_shift: u32,
        target_mask: u32,
        amount: u32,
        control_mask: u32,
        control_value: u32,
    }

    impl ShaderArithmetic {
        fn source(&self, i: u32) -> u32 {
            // main の範囲外呼び出しは書き込みをしない。証明では有効添字のみ渡す。
            if i >= self.state_count {
                return i;
            }
            let mut source = i;
            if (i & self.control_mask) == self.control_value {
                let field = (i >> self.target_shift) & self.target_mask;
                let previous = field.wrapping_sub(self.amount) & self.target_mask;
                let mask = self.target_mask << self.target_shift;
                source = (i & !mask) | (previous << self.target_shift);
            }
            source
        }
    }

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

    fn shader() -> ShaderArithmetic {
        let (n, wire, span) = dimensions();
        let field = ArithmeticField::new(wire, span);
        let controls = symbolic_controls();
        kani::assume(controls.mask() < (1u32 << n));
        let controls = field.external_controls(controls);
        ShaderArithmetic {
            state_count: 1u32 << n,
            target_shift: field.shift(),
            target_mask: field.mask(),
            amount: ArithmeticField::amount(kani::any()),
            control_mask: controls.mask(),
            control_value: controls.value(),
        }
    }

    #[kani::proof]
    fn arithmetic_permutation() {
        let params = shader();
        let i: u32 = kani::any();
        let j: u32 = kani::any();
        kani::assume(i < params.state_count && j < params.state_count && i != j);
        let a = params.source(i);
        let b = params.source(j);
        kani::assert(
            a < params.state_count && b < params.state_count && a != b,
            "arithmetic permutation stays in range and is injective",
        );
    }

    #[kani::proof]
    fn arithmetic_locality() {
        let params = shader();
        let i: u32 = kani::any();
        kani::assume(i < params.state_count);
        let source = params.source(i);
        let field_mask = params.target_mask << params.target_shift;
        let inside = (i >> params.target_shift) & params.target_mask;
        let enabled = i & params.control_mask == params.control_value;
        kani::assert(
            if enabled {
                (source & !field_mask) == (i & !field_mask)
                    && ((source >> params.target_shift) & params.target_mask)
                        == (inside.wrapping_sub(params.amount) & params.target_mask)
            } else {
                source == i
            },
            "arithmetic locality preserves outside and wraps inside or is identity",
        );
    }

    #[kani::proof]
    fn arithmetic_inverse() {
        let mut params = shader();
        let i: u32 = kani::any();
        kani::assume(i < params.state_count);
        params.amount = ArithmeticField::amount(true);
        let previous = params.source(i);
        params.amount = ArithmeticField::amount(false);
        kani::assert(
            params.source(previous) == i,
            "arithmetic increment and decrement compose to identity",
        );
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
