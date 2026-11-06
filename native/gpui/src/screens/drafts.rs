//! Drafts per thread survive a restart: one small JSON file (0600; drafts can hold pasted secrets) beside the
//! token. Pure load/save plus one background writer that debounces keystrokes and flushes on request.
use super::attach::raw_id;
use std::{collections::HashMap, path::{Path, PathBuf}, sync::mpsc, time::Duration};

pub const MAX_BYTES: usize = 256 * 1024;
const MAX_ONE: usize = 64 * 1024;
const MAX_FILE: u64 = 2 * MAX_BYTES as u64;
const DEBOUNCE: Duration = Duration::from_millis(400);

/// Bounded: a file over the limit, or a draft over one limit, is ignored rather than loaded.
pub fn load(path: &Path) -> HashMap<String, String> {
    if std::fs::metadata(path).map_or(true, |m| m.len() > MAX_FILE) { return HashMap::new(); }
    let m: HashMap<String, String> = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    normalize(m.into_iter().filter(|(k, v)| !k.is_empty() && v.len() <= MAX_ONE).collect())
}

/// Drafts saved before the client always used the thread's own id carry `<hub>|<id>` keys as well: strip them. When both forms hold a draft the
/// longer is kept (a draft being typed grows; the file has no times to say which was last), ties go to the plain key.
pub fn normalize(m: HashMap<String, String>) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::new();
    // qualified keys first, plain ones after: a plain key wins a tie (`>=`), the longer text wins otherwise
    let mut keys: Vec<&String> = m.keys().collect(); keys.sort_by_key(|k| (!k.contains('|'), (*k).clone()));
    for k in keys {
        let (r, v) = (raw_id(k).to_string(), m[k].clone());
        if out.get(&r).map_or(true, |old| v.len() >= old.len()) { out.insert(r, v); }
    }
    out
}

/// Switching threads: the draft in the box is saved under the thread being left (by its own id) and the target's draft is returned.
pub fn switch(map: &mut HashMap<String, String>, old: &str, new: &str, current: &str) -> String {
    let o = raw_id(old);
    if !o.is_empty() { if current.trim().is_empty() { map.remove(o); } else { map.insert(o.to_string(), current.to_string()); } }
    map.get(raw_id(new)).cloned().unwrap_or_default()
}

/// Which drafts are kept: not blank, none over MAX_ONE, and in key order while the total stays within MAX_BYTES.
pub fn kept(drafts: &HashMap<String, String>) -> Vec<(&String, &String)> {
    let mut all: Vec<_> = drafts.iter().filter(|(k, v)| !k.is_empty() && !v.trim().is_empty() && v.len() <= MAX_ONE).collect();
    all.sort();
    let mut total = 0usize;
    all.into_iter().filter(|(_, v)| if total + v.len() <= MAX_BYTES { total += v.len(); true } else { false }).collect()
}

/// Write through a temp file (mode 0600) so a crash never leaves half a file.
pub fn save(path: &Path, drafts: &HashMap<String, String>) {
    let map: HashMap<_, _> = kept(drafts).into_iter().collect();
    let Ok(text) = serde_json::to_string(&map) else { return };
    let tmp = path.with_extension("tmp");
    let ok = {
        use std::io::Write;
        let mut o = std::fs::OpenOptions::new(); o.write(true).create(true).truncate(true);
        #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; o.mode(0o600); }
        o.open(&tmp).and_then(|mut f| f.write_all(text.as_bytes())).is_ok()
    };
    if ok { let _ = std::fs::rename(tmp, path); }
}

enum Msg { Set(String, String), Flush(mpsc::Sender<()>) }

