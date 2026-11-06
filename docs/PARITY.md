# Parity with 0.14

Rust-only Backplane (`main`: hub `native/host`, GPUI client `native/gpui`, iOS, Android, no Bend) against the 0.14 release.

Baselines: tag `v0.14.1` (e564f904eb) and tag `main-bend-final` (749c17d8b0). The earlier checklist was `docs/gpui/PARITY.md` on the old `gpui` branch (generated 2026-11-05, 193 rows). The T3 Code comparison moved to `docs/t3code-parity.md`, because `parity.md` and `PARITY.md` are one file on macOS and Windows checkouts.

Status at 8d15bbb8ab, from the source: requests, tools, routes, settings pages and slash commands on both sides.

- **done**: on `main` and working on every client it applies to.
- **partial**: some of it is there. The row says what is not.
- **missing**: not on `main`.
- **unchecked**: code exists, nobody has driven it end to end on the Rust hub. The audit lane moves these.
- **dropped**: gone on purpose. The row says why.

Each lane updates its own rows in the commit that lands the work.

## Lanes

| Owner | Thread |
|---|---|
| voice | t1793986053-98 |
| commerce | t1793986071-105 |
| threads-small | t1793986085-109 |
| cleanup | t1793986102-114 |
| audit | t1793986118-117 |
| threads-deep, helpers, hub-rest, google, i18n | not started |

## Gaps

