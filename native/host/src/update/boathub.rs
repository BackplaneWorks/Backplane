//! The always-on Boat hub's cost (docs/BOAT-HUB.md): `scripts/boat-hub.sh cost --save`, run on the desktop (which has the `boat` CLI
//! signed in), writes one JSON line to `~/.config/backplane/boat-hub/cost.json` there and on the box. Every five minutes this reads it into the
//! info `boathub.cost`; Settings, Network draws it (`present::settings::boat_hub_rows`). A hub with no such file shows nothing.
use crate::{api::{Fut, Ticker}, hub::Hub};
use serde_json::Value;
use std::{path::PathBuf, sync::{atomic::{AtomicU64, Ordering::SeqCst}, Arc}};

/// `BACKPLANE_BOAT_COST`, else `~/.config/backplane/boat-hub/cost.json`; a test hub reads none (its HOME is the person's).
fn path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("BACKPLANE_BOAT_COST") { return (!p.is_empty()).then(|| PathBuf::from(p)) }
    if cfg!(test) { return None }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/backplane/boat-hub/cost.json"))
}
/// The file's one line when it is a JSON object of a sane size, else "" (the info key is then removed).
pub fn read(p: &std::path::Path) -> String {
    let s = std::fs::read_to_string(p).unwrap_or_default();
    let s = s.trim();
    if s.len() > 4096 { return String::new() }
    match serde_json::from_str::<Value>(s) { Ok(v) if v.is_object() => v.to_string(), _ => String::new() }
}

#[derive(Default)]
pub struct Cost { last: AtomicU64 }
impl Ticker for Cost {
    fn every_s(&self) -> u64 { 60 }
    fn tick<'a>(&'a self, hub: &'a Arc<Hub>, now_s: u64) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let last = self.last.load(SeqCst);
            if last != 0 && now_s < last + 300 { return vec![] }
            self.last.store(now_s.max(1), SeqCst);
            if let Some(p) = path() { hub.set_info("boathub.cost", &read(&p)).await; }
            vec![]
        })
    }
}
pub fn install(hub: &Arc<Hub>) { hub.register_ticker(Arc::new(Cost::default())); }

#[cfg(test)]
mod tests {
    #[test] fn only_a_json_object_is_read() {
        let d = std::env::temp_dir().join(format!("bp-boathub-{}", std::process::id())); std::fs::create_dir_all(&d).unwrap();
        let f = d.join("cost.json");
        for (body, want) in [("{\"rate_hour\":0.018}\n", "{\"rate_hour\":0.018}"), ("[1]", ""), ("not json", ""), ("", "")] {
            std::fs::write(&f, body).unwrap(); assert_eq!(super::read(&f), want, "{body}");
        }
        assert_eq!(super::read(&d.join("missing")), "");
        let _ = std::fs::remove_dir_all(&d);
    }
}