/// One background writer: `set` is cheap, the file is written after DEBOUNCE of quiet, `flush` writes now.
pub struct Writer { tx: mpsc::Sender<Msg> }
impl Writer {
    pub fn start(path: PathBuf, initial: HashMap<String, String>) -> Self {
        let (tx, rx) = mpsc::channel::<Msg>();
        std::thread::spawn(move || {
            let mut map = initial; let mut dirty = false;
            loop {
                match rx.recv_timeout(DEBOUNCE) {
                    Ok(Msg::Set(k, v)) => { if v.is_empty() { map.remove(&k); } else { map.insert(k, v); } dirty = true; }
                    Ok(Msg::Flush(ack)) => { if dirty { save(&path, &map); dirty = false; } let _ = ack.send(()); }
                    Err(mpsc::RecvTimeoutError::Timeout) => { if dirty { save(&path, &map); dirty = false; } }
                    Err(mpsc::RecvTimeoutError::Disconnected) => { if dirty { save(&path, &map); } return; }
                }
            }
        });
        Self { tx }
    }
    pub fn set(&self, thread: &str, text: &str) { let _ = self.tx.send(Msg::Set(thread.to_string(), text.to_string())); }
    /// Write what is pending now, without waiting.
    pub fn kick(&self) { let (a, _r) = mpsc::channel(); let _ = self.tx.send(Msg::Flush(a)); }
    /// Block (up to a second) until everything sent so far is on disk.
    pub fn flush(&self) { let (a, r) = mpsc::channel(); if self.tx.send(Msg::Flush(a)).is_ok() { let _ = r.recv_timeout(Duration::from_secs(1)); } }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dir(n: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("gpui-drafts-{n}-{}", std::process::id())); std::fs::create_dir_all(&d).unwrap(); d }
    #[test] fn drafts_round_trip_and_blank_ones_are_dropped() {
        let d = dir("rt"); let f = d.join("drafts.json");
        let mut m = HashMap::new(); m.insert("t1".to_string(), "line one\nline two 😀".to_string()); m.insert("t2".into(), "  \n".into()); m.insert("".into(), "x".into());
        save(&f, &m);
        let back = load(&f);
        assert_eq!(back.len(), 1); assert_eq!(back["t1"], "line one\nline two 😀");
        assert!(load(&d.join("none.json")).is_empty());
        let _ = std::fs::remove_dir_all(d);
    }
    #[test] fn a_draft_survives_a_switch_away_and_back_after_a_hub_set_selection() {
        // the selection was set by the hub's picture, in its qualified form; the person types, switches to t2 and back
        let mut map = HashMap::new();
        let back_at_t2 = switch(&mut map, "127.0.0.1:3834|t1", "t2", "hello world"); assert_eq!(back_at_t2, "");
        let back_at_t1 = switch(&mut map, "t2", "127.0.0.1:3834|t1", ""); assert_eq!(back_at_t1, "hello world", "the draft written under the qualified form must come back");
        assert!(!map.contains_key("127.0.0.1:3834|t1"), "one form only"); assert_eq!(map["t1"], "hello world");
        // a cleared box forgets the draft
        let _ = switch(&mut map, "t1", "t2", "  "); assert!(!map.contains_key("t1"));
    }
    #[test] fn saved_drafts_under_qualified_keys_are_not_lost_on_load() {
        let mut m = HashMap::new();
        m.insert("h:1|t1".to_string(), "typed under the hub id".to_string()); m.insert("t2".into(), "plain".into());
        m.insert("t3".to_string(), "short".to_string()); m.insert("h:1|t3".to_string(), "the longer one is kept".to_string());
        let n = normalize(m);
        assert_eq!(n["t1"], "typed under the hub id"); assert_eq!(n["t2"], "plain"); assert_eq!(n["t3"], "the longer one is kept"); assert!(n.keys().all(|k| !k.contains('|')));
        let mut tie = HashMap::new(); tie.insert("h|x".to_string(), "aaaa".to_string()); tie.insert("x".to_string(), "bbbb".to_string()); assert_eq!(normalize(tie)["x"], "bbbb", "a tie goes to the plain key");
    }
    #[test] fn one_huge_draft_does_not_drop_the_others() {
        let mut m = HashMap::new(); m.insert("a".to_string(), "x".repeat(MAX_ONE + 1)); m.insert("b".into(), "ok".into()); m.insert("c".into(), "y".repeat(MAX_ONE));
        for i in 0..6 { m.insert(format!("d{i}"), "z".repeat(MAX_ONE)); }
        let k = kept(&m);
        assert!(k.iter().all(|(_, v)| v.len() <= MAX_ONE)); assert!(k.iter().any(|(k, _)| *k == "b"));
        assert!(k.iter().map(|(_, v)| v.len()).sum::<usize>() <= MAX_BYTES);
    }
    #[test] fn the_file_is_private_and_load_is_bounded() {
        let d = dir("perm"); let f = d.join("drafts.json");
        let mut m = HashMap::new(); m.insert("t".to_string(), "secret".to_string()); save(&f, &m);
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; assert_eq!(std::fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o600); }
        std::fs::write(&f, vec![b' '; MAX_FILE as usize + 10]).unwrap(); assert!(load(&f).is_empty());
        let _ = std::fs::remove_dir_all(d);
    }
    #[test] fn the_writer_debounces_and_flushes() {
        let d = dir("w"); let f = d.join("drafts.json");
        let w = Writer::start(f.clone(), HashMap::new());
        for i in 0..50 { w.set("t1", &format!("draft {i}")); }
        assert!(!f.exists(), "written before the debounce");
        w.flush(); assert_eq!(load(&f)["t1"], "draft 49");
        w.set("t1", ""); w.flush(); assert!(load(&f).is_empty());
        let _ = std::fs::remove_dir_all(d);
    }
}
