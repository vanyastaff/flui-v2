//! K07 AppCell acquire/release microbenchmark.
//!
//! Run with:
//!
//! ```text
//! cargo run -p flui-core --release --features test-support --example app_cell_bench
//! ```
//!
//! This fixture measures the Candidate B compatibility path:
//! `AppCell::borrow_mut()` acquire, `black_box` of the guard, then drop.
//! It intentionally uses the `test-support` feature to obtain a real
//! `Rc<AppCell>` without widening the production public API.

use flui_core::TestAppContext;
use std::{process::ExitCode, time::Instant};

const DEFAULT_ITERATIONS: u64 = 5_000_000;
const BUDGET_NS: u128 = 1_000;

fn iterations() -> u64 {
    std::env::var("FLUI_APP_CELL_BENCH_ITERS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_ITERATIONS)
}

fn bench_borrow_mut_acquire_release(iterations: u64) -> u128 {
    let cx = TestAppContext::single();
    let app = &cx.app;

    let start = Instant::now();
    for _ in 0..iterations {
        let guard = app.borrow_mut();
        std::hint::black_box(&guard);
        drop(guard);
    }

    start.elapsed().as_nanos() / u128::from(iterations)
}

fn main() -> ExitCode {
    let iterations = iterations();
    let ns_per_op = bench_borrow_mut_acquire_release(iterations);
    let pass = ns_per_op <= BUDGET_NS;

    println!("# K07 app_cell_bench");
    println!("# iterations: {iterations}");
    println!(
        "bench=app_cell_borrow_mut_acquire_release ns/op={ns_per_op} budget_ns={BUDGET_NS} verdict={}",
        if pass { "pass" } else { "fail" }
    );

    if pass {
        ExitCode::SUCCESS
    } else {
        eprintln!("# overall verdict: FAIL - AppCell borrow_mut exceeded budget");
        ExitCode::from(1)
    }
}