| Feature | Status | Owner | Audit |
|---|---|---|---|
| **Voice** | | |
| Dictation from the mic, desktop (`Dictate` chip is a dead placeholder in `native/gpui/src/ui.rs`) | missing | voice |
| Dictation from the phones (`VoiceRecorder` posts to `/voice/in`, which the hub does not serve) | missing | voice |
| Whole recordings and Pebble ring notes | missing | voice |
| Voice ops of `bots.voice` (only the API key ops answer; the rest say "unknown voice request") | missing | voice |
| Settings: Voice page (engine, model, mic, dictionary; not in `PAGES`) | missing | voice |
| Personal dictionary, learned words, `voice_dictionary_add` | missing | voice |
| Local engine (Parakeet, sherpa-onnx through a Node addon today) | missing | voice |
| `tools/voice` helper still Bun, README still says "stays in" Bend | partial | voice |
| **Commerce** | | |
| Card and Ready to order data (`fab.look`, `fab.intent`) | partial | commerce |
| Buy it: `order.buy` is not ported (`settings/fab.rs`), so no buy thread starts | missing | commerce |
| Buy threads: tools `order_quote`, `order_status`, `order_ask` (no tool definitions; the hub only knows how to show their asks) | missing | commerce |
| `order_review` and the review gate (no tool, though the agent prompt tells agents to call it) | missing | commerce |
| Order card and steps, on every client | missing | commerce |
| Order watcher, customs line, pilot notice (no hub code; the phones can still draw the last two) | missing | commerce |
| Ready to order bar (`d1df376576` stopped drawing it above the composer; where Buy it lives now is to be settled) | partial | commerce |
| **Thread views** | | |
| Subagent drill-down | missing | threads-deep |
| Your machines, one list over machines | partial | threads-deep |
| Viewers for threads on another machine (`far.view`) | unchecked | threads-deep |
| Real provider logos (the picker draws a coloured initial; no provider artwork found in 0.14's `src/web` either) | missing | threads-small |
| Routine save errors on screen | missing | threads-small |
| Diff and Files toggle closed | missing | threads-small |
| **Bend-era cleanup** | | |
| Phone launcher code (`WebShell`, Classic screens, `mobile/shell`) | partial | cleanup |
| READMEs and docs that still describe `bridge.js`, `src/core`, the web page (`mobile/README.md`, `docs/order.md`, `docs/voice.md`, others) | partial | cleanup |
| `mobile/tests/improvements/bridge.mjs` and other tests of the old bridge | partial | cleanup |
| **Helpers** (goal: only `tools/browser` stays Bun) | | |
| `tools/kipc`: the hub has this in process; `BACKPLANE_KIPC_BIN` is the only way left to the Bun one | partial | helpers |
| `tools/step2glb`: FreeCAD comes first, Bun and OpenCascade WASM are the fallback for machines without FreeCAD | partial | helpers |
| `tools/voice` | partial | voice |
| `tools/platform/windows-path.ts` | unchecked | helpers |
| **Requests and tools with no handler on `main`** | | |
| Rules and task board from a client (`rules.set/drop`, `board.set/drop`; the agents' side is there) | missing | hub-rest | Audit: the four are unknown methods. 0.14 had no screen for them either (callers: `ledger.bend` and `docs/bots.md` only), so this is request parity, not a screen a person lost. |
| Locks (0.14: `lock_acquire`/`lock_release` for agents, Release/Cancel buttons under Background work in the Subthreads panel) | missing | hub-rest | Audit: the Rust hub has no lock at all. `agents/work.rs:42` refuses `work_run` with a `lock` ("named locks are not available in the Rust hub yet"), `lock.release`/`lock.cancel` are unknown methods on the live socket, and no lock tool or lock row is defined, yet the agent prompt (`prompt_text.rs`) still tells agents to use `lock_acquire`/`lock_release`. An agent's MCP call was not driven (stand-ins cannot call tools); the panel itself has no lock rows to show. |
| Open in KiCad (0.14: `kicad-open` button in the viewer bar) and Install KiCad (0.14: `kicad-install` on Settings → CAD) | missing | hub-rest | Audit, build a8dff03746: with a board in the project, the Board viewer shows History, Compare, Layers, Fit and the tabs Board, Schematic, 3D, Mech, Files, Review, Close; no Open in KiCad. Settings → CAD shows the KiCad path field, FreeCAD "was not found" and the API switch; no install button. `kicad.open`, `kicad.install` are unknown methods. Also seen: the KiCad line stayed "checking" for 10 s while FreeCAD answered (the rig's HOME has no KiCad, so it should say not found; the CAD lanes own this). Phones not driven. |
| Update now (`update.now`) | partial | hub-rest | Audit, build a8dff03746 (real mouse and keys on Xvfb): Settings → Network and updates has the channel chips (stable, nightly) and "Backplane 0.15.0, Updated by its package"; About has the same line. No Update now button on either. The hub's `update.check` answers (`{version:"",channel}`); `update.now` is an unknown method; `app.update` is registered. The "package" line follows the hub's `update` info; a hub with an update waiting was not driven, so whether a button appears then is unchecked. Phones not driven. |
| `/btw`, `/give` from every client (`thread.btw`, `bots.give`) | unchecked | hub-rest | Audit, build e9a7d6c9ba on a private hub (Xvfb, stand-ins): in the GPUI composer both echo the message, then the screen says `View request refused: "PRESENTER_UNAVAILABLE"`; nothing is posted and the text stays. `thread.btw` and `bots.give` are unknown methods. `session/composer.rs:80` returns no handler for `/btw`, `/give`, `/secret`, `/intent`. To confirm on a current build; `/secret` and `/intent` not driven; phones not driven. |
| Summaries and notes (`summary.set`, `thread.note`) | unchecked | hub-rest | Audit: both unknown methods, and in 0.14 only the server's own job replies sent them (`server.bend:2897`, `12468`), never a client, so the requests are dropped on purpose. Whether a thread summary or note still shows to a person on `main` was not driven. |
| Worktree controls (`thread.branch`, `thread.local`, `thread.worktree`; also `thread.merged`, `thread.unrooted`, `thread.titled`) | partial | hub-rest | Audit, build a8dff03746 (real mouse and keys on Xvfb): in a git project the + beside the project made a thread with env `worktree` and the hub logged `WorktreeSet` (branch `backplane/8b05553b`, a real `git worktree`); in a non-git folder `WorktreeSkipped`. The five request names are 0.14's agent-metadata list (`hub.bend:1134`), not client requests, and are unknown methods. Gaps seen: the phones' `thread.create` (data/1) made a `local` thread in the same git project, so a phone thread gets no worktree; no setting or control chooses local or worktree (no `thread.env` row in Settings) or moves a thread between them. |
| Google sign-in for bots (`bots.google`, `/oauth/google`) | missing | google |
| `gmail_read`, `gmail_search`, `gmail_draft`, `calendar_events`, `google_accounts` | missing | google |
| `deploy_status`, `deploy_cancel` | unchecked | hub-rest |
| `/viewer-asset`, `/debug/log` | unchecked | hub-rest |
| **Client coverage** | | |
| Japanese in the GPUI client (`i18n/ja.msg` exists; the client loads no catalog) | missing | i18n |
| Notification settings of 0.14 (`notify.*`: needs, settled, all, reminders, failed, idle) | partial | audit | Audit, build a8dff03746 (real mouse and keys on Xvfb): the hub still reads `notify.reminders` (`bots/routines.rs:116`) and `notify.<name>` (`bots/mail/escalate.rs:81`), but no Settings row sets them: the only control is the device-local Desktop notifications On/Off on This device (the pages read: This device, Network and updates, About; `present/settings.rs` has no `notify` row). Needs a hub page for them; owner hub-rest or the push lane. |
| Old Scratch chats: retired for new threads in 0.14 (38d3b10954) with their chats kept; 0.14's docs say they open "from Conversation history in the sidebar or search" | missing | hub-rest | Audit, build a8dff03746 (real mouse and keys on Xvfb): a Scratch project with chat "zebra scratch chat" next to "zebra thread": the sidebar lists neither the project nor the chat (`sidebar.rs:237` skips Scratch), and the palette, typed "zebra", offers only "zebra thread". Phones not driven. |
| `browser.seen` (0.14's client sent it from its browser-cast code, `client.bend:3811`, `3852`, naming a page) | unchecked | audit | Audit: unknown method on `main`; the GPUI client has no cast. What it acknowledged and whether anything replaced it is not shown. |
| `view.search` (0.14's Find: "Type a word said in any thread", text of messages across threads) | missing | audit | Audit, build a8dff03746 (real mouse and keys on Xvfb): on a valid open view the hub answers `view.search`, `view.message` and `view.cancel` with UNSUPPORTED: no handler is ever passed to `register_view`. A word said in a thread ("pinecone", sent with `message.send`) is not found by the sidebar search, the palette ("Nothing matches") or the phones' `search {q}` (empty; it matches titles and folders only). Proposed owner: hub-rest. `view.message` (0.14: continue a long message) and `view.cancel` are the same missing handlers; whether a long message is cut with no way to read the rest is not driven. |
| Every other row of the old checklist | unchecked | audit |

## Not gaps

| Feature | Status | Why |
|---|---|---|
| `attach.put`, `attach.file` | done | `POST /attach` (`files/attach_http.rs`) replaced them |
| `browser.place`, `browser.embed` | dropped | messages from 0.14's app to its own CEF helper, not hub requests; the GPUI client has its own (`viewers/cef.rs`) |
| `view.open`, `view.update` | done | answered by the hub's view path (`hub.rs`) |
| `view.body`, `view.get`, `view.send`, `view.show`, `view.ver` | dropped | no hub or client on `main` names them |
| `frontend.screen`, `frontend.history`, `mobile.screen`, `mobile.history` | dropped | screens for the old clients; the phones speak data/1 now |
| Web client, Electron | dropped | no web client in this tree |

## Owned elsewhere

| Feature | Lane |
|---|---|
| Floating terminal | t1793985121-51 |
| Performance overlay, `/debug/perf` | t1793985126-56 |
| KiCad layer focus | t1793985132-58 |
| Drawing pad | t1793985137-60 |
| Model picker keys | t1793985143-62 |
| Accounts failover, picker, select API | t1793985226-71 |
| Auto-settle | improvement-afbd8705bfffc64da9d8fb86cb508b0a |
| Lost message while a thread runs | improvement-8e5d0da899ccf3b001f04d5725fe1eed |
| Build speed | t1793983071-17 |
| Onboarding, first-run tour | t1793985644-80 |
| CI | t1793985650-82 |
| Cross-platform E2E, pairing matrix, remote reboot and update | t1793985636-78 |
| `$` skills | t1793985670-86 |
| Hub resilience | t1793985681-88 |
| Embedded browser | t1793950330-21 (finished) |
| Phone tool-call folds and model selection | t1793983480-114135 |
| Phone notifications, push | t1793985890-103 |
| E2E rows in `docs/e2e/MATRIX.md`: Linux with the Settings walk and CAD, macOS, Windows, pairing | t1793985955-52, t1793985958-55, t1793985962-58, t1793985966-62 |

## Rough in 0.14, fix when cheap

Filled in by the lanes as they meet them. A row needs what was rough and what it does now.

| Rough edge | Status | Owner |
|---|---|---|
| Dictation depended on a Bun helper and a Node addon fetched at runtime | missing | voice |
| A dead control looks live (the `Dictate` chip) | missing | voice |
| A failed routine save said nothing | missing | threads-small |
| Provider marks were initials | missing | threads-small |
