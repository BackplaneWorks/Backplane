//! The state policies' host in the Rust hub: one lean copy of the fold (no entry ids; bounded by threads, settings and thread pairs),
//! fed by the hub's commits (`Observer`), asked on the minute tick (`Ticker`). What a policy decides it commits as events, as the Bend hub's
//! Step does; the events it commits come back through the observer and are not folded twice (`expect`).
//! Declared from main.rs as `#[path = "state/host.rs"] mod statehost;` and started with `statehost::start(hub.clone()).await` next to files::start.
use crate::api::{Fut, Observer, Ticker};
use crate::hub::Hub;
use crate::state::{fam::autosettle, State};
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

struct Inner {
    st: State,
    /// log lines folded so far (1-based count): a batch already inside it is skipped
    applied: u64,
    /// batches seen while the log was being read at start
    booting: Option<Vec<(u64, Vec<Value>)>>,
    /// events this host committed and already folded: skipped once when they come back
    expect: VecDeque<Value>,
}
pub struct Host { inner: Mutex<Inner> }

impl Inner {
    /// fold a batch the hub committed and return the events the policies derive from it (already folded here)
    fn batch(&mut self, first_seq: u64, events: &[Value]) -> Vec<Value> {
        let end = first_seq + events.len() as u64 - 1;
        if events.is_empty() || end <= self.applied { return vec![] }
        let skip = self.applied.saturating_sub(first_seq.saturating_sub(1)) as usize;
        let events = &events[skip.min(events.len())..];
        let mut derived = vec![];
        let quiet = autosettle::quiet(&self.st, events);
        for e in events {
            if self.expect.front() == Some(e) { self.expect.pop_front(); continue }
            let meta = autosettle::metadata(&self.st, e);
            self.st.apply(e);
            for m in &meta { self.st.apply(m); }
            derived.extend(meta);
        }
        for q in &quiet { self.st.apply(q); }
        derived.extend(quiet);
        self.applied = end;
        for d in &derived { self.expect.push_back(d.clone()); }
        // the expectation list is a short queue of events in flight; a stale head (a commit that failed) must not pile up
        while self.expect.len() > 4096 { self.expect.pop_front(); }
        derived
    }
}

impl Observer for Host {
    fn committed(&self, hub: &Arc<Hub>, events: &[Value], first_seq: u32) {
        let derived = {
            let mut g = self.inner.lock().unwrap();
            if let Some(buf) = g.booting.as_mut() { buf.push((first_seq as u64, events.to_vec())); return }
            g.batch(first_seq as u64, events)
        };
        if derived.is_empty() { return }
        let hub = hub.clone();
        tokio::spawn(async move { let _ = hub.commit_many(&derived).await; });
    }
}
impl Ticker for Host {
    fn tick<'a>(&'a self, _hub: &'a Arc<Hub>, now_s: u64) -> Fut<'a, Vec<Value>> {
        Box::pin(async move { let g = self.inner.lock().unwrap(); autosettle::changes(&g.st, now_s) })
    }
}

/// Read the log into the lean state, then take over: start the observer first so no commit is missed, fold lines 1..=len, then the batches
/// the observer held that are beyond that.
pub async fn start(hub: Arc<Hub>) -> Arc<Host> {
    let mut st = State::new(); st.lean = true;
    let host = Arc::new(Host { inner: Mutex::new(Inner { st, applied: 0, booting: Some(vec![]), expect: VecDeque::new() }) });
    hub.register_observer(host.clone());
    let len = hub.store.len();
    let mut at = 0usize;
    while at < len {
        let to = (at + 2000).min(len);
        let lines = hub.store.range(at, to).unwrap_or_default();
        let mut g = host.inner.lock().unwrap();
        for l in lines { g.st.apply_line(&l); }
        at = to;
    }
    let held = {
        let mut g = host.inner.lock().unwrap();
        g.applied = len as u64;
        g.booting.take().unwrap_or_default()
    };
    let mut derived = vec![];
    { let mut g = host.inner.lock().unwrap(); for (seq, evs) in held { derived.extend(g.batch(seq, &evs)); } }
    if !derived.is_empty() { let _ = hub.commit_many(&derived).await; }
    hub.register_ticker(host.clone());
    host
}
