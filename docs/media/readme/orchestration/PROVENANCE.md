Actual installed Backplane captures, 2026-10-19

Suggested README grouping:

| Role | PNG | Presentation |
| --- | --- | --- |
| Lead image: uberbot, projects, threads, bots | [overview.png](overview.png) | Centered, width 1000; crop of a 2× web frame |
| Delegation detail | [delegation-mobile.png](delegation-mobile.png) | Width 480; real responsive thread with expanded Claude/Grok task rows and Codex composer |
| Persistent bot dashboard | [space-mobile.png](space-mobile.png) | Width 480; real Space tab; pair with delegation detail or place immediately below it |
| Mailbox detail | [mailroom.png](mailroom.png) | Native panel crop, 1336 × 440; link to full raw frame for inspection |

Optional wider alternatives: [delegation.png](delegation.png) and [space.png](space.png), each 1472 pixels wide, suitable at width 736. [webhooks.png](webhooks.png) is a compact optional detail. [space-webapp.png](space-webapp.png) is the separate, responsive HTML webapp actually served by the bot's Space endpoint, suitable at width 480. Prefer the Space-tab image when only one Space image fits: it retains Backplane's cat and tab context.

Raw evidence is in [raw/](raw/). The native high-resolution frame is [native-mailroom-hires.png](raw/native-mailroom-hires.png), 3200 × 2000. It shows the native UI at its ordinary font density on a larger framebuffer, not simulated 2× UI scaling. The readable mail crop comes from the 1600 × 1000 native frame and has not been enlarged. No 2× web Mailbox image is supplied: this installed web bundle has no mail tab. The native tab is actually labelled **Mailroom**, not Mailbox; preserve that wording when describing the screenshot.

What was captured

- Executed `/home/h/.local/share/backplane/backplane` with its installed web assets. No app implementation, build, install, deployment, commit or push.
- Final capture: `2026-10-19T16:38:02Z`, Xvfb `:90`, isolated home `/tmp/backplane-orchestration-d6awvcc3`, localhost port `41029`. The capture app and its Xvfb were terminated by their captured PIDs afterward.
- Actual binary SHA-256: `17e717c9104a77b185016196d1e367303690ea3bea1a68e50bd08254e1db8911`.
- Installed COMMIT label: `e6f8c9a48194d79cf71a9304fb12c57b82bff58b`; installed VERSION label: `0.11.2-dev.20261013.1838`. Those labels are older than the UI's displayed version, `0.11.2-dev.20261018.2043`. They are recorded as labels, not asserted as exact build provenance. The binary and every installed web asset are hashed in [capture.json](source/capture.json).
- Web frames are real Chromium PNG screenshots at deviceScaleFactor 2: 1000 × 720 CSS pixels for desktop and 480 × 720 for narrow variants. No UI stylesheet overrides, browser overlays, compositing, AI imagery or raster enlargement. Final images use only lossless cropping; exact source rectangles and image hashes are in [selection.json](source/selection.json).
- Native frames are the installed app's `BACKPLANE_SNAP` output, losslessly converted from PPM to PNG. Mail navigation reads one fictional message and marks it seen in the disposable home; no reply is sent.
- Everything shown as conversation content, mail, tasks, review results, model selection or project state is a concise synthetic README fixture. No private conversations or real hardware review results are copied. The HTML Space is fixture content, an example static dashboard persisted with `SpacePageSet`, served at `/bots/pixel/space.html` (HTTP 200). It is not a recreated Backplane UI or a claim that this particular dashboard ships by default.
- The registered webhook has a fictional ID, no stored secret, zero calls, and a temporary localhost address. No webhook was invoked.

Isolation and capture audit

`BACKPLANE_HOME`, `--home`, HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_CONFIG_HOME and XDG_CACHE_HOME all use the temporary directory. The app gets a reduced environment, no inherited API keys, `--no-tailscale`, empty BACKPLANE_PEERS, BACKPLANE_ADOPT=0 and BACKPLANE_NO_UPDATE=1. Harness names on the app's PATH resolve to local inert stubs which log their first argument and exit 125. Automatic model/version probes hit those stubs, including `claude -p` and `muse serve`; no real harness is executed in the final run. Final [runtime-audit.json](source/runtime-audit.json) contains no new SessionSet, TurnUsage, running TurnChanged or assistant MessagePosted events. The actual [app log](source/app-log.txt) and [browser text/navigation log](source/browser-log.txt) are included.

Earlier exploratory passes accidentally left completed tasks undelivered. Backplane automatically delivered those task results and started real Codex reply turns against the disposable demo project. Those captures were discarded and replaced. No surviving process belonging to those demo homes was found afterward. The fixture now includes TaskDelivered before startup; final capture additionally isolates HOME and blocks harness executables. Do not describe the whole preparation session as having executed no agent turns. The final published capture run executed none.

Reproduce or edit

From the docs worktree:

```sh
uv run --with pillow docs/media/readme/orchestration/source/capture.py
uv run --with pillow docs/media/readme/orchestration/source/finish.py
```

[Capture script](source/capture.py), [browser navigation](source/capture-web.mjs), [crop script](source/finish.py), [Space HTML](source/space.html), [49 seeded events](source/demo-events.jsonl), and [mail body fixtures](source/demo-mail.json) are editable sources. capture.py regenerates times and temporary paths, so a rerun has fresh fixture and pixel hashes. Animation phases and relative timestamps may differ. [Schema notes](source/NOTES.md) identify the read-only worktrees consulted; no newer or unbuilt source was substituted for installed UI pixels.

Limits

The overview shows this installation's Status tab and global Bots section; it does not demonstrate project-scoped bot nesting. The task capture demonstrates a Codex parent with Claude and Grok child tasks; it does not exercise live delegation. The Space webapp is static demo content served persistently by the real endpoint; it does not demonstrate a deployed backend. The installed web client lacks native Mailroom parity. None of these gaps is filled with a mockup.

Integration handoff

The README integrator visually reviewed and accepted the final overview, delegation, Space, Webhooks and Mailroom crops. No further capture pass is requested. README integration belongs to the integrator. The native Mailroom original remains unchanged here; a browser-enlarged 200% derivative will be documented by the integrator as a derived presentation asset, not a native 2× capture. Its provenance should reference `mailroom.png` and the original SHA-256 in `source/selection.json`.
