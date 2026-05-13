# A10 — XL-file decomposition (facade-pattern split of `window` / `geometry` / `elements::div`)

**Date:** 2026-05-13
**Phase:** Architecture & API hygiene (cross-cutting)
**Status:** Proposed
**Scope:** `crates/flui-core` — `window.rs`, `geometry.rs`, `elements/div.rs`. API-neutral.
**Policy:** `docs/research/adr/ADR-021-xl-file-split-discipline.md`
**Roadmap:** A10 (sub-tracks A10a / A10b / A10c).

## Summary

`crates/flui-core` накопил несколько монолитных файлов: `window.rs` — 6036 LoC, `geometry.rs` — 3802, `elements/div.rs` — 3673. Внутри каждого естественно выделяются 5-10 семантических кластеров, но физически они слиплись, что замедляет ревью, нагружает IDE и оставляет агенты без полного контекста.

Проект уже использует **facade-pattern** (`X.rs` + `X/<sub>.rs`) для `app`, `element`, `platform`, `keymap`, `text_system`. A10 распространяет этот паттерн на три самых крупных оставшихся файла **без изменения публичной поверхности**.

Для `window.rs` добавляется одно архитектурное усиление — private `WindowCore` struct (стиль bevy/wgpu), который выносит ~140 полей за пределы прямого доступа sibling-подмодулей. Это устраняет основной риск split-а — несанкционированное reach в чужое state-пространство.

## Motivation

| Симптом | Причина |
|---|---|
| Reviewer-агенты теряют контекст при чтении `window.rs` | 6036 LoC > типичный context-window |
| Перекрытия чужих зон в PR-ах (CodeRabbit/Copilot часто выдают замечания на код, не относящийся к патчу) | один `impl Window` блок занимает 4608 строк |
| `cargo rustdoc` / `cargo doc` сборки медленные для XL-файлов | rustdoc реренгерит весь файл при изменении одного блока |
| Cross-track work (K07/K01/K04) тянет одну точку конкуренции | большая часть Phase 0-K work проходит через `Window` |
| Test-fixtures дублируются между файлами `*_test.rs` рядом с XL | нет общего `test_fixtures` модуля |

A2 (remaining globs audit) и A8 (`#[non_exhaustive]` audit) — параллельные tracks; A10 пересекается с A2 точечно (см. **Decisions D2**), но не с A8.

## Non-Goals

- **НЕ трогаем** `crates/flui-core/src/app/cell.rs` — K07 (AppCell removal) in-flight (rev 7).
- **НЕ трогаем** `crates/flui-core/src/provider/` — K01 (Provider rewrite) on review-gates.
- **НЕ меняем** crate-root `pub use element::*;` re-export — K91 cross-track contract: `crates/flui-framework/src/key.rs` зависит от crate-root видимости `Key`/`ValueKey`/`GlobalKey`/`ElementId`.
- **НЕ выделяем** в новые крейты (`flui-window`, `flui-geometry`). Phase I заморожена; cyclic deps с `App`/`Element`/`FramePhase` делают это преждевременным до Phase III.
- **НЕ переписываем** алгоритмы (event dispatch, paint pipeline, layout). Только физический split с сохранением behaviour-byte-equivalence.
- **НЕ трогаем** другие XL-файлы (`app.rs`, `platform.rs`, `element.rs`, `style.rs`, `color.rs`, `text_system.rs`, `key_dispatch.rs`) в этой спеке. Они получат отдельные A11+ мини-spec'и того же паттерна после валидации.
- **НЕ применяем** `#[non_exhaustive]` к перемещаемым enum'ам. Это A8-track, отделённый, чтобы не смешивать структурный и API-семантический рефакторы.
- **НЕ добавляем** новые тесты в этом track. Только перемещаем существующие. Новые тесты — отдельный T-track.

## Current Inventory

### XL-files в `crates/flui-core/src/` (LoC > 1000)

| File | LoC | KB | Кластеры | A10 target |
|---|---:|---:|---|---|
| `window.rs` | 6036 | 264 | 10+ | **A10a** |
| `geometry.rs` | 3802 | 126 | 8 типов-владельцев | **A10b** |
| `elements/div.rs` | 3673 | 166 | 5 | **A10c** |
| `app.rs` | 3368 | 150 | 5 | A11 (defer) |
| `platform.rs` | 2419 | 97 | 6 | A11 (defer) |
| `element.rs` | 1902 | 72 | 4 | A11 (defer) |
| `style.rs` | 1548 | 62 | 4 | A11 (defer) |
| `color.rs` | 1164 | 39 | 3 | A11 (defer) |
| `text_system.rs` | 1078 | 42 | 4 | A11 (defer — уже есть `text_system/` подпапка) |
| `key_dispatch.rs` | 998 | 38 | 3 | A11 (defer) |

### `window.rs` cluster inventory (Explore-агент, верифицировано)

