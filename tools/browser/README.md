# backplane-browser

A headless Chromium that Backplane drives, both for the agents' `preview_*`
MCP tools and for the native preview panel, and one page per bot. It is a
thin Bun host adapter over `playwright-core`: it runs requests and
publishes frames. Product logic (which tab, when to show it, how to map
panel clicks) stays in Bend.

```sh
scripts/build-browser.sh              # -> dist/backplane-browser (bun --compile)
dist/backplane-browser --frames /tmp/bp-preview.rgb --profile ~/.x/browser-profile --jpeg-dir ~/.x/shots
bun tools/browser/test/smoke.ts [URL] # end-to-end check (src/main.ts, or $BACKPLANE_BROWSER)
bun tools/browser/test/pages.ts       # pages per owner, the new ops, profile persistence
bun tools/browser/test/record.ts      # the in-page recorder
```

Flags:

- `--frames FILE`: the shared page's raw frame file (default
  `$TMPDIR/backplane-browser-<pid>.rgb`, removed on exit).
- `--profile DIR`: the Chrome profile (cookies, logins, local storage). All
  pages live in one persistent context on this directory, so a login made
  on any page holds on every page and across restarts. Without it the
  profile is a temporary directory, gone on exit. The hub passes
  `<home>/browser-profile`. One Chrome at a time can use a profile.
- `--jpeg-dir DIR`: write every page's frames as JPEG (below).
- `--jpeg-fps N`: at most N JPEG writes a second per page (default 4, 1..30).
- `--chromium PATH`, `--headed` (debugging). `BACKPLANE_BROWSER_DEBUG=1`
  logs frame timings to stderr.

## Chrome

The agents' browser is Chrome. The first request that needs a page
launches one, chosen in this order:

1. `--chromium PATH`, then `$BACKPLANE_CHROME` (or `$BACKPLANE_CHROMIUM`)
2. an installed Google Chrome: `google-chrome-stable`, `google-chrome`,
   `chrome` on `PATH`, then `/opt/google/chrome/chrome`, the macOS
   `/Applications/Google Chrome.app`, or the Windows Program Files install
3. Google's Chrome for Testing in Backplane's cache
   (`~/.cache/backplane/chrome/<version>`; `~/Library/Caches/backplane/chrome`
   on macOS, `%LOCALAPPDATA%\backplane\chrome` on Windows)

With none of these, the helper downloads the current Stable Chrome for
Testing from Google (`googlechromelabs.github.io/chrome-for-testing`) into
that cache, with no root needed, and uses it. `status` reports the path in
`chromium`.

## Protocol

Newline-delimited JSON. One request per line on stdin; one reply per request
and unsolicited events on stdout, one JSON object per line. Nothing else is
written to stdout; diagnostics go to stderr.

```
-> {"id": 1, "op": "open", "url": "example.com", "width": 700, "height": 800}
-> {"id": 2, "op": "open", "page": "bot-7", "url": "news.ycombinator.com"}
<- {"id": 1, "ok": true, "result": {...}}
<- {"id": 3, "ok": false, "error": "locator.click: Timeout 15000ms exceeded."}
<- {"event": "frame", "w": 700, "h": 800, "seq": 3, "path": "/tmp/bp-preview.rgb"}
<- {"event": "frame", "page": "bot-7", "n": 12, "path": "/x/shots/bot-7.jpg"}
```

- `page` names the owner the request acts for (a bot id). Absent or `""`
  is the shared page every agent drives, exactly as before pages existed.
  Each owner's page is made on its first request, in the one persistent
  context. Only a top-level `page` counts; a `page` inside `args` is
  ignored, so tool arguments passed through cannot reach another owner.
- Each owner has its own tabs (its page, plus the popups it opens:
  `window.open` and `target=_blank` join the opener's tabs and become the
  active one), viewport, console log and JPEG frames. Every op below acts
  on the owner's active tab.

- `op` names the request. Arguments sit beside it (or inside `"args": {}`).
- `id` is echoed back as given (any JSON value). Requests run concurrently,
  so replies may come out of order; match them by `id`.
- `error` is one line. An unparseable line answers `{"id": null, "ok": false,
  "error": "bad JSON"}`; an unknown op answers `unknown op: X`.
- EOF on stdin, SIGTERM, or `close` without `page` shuts the browser down
  and exits. (Backplane's Bend side always sends `page`, so an agent's
  `close` closes only its page.)
