//! data/1 additions for the phones (features "actions", "search", "models", "mentions", "bots", "notify"): each is a thin map onto the hub's own RPCs and
//! folds, so a phone does what the desktop does and the hub stays the one place that decides. Added requests (docs/PHONE-DATA.md):
//! `thread.act {thread, action: pin|unpin|settle|unsettle|archive|unarchive|delete|detach|rename, title?}` → `{thread}` (delete: `{}`) · `project.act {project, action: rename|color|icon|remove, title?, value?}` → `{}` ·
//! `search {q, archived?=false, limit?=30}` → `{threads, projects}` · `models.list` → `{models:{provider:[{value,label,ok,why}]}}` ·
//! `thread.model {thread, provider, model?, effort?}` → `{thread}` · `mentions.list {q?, kind?, limit?}` → `{items:[{sigil,id,label,detail}]}` (`@` bots, `>` threads, `%` projects, `#` rooms, `&` models) ·
//! `bots.list` → `{bots:[{id,name,thread,uber}], rooms:[{id,name,members}]}` · `bot.say {bot, text, clientId?}` → `{}` · `room.posts {room, limit?}` → `{posts}` · `room.post {room, text, clientId?}` → `{}`.
//! Viewers (feature "viewers", "files", "browser"; the hub's own kicad.watch, files, mech and bots.browser requests, run for the thread's folder):
//! `viewer.files {thread}` → `{root, pro, pcb, sch}` (the project's canonical KiCad files) · `viewer.watch {thread, kind: board|schematic|3d|3d-bare|3d-full|split|"" (stop), path?, tree?}` → `{root}`:
//! the plot frames then arrive on this socket as binary CBOR (`{t:"plot", key, fam, cs, box, edge, thick}`, a 3D model `{key, box3, n, mesh}`: kicad/plot.rs), `{t:"plot", key, none}` as JSON ·
//! `viewer.probe {key,name,value}` → `{matches}`, `viewer.info {key,at,idx}` → `{info}` · `files.entries {thread, path?}` → `{dir, entries:[{dir,size,name}]}` ·
//! `mech.list {thread}` → the hub's listing · bytes of a file: GET /files/get?thread=&path=&token= · `browser {thread, op, ...}` → `{}` (bots.browser for the thread's page: takeover, handback, click {x,y 0..1000},
//! type, press, scroll, navigate, toolbar) · the page's newest frame: GET /bots/<thread>/screen.jpg, a new one announced by `{"t":"shot","bot":<thread>,"n":N}` (JSON, to every client that connected for viewers).
//! Diff and Review (features "diff", "review"): `viewer.diff {thread, from?, to?}` → `{diff, nogit?}` (the hub's git.diff: unified text) · `review.page {thread, page: bom|checks, a?, b?, commit?}` → `{report:{md, err, gate, line, tasks:[…]}}`
//! (review.bom / review.checks: Markdown the hub writes) · `review.tick {thread, index}` → `{ticked:{tasks}}` · `review.comment {thread, op: add|edit|resolve|reopen|drop, text?, id?, parent?, file?, layer?, x?, y?, item?}` → `{}` ·
//! `review.list {thread}` → `{comments:[{id, text, by, at, parent, resolved, file, layer, x, y, item}]}` (the thread's review acts folded, oldest first).
//! Picker (feature "picker"): `thread.picker {thread?, provider?, model?, effort?, runtime?}` → `{picker:{chip, sections:[{title, action, choices:[{label, value, on, ok, why}]}]}}` (a thread's own, or a new thread's from what is given) ·
//! `thread.pick {thread, action: model|effort|provider|runtime, value}` → `{thread}`: the desktop's picker, the same requests it sends.
//! Pushes on a `lists` subscription: `{"k":"notify","kind":"done"|"fail"|"ask","thread","title","body"}` when a turn ends or an ask opens.
use super::*;

pub const FEATURES: &[&str] = &["actions", "search", "models", "mentions", "bots", "notify", "viewers", "files", "browser", "diff", "review", "fold", "picker"];

const THREAD_ACTS: &[&str] = &["pin", "unpin", "settle", "unsettle", "archive", "unarchive", "delete", "detach"];

