---
name: flui-v2
last_updated: 2026-05-19
---

# flui-v2 Strategy

## Target problem

Rust-разработчики, которым нужен Flutter-style декларативный UI, упираются в две стены: реализация многотрассовой Flutter-pipeline (Element/RenderObject/Layer/Widget/Semantics) и порт Dart-кода в Rust без наследования. Существующие Rust UI-фреймворки уходят в HTML/CSS-стиль, immediate-mode или мосты через FFI/wasm/Tauri — Flutter-class DX остаётся незакрытым.

## Our approach

Full Flutter parity в Rust — копируем shape целиком (Widget DX, multi-platform desktop+mobile+web, hot-reload, inspector), а не cherry-pick. `gpui-ce` взят как production-grade Engine substrate, чтобы не строить рендер с 0; Framework-tier (`flui-framework`) воспроизводит Flutter API поверх. Альтернативы НЕ выбираем: HTML/CSS-style (Dioxus/Leptos), immediate-mode (egui), single-platform, abstraction через FFI/wasm/Tauri/Electron.

## Who it's for

**Primary:** Rust разработчик, который хочет писать UI любого размера — от быстрого эксперимента до полного приложения — без выхода из Rust toolchain. Они "нанимают" flui чтобы остаться в одном `cargo build`, не свитчиться на TS/React/Electron ради UI, и получить **Flutter-class API ergonomics с egui-class простотой старта** — низкий ceremony для прототипов, мощно для сложных apps.

## Key metrics

- **Личные проекты на flui** — количество новых проектов, где flui выбран над альтернативами (TS/React/Tauri/egui). Считается ежемесячно. Журнал: `docs/projects-using-flui.md`.
- **Time-from-cold-to-working-UI** — wall-time от `cargo new` до работающей admin-страницы (CRUD list+detail+form) с sqlx-backend'ом. Квартальный benchmark, запись в `docs/dx-baseline.md`.
- **Flutter parity %** — `(закрытые S08-S20 + SF02-SF08 + K-internal-org + K-independent) / total roadmap items`. Считается по `.ai-factory/ROADMAP.md`.
- **API friction log** — счётчик "застрял/боль" эпизодов в dev-журнале ("хотел сделать X в стиле Flutter, не вышло" / "написал N строк boilerplate под Y"). `docs/friction-log.md` или GitHub Issues с label `dx-friction`. Считается ежемесячно.

## Tracks

### Framework tier maturity

SF02-SF08 (reconciliation, BuildCx/Provider, State, setState, InheritedWidget, Widget→Element adapter, async widgets) + S08-S15 (Semantics, Canvas, Filters, Focus, Text, MediaQuery, Assets) — closing Flutter API + Engine gaps.

_Why serves the approach:_ без Framework tier "full Flutter copy" — пустые слова; downstream apps пишут поверх raw Engine.

### DX & low-ceremony onboarding

hot-reload, inspector (K22 substrate + UI), prelude expansion (K94), error messages, test harness (K17 `flui_core::testing`), examples crate (single-file demos), `cargo new --template flui` story, doctest fixtures.

_Why serves the approach:_ "egui-easy" — strategic differentiator. Без этого новый user уходит на egui для экспериментов и на Tauri для apps.

### Kernel hygiene & readability

K-internal-org (K06 Window decomposition, K08 Action dispatcher, K10 Style decomposition, K11 Hit-test arena), K-independent (K12-K22), K-hygiene (K90 GPUI→flui rebrand, K91 globs, K93 TODO triage, K97/K98 docs), A10b/A10c/A11 XL-file splits.

_Why serves the approach:_ долг `gpui-ce` → если не закрыть, каждая фича в Framework tier дороже; reviewer-агенты тонут в XL-файлах.

### Widget ecosystem (Tier C)

flui-widgets (core primitives), flui-material, flui-cupertino, flui-theme, flui-a11y (с S08), flui-navigator (уже есть).

_Why serves the approach:_ Flutter parity без widget library = empty Framework. Без `MaterialApp { Scaffold { ... } }` пользователь не пишет UI, он пишет boilerplate.

## Not working on

- **HTML/CSS abstraction** — не Dioxus/Leptos-style, не web-first ментальная модель.
- **Immediate-mode** — не egui-style render-each-frame.
- **FFI/wasm/мосты к JS-фреймворкам / Tauri / Electron** — flui native Rust по всему toolchain.
- **Cherry-pick стиля** — не "взять часть Flutter, остальное по-своему"; идём full copy.
- **Platform reach (iOS/Android/Web)** — Phase III, deferred; критическая chain (Framework tier + DX + kernel hygiene + widgets) идёт первой.
- **Standalone Engine** — `flui-core` не позиционируется как general-purpose graphics engine; его reason d'être = служить Framework tier.
