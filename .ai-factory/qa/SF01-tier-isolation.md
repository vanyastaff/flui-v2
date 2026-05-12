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

## Output captured 2026-05-12

```
flui-framework v0.1.0 (crates/flui-framework)
└── flui-core v0.1.0 (crates/flui-core)
    [build-dependencies]
```

## Verdict

✅ **Tier isolation holds.** Only `flui-core` appears in `flui-framework`'s direct dependency graph (plus transitive deps inherited from `flui-core` itself). No Tier C sibling crates are pulled in.

Notes:

- SF01 does NOT add `flui-macros` as a regular dependency yet. That addition is gated on **T4.4** (re-export of `derive(Widget)` for ergonomic consumer use). After T4.4, the expected graph is `flui-framework → { flui-core, flui-macros }` — both Tier A, still no Tier C.
- `flui-platform` is not pulled in because `flui-core`'s public surface used by `flui-framework` does not require platform types. If a future SF spec needs platform types, the dep adds as a Tier A → Tier A migration, no tier violation.
- `trybuild` dev-dep lands in T4.3 (`crates/flui-framework/[dev-dependencies]`) — dev-deps do not violate runtime tier rules.

## Re-run procedure

Reviewer agents can re-run the command above from the workspace root to verify the dependency graph has not regressed in subsequent commits. Expected output post-T4.4 (when `flui-macros` joins):

```
flui-framework v0.1.0 (crates/flui-framework)
├── flui-core v0.1.0 (crates/flui-core)
│   [build-dependencies]
└── flui-macros v0.1.0 (crates/flui-macros) (proc-macro)
```

If any Tier C crate (`flui-widgets`, `flui-material`, etc.) ever appears in this graph, that is a tier-boundary violation and MUST be reverted before the change lands.
