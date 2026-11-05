//! Per-thread cache of the last picture the hub drew (perf gate: a cached thread switch at 100 ms or less).
//! Selecting a thread shows its cached picture at once; the hub's fresh one replaces it when it lands.
use serde_json::Value;

pub struct PictureCache { cap: usize, items: Vec<(String, Value)> }

impl PictureCache {
    pub fn new(cap: usize) -> Self { Self { cap, items: Vec::new() } }
    pub fn put(&mut self, id: &str, thread: Value) {
        self.items.retain(|(k, _)| k != id);
        self.items.push((id.to_string(), thread));
        if self.items.len() > self.cap { self.items.remove(0); }
    }
    /// The cached thread picture, most recently used first.
    pub fn get(&mut self, id: &str) -> Option<Value> {
        let at = self.items.iter().position(|(k, _)| k == id)?;
        let item = self.items.remove(at);
        let v = item.1.clone();
        self.items.push(item);
        Some(v)
    }
    #[cfg(test)] pub fn len(&self) -> usize { self.items.len() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test] fn returns_what_was_put_and_evicts_the_least_recent() {
        let mut c = PictureCache::new(2);
        c.put("a", json!({"n":1})); c.put("b", json!({"n":2}));
        assert_eq!(c.get("a").unwrap()["n"], 1);   // a is now most recent
        c.put("c", json!({"n":3}));                // evicts b
        assert!(c.get("b").is_none()); assert!(c.get("a").is_some() && c.get("c").is_some());
        assert_eq!(c.len(), 2);
    }
    #[test] fn put_replaces_the_same_thread() {
        let mut c = PictureCache::new(3); c.put("a", json!(1)); c.put("a", json!(2));
        assert_eq!(c.len(), 1); assert_eq!(c.get("a").unwrap(), json!(2));
    }
}