- `timeoutMs` (where accepted) defaults to 15000 and is capped at 60000.
- URLs: absolute URLs (`https:`, `http:`, `file:`, `data:`, `about:`) pass
  through; a schemeless host gets `https://`, a loopback host
  (`localhost`, `127.0.0.1`, `[::1]`, `0.0.0.0`, `*.localhost`) `http://`.
- Coordinates are viewport CSS pixels; the viewport has device scale 1, so
  one CSS pixel is one frame pixel.

### Requests

| op | arguments | result |
|---|---|---|
| `status` | | Status |
| `open` | `url?`, `width?`, `height?`, `timeoutMs?` | Status. Launches Chromium and the owner's page if needed, sets the owner's viewport, navigates (waits for `load`) when `url` is given. Default viewport 1280x800. |
| `navigate` | `url`, `readiness?` (`"load"` default, `"domContentLoaded"`, `"none"`), `timeoutMs?` | Status |
| `back`, `forward`, `reload` | `timeoutMs?` | Status |
| `resize` | `width`, `height` (1..4096) | Status. With frames on, a fresh frame follows. |
| `click` | a target (below) or `x`, `y`; `button?` (`left`/`right`/`middle`), `clickCount?`, `timeoutMs?` | `{}` |
| `mouse` | `type` (`move`/`down`/`up`), `x?`, `y?`, `button?` | `{}`. For drags from the panel. |
| `type` | `text`, a target?, `clear?`, `timeoutMs?` | `{}`. Focuses the target (or clears it with `clear`), then inserts `text` literally. Without a target it types into the focused element. |
| `press` | `key` (Playwright key name: `Enter`, `Escape`, `Tab`, `ArrowDown`, `Backspace`, `a`), `modifiers?` (`["Alt","Control","Meta","Shift"]`) | `{}` |
| `scroll` | `dx?`, `dy?` (aliases `deltaX`, `deltaY`; positive is right/down), `x?`, `y?`, or a target | `{}`. A wheel event at (x, y) (default: viewport centre); with a target, `element.scrollBy(dx, dy)`. |
| `snapshot` | `screenshot?` (bool), `timeoutMs?` | Snapshot |
| `evaluate` | `expression`, `awaitPromise?` (default true) | the value, JSON-serialised by Playwright. Over 64 KB: `{"truncated": true, "json": "<first 64000 chars>"}` |
| `wait_for` | any of: a target (visible), `text` (substring of `document.body.innerText`), `url` (alias `urlIncludes`; substring of `location.href`); `timeoutMs?` | `{}` once all hold, else an error |
| `text` | a target?, `max?` (default 20000, up to 200000), `timeoutMs?` | `{url, title, text, length, truncated}`. The page's readable text: its `main`/`article` when that holds most of it, else the body (or the target's text); blank runs squeezed, cut to `max`. |
| `select` | a target (a `<select>`), and `value`, `values` (list), `label` or `index` | `{selected: [values]}` |
| `upload` | `path` or `paths`, a target? (default: the first `input[type=file]`) | `{files}`. A file input gets the files; any other target is clicked and the file chooser it opens gets them. |
| `download` | `dir`, and a target to click or a `url`; `timeoutMs?` (default 30000) | `{path, url, suggestedFilename, bytes}`. Waits for the download and saves it in `dir` (made if missing) under its suggested name, `name (1).ext` if taken. |
| `screenshot` | `path`, `fullPage?` (default true), a target? | `{path, bytes}`. PNG, JPEG when `path` ends in `.jpg`/`.jpeg`. |
| `pdf` | `path` | `{path, bytes}`. Headless only. |
| `cookies_clear` | `domain?` | `{}`. Clears cookies for every page (one profile). |
| `tabs` | none, or `new` (`true` or a URL), `select` (index), `close` (index) | `{page, active, tabs: [{index, url, title, active}]}` after the change. |
| `record` | `on` (bool) | `{on, page}`. Turns the in-page recorder on or off for the owner (all its tabs, popups too); steps arrive as `record` events. See Recording. |
| `pages` | | `{pages: [{page, url, title, tabs, shot, n}]}`: every owner with an open tab. |
| `frames` | `on` (bool), `maxFps?` (1..60, default 15) | `{on, maxFps, seq, path}`. Turning on publishes a frame at once. Shared page only. |
| `frame` | | the frame event it published (always published, even if unchanged). Shared page only. |
| `close` | with `page`: | `{}`. Closes that owner's tabs and removes its JPEG; the helper keeps running, and the owner's next request opens a fresh page. |
| `close` | without `page` | `{}`, then the helper exits |
| `close_page` | | the same as `close` with `page` (default the shared page) |

