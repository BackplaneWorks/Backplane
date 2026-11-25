//! One-round-trip reads by hashed parts (docs/PHONE-DATA.md, Parts and their hashes): `thread.open` and `shell.sync`.
//! Each part of a screen goes with a short hash of it as sent; a client that holds a part sends its hash back in `have`, and the part is left out. The parts are
//! what the single reads answer (`thread.entries`, `thread.work`, `thread.queue`, `thread.buy`, `viewer.rail`, the lists), read by those same handlers, so a
//! bundled read and its pieces can never disagree.
use super::*;
use sha2::{Digest, Sha256};

/// A part's hash: the first 8 bytes of SHA-256 over its JSON, as hex. Opaque to clients.
pub fn part_hash(v: &Value) -> String { hex::encode(&Sha256::digest(v.to_string().as_bytes())[..8]) }
fn held<'a>(have: &'a Value, key: &str) -> Option<&'a str> { have.get(key).and_then(Value::as_str) }
/// A part into a reply: its hash into `h` under `key`, the part itself under `wire` unless the client holds it as it is. Whether it was sent.
fn put(out: &mut serde_json::Map<String, Value>, h: &mut serde_json::Map<String, Value>, have: &Value, key: &str, wire: &str, v: Value) -> bool {
    let x = part_hash(&v); let send = held(have, key) != Some(x.as_str());
    if send { out.insert(wire.into(), v); }
    h.insert(key.into(), json!(x)); send
}

/// `thread.open {thread, limit?, have?}` → `{sub, thread?, ids?, entries, next?, work?, queue?, buy?, rail?, h}`: subscribes (as `subscribe {topic:"thread"}`) and
/// answers the newest page and the panels, each part only when the client's hash for it differs. `next` goes whenever the page is sent anew (with `ids`).
pub(super) async fn thread_open(hub: &Arc<Hub>, c: &mut Conn, subs: &mut Subs, p: &Value, is_browser: bool) -> Result<Value, Value> {
    let th = s(p, "thread").to_string();
    let known = hub.model().await.threads.get(&th).map_or(false, |t| !t.deleted) || hub.far().map_or(false, |f| f.has(&th));
    if !known { return Err(err("unknown thread")) }
    let have = p.get("have").filter(|h| h.is_object()).cloned().unwrap_or(json!({}));
    let page = entries(hub, &json!({"thread": th, "limit": limit_of(p, 40, 32)})).await?;
    let mut panels = serde_json::Map::new();
    for (key, m) in [("work", "thread.work"), ("queue", "thread.queue"), ("buy", "thread.buy"), ("rail", "viewer.rail")] {
        // a panel this thread has none of (no folder for a rail) is null, never a failed open
        let v = call(hub, c, m, &json!({"thread": th}), is_browser).await.unwrap_or(Value::Null);
        panels.insert(key.into(), v);
    }
    // subscribed once the reads are done: the pushes after this are news to what was read
    subs.next += 1; subs.subs.push((subs.next, Topic::Thread(th.clone()))); far_seed(hub, subs, &th);
    let (mut out, mut h) = (serde_json::Map::new(), serde_json::Map::new());
    out.insert("sub".into(), json!(subs.next));
    put(&mut out, &mut h, &have, "@thread", "thread", page["thread"].clone());
    let list = page["entries"].as_array().cloned().unwrap_or_default();
    let ids = json!(list.iter().map(|e| e["id"].clone()).collect::<Vec<_>>());
    if put(&mut out, &mut h, &have, "@ids", "ids", ids) { out.insert("next".into(), page["next"].clone()); }
    // an entry's hash rides on the entry (`h`), not in the reply's `h`: the client keeps it with the entry, and a warm open costs no list of them
    let sent: Vec<Value> = list.into_iter().filter_map(|mut e| { let x = part_hash(&e); let id = e["id"].as_str().unwrap_or("").to_string();
        (held(&have, &id) != Some(x.as_str())).then(|| { e["h"] = json!(x); e }) }).collect();
    out.insert("entries".into(), Value::Array(sent));
    for key in ["work", "queue", "buy", "rail"] { let v = panels.remove(key).unwrap_or(Value::Null); put(&mut out, &mut h, &have, &format!("@{key}"), key, v); }
    out.insert("h".into(), Value::Object(h));
    Ok(Value::Object(out))
}

