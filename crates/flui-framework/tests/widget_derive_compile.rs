//! SF01 T4.3 / T4.5 — trybuild compile-pass and compile-fail fixtures
//! for `#[derive(Widget)]`.
//!
//! Each fixture is a standalone `.rs` file under
//! `crates/flui-framework/tests/widget_derive/`. `pass_*.rs` files
//! must compile cleanly; `fail_*.rs` files must fail to compile with
//! the exact diagnostic captured in the adjacent `.stderr` snapshot.
//!
//! Re-bless procedure on rustc-diagnostic drift (e.g., MSRV bump): run
//! `TRYBUILD=overwrite cargo test -p flui-framework --test widget_derive_compile`
//! and commit the regenerated `.stderr` files in a dedicated
//! "ci: re-bless trybuild snapshots" commit. See SF01 design spec
//! §"Trybuild snapshot re-bless procedure" for details.

#[test]
fn widget_derive() {
    let t = trybuild::TestCases::new();

    // Compile-pass cases — the derive must accept these struct shapes
    // and emit a working `impl Widget`.
    t.pass("tests/widget_derive/pass_simple.rs");
    t.pass("tests/widget_derive/pass_with_key.rs");
    t.pass("tests/widget_derive/pass_generic.rs");
    t.pass("tests/widget_derive/pass_renamed_key_field.rs");

    // Compile-fail cases — the derive must reject these inputs with the
    // exact diagnostic captured in the matching `.stderr` snapshot.
    t.compile_fail("tests/widget_derive/fail_enum.rs");
    t.compile_fail("tests/widget_derive/fail_multiple_keys.rs");
    t.compile_fail("tests/widget_derive/fail_wrong_key_type.rs");
    // Bug fixes 2026-05-12 (S1, S2 from /aif-review):
    t.compile_fail("tests/widget_derive/fail_widget_empty_attr.rs");
    t.compile_fail("tests/widget_derive/fail_widget_unknown_subarg.rs");
    // Bug fixes PR #18 (S3 from Copilot review — non-list meta forms):
    t.compile_fail("tests/widget_derive/fail_widget_bare_path.rs");
    t.compile_fail("tests/widget_derive/fail_widget_name_value.rs");
}
