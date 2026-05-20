# v1 Crate Compile-Status Audit

**Status:** Filled 2026-05-19 (U2.5) per `docs/plans/2026-05-19-001-feat-v1-port-track-2-egui-easy-plan.md`.

**Purpose:** Classify each v1 module (across `flui-foundation`, `flui-types`, `flui-cli`, `flui-hot-reload`, `flui-devtools`) by port effort. Subsequent units' "mechanical port" framing depends on this audit's findings.

**Method:** Source-level audit via inspection of v1 `Cargo.toml` deps + ripgrep counts of `flui_engine` / `flui_rendering` / `flui_layer` / `flui_view` / `flui_painting` / `flui_log` / `flui_build` substrate references in each module. Plus per-file LoC and external-dep survey. No `cargo check` run on v1 (would need to repoint workspace; not necessary to validate classification).

**v1 source:** `<v1-root>/crates/`, where `<v1-root>` is your local checkout of the v1 flui repository. Maintainer's example: `C:\Users\vanya\RustroverProjects\flui`. Read-only reference — never write into v1 from this workspace. To obtain the v1 source: clone (or symlink from a prior checkout) the v1 repo to any local path; export `FLUI_V1_ROOT` or substitute `<v1-root>` accordingly when following audit steps below.

## Classification

| Status | Meaning | Action in unit |
|---|---|---|
| **MECHANICAL** | Module compiles cleanly against Rust 1.95 + workspace deps with at most cosmetic changes. Tests (if present in v1) pass. | Copy + minor modernize (let-chains, OnceLock, etc.). |
| **REPAIR** | Module compiles with localized fixes (rename collisions, swap `flui-log` → `tracing`, swap `std::process::Command` → `smol::process::Command`, etc.). Semantics preserved. | Port + apply mechanical fixes. |
| **REWRITE** | Module depends on v1 substrate that doesn't exist in v2 (`flui-engine`, `flui-rendering`, `flui-layer`, `flui-view`, `flui_log`, `pollster`-tokio integration). Cannot be ported mechanically. | Drop or redesign from scratch. Discard v1 file as reference-only. |
| **DROP** | Module is dead-on-arrival (commented out in v1 `lib.rs`, untested stub, RenderObject-dependent, or duplicates a v2 module). | Skip entirely. |

## flui-foundation modules

**Cargo.toml deps:** `bitflags`, `dashmap`, `parking_lot`, `thiserror`, `tracing`, optional `serde`/`serde_json`. **No** dep on `flui-engine`/`flui-rendering`/`flui-layer`/`flui-view` — self-contained.

**Substrate refs in src/:** All three matches (`key.rs:19`, `lib.rs:13`, `notifier.rs:26`) are **doc comments referencing other v1 crates**, not real `use`/`extern` statements. Verified clean.

| Module | LoC | Substrate? | Status | Notes |
|---|---:|:---:|---|---|
| `assert` | 401 | no | **MECHANICAL** | Assertion macros; `FluiError` re-export. Direct port. |
| `binding` | 272 | no | **MECHANICAL** | `BindingBase` / `HasInstance` pattern. Direct port. |
| `callbacks` | 263 | no | **MECHANICAL** | `ValueChanged`, `ValueGetter`, etc. Direct port. |
| `consts` | 185 | no | **REPAIR** | Includes `IS_DESKTOP`/`IS_MOBILE`/`IS_WEB` + EPSILON constants. Strip mobile/web flags (Phase III scope); keep desktop + numeric constants. |
| `debug` | 1065 | no | **MECHANICAL** | `Diagnostics{Builder,Node,Property,TreeStyle}` Flutter-shape. Heaviest single module. Direct port — overlap with `flui_core` debug machinery is name-only. |
| `error` | 335 | no | **MECHANICAL** | `FluiError` / `FoundationError` via `thiserror`. Direct port. |
| `id` | 879 | no | **MECHANICAL** | `Id<T: Marker>` generic + ~30 ID newtypes (`AnimationId`, `ElementId`, `FocusId`, `LayerId`, …). Keep all — they're cheap. |
| `key` | 841 | no | **REPAIR** | **Collision:** v1's `Key`/`ValueKey` vs v2's `flui_core::element::identity::Key`/`ValueKey`. Resolution: keep scoped under `flui_foundation::key::Key`; downstream users disambiguate via qualified path. Document collision in module rustdoc + `lib.rs` re-export. |
| `notifier` | 819 | no | **MECHANICAL** | `ChangeNotifier`, `Listenable`, `ValueListenable`, `ValueNotifier`, `MergedListenable`. Direct port. |
| `observer` | 520 | no | **MECHANICAL** | `ObserverList`, `HashedObserverList`, `SyncObserverList`. Direct port. |
| `platform` | 203 | no | **DROP** | `TargetPlatform` enum + checks. Duplicates `flui_core::platform`. Drop. |
| `wasm` | 67 | no | **DROP** | `WasmNotSend`/`WasmNotSendSync` marker traits. Phase III scope; drop. |

