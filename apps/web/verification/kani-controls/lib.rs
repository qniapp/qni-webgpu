#![allow(dead_code)]

#[path = "../../src/qubit_bit.rs"]
mod qubit_bit;

mod controls {
    // 子モジュールから非公開フィールドを生成するため、実ソースをこの位置に取り込む。
    include!("../../src/gates/controls.rs");

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
