# GPUI closed issues — cross-walk to ADRs

**Date:** 2026-05-12
**Status:** Derived artifact. Sources:
[`gpui-issues.md`](gpui-issues.md) Closed section (199 closed
`area:gpui` issues) + [`adr/`](adr/) (ADRs 001–020).

## Why

The open-issue cross-walk (in [GPUI overlay](gpui-issues-overlay.yaml))
captured what is **broken**. The closed corpus captures what is
**fixed** — and a closed issue with 20+ comments is usually a record
of *how* upstream solved a problem we may face. This document maps
the most-discussed closed issues to our ADRs and notes whether the
upstream fix gives us a reusable solution.

The cutoff is "comments ≥ 10 **and** clearly architectural". The
long tail of small bug-fix closures is not listed.

## Mapping

### ADR-001 — Invalidation scope

| Closed issue | 💬 | Outcome | Why it maps |
|---|---|---|---|
| [#15311](https://github.com/zed-industries/zed/issues/15311) | 28 | completed | Linux window resizes slow — invalidation + relayout under configure events. Worth reading their patch for our partial-present ADR-006. |
| [#32792](https://github.com/zed-industries/zed/issues/32792) | 21 | completed | Sway/wlroots window flashes rapidly — Wayland configure storm + over-refresh. Same family as ADR-001's contract. |
| [#43366](https://github.com/zed-industries/zed/issues/43366) | 10 | completed | "Intense flickering with GPUI apps" — likely the same Wayland configure path; check whether their fix landed in our wayland glue. |

### ADR-003 — Color / alpha pipeline

| Closed issue | 💬 | Outcome | Why it maps |
|---|---|---|---|
| [#10993](https://github.com/zed-industries/zed/issues/10993) | 13 | completed | macOS blurry background + broken shadow — alpha + filter composition near a window backdrop. Reuse their NSVisualEffectView positioning if we hit the same. |

### ADR-005 — GPU device-loss recovery

| Closed issue | 💬 | Outcome | Why it maps |
|---|---|---|---|
| [#14071](https://github.com/zed-industries/zed/issues/14071) | 14 | completed | "Zed opens but fails to draw" — adapter init failure path. Their fix likely matches our `recover()` strategy at adapter level. |
| [#14446](https://github.com/zed-industries/zed/issues/14446) | 12 | not_planned | Linux NVIDIA: no text/icons after suspend/resume — device-loss without the loss flag firing. Closed as not-planned upstream; their reasoning may inform our gap list (ADR-005 gap 1). |
| [#49181](https://github.com/zed-industries/zed/issues/49181) | 12 | completed | Crashing consistently on Intel Mac — adapter classification + Metal path. Cross-reference ADR-014's `RendererKind` audit. |

### ADR-007 — Display lifecycle (DPI / scale)

| Closed issue | 💬 | Outcome | Why it maps |
|---|---|---|---|
| [#25195](https://github.com/zed-industries/zed/issues/25195) | 16 | completed | Sway fractional-scaling text blur on resize — DPI mid-frame + cosmic-text resample. Same family as ADR-007's scale-factor invariant. |

### ADR-013 — Text rasterization strategy

| Closed issue | 💬 | Outcome | Why it maps |
|---|---|---|---|
| [#9403](https://github.com/zed-industries/zed/issues/9403) | 16 | completed | "Better text rendering" — umbrella issue closed once the bundle of fixes landed. Read their resolution comments for the rasterizer-mode decision tree. |
| [#25195](https://github.com/zed-industries/zed/issues/25195) | 16 | completed | Listed again — text-blur on fractional scaling is *both* DPI and rasterizer-mode. |

### ADR-014 — Software rendering / frame budget

| Closed issue | 💬 | Outcome | Why it maps |
|---|---|---|---|
| [#7413](https://github.com/zed-industries/zed/issues/7413) | 18 | completed | Frame rate capped to 60 fps on Mac ProMotion — the inverse symptom of GPUI #45897, but the same machinery: the event loop's notion of "target frame interval". Their fix is the model for our ADR-014 action item 2. |
| [#27500-family on hardware vs software paths](https://github.com/zed-industries/zed/issues/14074) (#14074, 27 comments, not_planned) | 27 | not_planned | "Extremely high GPU usage with nothing open on Linux" — closed as not-planned; root cause is the same as #45897 (software path or compositor pacing). The discussion is the corpus. |

### ADR-017 — Window background blur / transparency

| Closed issue | 💬 | Outcome | Why it maps |
|---|---|---|---|
| [#5040](https://github.com/zed-industries/zed/issues/5040) | 51 | completed | "GPUI window transparency" — long umbrella, eventually closed once the platform branches all landed. Cross-reference for ADR-017 KDE/X11 implementation; the macOS/Windows pieces here are the model. |
| [#19405](https://github.com/zed-industries/zed/issues/19405) | 18 | completed | "Transparency on Windows" — read for the `DwmEnableBlurBehindWindow` / Mica branch. |
| [#10993](https://github.com/zed-industries/zed/issues/10993) | 13 | completed | Listed again — blurry background plus shadow ordering. |

### ADR-019 — Scroll physics

| Closed issue | 💬 | Outcome | Why it maps |
|---|---|---|---|
| [#13720](https://github.com/zed-industries/zed/issues/13720) | 13 | completed | "Scrolling sticks to top when cursor_blink is false" — invalidation + scroll-position interaction. Closed; the discussion documents how `notify` + scroll-offset state interact. Read before implementing the scroll widget. |

### Strategic / roadmap

| Closed issue | 💬 | Outcome | Why it maps |
|---|---|---|---|
| [#7015](https://github.com/zed-industries/zed/issues/7015) | 85 | completed | "Linux Roadmap" — closed after the Linux platform shipped; the comment thread is a record of platform-introduction lessons. Read before serious mobile work (mobile-roadmap.md). |
| [#5395](https://github.com/zed-industries/zed/issues/5395) | 57 | completed | "Linux support" — same family. |
| [#5394](https://github.com/zed-industries/zed/issues/5394) | 36 | completed | "Windows support" — platform-shipping reflection. |
| [#5391](https://github.com/zed-industries/zed/issues/5391) | 17 | completed | "Platform Support 💻" — meta. |

### Not-planned closures worth re-reading

These were closed without a fix; the discussion often explains
why. They are *not* contracts we missed — they are decisions we
might revisit if circumstances change.

| Closed issue | 💬 | Outcome | Why noteworthy |
|---|---|---|---|
| [#14074](https://github.com/zed-industries/zed/issues/14074) | 27 | not_planned | Listed under ADR-014. Closed because root cause was outside Zed's control (compositor pacing). |
| [#14446](https://github.com/zed-industries/zed/issues/14446) | 12 | not_planned | Listed under ADR-005. NVIDIA driver suspend bug; closed because no in-Zed workaround. |
| [#12068](https://github.com/zed-industries/zed/issues/12068) | 12 | not_planned | "Windows & Linux: Support set_dock_menu equivalents" — cross-platform feature, closed without unifying. We may want to make a different choice. |
| [#39806](https://github.com/zed-industries/zed/issues/39806) | 34 | not_planned | RPi OS video memory corruption — hardware-specific; informs ADR-014 software fall-back triggers. |
| [#50198](https://github.com/zed-industries/zed/issues/50198) | 33 | duplicate | OpenSuSE Leap 16 startup failure — closed as duplicate; the duplicate trail is platform-init lore. |

## What this does and does not give us

**Gives:** a curated reading list. For each ADR we now know which
closed upstream issues are likely to contain the actual *fix
narrative* — far more useful than reading 199 closures cold.

**Does not give:** a guarantee that the upstream fix is right for
us. Upstream's invariants are not always our invariants (e.g. our
multi-window contract from ADR-018 differs from theirs). The ADRs
remain the source of truth; the closed corpus is evidence.

## How to use this document

When implementing an action item on an ADR, open the listed
closed issues in this section first. Read the *resolution
comments*, not just the body — closed issues are mostly
discussion threads in their final state.

## References

- [docs/research/gpui-issues.md](gpui-issues.md) — full snapshot including 199 closed.
- [docs/research/gpui-issues-overlay.yaml](gpui-issues-overlay.yaml) — open-issue overlay (closed not enumerated there).
- [docs/research/adr/](adr/) — ADR 001-019.
- [docs/research/flutter-cross-walk.md](flutter-cross-walk.md) — sibling document for Flutter.
