//! The mirrors of the owner's other machines, one per link, kept outside `events.jsonl` and `Model` (hub reads
//! and writes that log: a mirror there would fold as local ids). Two files per link under `<home>/far/`:
//!
//! * `<link>.jsonl` — hub's own mirror format, so either hub reads it: a header `{"epoch":..,"tail":0}`, then
//!   each batch taken as its items `{"n":..,"c":..}` (the change as its owner numbers it, ids its own) and a
//!   `{"head":n}` line.
//! * `<link>.events` — derived and rebuilt at start from the items: each change renamed into this hub's ids
//!   (`far::change`, with `Origin.show`), one `Store` and one `Model` per link, so a page of a mirrored thread is
//!   `Model::page` over the mirror's own store. Nothing here grows the hub's log or index.
//!
//! `FarStore` is the hub's `api::FarView`: sidebar rows come from `fold_mirror` (the presenter), pages from here.
use super::{far::{self, Got, Item}, origin};
use crate::{api::FarView, model::{Model, Page, Row}, store::Store};
use serde_json::{json, Value};
use std::{collections::HashMap, path::{Path, PathBuf}, sync::{Arc, Mutex, RwLock}};

/// One mirror. The writer (epoch, head, the items file) is behind its own lock, held through the fsyncs of `take` and
/// nothing else; the derived events store is read without any lock (`Store` readers never wait for a writer); the
/// `Model` has a read-write lock held only to apply or to read an index. So a view of a mirrored thread never waits for a
/// disk write, and one link never waits for another.
struct Mirror { w: Mutex<Writer>, events: Store, model: RwLock<Model> }
struct Writer { epoch: String, head: u64, items: Store }
pub struct FarStore { dir: PathBuf, m: RwLock<HashMap<String, Arc<Mirror>>> }

/// What a batch did: the new head, whether a gap came first, and the renamed events for the presenter. `reset`: the
/// epoch changed and the mirror started over: the presenter must forget the link's old rows first (`Fr.forget`).
pub struct Taken { pub got: Got, pub events: Vec<Value>, pub reset: bool }

fn safe(link: &str) -> bool { super::botnet::safe(link) }
fn path(dir: &Path, link: &str, ext: &str) -> PathBuf { dir.join(format!("{link}.{ext}")) }
/// Fr.epoch.changed: a new, different epoch means another history.
pub fn epoch_changed(old: &str, new: &str) -> bool { !new.is_empty() && old != new }
fn rename(link: &str, c: &Value) -> Option<Value> { far::change(link, c, &origin::show) }

impl Mirror {
    fn fresh(dir: &Path, link: &str, epoch: &str) -> anyhow::Result<Mirror> {
        for e in ["jsonl", "events"] { let _ = std::fs::remove_file(path(dir, link, e)); }
        let items = Store::open(&path(dir, link, "jsonl"))?;
        items.append(&json!({"epoch": epoch, "tail": 0}).to_string())?;
        Ok(Mirror { w: Mutex::new(Writer { epoch: epoch.into(), head: 0, items }), events: Store::open(&path(dir, link, "events"))?, model: RwLock::new(Model::default()) })
    }
    /// Read a mirror back: the items file, then the derived events rebuilt from it.
    fn load(dir: &Path, link: &str) -> anyhow::Result<Mirror> {
        let items = Store::open(&path(dir, link, "jsonl"))?;
        let _ = std::fs::remove_file(path(dir, link, "events"));
        let events = Store::open(&path(dir, link, "events"))?;
        let (mut epoch, mut head) = (String::new(), 0u64);
        for a in (0..items.len()).step_by(4096) {
            let mut out: Vec<String> = vec![];
            for l in items.range(a, a + 4096)? {
                let Ok(o) = serde_json::from_str::<Value>(&l) else { continue };
                if o.get("c").is_some() { if let Some(e) = rename(link, &o["c"]) { out.push(e.to_string()) } }
                else { if let Some(h) = o["head"].as_u64() { head = h } if let Some(e) = o["epoch"].as_str() { epoch = e.to_string() } }
            }
            if !out.is_empty() { events.append_many(&out.iter().map(String::as_str).collect::<Vec<_>>())?; }
        }
        let model = Model::build(&events)?;
        Ok(Mirror { w: Mutex::new(Writer { epoch, head, items }), events, model: RwLock::new(model) })
    }
}

