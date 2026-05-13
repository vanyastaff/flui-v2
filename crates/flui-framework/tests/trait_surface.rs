//! SF01 T3.5 — Trait surface conformance tests.
//!
//! These tests prove that the SF01 trait surface compiles for the
//! canonical widget shapes a Tier C / app author will write:
//!
//! - A leaf widget with no key and no children (`Leaf`).
//! - A container widget with an explicit `Key` accessor (`Container`).
//! - A stateful widget with associated state and factory (`Counter`).
//!
//! The tests are compile-only beyond a single smoke assertion on
//! `Widget::key()`. They do NOT invoke `Widget::build` — SF01 widgets
//! cannot be mounted until SF07 ships the adapter.

use flui_framework::{Empty, IntoWidget, Key, StatefulWidget, Widget, WidgetState};

// -------- Leaf: no key, default `key()` returns None ---------------

struct Leaf;

impl Widget for Leaf {
    fn build(&self) -> impl IntoWidget {
        Empty
    }
}

#[test]
fn leaf_widget_default_key_is_none() {
    let leaf = Leaf;
    assert!(leaf.key().is_none());
}

// -------- Container: explicit Key field, overrides `key()` --------

struct Container {
    key: Option<Key>,
}

impl Widget for Container {
    fn key(&self) -> Option<&Key> {
        self.key.as_ref()
    }

    fn build(&self) -> impl IntoWidget {
        Empty
    }
}

#[test]
fn container_widget_key_passthrough() {
    let with_key = Container {
        key: Some(Key::local()),
    };
    let without_key = Container { key: None };

    assert!(with_key.key().is_some());
    assert!(without_key.key().is_none());
}

// -------- Counter: full StatefulWidget shape per ARCHITECTURE.md -----
//
// Matches the example in `.ai-factory/ARCHITECTURE.md` §"Framework:
// defining a Stateful Widget" — the canonical Tier C / app-author
// pattern. The `WidgetState<W>` impl is empty in SF01 (body lands in
// SF04). The state-factory path (`Counter::create_state()`) IS
// callable in SF01.

struct Counter {
    initial: i32,
    key: Option<Key>,
}

impl Widget for Counter {
    fn key(&self) -> Option<&Key> {
        self.key.as_ref()
    }

    fn build(&self) -> impl IntoWidget {
        Empty
    }
}

impl StatefulWidget for Counter {
    type State = CounterState;

    fn create_state(&self) -> CounterState {
        CounterState {
            value: self.initial,
        }
    }
}

struct CounterState {
    value: i32,
}

impl WidgetState<Counter> for CounterState {}

#[test]
fn counter_create_state_seeds_initial_value() {
    let counter = Counter {
        initial: 7,
        key: None,
    };
    let state = counter.create_state();
    assert_eq!(state.value, 7);
}

#[test]
fn counter_with_value_key_round_trips_through_key_accessor() {
    let counter = Counter {
        initial: 0,
        key: Some(Key::value(123_usize)),
    };
    let k = counter.key().expect("Counter has a Key");
    // Key is opaque — we cannot inspect its kind directly, but the
    // accessor returning Some(_) is the type-level guarantee.
    let _ = k;
}

// -------- IntoWidget blanket impl conformance -----------------------

#[test]
fn empty_widget_implements_into_widget_via_blanket() {
    fn require_into_widget<W: IntoWidget>(_w: &W) {}
    require_into_widget(&Empty);
    require_into_widget(&Leaf);
    require_into_widget(&Container { key: None });
    require_into_widget(&Counter {
        initial: 0,
        key: None,
    });
}

#[test]
fn into_widget_conversion_is_identity() {
    let counter = Counter {
        initial: 5,
        key: None,
    };
    // Identity blanket: `<Counter as IntoWidget>::Widget = Counter`.
    let same: <Counter as IntoWidget>::Widget = counter.into_widget();
    assert_eq!(same.initial, 5);
}
