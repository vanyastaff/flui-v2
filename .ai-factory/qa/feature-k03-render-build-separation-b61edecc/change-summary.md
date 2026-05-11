# Change Summary: K03 Render to Build Separation

## Snapshot

- Branch: `feature/k03-render-build-separation`
- Compared against: `main`
- Date: 2026-05-11
- Pipeline mode: full QA package
- Implementation status: implemented in PR #14
- Initial implementation commit: `aa5e48bd76`
- Committed files changed versus `main`: 22 K03 files in the implementation PR
- Relevant artifacts: `.ai-factory/plans/feature-k03-render-build-separation.md`,
  `docs/superpowers/specs/2026-05-11-K03-render-build-separation-design.md`,
  and `docs/superpowers/migrations/K03-render-build-separation.md`

This QA package now corresponds to the implemented K03 PR. It covers the committed
runtime substrate, macro compatibility test, documentation, migration guide,
roadmap/context updates, and validation evidence for the K03 render/build
separation work.

Unrelated local working-tree changes and AI context files are out of scope unless they are intentionally staged into the K03 work later.

## What Changed

K03 separates engine-level `Render` behavior from future framework-level pure
build semantics. The implementation adds `ElementBuilder`, `ElementBuildCx`,
`BuildElement`, and `build_element` as a deliberately narrow `flui-core`
substrate for immutable engine recipes while preserving existing `Render`,
`RenderOnce`, `IntoElement`, `Component<C>`, root mounting, provider, cache,
deferred-draw, macro, and Tier C behavior.

The supporting plan contains 29 completed tasks and explicit coverage for Tier C
consumer crates, object-safety and RPITIT decisions, workspace dependency
boundaries, macro compatibility, identity propagation, provider semantics, and
documentation/migration updates.

## Affected Areas

| Area | Current change | Expected K03 impact |
|---|---:|---|
| `.ai-factory/plans/feature-k03-render-build-separation.md` | Added/refined | Records implementation order and completed acceptance gates |
| `crates/flui-core/src/build.rs` | Added | Defines the K03 `ElementBuilder` substrate and regression coverage |
| `crates/flui-core/src/element.rs` | Updated | Preserves `Render`, `RenderOnce`, `IntoElement`, `Component`, and build boundary behavior |
| `crates/flui-core/src/lib.rs` / `prelude.rs` | Updated | Curates public exports for the new K03 API |
| `crates/flui-macros` | Updated tests | Proves derive-generated render and element code remains source-compatible |
| `crates/flui-widgets`, `crates/flui-material`, `crates/flui-navigator` | Audited/docs updated | Confirms Tier C consumers compile unchanged or have documented vocabulary |
| `docs/superpowers/*` and `.ai-factory/*` | Updated | Captures spec, migration, roadmap, research, QA, and status updates |

## Risk Level

Overall implementation risk: High before validation.

Implemented PR risk: Medium after validation. The public API boundary is new, but
the implementation keeps final Framework-tier behavior deferred and preserves the
existing Engine compatibility paths.

K03 remains a high-attention area because it touches public trait semantics at
the engine/framework boundary. It is also the critical-chain item before K04, so
hidden coupling here can leak into the next cleanup step.

## Primary Risks

| Risk | Severity | Why it matters |
|---|---:|---|
| Boundary drift between engine `Render` and framework-style build | Critical | K03 must prepare the framework tier without building it inside `flui-core` |
| Incomplete object-safety or RPITIT decision | High | Public trait shape can become hard to migrate once consumers adopt it |
| `RenderOnce` and `Component<C>` breakage | High | Existing examples, widgets, and macro output depend on these paths |
| K02 identity propagation regression | High | `Key`, `ParentElement`, and `IntoElement` must keep stable local/value/global identity behavior |
| K01 provider semantics regression | High | Build/render boundary changes can accidentally allow reads from the wrong lifecycle scope |
| `AnyView::cached` or deferred draw behavior regression | High | Cached render replay is easy to break when separating build and render phases |
| Tier C consumer drift | High | `flui-widgets`, `flui-material`, and `flui-navigator` currently model widgets through `RenderOnce` |
| Macro compatibility drift | Medium | Derive macros may continue compiling while generating outdated trait assumptions |
| Documentation drift | Medium | Existing docs can still describe `Render::render`/`RenderOnce::render` as final widget build semantics |
| Scope creep into full Framework tier | Medium | K03 should not implement reconciliation, dirty-list scheduling, `setState`, or the complete Widget API |

## QA Recommendation

Before implementation begins, treat the K03 plan as a blocking contract. The next quality gate should confirm that the spec states the `Render` versus build boundary, names any new public API precisely, explains object-safety tradeoffs, and lists migration expectations for Tier C consumers.

After implementation lands, rerun this QA package against the actual diff and expand the execution evidence to include workspace checks, Tier C compile coverage, macro coverage, example coverage, and docs/migration review.
