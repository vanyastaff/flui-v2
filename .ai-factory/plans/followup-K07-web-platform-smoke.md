# Follow-up: K07 Web Platform Smoke Recipe

- **Created:** 2026-05-10
- **Source:** K07 Task 41 validation gap
- **Status:** Planned

## Context

K07 requires a web-platform smoke check if reachable from existing test
infrastructure. During K07 validation, no documented recipe was found for
`wasm32-unknown-unknown` smoke/build verification in `README.md`, `docs/`,
`.github/`, `examples/`, or the relevant Cargo manifests. The local toolchain
also only had `x86_64-pc-windows-msvc` installed.

This follow-up does not claim a K07 web regression. It records the missing
test path so the web dispatcher re-entry exposure inherited from K15 can be
verified intentionally.

## Goal

Add a repeatable web-platform smoke recipe for flui-core and at least one
minimal example.

## Tasks

- [ ] Document prerequisites: `rustup target add wasm32-unknown-unknown` and
      any required wasm runner/bundler.
- [ ] Add a minimal build command for `flui-core` on `wasm32-unknown-unknown`
      with the correct feature set.
- [ ] Add or document one minimal web example smoke path.
- [ ] Decide whether this belongs in CI as a PR gate or scheduled job.
- [ ] Re-run the K07 AppCell path under the new web recipe and confirm no
      `flui_core::app::cell` warning events under normal startup.

## Acceptance

- A contributor can copy one documented command sequence and verify the web
  build/smoke locally.
- CI coverage decision is recorded in the roadmap or the follow-up plan.
- The K15/K07 web re-entry limitation is either closed or explicitly carried
  forward with a concrete owner.