| # | Кластер | Строки | Размер | Сложность |
|---|---|---|---|---|
| 1 | `DispatchPhase` enum + impl | 80-107 | 30 | LOW |
| 2 | `WindowInvalidator` struct + impl | 109-211 | 100 | LOW |
| 3 | `FocusHandle` / `WeakFocusHandle` / `Focusable` / `ManagedView` | 212-527 | 350 | MEDIUM |
| 4 | `HitboxId` / `Hitbox` / `HitboxBehavior` / `TooltipId` / `WindowControlArea` | 550-755 | 220 | LOW |
| 5 | `Frame` struct + impl | 798-960 | 200 | MEDIUM |
| 6 | `Window` struct decl (~140 полей) + `impl Window::new` | 961-1700 | 700 | HIGH |
| 7 | `DrawPhase` enum (K04 anchor) | 1155-1175 | 25 | MEDIUM |
| 8 | Draw/prepaint/paint pipeline методы | 1700-2440 | 700 | VERY HIGH (K04) |
| 9 | Paint primitives (`PaintQuad`, `quad`/`fill`/`outline`) | 6550-6644, 2080-2440 | 450 | MEDIUM |
| 10 | Layout / element scoping | разбросано | 700 | HIGH |
| 11 | Event dispatch + keyboard | 2890-4250 | 1350 | HIGH |
| 12 | Key bindings | 3030-3500 | 400 | MEDIUM |
| 13 | Window operations (move/resize/focus/close) | 4020-4395 | 400 | MEDIUM |
| 14 | Prompts | существующий `mod prompts;` | — | LOW |
| 15 | Inspector code (cfg-gated) | 4400-4620 | 220 | MEDIUM |
| 16 | `WindowHandle<V>` / `AnyWindowHandle` | 6320-6549 | 230 | LOW |

`Window` struct содержит ~140 полей — большинство уже `pub(crate)`. Около 20 полей truly private (`display_id`, `sprite_atlas`, `text_system`, `next_hitbox_id`, `focus_listeners`, `appearance`, `mouse_position`, `last_input_modality`, `pending_input`, etc.).

### `geometry.rs` per-type inventory

| Тип | Количество `impl` | Note |
|---|---:|---|
| `Axis` enum + `Along` trait | ~6 | helpers + axis-paramethrized accessors |
| `Point<T>` (+ `IsZero` blanket) | ~8 | базовые arithmetic + conversion |
| `Size<T>` | ~10 | arithmetic + Default + From |
| `Bounds<T>` | ~12 (5 с `where T: PartialOrd + Add + Sub`) | self-intersect / contains / map / shift |
| `Edges<T>`, `Corner` enum, `Corners<T>` | ~15 | edge math + corner enum |
| `Pixels` / `DevicePixels` / `ScaledPixels` / `Rems` / `Radians` / `Percentage` | ~80 | newtype family + conversions |
| `AbsoluteLength` / `DefiniteLength` / `Length` / `GridLocation` / `GridPlacement` | ~25 | CSS-like values |
| `Affine2` | ~6 | 2×3 matrix math |

Всего: **~192 `impl` блока на 17 типах**.

### `elements/div.rs` inventory

| Кластер | Строки прим. | Note |
|---|---|---|
| `Div` struct + `impl Element for Div` | ~700 | базовый container |
| `GroupStyle`, `DragMoveEvent<T>` | ~200 | top-level types |
| `InteractiveElement` / `StatefulInteractiveElement` traits + `on_*` builders | ~700 | fluent API |
| `Interactivity` + `InteractiveElementState` (state machine) | ~1100 | hover/click/drag tracking |
| `Stateful<E>` (tightly coupled to `Div` via `Interactivity` invariants) | ~500 | NOT to be split off |
| `ScrollHandle`, `ScrollAnchor` | ~300 | scroll primitives |
| `DivFrameState`, `DivInspectorState` (cfg-gated) | ~200 | per-frame state + inspector glue |

## Decisions

| # | Decision | Rationale |
|---|---|---|
| **D1** | `WindowCore` foundation в PR 1.0 (defensive) | Без `Core` struct sibling-подмодули будут reach в чужие поля через `pub(crate)`. `WindowCore` (`window/core.rs`, `pub(super) struct`) выносит ~140 полей за рамки прямого доступа. Стиль bevy/wgpu. |
| **D2** | A2 globs synergy: при split `window.rs` переписываем `pub use window::*` в `lib.rs` на explicit per-symbol. `pub use elements::*` и `pub use geometry::*` остаются glob (см. D13 + Practice 4). | Закрывает **1** из ~29 globs из A2 audit. См. D13 для elements rationale. Не блокирует остальной A2. |
| **D3** | Sequential order A10a → A10b → A10c | window валидирует `WindowCore` паттерн. geometry — самый простой (per-type), низкий риск. div — последний, зависит от наработок А10a. |
| **D4** | `DrawPhase` enum остаётся в фасаде `window.rs` | K04 reviewer audit требует видимости enum + assert'ов (`debug_assert!(self.draw_phase == DrawPhase::Paint)`) в одном месте. Transitions едут в `window/draw.rs`. |
| **D5** | `geometry` split per-type, не semantic | `impl<T> Bounds<T> where T: PartialOrd + ...` встречается 5 раз; semantic split разнёс бы их и сделал IDE jump-to-definition guessing-game. Per-type → `bounds.rs` содержит ВСЕ `impl Bounds`. |
| **D6** | `Stateful<E>` остаётся в `elements/div/base.rs` рядом с `Div` | Tight coupling через `Interactivity` invariants. Split нарушил бы инкапсуляцию. |
| **D7** | Подмодули с inspector называются `inspect_state.rs`, не `inspector.rs` | Избегаем имя-коллизию с `crate::inspector` (top-level module). Применимо к `window/` и `elements/div/`. |
| **D8** | Shared `test_fixtures` модуль в фасаде каждого split-крупного-файла | Без этого — каждый submodule получит копию `make_test_window()`. Создаётся в **PR 1.2** (`window/test_fixtures.rs`, `#[cfg(test)] pub(super) fn ...`). |
| **D9** | `#[non_exhaustive]` НЕ применяется в этом track | Отделено в A8 cycle. Не смешиваем структурный и API-семантический рефакторы. |
| **D10** | Новые тесты НЕ добавляются | Только перемещение существующих. Coverage extension — отдельный T-track. |
| **D11** | `DrawPhase` остаётся `pub(crate)`, **не** promote в `pub` | Auditor caught spec diagram error — нечаянная promote сломала бы semver guarantee «no new pub symbols». `DrawPhase` живёт в фасаде но `pub(crate)`. Submodules импортируют через `use super::DrawPhase;`. |
| **D12** | `Window { pub(super) core: WindowCore }` — **plain field**, никакого `Deref<Target = WindowCore>` | Auditor finding: Deref impl даёт `&Window → &WindowCore` любому caller, даже извне `window/`, обходя `pub(super)` boundary. Plain field access внутри `window/` решает задачу без leak. ADR-021 Practice 1 amended. |
| **D13** | `pub use elements::*` rewrite в A10c должен покрыть **полный** elements subtree, не только `div` | A10c затрагивает `pub use elements::*` glob → если оставить glob, A10c не делает A2 synergy для elements. Если переписать на explicit — нужно полный inventory всех 14 element-модулей (`anchored`, `animation`, `canvas`, `deferred`, `div`, `image_cache`, `img`, `list`, `modal_backdrop`, `surface`, `svg`, `text`, `uniform_list`), а это выходит за scope A10c. **Решение**: `pub use elements::*` остаётся **glob** в A10c. Сужаем D2: synergy A2 закрывает **только** `pub use window::*` (1 из ~29 globs). |

