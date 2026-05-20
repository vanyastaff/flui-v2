# v1 Crate Compile-Status Audit

**Status:** TODO — runs before U3/U4 implementation per `docs/plans/2026-05-19-001-feat-v1-port-track-2-egui-easy-plan.md` U2.

**Purpose:** Classify each v1 module (across `flui-foundation`, `flui-types`, `flui-cli`, `flui-hot-reload`, `flui-devtools`) by port effort. Subsequent units' "mechanical port" framing depends on this audit's findings.

**v1 source:** `C:\Users\vanya\RustroverProjects\flui\crates\` (maintainer's machine; read-only reference).

## Classification

| Status | Meaning | Action in unit |
|---|---|---|
| **MECHANICAL** | Module compiles cleanly against Rust 1.95 + workspace deps with at most cosmetic changes. Tests (if present in v1) pass. | Copy + minor modernize (let-chains, OnceLock, etc.). |
| **REPAIR** | Module compiles with localized fixes (rename collisions, swap `flui-log` → `tracing`, swap `std::process::Command` → `smol::process::Command`, etc.). Semantics preserved. | Port + apply mechanical fixes. |
| **REWRITE** | Module depends on v1 substrate that doesn't exist in v2 (`flui-engine`, `flui-rendering`, `flui-layer`, `flui-view`, `flui_log`, `pollster`-tokio integration). Cannot be ported mechanically. | Drop or redesign from scratch. Discard v1 file as reference-only. |
| **DROP** | Module is dead-on-arrival (commented out in v1 `lib.rs`, untested stub, RenderObject-dependent). | Skip entirely. |

## flui-foundation modules

| Module | v1 path | Compiles on 1.95? | Deps on v1 substrate? | Status | Notes |
|---|---|---|---|---|---|
| `assert` | `flui-foundation/src/assert.rs` | TBD | TBD | TBD | |
| `binding` | `flui-foundation/src/binding.rs` | TBD | TBD | TBD | |
| `callbacks` | `flui-foundation/src/callbacks.rs` | TBD | TBD | TBD | |
| `consts` | `flui-foundation/src/consts.rs` | TBD | TBD | TBD | Includes `IS_DESKTOP`/`IS_MOBILE`/`IS_WEB` — drop mobile/web if Phase III-only. |
| `debug` | `flui-foundation/src/debug.rs` | TBD | TBD | TBD | Diagnostics builder shape — may collide with `flui_core` debug machinery. |
| `error` | `flui-foundation/src/error.rs` | TBD | TBD | TBD | |
| `id` | `flui-foundation/src/id.rs` | TBD | TBD | TBD | 30+ ID types — keep only those with consumers in this port. |
| `key` | `flui-foundation/src/key.rs` | TBD | TBD | TBD | **Collision**: v1's `Key` vs v2's `flui_core::element::identity::Key` — scope foundation's as `foundation::key::Key`. |
| `notifier` | `flui-foundation/src/notifier.rs` | TBD | TBD | TBD | |
| `observer` | `flui-foundation/src/observer.rs` | TBD | TBD | TBD | |
| `platform` | `flui-foundation/src/platform.rs` | TBD | TBD | TBD | Likely DROP (duplicates `flui-core::platform`). |
| `wasm` | `flui-foundation/src/wasm.rs` | TBD | TBD | TBD | Phase III scope — likely DROP. |

## flui-types modules

| Module | v1 path | Overlap with v2? | Status |
|---|---|---|---|
| `geometry/bezier` | `flui-types/src/geometry/bezier.rs` | Net-new | TBD |
| `geometry/bounds` | `flui-types/src/geometry/bounds.rs` | Overlaps `flui-core::geometry::Bounds<T>` | DROP (use core's). |
| `geometry/circle` | `flui-types/src/geometry/circle.rs` | Net-new | TBD |
| `geometry/corner` | `flui-types/src/geometry/corner.rs` | Overlaps `flui-core::geometry::Corner` | DROP. |
| `geometry/corners` | `flui-types/src/geometry/corners.rs` | Overlaps `flui-core::geometry::Corners<T>` | DROP. |
| `geometry/edges` | `flui-types/src/geometry/edges.rs` | Overlaps `flui-core::geometry::Edges<T>` | DROP. |
| `geometry/length` | `flui-types/src/geometry/length.rs` | Overlaps `flui-core::geometry::Length` | DROP. |
| `geometry/line` | `flui-types/src/geometry/line.rs` | Net-new | TBD |
| `geometry/matrix4` | `flui-types/src/geometry/matrix4.rs` | Net-new (v2 has only `Affine2`) | TBD |
| `geometry/offset` | `flui-types/src/geometry/offset.rs` | Net-new (v2 has `Point<T>`) | TBD |
| `geometry/point` | `flui-types/src/geometry/point.rs` | Overlaps `flui-core::geometry::Point<T>` | DROP. |
| `geometry/rect` | `flui-types/src/geometry/rect.rs` | Net-new (distinct from v2's `Bounds<T>`) | TBD |
| `geometry/relative_rect` | `flui-types/src/geometry/relative_rect.rs` | Net-new | TBD |
| `geometry/rotation` | `flui-types/src/geometry/rotation.rs` | TBD | TBD |
| `geometry/rrect` | `flui-types/src/geometry/rrect.rs` | Net-new | TBD |
| `geometry/rsuperellipse` | `flui-types/src/geometry/rsuperellipse.rs` | Net-new | TBD |
| `geometry/size` | `flui-types/src/geometry/size.rs` | Overlaps `flui-core::geometry::Size<T>` | DROP. |
| `geometry/text_path` | `flui-types/src/geometry/text_path.rs` | Net-new | TBD |
| `geometry/transform` | `flui-types/src/geometry/transform.rs` | Net-new | TBD |
| `geometry/transform2d` | `flui-types/src/geometry/transform2d.rs` | Net-new | TBD |
| `geometry/units` | `flui-types/src/geometry/units.rs` | Overlaps Pixels family | DROP. |
| `geometry/vector` | `flui-types/src/geometry/vector.rs` | Net-new | TBD |
| `styling/border` | `flui-types/src/styling/border.rs` | Net-new | TBD |
| `styling/border_radius` | `flui-types/src/styling/border_radius.rs` | Net-new | TBD |
| `styling/box_border` | `flui-types/src/styling/box_border.rs` | Net-new | TBD |
| `styling/color` | `flui-types/src/styling/color.rs` | Overlaps `flui-core::color` | DROP (use core's). |
| `styling/color32` | `flui-types/src/styling/color32.rs` | Net-new (4-byte RGBA8; v2's `Rgba` is 16-byte f32) | TBD |
| `styling/decoration` | `flui-types/src/styling/decoration.rs` | Net-new | TBD |
| `styling/gradient` | `flui-types/src/styling/gradient.rs` | Net-new | TBD |
| `styling/hsl_hsv` | `flui-types/src/styling/hsl_hsv.rs` | Net-new (v2 has only `Hsla`) | TBD |
| `styling/material_colors` | `flui-types/src/styling/material_colors.rs` | Net-new | TBD |
| `styling/physical_model` | `flui-types/src/styling/physical_model.rs` | Net-new | TBD |
| `styling/shadow` | `flui-types/src/styling/shadow.rs` | Net-new | TBD |

## flui-cli commands

| Command | v1 path | Depends on `flui-build`? | Mobile/web? | Status |
|---|---|---|---|---|
| `create` | `flui-cli/src/commands/create.rs` | No (templates inline) | Mobile templates strip | REPAIR |
| `create_interactive` | `flui-cli/src/commands/create_interactive.rs` | No | No | REPAIR |
| `run` | `flui-cli/src/commands/run.rs` | Yes (heavy) | Yes (Android/iOS branches) | REWRITE (5% of v1 LoC survives strip) |
| `build` | `flui-cli/src/commands/build.rs` | Yes (heavy) | Yes (4 branches: desktop/android/ios/web) | REWRITE |
| `test` | `flui-cli/src/commands/test.rs` | No (thin) | No | MECHANICAL |
| `clean` | `flui-cli/src/commands/clean.rs` | No (thin) | No | MECHANICAL |
| `doctor` | `flui-cli/src/commands/doctor.rs` | No | Yes (Android/iOS toolchain checks) | REWRITE (mobile checks strip) |
| `completions` | `flui-cli/src/commands/completions.rs` | No | No | MECHANICAL |
| `format` | `flui-cli/src/commands/format.rs` | No | No | MECHANICAL |
| `analyze` | `flui-cli/src/commands/analyze.rs` | Yes (light) | No | REPAIR |
| `upgrade` | `flui-cli/src/commands/upgrade.rs` | No | No | REPAIR |
| `devices` | `flui-cli/src/commands/devices.rs` | — | Yes (mobile only) | DROP (Phase III) |
| `emulators` | `flui-cli/src/commands/emulators.rs` | — | Yes (mobile only) | DROP (Phase III) |
| `platform` | `flui-cli/src/commands/platform.rs` | — | Yes (Phase III platform-specific) | DROP (Phase III) |
| `devtools` | `flui-cli/src/commands/devtools.rs` | — | — | DROP (devtools UI not in scope) |

## flui-hot-reload modules

All modules depend on v1's `flui-layer::Scene` + `flui-view::WidgetsBinding` substrates (discarded). **ALL = REWRITE.** v1 files are reference-only; actual implementation depends on U11 mechanism decision.

| Module | v1 path | Status | Reference value |
|---|---|---|---|
| `dynlib` | `flui-hot-reload/src/dynlib.rs` | REWRITE | High (FFI primitive shape; cross-platform handling). |
| `driver` | `flui-hot-reload/src/driver.rs` | REWRITE | High (mtime-poll loop shape). |
| `host` | `flui-hot-reload/src/host.rs` | REWRITE | Medium (loader; Scene-handle returns invalidated). |
| `plugin` | `flui-hot-reload/src/plugin.rs` | REWRITE | Low (FFI export macros tied to `build_scene` shape). |
| `pipeline` | `flui-hot-reload/src/pipeline.rs` | DROP | None (depends on `flui_rendering::PipelineOwner`). |

## flui-devtools modules

| Module | v1 path | Status |
|---|---|---|
| `common` | `flui-devtools/src/common.rs` | MECHANICAL (DevToolsConfig, FrameNumber, Timestamp, DurationNanos) |
| `profiler` | `flui-devtools/src/profiler.rs` | REPAIR (replace v1 `FramePhase` 3-variants with v2's 8-variant via `flui_core::frame::FramePhase`; subscribe to v2 `FrameProfile`) |
| `timeline` | `flui-devtools/src/timeline.rs` | REPAIR (Chrome trace JSON export; `EventCategory` mapped к v2 `FramePhase`) |
| `hot_reload` | `flui-devtools/src/hot_reload.rs` | REPAIR (devtools↔hot-reload bridge; rename if collides with `flui-hot-reload` crate) |
| `memory` | `flui-devtools/src/memory.rs` | DROP (v1 stub — replaced by VM Service protocol layer) |
| `network` | `flui-devtools/src/network.rs` | DROP (v1 stub) |
| `remote` | `flui-devtools/src/remote.rs` | DROP (v1 stub — replaced by VM Service protocol layer) |

## Findings summary

To fill during audit:
- Total MECHANICAL modules: TBD
- Total REPAIR modules: TBD
- Total REWRITE modules: TBD (likely all of flui-hot-reload, build/run/doctor in flui-cli)
- Total DROP modules: TBD (~30% — overlap with v2 + Phase III commands + v1 stubs)
- Aggregate port effort estimate: TBD (revise plan unit time estimates after audit)

## Conducting the audit

1. Clone or symlink v1 source: `C:\Users\vanya\RustroverProjects\flui\crates\`.
2. For each module: `cd <v1-crate> && cargo check --offline` against Rust 1.95 (override `rust-toolchain.toml` locally).
3. Record compile errors + dep failures per module.
4. Cross-check each module against v2 `flui-core` for overlap (use ripgrep).
5. Update this table; commit findings.

The audit informs U3/U4/U6/U10/U12 effort estimates. Subsequent units' "mechanical port" framing requires this audit to validate.