/// `shell.sync {have?, settled?=25, open?}` → `{projects?, threads?, sections?, removed?, h, sub}`: the sidebar in one reply, and a `lists` subscription. Per project its active and
/// pinned threads and its newest `settled` settled ones; an idle child (a subagent, a subthread) only with its parent listed. A project goes again, with all of its listed
/// threads, only when the hash of the two together differs from the client's (`have["p:<id>"]`); `removed` names the held projects that are gone.
pub(super) async fn shell_sync(hub: &Arc<Hub>, c: &mut Conn, subs: &mut Subs, p: &Value, is_browser: bool) -> Result<Value, Value> {
    let have = p.get("have").filter(|h| h.is_object()).cloned().unwrap_or(json!({}));
    let settled_n = p["settled"].as_u64().unwrap_or(25).min(200) as usize;
    let mut projects: Vec<Value> = vec![]; let mut before = Value::Null;
    loop { // every project, a page at a time
        let mut q = json!({"limit": 200}); if !before.is_null() { q["before"] = before.clone(); }
        let r = call(hub, c, "projects.list", &q, is_browser).await?;
        projects.extend(r["projects"].as_array().cloned().unwrap_or_default());
        before = r["next"].clone(); if before.is_null() || projects.len() > 2000 { break }
    }
    let mut sec_q = json!({});
    if p.get("open").is_some_and(Value::is_array) { sec_q["open"] = p["open"].clone(); }
    let sections = call(hub, c, "sidebar.sections", &sec_q, is_browser).await.unwrap_or(Value::Null);
    subs.next += 1; subs.subs.push((subs.next, Topic::Lists));

    let at = hub.setting("home.at").await.unwrap_or_default();
    let farv = hub.far();
    let moved = |id: &str, project: &str| crate::net::homemove::moved_thread(&at, id, Some(project), &|x| farv.as_ref().map_or(false, |f| f.has(x))).is_some();
    let g = hub.model().await;
    let asks = open_ask_counts(hub);

    // Collect local active/settled thread keys per project in one state pass instead of 2xN threads.list calls.
    let mut local_by_proj: HashMap<String, (Vec<(u64, String)>, Vec<(u64, String)>)> = HashMap::new();
    let mut collect_local = |st: Option<&crate::state::State>| {
        for (id, t) in &g.threads {
            if t.deleted || t.archived || t.project == crate::sidebar::SCRATCH || crate::state::is_mail_run_id(id) || moved(id, &t.project) { continue }
            if st.is_some_and(|s| s.run_hidden(id)) { continue }
            let (_, settled, settled_at) = st.and_then(|s| s.threads.get(id)).map(shelf_flags).unwrap_or((false, false, 0));
            let entry = local_by_proj.entry(t.project.clone()).or_default();
            if settled {
                entry.1.push((settled_at, id.clone()));
            } else {
                entry.0.push((ms(t.activity.max(t.created)), id.clone()));
            }
        }
    };
    if crate::statehost::policies().and_then(|pol| pol.with_state(|st| collect_local(Some(st)))).is_none() {
        collect_local(None);
    }

    let (join, mut far_by_proj) = with_far(hub, "", |far, un, _| (un.far.clone(), machines::thread_keys_by_project(far, "", un.far, false)));

    let mut per_project: Vec<(Value, String, Vec<String>)> = Vec::with_capacity(projects.len());
    let mut all_ids: Vec<String> = Vec::new();
    let mut all_seen: HashSet<String> = HashSet::new();
    for pr in projects {
        let id = pr["id"].as_str().unwrap_or("").to_string(); if id.is_empty() { continue }
        let (mut active, mut settled) = local_by_proj.remove(&id).unwrap_or_default();
        if let Some((fa, fs)) = far_by_proj.remove(&id) {
            active.extend(fa);
            settled.extend(fs);
        }
        active.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        active.truncate(200);
        settled.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        settled.truncate(settled_n);
        let mut ids: Vec<String> = Vec::with_capacity(active.len() + settled.len());
        for (_, tid) in active.into_iter().chain(settled.into_iter()) {
            if all_seen.insert(tid.clone()) { all_ids.push(tid.clone()); }
            ids.push(tid);
        }
        per_project.push((pr, id, ids));
    }

    let built = thread_rows_batch(hub, &g, &all_ids, &asks, &join);
    drop(g);

    let (mut out, mut h) = (serde_json::Map::new(), serde_json::Map::new());
    let (mut rows, mut threads, mut seen) = (vec![], vec![], HashSet::new());
    for (pr, id, ids) in per_project {
        let mut ts: Vec<Value> = ids.iter().filter_map(|tid| built.get(tid).cloned()).collect();
        let listed: HashSet<String> = ts.iter().filter_map(|t| t["id"].as_str().map(str::to_string)).collect();
        // an idle child whose parent is not listed would stand alone among the project's threads: it comes with its parent's shelf
        ts.retain(|t| { let parent = t["parent"].as_str().unwrap_or(""); parent.is_empty() || listed.contains(parent) || t["state"] != "idle" || t["pinned"] == true });
        let key = format!("p:{id}"); seen.insert(key.clone());
        let x = part_hash(&json!([pr, ts]));
        if held(&have, &key) != Some(x.as_str()) { rows.push(pr); threads.extend(ts); }
        h.insert(key, json!(x));
    }
    if !rows.is_empty() { out.insert("projects".into(), Value::Array(rows)); out.insert("threads".into(), Value::Array(threads)); }
    let gone: Vec<Value> = have.as_object().into_iter().flatten().filter_map(|(k, _)| k.strip_prefix("p:").filter(|_| !seen.contains(k)).map(|i| json!(i))).collect();
    if !gone.is_empty() { out.insert("removed".into(), Value::Array(gone)); }
    put(&mut out, &mut h, &have, "@sections", "sections", sections);
    out.insert("h".into(), Value::Object(h)); out.insert("sub".into(), json!(subs.next));
    Ok(Value::Object(out))
}

