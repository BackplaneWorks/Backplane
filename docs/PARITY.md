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

| Feature | Status | Owner |
|---|---|---|
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
| Rules and task board from a client (`rules.set/drop`, `board.set/drop`; the agents' side is there) | missing | hub-rest |
| Locks panel (`lock.release`, `lock.cancel`) | unchecked | hub-rest |
| Open in KiCad (`kicad.open`) and KiCad install (`kicad.install`) | unchecked | hub-rest |
| Update now (`update.now`) | unchecked | hub-rest |
| `/btw`, `/give` from every client (`thread.btw`, `bots.give`) | unchecked | hub-rest |
| Summaries and notes (`summary.set`, `thread.note`) | unchecked | hub-rest |
| Worktree controls (`thread.branch`, `thread.local`, `thread.worktree`) | unchecked | hub-rest |
| Google sign-in for bots (`bots.google`, `/oauth/google`) | missing | google |
| `gmail_read`, `gmail_search`, `gmail_draft`, `calendar_events`, `google_accounts` | missing | google |
| `deploy_status`, `deploy_cancel` | unchecked | hub-rest |
| `/viewer-asset`, `/debug/log` | unchecked | hub-rest |
| **Client coverage** | | |
| Japanese in the GPUI client (`i18n/ja.msg` exists; the client loads no catalog) | missing | i18n |
| Notification settings of 0.14 (`notify.*`) | unchecked | audit |
| Old Scratch chats: retired for new threads in 0.14 (38d3b10954) with their chats kept, but `main`'s sidebar does not list the Scratch project. Can a person still reach them? | unchecked | audit |
| `browser.seen` (0.14's client called it; nothing on `main` does) | unchecked | audit |
| `view.search`, `view.cancel`, `view.message` (referenced, not driven) | unchecked | audit |
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