## Target Structure

### A10a — `crates/flui-core/src/window.rs` (6036 → ~700 LoC фасад)

```
crates/flui-core/src/
├── window.rs                    (~700 LoC — фасад)
│   ├── //! module-level rustdoc
│   ├── pub enum DispatchPhase { ... }       // оставлен (см. D4)
│   ├── pub(crate) enum DrawPhase { ... }    // ИСХОДНАЯ pub(crate) VISIBILITY — НЕ promote в pub (D4 + auditor finding)
│   ├── pub struct Window { pub(super) core: WindowCore }  // plain field, НЕ Deref<Target = WindowCore> (см. ADR-021 Practice 1)
│   ├── impl Window { pub(crate) fn new(...) -> Result<Self> { ... } }
│   ├── mod prompts;                          // существующий
│   ├── pub use prompts::*;
│   ├── (re-export блоки — explicit per-symbol; полный inventory см. Appendix A)
│   └── #[cfg(test)] mod test_fixtures;
└── window/
    ├── core.rs              (private WindowCore struct + getters)
    ├── invalidator.rs       (WindowInvalidator)
    ├── frame.rs             (Frame struct + impl, БЕЗ DrawPhase)
    ├── focus.rs             (Focusable / ManagedView / FocusHandle / WeakFocusHandle)
    ├── hitbox.rs            (HitboxId / Hitbox / HitboxBehavior / TooltipId / WindowControlArea)
    ├── handle.rs            (WindowHandle<V> / AnyWindowHandle)
    ├── paint.rs             (PaintQuad + free fn quad/fill/outline + impl Window paint методы)
    ├── layout.rs            (impl Window layout + element scoping методы)
    ├── draw.rs              (impl Window draw/prepaint/paint pipeline + DrawPhase transitions)
    ├── control.rs           (impl Window move/resize/focus_window/close/show/hide)
    ├── event_dispatch.rs    (impl Window mouse/keyboard dispatch)
    ├── key_dispatch_ext.rs  (impl Window key bindings)
    ├── inspect_state.rs     (cfg-gated, инспектор glue, НЕ inspector.rs)
    └── test_fixtures.rs     (#[cfg(test)] pub(super) fn make_test_window() etc.)
```

`Cargo.toml` нетронут. `lib.rs:317` `pub use window::*;` → переписывается на explicit per-symbol список в **PR 1.0**, прежде чем подмодули появятся, чтобы публичная поверхность была attached к фасаду. Re-export блоки в фасаде `window.rs` могут использовать `pub use focus::{Focusable, FocusHandle, WeakFocusHandle, ManagedView};` стиль.

**ВАЖНО (PR 1.0 hard block)**: полный inventory публичных символов window-subtree — в **Appendix A** этого спека. Категорически запрещено использовать `...` в финальном списке; PR 1.0 должен сначала сгенерировать список через `cargo public-api -p flui-core > before.txt` (или ручной grep если tool не установлен), и только потом писать explicit re-export.

### A10b — `crates/flui-core/src/geometry.rs` (3802 → ~150 LoC фасад)

```
crates/flui-core/src/
├── geometry.rs              (~150 LoC — фасад с glob re-export)
│   ├── //! module-level rustdoc
│   ├── mod axis; pub use axis::*;
│   ├── mod point; pub use point::*;
│   ├── mod size; pub use size::*;
│   ├── mod bounds; pub use bounds::*;
│   ├── mod edges_corners; pub use edges_corners::*;
│   ├── mod units; pub use units::*;
│   ├── mod css; pub use css::*;
│   └── mod transform; pub use transform::*;
└── geometry/
    ├── axis.rs              (Axis + Along trait + helpers)
    ├── point.rs             (Point<T> + IsZero)
    ├── size.rs              (Size<T>)
    ├── bounds.rs            (Bounds<T> + все 12 impl блоков)
    ├── edges_corners.rs     (Edges<T> + Corner + Corners<T>)
    ├── units.rs             (Pixels / DevicePixels / ScaledPixels / Rems / Radians / Percentage)
    ├── css.rs               (AbsoluteLength / DefiniteLength / Length / GridLocation / GridPlacement)
    └── transform.rs         (Affine2)
```

`lib.rs` `pub use geometry::*;` остаётся как **glob** — все типы публичные, никаких `pub(crate)` сюрпризов, glob-re-export безопасен и идиоматичен для математических примитивов.

