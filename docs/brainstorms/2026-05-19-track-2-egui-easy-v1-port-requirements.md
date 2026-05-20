---
date: 2026-05-19
topic: track-2-egui-easy-v1-port
---

# Track 2 "egui-easy" v1 port — Requirements

## Summary

Port subset из v1 (`C:\Users\vanya\RustroverProjects\flui\crates`) в v2 — 5 крейтов реализуют track 2 "DX & low-ceremony onboarding" из `STRATEGY.md` целиком: `flui-cli` (CLI tooling, subset 10+ commands), `flui-foundation` (utility primitives), `flui-types` (base types), `flui-hot-reload` (dynamic reload), `flui-devtools` (profiler/memory/network/timeline). Каждый крейт kept as separate workspace member; не folding в flui-core.

---

## Problem Frame

flui v1 имеет реальный код под "Flutter dev experience" — `flui-cli` (18 commands clone `flutter`), `flui-devtools` (Flutter DevTools clone), `flui-hot-reload` (custom dynlib + driver pipeline), `flui-foundation`/`flui-types` (Flutter-shaped primitives). v1 был abandoned после того как RenderObject pipeline сломался; основная работа шла в render-layer, остальные крейты "частично работали" но не доводились/не проверялись.

v2 hard-fork`gpui-ce` обошёл render wall (используя готовый gpui Engine substrate), но всё ещё пустой по DX-tooling — нет `flui` CLI binary, нет hot-reload story, нет DevTools, нет ergonomic prelude для базовых типов. Primary user (Rust dev, который пишет UI любого размера без выхода из Rust toolchain — see `STRATEGY.md`) сегодня:
- генерит проект через `cargo new` + копипасту из examples
- запускает через `cargo run --example`
- не имеет hot-reload — каждое изменение = ~10s compile cycle
- не имеет inspector/profiler

Этот brainstorm определяет scope порта DX-уровня v1 крейтов в v2 workspace.

---

## Actors

- A1. **Primary user (Rust dev)**: Hires v2 чтобы строить UI любого размера в Rust toolchain. Жертва текущего DX gap.
- A2. **Maintainer (project author)**: Делает port + integration. Хочет cherry-pick v1 working code вместо greenfield.
- A3. **Future contributor**: После port'а должен мочь run `flui create`, использовать hot-reload, открыть devtools без чтения v2 internals.

---

## Requirements

**flui-cli (new crate `crates/flui-cli/`)**

- R1. Бинарь `flui` (через `[[bin]]` в Cargo.toml) запускается командой `flui <subcommand>` после `cargo install --path crates/flui-cli`.
- R2. Subset 10+ runtime-agnostic команд портируется из v1: `create`, `run`, `build`, `test`, `clean`, `doctor`, `completions`, `format`, `analyze`, `upgrade`, `create_interactive`. Mobile/web/devtools-UI команды стрипаются.
- R3. `flui create <name>` создаёт новый v2 проект из template'а. Минимум один template "hello-world" (single-window, single-widget) shippable в v1 brainstorm-результата.
- R4. `flui doctor` проверяет environment (cargo + rustc version ≥ MSRV 1.95, Git, platform-specific deps для desktop) и выводит actionable diagnostics.
- R5. `flui completions <shell>` генерирует shell completions для bash/zsh/fish/powershell.
- R6. Все runtime-wrapper команды (`run`, `build`, `test`, `clean`, `format`) корректно проходят args в cargo. Desktop targets only (mac/linux/windows native).

**flui-foundation (new crate `crates/flui-foundation/`)**

- R7. Порт `assert`, `binding`, `callbacks`, `debug`, `error`, `id`, `key`, `notifier`, `observer` модулей из v1 как новый крейт.
- R8. Крейт не зависит от `flui-core` — primitives runtime-agnostic.
- R9. `consts`, `platform`, `wasm` модули из v1 OPTIONAL (port если содержит value для desktop; иначе drop).

**flui-types (new crate `crates/flui-types/`)**

