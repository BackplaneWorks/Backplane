//! The viewers of a project on another of the owner's machines: the drawing is made where the files are.
//!
//! Owning side: a request of `Router` (`viewer.call`) runs the viewers' own methods (`kicad.watch`, `kicad.info`, ...) for a client of its own that
//! stands for one window on the other machine (`sub`); what those methods push to a client (plot frames, notes, sheets) is held in that client's outbox
//! until `viewer.pull` takes it. The pulling side forwards each frame to its real client as it came, so the picture is the owner's, not drawn here.
use crate::hub::{Conn, Hub, Out, Outbox};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::{Arc, Mutex, OnceLock}, time::{Duration, Instant}};

/// The methods a window of another machine may run here for what it watches: only the viewers' reads.
pub const METHODS: &[&str] = &["kicad.watch", "kicad.info", "kicad.warm", "kicad.probe", "history.steps"];
pub fn is_viewer(m: &str) -> bool { METHODS.contains(&m) }
/// The most a window may hold open here at once (a thread of its own per window), and how long one may sit unread.
const MAX_SUBS: usize = 64;
const IDLE: Duration = Duration::from_secs(120);
/// The most one pull carries (a whole model may be larger: it goes alone).
const PULL_BYTES: usize = 6 << 20;
const PULL_WAIT: Duration = Duration::from_millis(900);

struct Sub { client: u64, out: Arc<tokio::sync::Mutex<Outbox>>, seen: Instant }
fn subs() -> &'static Mutex<HashMap<(String, u64), Sub>> { static S: OnceLock<Mutex<HashMap<(String, u64), Sub>>> = OnceLock::new(); S.get_or_init(Default::default) }

fn sub_of(hub: &Arc<Hub>, link: &str, sub: u64, create: bool) -> Option<(u64, Arc<tokio::sync::Mutex<Outbox>>)> {
    let mut m = subs().lock().unwrap();
    let old: Vec<(String, u64)> = m.iter().filter(|(_, s)| s.seen.elapsed() > IDLE).map(|(k, _)| k.clone()).collect();
    for k in old { if let Some(s) = m.remove(&k) { hub.detach(s.client); } }
    if let Some(s) = m.get_mut(&(link.to_string(), sub)) { s.seen = Instant::now(); return Some((s.client, s.out.clone())) }
    if !create || m.len() >= MAX_SUBS { return None }
    let client = hub.fresh_client(); let out = Arc::new(tokio::sync::Mutex::new(hub.attach(client)));
    m.insert((link.to_string(), sub), Sub { client, out: out.clone(), seen: Instant::now() });
    Some((client, out))
}

/// `viewer.call {sub, m, p}`: the viewer method `m` for the window `sub` of `link`; the reply frame is the answer.
pub async fn call(hub: &Arc<Hub>, link: &str, p: &Value) -> Value {
    let (sub, m) = (p["sub"].as_u64().unwrap_or(0), p["m"].as_str().unwrap_or(""));
    if !is_viewer(m) { return json!({"t":"reply","ok":false,"error":"not something this machine shows"}) }
    let Some((client, _)) = sub_of(hub, link, sub, true) else { return json!({"t":"reply","ok":false,"error":"too many windows of other machines are open here"}) };
    let mut c = Conn { sent: vec![], client, gen: "far".into(), view_id: String::new(), target: Value::Null, cbor: false, lastrev: 0 };
    let out = hub.request_here(&mut c, &json!({"id": 0, "m": m, "p": p.get("p").cloned().unwrap_or(Value::Null)})).await;
    out.into_iter().find(|v| v["t"] == "reply").unwrap_or_else(|| json!({"t":"reply","ok":false,"error":"no answer"}))
}
/// `viewer.pull {sub}`: what was pushed to that window since the last pull, waiting a moment for the first: `{"frames":[{"j": json} | {"r": base64}], "gone": bool}`.
pub async fn pull(hub: &Arc<Hub>, link: &str, p: &Value) -> Value {
    use base64::Engine;
    let Some((_, out)) = sub_of(hub, link, p["sub"].as_u64().unwrap_or(0), false) else { return json!({"frames":[], "gone": true}) };
    let mut out = out.lock().await;
    let (mut frames, mut bytes) = (vec![], 0usize);
    let mut take = |o: Out, frames: &mut Vec<Value>, bytes: &mut usize| match o {
        Out::Value(v) => { *bytes += v.to_string().len(); frames.push(json!({"j": v})) }
        Out::Raw(b) => { *bytes += b.len(); frames.push(json!({"r": base64::engine::general_purpose::STANDARD.encode(b)})) }
        Out::Close => {}
    };
    match tokio::time::timeout(PULL_WAIT, out.recv()).await { Ok(Some(o)) => take(o, &mut frames, &mut bytes), Ok(None) => return json!({"frames":[], "gone": true}), Err(_) => return json!({"frames":[], "gone": false}) }
    while bytes < PULL_BYTES { match out.try_recv() { Ok(o) => take(o, &mut frames, &mut bytes), Err(_) => break } }
    json!({"frames": frames, "gone": false})
}
/// `viewer.drop {sub}`: the window left: its watches stop and its outbox goes.
pub async fn drop_sub(hub: &Arc<Hub>, link: &str, p: &Value) -> Value {
    let sub = p["sub"].as_u64().unwrap_or(0);
    let gone = subs().lock().unwrap().remove(&(link.to_string(), sub));
    if let Some(s) = gone {
        let mut c = Conn { sent: vec![], client: s.client, gen: "far".into(), view_id: String::new(), target: Value::Null, cbor: false, lastrev: 0 };
        let _ = hub.request_here(&mut c, &json!({"id": 0, "m": "kicad.watch", "p": {"kind": ""}})).await;
        hub.detach(s.client);
    }
    json!({"t":"reply","ok":true})
}
/// The frames of a pull as pushes to a client: JSON frames and binary frames, in order.
pub fn frames_of(body: &Value) -> Vec<Out> {
    use base64::Engine;
    body["frames"].as_array().into_iter().flatten().filter_map(|f| {
        if let Some(j) = f.get("j") { Some(Out::Value(j.clone())) }
        else { f["r"].as_str().and_then(|r| base64::engine::general_purpose::STANDARD.decode(r).ok()).map(Out::Raw) }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn only_the_viewers_reads_are_shown_to_another_machine() {
        for m in ["kicad.watch", "kicad.info", "kicad.warm", "kicad.probe", "history.steps"] { assert!(is_viewer(m)); }
        for m in ["kicad.open", "turn.start", "fs.list", "thread.delete"] { assert!(!is_viewer(m), "{m}"); }
    }
    #[test] fn pulled_frames_come_out_as_they_went_in() {
        use base64::Engine;
        let body = json!({"frames":[{"j":{"t":"plot","key":"0|/p|"}},{"r": base64::engine::general_purpose::STANDARD.encode([1u8,2,3])},{"r":"!!"}]});
        let f = frames_of(&body); assert_eq!(f.len(), 2, "a frame that is not base64 is dropped");
        assert!(matches!(&f[0], Out::Value(v) if v["key"] == "0|/p|")); assert!(matches!(&f[1], Out::Raw(b) if b == &[1, 2, 3]));
    }
}