### A10c — `crates/flui-core/src/elements/div.rs` (3673 → ~200 LoC фасад)

```
crates/flui-core/src/elements/
├── div.rs                  (~200 LoC — фасад)
│   ├── //! module-level rustdoc
│   ├── mod base;
│   ├── mod types;
│   ├── mod interactive;
│   ├── mod interactivity;
│   ├── mod scroll;
│   ├── #[cfg(any(feature = "inspector", debug_assertions))]
│   │   mod inspect_state;
│   └── pub use base::{Div, Stateful, div};  // explicit (D6: Stateful идёт в base)
│       pub use types::*;
│       pub use interactive::{InteractiveElement, StatefulInteractiveElement};
│       pub use interactivity::{Interactivity, InteractiveElementState};
│       pub use scroll::{ScrollHandle, ScrollAnchor};
└── div/
    ├── base.rs              (Div + impl Element for Div + Stateful<E>; см. D6)
    ├── types.rs             (GroupStyle, DragMoveEvent<T>)
    ├── interactive.rs       (InteractiveElement / StatefulInteractiveElement traits + on_* builders)
    ├── interactivity.rs     (Interactivity + InteractiveElementState state machine)
    ├── scroll.rs            (ScrollHandle + ScrollAnchor)
    └── inspect_state.rs     (cfg-gated; DivFrameState + DivInspectorState; см. D7)
```

## Naming, visibility и re-export discipline

(Полная политика — см. ADR-021. Здесь — короткие правила для исполнения.)

1. **Facade file** (`X.rs`):
   - объявление главного struct/enum/trait + `new`/конструкторы;
   - re-export `pub use X::sub::...`;
   - module-level rustdoc описывает архитектуру;
   - **никаких** business-logic методов.
2. **Sub-files** (`X/<cluster>.rs`):
   - `impl Type {...}` блоки своей семантической группы;
   - `pub(crate)` хелперы и `pub(super)` хелперы для siblings;
   - **запрет** новых `pub fn` без явного re-export через фасад.
3. **Visibility ladder**: `pub` → `pub(crate)` → `pub(super)` → private. Никаких новых `pub` без явного обоснования.
4. **Re-export style**:
   - `window/` → **explicit per-symbol** (semver-чувствительно);
   - `geometry/` → **glob** (математические примитивы, всё `pub`);
   - `elements/div/` → **explicit** для publi API.
5. **Файл-конвенции**:
   - `snake_case` имена (`event_dispatch.rs`, `key_dispatch_ext.rs`);
   - один кластер = один файл; мягкий потолок ~800 LoC;
   - module-level rustdoc с пометкой `//! See spec: docs/superpowers/specs/2026-05-13-A10-xl-file-split-design.md`.
6. **Тесты**: `#[cfg(test)] mod tests {}` внутри каждого подмодуля; shared fixtures — `window/test_fixtures.rs` (один источник).
7. **Cfg-gates**: `#[cfg(...)]` ставится на `mod inspect_state;` объявление в фасаде, **И** на call-site методов в other модулях, чтобы избежать compile-time ошибок в одной конфигурации.

## Migration Plan

Каждый PR — один кластер, минимальный diff, public API не меняется. После каждого шага: `cargo build -p flui-core` + `cargo test --workspace` зелёные.

### Phase 1 — `window.rs` (A10a, 11 PR-ов)

| PR | Кластер | Риск | Pre-PR review |
|---|---|---|---|
| 1.0 | **Foundation**: `WindowCore` private struct в `window/core.rs` (**embed by value, no Box/Arc, NO `impl Deref<Target = WindowCore>`** — D12); `Window { pub(super) core: WindowCore }` через **plain field**; A2 synergy — `pub use window::*` в `lib.rs` → explicit per-symbol список из **Appendix A** (НЕ `...` placeholder!). **Checklist hard-blocks**: (a) полный inventory сгенерирован через `cargo public-api -p flui-core > before.txt` ИЛИ задокументированный manual grep всех `^pub ` в `window.rs` + `window/prompts.rs` + macro-emitted symbols (`slotmap::new_key_type!` блоки) — результат paste в PR description; (b) `cargo public-api diff main..HEAD` empty; (c) `Rc::ptr_eq` semantics для `active`, `needs_present`, `input_rate_tracker` сохранены (no heap-relocation); (d) `DrawPhase` остаётся `pub(crate)` — НЕ `pub` (D11); (e) `cargo build -p flui-core --no-default-features` + `cargo build -p flui-core` зелёные; (f) flui-framework + examples собираются без правок. | Средний (architectural) | **Triple launch** обязателен: `flui-arch-reviewer` + `migration-risk-adversary` + `rust-api-migration-auditor` |
| 1.1 | `window/handle.rs` (`WindowHandle<V>` / `AnyWindowHandle`) | Минимальный | стандарт |
| 1.2 | `window/test_fixtures.rs` (`#[cfg(test)] pub(super) fn make_test_window()` + shared helpers) | Минимальный | стандарт |
| 1.3 | `window/focus.rs` (Focusable / ManagedView traits + FocusHandle / WeakFocusHandle + impl) | Низкий | стандарт |
| 1.4 | `window/hitbox.rs` (Hitbox / HitboxId / TooltipId / WindowControlArea + impl) | Низкий | стандарт |
| 1.5 | `window/paint.rs` (PaintQuad + free fn quad/fill/outline + Window paint методы) | Средний | стандарт |
| 1.6 | `window/layout.rs` (Window layout + element scoping методы) | Средний | стандарт |
| 1.7 | `window/control.rs` (Window move/resize/focus_window/close/show/hide) | Средний | стандарт |
| 1.8 | `window/invalidator.rs` + `window/frame.rs` + `window/draw.rs` (K04 anchor). **Checklist**: (a) `DrawPhase` enum остаётся в фасаде как `pub(crate)`; (b) `window/invalidator.rs` импортирует `DrawPhase` через `use super::DrawPhase;`, не redeclare; (c) audit ВСЕХ `crate::frame::FramePhase` doc-comment ссылок в moved-out коде — fully-qualified path обязателен (избегаем shadow `window::frame`); (d) **bare `cfg(debug_assertions)` audit** — каждое occurrence из `window.rs:1172,3554,3561,3570,3604,3615` проверить: не ссылается ли оно на символы, попавшие в compound-gated `window/inspect_state.rs` (`any(feature = "inspector", debug_assertions)`)? Если да → либо изменить gate на compound, либо переместить символ; (e) `cargo build -p flui-core --no-default-features --debug` зелёный. | **Высокий** | **Triple launch** обязателен. `DrawPhase` enum остаётся в фасаде! |
| 1.9 | `window/event_dispatch.rs` (mouse/keyboard dispatch) | **Высокий** | **Triple launch** обязателен |
| 1.10 | `window/key_dispatch_ext.rs` (key bindings) | Средний | стандарт |
| 1.11 | `window/inspect_state.rs` (cfg-gated; cfg-parity на call-site и def-site) | Низкий | стандарт |