A target is one of:

- `ref`: a ref from the snapshot outline, e.g. `"e5"` (`aria-ref=e5`)
- `locator`: a Playwright selector, preferably role/text based, e.g.
  `role=button[name='Send']`, `text=Continue`, `textarea[placeholder*='Message']`
- `selector`: a CSS selector

A locator or selector acts on its first match.

Status:

```json
{"version": "2", "page": "", "open": true, "chromium": "/path/to/chrome", "profile": "/x/browser-profile",
 "url": "https://example.com/", "title": "Example Domain", "loading": false, "width": 700, "height": 800,
 "tabs": 1, "frames": {"on": true, "maxFps": 15, "seq": 3, "path": "/tmp/bp-preview.rgb"},
 "shot": {"path": "/x/shots/shared.jpg", "n": 5, "maxFps": 4}}
```

`frames` is always the shared page's raw frames; `shot` is the owner's
JPEG (null without `--jpeg-dir`).

Snapshot:

```json
{"url": "...", "title": "...", "loading": false,
 "visibleText": "first 20000 chars of body.innerText",
 "interactiveElements": [{"tag": "button", "role": null, "name": "Go", "selector": "#go",
                          "x": 190, "y": 176, "width": 34, "height": 21}],
 "outline": "- heading \"Backplane browser\" [level=1] [ref=e2]\n- textbox \"Search\" [ref=e4]\n- button \"Go\" [ref=e5]\n...",
 "console": [{"level": "error", "text": "...", "timestamp": "2026-09-22T12:00:00.000Z"}],
 "screenshot": {"mimeType": "image/png", "data": "<base64>", "width": 700, "height": 800}}
```

`outline` is Playwright's AI aria snapshot (YAML-like, one node per line,
with `[ref=eN]` on each). Refs stay valid until the next snapshot.
`interactiveElements` (up to 200) are visible `a[href]`, `button`, `input`,
`textarea`, `select`, `[role]`, `[tabindex]` with viewport boxes. `console`
is the last 50 console messages and page errors. `screenshot` is present
only when asked for.

### Events

| event | fields | when |
|---|---|---|
| `ready` | `version`, `frames` (the frame file path), `profile`, `jpegDir` | once, at start |
| `frame` | `w`, `h`, `seq`, `path` (no `page`) | a new raw frame file of the shared page is in place |
| `frame` | `page`, `n`, `path` | a new JPEG of that owner's page is in place (with `--jpeg-dir`) |
| `navigated` | `page`, `url` | an owner's active tab committed a navigation |
| `load` | `page`, `url` | an owner's active tab fired `load` |
| `record` | `page`, `step` | a step the owner's recorder saw while `record` is on: `{do, target?, value?, key?, url, at, frame}` |
| `closed` | | Chromium went away (crash or close); the next request relaunches it |

## Recording