/// The composer's chip as the desktop draws it: "claude / opus / high".
pub fn chip(provider: &str, model: &str, effort: &str) -> String { format!("{provider} / {} / {}", if model.is_empty() { "default" } else { model }, if effort.is_empty() { "auto" } else { effort }) }

/// The picker the desktop opens from that chip (present/keys.rs), as sections a phone draws in order and answers with `thread.pick`. Models carry the hub's labels and
/// say when one cannot be used; a provider with no list offers its default. Pure: `models` is `settings::models::all`.
pub fn picker_sections(models: &Value, provider: &str, model: &str, effort: &str, runtime: &str) -> Value {
    let k = crate::present::keys::picker(provider, model, effort, "");
    let plain = |list: &Value, strip: bool| -> Vec<Value> { list.as_array().into_iter().flatten().map(|c| { let v = c["value"].as_str().unwrap_or(""); json!({"label":c["label"],"value":if strip { v.trim_start_matches('@') } else { v },"on":c["on"],"ok":true,"why":""}) }).collect() };
    let mut rows = vec![json!({"label":"default","value":"","on":model.is_empty(),"ok":true,"why":""})];
    for r in models[provider].as_array().into_iter().flatten() { let v = r["value"].as_str().unwrap_or(""); if !v.is_empty() { rows.push(json!({"label":r["label"],"value":v,"on":v == model,"ok":r["ok"],"why":r["why"]})) } }
    if !model.is_empty() && !rows.iter().any(|c| c["value"] == model) { rows.push(json!({"label":model,"value":model,"on":true,"ok":true,"why":""})) }
    let efforts = plain(&k["efforts"], false);
    let mut sections = vec![json!({"title":"Model","action":"model","choices":rows})];
    if !efforts.is_empty() { sections.push(json!({"title":"Effort","action":"effort","choices":efforts})) }
    sections.push(json!({"title":"Provider","action":"provider","choices":plain(&k["providers"], true)}));
    sections.push(json!({"title":"Permissions","action":"runtime","choices":plain(&crate::present::keys::permissions(runtime)["choices"], false)}));
    json!({"chip":chip(provider, model, effort),"sections":sections})
}

async fn picker(hub: &Arc<Hub>, p: &Value) -> Result<Value, Value> {
    let th = s(p, "thread");
    let now = if th.is_empty() { Value::Null } else { thread_now(hub, th).await? };
    // a new thread: what the phone has chosen so far, else the hub's default agent, and approvals on (the phones' default)
    let setting = |k: &str| crate::statehost::policies().and_then(|x| x.with_state(|st| st.settings.get(k).cloned())).flatten().unwrap_or_default();
    let pick = |k: &str, dflt: String| if p.get(k).is_some() { s(p, k).to_string() } else if th.is_empty() { dflt } else { s(&now, k).to_string() };
    let provider = pick("provider", { let d = setting("agent.provider"); if d.is_empty() { "claude".into() } else { d } });
    let model = pick("model", if p.get("provider").is_some() { String::new() } else { setting("agent.model") });
    let models: Value = serde_json::from_str(&crate::settings::models::all()).unwrap_or(Value::Null);
    Ok(json!({"picker":picker_sections(&models, &provider, &model, &pick("effort", String::new()), &pick("runtime", "approval-required".into()))}))
}

async fn thread_now(hub: &Arc<Hub>, id: &str) -> Result<Value, Value> {
    // the planner's events are committed before its reply; read the model again for the thread as it is now
    let g = hub.model().await; let asks = open_ask_counts(hub);
    thread_json(hub, &g, id, &asks).ok_or_else(|| err("unknown thread"))
}
fn known_thread(m: &crate::model::Model, id: &str) -> bool { m.threads.get(id).map_or(false, |t| !t.deleted) }

fn contains(hay: &str, needle: &str) -> bool { hay.to_lowercase().contains(needle) }