### Phase 2 — `geometry.rs` (A10b, 6 PR-ов; per-type)

| PR | Модуль | Риск |
|---|---|---|
| 2.0 | Подготовка фасада `geometry.rs` под новый layout: внутри `geometry.rs` появляются `mod axis; pub use axis::*;` и аналоги для каждого подмодуля. `pub use geometry::*` в `lib.rs:???` остаётся **glob** (Practice 4 ADR-021 — математические примитивы). НЕ A2-synergy для этого track. | Минимальный |
| 2.1 | `geometry/axis.rs` (Axis + Along + helpers) | Минимальный |
| 2.2 | `geometry/point.rs` (Point<T> + IsZero) | Минимальный |
| 2.3 | `geometry/size.rs` (Size<T>) | Минимальный |
| 2.4 | `geometry/bounds.rs` (Bounds<T>, все 12 `impl` блоков в одном файле) | Низкий |
| 2.5 | `geometry/edges_corners.rs` (Edges<T> + Corner + Corners<T>) | Низкий |
| 2.6 | `geometry/units.rs` + `geometry/css.rs` + `geometry/transform.rs` (можно одним PR — небольшие модули) | Низкий |

### Phase 3 — `elements/div.rs` (A10c, 5 PR-ов)

| PR | Кластер | Риск |
|---|---|---|
| 3.0 | Подготовка фасада `elements/div.rs` под новый layout (внутренние `mod base; mod types; ...` и `pub use base::*; pub use types::*;` стиль). `pub use elements::*` в `lib.rs` **остаётся glob** (D13 — переписать на explicit требует полный inventory всех 14 element-модулей, выходит за scope A10c). | Минимальный |
| 3.1 | `elements/div/types.rs` (GroupStyle, DragMoveEvent) | Минимальный |
| 3.2 | `elements/div/scroll.rs` (ScrollHandle + ScrollAnchor; **БЕЗ Stateful**) | Низкий |
| 3.3 | `elements/div/interactive.rs` (InteractiveElement / StatefulInteractiveElement traits + `on_*` builders) | Средний |
| 3.4 | `elements/div/interactivity.rs` (Interactivity + InteractiveElementState state machine целиком на типах) | **Высокий** |
| 3.5 | `elements/div/inspect_state.rs` (cfg-gated). **Checklist**: (a) `DivFrameState` (без cfg на declaration) **остаётся в `base.rs`**; (b) `DivInspectorState` struct declaration **остаётся в `base.rs`** (`bounds`/`content_size` не cfg-gated) — только cfg-gated **methods/populating logic** уезжают в `inspect_state.rs`; (c) audit `--no-default-features --no-debug-assertions` build green. | Низкий |

`Stateful<E>` остаётся в `elements/div/base.rs` рядом с `Div` — coupled invariants (D6).

## Risks

(Findings от Plan-agent adversarial review.)

### CRIT — обязательно учесть до начала PR-ов

1. **K06 vs A10a структурный конфликт** (flui-arch-reviewer) — `ROADMAP.md:67` K06 entry: «split `window.rs` into `window/{lifecycle,layout,paint,hit_test,dispatch,focus,state,frame,actions}.rs`. Beyond cosmetic — split Window's monolithic borrow domain into `BuildOwner` / `PipelineOwner` / `SemanticsOwner`». K06 unscheduled но unblocked (после K05). K06 целится в ту же декомпозицию + ownership-shard model. **A10a `WindowCore(all 140 fields)` несовместим с K06 `BuildOwner/PipelineOwner/SemanticsOwner` целью**.
   - **Mitigation**: ROADMAP K06 entry помечен как `blocked-on: A10a`, с явной нотой что K06 supersedes `WindowCore` (A10a treats `WindowCore` как transient scaffold). Этот spec НЕ блокирует K06 future redesign — `WindowCore` сознательно простой переходный шаг. K06 follow-up spec будет re-decompose `WindowCore` в три owner-struct'а.

2. **Cyclic deps между подмодулями `window/`** — `focus.rs` нуждается в `DrawPhase` для focus-restore, `draw.rs` — в `FocusId` для paint phases, `event_dispatch.rs` cross-cuts всё.
   - **Mitigation**: `WindowCore` pattern (D1, PR 1.0). Подмодули видят только `pub(super)` интерфейс `WindowCore`, не reach в siblings напрямую.

