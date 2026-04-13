//! S01c behavior pinning tests.
//!
//! These tests lock current behavior for features that the migration
//! (S02-S06) might silently regress. They exercise the public
//! `TestAppContext` / `ClipboardItem` / `Keystroke` surface and are
//! intentionally kept simple — the goal is a regression guard, not a
//! correctness proof.
//!
//! Scope (from spec S01c):
//! - Clipboard read/write round-trip
//! - `ClipboardItem` constructors and accessors
//! - `Keystroke::parse` → `to_string` round-trip
//! - `Modifiers` combinations
//! - Scheduler determinism through `run_until_parked`
//!
//! Behaviors NOT pinned here (documented in `docs/lock-coverage-gaps.md`):
//! IME composition, drag-and-drop, display-link timing, mac find-pasteboard,
//! Linux primary selection, NSServices integration, Wayland special protocols,
//! custom cursors at runtime.

#![cfg(test)]

use crate::{ClipboardItem, Keystroke, Modifiers, TestAppContext};

// ---------------------------------------------------------------------
// Clipboard round-trip
// ---------------------------------------------------------------------

#[test]
fn clipboard_round_trip_string() {
    let cx = TestAppContext::single();
    cx.write_to_clipboard(ClipboardItem::new_string("hello world".into()));
    let readback = cx.read_from_clipboard().expect("clipboard was empty");
    assert_eq!(readback.text().as_deref(), Some("hello world"));
}

#[test]
fn clipboard_overwrite() {
    let cx = TestAppContext::single();
    cx.write_to_clipboard(ClipboardItem::new_string("first".into()));
    cx.write_to_clipboard(ClipboardItem::new_string("second".into()));
    let readback = cx.read_from_clipboard().expect("clipboard was empty");
    assert_eq!(readback.text().as_deref(), Some("second"));
}

#[test]
fn clipboard_empty_on_fresh_context() {
    let cx = TestAppContext::single();
    assert!(cx.read_from_clipboard().is_none());
}

#[test]
fn clipboard_string_with_metadata() {
    let cx = TestAppContext::single();
    cx.write_to_clipboard(ClipboardItem::new_string_with_metadata(
        "content".into(),
        "meta-payload".into(),
    ));
    let readback = cx.read_from_clipboard().expect("clipboard was empty");
    assert_eq!(readback.text().as_deref(), Some("content"));
}

// ---------------------------------------------------------------------
// Keystroke parsing and round-trip
// ---------------------------------------------------------------------

#[test]
fn keystroke_parse_simple_letter() {
    let ks = Keystroke::parse("a").expect("parse");
    assert_eq!(ks.key, "a");
    assert_eq!(ks.modifiers, Modifiers::default());
}

#[test]
fn keystroke_parse_single_modifier() {
    let ks = Keystroke::parse("ctrl-a").expect("parse");
    assert_eq!(ks.key, "a");
    assert!(ks.modifiers.control);
    assert!(!ks.modifiers.shift);
    assert!(!ks.modifiers.alt);
    assert!(!ks.modifiers.platform);
}

#[test]
fn keystroke_parse_multiple_modifiers() {
    let ks = Keystroke::parse("ctrl-shift-k").expect("parse");
    assert_eq!(ks.key, "k");
    assert!(ks.modifiers.control);
    assert!(ks.modifiers.shift);
    assert!(!ks.modifiers.alt);
}

#[test]
fn keystroke_parse_tolerance_probe() {
    // Probe `Keystroke::parse` tolerance for degenerate inputs. This
    // test PINS the current behavior (whatever it is) so we notice if
    // it changes — not the other way around. If any of these probes
    // flips outcome, update the assertion and document why.
    //
    // Current behavior (pinned):
    //   ""        Ok — permissive empty parse
    //   "ctrl-"   Ok — dangling modifier accepted
    //   "ctrl-a"  Ok — normal
    //   "a"       Ok — bare letter
    assert!(
        Keystroke::parse("").is_ok(),
        "empty string currently parses"
    );
    assert!(
        Keystroke::parse("ctrl-").is_ok(),
        "dangling modifier currently parses"
    );
    assert!(
        Keystroke::parse("ctrl-a").is_ok(),
        "normal modifier+key parses"
    );
    assert!(Keystroke::parse("a").is_ok(), "bare letter parses");
}

