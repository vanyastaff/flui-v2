# Test Plan: K03 Render to Build Separation

## Overview

- Branch: `feature/k03-render-build-separation`
- Date: 2026-05-11
- Status: pre-implementation QA plan
- Target: K03 Render to Build separation
- Source artifact: `.ai-factory/plans/feature-k03-render-build-separation.md`

This plan defines the QA contract for K03 before code lands. The current branch has no committed implementation diff versus `main`, so execution should begin with plan/spec validation and then be repeated once code changes are present.

## Goals

- Confirm K03 preserves existing engine-level `Render` behavior while introducing or preparing a framework-level pure build boundary.
- Confirm any new build-facing API is intentionally scoped and does not become the full `flui-framework` tier.
- Confirm existing `RenderOnce`, `IntoElement`, `Component<C>`, `AnyView`, root mounting, and macro-generated code remain compatible or have explicit migration paths.
- Confirm K01 provider semantics and K02 identity semantics are preserved across any new build boundary.
- Confirm Tier C consumers are included in compatibility and migration coverage.
- Confirm documentation no longer describes current render methods as the final Flutter-style widget build API unless that wording is intentionally retained with caveats.

## In Scope

- K03 spec completeness and implementation plan quality.
- Trait boundary decisions for `Render`, `RenderOnce`, and any pure build-facing trait.
- Object-safety, RPITIT, and erasure strategy decisions.
- `IntoElement`, `ParentElement`, `Component<C>`, and key propagation behavior.
- `AnyView` render caching, provider dependency replay, and deferred draw behavior.
- Window/root mounting compatibility.
- Macro output compatibility for `derive(Render)` and `derive(IntoElement)`.
- Tier C consumer compatibility across `flui-widgets`, `flui-material`, and `flui-navigator`.
- Migration guide, roadmap, research, and docs updates.

## Out Of Scope

- Full Phase II-F `flui-framework` implementation.
- Stateful widget reconciliation, dirty-list scheduling, `setState`, inherited widget system, and complete Flutter widget lifecycle.
- K04 frame/effect cleanup.
- Platform backend extraction work.
- GPU renderer changes unless K03 unexpectedly touches render backend code.
- Unrelated dirty files in the local working tree.

## Test Data

Use representative cases that cover both current APIs and the planned boundary:

- A mutable root view implementing `Render` directly.
- A presentational component implementing or deriving `RenderOnce`.
- A `Component<C>` with `Component::key` and nested child elements.
- A candidate pure-build value object that can build from `&self` without needing mutable state.
- Repeated children using `ParentElement::child` and `IntoElement::into_any_element`.
- A provider subtree using `Provider::new_keyed(Key::value("theme"), ...)`.
- A cached view using `AnyView::cached`.
- Tier C widgets or routes from `flui-widgets`, `flui-material`, and `flui-navigator`.

## Acceptance Criteria

- K03 scope is explicit: engine render compatibility plus build-boundary preparation, not full Framework-tier delivery.
- Public trait decisions are documented, including object-safety and RPITIT tradeoffs.
- Existing `Render`, `RenderOnce`, `Component<C>`, and `IntoElement` consumers either compile unchanged or are covered by migration docs.
- K01 provider read rules remain lifecycle-scoped and do not become ambient global reads.
- K02 identity and key behavior remain stable through build and element conversion paths.
- Root mounting and `AnyView` cached/deferred draw behavior remain compatible.
- Macro-generated code compiles against the final trait surface.
- Tier C crates are covered by compile checks or explicit migration tasks.
- Docs and migration guides reflect the final public API.
- No unrelated local files are treated as part of the K03 QA scope.

## Coverage Matrix

| Area | Priority | Coverage expectation |
|---|---:|---|
| Spec and plan completeness | High | Reviewed before implementation begins |
| Public API boundary | High | Reviewed against code and docs |
| Existing render compatibility | High | Verified with representative root and component cases |
| Identity and provider behavior | High | Verified across nested, keyed, and cached paths |
| Tier C consumers | High | Verified or explicitly migrated |
| Macro output | Medium | Verified with derive-generated cases |
| Documentation and migration | Medium | Reviewed for stale terminology and missing examples |
| Performance and allocation shape | Medium | Checked for avoidable dynamic dispatch or repeated build work |
| Negative scope checks | Medium | Confirm no full framework features are smuggled into K03 |

## Execution Checklist

- Validate the plan and spec before implementation begins.
- Inspect any implementation diff for public API shape and dependency direction.
- Review representative `Render` and `RenderOnce` consumers.
- Review identity propagation through `IntoElement` and `ParentElement`.
- Review provider access paths around any new build context.
- Review cached and deferred draw paths in `AnyView` and `Window`.
- Review derive macro output against the final trait surface.
- Review Tier C crates for compatibility or migration coverage.
- Review docs, roadmap, research, and migration guide updates.
- Record any failing or deferred items before K03 is considered complete.