pub async fn call(hub: &Arc<Hub>, c: &mut Conn, m: &str, p: &Value) -> Option<Result<Value, Value>> {
    Some(match m {
        "thread.act" => thread_act(hub, c, p).await,
        "project.act" => {
            let (id, act) = (s(p, "project"), s(p, "action"));
            if !hub.model().await.projects.get(id).map_or(false, |x| !x.removed) { return Some(Err(err("unknown project"))) }
            match act {
                "rename" => rpc(hub, c, "project.rename", json!({"project":id,"title":s(p, "title")})).await.map(|_| json!({})),
                "remove" => rpc(hub, c, "project.remove", json!({"project":id})).await.map(|_| json!({})),
                // the badge: `color` takes a tint name, `icon` an icon name (present/badge.rs lists both; "" takes the default back)
                "color" | "icon" => match crate::present::badge::setting_of(&format!("project.{act}.{id}={}", s(p, "value"))) {
                    Some((key, value)) => rpc(hub, c, "setting.set", json!({"key":key,"value":value})).await.map(|_| json!({})),
                    None => Err(err("value: a tint or an icon of the list")),
                },
                _ => Err(err("action: rename, color, icon or remove")),
            }
        }
        "badge.list" => Ok(crate::present::badge::list()),
        "search" => search(hub, p).await,
        "models.list" => Ok(json!({"models": serde_json::from_str::<Value>(&crate::settings::models::all()).unwrap_or(json!({}))})),
        "thread.model" => {
            let th = s(p, "thread"); if !known_thread(&*hub.model().await, th) { return Some(Err(err("unknown thread"))) }
            let mut q = json!({"thread":th,"provider":s(p, "provider"),"model":s(p, "model")});
            if !s(p, "effort").is_empty() { q["effort"] = p["effort"].clone() }
            if s(p, "provider").is_empty() { return Some(Err(err("provider"))) }
            // the same provider: only the model changes (thread.modes); another provider is a switch (thread.provider, a handoff between turns)
            let same = hub.model().await.threads.get(th).map_or(false, |t| t.provider == s(p, "provider"));
            let r = if same { rpc(hub, c, "thread.modes", json!({"thread":th,"model":s(p, "model")})).await } else { rpc(hub, c, "thread.provider", q).await };
            match r { Ok(_) => thread_now(hub, th).await.map(|t| json!({"thread":t})), Err(e) => Err(e) }
        }
        "thread.picker" => picker(hub, p).await,
        "thread.pick" => {
            let (th, value) = (s(p, "thread"), s(p, "value")); if !known_thread(&*hub.model().await, th) { return Some(Err(err("unknown thread"))) }
            let r = match s(p, "action") {
                "model" => rpc(hub, c, "thread.modes", json!({"thread":th,"model":value})).await,
                "effort" => rpc(hub, c, "thread.effort", json!({"thread":th,"effort":value})).await,
                "provider" => rpc(hub, c, "thread.provider", json!({"thread":th,"provider":value})).await,
                "runtime" => rpc(hub, c, "thread.modes", json!({"thread":th,"runtime":value})).await,
                _ => return Some(Err(err("action: model, effort, provider or runtime"))),
            };
            match r { Ok(_) => thread_now(hub, th).await.map(|t| json!({"thread":t})), Err(e) => Err(e) }
        }
        "mentions.list" => mentions(hub, p).await,
        "viewer.files" | "viewer.watch" | "viewer.probe" | "viewer.info" | "files.entries" | "mech.list" | "browser" => viewers(hub, c, m, p).await?,
        "viewer.diff" | "review.page" | "review.tick" | "review.comment" | "review.list" => review(hub, c, m, p).await?,
        "bots.list" => Ok(crate::bots::api::directory(hub).await),
        "bot.say" => {
            let (bot, text) = (s(p, "bot"), s(p, "text")); if text.trim().is_empty() { return Some(Err(err("empty message"))) }
            let mut q = json!({"bot":bot,"text":text}); if safe_id(s(p, "clientId")) && !s(p, "clientId").is_empty() { q["msg"] = p["clientId"].clone() }
            rpc(hub, c, "bots.say", q).await.map(|_| json!({}))
        }
        "room.posts" => match crate::bots::api::room_posts(hub, s(p, "room"), limit_of(p, 40, 200)).await { Some(x) => Ok(json!({"posts":x})), None => Err(err("unknown room")) },
        "room.post" => {
            let (room, text) = (s(p, "room"), s(p, "text")); if text.trim().is_empty() { return Some(Err(err("empty message"))) }
            let mut q = json!({"room":room,"text":text}); if safe_id(s(p, "clientId")) && !s(p, "clientId").is_empty() { q["msg"] = p["clientId"].clone() }
            rpc(hub, c, "bots.post", q).await.map(|_| json!({}))
        }
        _ => return None,
    })
}

