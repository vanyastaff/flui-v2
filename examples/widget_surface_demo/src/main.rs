//! SF01 trait-surface demo — `cargo check` only.
//!
//! Demonstrates that a third-party / Tier C consumer can implement the
//! `flui_framework::Widget` and `StatefulWidget` traits against the SF01
//! public surface without reaching into `flui-core` internals. Matches
//! the canonical `Counter` example from `.ai-factory/ARCHITECTURE.md`
//! §"Framework: defining a Stateful Widget".
//!
//! **This example is `cargo check`-only.** `fn main()` constructs a
//! widget and reads its key — it does NOT invoke `Widget::build`,
//! because no mounting adapter exists until SF07. Calling `build` would
//! return the `Empty` widget directly (no panic) but would prove
//! nothing useful since the result cannot be mounted.

use flui_framework::prelude::*;
// WidgetState is intentionally not in the prelude (stability rationale —
// SF01 spec § "prelude"). Out-of-tree consumers import it individually.
use flui_framework::WidgetState;

#[derive(Widget)]
struct Counter {
    initial: i32,
    #[widget(key)]
    id: Option<Key>,
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

// WidgetState body lands in SF04. SF01 ships the marker only.
impl WidgetState<Counter> for CounterState {}

fn main() {
    let counter = Counter {
        initial: 0,
        id: Some(Key::value(42_usize)),
    };

    // Surface check 1: derive(Widget) generated fn key threading
    // through the renamed `id` field.
    let key = counter.key();
    println!("counter.key() = {key:?}");
    debug_assert!(key.is_some(), "Counter constructed with Some(Key)");

    // Surface check 2: StatefulWidget::create_state seeds the state.
    let state = counter.create_state();
    debug_assert_eq!(state.value, 0);

    // Surface check 3: a widget without an explicit key falls through
    // to the default `Widget::key -> None` impl.
    let keyless = Counter {
        initial: 7,
        id: None,
    };
    debug_assert!(keyless.key().is_none());
    debug_assert_eq!(keyless.create_state().value, 7);

    println!("SF01 trait surface compiles for an out-of-tree consumer.");
}
