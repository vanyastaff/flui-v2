# ADR-016: Wasm target dependency gating — keep `imp` and native crates out

**Date:** 2026-05-12
**Status:** Draft — contract only. No code changes land with this ADR.
**Scope:** `crates/flui-core/Cargo.toml`, the `platform/web/` module,
`platform/wgpu/wgpu_context.rs` (wasm adapter selection).
**Drivers:** [zed-industries/zed#52715](https://github.com/zed-industries/zed/issues/52715).

## Context

GPUI #52715 reports two distinct wasm regressions in the `gpui_web`
build of the upstream `hello_world` example:

1. A runtime error `Uncaught Error: closure invoked recursively or
   after being dropped` originating from a fix introduced by PR
   #50985 — wasm-bindgen's reentrancy rules clashed with the new
   closure shape.
2. A build error caused by `proptest` becoming a non-optional
   dependency in PR #51569; on wasm, `proptest` transitively pulls
   `imp` (a libc/syscalls crate that does not compile to
   `wasm32-unknown-unknown`).

Both share a single root cause: the upstream `Cargo.toml` does not
treat the wasm target as a first-class constraint. A test dependency
ends up being a build dependency in production; a runtime helper
ends up being re-invoked recursively in a context wasm-bindgen does
not allow.

flui-v2 has historically been more disciplined here — let's verify
that, write the discipline down, and lock it in.

## Current behaviour (verified)

[`crates/flui-core/Cargo.toml:15`](../../../crates/flui-core/Cargo.toml#L15):

```toml
default = ["font-kit", "wayland", "x11", "windows-manifest"]
test-support = [
    "leak-detection",
    "backtrace",
    "collections/test-support",
    "util/test-support",
    "http_client/test-support",
    "dep:proptest",
]
```

[`crates/flui-core/Cargo.toml:96`](../../../crates/flui-core/Cargo.toml#L96)
carries an explicit note:

```toml
# `proptest` is gated behind the `test-support` feature (see `[features]`)
# duplicate non-optional entry that made proptest a production link dep for
proptest = { version = "1", optional = true }
```

— so `proptest` is `optional = true` and gated behind `test-support`,
never pulled by `default`. The GPUI #52715 second sub-bug
(`proptest` pulling `imp` in production wasm builds) is **not
reproduced** in flui-v2.

[`crates/flui-core/src/platform/wgpu/wgpu_context.rs:136`](../../../crates/flui-core/src/platform/wgpu/wgpu_context.rs#L136):

```rust
backends: wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
```

— the wasm branch of adapter selection is separated, distinct from
the native VULKAN+GL branch.

[`crates/flui-core/src/platform/wgpu/wgpu_renderer.rs:1638`](../../../crates/flui-core/src/platform/wgpu/wgpu_renderer.rs#L1638)
gates `Renderer::recover` behind `#[cfg(not(target_family = "wasm"))]`
— another point where the wasm target has been remembered
explicitly.

Target-specific `[target.'cfg(target_os = …)'.dependencies]` sections
exist for macOS, Linux/FreeBSD, and Windows — none for wasm. The
wasm path piggybacks on cross-target `dependencies` plus per-cfg
attributes inside the code. No documented policy.

## Findings vs upstream

| Issue | Sub-bug | Repro in flui-v2 today |
|-------|---------|-------------------------|
| [zed-industries/zed#52715](https://github.com/zed-industries/zed/issues/52715) | `proptest` not behind a feature → pulls `imp` on wasm. | **no** — already gated through `dep:proptest` in `test-support`. |
| [zed-industries/zed#52715](https://github.com/zed-industries/zed/issues/52715) | wasm-bindgen closure reentrancy. | **unknown** — depends on whether flui-core has the same closure helper that PR #50985 introduced upstream. Worth a focused audit when we ship our first wasm `hello_world`. |

## Decision (contract)

1. **Dev-dependencies must never become production dependencies on
   any target.** `dev-dependencies` are the right home for `proptest`,
   `criterion`, `tempfile`, etc. Anything that test code uses sits
   under `dev-dependencies`. The exception is `dep:foo` in a
   feature — opt-in, not default.

2. **A dependency that does not compile on `wasm32-unknown-unknown`
   is gated by `cfg(not(target_family = "wasm"))`** at the
   `Cargo.toml` level (`[target.'cfg(not(target_family =
   "wasm"))'.dependencies]`), not only inside `lib.rs`. The compile
   error must surface at dependency resolution, not after a long
   build.

3. **The `default` feature set is wasm-buildable.** Running `cargo
   check --target wasm32-unknown-unknown -p flui-core` with no
   feature overrides must succeed. CI on a future schedule should
   verify it.

4. **Wasm-specific dependencies (`web-sys`, `js-sys`,
   `wasm-bindgen`) sit under `[target.'cfg(target_family =
   "wasm")'.dependencies]`.** They are not under the cross-target
   `[dependencies]` block.

5. **A closure registered with wasm-bindgen (`Closure::wrap`) is
   stored on the JS side; the Rust side must hand off
   ownership.** A future helper that recursively re-invokes a JS
   callback must use `wasm_bindgen_futures::spawn_local` (or
   equivalent) rather than calling into itself synchronously. This
   closes the GPUI #52715 first sub-bug pattern. Concrete
   call-sites are an audit, not a code change in this ADR.

6. **The wasm renderer-recovery path documents what it
   omits.** ADR-005 explicitly gates `Renderer::recover` to
   non-wasm builds (the wgpu device-loss API differs on web). The
   contract: the web platform handles loss through page reload,
   not through `recover()`.

## Consequences

- A regression like GPUI #52715 sub-bug 2 cannot land in flui-core
  without breaking CI (once the wasm check job is added).
- The wasm closure-reentrancy class of bug remains possible until
  every `Closure::wrap` site has been audited; the contract makes
  the audit a one-time task instead of an ongoing risk.
- A future flui-platform / flui-web crate split (currently
  `flui-platform` is empty) does not need to redesign the
  dependency policy.

## Out of scope (separate ADRs)

- **A wasm `hello_world` example** that exercises every code path.
  Belongs to the example-gallery / DX cycle, not this ADR.
- **WebGPU vs WebGL adapter selection** under different browsers.
  ADR-014's `RendererKind` enum is the right place for that
  classification.
- **Hot-reload / wasm-pack workflow.** DX tooling, orthogonal.
- **JS-Rust async-bridge ergonomics** beyond closure reentrancy.

## Action items (tracked; no code lands with this ADR)

1. Add a `cargo check --target wasm32-unknown-unknown -p flui-core`
   job to CI (or a manual `make` target until CI is wired). The
   first failure is allowed to be informational; once green, it
   becomes blocking.
2. Audit every `wasm_bindgen::Closure` / `wasm_bindgen::closure::Closure`
   site in flui-core. Document the lifetime of the closure with a
   comment block; convert recursive invocations to `spawn_local`
   where possible.
3. Move any wasm-specific dependency under a
   `[target.'cfg(target_family = "wasm")'.dependencies]` block in
   `Cargo.toml`. Audit the current cross-target block for unused
   wasm-only crates.
4. Add a `// CONTRACT:` comment block near the top of
   [`Cargo.toml`](../../../crates/flui-core/Cargo.toml) summarising
   the gating policy.

## References

### Upstream issues
- [zed-industries/zed#52715](https://github.com/zed-industries/zed/issues/52715) — wasm regressions (closure recursion, proptest).

### Internal
- [docs/research/adr/ADR-005-gpu-device-loss.md](ADR-005-gpu-device-loss.md) — `recover()` is non-wasm by `cfg`.
- [docs/research/adr/ADR-014-software-rendering-fallback.md](ADR-014-software-rendering-fallback.md) — `RendererKind` is the right home for WebGPU vs WebGL classification.
- [docs/research/gpui-adr-candidates.md](../gpui-adr-candidates.md) — theme #8 (_Strategic / roadmap_).
