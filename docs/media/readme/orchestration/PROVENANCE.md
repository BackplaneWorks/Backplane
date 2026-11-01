Current Backplane README captures — 2026-11-01

Build: **0.13.2-dev.20261101.1606**, main **36e6a1024**
(full commit `36e6a102411eac368f9ad458fb23167225786d27`).
The supplied `scratch/readme/app/backplane-serve` and `app/web` were used as-is.
No builds, app-source edits, installation, commits or pushes.

Published captures

- `overview.png` — 2000 × 1050: Mochi's Mailroom → Status, Sensor node and Gateway projects, their threads, Tofu and Pixel. The built-in Scratch thread appears on the current **Conversations in progress** shelf rather than under an empty Scratch project heading.
- `delegation.png` — 1472 × 1020: Coordinate rev C, expanded **2 done** children (Grok layout and Claude/Sonnet power review), and the synthetic conversation. The current header lives above the sidebar; a lossless composition joins its 1472 × 72 strip to the corresponding main content crop. No UI was redrawn.
- `webhooks.png` — 1472 × 460: Tofu's Webhooks tab, fictional `demo-review-hook`, **0 calls**. This build displays `/hook/demo-review-hook` as a relative path; it resolves on the isolated localhost hub at port 47820. No webhook or token-generation action was invoked.
- `mailroom.png` and `../mailroom-200.png` — both 2672 × 1128, byte-identical: actual current web Mailroom at 2× density, three fictional conversations and the open Gateway handoff body. The old native Mailroom and nearest-neighbor enlargement pipeline are replaced. The crop is taller to retain the current filters and all three conversations.
- `space.png` — 1472 × 1050: Pixel's persistent structured Rev C control desk.
- `space-webapp.png` — 960 × 1440: the unchanged demo `space.html`, served with HTTP 200 from `/bots/pixel/space.html`, viewport 480 × 720 CSS pixels.

Method and fixtures

Headless Chromium through the brief's Playwright installation, `deviceScaleFactor: 2`.
Every final image was opened and compared with its old counterpart. Only exact
PNG crops and the documented delegation composition are used; no scaling,
stylesheet overrides, UI overlays, image generation or recreated app chrome.
Only the narrow Space image has an explicit README presentation width (480);
the other assigned image tags have no width attribute in either language.

[Capture driver](source/capture.py), [isolated hub](source/current-hub.py),
[browser navigation](source/current-capture.mjs), [selection](source/current-finish.py),
and [audit](source/current-audit.py) are the current reproduction scripts.
Run `python3 docs/media/readme/orchestration/source/capture.py` from the copied repo.
It creates a fresh `scratch/readme/run/B-recapture-*/home`, then stops its hub
and namespace in a `finally` block. It never reads or seeds from a live home.

The original [demo events](source/demo-events.jsonl), [mail bodies](source/demo-mail.json)
and [Space HTML](source/space.html) are reused. The generated replay is
[current-events.jsonl](source/current-events.jsonl): fresh timestamps and copied
project roots, ordering/telemetry off, no pending gateway question, plus one
completed synthetic Scratch notes thread to show the current shelf. Both
TaskDelegated records have TaskDelivered and done state before startup.
There are no running turns, queued messages, routines or undelivered tasks.
All review claims are illustrative fiction.

Isolation and audit

One hub and one browser at a time, port 47820, a user/network/UTS namespace with
only loopback up, neutral hostname `desktop`, fresh HOME/config/cache/provider
folders, no inherited credentials or desktop display. `BACKPLANE_AGENTS` names
inert executable stand-ins for every provider. A synthetic `tailscale` stand-in
returns only `desktop` and no peers; the real daemon is not queried in the final
capture run. Ordering points at `127.0.0.1:9`, updates and KiStack sync are off,
peers are empty, and the vault uses the disposable file keyring.
No Send, Reply, Run, webhook, routine, or real agent action was performed.
Opening the fictional mail marks it seen only in the disposable home.

[current-runtime-audit.json](source/current-runtime-audit.json) verifies zero new
SessionSet, TurnUsage, running TurnChanged, TaskDelegated or assistant MessagePosted
events, both tasks delivered/done, exact pixel equality with the source frames,
and identical Mailroom files. Model/version probes reached the inert stand-ins.
[current-capture.json](source/current-capture.json) records build, hashes, viewports,
fixture and web-asset hashes. [Current selections](source/current-selection.json)
record every crop rectangle and final hash. Final logs are
[current-hub-log.txt](source/current-hub-log.txt) and
[current-browser-log.txt](source/current-browser-log.txt).
The browser closed normally; each started hub/namespace PID was checked against
`/proc/PID/cmdline` before termination. No Xvfb or desktop window was started.

Historical material

Unprefixed old raw frames, mobile alternatives, `selection.json`, `capture.json`,
`runtime-audit.json` and old logs remain historical evidence from 2026-10-19;
they are not provenance for the six refreshed README captures. The original
[provenance](source/PROVENANCE-20261019.md) is archived verbatim (its relative links
refer to its former location). In particular, its old claim that the web client
lacks Mailroom no longer applies. Its warning about accidental agent turns when
completed delegated results were left undelivered remains relevant: preserve the
TaskDelivered events and stand-in isolation when changing the fixture.