async fn thread_act(hub: &Arc<Hub>, c: &mut Conn, p: &Value) -> Result<Value, Value> {
    let (th, act) = (s(p, "thread"), s(p, "action"));
    if !known_thread(&*hub.model().await, th) { return Err(err("unknown thread")) }
    if act == "rename" {
        if s(p, "title").trim().is_empty() { return Err(err("title")) }
        rpc(hub, c, "thread.rename", json!({"thread":th,"title":s(p, "title")})).await?;
    } else if THREAD_ACTS.contains(&act) {
        rpc(hub, c, &format!("thread.{act}"), json!({"thread":th})).await?;
        if act == "delete" { return Ok(json!({})) }
    } else { return Err(err("action: pin, unpin, settle, unsettle, archive, unarchive, delete, detach or rename")) }
    thread_now(hub, th).await.map(|t| json!({"thread":t}))
}

async fn search(hub: &Arc<Hub>, p: &Value) -> Result<Value, Value> {
    let q = s(p, "q").trim().to_lowercase(); let lim = limit_of(p, 30, 100); let arch = p["archived"] == true;
    if q.is_empty() { return Ok(json!({"threads":[],"projects":[]})) }
    let g = hub.model().await; let asks = open_ask_counts(hub);
    let mut ts: Vec<(u64, String)> = g.threads.iter().filter(|(_, t)| !t.deleted && (arch || !t.archived) && (contains(&t.title, &q) || g.projects.get(&t.project).map_or(false, |x| contains(&x.title, &q)))).map(|(i, t)| (ms(t.activity.max(t.created)), i.clone())).collect();
    ts.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1))); ts.truncate(lim);
    let threads: Vec<Value> = ts.iter().filter_map(|(_, i)| thread_json(hub, &g, i, &asks)).collect();
    let mut ps: Vec<(u64, String)> = g.projects.iter().filter(|(_, x)| !x.removed && (contains(&x.title, &q) || contains(&x.root, &q))).map(|(i, _)| (0, i.clone())).collect(); ps.truncate(lim);
    let projects: Vec<Value> = ps.iter().filter_map(|(_, i)| project_json(&g, i, &asks)).collect();
    Ok(json!({"threads":threads,"projects":projects}))
}

async fn mentions(hub: &Arc<Hub>, p: &Value) -> Result<Value, Value> {
    let q = s(p, "q").trim().to_lowercase(); let kind = s(p, "kind"); let lim = limit_of(p, 12, 50);
    let want = |k: &str| kind.is_empty() || kind == k;
    let mut items: Vec<Value> = vec![];
    let mut push = |sigil: &str, id: &str, label: &str, detail: &str| { if q.is_empty() || contains(label, &q) { items.push(json!({"sigil":sigil,"id":id,"label":label,"detail":detail})) } };
    if want("@") { let d = crate::bots::api::directory(hub).await; for b in d["bots"].as_array().into_iter().flatten() { push("@", s(b, "id"), s(b, "name"), if b["uber"] == true { "uberbot" } else { "bot" }) } }
    if want("#") { let d = crate::bots::api::directory(hub).await; for r in d["rooms"].as_array().into_iter().flatten() { push("#", s(r, "id"), s(r, "name"), "room") } }
    if want(">") || want("%") {
        let g = hub.model().await;
        if want(">") { let mut ts: Vec<_> = g.threads.iter().filter(|(_, t)| !t.deleted && !t.archived).collect(); ts.sort_by(|a, b| b.1.activity.cmp(&a.1.activity)); for (i, t) in ts.into_iter().take(200) { push(">", i, &t.title, g.projects.get(&t.project).map_or("", |x| x.title.as_str())) } }
        if want("%") { for (i, x) in g.projects.iter().filter(|(_, x)| !x.removed) { push("%", i, &x.title, &x.root) } }
    }
    if want("&") {
        let all: Value = serde_json::from_str(&crate::settings::models::all()).unwrap_or(json!({}));
        for (prov, rows) in all.as_object().into_iter().flatten() { for r in rows.as_array().into_iter().flatten() { if r["ok"] == true { push("&", &format!("{prov}/{}", s(r, "value")), &format!("{prov}/{}", s(r, "value")), s(r, "label")) } } }
    }
    items.truncate(lim);
    Ok(json!({"items":items}))
}