**Net flui-foundation port LoC:** ~5400 (sum of MECHANICAL + REPAIR; ~5910 if `debug.rs` and `id.rs` survive in full).

## flui-types modules

**Cargo.toml deps:** `tracing`, `num-traits = "0.2"`, `thiserror = "2.0"`, optional `serde`, `smallvec`, optional `mint = "0.5"`, optional `glam = "0.30"`. `[dev-dependencies]`: `criterion`, `tokio`, `proptest`. **No** runtime substrate deps.

**Substrate refs in src/:** Empty grep — fully self-contained.

**Note:** `tokio` is only a `[dev-dependencies]` entry used for v1's `proptest` async tests. For v2 port, swap to `smol = "2"` workspace-aligned in dev-deps.

### geometry/

| Module | Net-new vs v2? | Status | Notes |
|---|:---:|---|---|
| `bezier` | net-new | MECHANICAL | Cubic/quadratic Bezier curves. |
| `bounds` | overlaps `flui_core::geometry::Bounds<T>` | **DROP** | Use core's. |
| `circle` | net-new | MECHANICAL | |
| `corner` | overlaps `flui_core::geometry::Corner` | **DROP** | |
| `corners` | overlaps `flui_core::geometry::Corners<T>` | **DROP** | |
| `edges` | overlaps `flui_core::geometry::Edges<T>` | **DROP** | |
| `error` | net-new | MECHANICAL | Just `thiserror`-derived error type for geometry ops. |
| `length` | overlaps `flui_core::geometry::Length` | **DROP** | |
| `line` | net-new | MECHANICAL | 2D line segment. |
| `matrix4` | net-new (v2 only has `Affine2`) | MECHANICAL | Full 4×4 matrix. |
| `mod` | n/a | REPAIR | Module re-exports — drop entries for DROP modules. |
| `offset` | net-new (v2 has `Point<T>`) | MECHANICAL | Flutter-style `Offset` vector. |
| `point` | overlaps `flui_core::geometry::Point<T>` | **DROP** | |
| `rect` | net-new (distinct from v2 `Bounds<T>`) | MECHANICAL | Axis-aligned `Rect` w/ Flutter semantics. |
| `relative_rect` | net-new | MECHANICAL | |
| `rotation` | net-new | MECHANICAL | |
| `rrect` | net-new | MECHANICAL | Rounded rect. |
| `rsuperellipse` | net-new | MECHANICAL | Squircle / superellipse. |
| `size` | overlaps `flui_core::geometry::Size<T>` | **DROP** | |
| `text_path` | net-new | MECHANICAL | Text-along-path geometry. |
| `traits` | mixed | REPAIR | Trait re-exports — keep traits whose impl modules survive; drop the rest. |
| `transform` | net-new | MECHANICAL | |
| `transform2d` | net-new | MECHANICAL | |
| `units` | overlaps Pixels family | **DROP** | |
| `vector` | net-new | MECHANICAL | 2D/3D vector ops. |

### styling/