- R10. Порт `geometry` (bezier, bounds, circle, corner, ...) и `color` модулей из v1 как новый крейт.
- R11. Параллельная implementation с v2 `crates/flui-core/src/geometry.rs` + `color.rs` — НЕ fold пока. Consolidation deferred к будущему K-track audit.
- R12. Крейт не зависит от `flui-core`.

**flui-hot-reload (new crate `crates/flui-hot-reload/`)**

- R13. Hot-reload primitive портируется из v1 (`driver.rs`, `dynlib.rs`, `host.rs`, `pipeline.rs`, `plugin.rs`).
- R14. **Re-architected над gpui-ce primitives** — v1 hot-reload завязан на v1 render pipeline (выкинут). Замена интегрирует с v2's `Window` / `App` / `K04 FramePhase`.
- R15. Demo: изменение `RenderOnce` widget в example crate триггерит reload без полного rebuild. Acceptable latency ≤ 2s для smallest test case.
- R16. Mechanism stack (custom dynlib vs ecosystem crates `subsecond` / `hot-lib-reloader`) — RESEARCH в `/ce-plan`, не resolved здесь.

**flui-devtools (new crate `crates/flui-devtools/`)**

- R17. Порт `profiler`, `memory`, `network`, `timeline`, `remote`, `common`, `hot_reload` модулей из v1.
- R18. Tap into v2's `K22 InspectableElement` substrate (для tree introspection) и `K04 FramePhase` / `FrameProfile` (для timing).
- R19. Wire protocol = **copy Flutter DevTools protocol** (Flutter parity per STRATEGY approach), не v1 custom.
- R20. UI = headless first (TCP + JSON). DevTools UI app — deferred к будущему brainstorm (Tier C ecosystem).

**Cross-crate**

- R21. Все 5 новых крейтов добавлены в workspace `Cargo.toml` members list.
- R22. `flui-log` (v1 logging) НЕ портируется — v2 использует workspace `tracing`. Where v1 cli depended on `flui-log`, replace с `tracing`.
- R23. v1 crate names сохраняются: `flui-cli`, `flui-foundation`, `flui-types`, `flui-hot-reload`, `flui-devtools`. Не rebrand'ятся.

---

## Acceptance Examples

- AE1. **Covers R1, R3.** Given `cargo install --path crates/flui-cli` succeeded, when user runs `flui create my-app`, then a new directory `my-app/` is created with valid Cargo.toml + `src/main.rs` containing hello-world widget that compiles via `cargo run` and opens a window.
- AE2. **Covers R4.** Given Rust 1.95+ installed + Git in PATH, when user runs `flui doctor`, then output reports OK status for all checks; if rustc < 1.95, output reports failing check with command to fix.
- AE3. **Covers R15.** Given example crate uses flui-hot-reload + has Render widget defined, when widget source is edited and saved, then running app reflects change in ≤ 2s without full recompile.
- AE4. **Covers R19.** Given flui-devtools running + flui app launched, when external Flutter DevTools client connects on configured port, then it receives expected protocol handshake response.

---

## Success Criteria

- **Human outcome:** Primary user может выполнить `cargo install --path crates/flui-cli && flui create my-app && cd my-app && flui run` и увидеть работающее окно — за < 1 минуты wall-time, без чтения flui-core docs. Это первый concrete proof of `STRATEGY.md` track 2 "egui-easy" commitment.
- **Downstream handoff:** `/ce-plan` может прочитать этот doc и не нуждается в изобретении product behavior. Только implementation choices (наименование функций, exact module layout, dep versions, hot-reload mechanism stack research) остаются для plan.

---

## Scope Boundaries