impl FarStore {
    /// Every mirror under `<home>/far`, rebuilt (blocking: call it off the async workers).
    pub fn open(home: &Path) -> anyhow::Result<Arc<FarStore>> {
        let dir = home.join("far"); std::fs::create_dir_all(&dir)?;
        let mut m = HashMap::new();
        for e in std::fs::read_dir(&dir)?.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("jsonl") { continue }
            let Some(link) = p.file_stem().and_then(|s| s.to_str()).map(str::to_string) else { continue };
            if safe(&link) { m.insert(link.clone(), Arc::new(Mirror::load(&dir, &link)?)); }
        }
        Ok(Arc::new(FarStore { dir, m: RwLock::new(m) }))
    }
    fn get(&self, link: &str) -> Option<Arc<Mirror>> { self.m.read().unwrap().get(link).cloned() }
    pub fn head(&self, link: &str) -> u64 { self.get(link).map_or(0, |m| m.w.lock().unwrap().head) }
    pub fn epoch(&self, link: &str) -> String { self.get(link).map(|m| m.w.lock().unwrap().epoch.clone()).unwrap_or_default() }
    pub fn links(&self) -> Vec<String> { let mut v: Vec<String> = self.m.read().unwrap().keys().cloned().collect(); v.sort(); v }
    /// Fr.recv: a batch from `link` that follows `since` and ends at `to`. A different epoch starts the mirror over; a
    /// batch past a gap changes nothing; of an overlapping batch only the new items are taken, durably, in order.
    /// (Blocking: fsyncs, under this link's writer lock only.)
    pub fn take(&self, link: &str, epoch: &str, since: u64, to: u64, items: &[Item]) -> anyhow::Result<Taken> {
        anyhow::ensure!(safe(link), "not a link id");
        let mut reset = false;
        let mirror = {
            let cur = self.get(link);
            let changed = match &cur { Some(m) => epoch_changed(&m.w.lock().unwrap().epoch, epoch), None => false };
            if changed || cur.is_none() {
                // (a new mirror is made before the map is locked: its files are written without holding up other links)
                let fresh = Arc::new(Mirror::fresh(&self.dir, link, epoch)?);
                reset = changed;
                self.m.write().unwrap().insert(link.to_string(), fresh.clone());
                fresh
            } else { cur.unwrap() }
        };
        let mut w = mirror.w.lock().unwrap();
        let got = far::take(w.head, since, to, items);
        if got.gap { return Ok(Taken { got, events: vec![], reset }) }
        let mut lines: Vec<String> = got.items.iter().map(|i| i.json().to_string()).collect();
        lines.push(json!({"head": got.head}).to_string());
        w.items.append_many(&lines.iter().map(String::as_str).collect::<Vec<_>>())?;
        let events: Vec<Value> = got.items.iter().filter_map(|i| rename(link, &i.c)).collect();
        if !events.is_empty() {
            let texts: Vec<String> = events.iter().map(Value::to_string).collect();
            let first = mirror.events.append_many(&texts.iter().map(String::as_str).collect::<Vec<_>>())?;
            let mut model = mirror.model.write().unwrap();
            for (i, t) in texts.iter().enumerate() { model.apply_line((first + i + 1) as u32, t) }
        }
        w.head = got.head;
        Ok(Taken { got, events, reset })
    }
    /// What the presenter needs to show a mirror after a restart: every event except the history of messages and
    /// activity, plus the last message of each thread (sidebar rows and previews), not the whole mirror. Reads the
    /// derived store in chunks without any lock and keeps one message per thread while it scans (blocking).
    pub fn replay_events(&self, link: &str) -> Vec<Value> {
        let Some(m) = self.get(link) else { return vec![] };
        let events = m.events.clone();
        let mut last: HashMap<String, usize> = HashMap::new();
        let mut all: Vec<Option<Value>> = vec![];
        for a in (0..events.len()).step_by(4096) {
            for l in events.range(a, a + 4096).unwrap_or_default() {
                // (a message's text is parsed only to keep it if it is its thread's newest so far)
                let kind = if l.contains("\"$\":\"MessagePosted\"") { 1 } else if ["MessageFrom", "ActivityLogged", "TodosChanged", "TodosSet", "WorkSet"].iter().any(|k| l.contains(&format!("\"$\":\"{k}\""))) { 2 } else { 0 };
                if kind == 2 { continue }
                let Ok(e) = serde_json::from_str::<Value>(&l) else { continue };
                if kind == 1 {
                    let th = e["thread"].as_str().unwrap_or("").to_string();
                    if let Some(&old) = last.get(&th) { all[old] = None } // the previous one is dropped as soon as a newer one is seen
                    last.insert(th, all.len());
                }
                all.push(Some(e));
            }
        }
        all.into_iter().flatten().collect()
    }
}

