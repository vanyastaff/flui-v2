# Test Cases: K03 Render to Build Separation

## Preconditions

- Current branch is `feature/k03-render-build-separation`.
- The K03 implementation plan exists at `.ai-factory/plans/feature-k03-render-build-separation.md`.
- No implementation diff is currently committed versus `main`; implementation-specific cases should be executed again after code lands.
- Unrelated local files are excluded from this QA scope unless explicitly added to K03.

## Cases

### TC-K03-001: Plan Identifies K03 Scope

Priority: High

Steps:
1. Open the K03 plan and related spec.
2. Confirm the scope names Render-to-build separation as the target.
3. Confirm the plan does not claim to implement the complete Framework tier.

Expected result:

The plan clearly separates K03 from full Phase II-F work and names deferred framework features.

### TC-K03-002: Public Trait Boundary Is Explicit

Priority: High

Steps:
1. Inspect the final K03 spec and implementation.
2. Identify the role of `Render`, `RenderOnce`, and any build-facing trait.
3. Confirm each trait has a clear ownership and lifecycle purpose.

Expected result:

The public API distinguishes mutable engine render behavior from pure build-facing behavior without ambiguous duplicate concepts.

### TC-K03-003: Object-Safety And RPITIT Decision Is Documented

Priority: High

Steps:
1. Locate the K03 object-safety or erasure decision.
2. Confirm whether the build-facing API uses RPITIT, associated types, boxed erasure, or another strategy.
3. Confirm the decision explains compatibility and migration tradeoffs.

Expected result:

The trait shape is deliberate, documented, and compatible with MSRV 1.95.

### TC-K03-004: Existing Mutable `Render` Root Still Works

Priority: High

Steps:
1. Use a root view that implements `Render`.
2. Mount it through the existing window/root path.
3. Trigger multiple draws that mutate view-owned state through `render(&mut self, ...)`.

Expected result:

Existing mutable root render behavior remains valid, or any breaking change is documented with a migration path.

### TC-K03-005: Existing `RenderOnce` Component Still Works

Priority: High

Steps:
1. Use a presentational component that implements or derives `RenderOnce`.
2. Convert it through `IntoElement`.
3. Render it as a child of another element.

Expected result:

`RenderOnce` remains usable for existing presentational components, or the replacement path is fully documented.

### TC-K03-006: `Component<C>` Preserves Keyed Identity

Priority: High

Steps:
1. Create a `Component<C>` wrapping a `RenderOnce` component.
2. Apply `Component::key`.
3. Place it among siblings with other keyed and unkeyed elements.
4. Reorder siblings across a build/render pass.

Expected result:

The keyed component keeps the same normalized identity semantics introduced by K02.

### TC-K03-007: Pure Build Candidate Does Not Require `&mut self`

Priority: High

Steps:
1. Implement or inspect a value-object style build-facing component.
2. Confirm the build entry point receives immutable self access if the K03 API introduces one.
3. Confirm mutable engine state remains handled by `Render` or a documented state owner.

Expected result:

Pure build semantics are not accidentally modeled as `render(&mut self, ...)`.

### TC-K03-008: Provider Reads Stay Lifecycle-Scoped

Priority: High

Steps:
1. Inspect provider access inside any build-facing context.
2. Confirm provider reads use scoped context objects and do not fall back to global app reads.
3. Verify missing-provider behavior is explicit.

Expected result:

K01 provider semantics remain intact across the build boundary.

### TC-K03-009: Keyed Provider Subtree Survives Build Boundary

Priority: High

Steps:
1. Build a subtree using `Provider::new_keyed(Key::value("theme"), ...)`.
2. Nest a build-facing or `RenderOnce` component inside the provider.
3. Rebuild the parent while preserving the provider key.

Expected result:

Provider identity and dependency replay remain stable across rebuilds.

### TC-K03-010: `AnyView::cached` Behavior Is Preserved

Priority: High

Steps:
1. Use a cached view that renders a child through the K03 boundary.
2. Render once with cache enabled.
3. Trigger a second pass without changing dependencies.
4. Trigger a provider or input change that should invalidate the cache.