`record {on:true}` (owner page only) starts the recorder for that owner's
tabs, including popups adopted later; `{on:false}` stops it, and closing
the owner or the browser does too. One binding (`__bpRec`) and one init
script live on the context; the script is inert until its owner records
(documents ask the helper on load, open ones are told). The helper ignores
reports from owners that are not recording and rebuilds each step from known
keys. Steps come as `{"event":"record","page":OWNER,"step":S}`, each with
`at` (ms epoch) and `frame` (the frame's URL, `""` for the main frame):

| `do` | fields |
|---|---|
| `click` | `target`, `url` (capture phase, nearest `a`/`button`/`input`/`select`/`textarea`/`label`/`summary`/`[role]`/`[onclick]`/`[tabindex]`) |
| `type` | `target`, `value`, `url`: one step per field edit (on `change`, or before Enter/Tab), the final value |
| `select` | `target`, `value`, `text`, `url` |
| `press` | `key` (`Enter`, `Tab`, `Escape` only), `target`, `url` |
| `submit` | `target`, `url` |
| `navigate` | `url`: main-frame navigation of the active tab, not repeated for the same URL (from the helper, not the page) |

`target` is `{role, name, label, placeholder, text, testid, css, nth, near,
tag, type}` (strings, `nth` a number; `""` when unknown): role and accessible
name, the field's label, `data-testid`/`data-test`/`data-qa`, a short stable
CSS selector (a `#id` unless it looks generated, else tag, stable classes
and `:nth-of-type` up to four levels), `nth` among elements with the same
role and name, and the nearest preceding heading or labelled landmark.

Masking, done in the page before anything is sent: a field is secret when
it is `type=password`, its `autocomplete` has `password` or `one-time-code`
or starts `cc-`, or its name, id, label, placeholder or aria-label matches
`card|cvv|cvc|iban|ssn|social security|\bpin\b|otp|2fa|one.?time|security
code|passcode`. A secret's step is `{do:"type", target, secret:true, label}`
with no `value` key; its value is also kept out of every target's `text` and
`name`. Real clicks, keys and submits are recorded; of scripted events only
`change` is (Playwright's `selectOption` fires one).

## Frames

Frames are raw pixels, because the Bend side cannot decode PNG or JPEG
cheaply. The helper takes Chromium's screencast (`Page.startScreencast`,
PNG, pushed by the compositor only when something repaints), decodes the
PNG itself (`src/png.ts`: zlib inflate plus filter reconstruction), and
writes the pixels to the frame file.

The frame file is 16 bytes of header and then the pixels:

| offset | size | |
|---|---|---|
| 0 | 4 | `BPF1` |
| 4 | 4 | width, u32 little-endian |
| 8 | 4 | height, u32 little-endian |
| 12 | 4 | seq, u32 little-endian |
| 16 | w*h*3 | RGB, 8 bits per channel, rows top to bottom, no padding |

Each frame is written to `FILE.tmp` and renamed over `FILE`, so a reader
that opens `FILE` always gets one whole frame, never a torn one, and needs
no locking: plain `open` + `read` of the whole file. The header makes the
file self-describing, so a reader that is one frame behind (or reads after a
resize) still decodes it correctly; `seq` tells it which frame it has. The
`frame` event carries the same `w`, `h`, `seq`; a reader may skip reading
when `seq` has not moved.

Pacing: while frames are on, a timer runs at `maxFps`. Each tick takes the
latest screencast frame, if one came since the last tick (older ones are
dropped), decodes it, and hashes the pixels; it writes and announces only
when the hash differs from the last published frame. A still page costs
nothing and emits nothing. `frame`, `resize` and turning frames on publish
at once, even if unchanged.

In Bend (`src/gfx/bitmap.bend`):

```python
f : File <- IO.try(File, File.open(path, "r"))
r : File & Result<&1, &1, U32 & String, List<&2, U32>> <- File.read_bytes(f, 100000000)
# ... bytes bs
+bm = Bitmap.load(bs, x0, y0, fw, fh)          # header + pixels -> Bitmap
+img = Bitmap.over(W, H, bm, base)              # composite into a W x H frame
```

A 700x800 frame is 1,680,016 bytes. The helper's decode and write take
about 5-20 ms a frame.

## JPEG frames

With `--jpeg-dir DIR`, every owner's active tab runs its own screencast
(`Page.startScreencast`, JPEG, quality 70, at most 1280 wide), which Chrome
pushes only when the page repaints; after `open` and `navigate` one frame
is also taken by hand, since a still page may have painted before its
screencast began. The newest frame is written to
`DIR/<owner>.jpg` via `DIR/<owner>.jpg.tmp` and a rename, at most
`--jpeg-fps` times a second (a frame arriving early waits for its slot;
older ones are dropped; a frame identical to the last is skipped), and
announced with `{"event":"frame","page":<owner>,"n":<seq>,"path":...}`.
`n` counts that owner's writes from 1 in this helper run.

The file name is the owner mapped to `[A-Za-z0-9_-]` (every other
character becomes `_`); `""` is `shared`. Distinct owners that map to the
same name share a file, so owners should already be safe names (bot ids
are). Switching tabs moves the screencast to the new active tab. The
shared page has both: raw RGB for the native window and JPEG for web and
phone clients (two screencasts on one page, one per CDP session).

## Choices

- Why a file and not fd 3 or stdout: a Bend program reads files with the
  stock `File` effects; a pipe would need a custom effect and framing, and a
  slow reader would block the helper. With rename, the reader never blocks
  the writer and always sees the newest whole frame.
- Why PNG and not JPEG: lossless (text stays crisp), and decoding needs only
  zlib, which Bun has; no extra dependency.
- `chromium-bidi` is left external in the compiled binary (Playwright only
  needs it for Firefox/BiDi).