impl FarView for FarStore {
    fn has(&self, thread: &str) -> bool {
        let l = far::link(thread);
        !l.is_empty() && self.get(&l).map_or(false, |m| m.model.read().unwrap().threads.contains_key(thread))
    }
    fn page(&self, thread: &str, before: u64, limit: usize) -> Option<Page> {
        let m = self.get(&far::link(thread))?;
        let model = m.model.read().unwrap();
        model.threads.contains_key(thread).then(|| model.page(&m.events, thread, before, limit).ok()).flatten()
    }
    fn latest_context(&self, thread: &str) -> Vec<Row> {
        self.get(&far::link(thread)).and_then(|m| m.model.read().unwrap().latest_context(&m.events, thread).ok()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn home(tag: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("bp-farstore-{tag}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }
    fn it(n: u64, c: Value) -> Item { Item { n, c } }
    fn thread(n: u64, id: &str) -> Item { it(n, json!({"$":"ThreadCreated","id":id,"project":"","title":"T","env":"","branch":"","worktree":"","provider":"claude","model":"","at":n})) }
    fn msg(n: u64, th: &str, role: &str, text: &str) -> Item { it(n, json!({"$":"MessagePosted","thread":th,"msg":format!("m{n}"),"role":role,"text":text,"at":n})) }

    #[test] fn far_take_keeps_a_mirror_outside_the_hub_and_pages_it() {
        let h = home("take"); let fs = FarStore::open(&h).unwrap();
        let t = fs.take("L1", "e1", 0, 3, &[thread(1, "t9"), msg(2, "t9", "user", "hello"), msg(3, "t9", "assistant", "hi back")]).unwrap();
        assert!(!t.got.gap && t.got.head == 3 && t.events.len() == 3);
        assert_eq!(t.events[0]["id"], "L1~t9", "renamed into this hub's ids");
        assert!(fs.has("L1~t9") && !fs.has("t9") && !fs.has("L2~t9") && !fs.has("nope"));
        let p = fs.page("L1~t9", 0, 32).unwrap();
        assert_eq!(p.rows.len(), 2); assert_eq!(p.rows[0].text, "hi back"); assert_eq!(p.rows[0].thread, "L1~t9");
        assert!(fs.page("L1~zz", 0, 32).is_none());
        let ctx = fs.latest_context("L1~t9"); assert_eq!(ctx.len(), 2);
        assert!(!h.join("events.jsonl").exists(), "nothing touches the hub's log");
        let _ = std::fs::remove_dir_all(h);
    }
    #[test] fn far_a_batch_twice_late_or_past_a_gap_is_harmless() {
        let h = home("order"); let fs = FarStore::open(&h).unwrap();
        fs.take("L", "e", 0, 2, &[thread(1, "t"), msg(2, "t", "user", "a")]).unwrap();
        let again = fs.take("L", "e", 0, 2, &[thread(1, "t"), msg(2, "t", "user", "a")]).unwrap();
        assert!(again.events.is_empty() && fs.head("L") == 2, "the same batch again takes nothing");
        let gap = fs.take("L", "e", 5, 7, &[msg(6, "t", "user", "x")]).unwrap();
        assert!(gap.got.gap && gap.events.is_empty() && fs.head("L") == 2, "past a gap nothing is taken");
        let over = fs.take("L", "e", 1, 3, &[msg(2, "t", "user", "a"), msg(3, "t", "assistant", "b")]).unwrap();
        assert_eq!(over.events.len(), 1, "of an overlap only what is new"); assert_eq!(fs.head("L"), 3);
        assert_eq!(fs.page("L~t", 0, 32).unwrap().rows.len(), 2);
        let _ = std::fs::remove_dir_all(h);
    }
    #[test] fn far_a_mirror_survives_a_restart_in_the_format() {
        let h = home("restart");
        { let fs = FarStore::open(&h).unwrap(); fs.take("L", "e1", 0, 2, &[thread(1, "t"), msg(2, "t", "user", "kept")]).unwrap(); }
        let text = std::fs::read_to_string(h.join("far/L.jsonl")).unwrap(); let mut ls = text.lines();
        assert_eq!(ls.next().unwrap(), r#"{"epoch":"e1","tail":0}"#); assert!(ls.next().unwrap().starts_with(r#"{"n":1,"c":{"$":"ThreadCreated""#));
        assert_eq!(text.lines().last().unwrap(), r#"{"head":2}"#);
        let fs = FarStore::open(&h).unwrap();
        assert_eq!((fs.head("L"), fs.epoch("L")), (2, "e1".to_string())); assert_eq!(fs.page("L~t", 0, 32).unwrap().rows[0].text, "kept");
        let again = fs.take("L", "e1", 0, 2, &[thread(1, "t"), msg(2, "t", "user", "kept")]).unwrap(); assert!(again.events.is_empty());
        let _ = std::fs::remove_dir_all(h);
    }
    #[test] fn far_another_epoch_starts_the_mirror_over() {
        let h = home("epoch"); let fs = FarStore::open(&h).unwrap();
        fs.take("L", "old", 0, 2, &[thread(1, "t"), msg(2, "t", "user", "old")]).unwrap();
        let t = fs.take("L", "new", 0, 1, &[thread(1, "u")]).unwrap();
        assert!(t.reset && fs.head("L") == 1 && fs.epoch("L") == "new");
        assert!(!fs.has("L~t") && fs.has("L~u"), "what the old history held is gone");
        let t = fs.take("L", "", 1, 1, &[]).unwrap(); assert!(!t.reset, "a batch with no epoch (legacy) keeps the mirror");
        assert_eq!(fs.take("L", "new", 1, 1, &[]).unwrap().reset, false);
        let _ = std::fs::remove_dir_all(h);
    }
    #[test] fn far_the_first_epoch_replaces_a_legacy_projection_and_survives_a_restart() {
        let h = home("late");
        { let fs = FarStore::open(&h).unwrap();
          fs.take("L", "", 0, 1, &[thread(1, "t")]).unwrap();           // a legacy push: no epoch yet
          let t = fs.take("L", "learned", 0, 2, &[thread(1, "u"), msg(2, "u", "user", "x")]).unwrap();
          assert!(t.reset, ": the first epoch-aware pull replaces the legacy projection (Fr.epoch.changed \"\" -> e)"); }
        let fs = FarStore::open(&h).unwrap();
        assert_eq!((fs.epoch("L"), fs.head("L")), ("learned".to_string(), 2), "the epoch is in the file's header: a restart keeps it");
        assert!(fs.has("L~u") && !fs.has("L~t"));
        let _ = std::fs::remove_dir_all(h);
    }
    #[test] fn far_a_slow_writer_does_not_hold_up_a_view_or_another_link() {
        let h = home("lock"); let fs = FarStore::open(&h).unwrap();
        fs.take("A", "e", 0, 2, &[thread(1, "t"), msg(2, "t", "user", "one")]).unwrap(); fs.take("B", "e", 0, 1, &[thread(1, "u")]).unwrap();
        // hold A's writer lock as a take in its fsync does: reads of A and takes on B go on
        let a = fs.get("A").unwrap(); let _w = a.w.lock().unwrap();
        use crate::api::FarView;
        assert!(fs.has("A~t") && fs.page("A~t", 0, 32).is_some() && !fs.latest_context("A~t").is_empty(), "views of A do not wait for A's writer");
        fs.take("B", "e", 1, 2, &[msg(2, "u", "user", "two")]).unwrap();
        assert_eq!(fs.page("B~u", 0, 32).unwrap().rows.len(), 1, "another link takes meanwhile");
        let _ = std::fs::remove_dir_all(h);
    }
    #[test] fn far_what_stays_home_never_reaches_a_mirror() {
        let h = home("home"); let fs = FarStore::open(&h).unwrap();
        let t = fs.take("L", "e", 0, 2, &[it(1, json!({"$":"SettingSet","key":"k","value":"v","at":1})), thread(2, "t")]).unwrap();
        assert_eq!(t.events.len(), 1, "a setting is not a mirrored change"); assert_eq!(t.got.head, 2);
        assert!(fs.take("../x", "e", 0, 0, &[]).is_err() && fs.take("", "e", 0, 0, &[]).is_err(), "a link id is a plain word");
        let _ = std::fs::remove_dir_all(h);
    }
    #[test] fn far_replay_keeps_state_and_the_last_message_not_the_history() {
        let h = home("replay"); let fs = FarStore::open(&h).unwrap();
        let mut xs = vec![thread(1, "t")];
        for i in 2..60 { xs.push(msg(i, "t", if i % 2 == 0 { "user" } else { "assistant" }, &format!("m{i}"))) }
        xs.push(it(60, json!({"$":"TurnChanged","thread":"t","state":"running","at":60})));
        fs.take("L", "e", 0, 60, &xs).unwrap();
        let r = fs.replay_events("L");
        assert_eq!(r.len(), 3, "the thread, its last message, its turn: {r:?}");
        assert_eq!(r.iter().filter(|e| e["$"] == "MessagePosted").count(), 1); assert_eq!(r[1]["text"], "m59");
        let _ = std::fs::remove_dir_all(h);
    }
    /// Start time and memory of a big mirror (ignored: run by test/net/boat-scale-job.sh; prints one number each).
    #[test] #[ignore] fn scale_a_million_line_mirror() {
        use std::io::Write;
        fn rss_kb() -> u64 { std::fs::read_to_string("/proc/self/status").unwrap().lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| l.split_whitespace().nth(1)?.parse().ok()).unwrap_or(0) }
        let h = home("scale"); std::fs::create_dir_all(h.join("far")).unwrap();
        let n: u64 = std::env::var("SCALE_LINES").ok().and_then(|v| v.parse().ok()).unwrap_or(1_000_000);
        { let mut f = std::io::BufWriter::new(std::fs::File::create(h.join("far/L.jsonl")).unwrap());
          writeln!(f, r#"{{"epoch":"e","tail":0}}"#).unwrap();
          for t in 0..1000u64 { writeln!(f, "{}", thread(t + 1, &format!("t{t}")).json()).unwrap() }
          for i in 1001..=n { writeln!(f, "{}", msg(i, &format!("t{}", i % 1000), if i % 2 == 0 { "user" } else { "assistant" }, "a message of about a hundred bytes, the kind a thread holds many thousands of, for a size check").json()).unwrap() }
          writeln!(f, r#"{{"head":{n}}}"#).unwrap(); }
        let before = rss_kb(); let t0 = std::time::Instant::now();
        let fs = FarStore::open(&h).unwrap();
        let open = t0.elapsed(); let after = rss_kb();
        let t1 = std::time::Instant::now(); let p = { use crate::api::FarView; fs.page("L~t7", 0, 32).unwrap() }; let page = t1.elapsed();
        let t2 = std::time::Instant::now(); let r = fs.replay_events("L"); let replay = t2.elapsed();
        println!("SCALE lines={n} open={:.2}s rss_before={}MB rss_after={}MB (+{}MB) page(32)={:.2}ms rows={} replay={:.2}s replay_events={}", open.as_secs_f64(), before / 1024, after / 1024, (after - before) / 1024, page.as_secs_f64() * 1000.0, p.rows.len(), replay.as_secs_f64(), r.len());
        let _ = std::fs::remove_dir_all(h);
    }
}
