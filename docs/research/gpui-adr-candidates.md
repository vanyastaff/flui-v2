# ADR-кандидаты на основе GPUI issues

Производный документ от [gpui-issues.md](gpui-issues.md) (overlay: [gpui-issues-overlay.yaml](gpui-issues-overlay.yaml)).
Все issue, у которых в overlay `adr: yes` или `adr: maybe`, сгруппированы по теме.
Несколько issue часто покрываются **одним** ADR — это явно отмечено.

Цель ADR: зафиксировать решение **до** того, как мы повторим ту же ошибку. После выбора темы — следующий шаг это:

1. Подтвердить/опровергнуть `repro: unknown` (быстрый код-аудит в указанных модулях).
2. Если повторили — починить + написать ADR. Если нет — написать ADR-инвариант, чтобы не повторить.

---

## 1. Rendering / GPU pipeline (high-impact)

| Issue | Тема | flui-v2 точка касания |
|-------|------|------------------------|
| [#8043](https://github.com/zed-industries/zed/issues/8043) | overdraw 5-6× per pixel; нужен front-to-back opaque pass + depth/stencil reject | `flui-core/src/platform/wgpu`, `scene.rs` (когда появится) |
| [#15166](https://github.com/zed-industries/zed/issues/15166) | damage / present regions API (partial present) | `flui-core/src/window`, wgpu surface presentation |
| [#50392](https://github.com/zed-industries/zed/issues/50392) | анимация триггерит full layout repaint (invalidation scope) | `flui-core/src/animation`, K02/K03 (element identity + render/build) |
| [#37727](https://github.com/zed-industries/zed/issues/37727) | Windows: ввод текста = GPU 20% vs VSCode 2% | `flui-core/src/text_system`, `platform/windows` |
| [#44339](https://github.com/zed-industries/zed/issues/44339) | ObjectFit::Cover + rounded corners: clip vs image transform ordering | image-элемент, clip path порядок |
| [#45897](https://github.com/zed-industries/zed/issues/45897) | нет Vulkan → 100% CPU на всех ядрах | `platform/wgpu` software fallback |

**Кандидат на единый ADR:** _Rendering invariants (overdraw, partial present, invalidation scope)_ — #8043 + #15166 + #50392 + #37727. Слишком связано, чтобы дробить.

## 2. Color / alpha pipeline

| Issue | Тема |
|-------|------|
| [#55972](https://github.com/zed-industries/zed/issues/55972) | opacity двух полупрозрачных слоёв суммируется, >100% = непрозрачно |
| [#33050](https://github.com/zed-industries/zed/issues/33050) | gpui blending не совпадает со стандартным source-over alpha |

**Единый ADR:** _Color/alpha pipeline contract_ — source-over alpha + premultiplied RGB.

## 3. Text rendering

| Issue | Тема | flui-v2 точка касания |
|-------|------|------------------------|
| [#49860](https://github.com/zed-industries/zed/issues/49860) | CJK truncate panic «not a char boundary» — UTF-8 unsafety при ellipsis | срочный код-аудит наших truncation helpers |
| [#55214](https://github.com/zed-industries/zed/issues/55214) | metrics hinting + bi-level rendering (sharper text) — выбор стратегии растеризации | стратегия растеризации текста |

**Кандидаты:** _Text slicing UTF-8 safety_ (короткий ADR + проверка кода) и _Text rasterization strategy_ (более длинный, про Skrifa и режимы).

## 4. Window / display lifecycle

| Issue | Тема |
|-------|------|
| [#56294](https://github.com/zed-industries/zed/issues/56294) | `on_next_frame` в `paint` → 1px shift при resize. Контракт frame-callback lifecycle |
| [#46378](https://github.com/zed-industries/zed/issues/46378) | `displays()` пуст до создания окна (Wayland). Lifecycle обнаружения мониторов |
| [#21851](https://github.com/zed-industries/zed/issues/21851) | DPI scale не обновляется при переносе окна между X11-мониторами |
| [#27500](https://github.com/zed-industries/zed/issues/27500) | drag-region на title bar тащит окно при клике на кнопку |
| [#52067](https://github.com/zed-industries/zed/issues/52067) | `is_minimizable: false` всё равно минимизирует |
| [#14590](https://github.com/zed-industries/zed/issues/14590) | X11 background blur (xprops) |

**ADR (короткий):** _Frame callback lifecycle_ — когда `on_next_frame` валиден, что происходит при resize/configure.
**ADR:** _Display discovery contract_ — `displays()` до и после создания окна.
**ADR:** _WindowOptions invariants_ — что значит `is_minimizable: false` (платформа должна _действительно_ блокировать).

## 5. Input / focus / hit-testing

| Issue | Тема | flui-v2 точка касания |
|-------|------|------------------------|
| [#52550](https://github.com/zed-industries/zed/issues/52550) | macOS doCommandBySelector ignores → IME/keybinding pipeline ломается | контракт input/IME |
| [#38350](https://github.com/zed-industries/zed/issues/38350) | hover events когда окно в background | pointer-events lifecycle |
| [#34796](https://github.com/zed-industries/zed/issues/34796) | локальный tab-index API (не глобальный) | focus management, `flui-a11y` |
| [#24405](https://github.com/zed-industries/zed/issues/24405) | hover просачивается сквозь панели | hit-test layering |
| [#52013](https://github.com/zed-industries/zed/issues/52013) (maybe) | folder picker за modal | overlay/portal ordering |
| [#52448](https://github.com/zed-industries/zed/issues/52448) (maybe) | modal в одном окне блокирует другие | scope модальности |
| [#54017](https://github.com/zed-industries/zed/issues/54017) (maybe) | floating window не key | macOS window levels |

**Кандидат на единый ADR:** _Hit-test, overlay ordering, hover/focus scope_ — #24405 + #52013 + #38350 связаны через одну ментальную модель «event routing layers».

## 6. Drag-and-drop / custom paint

| Issue | Тема |
|-------|------|
| [#52110](https://github.com/zed-industries/zed/issues/52110) | external DnD API на всех платформах |
| [#43273](https://github.com/zed-industries/zed/issues/43273) | `<canvas>`-аналог для one-off shaders |

## 7. Resilience: GPU device-loss

**Единый большой ADR:** _GPU device-loss graceful recovery_ — покрывает [#23288](https://github.com/zed-industries/zed/issues/23288) + [#30469](https://github.com/zed-industries/zed/issues/30469) + [#52085](https://github.com/zed-industries/zed/issues/52085).

Что включить: detection (`wgpu::DeviceLost`), recreation strategy, persistence пользовательского state, monitor-disconnect hook.

## 8. Strategic / roadmap

| Issue | Тема |
|-------|------|
| [#52715](https://github.com/zed-industries/zed/issues/52715) | wasm regression (closure recursion, proptest pulls `imp`) — gating dev-deps для wasm target |
| [#43207](https://github.com/zed-industries/zed/issues/43207) | Android target |
| [#43206](https://github.com/zed-industries/zed/issues/43206) | iOS target |
| [#21341](https://github.com/zed-industries/zed/issues/21341) (maybe) | shadow behind transparent windows |

---

## Приоритезация (предложение)

Сделаем ADR в таком порядке — от срочного к стратегическому:

1. **Text slicing UTF-8 safety** (#49860) — есть `flui-core/src/text_system`, проверить руками, может уже воспроизводимо. **5 минут аудита.**
2. **Rendering invariants** (#8043 + #15166 + #50392 + #37727) — наш ключевой ADR; критично для Windows-производительности.
3. **Color/alpha pipeline contract** (#55972 + #33050) — определяет API раньше, чем мы успеем накопить пользователей.
4. **Hit-test, overlay ordering, hover/focus** (#24405 + #52013 + #38350) — лучше зафиксировать до того, как появятся первые сложные виджеты.
5. **GPU device-loss** (#23288 + #30469 + #52085) — большой, но важный, для production-готовности.
6. Остальное — по мере появления конкретных задач.

После шага 1 (аудит text_system) — обновляем `repro` в overlay и решаем, нужен ли тут же фикс.
