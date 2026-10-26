#![cfg(not(target_arch = "wasm32"))]
#[path = "../src/gpu/arithmetic_index.rs"]
mod arithmetic_index;
#[path = "../build_support/arithmetic_codegen.rs"]
mod codegen;

const SOURCE: &str = include_str!("../src/gpu/arithmetic_index.rs");
const SHELL: &str = include_str!("../src/gpu/shaders/arithmetic.wgsl");

fn changed(from: &str, to: &str) -> String {
    if SOURCE.matches(from).count() != 1 {
        panic!("test mutation must match exactly once: {from}");
    }
    SOURCE.replace(from, to)
}
macro_rules! rejects {
    ($($name:ident: $from:expr => $to:expr;)*) => {$(
        #[test]
        fn $name() { assert!(codegen::translate(&changed($from, $to)).is_err()); }
    )*};
}
rejects! {
    rejects_add: "field.wrapping_sub(amount)" => "field + amount";
    rejects_sub: "field.wrapping_sub(amount)" => "field - amount";
    rejects_method: "wrapping_sub" => "wrapping_add";
    rejects_call: "field.wrapping_sub(amount)" => "other(amount)";
    rejects_mut: "let field" => "let mut field";
    rejects_shadow: "let mask" => "let i";
    rejects_unknown: "let mask = target_mask" => "let mask = missing";
    rejects_cast: "let mask = target_mask" => "let mask = target_mask as u32";
    rejects_index: "let mask = target_mask" => "let mask = target_mask[0]";
    rejects_loop: "let mask = target_mask" => "loop {} let mask = target_mask";
    rejects_path: "let mask = target_mask" => "let mask = module::target_mask";
    rejects_bool_condition: "(i & control_mask) != control_value" => "i";
    rejects_bool_local: "let mask = target_mask << target_shift" => "let mask = i == target_mask";
    rejects_bool_operand: "let mask = target_mask << target_shift" => "let mask = (i == target_mask) << target_shift";
    rejects_bool_return: "return i;" => "return i == control_mask;";
    rejects_bool_tail: "(i & !mask) | (previous << target_shift)" => "i == control_mask";
    rejects_bool_not: "!mask" => "!(i == control_mask)";
    rejects_bool_method: "field.wrapping_sub(amount)" => "(i == control_mask).wrapping_sub(amount)";
    rejects_bool_argument: "wrapping_sub(amount)" => "wrapping_sub(i == control_mask)";
    rejects_signed: "let mask = target_mask" => "let mask = 1i32";
    rejects_large_literal: "let mask = target_mask" => "let mask = 4294967296u32";
    rejects_inferred_integer: "let mask = target_mask" => "let mask = 1";
    rejects_signed_inferred_shift: "(i & control_mask) != control_value" => "(1 << target_shift) >> 31 != 1";
    rejects_reference: "i: u32" => "i: &u32";
    rejects_return_type: "-> u32" => "-> bool";
    rejects_generic: "source_index(" => "source_index<T>(";
    rejects_attribute: "pub(crate)" => "#[inline] pub(crate)";
    rejects_local_attribute: "let mask" => "#[allow(unused)] let mask";
    rejects_expression_attribute: "!mask" => "#[allow(unused)] !mask";
    rejects_extra_item: "pub(crate)" => "const X:u32=0; pub(crate)";
    rejects_else: "return i;\n    }" => "return i; } else { return i; }";
    rejects_assignment: "let mask = target_mask << target_shift;" => "i = target_mask;";
    rejects_typed_bool: "let mask = target_mask << target_shift" => "let mask: bool = target_mask << target_shift";
    rejects_method_generics: "wrapping_sub(amount)" => "wrapping_sub::<u32>(amount)";
    rejects_wrong_name: "source_index(" => "other(";
    rejects_const: "pub(crate) fn" => "pub(crate) const fn";
    rejects_async: "pub(crate) fn" => "pub(crate) async fn";
    rejects_unsafe: "pub(crate) fn" => "pub(crate) unsafe fn";
    // syn rejects top-level `safe fn` during parsing, before signature validation.
    rejects_explicit_safe: "pub(crate) fn" => "pub(crate) safe fn";
    rejects_missing_argument: "    control_value: u32,\n" => "";
    rejects_extra_argument: "    control_value: u32," => "    control_value: u32, z: u32,";
    rejects_duplicate_argument: "control_value: u32" => "i: u32";
    rejects_arg_attribute: "i: u32" => "#[allow(unused)] i: u32";
    rejects_mut_arg: "i: u32" => "mut i: u32";
    rejects_missing_initializer: "let mask = target_mask << target_shift" => "let mask";
    rejects_destructure: "let mask" => "let (mask,)";
    rejects_missing_tail: "(i & !mask) | (previous << target_shift)" => "";
    rejects_tail_semicolon: "(i & !mask) | (previous << target_shift)" => "(i & !mask) | (previous << target_shift);";
    rejects_extra_guard_statement: "return i;" => "let x = i; return x;";
    rejects_non_return_guard: "return i;" => "i;";
    rejects_method_arity: "wrapping_sub(amount)" => "wrapping_sub(amount, i)";
    rejects_float: "let mask = target_mask" => "let mask = 1.0";
    rejects_boolean: "let mask = target_mask" => "let mask = true";
    rejects_bool_comparison: "(i & control_mask) != control_value" => "(i == control_mask) != (i == control_value)";
    rejects_file_attribute: "pub(crate)" => "#![allow(unused)] pub(crate)";
}

