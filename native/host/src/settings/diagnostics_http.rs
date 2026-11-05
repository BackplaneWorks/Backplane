//! GET /debug/diagnostics.json (server.bend, docs/diagnostics-backend.md): the sanitized, bounded local diagnostics document, read by the client
//! only when the person elects diagnostics for a report. The hub has already checked the pairing token. No raw logs; `diagnostics::export()` re-allowlists
//! the persisted bytes and is capped at 1 MiB.
use crate::{api::{Fut, HttpHandler, HttpReply}, auth::Req, hub::Hub};
use std::sync::Arc;

pub struct DiagnosticsJson;
impl HttpHandler for DiagnosticsJson {
    fn routes(&self) -> &'static [&'static str] { &["/debug/diagnostics.json"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, _req: &'a Req) -> Fut<'a, HttpReply> {
        Box::pin(async { match tokio::task::spawn_blocking(super::diagnostics::export).await {
            Ok(text) => HttpReply { status: "200 OK", ctype: "application/json".into(), extra: "Cache-Control: no-store\r\n".into(), body: text.into_bytes() },
            Err(_) => HttpReply::text("503 Service Unavailable", "diagnostics unavailable") } })
    }
}
