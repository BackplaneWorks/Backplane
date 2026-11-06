# Phone data protocol (v1): plain data over one WebSocket, no presenter, no view schema

The Rust hub sends data; the SwiftUI app renders it natively with typed Codable structs, cache-first. Fresh-home screens only: projects, threads, one
thread (entries, composer, asks), Improvements. Written by the iOS lane with the user's design ("we just send the data"); net owns the hub side.

## Transport
- Discovery: `GET /client/capabilities` (Bearer token) gains `"data":{"version":1}`. The app uses this protocol when it is present.
- Socket: `GET /ws?token=<tok>&data=1` (token = hub token or a paired device's). No log stream, no view lane. One socket per hub, JSON text frames (UTF-8).
- Greeting, sent at once: `{"t":"data","version":1,"hub":"<host:port key>","now":<ms>}`.
- Request: `{"id":<int>,"m":"<method>","p":{...}}`. Reply: `{"t":"reply","id":<int>,"ok":true,"result":{...}}` or
  `{"t":"reply","id":<int>,"ok":false,"error":{"code":"<CODE>","message":"<text>"}}`.
- Push (hub → app, unsolicited): `{"t":"d","k":"<kind>", ...}` (below). Pushes go only for what the app subscribed to.
- Times are integer milliseconds since the epoch. Ids are opaque strings. Cursors are opaque strings (null/absent = the first page).

## Types
```
Project = {id, name, path, activity:int, threads:int}                 // threads = live (not archived) count; activity = newest thread activity
Thread  = {id, project, title, state, activity:int, provider, model, preview}
          state ∈ "idle" | "working" | "ask" | "failed"                // "ask": waiting for the person
          preview = the last assistant text line, ≤ 160 chars ("" when none)
Entry   = {id, kind, text, at:int, clientId?:string, tool?:{name, summary, status}}
          kind ∈ "user" | "assistant" | "tool" | "note" | "error"
          tool.status ∈ "running" | "ok" | "error"; tool rows have text = "" or a short result line
Ask     = {id, kind, title, detail, options:[{id,label}]}              // kind ∈ "permission" | "question"
```

## Methods
| method | params | result |
|---|---|---|
| `projects.list` | `{limit?:int=50, cursor?:string}` | `{projects:[Project], next:string\|null}` most recent activity first |
| `threads.list` | `{project?:string, limit?:int=50, cursor?:string}` | `{threads:[Thread], next:string\|null}` most recent first; `project` omitted = all |
| `thread.entries` | `{thread, before?:string, limit?:int=40}` | `{thread:Thread, entries:[Entry] (oldest first within the page), older:string\|null, live:{text}\|null, asks:[Ask]}` newest page first; `before` = the `older` cursor |
| `project.add` | `{path}` | `{project:Project}` (absolute or `~/…` path on the hub's machine; created if missing) |
| `thread.create` | `{project, text?:string, clientId?:string}` | `{thread:Thread}`; a non-empty `text` is sent as the first message and starts the turn |
| `message.send` | `{thread, text, clientId}` | `{entry:Entry}` the stored user entry, carrying `clientId`; the turn starts (or the message rides the running one) |
| `ask.answer` | `{thread, ask, option}` | `{}`; the turn continues |
| `turn.interrupt` | `{thread}` | `{}` |
| `sync.subscribe` | `{thread?:string\|null}` | `{}`; lists are always pushed; entry/live/ask deltas only for this thread (null = none) |
Errors: `NOT_FOUND`, `BAD_PARAMS`, `REFUSED` (text says why), `INTERNAL`.

## Pushes
```
{"t":"d","k":"project","project":Project}                  // added or changed
{"t":"d","k":"thread","thread":Thread}                     // added or changed (state, title, preview, activity)
{"t":"d","k":"gone","thread":"<id>"}                       // archived or deleted
{"t":"d","k":"entry","thread":"<id>","entry":Entry}        // appended, or replaces the entry with this id (subscribed thread only)
{"t":"d","k":"live","thread":"<id>","text":"<so far>"}     // the streaming assistant text, whole so far; coalesced (≤ 10/s); subscribed thread only
{"t":"d","k":"live.end","thread":"<id>"}                   // the live text is final: the assistant Entry follows or came before
{"t":"d","k":"asks","thread":"<id>","asks":[Ask]}          // the open asks (empty = none); subscribed thread only
```
Order guarantee per thread: a `thread` push that moves state to idle comes after the `entry` push of the last message.

## App behaviour (for the hub author)
- Opens: `sync.subscribe {thread:null}` → `projects.list` + `threads.list` (paint from its disk cache first); opening a thread → `sync.subscribe {thread}` →
  `thread.entries`. Reconnect: same socket URL, then the same calls (no resume; lists and the open thread are re-read, the cache shows meanwhile).
- Sends carry a `clientId`; the app shows the message at once and replaces it when the `entry` with the same `clientId` arrives (a resend with the same
  `clientId` must not post twice).
- Paging: older entries are asked for only on scroll to the top.
