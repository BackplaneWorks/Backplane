# Settings

The hub decides Settings' pages, their order and their groups (`native/host/src/present/settings.rs`: `PAGES`, `GROUPS`); the
desktop draws what the menu rows say and sorts nothing.

| Group | Pages |
| --- | --- |
| General | Appearance, Notifications (with Time zone and the webhook), Threads and git (with Git), Mouse and trackpad |
| Agents | Agents (with the failover order and Writing), API keys, MCP servers, Rules and task board, Voice (with Ring), Usage |
| Workspace | CAD, Browser, Ordering |
| Connect | Devices (with Web UI: browser access, forwarding setup, the web address), Machines |
| System | Setup (with Import from 0.14 and Developer), Privacy, About and updates (with Network) |

Pages are by topic. Each menu row carries `group`, `words` (its rows' labels, one a line, from the page as it stands: a row the page does
not show now, such as Install FreeCAD once FreeCAD is found, is not there), `notes` (each label's note on the same line), `also` (plain
words each row is found by on the same line, `ALSO`: what it does or was called, "night" for Quiet hours, "telemetry" for Share anonymous
usage; a test holds that every key is a row) and `aka` (other names the page is found by, one a line: what it was called before, such as
Time zone, Writing or Network and updates, or what it holds, such as Webhook or Tailscale). The desktop's search matches titles, aka names,
labels, notes and also words (this device's rows too, with a small table of their own). While the box holds something the page area lists
what it found ("N found", one line a setting: its label, its note on one line, its page at the right; a page found only by its name is its
own line), and the page list stays whole and in place: pages with nothing found dimmed, the others with their count. A press on a result
(or Enter, on the one Up and Down highlight) opens its page, opens Advanced if the row is under it, scrolls the row into view and lights it
for 1.5 s; the search text stays, and Esc or an empty box returns to the page as it was. Eight pages were folded into their
neighbours: the ids `web`, `ring`, `import`, `developer`, `timezone`, `writing`, `git` and `network` still open Devices, Voice, Setup,
Setup, Notifications, Agents, Threads and git, and About and updates (`canon`: a `settings-page` press, a saved session, a
`backplane://settings/<id>` link). A test (`settings_inventory_is_all_still_reachable`) holds the list of every setting, action and field
the pages had before they were regrouped, so none is lost. The desktop puts this computer's own settings on the page of their topic, under
"This device" above the hub's rows ("Every device", left out where the hub's part opens with a heading of its own, Notifications' Notify me for):
Language, Theme, Text size, Font, the 3D background and the layout on Appearance,
Desktop notifications on Notifications, Streamer mode on Privacy. It adds three pages of its own: Keyboard (before Mouse and trackpad;
the keys that cannot be changed under Advanced), Hubs (after Machines), both kept on this computer, and Accounts (leading Agents). A page
opens at its top. The phones keep their own pages (`devices.page`, `ring.page`, `notify.page`, ...), unchanged by
the regrouping, and the web client its own dialog (`web/src/settings.ts`).

A row marked `"adv": true` is rarely needed: a client folds these under Advanced at the end of the page, and a client that does not
know the mark draws them where they are, last (the hub puts them after every other row, each part in its own order; a heading so
marked comes only before rows so marked). Under Advanced: the webhook (Notifications), Short grace for read conversations and Largest
attachment (Threads and git), each input of Mouse and trackpad under "Board and schematic: each input" and "3D and STEP: each input"
(the page itself shows the zoom direction and the two presets), Bots: infinite context, the failover order and the writing model
(Agents), How eager, End of turn: Jev, Voice commands and Send after silence (Voice), KiCad over its API, Board 3D and Open designs in
(CAD), and Developer (Setup).

Rows follow one pattern. A switch says On and Off (the values stay `on` and `off`). A text box and its button share the row's line, the
box narrowed by the button. A page row the hub marks `"changed": true` lights another choice than a fresh hub would (`mark_changed`: the
page built again from an empty state, rows matched by label); with `"reset": {action, value}` it says the press that puts it back, and the
desktop draws a small mark after the label and Reset beside it. This computer's rows are marked the same way against a fresh install, and a
shortcut when its key is not its default (Shortcuts to defaults puts them back). Setup's rows, which read the disk, are not marked. A row
marked `"intro": true` is a paragraph that opens a group (Notify me for, Webhook, Standing rules, Task board, API keys, Ordering's How it
works, Provider and model for new threads): drawn as a heading with its words under it, not as a setting.

The occasional second click had a reproducible transport cause: a background
`view.update` or `view.changed` could issue a newer presentation revision while
Settings was opening or a same-target `settings-page` action was in flight.
That retired the navigation response's display authority. ThinHost now defers
those refreshes until the Settings action settles, then refreshes with the
accepted session. Existing upload deferral is preserved. Transport identity,
privacy filtering, selection changes and other action behavior are unchanged.

Focused evidence: `desktop/test/thin-host.test.cjs` includes failing-before,
passing-after cases for both Settings open and submenu navigation raced by a
background refresh. `test/tools/settings_review.mjs` runs the emitted production
`settings_fixture` in an isolated browser: thirteen categories, icons,
search/Enter, Japanese labels and a 390 px layout. It does not write live
settings or claim live hub/device validation.

Antigravity installation and sign-in controls live in Settings → Agents on
web, desktop and phones. There is no separate Antigravity category; searching
for Antigravity, Google or Gemini opens Agents. Existing Antigravity settings
links also open Agents.

## Show thinking

Settings → Agents → Show thinking (`agent.thinking`, off until turned on)
asks agents for their thinking and shows it in the thread, folded under
"Thinking" and its first line. Claude is started with
`--thinking-display summarized` (without it its thinking blocks come with
no text); turning the setting either way restarts a thread's Claude
process at its next turn (the setting is part of `Agent.key`). Codex runs
with `model_reasoning_summary="detailed"` and its `reasoning` items are
kept. Pi's `thinking_end` carries a whole thought. Each finished thought is
an `ActivityLogged` of tone `thinking`, logged only while the setting is on;
turning it off hides the ones already logged (web and phones). Thoughts
arrive whole, after the model finishes thinking, not streamed. The ACP
agents (OpenCode, Grok, Oh My Pi, Antigravity) send thinking as chunks that
are not collected yet, and Muse's reasoning is not read.


## Status and empty rows

Rows without controls are informational: web and desktop draw them as plain
text with a left rule instead of another setting card. On iOS they do not
create an empty boxed section; both phone clients dim their labels. Existing
buttons and fields keep their normal treatment throughout Settings.

Voice explains how an empty dictionary and Learned fixes become populated.
Learned fixes are available now: choosing a suggested correction or using Fix
after dictation adds one, and confirming the same correction twice lets it
apply automatically. Existing fixes retain their forget controls.

Local model setup shows starting, bytes received (and a percentage when the
size is known), verification/unpacking, readiness, and errors with a retry.
Unknown model metadata offers Check availability instead of a zero-size
download. The same state and text drive the web, desktop and both phones.
The web/desktop page also has an accessible progress bar and live status.

Regression checks: `test/settings_status_test`, the Voice states in
`test/tools/settings_review.mjs`, and `test/tools/voice_download_e2e.ts` (an
isolated hub with a deterministic helper, including fragmented progress lines,
status racing a download, errors followed by status, retries and helper exit).
