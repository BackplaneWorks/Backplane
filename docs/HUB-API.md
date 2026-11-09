# backplane-hub: the interfaces other streams build on

Code: `native/host/src/api.rs`, `hub.rs`, `model.rs`. Stable names; bodies may grow. Everything is `Arc<Hub>` and safe to call from spawned tasks.

## Events: the one way in
`Hub::commit_many(&[Value]) -> Result<u64>`: group commit (one write, one fsync), then the model indexes them, the presenter folds them, subscribers are told. Returns the log's new length. Ack the person only after it returns. `Hub::commit(&Value)` is the one-event form. Events are the JSON hub writes, field order as there. Single writer: take no lock yourself.

## Reading the world
`hub.model().await` (a `MutexGuard<Model>`, hold it briefly): `threads: HashMap<id, ThreadMeta{project,title,env,branch,worktree,provider,model,session,turn,pending,created,activity,archived,deleted}>`, `projects: HashMap<id, ProjectMeta{title,root,removed}>`, `page(store, thread, before, limit)`, `entries()`. Not yet (M4): queues, settings, asks, tasks. Until then fold the events you need yourself from `subscribe()` / the log; I will add `Model::settings`, `queue(thread)` and `asks` as you name what you need.
`hub.store.lock().await.range(a, b)` reads log lines a..b (0-based); `len()`.

## Listening
`hub.subscribe() -> broadcast::Receiver<u64>`: the log's new length after each commit and after each live delta. Lagging receivers get `Lagged`: re-read from the length you last saw.

## Live deltas (text streamed before it is an event)
`hub.live_delta(thread, text).await`, `hub.live_end(thread).await` (call it before or with the final `MessagePosted` commit). `hub.live_snapshot()` for a client that joins mid-turn. Views show the live text; nothing is written to the log.

## Handlers
```rust
impl RpcHandler for X { fn methods(&self) -> &'static [&'static str]; fn call(&self, hub: &Arc<Hub>, who: &Caller, id: &Value, method: &str, params: &Value) -> Fut<Vec<Value>> }
hub.register_rpc(Arc::new(X))      // later registration wins for the same method: agents replaces the built-in turn.start
impl HttpHandler for Y { fn routes(&self) -> &'static [&'static str]; fn call(&self, hub: &Arc<Hub>, req: &Req) -> Fut<HttpReply> }
hub.register_http(Arc::new(Y))     // GET prefix match; the pairing token is checked first
```
`call` returns the frames to send back (usually one `{"t":"reply","id":id,"ok":true,...}`); errors as `{"t":"reply","id":id,"ok":false,"error":"..."}`.

## Search (what was said)
`session/index.rs` indexes every message (`MessagePosted`, role user or assistant) by its tokens (runs of letters and digits, lower case). It is built from the log once on a thread of its own as the hub starts (a 150 MB log of 465k lines and 20k messages: 0.47 s; a query 1-3 ms) and then reads only the lines appended since, before each query and each checkpoint round; nothing is saved, so no checkpoint can go stale. A query word is looked up in every token that contains its tokens, and the candidates are read back from the log and checked against the whole text: a message is found when its newest text has every word of the query, anywhere, any case (0.14's Find). Queries shorter than two characters find nothing.
- `thread.search {q, thread?, limit?=40}` → `{hits: [{thread, title, project, msg, role, text, at}], more, titles: [{thread, title, project}]}`: newest first, at most 3 a thread across threads (none with `thread`); `text` is a line around the first word; `at` unix s. Deleted threads and ones the sidebar never lists are left out.
- `thread.find {thread, q}` → `{q, matches: [{msg, role, at, text}], total, more}`: find in one thread (Ctrl+F): every matching message, oldest first (the thread's order, for next and previous), at most 1000 (the newest 1000, `more` set, when there are more).
- 0.14's `view.search`, `view.message` and `view.cancel` are retired: no client sends them, a newer query simply replaces an older one, and no message row is ever cut for `view.message` to continue. They answer INVALID_REQUEST like any method a view does not have.

## Unsolicited frames
`hub.push(client, frame) -> bool` (one connection; `who.client` is the id of the caller), `hub.push_all(&frame)`. Frames go out in the connection's encoding (binary CBOR).

## main.rs
One place to start modules, after `Hub::start`: `agents::Agents::start(hub.clone())`, `files::start(hub.clone())`, each registering its handlers. Add `mod agents; mod files; mod kicad;` there and nothing else; I own the rest of `main.rs`/`hub.rs`/`store.rs`/`model.rs`.

## Rules process
Not wired yet. Planned call: one persistent native process, a JSON line in, one out: `{"v":1,"fn":"Fm.safe","args":{...}}` -> `{"ok":..}`; 6.7 us round trip measured (docs/HUB-M1.md). Until then implement the rule in Rust with a corpus test and keep name in the test.

## Agent providers (agents lane, commit 51b4ac53d)

Contract: `native/host/src/agents/providers/mod.rs` (read its docs); reference driver: `providers/codex.rs`.
A provider is a `Driver` built fresh per turn by `providers::driver(provider_id)` (one chain; a lane edits only its own file, each exports `pub fn driver(provider:&str)->Option<Made>`).
The actor owns delivery (ride mail while a turn runs, steer/interrupt), the process (spawn, queues, reaping, stand-in-only in test mode), and the log: every Codex-shaped line a driver returns goes through `Cx::codex_line` (`agents/codex.rs`).
Driver: `argv(&TurnIn)`, `env`, `start` (stdin lines after spawn), `on_line(&Value)->Reaction{send, shapes}` (non-JSON lines arrive as `Value::String`), `on_stop`, `on_exit`, `exit_why(code)` (Proto.gone text), `ends_on_turn_end` (Grok: process ended after turn.completed/failed), `ask_info`/`answer` (permissions).
Permissions: put `{"type":"bp.ask","request":<raw JSON-RPC line string or envelope object>}` in `shapes`; the actor asks `ask_info`, answers `answer(request,"accept")` at once if the tool is allowed for the session, else opens an AskOpened and calls `answer(request, <person's answer>)` later; write the returned lines.
Test one without a CLI: `providers::test_driver` + `a_drivers_permission_reaches_the_person_and_the_answer_goes_back` in `agents/tests.rs`; stand-ins are shell scripts in a temp folder (`Cli::standin`). Run the suite with `sh test/agents/boat-test-job.sh agents::`.