/// Notifications for a `lists` subscription: an ask opened, a turn that ended (done or failed).
pub fn notify_for(e: &Value, thread: &str, title: &str, ask_text: &str) -> Option<Value> {
    match s(e, "$") {
        "AskOpened" => Some(json!({"kind":"ask","thread":thread,"title":title,"body":if ask_text.is_empty() { "Waiting for your answer" } else { ask_text }})),
        "TurnChanged" => match s(e, "state") { "failed" => Some(json!({"kind":"fail","thread":thread,"title":title,"body":"The turn failed"})), "running" | "" | "interrupted" | "cancelled" | "stopped" => None, _ => Some(json!({"kind":"done","thread":thread,"title":title,"body":"Finished"})) },
        _ => None,
    }
}


// ---- viewers, files, browser (each the hub's own request, for the folder of the thread) ----
async fn root_of(hub: &Arc<Hub>, thread: &str) -> Result<String, Value> {
    crate::files::root_of(hub, thread).await.map(|p| p.to_string_lossy().into_owned()).ok_or_else(|| err("that thread has no folder"))
}
/// A reply's `result`, or its error.
fn result(r: Value) -> Value { r.get("result").cloned().unwrap_or(json!({})) }
fn entries_of(text: &str) -> Vec<Value> {
    // the hub's listing: one line per entry, `d` or `f`, a tab, the size, a tab, the name
    text.lines().filter_map(|l| { let mut it = l.splitn(3, '\t'); let (k, size, name) = (it.next()?, it.next()?, it.next()?);
        Some(json!({"dir":k.starts_with('d'),"size":size.parse::<u64>().unwrap_or(0),"name":name})) }).collect()
}
async fn viewers(hub: &Arc<Hub>, c: &mut Conn, m: &str, p: &Value) -> Option<Result<Value, Value>> {
    let th = s(p, "thread");
    Some(async {
        match m {
            "viewer.files" => { let root = root_of(hub, th).await?; let f = crate::files::kicad::project::read(&root); Ok(json!({"root":root,"pro":f.pro,"pcb":f.pcb,"sch":f.sch})) }
            "viewer.watch" => {
                let root = root_of(hub, th).await?; let kind = s(p, "kind"); let path = s(p, "path");
                if path.split('/').any(|x| x == "..") || path.contains('\0') { return Err(err("that path is not in this project")) }
                let path = if path.is_empty() || path.starts_with('/') { path.to_string() } else { format!("{root}/{path}") };
                let mut q = json!({"kind":kind,"root":root,"path":path});
                if !s(p, "tree").is_empty() { q["tree"] = p["tree"].clone() }
                rpc(hub, c, "kicad.watch", q).await?; Ok(json!({"root":root}))
            }
            "viewer.probe" => rpc(hub, c, "kicad.probe", json!({"key":s(p, "key"),"name":s(p, "name"),"value":s(p, "value")})).await.map(result),
            "viewer.info" => rpc(hub, c, "kicad.info", json!({"key":s(p, "key"),"at":p["at"],"idx":p["idx"]})).await.map(result),
            "files.entries" => {
                let r = rpc(hub, c, "files.dir", json!({"thread":th,"path":s(p, "path")})).await?; let res = result(r);
                Ok(json!({"dir":s(p, "path"),"entries":entries_of(s(&res, "fmEntries"))}))
            }
            "mech.list" => {
                // the hub's listing is `cksum` lines (`<crc> <size> <path>`): models (step, stp, glb) and pictures, as paths
                let res = result(rpc(hub, c, "mech.list", json!({"thread":th})).await?);
                let paths: Vec<String> = s(&res, "mech").lines().filter_map(|l| l.splitn(3, ' ').nth(2).map(str::to_string)).collect();
                let model = |p: &String| { let l = p.to_lowercase(); l.ends_with(".step") || l.ends_with(".stp") || l.ends_with(".glb") };
                Ok(json!({"parts":paths.iter().filter(|p| model(p)).collect::<Vec<_>>(),"renders":paths.iter().filter(|p| !model(p)).collect::<Vec<_>>(),"primary":s(&res, "primary")}))
            }
            _ => { // browser
                if !hub.model().await.threads.get(th).map_or(false, |t| !t.deleted) { return Err(err("unknown thread")) }
                let mut q = p.clone(); if let Some(o) = q.as_object_mut() { o.remove("thread"); o.insert("bot".into(), json!(th)); }
                rpc(hub, c, "bots.browser", q).await.map(|_| json!({}))
            }
        }
    }.await)
}
/// Frames the hub pushes to this connection that a phone's viewer reads: a plot (binary CBOR, or JSON for "none") and a browser frame's announcement.
pub fn wants_raw(b: &[u8]) -> bool { b.len() > 7 && (0xA0..=0xB7).contains(&b[0]) && b[1] == 1 && b[2..7] == [0x64, b'p', b'l', b'o', b't'] }
pub fn wants_value(v: &Value) -> bool { matches!(s(v, "t"), "plot" | "shot") }

