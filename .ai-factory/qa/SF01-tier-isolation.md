# SF01 Tier Isolation Verification

**Date:** 2026-05-12
**Spec:** `docs/superpowers/specs/2026-05-12-SF01-widget-key-trait-design.md`
**Plan:** `.ai-factory/plans/feature-SF01-widget-key-trait.md` — task T1.3

## Purpose

`flui-framework` is the new Tier B crate created by SF01. Per the project's three-tier architecture (`.ai-factory/ARCHITECTURE.md`), Tier B may depend on Tier A only:

| Tier | Crate(s) | May depend on |
|---|---|---|
| B — Framework | `flui-framework` | Tier A only |
| A — Engine | `flui-core`, `flui-platform`, `flui-macros` | Tier A only |

Tier C sibling crates (`flui-widgets`, `flui-material`, `flui-cupertino`, `flui-theme`, `flui-a11y`, `flui-navigator`) MUST NOT appear in `flui-framework`'s dependency graph in SF01.

## Verification command

```bash
cargo tree -p flui-framework --depth 1
```

## Output captured 2026-05-12 (post-T4.4 + T4.3 final state)

```
flui-framework v0.1.0 (crates/flui-framework)
├── flui-core v0.1.0 (crates/flui-core)
│   [build-dependencies]
└── flui-macros v0.1.0 (proc-macro) (crates/flui-macros)
[dev-dependencies]
└── trybuild v1.0.116
```

## Verdict

✅ **Tier isolation holds.** Only Tier A crates (`flui-core`, `flui-macros`) appear in `flui-framework`'s direct runtime dependency graph. `trybuild` is a dev-dependency (third-party crate, no tier implications). No Tier C sibling crates are pulled in.

Notes:

- `flui-macros` is a regular (not dev) dependency because the `pub use flui_macros::Widget;` proc-macro re-export at `crates/flui-framework/src/lib.rs` is part of the public API surface — consumers need `flui-macros` linked transitively for `#[derive(Widget)]` to resolve.
- `flui-platform` is not pulled in because `flui-core`'s public surface used by `flui-framework` does not require platform types. If a future SF spec needs platform types, the dep adds as a Tier A → Tier A migration, no tier violation.
- `trybuild` dev-dep was introduced by T4.3 for derive-macro compile-pass / compile-fail snapshot tests. Dev-deps do not violate runtime tier rules.

## Re-run procedure

Reviewer agents can re-run the command above from the workspace root to verify the dependency graph has not regressed in subsequent commits. The output above is the canonical baseline.

If any Tier C crate (`flui-widgets`, `flui-material`, `flui-navigator`, `flui-theme`, `flui-a11y`, `flui-cupertino`) ever appears in this graph as a regular dep, that is a tier-boundary violation and MUST be reverted before the change lands.