| Module | Net-new? | Status |
|---|:---:|---|
| `border` | net-new | MECHANICAL |
| `border_radius` | net-new | MECHANICAL |
| `box_border` | net-new | MECHANICAL |
| `color` | overlaps `flui_core::color::Rgba`/`Hsla` | **DROP** |
| `color32` | net-new (4-byte RGBA8; v2 has 16-byte f32 only) | MECHANICAL |
| `decoration` | net-new | MECHANICAL |
| `gradient` | net-new | MECHANICAL |
| `hsl_hsv` | net-new (v2 only has `Hsla`) | MECHANICAL |
| `material_colors` | net-new | MECHANICAL |
| `mod` | n/a | REPAIR (drop entries for DROP modules) |
| `physical_model` | net-new | MECHANICAL |
| `shadow` | net-new | MECHANICAL |

**Net flui-types port effort:** ~16 geometry + ~10 styling = ~26 net-new modules MECHANICAL. ~7 DROP via overlap. Re-export cleanup in `mod.rs` + `lib.rs` = REPAIR.

## flui-cli modules

**Cargo.toml deps:** `clap` 4.5, `clap_complete` 4.5, `cliclack` 0.3.6, `console` 0.15, `which` 8.0, `dirs` 5.0, `notify-debouncer-mini` 0.5, `toml` 0.9, `serde`, `serde_json`, `thiserror`, `tracing`, `pollster`. **Path deps:** `flui-build` (mandatory), `flui-log` (mandatory), `flui-devtools` (optional behind `devtools` feature).

**Substrate ref counts in src/:**
- `std::process::Command` — 5 sites (must rewrite to `smol::process::Command` per `clippy.toml:17-22`).
- `pollster::` — 16 sites (must rewrite to `smol::block_on`).
- `flui_log::` — 3 sites (must rewrite to `tracing` + `tracing-subscriber`).
- `flui_build::` — 7 sites (must rewrite to direct `runner::CargoCommand::*` per origin Scope Boundary).

### Per-command classification

| Command | LoC | Substrate weight | Status | Notes |
|---|---:|:---:|---|---|
| `analyze` | 40 | light | **REPAIR** | Calls `cargo clippy`; minimal flui-build wrapper. |
| `build` | 618 | heavy | **REWRITE** | 70%+ orchestration via `flui_build::{AndroidBuilder,DesktopBuilder,WebBuilder,ProgressManager}`. Strip mobile/web; remaining desktop logic is a thin `cargo build` wrapper. ~5% of LoC survives. |
| `clean` | 96 | none | **MECHANICAL** | Thin `cargo clean` wrapper. |
| `completions` | 107 | none | **MECHANICAL** | `clap_complete::generate` call. |
| `create` | 169 | light | **REPAIR** | Template generation; strip mobile templates. |
| `create_interactive` | 107 | none | **MECHANICAL** | `cliclack` prompts. |
| `devices` | 409 | n/a | **DROP** | Mobile-only (Phase III). |
| `devtools` | 100 | n/a | **DROP** | DevTools UI launcher; UI not in scope. |
| `doctor` | 406 | medium | **REWRITE** | ~150 LoC mobile-toolchain checks (Android SDK, Xcode, Java); strip leaves ~250 LoC desktop checks. Still substantial after strip. |
| `emulators` | 619 | n/a | **DROP** | Mobile-only. |
| `format` | 40 | none | **MECHANICAL** | Thin `cargo fmt` wrapper. |
| `mod` | 21 | n/a | REPAIR | Drops entries for DROP commands. |
| `platform` | 249 | n/a | **DROP** | Phase III platform-specific. |
| `run` | 490 | heavy | **REWRITE** | Heavy Android/iOS branches; strip leaves desktop-only `cargo run` wrapper. ~10% survives. |
| `test` | 56 | none | **MECHANICAL** | Thin `cargo test` wrapper. |
| `upgrade` | 53 | light | **REPAIR** | Rustup hint generator. |

### Per-support-module classification