3. **K04 `FramePhase` coupling** — `DrawPhase` enum (None/Prepaint/Paint/Focus) ≠ глобальный `FramePhase`, но K04 контракт требует `debug_assert!`-ов «DrawPhase::Paint только внутри FramePhase::Paint».
   - **Mitigation**: `DrawPhase` enum остаётся в фасаде `window.rs` (D4). `with_draw_phase` / transition методы — в `window/draw.rs`. `window/invalidator.rs` импортирует `DrawPhase` через `use super::DrawPhase;` (НЕ redeclare).

4. **`window/frame.rs` vs `crate::frame` name collision** (flui-arch-reviewer) — `crate::frame` это K04 `FramePhase` module (`lib.rs:41`). Submodule `crate::window::frame` shadow'ит `crate::frame` для кода внутри `window/`. Не compile error, но `window/draw.rs` doc-links и imports на `crate::frame::FramePhase` должны использовать **fully-qualified path** (`crate::frame::FramePhase`, не `frame::FramePhase`).
   - **Mitigation**: PR 1.8 checklist обязан включить audit всех `crate::frame::FramePhase` doc-comment ссылок в moved-out коде. Альтернативное название `window/frame.rs` → `window/per_frame.rs` или `window/frame_state.rs` рассмотреть, если name collision вызывает боль в первой итерации.

### CRIT (продолжение) — от rust-api-migration-auditor

15. **Incomplete explicit re-export inventory** (auditor) — изначальный пример `pub use window::{Window, WindowHandle, AnyWindowHandle, FocusHandle, ...}` использовал `...` placeholder. Реальный pub surface window-subtree (включая `prompts::*` chain) — **31+ символ**, включая `DEFAULT_WINDOW_SIZE`, `DEFAULT_ADDITIONAL_WINDOW_SIZE`, `FocusId`, `FocusOutEvent`, `ArenaClearNeeded`, `DismissEvent`, `DispatchEventResult`, `ContentMask`, `PaintQuad`, `quad`/`fill`/`outline`, `PromptResponse`, `Prompt`, `PromptHandle`, `RenderablePromptHandle`, `FallbackPromptRenderer`, `fallback_prompt_renderer`. Missing items → silent breaking removal.
    - **Mitigation**: Appendix A фиксирует FULL inventory; PR 1.0 hard-blocked до его генерации (`cargo public-api -p flui-core` или ручной grep всех `^pub ` в `window.rs` + `prompts.rs` + macro-emitted symbols через `slotmap::new_key_type!`).

16. **`DrawPhase` accidental `pub(crate) → pub` promotion** (auditor) — spec diagram изначально содержал `pub enum DrawPhase` хотя в коде `window.rs:1155` is `pub(crate)`. Это нарушает «no new pub symbols» гарантию.
    - **Mitigation**: D11 lock-in — `DrawPhase` остаётся `pub(crate)` в фасаде. Spec diagram исправлен.

17. **`Deref<Target = WindowCore>` reach-hole** (auditor) — даже при `WindowCore: pub(super)`, `impl Deref` даёт `&Window → &WindowCore` любому caller через `*window_ref`. Это обходит `pub(super)` boundary и делает `WindowCore` методы reachable извне `window/`.
    - **Mitigation**: D12 lock-in — `Window { pub(super) core: WindowCore }` через **plain field**, никакого Deref impl. ADR-021 Practice 1 amended.

18. **`cargo public-api` + `cargo-semver-checks` оба отсутствуют в CI** (auditor) — Public API guarantee «zero breaking change» relies на manual verification. R2 / R3 tracks ещё не landed.
    - **Mitigation**: Public API guarantees секция amended — wording «zero breaking change, verified manually until R2 lands». Все PR-описания обязаны включить вывод `cargo public-api diff` (или manual grep) inline.

### IMP — важные, решаемые механически

5. **`Rc<...>` shared-state в `WindowCore`** (flui-arch-reviewer) — `Window` имеет несколько `Rc<Cell<bool>>` / `Rc<RefCell<...>>` полей (`active`, `needs_present`, `input_rate_tracker`, `next_frame_callbacks`), которые cloned и shared с platform callbacks. Если `WindowCore` embed by value (просто struct внутри `Window`), `Rc::ptr_eq` semantics не меняется. Но если `WindowCore` обёрнут в `Box<WindowCore>` или `Arc<WindowCore>`, heap allocation сдвигается и `Rc` clone-holders ломаются.
   - **Mitigation**: PR 1.0 checklist обязан явно verify: «WindowCore is embedded **by value** (no `Box` / `Arc` around Core); `active`, `needs_present`, `input_rate_tracker` fields move into WindowCore but their `Rc<...>` heap allocations are unchanged».

6. **`DivInspectorState` имеет unconditional `pub` fields** (flui-arch-reviewer) — на `crates/flui-core/src/elements/div.rs:1644-1655` struct `DivInspectorState` имеет `bounds: Bounds<Pixels>` и `content_size: Size<Pixels>` **без cfg-gate**. Только `base_style` поле cfg-gated. Если переместить весь struct в cfg-gated `inspect_state.rs` модуль, билды `--no-default-features --no-debug-assertions` сломаются.
   - **Mitigation**: PR 3.5 audit access-sites `DivInspectorState::bounds` / `::content_size`; если есть non-cfg-gated callers — struct declaration остаётся в `base.rs`, в `inspect_state.rs` уезжает только cfg-gated **methods** на `DivInspectorState` (populating logic).

