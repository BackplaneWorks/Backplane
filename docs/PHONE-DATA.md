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
Project = {id, title, root, threadCount, updatedAt, waiting}
Thread  = {id, project, title, state, updatedAt, pinned, settled, archived, provider, model, preview, waiting}
          state ∈ "idle" | "working" | "asking" | "failed"; preview ≤ 140 chars; waiting = open asks
Entry   = {id, kind, text, at, clientId?, live?, tool?, ask?}
          kind ∈ "user" | "assistant" | "tool" | "ask" | "note" | "step"
          live: true on the assistant entry still streaming (its text so far; later `k:"text"` pushes append to it)
          tool = {name, summary, state ∈ "running" | "done" | "failed"}
          ask  = {id, state ∈ "open" | "answered", question, options:[{id,label}], answered?:<option id or text>}
```
## Reads
`projects.list {limit?=50, before?}` → `{projects, next}` (recent first) · `threads.list {project?, limit?=50, before?, archived?=false}` → `{threads, next}` ·
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
- `thread.act {thread, action: pin|unpin|settle|unsettle|archive|unarchive|delete|rename, title?}` → `{thread}` (delete: `{}`); each is the hub's own `thread.<action>` request, so the desktop and the phone agree. `project.act {project, action: rename|remove, title?}` → `{}`.
- `search {q, archived?=false, limit?=30}` → `{threads, projects}` (title, project title, project folder; case-insensitive).
- `models.list` → `{models:{provider:[{value,label,ok,why}]}}` (the hub's `models.all`); `thread.model {thread, provider, model, effort?}` → `{thread}` (the same provider: `thread.modes`; another provider: `thread.provider`, between turns only). A thread's `model` is the one chosen (ModesSet) else the provider's.
- `mentions.list {q?, kind?: @|>|%|#|&, limit?=12}` → `{items:[{sigil,id,label,detail}]}`: bots, threads, projects, rooms, models.
- `bots.list` → `{bots:[{id,name,thread,uber}], rooms:[{id,name,members}]}` (a bot's conversation is its `thread`: read and send it like any thread); `bot.say {bot,text,clientId?}`; `room.posts {room,limit?}` → `{posts:[{id,from,text,at}]}`; `room.post {room,text,clientId?}`.
- Push on a `lists` subscription: `{k:"notify", kind:"done"|"fail"|"ask", thread, title, body}` when a turn ends (not interrupted) or an ask opens.
- Pictures: the agents' `turn.start` keeps `attachments` (a `MessageAttached` event in the same commit) and `agentText`; the message's text carries the `[Attached image: <path>]` lines for the agent and the phone data API strips them from what it shows.
Viewer, files and browser requests are not here: the lanes adding them in `native/host` (viewers, files, cef) define them; the phones will take them as further features.

## Viewers, files, browser (2026-11-06; features `viewers`, `files`, `browser`; `native/host/src/data/ext.rs`)
Each is the hub's own request (kicad.watch, files.dir, mech.list, bots.browser, kicad.info), run for the folder of the thread. `viewer.watch` makes plots arrive on this socket, so a phone subscribes to a source the way the desktop does.
- `viewer.files {thread}` → `{root, pro, pcb, sch}`: the project's canonical KiCad files.
- `viewer.watch {thread, kind: board|schematic|3d|3d-bare|3d-full|split|"" (stop), path?, tree?}` → `{root}`. Then, until another watch or a close: a **binary** CBOR frame per change, the hub's own (`kicad/plot.rs`): a plot `{t:"plot", key, fam, base, box, edge, thick, cs:[index of a chunk the phone holds | {s,l,c,a,m,w,d}]}` (the delta against what this connection holds; chunks are varint paths in micrometres, `m` 0 fill / 1 stroke / 2 dot), a model `{t:"plot", key, box3, n, mesh}` (triangles as zigzag-delta varints, 10 µm), or `{t:"plot", key, none:"why"}`. Keys: `0|root|path` board, `1|…` schematic, `2|…` 3D model (`3|` bare), the board's plot rides under its model. A path is from the project's root (a STEP for Mech: kind `3d`).
- `viewer.info {key, at, idx}` → `{info}`: what the piece is (`kind;net=…;layer=…;width=…`), `at` the chunk's place in the plot held, `idx` the piece's info index; `viewer.probe {key, name, value}` → `{matches:[[at, idx]]}` (net or part highlighting).
- `files.entries {thread, path?}` → `{dir, entries:[{dir, size, name}]}`; `mech.list {thread}` → `{parts:[path], renders:[path], primary}`; bytes of a file: `GET /files/get?thread=&path=&token=` (a path outside the project is 400).
- `browser {thread, op, …}` → `{}`: `takeover`, `handback`, then `click {x, y}` (0..1000 of the page), `type {text}`, `press {key}`, `scroll {dx, dy}`, `navigate {url}`, `toolbar {command: back|forward|reload}`; without the takeover the input is refused ("take over the browser first"). The page's newest frame is `GET /bots/<thread>/screen.jpg?token=` (404 until the helper writes one), a new one announced as `{"t":"shot","bot":<thread>,"n":N}` (JSON, text frame).

## Diff and Review (features `diff`, `review`; `native/host/src/data/ext.rs`)
- `viewer.diff {thread, from?, to?}` → `{diff, nogit?}`: the hub's `git.diff` as unified text, by default the thread's checkpoint (made when its first turn started) against the work tree now; `nogit` names the folder when it is not a repository.
- `review.page {thread, page: bom|checks, a?, b?, commit?}` → `{report:{md, err, gate, line, tasks:[{text, done, group}]}}` (`review.bom` / `review.checks`: Markdown the hub writes, the design checks' gate line and the checklist's boxes); `review.tick {thread, index}` → `{ticked:{tasks}}`.
- `review.comment {thread, op: add|edit|resolve|reopen|drop, text?, id?, parent?, file?, layer?, x?, y?, item?}` → `{}`; `review.list {thread}` → `{comments:[{id, text, by, at, parent, resolved, file, layer, x, y, item}]}`: the thread's review acts folded, oldest first (the agent reads the open ones with its next message).
- Split needs no request of its own: `viewer.watch {kind: "split"}` sends the schematic (`1|root|`), the board (`0|root|`) and the 3D model (`2|root|`) plots on the one socket.

## Fold and picker (features `fold`, `picker`; `native/host/src/data/`)
The phones draw a thread as the desktop does; the words are the hub's.
- Fold: activity arrives flat (paging, pushes and the disk cache stay as they were) in the desktop's words: `tool.name` is its shown name (`viewer_show`, not `mcp__backplane__viewer_show`; `To-do` for a to-do tool), `tool.summary` is empty when it would only repeat the name, a `step` carries `label` ("Design change · board.kicad_pcb"), a `note` carries `label` and `tone`, and the design as it was before an edit is no entry, like a blank reply. A phone folds each run of `tool` and `step` entries into one row (`DataFold.swift`, `DataFold.kt`: pure functions); `test/mobile/fold_labels.json` holds the hub's `present/thread.rs` and both of them to the same words (`cargo test fold_label`, `DataFoldTest`, `scripts/mobile/fold-check.sh`).
- A `Thread` gains `effort`, `runtime` ("" is the default, full access) and `chip` ("claude / opus / high", the composer's chip). `preview` is the last thing said, not the last tool call.
- `thread.picker {thread?, provider?, model?, effort?, runtime?}` → `{picker:{chip, sections:[{title, action, choices:[{label, value, on, ok, why}]}]}}`: what the desktop's picker offers, in order: Model, Effort (when the provider has levels), Provider, Permissions. With `thread` it is that thread's; without, a new thread's from what is given (the hub's default agent, approvals on). A choice with `ok:false` is drawn disabled with its `why`.
- `thread.pick {thread, action: model|effort|provider|runtime, value}` → `{thread}`: the desktop's own request (`thread.modes`, `thread.effort`, `thread.provider`). A provider changes between turns only; the refusal comes back as the error, to show as it stands.
- `thread.create` also takes `effort` and `runtime` (`runtime` wins over `permission`). `thread.model` stays for older apps.