| Module | LoC (approx) | Status | Notes |
|---|---:|---|---|
| `main.rs` | 659 | REPAIR | Clap structure + subcommand dispatch; replace `flui_log::Logger::new().init()` with `tracing_subscriber::fmt().init()`; strip mobile/web subcommand variants. |
| `runner.rs` | ~150 | REPAIR | Replace every `std::process::Command::*` with `smol::process::Command::*`; replace `pollster::block_on` with `smol::block_on`. |
| `error.rs` | small | MECHANICAL | `CliError` `#[non_exhaustive]` + `OptionExt`/`ResultExt`. |
| `types.rs` | small | REPAIR | Add `ProjectPath` traversal validation per security review. |
| `config.rs` | small | MECHANICAL | `flui.toml` parsing. |
| `templates/` | ~200+ | REPAIR | `TemplateBuilder`; strip mobile templates (basic, counter only for v1 brainstorm). |
| `utils.rs` | small | MECHANICAL | |
| `prelude.rs` | inline in main | MECHANICAL | |

## flui-hot-reload modules

**Cargo.toml deps:** `flui-layer` (mandatory), optional `flui-view + flui-rendering + flui-types` behind `app-plugin` feature. Cross-platform: `libc`/`windows`/`android_log-sys`.

**Substrate ref counts:**
- `dynlib.rs` (0 refs) — **MECHANICAL**. Cross-platform FFI primitives (`dlopen`/`LoadLibraryW`). Port directly OR replace with `libloading = "0.8"` crate (cleaner).
- `driver.rs` (1 real ref at `:35`: `use flui_layer::Scene;`) — **REWRITE**. mtime-poll loop shape is reusable; Scene-dep needs rework.
- `host.rs` (1 real ref at `:20`: `use flui_layer::Scene;`) — **REWRITE**. Same.
- `lib.rs` (refs at `:14`, `:29` are doc comments only) — **REPAIR**. Macros `scene_plugin!`/`app_plugin!` need adapter per U11 outcome.
- `plugin.rs` (4 substrate refs) — **REWRITE**. FFI export macros tied to v1 `build_scene -> Scene` shape; rework к U11 mechanism's plugin shape.
- `pipeline.rs` (4 substrate refs) — **DROP**. Uses `flui_rendering::pipeline::PipelineOwner`; v2 has no equivalent.

**All hot-reload work is contingent on U11 research outcome** (subsecond / hot-lib-reloader / custom dynlib). If `subsecond` selected: most v1 files become reference-only because semantic-patching has no FFI boundary. If `hot-lib-reloader`: only dynlib.rs survives as reference. If custom dynlib: dynlib.rs is the highest-reuse candidate.

## flui-devtools modules

**Cargo.toml deps:** `web-time`, `serde`, `serde_json`, optional `notify`; Windows-specific `windows-sys` 0.59 for memory APIs. **Path deps:** `flui-engine` (mandatory) — BUT grep for `flui_engine::` in src/ returns **zero matches**. Dep is aspirational / unused; can safely drop. The flui-devtools port to v2 will omit `flui-core` as a runtime dep at the protocol level (FrameProfile telemetry flows via the K22 substrate added in U8, not via a direct flui-core dep on flui-devtools).

| Module | LoC | Status | Notes |
|---|---:|---|---|
| `common.rs` | 90 | **MECHANICAL** | `DevToolsConfig`, `FrameNumber`, `Timestamp`, `DurationNanos`. |
| `hot_reload.rs` | 502 | **REPAIR** | File-watch + reload-event types via `notify`. Bridge to `flui-hot-reload` crate. Rename if collision with hot-reload's hot_reload module (use `hot_reload_bridge.rs`). |
| `lib.rs` | 150 | **REPAIR** | Module declarations + feature gates. Reconcile v1's `FramePhase` (3 variants) with v2's `flui_core::frame::FramePhase` (8 variants). |
| `memory.rs` | 292 | **MECHANICAL** | Memory profiling via `web_time::Instant` + `serde`. Clean external deps. NOT stubbed — full impl. |
| `network.rs` | 183 | **MECHANICAL** | HTTP request/response tracking; std deps only. NOT stubbed — full impl. |
| `profiler.rs` | 609 | **REPAIR** | Replace v1 `FramePhase` 3-variants with v2's 8-variant via `flui_core::frame::FramePhase`; subscribe to v2 `FrameProfile`. |
| `remote.rs` | 123 | **REPAIR** | WebSocket server skeleton; expand to full Flutter VM Service per origin R19. NOT a stub but minimal — needs U9 protocol layer wrapping it. |
| `timeline.rs` | 610 | **REPAIR** | Chrome trace JSON export; `EventCategory` mapped к v2 `FramePhase`. |