7. **`WindowControlArea` ADR-008 coupling** (flui-arch-reviewer) — `WindowControlArea` enum (decl `window.rs:585`) семантически tied к ADR-008 chrome invariant-enforcement methods (`start_window_resize`, `zoom_window`, `start_window_move`, `minimize_window`). Spec помещает enum в `window/hitbox.rs`, а методы — в `window/control.rs`. Split coupling.
   - **Mitigation (выбираем)**: ADR-008 fix PR-ы touch обоих файлов (`hitbox.rs` для enum, `control.rs` для invariants). Допустимо, поскольку enum используется и hitbox-mappingом и chrome-control'ом. Альтернатива — co-locate enum в `control.rs` (но тогда `hitbox.rs` импортирует back).

8. **`geometry.rs` per-type vs semantic** — `impl<T> Bounds<T> where T: PartialOrd + ...` встречается 5 раз. Semantic split разнёс бы их и нарушил IDE jump-to-definition.
   - **Mitigation**: per-type split (D5). `bounds.rs` содержит ВСЕ `impl Bounds<T>`.

9. **`window/inspector.rs` name collision** с `crate::inspector` (top-level module).
   - **Mitigation**: `window/inspect_state.rs` (D7). То же для `elements/div/inspect_state.rs`.

10. **Cfg-gated parity** — 23 occurrence'а `#[cfg(any(feature = "inspector", debug_assertions))]` в `window.rs`.
    - **Mitigation**: cfg ставится на `mod inspect_state;` объявление в фасаде, И на call-site методов в other модулях.

11. **`Stateful<E>` / `Div` coupling** через `Interactivity` invariants.
    - **Mitigation**: `Stateful<E>` остаётся в `elements/div/base.rs` (D6).

12. **Test fixture duplication** — без shared `test_fixtures` каждый submodule получит копию `make_test_window()`.
    - **Mitigation**: `window/test_fixtures.rs` (D8, PR 1.2).

### MINOR — фон

13. **K91 collision risk** — `lib.rs:154` имеет `pub use element::*;` (Key/ValueKey/GlobalKey/ElementId). Grep `window.rs` — нет collision'ов.
    - **Mitigation**: spec фиксирует doc-anchor на `Frame` (общее имя), чтобы future-split не переиспользовал.

14. **Free helpers** (`default_bounds`, `with_element_arena`, `quad`, `fill`, `outline`).
    - **Mitigation**: public (`quad`/`fill`/`outline`) → `window/paint.rs` + re-export через фасад. Shared internal (`with_element_arena`) → ближайший by-semantics модуль, `pub(super) fn`.

19. **Pre-existing wasm import breakage** (auditor) — `platform/web/events.rs:12` имеет `use crate::window::WebWindowInner`, `platform/web/platform.rs:5` имеет `use crate::window::WebWindow`. Оба пути НЕВЕРНЫ: правильные `crate::platform::web::window::WebWindowInner` / `WebWindow`. wasm CI `continue-on-error: true` (`wasm-check.yml:67`) маскирует.
    - **Mitigation**: PR 1.0 description должен явно отметить это как **pre-existing**, не вызванное split-ом. A10 НЕ исправляет это (вне scope) и НЕ claim-ит wasm-clean build.

20. **Bare `cfg(debug_assertions)` без `feature = "inspector"`** (auditor) — `window.rs:1172` (`ElementStateBox::type_name`) и lines 3554/3561/3570/3604/3615 (pattern-match блоки) используют **bare** `cfg(debug_assertions)`, не compound `any(feature = "inspector", debug_assertions)`. Spec изначально считал только 23 occurrence compound-gate, упустив 7 bare-gate.
    - **Mitigation**: PR 1.8 checklist amended — audit ВСЕХ bare `cfg(debug_assertions)` в moved-out коде; confirm они НЕ ссылаются на символы, перемещённые в `inspect_state.rs` (потому что `inspect_state.rs` gated compound `any(feature = "inspector", debug_assertions)`, а call-site gated `debug_assertions` alone → mismatch break `--no-default-features` + debug build).

## Public API guarantees

**Zero breaking change, verified manually until R2 lands** (auditor finding: ни `cargo public-api`, ни `cargo-semver-checks` пока не в CI — `.github/workflows/ci.yml` не содержит). Каждый PR обязан:

1. `cargo build -p flui-core` — успешно.
2. `cargo build -p flui-core --no-default-features` — успешно (защита от cfg-parity drift).
3. `cargo test --workspace` — успешно.
4. `cargo doc -p flui-core --no-deps` — без новых warning'ов про missing-docs.
5. `cargo-semver-checks` (manual install + run) — «no breaking changes». PR description должен paste-ить inline вывод. Когда R2 landed — это move в CI.
6. `cargo public-api -p flui-core` diff между `main` и PR — empty. Manual install через `cargo install cargo-public-api`. PR description также paste-ит inline.
7. `crates/flui-framework/src/key.rs` собирается без правок (K91 contract).
8. Зависящие крейты (`flui-widgets`, `flui-material`, examples) собираются без правок.
9. **A2 synergy check** (PR 1.0 only): diff между `cargo public-api` ДО/ПОСЛЕ переписки `pub use window::*` — пустой. Никаких потерянных и никаких новых публичных символов.
10. **No new `pub` symbols, no `pub(crate) → pub` promotions** (D11 / auditor finding) — каждое visibility change в PR явно задокументировано и обосновано в PR description.

## Verification (acceptance criteria для каждого PR)

