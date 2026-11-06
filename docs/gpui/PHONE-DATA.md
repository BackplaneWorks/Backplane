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
`project.add {path, title?}` → `{project}` · `thread.create {project, title?, text?, provider?, model?, permission?:"ask"|"auto"}` → `{thread}` (with `text` also sends it; default
permission "ask" so approvals appear) · `message.send {thread, text, clientId?}` → `{entry}` (same clientId twice posts once) · `ask.answer {ask, answer}` → `{}` (answer = an option id,
or free text for a question) · `turn.interrupt {thread}` → `{}`.
## Subscriptions
`subscribe {topic:"lists"}` / `{topic:"thread", thread, sinceEntry?}` → `{sub}` · `unsubscribe {sub}`. Pushes `{"t":"d","sub","k",…}`: `thread` {thread} (upsert) · `thread.removed` {id} ·
`project` {project} · `entry` {thread, entry} (appended or whole-replaced by id; a user entry carries the sender's clientId) · `text` {thread, entry:<id>, append, at} · `turn` {thread, state}.
On reconnect the app resubscribes and re-reads lists and the open thread (no resume).