Expected result:

The cached view reuses valid cached output and invalidates when dependencies change.

### TC-K03-011: Deferred Draw Behavior Is Preserved

Priority: High

Steps:
1. Use a view that schedules deferred work through the existing window/app escape hatch.
2. Ensure the deferred callback causes a later draw.
3. Confirm build/render separation does not borrow app or view state across the deferred boundary.

Expected result:

Deferred work still schedules safely and does not trigger re-entrancy failures beyond the existing contract.

### TC-K03-012: Root Mounting Remains Compatible

Priority: High

Steps:
1. Inspect root window creation and `WindowHandle<V>` bounds.
2. Confirm the accepted root type remains intentionally `Render`, or the migration to a new root type is documented.
3. Confirm examples still match the final API.

Expected result:

Application root mounting has a clear and compatible API.

### TC-K03-013: Derive Macros Generate Compatible Code

Priority: Medium

Steps:
1. Inspect `derive(Render)` and `derive(IntoElement)` output assumptions.
2. Use representative derived components after K03 changes.
3. Confirm generated code targets the final trait names and method signatures.

Expected result:

Macro-derived components compile against the K03 public API without stale render/build assumptions.

### TC-K03-014: Tier C Consumer Crates Are Covered

Priority: High

Steps:
1. Audit `flui-widgets`, `flui-material`, and `flui-navigator` for `RenderOnce`, `Render`, `IntoElement`, and `Component<C>` usage.
2. Confirm each use is compatible with K03 or has a migration task.
3. Confirm examples relying on those crates are included in final validation.

Expected result:

Tier C consumers are not silently broken by K03.

### TC-K03-015: Workspace Dependency Direction Is Preserved

Priority: Medium

Steps:
1. Inspect `Cargo.toml` workspace membership and crate dependencies after implementation.
2. If a new precursor crate is added, confirm dependency direction follows the tier model.
3. Confirm `flui-core` does not depend upward on framework or ecosystem crates.

Expected result:

The crate graph preserves Tier A engine independence.

### TC-K03-016: Documentation Terminology Is Updated

Priority: Medium

Steps:
1. Search docs and crate comments for descriptions equating `Render::render` or `RenderOnce::render` with final Flutter-style `Widget.build`.
2. Confirm outdated wording is updated or caveated.
3. Confirm new examples use the final K03 terminology.

Expected result:

Docs match the final API and do not mislead framework authors.

### TC-K03-017: Migration Guide Covers Breaking Changes

Priority: High

Steps:
1. Review the K03 migration guide after implementation.
2. Confirm it includes before/after examples for affected `Render`, `RenderOnce`, `IntoElement`, and macro users.
3. Confirm Tier C consumer impact is called out when applicable.

Expected result:

External and internal callers have a concrete path to update code.

### TC-K03-018: Full Framework Scope Is Rejected

Priority: Medium

Steps:
1. Review the K03 diff for reconciliation, dirty-list scheduling, `setState`, complete inherited-widget APIs, or widget catalog work.
2. Compare any such additions against the K03 spec.
3. Mark unexpected framework implementation as out of scope unless explicitly approved in the spec.

Expected result:

K03 remains a focused kernel cleanup step and does not absorb Phase II-F.

### TC-K03-019: Conflicting Trait Names Are Avoided

Priority: Medium

Steps:
1. Search for newly introduced `Widget`, `Build`, or similarly broad trait names.
2. Confirm names do not conflict with planned `flui-framework` concepts.
3. Confirm public names are documented as engine-level, framework-level, or temporary compatibility surfaces.

Expected result:

Naming leaves room for the Framework tier without ambiguous API ownership.

### TC-K03-020: QA Scope Matches Branch Contents

Priority: Medium

Steps:
1. Compare the branch against `main`.
2. Review unstaged and untracked files.
3. Confirm only intentional K03 artifacts and implementation files are included in K03 QA findings.

Expected result:

Unrelated local context files are not treated as implementation changes.