// ---- diff and review (the hub's git.diff and review.* requests, and the thread's review acts) ----
async fn review(hub: &Arc<Hub>, c: &mut Conn, m: &str, p: &Value) -> Option<Result<Value, Value>> {
    let th = s(p, "thread");
    Some(async {
        if !known_thread(&*hub.model().await, th) { return Err(err("unknown thread")) }
        match m {
            "viewer.diff" => {
                let mut q = json!({"thread":th}); for k in ["from", "to"] { if p.get(k).is_some() { q[k] = p[k].clone() } }
                rpc(hub, c, "git.diff", q).await.map(result)
            }
            "review.page" => {
                let (page, mut q) = (s(p, "page"), json!({"thread":th}));
                match page {
                    "bom" => { for k in ["a", "b"] { if !s(p, k).is_empty() { q[k] = p[k].clone() } } rpc(hub, c, "review.bom", q).await.map(result) }
                    "checks" => { if !s(p, "commit").is_empty() { q["commit"] = p["commit"].clone() } q["trigger"] = json!("manual"); rpc(hub, c, "review.checks", q).await.map(result) }
                    _ => Err(err("page: bom or checks")),
                }
            }
            "review.tick" => rpc(hub, c, "review.tick", json!({"thread":th,"index":p["index"]})).await.map(result),
            "review.comment" => {
                let mut q = p.clone(); if let Some(o) = q.as_object_mut() { o.insert("thread".into(), json!(th)); }
                rpc(hub, c, "review.comment", q).await.map(|_| json!({}))
            }
            _ => { // review.list: this thread's review acts, in the log's tail, folded
                use crate::files::kicad::review as rv;
                let n = hub.store.len(); let from = n.saturating_sub(300_000);
                let lines = hub.store.range(from, n).map_err(err)?;
                let mark = format!("\"thread\":\"{th}\"");
                let ops: Vec<rv::Op> = lines.iter().filter(|l| l.contains("\"tone\":\"review\"") && l.contains(&mark)).filter_map(|l| serde_json::from_str::<Value>(l).ok())
                    .filter(|e| s(e, "$") == "ActivityLogged" && s(e, "thread") == th).filter_map(|e| rv::of(s(&e, "tone"), s(&e, "kind"), s(&e, "summary"), ms(e["at"].as_u64().unwrap_or(0)) / 1000)).collect();
                let cs = rv::fold(&ops);
                Ok(json!({"comments":cs.iter().map(|c| json!({"id":c.id,"text":c.text,"by":c.by,"at":ms(c.at),"parent":c.parent,"resolved":c.resolved,"file":c.anchor.file,"layer":c.anchor.layer,"x":c.anchor.x,"y":c.anchor.y,"item":c.anchor.item})).collect::<Vec<_>>()}))
            }
        }
    }.await)
}
