# Size matrix

Layout bugs (the model picker overlapping and not reflowing) passed every functional row because the rows ran at one window size. This adds a size and text-scale dimension to the e2e, with automatic layout checks, on every client.

## The matrix

`test/e2e/layout/sizes.json`.

| Client | Sizes | Text |
|---|---|---|
| Desktop (Linux, Windows, macOS) | 800×600, 1024×640, 1280×720, 1920×1080, hidpi 2880×1920 (scale 2: a 1440×960 window) | 80, 100, 125 % (`text_pct`, this device's own setting) |
| iOS simulator | iPhone SE (3rd generation), iPhone 16, iPhone 16 Pro Max, iPad (10th generation) | portrait and landscape |
| Android emulator | small 360×640 dp, regular 411×891, large 480×1040, tablet 800×1280 | portrait and landscape |

Linux runs the HiDPI case with a real scale (sway output `scale 2`, or Xft.dpi 192 on Xvfb). Windows and macOS run it at its 1440×960 logical size; the machine's own scale applies.

## What is checked

The client records every painted text run, every named box (`div().id(..)`) and every opaque box (`BACKPLANE_GPUI_AUDIT=1`, `gpui::audit`, a patch in the vendored gpui). The script step `{"audit": "<screen>"}` writes the last frame as an `audit` metric; `test/e2e/layout/check.py` judges it (pure, `python3 check.py` is its self-test):

| Rule | Fails when |
|---|---|
| `overlap` | two text runs share visible area (a run an opaque box covers is dropped first, so a modal does not overlap the timeline under it) |
| `clip-x` | a text run reaches past the left/right edge of what clips it (cut off with no ellipsis, or off the window) |
| `clip-y` | a text run reaches past the window's top or bottom outside a scroll box |
| `offscreen` | a box the screen must keep on screen (a popover) is not entirely in the window |
| `missing` | a box or text the screen must show is not painted (the model list) |
| `min-width` | a box is narrower than its screen's minimum (`picker-list` 160 px) |
| `hscroll` | a box is wider than the window |

Findings already in `test/e2e/layout/known.json` (`rule|text`) are reported but do not fail; anything else fails. A new platform's first run records its baseline with `--allow-known`; the list only shrinks afterwards.

Not seen by the checks: icons and boxes without text (an icon drawn over a label), and the 3D/board canvases. The screenshots and the contact sheet are for those.

## Screens

`scenarios.json`: home with long titles, sidebar fold, the model picker (open), the command palette, thread menu, hub picker, Improvements, Settings (This device, Hubs, Accounts, every hub page), the viewers (Board, Schematic, 3D, Mech, Files, Diff, Review, Terminal) and a thread with a streaming turn.

## Running it

- Here: `scripts/e2e-sizes.sh [--quick] [--sizes ..] [--text ..] [--only ..] [OUT]` (headless sway + grim here; Xvfb + i3 + lavapipe where there is no sway, as in CI). `scripts/e2e-linux.sh` runs it as its `sizes` driver (`E2E_SHORT=1` is the quick set; `NO_SIZES=1` skips it).
- Windows, macOS: `python3 test/e2e/layout/matrix.py --platform windows|mac --client .. --hub .. --assets .. --out ..` inside the desktop session.
- CI: `.github/workflows/e2e-sizes.yml`.
- Output: `OUT/<size>-t<pct>/{shots/*.png,findings.json,audit.jsonl,client.log}`, `OUT/report.md`, `OUT/contact-<size>.png` (one sheet per size).

## The seeded bug

At 1280×720 and text 80 %, before 838cb1a7f the picker's header labels (EFFORT and ↑↓) overlapped, the ☆ buttons overlapped the model names and the model column was about 50 px wide: `overlap` and `min-width` on the `picker` screen. After the fix the screen is clean.
