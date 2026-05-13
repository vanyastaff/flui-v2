//! SF01 T2.3 — Re-export semantics test for the Key family.
//!
//! Proves the Framework `Key`, `ValueKey`, `GlobalKey` re-exports are
//! thin aliases of the K02 [`flui_core`] types, not divergent
//! definitions. The test takes a value constructed via the Framework
//! re-export and verifies it converts identically to one constructed
//! directly via the engine type.
//!
//! This is NOT a Key behavior test — K02 already has those. The goal is
//! to guard against an accidental re-definition of `Key` inside
//! `flui_framework` that would silently produce different `ElementId`
//! values for the same identity input.

use flui_core::ElementId;
use flui_framework::{GlobalKey, Key, ValueKey};

/// Re-export identity: the Framework `Key` type IS `flui_core::Key`.
/// Compile-time check via the explicit type-equality assertion.
#[test]
fn key_type_is_same_as_flui_core_key() {
    fn assert_same_type<T>(_a: &T, _b: &T) {}
    let from_framework = Key::value(0_usize);
    let from_engine = flui_core::Key::value(0_usize);
    assert_same_type(&from_framework, &from_engine);
}

#[test]
fn value_key_from_usize_roundtrip() {
    let via_framework: ElementId = Key::value(42_usize).into();
    let via_engine: ElementId = flui_core::Key::value(42_usize).into();
    assert_eq!(via_framework, via_engine);
}

#[test]
fn value_key_from_string_roundtrip() {
    let via_framework: ElementId = Key::value(String::from("first-item")).into();
    let via_engine: ElementId = flui_core::Key::value(String::from("first-item")).into();
    assert_eq!(via_framework, via_engine);
}

#[test]
fn value_key_try_from_i32_succeeds_and_roundtrips() {
    // Augmented per the T0.2 reviewer-flagged SR2 silent-regression vector:
    // K02 publishes a fallible TryFrom<i32> for ValueKey impl. SF01 must
    // confirm the re-export chain preserves it.
    let vk_framework = ValueKey::try_from(42_i32).expect("i32 -> ValueKey must succeed");
    let vk_engine =
        flui_core::ValueKey::try_from(42_i32).expect("i32 -> ValueKey must succeed in engine too");

    let via_framework: ElementId = Key::value(vk_framework).into();
    let via_engine: ElementId = flui_core::Key::value(vk_engine).into();
    assert_eq!(via_framework, via_engine);
}

#[test]
fn value_key_try_from_negative_i32_propagates_engine_behavior() {
    // The framework re-export must NOT silently change the fallible
    // semantics of the engine TryFrom impl.
    let framework_result = ValueKey::try_from(-1_i32);
    let engine_result = flui_core::ValueKey::try_from(-1_i32);
    assert_eq!(framework_result.is_err(), engine_result.is_err());
}

#[test]
fn global_key_roundtrip() {
    let via_framework: ElementId = Key::global(GlobalKey::new("app-root")).into();
    let via_engine: ElementId =
        flui_core::Key::global(flui_core::GlobalKey::new("app-root")).into();
    assert_eq!(via_framework, via_engine);
}

#[test]
fn global_key_from_static_str_roundtrip() {
    // GlobalKey has a From<&'static str> impl in K02 — exercised through
    // the Framework re-export to prove the conversion chain is preserved.
    let via_framework: ElementId = Key::global("app-root").into();
    let via_engine: ElementId = flui_core::Key::global("app-root").into();
    assert_eq!(via_framework, via_engine);
}

/// `Key::local()` carries the source location of the call site. Two
/// `Key::local()` calls on the SAME line within a single function should
/// produce equal `ElementId`s only if the engine attributes both to the
/// same `Location`. The framework re-export must not change that
/// behavior.
#[test]
fn local_key_uses_caller_location() {
    // Both calls live on adjacent lines — different source locations.
    let one: ElementId = Key::local().into();
    let two: ElementId = Key::local().into();
    // Distinct source locations → distinct ElementIds.
    assert_ne!(one, two);

    // A single call routed through the engine alias at the matching line
    // is not testable here without macro magic; we assert only that the
    // framework path produces a `CodeLocation` variant that the engine
    // stack normalizes downstream (proven separately by K02 tests).
    let framework_local: ElementId = Key::local().into();
    assert!(
        matches!(framework_local, ElementId::CodeLocation(_)),
        "Framework Key::local() must lower to ElementId::CodeLocation, got {framework_local:?}"
    );
}
