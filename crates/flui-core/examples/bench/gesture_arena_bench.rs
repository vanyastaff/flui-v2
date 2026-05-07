//! T22 — Gesture arena performance bench fixture.
//!
//! Run with:
//!
//! ```text
//! cargo run -p flui-core --release --example gesture_arena_bench
//! ```
//!
//! # Sub-benchmarks and pass/fail thresholds
//!
//! | Sub-bench           | Operation                                              | Budget (M2-class)         |
//! |---------------------|--------------------------------------------------------|---------------------------|
//! | `hit_test_8deep`    | Linear scan of an 8-deep `HitboxId` slice              | < 2 µs/query              |
//! | `arena_tick`        | VelocityTracker.add_position+estimate (8 samples)      | < 1.25 µs/event-recognizer|
//! | `full_frame_120hz`  | One hit-test + 8 VelocityTracker ticks + 8 settings rd | < 8 ms p99                |
//!
//! # Coverage scope
//!
//! The exercised paths are the parts of the arena/recognizer hot path
//! that are reachable from `flui-core`'s **public API** alone:
//! hit-test linear scans (proxied with a `SmallVec<u64>` of identical
//! shape because `HitboxId`'s constructor is `pub(crate)`), the
//! `VelocityTracker` (Flutter-LSQ port — used by every drag and scale
//! recognizer), and recognizer settings reads.
//!
//! `PointerEvent` is `#[non_exhaustive]` and can only be constructed
//! by the crate's own `gesture::dispatch` module, so this fixture
//! cannot drive `recognizer.handle_event` directly. Full
//! `GestureArenaManager::dispatch` measurement requires a `Window`+
//! `App`, which would need the `test-support` feature. That feature
//! transitively enables `wayland`/`x11`, which does not build on
//! Windows — so this fixture stays standalone on every platform. The
//! arena-dispatch path is exercised by the T16 lifecycle tests and
//! the T17 per-recognizer tests under `cargo test`.
//!
//! # CI behaviour
//!
//! Each sub-benchmark prints
//! `bench=<name> ns/op=<n> budget_ns=<n> verdict=<pass|fail>`. Process
//! exits with code 1 if any verdict is `fail`, so this fixture is
//! suitable as a CI gate.

use flui_core::scheduler::Instant as SchedulerInstant;
use flui_core::{
    GestureSettings, Pixels, Point, PositionSample, VelocityTracker, px,
};
use std::process::ExitCode;
use std::time::{Duration, Instant};

const HIT_TEST_BUDGET_NS: u128 = 2_000;
const ARENA_TICK_BUDGET_NS: u128 = 1_250;
const FULL_FRAME_BUDGET_NS: u128 = 8_000_000;

const ITERATIONS: usize = 50_000;

fn p(x: f32, y: f32) -> Point<Pixels> {
    Point::new(px(x), px(y))
}

fn bench_hit_test_8deep() -> u128 {
    // Stand-in for `Window::hit_test` — measure the linear scan over
    // an 8-deep hitbox-id slice (the data structure backing
    // `Window::mouse_hit_test`). `HitboxId`'s `u64` field is
    // `pub(crate)`, so we use a `SmallVec<u64; 8>` of identical
    // shape; the cost model is the same: linear scan + integer eq.
    let ids: smallvec::SmallVec<[u64; 8]> = (0..8).collect();
    let target: u64 = 7; // worst case: at the back

    let start = Instant::now();
    let mut hits = 0usize;
    for _ in 0..ITERATIONS {
        for id in ids.iter() {
            if *id == target {
                hits += 1;
                break;
            }
        }
    }
    let elapsed = start.elapsed();
    std::hint::black_box(hits);
    elapsed.as_nanos() / ITERATIONS as u128
}

fn bench_arena_tick() -> u128 {
    // Stand-in for the dispatch hot path — the VelocityTracker
    // `add_position` + `estimate` cycle that drag/scale recognizers
    // run on every PointerEvent. One iteration ≈ one of the 8
    // competing recognizers ticking on one pointer event.
    let settings = GestureSettings::default();
    let mut tracker = VelocityTracker::new(&settings);
    let now = SchedulerInstant::now();
    // Pre-fill so estimate() runs the full quadratic fit.
    for i in 0..8 {
        tracker.add_position(PositionSample::new(
            p(i as f32 * 10.0, 0.0),
            now - Duration::from_millis((20 - i * 2) as u64),
        ));
    }

    let start = Instant::now();
    let mut acc = 0.0f32;
    for i in 0..ITERATIONS {
        tracker.add_position(PositionSample::new(p(i as f32, 0.0), SchedulerInstant::now()));
        let v = tracker.estimate();
        acc += v.pixels_per_second.x;
    }
    let elapsed = start.elapsed();
    std::hint::black_box(acc);
    elapsed.as_nanos() / ITERATIONS as u128
}

fn bench_full_frame_120hz() -> u128 {
    // Combined frame work: 1 hit-test linear scan + 8 VelocityTracker
    // estimates + 8 recognizer-settings reads. Reports p99 over 120
    // frames.
    let settings = GestureSettings::default();
    let ids: smallvec::SmallVec<[u64; 8]> = (0..8).collect();
    let target: u64 = 7;
    let now = SchedulerInstant::now();

    let mut samples_ns: Vec<u128> = Vec::with_capacity(120);
    for _frame in 0..120 {
        let frame_start = Instant::now();

        // 1. Hit-test pass.
        let mut hits = 0usize;
        for id in ids.iter() {
            if *id == target {
                hits += 1;
                break;
            }
        }
        std::hint::black_box(hits);

        // 2. 8 competing recognizers each tick once on the pointer.
        let mut trackers: Vec<VelocityTracker> =
            (0..8).map(|_| VelocityTracker::new(&settings)).collect();
        for tracker in trackers.iter_mut() {
            for i in 0..3 {
                tracker.add_position(PositionSample::new(p(i as f32 * 10.0, 0.0), now));
            }
            let v = tracker.estimate();
            std::hint::black_box(v);
        }

        // 3. 8 recognizer-settings reads (the construction-time path
        //    each recognizer takes when it's added to the arena).
        for _ in 0..8 {
            let s = GestureSettings::default();
            std::hint::black_box(s);
        }

        samples_ns.push(frame_start.elapsed().as_nanos());
    }

    samples_ns.sort_unstable();
    samples_ns[118] // p99 over 120 samples
}

fn report(name: &str, ns_per_op: u128, budget_ns: u128) -> bool {
    let pass = ns_per_op <= budget_ns;
    println!(
        "bench={name} ns/op={ns_per_op} budget_ns={budget_ns} verdict={}",
        if pass { "pass" } else { "fail" }
    );
    pass
}

fn main() -> ExitCode {
    println!("# T22 — gesture_arena_bench");
    println!("# iterations per micro-bench: {ITERATIONS}");

    let mut all_pass = true;

    let hit_test_ns = bench_hit_test_8deep();
    all_pass &= report("hit_test_8deep", hit_test_ns, HIT_TEST_BUDGET_NS);

    let arena_tick_ns = bench_arena_tick();
    all_pass &= report("arena_tick", arena_tick_ns, ARENA_TICK_BUDGET_NS);

    let full_frame_ns = bench_full_frame_120hz();
    all_pass &= report("full_frame_120hz", full_frame_ns, FULL_FRAME_BUDGET_NS);

    if all_pass {
        println!("# overall verdict: pass");
        ExitCode::SUCCESS
    } else {
        eprintln!("# overall verdict: FAIL — at least one budget violated");
        ExitCode::from(1)
    }
}