/// A subagent's view by parts (feature `subview`, `parts:1`): `{thread, key, head?, ids?, entries, h}` against the parts `have` holds. `head` is the view without its
/// entries, `ids` its entries' ids oldest → newest, `entries` those new or changed (each with its `h`). `held` is updated to what the client holds after this.
pub fn sub_parts(thread: &str, key: &str, view: &Value, held: &mut HashMap<String, String>) -> Value {
    let have = Value::Object(held.iter().map(|(k, v)| (k.clone(), json!(v))).collect());
    let (mut out, mut h) = (serde_json::Map::new(), serde_json::Map::new());
    out.insert("thread".into(), json!(thread)); out.insert("key".into(), json!(key));
    let mut head = view.clone(); let list = head.as_object_mut().and_then(|o| o.remove("entries")).and_then(|e| e.as_array().cloned()).unwrap_or_default();
    put(&mut out, &mut h, &have, "@head", "head", head);
    put(&mut out, &mut h, &have, "@ids", "ids", json!(list.iter().map(|e| e["id"].clone()).collect::<Vec<_>>()));
    let mut sent = vec![]; let mut now: HashMap<String, String> = h.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect();
    for mut e in list {
        let x = part_hash(&e); let id = e["id"].as_str().unwrap_or("").to_string();
        if held.get(&id) != Some(&x) { e["h"] = json!(x); sent.push(e); }
        now.insert(id, x);
    }
    out.insert("entries".into(), Value::Array(sent)); out.insert("h".into(), Value::Object(h));
    *held = now; Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn view(state: &str, steps: &[(&str, &str)]) -> Value { json!({"key":"k1","title":"Explore","state":state,"entries":steps.iter().map(|(i, t)| json!({"id":i,"kind":"tool","text":t})).collect::<Vec<_>>()}) }
    #[test] fn a_subagent_view_goes_by_what_moved() {
        let mut held = HashMap::new();
        let first = sub_parts("t1", "k1", &view("working", &[("s1", "ls"), ("s2", "grep")]), &mut held);
        assert!(first.get("head").is_some() && first.get("ids").is_some()); assert_eq!(first["entries"].as_array().unwrap().len(), 2, "the first push carries everything");
        assert!(first["head"].get("entries").is_none(), "the head is the view without its entries");
        let same = sub_parts("t1", "k1", &view("working", &[("s1", "ls"), ("s2", "grep")]), &mut held);
        assert!(same.get("head").is_none() && same.get("ids").is_none() && same["entries"] == json!([]) && is_empty_parts(&same), "nothing moved: {same}");
        let step = sub_parts("t1", "k1", &view("working", &[("s1", "ls"), ("s2", "grep -n"), ("s3", "cat")]), &mut held);
        assert!(step.get("head").is_none()); assert_eq!(step["ids"], json!(["s1", "s2", "s3"]));
        assert_eq!(step["entries"].as_array().unwrap().iter().map(|e| e["id"].clone()).collect::<Vec<_>>(), [json!("s2"), json!("s3")], "a step changed in place and a new one: {step}");
        let done = sub_parts("t1", "k1", &view("completed", &[("s1", "ls"), ("s2", "grep -n"), ("s3", "cat")]), &mut held);
        assert_eq!((done["head"]["state"].as_str(), done.get("ids").is_none(), done["entries"].as_array().unwrap().len()), (Some("completed"), true, 0));
        assert_eq!(part_hash(&json!({"a":1})), part_hash(&json!({"a":1}))); assert_eq!(part_hash(&json!(1)).len(), 16);
    }
}