#[test]
fn actual_source_generates_expected_helper() {
    assert_eq!(codegen::translate(SOURCE).unwrap(), concat!(
        "fn source_index(arg0: u32, arg1: u32, arg2: u32, arg3: u32, arg4: u32, arg5: u32) -> u32 {\n",
        "    if ((((arg0 & arg4)) != arg5)) { return arg0; }\n",
        "    let local0: u32 = (((arg0 >> arg1)) & arg2);\n",
        "    let local1: u32 = ((local0 - arg3) & arg2);\n",
        "    let local2: u32 = (arg2 << arg1);\n",
        "    return (((arg0 & (~local2))) | ((local1 << arg1)));\n}\n",
    ));
}
#[test]
fn generation_is_deterministic() {
    assert_eq!(
        codegen::assemble(SOURCE, SHELL),
        codegen::assemble(SOURCE, SHELL)
    );
}
#[test]
fn generated_artifact_matches_production_inputs() {
    let temp = std::env::temp_dir().join(format!("qni-arithmetic-codegen-{}", std::process::id()));
    std::fs::create_dir(&temp).unwrap();
    codegen::generate(std::path::Path::new(env!("CARGO_MANIFEST_DIR")), &temp);
    let generated = std::fs::read_to_string(temp.join("arithmetic.wgsl")).unwrap();
    std::fs::remove_dir_all(&temp).unwrap();
    assert_eq!(
        generated,
        include_str!(concat!(env!("OUT_DIR"), "/arithmetic.wgsl"))
    );
}
#[test]
fn missing_marker_fails() {
    assert!(codegen::assemble(SOURCE, "").is_err());
}
#[test]
fn duplicate_marker_fails() {
    assert!(codegen::assemble(SOURCE, &codegen::MARKER.repeat(2)).is_err());
}
#[test]
fn reserved_names_are_mapped() {
    assert_eq!(
        codegen::translate(SOURCE),
        codegen::translate(&SOURCE.replace("field", "texture"))
    );
}
#[test]
fn unsigned_literal_and_typed_local_are_supported() {
    assert!(codegen::translate(&changed(
        "let mask = target_mask << target_shift",
        "let mask: u32 = 0xffffffffu32 << 0u32"
    ))
    .unwrap()
    .contains("(4294967295u << 0u)"));
}
#[test]
fn equality_condition_is_supported() {
    assert!(codegen::translate(&changed("!=", "=="))
        .unwrap()
        .contains("=="));
}
#[test]
fn increment_gathers_preceding_field_with_wrap() {
    assert_eq!(
        arithmetic_index::source_index(0b10001, 1, 7, 1, 0, 0),
        0b11111
    );
}
#[test]
fn decrement_gathers_following_field_with_wrap() {
    assert_eq!(
        arithmetic_index::source_index(0b11111, 1, 7, u32::MAX, 0, 0),
        0b10001
    );
}
#[test]
fn disabled_control_preserves_index() {
    assert_eq!(
        arithmetic_index::source_index(0b10001, 1, 7, 1, 16, 0),
        0b10001
    );
}
#[test]
fn selected_control_moves_field() {
    assert_eq!(
        arithmetic_index::source_index(0b10001, 1, 7, 1, 16, 16),
        0b11111
    );
}