**Correction vs initial table:** `memory.rs`, `network.rs`, `remote.rs` are NOT stubs as initially classified — they're real impls. The "v1 `memory.rs`/`network.rs`/`remote.rs` were stubs" line in `crates/flui-devtools/src/lib.rs` was inaccurate and should be corrected in U9.

## Findings summary

| Status | Count (approx) | Notes |
|---|---:|---|
| MECHANICAL | ~30 modules | Heaviest concentrations: flui-foundation (8 modules) + flui-types geometry net-new (16 modules) + flui-types styling net-new (10 modules) + flui-devtools common/memory/network (3). |
| REPAIR | ~17 modules | Mostly flui-cli (8 commands/support) + flui-devtools/profiler/timeline/hot_reload/remote/lib (5) + flui-foundation key/consts (2) + flui-types mod/traits (2). |
| REWRITE | ~7 modules | flui-cli build/run/doctor (3) + flui-hot-reload driver/host/plugin (3) + 1 misc. |
| DROP | ~17 modules | flui-foundation platform/wasm (2) + flui-types overlap (7) + flui-cli mobile/devtools-ui (4) + flui-hot-reload pipeline (1) + a few mod.rs entries. |

**Aggregate port effort estimate update vs initial plan:**

- **flui-foundation port (U3):** ~5400 LoC across 8 MECHANICAL + 2 REPAIR modules. Mostly mechanical; key.rs collision is the main repair. **Confirmed feasible as single PR.**
- **flui-types port (U4):** ~26 net-new modules MECHANICAL + DROP cleanup in mod.rs. **Confirmed feasible as single PR.**
- **flui-cli port (U5/U6/U7):** ~6 MECHANICAL commands + 5 REPAIR + 3 REWRITE. Doctor + build + run are the heavy rewrites (each substantial). May need split into multiple PRs (U5 scaffolding+thin-wrappers first, then U6 heavy commands separately).
- **flui-devtools port (U9/U10):** Lighter than expected — memory/network/remote already implemented (just need VM Service protocol layer above them). U9 protocol layer = new code; U10 profiler/timeline = REPAIR.
- **flui-hot-reload port (U12/U13):** Most of v1 source becomes reference-only depending on U11 outcome. Effort estimate cannot solidify until U11 lands.

## Conducting follow-up checks

1. Clone or symlink v1 source to a local path of your choice (e.g., `~/flui-v1` on Linux/macOS, `C:\flui-v1` on Windows; maintainer's path is `C:\Users\vanya\RustroverProjects\flui`). Export `FLUI_V1_ROOT` to that path and substitute for `<v1-root>` in commands below.
2. For modules classified MECHANICAL: copy + apply Rust 1.95 modernization (let-chains, OnceLock, edition-2024 lifetime captures). Audit row stays MECHANICAL.
3. For modules classified REPAIR: apply listed fixes (rename, smol-swap, tracing-swap, drop mobile branches) and re-verify compile.
4. For modules classified REWRITE: discard v1 impl as reference-only; redesign over v2 substrates (`flui_core::frame::FramePhase`, `App::defer_to`, smol runtime, etc).
5. For modules classified DROP: do not port. Update relevant `mod.rs` / `lib.rs` re-exports accordingly.

The audit informs U3/U4/U6/U10/U12 effort estimates. Subsequent units' "mechanical port" framing is validated by this audit: most foundation/types modules ARE mechanical; CLI build/run/doctor are NOT (revise expectations there per U6's reframe in the plan).
