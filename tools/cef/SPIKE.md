# browser-cef step 1: CEF child window vs the current helper

2026-10-16. CEF 154.0.34 (Chromium 154.0.8037.98, prebuilt minimal distribution, Alloy style, `SetAsChild`) against the Playwright helper as of backplane/cast-idle (46e19921f, Chrome 152). Both used the same pages at 800x600 on a private Xvfb. There were 3 rounds with the order alternated, 15 s CPU windows and 30 input samples each. The machine was shared, with a load average of 4 to 21 during the rounds. Run it with `tools/cef/bench/bench.ts`.

## Results (mean of the 3 rounds)

| | CEF child window | current helper | |
|---|---|---|---|
| key press → on screen, p50 / p95 | **2.1 / 4.2 ms** | 8.7 / 14.7 ms ¹ | 4x faster |
| wheel notch → on screen, p50 / p95 | **3.1 / 12.0 ms** | 17.0 / 22.3 ms ¹ | 5x faster |
| startup → first page pixels | **~210 ms** | ~470 ms (first frame) | 2x faster |
| still page, shown: CPU | **0.0%** | 1.1% | |
| animated page, shown: CPU | **11.5%** | 54.8% ² | 5x less |
| animated page, shown + a phone watching (CDP JPEG cast, 60 fps) | **35.5%** | 54.8% (30 fps) ² | |
| animated page, hidden / unwatched: CPU | 12.4% | **7.0%** | **CEF worse** |
| memory (PSS, browser process tree) | **~360 MB** | ~450 MB ³ | 20% less |

¹ This measures up to the helper writing its JPEG. The window still has to read, decode and draw it, so the person waits longer than shown here.
² The window's JPEG decode at 30 fps is not counted either, which also favours the helper.
³ This includes the helper's Bun process, which CEF doesn't need. In step 2 the Bun process stays for agents, at roughly 40 to 60 MB.

## Agents on the same page

Playwright `connectOverCDP` to the host's `--port` finds exactly one context and one page: the one on screen. These all worked on it: goto, title, evaluate, fill, role locator click, aria snapshot, screenshot and a JPEG screencast. The connection took 21 to 26 ms. Findings:
- Under Bun 1.3, Playwright's vendored `ws` never completes the handshake. A 25-line adapter over Bun's own WebSocket fixes it, proven against CEF (`tools/cef/bench/bunws.ts`). It needs a one-line build-time patch to playwright-core's `coreBundle.js`.
- Over CDP, `browser.close()` closes the host's browser. The helper must only disconnect, and the host should refuse `Browser.close`.

## The one failure: hidden animating pages

A hidden page that keeps animating costs CEF about 12% CPU, against about 7% for the helper's unwatched page.
- `CefBrowserHost::WasHidden` only works for off-screen rendering.
- Unmapping the child X window does not hide the page from Chromium.
- CDP `Page.setWebLifecycleState frozen` brings it to 0.1%, but it is one-way in windowed CEF. The page stays `document.hidden` after unfreezing, through resize, focus and reload.

Still pages cost nothing either way. This only matters when an unseen page runs animations, such as a bot's page in the background.

Options for step 2, if this must be fixed:
- **(a)** Accept it, since it is close to today's cost.
- **(b)** Use one host browser for the visible owner, and run pages nobody sees as CEF off-screen (windowless) browsers, where `WasHidden` works. This needs a page swap between the two modes.
- **(c)** Keep unseen owners' pages in the existing headless helper and move a page into CEF only when it is shown. This also needs a swap, which loses in-page state.
- **(d)** Patch Chromium's visibility in a host-side DevTools hook. That is unproven.

## Not yet checked (needs the real desktop, step 2)

Japanese IME, clipboard against other apps, drag-and-drop, real GPU compositing (Xvfb used software), and multiple owners' pages. Packaging adds 260 MB for the stripped `libcef.so` plus about 100 MB of paks and locales.