- [ ] Public API diff пустой (`cargo public-api diff main..HEAD`).
- [ ] `cargo build -p flui-core` зелёный.
- [ ] `cargo test --workspace` зелёный.
- [ ] `cargo doc -p flui-core --no-deps` без новых warning'ов.
- [ ] `flui-framework` собирается без правок.
- [ ] `flui-widgets` + examples собираются без правок.
- [ ] Для PR-ов из «Высокий риск» — triple-launch reviewers (`flui-arch-reviewer` + `migration-risk-adversary` + `rust-api-migration-auditor`) дали зелёный.
- [ ] Module-level rustdoc нового файла содержит ссылку `//! See spec: docs/superpowers/specs/2026-05-13-A10-xl-file-split-design.md`.
- [ ] Никаких новых `pub` символов (включая `pub(crate)` → `pub` upgrade), кроме точных переэкспортов через фасад.

## References

- **Policy**: `docs/research/adr/ADR-021-xl-file-split-discipline.md`
- **Existing facade-pattern precedents**:
  - `crates/flui-core/src/app.rs` + `crates/flui-core/src/app/`
  - `crates/flui-core/src/element.rs` + `crates/flui-core/src/element/`
  - `crates/flui-core/src/platform.rs` + `crates/flui-core/src/platform/`
  - `crates/flui-core/src/keymap.rs` + `crates/flui-core/src/keymap/`
  - `crates/flui-core/src/text_system.rs` + `crates/flui-core/src/text_system/`
- **Cross-track contracts**:
  - **K91**: crate-root visibility of `Key`/`ValueKey`/`GlobalKey`/`ElementId` from `flui_framework::key`. Этот spec не трогает `element.rs` → contract сохранён.
  - **K04**: `DrawPhase` enum остаётся в фасаде `window.rs` (D4).
  - **A2**: synergy — закрываются 2 из ~29 globs (D2): `pub use window::*` + `pub use elements::*`. `pub use geometry::*` остаётся glob по Practice 4.
- **Spec series**: `docs/superpowers/specs/2026-05-08..` (K-track). Этот spec — первый A-track design-документ.
- **Roadmap entry**: `.ai-factory/ROADMAP.md` → `Architecture & API hygiene` → A10.

## Appendix A — Full inventory `window::*` public surface

(Derived from rust-api-migration-auditor read of `crates/flui-core/src/window.rs` @ 2026-05-13. Этот список — **stand-in** до момента, когда PR 1.0 author запустит `cargo public-api -p flui-core` против `main` HEAD и подтвердит точный список. Расхождение между Appendix A и `cargo public-api` вывода → **PR 1.0 hard-block**.)

### Direct `pub` declarations in `window.rs`

```rust
DEFAULT_WINDOW_SIZE         // const Size<Pixels>
DEFAULT_ADDITIONAL_WINDOW_SIZE  // const Size<Pixels>
DispatchPhase               // enum (Bubble, Capture)
FocusId                     // struct (newtype via slotmap::new_key_type!)
FocusOutEvent               // struct (через ManagedView pattern)
ArenaClearNeeded            // sentinel struct
FocusHandle                 // struct
WeakFocusHandle             // struct
Focusable                   // trait
ManagedView                 // trait (Focusable + EventEmitter + Render)
DismissEvent                // struct
WindowControlArea           // enum
HitboxId                    // struct (newtype u64)
Hitbox                      // struct
HitboxBehavior              // enum (Opaque, Translucent, DeferToChild)
TooltipId                   // struct (newtype usize)
Window                      // struct (главное)
DispatchEventResult         // struct
ContentMask<P>              // generic struct
WindowHandle<V>             // generic struct
AnyWindowHandle             // struct (erased)
PaintQuad                   // struct
quad                        // free fn
fill                        // free fn
outline                     // free fn
```

### Via `pub use prompts::*` chain

```rust
PromptResponse              // enum
Prompt                      // struct
PromptHandle                // struct
RenderablePromptHandle      // struct
FallbackPromptRenderer      // struct
fallback_prompt_renderer    // free fn
```

### Items currently `pub(crate)` — STAY `pub(crate)` (D11)

```rust
DrawPhase                   // pub(crate) enum — НЕ promote!
WindowInvalidator           // pub(crate) struct
Frame                       // pub(crate) struct
DispatchNode (etc.)         // pub(crate)
```

### Verification procedure (PR 1.0 hard-block)

```bash
# 1. Установить cargo public-api
cargo install cargo-public-api --locked

# 2. Snapshot главной ветки
git switch main
cargo public-api -p flui-core > /tmp/before.txt

# 3. Apply PR 1.0 (WindowCore + explicit re-export)
git switch <branch>
cargo public-api -p flui-core > /tmp/after.txt

# 4. Diff
diff /tmp/before.txt /tmp/after.txt
# Ожидаемый результат: empty.

# 5. Если нет cargo public-api — fallback ручной grep:
grep -nE "^pub (struct|enum|trait|fn|const|type) " crates/flui-core/src/window.rs
grep -nE "^pub (struct|enum|trait|fn|const|type) " crates/flui-core/src/window/prompts.rs
grep -nE "slotmap::new_key_type" crates/flui-core/src/window.rs
# Сравнить с Appendix A. Расхождение → пометить PR как DO-NOT-MERGE.
```

## Out of scope

- Other XL-files (`app.rs`, `platform.rs`, `element.rs`, `style.rs`, `color.rs`, `text_system.rs`, `key_dispatch.rs`) — A11+ tracks после валидации паттерна на A10a.
- `#[non_exhaustive]` audit — отдельный A8 track.
- Sub-crate extraction (`flui-window`, `flui-geometry`) — Phase III only, после конкретного embedding driver.
- `cargo public-api` adoption — отдельная R-track задача (R2 / R3); spec использует ручную проверку до того.
