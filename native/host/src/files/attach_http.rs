//! POST /attach (see attach.rs) on the hub's streaming-body API (api::HttpHandler::call_body, commit c7bf5f574 on gpui-core).
//! The hub has already checked POST (405), the credential (401), Content-Length present and no Transfer-Encoding (411) and len <= STREAM_MAX (413).
use super::attach;
use crate::{api::{Body, Fut, HttpHandler, HttpReply}, auth::Req, hub::Hub};
use std::sync::Arc;

pub struct Attach;

fn pct(s: &str) -> String { // Att.unpct
    let b = s.as_bytes(); let mut o = Vec::with_capacity(b.len()); let mut i = 0; let hx = |c: u8| (c as char).to_digit(16);
    while i < b.len() { match (b[i], b.get(i + 1).and_then(|c| hx(*c)), b.get(i + 2).and_then(|c| hx(*c))) { (b'%', Some(h), Some(l)) => { o.push((h * 16 + l) as u8); i += 3 } _ => { o.push(b[i]); i += 1 } } }
    String::from_utf8_lossy(&o).into_owned()
}
fn json(status: &'static str, v: serde_json::Value) -> HttpReply { HttpReply { status, ctype: "application/json".into(), extra: String::new(), body: v.to_string().into_bytes() } }

impl HttpHandler for Attach {
    fn routes(&self) -> &'static [&'static str] { &["/attach"] }
    fn takes_body(&self) -> bool { true }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, _req: &'a Req) -> Fut<'a, HttpReply> { Box::pin(async { HttpReply::text("405 Method Not Allowed", "POST only") }) }
    fn call_body<'a>(&'a self, hub: &'a Arc<Hub>, req: &'a Req, mut body: Body<'a>) -> Fut<'a, HttpReply> {
        Box::pin(async move {
            let (thread, key, name) = (pct(&req.param("thread")), pct(&req.param("key")), pct(&req.param("name")));
            // PR #960: an unknown, empty or deleted thread gets no attachment folder
            let known = !thread.is_empty() && hub.model().await.threads.get(&thread).map_or(false, |t| !t.deleted);
            if !known { return json("404 Not Found", attach::failed_plain(&thread, "No folder for this thread or project: unknown, deleted, removed or unavailable")) }
            let lim = hub.setting(attach::KEY).await.unwrap_or_default();
            let len = body.len;
            // refuse a body that cannot fit before telling the client to send it (Upload.into.k: 413 before 100 Continue)
            if len > attach::max_bytes(&lim) { return json("413 Payload Too Large", attach::failure(&thread, "upload.cap", &attach::mb(&lim).to_string())) }
            let _ = body.proceed().await;
            let (status, v) = attach::receive(&hub.cfg.home, &thread, &key, &name, len, &lim, crate::api::STREAM_MAX, &mut body).await;
            json(status, v)
        })
    }
}
