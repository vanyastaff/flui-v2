# Change Summary: K03 Render to Build Separation

## Snapshot

- Branch: `feature/k03-render-build-separation`
- Compared against: `main`
- Date: 2026-05-11
- Pipeline mode: full QA package
- Implementation status: pre-implementation
- Commits ahead of `main`: 0
- Committed files changed versus `main`: 0
- Relevant working-tree artifact: `.ai-factory/plans/feature-k03-render-build-separation.md`

This branch currently has no committed runtime, macro, documentation, or test changes relative to `main`. The QA scope is therefore a pre-implementation gate for the K03 implementation plan, not a verification of completed code.

Unrelated local working-tree changes and AI context files are out of scope unless they are intentionally staged into the K03 work later.

## What Changed

The relevant K03 artifact is an implementation plan for separating engine-level `Render` behavior from future framework-level pure build semantics. The plan was refined after a second pass to include 29 tasks and explicit coverage for Tier C consumer crates, object-safety and RPITIT decisions, workspace dependency boundaries, macro compatibility, identity propagation, provider semantics, and documentation/migration updates.

## Affected Areas

| Area | Current change | Expected K03 impact |
|---|---:|---|
| `.ai-factory/plans/feature-k03-render-build-separation.md` | Added/refined | Drives implementation order and acceptance gates |
| `crates/flui-core/src/element.rs` | Planned | Defines or preserves `Render`, `RenderOnce`, `IntoElement`, `Component`, and build boundary behavior |
| `crates/flui-core/src/view.rs` | Planned | Keeps `AnyView` render, cache, and provider replay behavior correct |
| `crates/flui-core/src/window.rs` | Planned | Keeps root mounting and draw traversal compatible |
| `crates/flui-macros` | Planned | Keeps derive-generated render and element code source-compatible |
| `crates/flui-widgets`, `crates/flui-material`, `crates/flui-navigator` | Planned | Ensures Tier C consumers either compile unchanged or have an intentional migration path |
| `docs/superpowers/*` and `.ai-factory/*` | Planned | Captures spec, migration, roadmap, and research updates |

## Risk Level

Overall planned implementation risk: High.

Current plan-only branch risk: Low for runtime behavior, because no implementation has landed yet.

The future K03 implementation is high risk because it touches public trait semantics at the engine/framework boundary. It is also the critical-chain item before K04, so hidden coupling here can leak into the next cleanup step.

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