- Остальные 16 v1 крейтов (`flui-animation`, `flui-app`, `flui-assets`, `flui-build`, `flui-engine`, `flui-interaction`, `flui-layer`, `flui-log`, `flui-painting`, `flui-platform`, `flui-reactivity`, `flui-rendering`, `flui-scheduler`, `flui-semantics`, `flui-tree`, `flui-view`) — runtime-уровень, replaced by gpui-ce; не в track 2 "egui-easy".
- Phase III platform reach (mobile/web/iOS/Android/wasm) — cli `devices`, `emulators`, `platform`, mobile branches в `build`/`run`; build/devtools mobile-specific paths.
- DevTools UI app (frontend) — deferred к Tier C ecosystem brainstorm. flui-devtools здесь = headless protocol substrate only.
- flui-build crate port — DEFERRED. cli's `build`/`analyze`/`upgrade` shell out к cargo напрямую (через в-крейт-локальный runner module из v1). Когда понадобится cross-platform build orchestration (Phase III), отдельный brainstorm.
- Templates beyond hello-world (admin-shape, material-shape, etc) — заблокированы Tier C widget library, которой ещё нет. Deferred.
- flui-foundation/flui-types fold в flui-core — deferred к будущему K-track audit. Сейчас parallel-impl.
- v1 `flui-log` crate — НЕ портируется (use workspace `tracing`).
- v1 RenderObject-dependent code в любом из 5 портируемых крейтов — discard or rewrite, не carry forward.

---

## Key Decisions

- **5 separate crates, не consolidated mega-crate**: Preserves modularity, allows parallel work на /ce-plan этапе, follows v1 lineage.
- **foundation/types parallel-impl, не fold в flui-core**: Keeps lineage clean, не breaking changes к flui-core API. Audit/fold deferred.
- **DevTools wire protocol = Flutter DevTools protocol clone**: Aligns с STRATEGY approach "full Flutter parity, copy the shape". Bonus: future Flutter DevTools client может подключаться к flui app.
- **Hot-reload mechanism = deferred research**: Brainstorm не может это решить без verification что v1 dynlib работал + state Rust hot-reload экосистемы (`subsecond`, `hot-lib-reloader`, custom) в 2026. Передаётся в /ce-plan.
- **flui-log replaced by workspace tracing**: Воркспейс уже стандартизован на `tracing`, дублирование убирается.
- **v1 crate names preserved**: Не rebrand'инг, чтобы port был mechanical где возможно.
- **Только hello-world template в v1 brainstorm scope**: Дополнительные templates (admin-shape, material-shape, и т.д.) deferred — blocked Tier C widget library, которой ещё нет. Один template + scope boundary на остальные достаточно для proof of "egui-easy" commitment.

---

## Dependencies / Assumptions

- v1 source tree доступен по `C:\Users\vanya\RustroverProjects\flui\crates` (verified during brainstorm).
- v1 крейты "частично работали" но не верифицированы compile'ом против актуального Rust 1.95 — likely потребуется fix-and-port pass (verified user statement).
- v2's `K22 InspectableElement` substrate ещё не landed (см. roadmap `K-independent K22`). flui-devtools R18 требует K22 как prerequisite, ИЛИ принимает что K22 substrate приходит параллельно с devtools work.
- v2's `K04 FramePhase` / `FrameProfile` substrate landed (per ROADMAP.md, K04 complete).
- Workspace Cargo.toml members list поддерживает добавление 5 новых крейтов без structural changes (assumption — verify во время /ce-plan).
- `flui` binary name доступен на crates.io (assumption — verify во время /ce-plan через `cargo search flui`).

---

## Outstanding Questions

### Deferred to Planning

- [Affects R16][Needs research] Какой hot-reload mechanism в Rust 2026: v1 custom dynlib (port as-is), `subsecond` (Dioxus team), `hot-lib-reloader`, или гибрид? Verification что v1 dynlib работал нужна перед решением.
- [Affects R19][Technical] Flutter DevTools protocol versioning — какая версия (DDS / VM Service / DAP)? Проверить compat target.
- [Affects R7-R9][Technical] Какие именно v1 foundation модули compile'ятся против Rust 1.95 — нужен compile-pass audit перед port'ом.
- [Affects R11][Technical] v1 `flui-types/src/geometry/*` shape compat с v2 `flui-core::geometry` — какие types overlap, какие unique? Influences fold decision (deferred но influences API surface).
- [Affects R18][Technical] K22 InspectableElement substrate state — landed parallel или blocking prerequisite для flui-devtools work?
- [Affects R22][Technical] Все ли v1 cli файлы depending на `flui-log` могут быть rewritten на `tracing` mechanically?
