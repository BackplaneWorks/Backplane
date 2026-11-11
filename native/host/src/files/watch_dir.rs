//! The folders an open Files page shows, watched on disk: an agent (or anything) adding, removing, renaming or growing a file there makes the hub tell every view
//! (`Hub::bump_views` → `view.changed`), so the page asks again and draws the new listing without the person pressing anything. A page registers the folder it drew with
//! the signature of what it drew (`touch`, from present/viewer.rs); a 400 ms poll compares the folder's listing now and, on a difference, bumps once. Entries
//! expire five minutes after the last draw, and at most 32 folders are kept, so a closed panel costs nothing for long.
use crate::hub::Hub;
use std::{collections::{hash_map::DefaultHasher, HashMap}, hash::{Hash, Hasher}, path::Path, sync::{Arc, LazyLock, Mutex}, time::{Duration, Instant}};

const KEEP: Duration = Duration::from_secs(300);
const MAX: usize = 32;
type Key = (String, String);
static WATCHED: LazyLock<Mutex<HashMap<Key, (Instant, u64)>>> = LazyLock::new(Default::default);

pub fn signature(listing: &str) -> u64 { let mut h = DefaultHasher::new(); listing.hash(&mut h); h.finish() }
/// A view drew `dir` of `root` showing a listing with this signature.
pub fn touch(root: &str, dir: &str, sig: u64) {
    let mut w = WATCHED.lock().unwrap();
    if w.len() >= MAX && !w.contains_key(&(root.to_string(), dir.to_string())) { if let Some(old) = w.iter().min_by_key(|(_, v)| v.0).map(|(k, _)| k.clone()) { w.remove(&old); } }
    w.insert((root.to_string(), dir.to_string()), (Instant::now(), sig));
}
/// One poll: the folders whose listing differs from what was last drawn or seen (their signature is replaced, so each change is reported once), expired ones dropped.
pub fn poll() -> usize {
    let keys: Vec<(Key, u64)> = { let mut w = WATCHED.lock().unwrap(); w.retain(|_, v| v.0.elapsed() < KEEP); w.iter().map(|(k, v)| (k.clone(), v.1)).collect() };
    let mut changed = 0;
    for ((root, dir), sig) in keys {
        let now = signature(&super::dir::list(Path::new(&root), &dir).unwrap_or_default());
        if now != sig { if let Some(e) = WATCHED.lock().unwrap().get_mut(&(root, dir)) { e.1 = now; changed += 1; } }
    }
    changed
}
pub fn start(hub: Arc<Hub>) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_millis(400));
        loop { tick.tick().await; if WATCHED.lock().unwrap().is_empty() { continue } if tokio::task::spawn_blocking(poll).await.unwrap_or(0) > 0 { hub.bump_views(); } }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_new_file_is_reported_once_and_a_drawn_listing_is_not() {
        let d = crate::plat::tmp::dir("wd");
        let root = d.to_string_lossy().into_owned();
        let sig = |r: &str| signature(&super::super::dir::list(Path::new(r), "").unwrap());
        touch(&root, "", sig(&root));
        assert_eq!(poll(), 0, "what the page drew is current");
        std::fs::write(d.join("new.txt"), "x").unwrap();
        assert_eq!(poll(), 1, "an added file differs"); assert_eq!(poll(), 0, "reported once");
        touch(&root, "", sig(&root)); std::fs::write(d.join("new.txt"), "xyz").unwrap(); assert_eq!(poll(), 1, "a changed size too");
        std::fs::remove_file(d.join("new.txt")).unwrap(); assert_eq!(poll(), 1, "a removed file too");
    }
}
