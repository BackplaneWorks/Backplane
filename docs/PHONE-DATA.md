# Phone data protocol (data/1): plain data over one WebSocket, no presenter, no view schema

Agreed between the iOS lane and net (hub side), from the user's design ("we just send the data"). The Rust hub sends data; the SwiftUI app renders it
natively with typed Codable structs, cache-first. Fresh-home screens only: projects, threads, one thread (entries, composer, asks), Improvements.

## Transport
- Discovery: `GET /client/capabilities` (Bearer) gains `"data":{"proto":"data/1"}`; the app uses this protocol when it is present.
- Socket: `ws://host:port/ws?data=1&enc=json&token=<tok>` (hub token or a paired device's; revoke closes the socket). One socket per hub.
  `enc=json`: JSON text frames both ways (the app has no CBOR codec; default without `enc` is binary CBOR, plain string keys).
- First frame: `{"t":"hello","proto":"data/1","hub":"<host:port as asked>","version":"…","epoch":"…","now":<ms>}`; `{"t":"hb"}` every 15 s.
- Request `{"id":<u64>,"m":"…","p":{…}}`; reply `{"t":"reply","id","ok":true,"result":{…}}` or `{"t":"reply","id","ok":false,"error":"text"|{"code","message"}}`.
- Times: unix ms. Ids and cursors: opaque strings. `next:null` = no more.

## Types
```
Project = {id, title, root, threadCount, updatedAt, waiting, icon, color, css}
Thread  = {id, project, title, state, updatedAt, pinned, settled, settledAt, archived, provider, model, preview, waiting, parent}
          state ∈ "idle" | "working" | "asking" | "failed"; preview ≤ 140 chars; waiting = open asks
Entry   = {id, kind, text, at, clientId?, live?, agentOnly?, tool?, ask?}
          kind ∈ "user" | "assistant" | "tool" | "ask" | "note" | "step"
          agentOnly: true on a message only the agent reads (the hub's note about a quote it posted): a client does not show it
          live: true on the assistant entry still streaming (its text so far; later `k:"text"` pushes append to it)
          tool = {name, summary, state ∈ "running" | "done" | "failed"}
          ask  = {id, state ∈ "open" | "answered", question, options:[{id,label}], answered?:<option id or text>}
```
## Reads
`projects.list {limit?=50, before?}` → `{projects, next}` (recent first) · `threads.list {project?, limit?=50, before?, archived?=false, settled?}` → `{threads, next}` ·
`thread.entries {thread, limit?=40, before?}` → `{thread, entries (oldest→newest within the page; newest page first), next}`; the streaming entry (live:true) is included.
`dirs.list {path?}` → `{path, parent, dirs:[name]}` (browse the hub's folders to choose a project; default = home) — for `project.add`.
## Writes
`project.add {path, title?}` → `{project}` · `thread.create {project, title?, text?, provider?, model?, effort?, runtime?, permission?:"ask"|"auto"}` → `{thread}` (with `text` also sends it; default
permission "ask" so approvals appear) · `message.send {thread, text, clientId?}` → `{entry}` (same clientId twice posts once) · `ask.answer {ask, answer}` → `{}` (answer = an option id,
or free text for a question) · `turn.interrupt {thread}` → `{}`.
## Subscriptions
`subscribe {topic:"lists"}` / `{topic:"thread", thread, sinceEntry?}` → `{sub}` · `unsubscribe {sub}`. Pushes `{"t":"d","sub","k",…}`: `thread` {thread} (upsert) · `thread.removed` {id} ·
`project` {project} · `entry` {thread, entry} (appended or whole-replaced by id; a user entry carries the sender's clientId) · `text` {thread, entry:<id>, append, at} · `turn` {thread, state}.
On reconnect the app resubscribes and re-reads lists and the open thread (no resume).

## Additions (2026-11-06, gpui-mobile; `native/host/src/data/ext.rs`)
`hello.features` lists what a hub does beyond the base: `images` (above), `actions`, `search`, `models`, `mentions`, `bots`, `notify`. An app checks the feature before offering the control.
- `thread.act {thread, action: pin|unpin|settle|unsettle|archive|unarchive|delete|rename, title?}` → `{thread}` (delete: `{}`); each is the hub's own `thread.<action>` request, so the desktop and the phone agree. `badge.list` → `{colors:[{name,css}], icons:[name]}` · `project.act {project, action: rename|color|icon|remove, title?, value?}` → `{}` (color: a tint name, icon: an icon name; `detach` on a thread makes it top level).
- `search {q, archived?=false, limit?=30}` → `{threads, projects}` (title, project title, project folder; case-insensitive).
- `models.list` → `{models:{provider:[{value,label,ok,why}]}}` (the hub's `models.all`); `thread.model {thread, provider, model, effort?}` → `{thread}` (the same provider: `thread.modes`; another provider: `thread.provider`, between turns only). A thread's `model` is the one chosen (ModesSet) else the provider's.
- `mentions.list {q?, kind?: @|>|%|#|&, limit?=12}` → `{items:[{sigil,id,label,detail}]}`: bots, threads, projects, rooms, models.
- `bots.list` → `{bots:[{id,name,thread,uber}], rooms:[{id,name,members}]}` (a bot's conversation is its `thread`: read and send it like any thread); `bot.say {bot,text,clientId?}`; `room.posts {room,limit?}` → `{posts:[{id,from,text,at}]}`; `room.post {room,text,clientId?}`.
- Push on a `lists` subscription: `{k:"notify", kind:"needs_you"|"done"|"reminder"|"digest", thread, ask, key, title, body, badge, through}` when the uberbot's gate lets one through, never for a turn's end or an ask by themselves, and `{k:"clear", keys, badge}` when one is dealt with (`phone-push.md`).
- Feature `push`: `device.register {token, platform, kind, env, bundle}`, `device.drop {token}`, `device.prefs {token, mute}`, `push.seen {thread}`, `push.state` → `{keys, badge, quiet, urgent}`, `push.prefs {"notify.quiet"?, "notify.quiet.urgent"?}`.
- Pictures: the agents' `turn.start` keeps `attachments` (a `MessageAttached` event in the same commit) and `agentText`; the message's text carries the `[Attached image: <path>]` lines for the agent and the phone data API strips them from what it shows.
Viewer, files and browser requests are not here: the lanes adding them in `native/host` (viewers, files, cef) define them; the phones will take them as further features.

## Viewers, files, browser (2026-11-06; features `viewers`, `files`, `browser`; `native/host/src/data/ext.rs`)
Each is the hub's own request (kicad.watch, files.dir, mech.list, bots.browser, kicad.info), run for the folder of the thread. `viewer.watch` makes plots arrive on this socket, so a phone subscribes to a source the way the desktop does.
- `viewer.files {thread}` → `{root, pro, pcb, sch}`: the project's canonical KiCad files.
- `viewer.watch {thread, kind: board|schematic|3d|3d-bare|3d-full|split|"" (stop), path?, tree?, kept?}` → `{root}`. `kept`: the 3D model the phone has cached for this view, as `<key>\t<id>` (its plot key and FNV-1a 64 of its `mesh` bytes, 16 hex digits, `plot::mesh_id`; an older phone sends the id alone); the hub skips sending that model for that key only. The reply's `keys` are the plots this view shows: a phone shows only plots with those keys and drops what comes late for the view it left (an older hub sends no `keys`: the phone derives them from `root` by the hub's `kind|root|path` rule). A `{t:"plot", key, none}` comes as JSON on the data socket (why a view has nothing to show). Then, until another watch or a close: a **binary** CBOR frame per change, the hub's own (`kicad/plot.rs`): a plot `{t:"plot", key, fam, base, box, edge, thick, cs:[index of a chunk the phone holds | {s,l,c,a,m,w,d}]}` (the delta against what this connection holds; chunks are varint paths in micrometres, `m` 0 fill / 1 stroke / 2 dot), a model `{t:"plot", key, box3, n, mesh}` (triangles as zigzag-delta varints, 10 µm), or `{t:"plot", key, none:"why"}`. Keys: `0|root|path` board, `1|…` schematic, `2|…` 3D model (`3|` bare), the board's plot rides under its model. A path is from the project's root (a STEP for Mech: kind `3d`).
- Parts of a board's 3D model (`2|…`) stream after the bare board when the hub makes the model itself (`kicad/pcb3d`, the default; setting `viewer.board3d` = `kicad-cli` turns it off, and then the model has its parts in it as before). Two more binary frames, under the model's key:
  - `{t:"plot", key, parts:{of, rev}}` starts the parts again: drop every part held for `key`. `of` instances will come (0: none); `rev` is the board file's revision they are of. It always follows a board `mesh` or, when the phone's `kept` model was the board, the watch.
  - `{t:"plot", key, part, n, mesh, at, refs, done}`: one model and every instance of it. `part` is the model's id (16 hex digits); `n` and `mesh` are its triangles in the model's own frame, encoded as a board's `mesh` (10 µm integers, board axes); `at` is a byte string of 48 bytes an instance: 12 little-endian f32, a row-major 3×4 affine that takes the mesh's integer coordinates to the board model's (`x' = a0·x + a1·y + a2·z + a3`, then the next row for y', z'); `refs` the instances' references, in order; `done` how many instances have now been sent (the progress is `done`/`of`). A part whose model is missing or would not convert comes the same way, as a box over its courtyard (an id starting `b`).
  - Each instance fades in, opacity 0 to 1 over 300 ms from its arrival (redraw only while one is fading). A tap still picks from the board's plot, so refs work before a model arrives.
- `viewer.info {key, at, idx}` → `{info}`: what the piece is (`kind;net=…;layer=…;width=…`), `at` the chunk's place in the plot held, `idx` the piece's info index; `viewer.probe {key, name, value}` → `{matches:[[at, idx]]}` (net or part highlighting).
- `files.entries {thread, path?}` → `{dir, entries:[{dir, size, name}]}`; `mech.list {thread}` → `{parts:[path], renders:[path], primary}`; bytes of a file: `GET /files/get?thread=&path=&token=` (a path outside the project is 400).
- `browser {thread, op, …}` → `{}`: `takeover`, `handback`, then `click {x, y}` (0..1000 of the page), `type {text}`, `press {key}`, `scroll {dx, dy}`, `navigate {url}`, `toolbar {command: back|forward|reload}`; without the takeover the input is refused ("take over the browser first"). The page's newest frame is `GET /bots/<thread>/screen.jpg?token=` (404 until the helper writes one), a new one announced as `{"t":"shot","bot":<thread>,"n":N}` (JSON, text frame).

## Diff and Review (features `diff`, `review`; `native/host/src/data/ext.rs`)
- `viewer.diff {thread, from?, to?}` → `{diff, nogit?}`: the hub's `git.diff` as unified text, by default the thread's checkpoint (made when its first turn started) against the work tree now; `nogit` names the folder when it is not a repository.
- `review.page {thread, page: bom|checks, a?, b?, commit?}` → `{report:{md, err, gate, line, tasks:[{text, done, group}]}}` (`review.bom` / `review.checks`: Markdown the hub writes, the design checks' gate line and the checklist's boxes); `review.tick {thread, index}` → `{ticked:{tasks}}`.
- `review.comment {thread, op: add|edit|resolve|reopen|drop, text?, id?, parent?, file?, layer?, x?, y?, item?}` → `{}`; `review.list {thread}` → `{comments:[{id, text, by, at, parent, resolved, file, layer, x, y, item}]}`: the thread's review acts folded, oldest first (the agent reads the open ones with its next message).
- Layers and Compare (feature `layers`): the phone keeps `off` and `focus` and holds no rule. `viewer.layers {kind: board|schematic, present, off, focus, action?: toggle|pick|mode|cycle|reset, value?}` → `{own, focus, off, dim, lift, layerList, layerMarks, layerHead, layerMode, layerModes, layerReset, layerBits}`: `present` is the layers the plot on show has (a bit each), `own`/`focus` what to send next, `off`/`dim`/`lift` what to draw, `layerMarks` a comparison's rows (show the rows the plot has). `viewer.refs {thread}` → `{refs:[{kind, label, commit}]}`; `viewer.compare {thread, kind, commit, bits?, path?}` → `{root}` watches the Diff of the file against that version, its marks kept to `bits` (`layerBits`: send it again when that changes); `viewer.watch` puts it away.
- Split needs no request of its own: `viewer.watch {kind: "split"}` sends the schematic (`1|root|`), the board (`0|root|`) and the 3D model (`2|root|`) plots on the one socket.

## Fold and picker (features `fold`, `picker`; `native/host/src/data/`)
The phones draw a thread as the desktop does; the words are the hub's.
- Fold: activity arrives flat (paging, pushes and the disk cache stay as they were) in the desktop's words: `tool.name` is its shown name (`viewer_show`, not `mcp__backplane__viewer_show`; `To-do` for a to-do tool), `tool.summary` is empty when it would only repeat the name, a `step` carries `label` ("Design change · board.kicad_pcb"), a `note` carries `label` and `tone`, and the design as it was before an edit is no entry, like a blank reply. A phone folds each run of `tool` and `step` entries into one row (`DataFold.swift`, `DataFold.kt`: pure functions); `test/mobile/fold_labels.json` holds the hub's `present/thread.rs` and both of them to the same words (`cargo test fold_label`, `DataFoldTest`, `scripts/mobile/fold-check.sh`).
- A `Thread` gains `effort`, `runtime` ("" is the default, full access) and `chip` ("claude / opus / high", the composer's chip). `preview` is the last thing said, not the last tool call.
- `thread.picker {thread?, provider?, model?, effort?, runtime?}` → `{picker:{chip, sections:[{title, action, choices:[{label, value, on, ok, why}]}]}}`: what the desktop's picker offers, in order: Model, Effort (when the provider has levels), Provider, Permissions. With `thread` it is that thread's; without, a new thread's from what is given (the hub's default agent, approvals on). A choice with `ok:false` is drawn disabled with its `why`.
- `thread.order {thread}` → `{order}` (feature `order`): a buy thread's Order card, null for any other thread: `{name, steps:[{label, mark: "done"|"on"|""}], line, waiting, total, customs, pay, label}`. `pay` is the Buy now link (an http(s) address to open in the browser), empty unless the quote can be paid; `label` is its button's text. The hub decides all of it; the app asks again when the thread's entries or turn change. An assistant entry's text carries Buy now links ready to open (never `order:<quote>`).
- `thread.pick {thread, action: model|effort|provider|runtime, value}` → `{thread}`: the desktop's own request (`thread.modes`, `thread.effort`, `thread.provider`). A provider changes between turns only; the refusal comes back as the error, to show as it stands.
- `thread.create` also takes `effort` and `runtime` (`runtime` wins over `permission`). `thread.model` stays for older apps.

## Background work and locks (feature `work`; `native/host/src/data/ext.rs`)
- `thread.work {thread}` → `{thread, subs}`: `subs` is the desktop panel's own data or null: `{word, live, rows:[{title, kind, state, doing, act?, value?, label?}]}`. A row with a `label` offers a button; `work.act {thread, act, value}` → `{}` sends it, `act` as the row names it: `lock-release`, `lock-cancel`, `work-cancel`.
- `locks.list` → `{locks:[{name, holder, thread, line, waiting}]}`: every lock with a holder or a queue; `line` is the hub's words, `thread` the holder's to open ("" for a shell helper).
- Pushes: `k:"work"` `{thread, subs}` on a thread subscription, when its rows or any lock move; `k:"locks"` `{locks}` on a `lists` subscription.

## Dictation (feature `voice`; `native/host/src/voice/`, docs/voice.md)
The phone records itself; the engine is the hub's. The hub decides everything else, so the app keeps one number per thread (the mark) and draws what it is sent.
- `voice.start {thread, draft, caret}` → `{path}`: `caret` is in characters. Start the recorder on the press, before the reply; what is heard before the path comes waits for it. An empty `path` means no session started: the `view` frame says why.
- Audio: 16 kHz mono s16le, in order, in about 200 ms pieces, each `POST <path>` with `Authorization: Bearer <token>` and a Content-Length; the last with `&end=1`. A 404 means the session is over: stop.
- `voice.stop {thread}` → `{}`: send what is left with `&end=1` first. The hub waits 3 s for it.
- `voice.act {thread, action, value, draft}` → `{}`: a chip. `action` and `value` come from the view; `voice-fixed`'s value is the text typed; `draft` is the draft now. Also `voice-clear` when the message is sent and the view has fixes or an ask.
- `voice.view {thread}` → `{view}`: read it again after a reconnect.
- `voice.page` → `{rows}`, `voice.page.act {action, value}` → `{}`: Settings, Voice, as rows `{label, note, buttons:[{label, action, value, on}], fields?:[{name, hint, secret, vault?, value}], tone?}`. A press sends the button's `action` and `value`; `voice-term` sends the `vterm` field's text as `value`. A button `secret-save` is the hub's `bots.voice` `api-add`: leave it out of `voice.page.act` and send the `kval` field as `voice.key {key}` → `{}`.

Frames, to the client that started the session (`{"t":"voice","thread","ev",…}`, not a subscription push):
- `view {view}`: draw it. `view` is `{on, state, status, level, note, bad, mic, stop, fixes:[{label, tip, action, value, primary}], ask}`. `on` false ends the session: stop the recorder and forget the mark. `ask` is null or `{text, word, guess, count, fixing, field?:{name, hint, secret, value}, buttons:[{label, action, value, primary}]}`. The strip says `status` and shows `level`, never the words; `note` closes with `voice-err`.
- `level {v}`: 0..100, four times a second.
- `edit {del, ins, adv, to}`: the draft, in characters. With `m` the mark (starts at `caret`; past the end means the end): replace the `del` characters after `m` with `ins`, put the caret at `m + to`, then `m += adv`. Text typed before the mark moves the mark by what was typed.
- `draft {text}`: a chip changed words already there; the draft is now `text`.
- `page {rows}`: Settings, Voice changed (a download's progress).

## Updates

The hub decides what to say; a phone shows it. Restart and rollback are not here: they are the guard's, on the Hub screen below.
- `update.status` → `{current, version, channel, state, note, checked, line, canCheck}`: `line` is the sentence to show, the same one the desktop's Settings shows; `version` is what is available, "" for none; `channel` is `stable` or `beta` (a stored `nightly` reads as `beta`); `canCheck` says whether to offer Check now.
- `update.check` looks now and answers the same; on a hub that updates itself this also installs what it finds once no turn runs. `app.update` asks for an install and answers the same. `update.now` is the same, and goes ahead while turns run (they continue after the restart): Settings offers it as Update now while an update waits for running turns.
- `update.channel {channel}` sets it to `stable` or `beta` and answers the same.

## The Hub screen (`HubRescue.kt`/`.swift`, `HubScreen.kt`, `HubView.swift`)
Restarting the hub must work when its views are broken, the socket is down, or only its guard answers, so this is native UI on plain HTTP to the paired address, and nothing here goes through the socket. The app reaches it from Settings and from the offline banner.
- `GET /healthz`, no credential: 200 `{status: ok|degraded, safe, …}`, 503 `{status: starting|hung|down, …}`; `status:"down"` with `rescue:1` is the guard answering on the hub's address. Every one of these bodies is an answer, a 503 as much as a 200.
- `GET /hub/status`, `Authorization: Bearer <token>`: `{guard:{state, mode, build, text, actions:[{id, label, text?, remote?}]}, incidents:[{at, what, why, did}], hub:{…}, notice:{text, tone, actions}}`. In rescue the same with `rescue:1` and no `hub` or `notice`. `guard:null`: this hub has no guard. Incidents come oldest first and `at` is in unix **seconds**, unlike the rest of this file. `remote:false` on a rollback says the hub will refuse it from a phone; the app draws it anyway and shows the refusal.
- `POST /hub/control {action: restart|normal|safe|rollback}`, the same credential: 202 `{ok:true, text}`; 409 `{ok:false, text}` is a refusal to show as it stands (3 restarts in 5 minutes, a rollback that would lock the person out, no guard); 401 the credential was refused. Another status with a `text` (503 "the guard did not answer") is shown as well.
- The buttons are `guard.actions`, one each; the app adds none of its own unless `/hub/status` cannot be fetched or `guard` is null, when it offers a built-in `restart`. With no guard the hub's reason is control's 409, so the app sends that only on a confirmed tap, never by itself. Each action is confirmed first, with its `text` when it has one.
- After a 202: `/healthz` every 2 s for up to 3 minutes until `ok`, `degraded` or `down`, stopping when the screen goes away and pausing while the app is in the background. The old hub (or rescue) goes on answering for a moment after its 202, so such an answer counts only once the restart has shown (no answer, `starting`, `hung`) or 10 s have passed. A request to `/healthz` gives up after 5 s, the others after 12 s; the credential is not sent on through a redirect.
- `hello` carries `notice:{text, tone, actions}` (`text` empty when all is well, `tone` `warn` or `info`): a thin banner on the lists and the thread opens the Hub screen. `/hub/status` updates it too. The RPCs `hub.status` and `hub.control {action}` exist; the apps do not use them, since they need the socket.
- `test/mobile/hub_cases.json` holds both apps to the same decisions (`HubRescueTest`, `scripts/mobile/hub-check.sh`); `test/mobile/hub_stub.py` plays these routes.

- Feature `settled`: `settled` is the desktop's Settled shelf (a settle by the clock, by hand or by a merge; an unsettle or a new message takes a thread off it) and `settledAt` is when, in ms (0 off the shelf). `Project.settledCount` counts a project's settled threads that are not archived. `threads.list {settled: true}` lists only those, newest settle first (its cursor is `<settledAt>:<id>`); `{settled: false}` lists only the rest, newest activity first, so settled threads never push live ones off a page. Left out, both, as before. A `lists` subscription pushes `thread` on every settle, unsettle, wake and merge, and `project` when a project's thread or settled count moves. A phone shows the active threads under each project and the settled ones behind a "Settled (N)" row, loading them when it is opened.