// macOS Keystroke renderer uses platform-symbolic forms (`^a` for
// `ctrl-a`, `⌥f4` for `alt-f4`, `⌘k` for `cmd-k`) that do not
// round-trip through `Keystroke::parse`. The round-trip property
// holds only on non-macOS platforms where the renderer uses the
// literal `modifier-key` form.
#[cfg(not(target_os = "macos"))]
#[test]
fn keystroke_to_string_round_trip_key_preserved() {
    // Pin the round-trip property that `Keystroke::parse(input).to_string()`
    // produces something that re-parses to the same key. See
    // `keystroke_to_string_round_trip_modifiers` for the modifier-side
    // round-trip and the synthetic-shift exception.
    for input in ["ctrl-a", "ctrl-shift-k", "alt-f4"] {
        let parsed = Keystroke::parse(input).unwrap_or_else(|e| {
            panic!("parse failed for {input}: {e:?}");
        });
        let rendered = parsed.to_string();
        let reparsed = Keystroke::parse(&rendered).unwrap_or_else(|e| {
            panic!("reparse failed for {input} -> {rendered}: {e:?}");
        });
        assert_eq!(
            parsed.key, reparsed.key,
            "round-trip key mismatch for {input}"
        );
    }
}

// Same macOS-specific renderer caveat as `..._round_trip_key_preserved`.
#[cfg(not(target_os = "macos"))]
#[test]
fn keystroke_to_string_round_trip_modifiers() {
    // Separate test: pin that modifier flags survive the round-trip
    // for inputs whose `key` is not an ASCII letter. Letters trigger
    // case-normalization with a synthetic shift in the renderer — e.g.
    // `ctrl-a` renders as `ctrl-A` which re-parses with `shift: true`.
    // That's a known quirk of the Keystroke renderer, not a regression
    // we want to catch here. The tests below use function keys and
    // named keys that bypass the normalization.
    for input in ["alt-f4", "ctrl-f1", "ctrl-alt-f12"] {
        let parsed = Keystroke::parse(input).unwrap_or_else(|e| {
            panic!("parse failed for {input}: {e:?}");
        });
        let rendered = parsed.to_string();
        let reparsed = Keystroke::parse(&rendered).unwrap_or_else(|e| {
            panic!("reparse failed for {input} -> {rendered}: {e:?}");
        });
        assert_eq!(
            parsed.modifiers, reparsed.modifiers,
            "round-trip modifiers mismatch for {input} (rendered as {rendered})"
        );
    }
}

// ---------------------------------------------------------------------
// Modifiers
// ---------------------------------------------------------------------

#[test]
fn modifiers_default_is_empty() {
    let m = Modifiers::default();
    assert!(!m.control);
    assert!(!m.shift);
    assert!(!m.alt);
    assert!(!m.platform);
    assert!(!m.function);
}

#[test]
fn modifiers_equality() {
    let a = Modifiers {
        control: true,
        shift: true,
        ..Default::default()
    };
    let b = Modifiers {
        control: true,
        shift: true,
        ..Default::default()
    };
    let c = Modifiers::default();
    assert_eq!(a, b);
    assert_ne!(a, c);
}

// ---------------------------------------------------------------------
// Scheduler determinism
// ---------------------------------------------------------------------

#[test]
fn scheduler_run_until_parked_drains_tasks() {
    use std::cell::RefCell;
    use std::rc::Rc;

    let cx = TestAppContext::single();
    let order: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));

    // Spawn three foreground tasks in a defined order. With `TestDispatcher`
    // behind the foreground executor, `run_until_parked` drains them
    // deterministically.
    for i in [1u32, 2, 3] {
        let order = order.clone();
        cx.foreground_executor()
            .spawn(async move {
                order.borrow_mut().push(i);
            })
            .detach();
    }

    cx.run_until_parked();

    let final_order = order.borrow().clone();
    assert_eq!(
        final_order.len(),
        3,
        "expected 3 tasks, got {final_order:?}"
    );
    // Do not assert the exact permutation — `TestDispatcher` is allowed
    // to randomize order when seeded for fuzzing. The contract we pin
    // here is that ALL scheduled tasks run by the time
    // `run_until_parked` returns.
    let mut sorted = final_order;
    sorted.sort_unstable();
    assert_eq!(sorted, vec![1, 2, 3]);
}

// ---------------------------------------------------------------------
// `ClipboardItem::new_string_with_json_metadata` smoke
// ---------------------------------------------------------------------

#[test]
fn clipboard_item_json_metadata() {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Payload {
        version: u32,
        label: String,
    }

    let payload = Payload {
        version: 1,
        label: "test".into(),
    };
    let item = ClipboardItem::new_string_with_json_metadata("body".into(), &payload);
    assert_eq!(item.text().as_deref(), Some("body"));
    // No assertion on the metadata extraction API because it's
    // version-specific; we're pinning that construction at least works.
    let _ = item;
}
