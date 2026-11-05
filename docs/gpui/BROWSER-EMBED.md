# How GPUI shows the agent's browser page (term lane, 2026-11-05)

Status: a proposal, nothing ported. The Rust hub already drives the page (`term/browser.rs`: the helper process, `preview_*`, the person's input through `bots.browser`, JPEG frames in `<home>/shots/<owner>.jpg`, `{"t":"shot","bot":<owner>,"n":N}` when a frame changes, `browser.tabs.<page>` info). What is open is only where the pixels appear in the GPUI window.

## Option A: the frames (recommended first)
The Browser tab draws the newest JPEG of the thread's page as an image and sends the person's input as `bots.browser` (click/mouse in thousandths of the frame, type, press, scroll, toolbar). This is what the Android and iOS apps do today, it needs nothing from GPUI beyond drawing an image and capturing pointer/keys over it, it works on Linux and macOS alike, and the helper stays headless Chrome (already tested on Boat). Cost: a few frames a second (`--jpeg-fps`, default 4; the helper's `cast` op raises it to 30 while a client shows the page), no native text selection or IME inside the page. Work: gpui-screens lane (an image element, hit-testing to frame thousandths, the toolbar). Nothing in the hub changes.

## Option B: CEF as a native child window (what the Bend window did)
`tools/cef` (host.cc) puts Chromium pages in native child windows of the Backplane window; the window says its X window (`browser.embed`, read once at start) and where the page goes (`browser.place`, after each frame); the hub forwards show/hide to the helper; a shield over the page swallows the person's pointer while the agent drives (`drive.<page>`). To do this for GPUI the following must be true first, and none is verified:
1. GPUI 0.2.2 (features `x11` on Linux) exposes the window's native handle (an X11 window id, a `HasWindowHandle`) so a child window can be parented to it. On macOS GPUI would need an `NSView` child instead; `browser.embed` carries only an X window id, so macOS has no path today.
2. A child X window paints above GPUI's own surface (GPUI draws with its own GPU swapchain on that window): z-order and resize flicker must be tried on an isolated display.
3. Keyboard focus moves between GPUI and the child window without the shortcuts (Ctrl+J...) being lost.
The Rust hub would then register `browser.embed`, `browser.place`, `browser.seen` and the engine state (`browser.engine.live/.err`, `CefSession`), port `core/cef.bend` (laws `cef_*`) and keep the shield rule. Roughly: a pure port of cef.bend, a CEF helper restart rule, and the three requests; plus a spike in the gpui client. CEF itself is a ~200 MB runtime that is not counted in any memory win (PLAN.md).

## Option C: a platform webview or a second window
A companion window with the existing web viewer, or wry/WebKit in a dock: PLAN.md lists it as the fastest compatibility option; it does not show the agent's headless page, only a second browser.

## Decision needed (gpui-screens lane / lead)
Ship A. Take B only if frame rate or fidelity is a requirement, and then run the spike for point 1 first (print the GPUI window's raw handle on the target machine); I will port the hub side (cef.bend, the three requests, shield) only after that answer. Until then `browser.embed`/`browser.place`/`browser.seen` stay unported and `browser.engine.live` stays "".
