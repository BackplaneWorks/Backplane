//! GET /dev/turns: how many turns this hub is running, as a count and nothing else. scripts/dev-reload.sh asks before it restarts the hub for a new
//! test build: a restart cuts a running turn, and on a copy nothing resumes it. The hub checks the pairing token first (default-deny).
use super::Agents;
use crate::{api::{Fut, HttpHandler, HttpReply}, auth::Req, hub::Hub};
use std::sync::Arc;

pub struct DevTurns(pub Arc<Agents>);
pub fn running<'a>(turns: impl Iterator<Item = &'a str>) -> usize { turns.filter(|t| *t == "running").count() }
pub fn body(n: usize) -> Vec<u8> { format!("{{\"running\":{n}}}").into_bytes() }
impl HttpHandler for DevTurns {
    fn routes(&self) -> &'static [&'static str] { &["/dev/turns"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, req: &'a Req) -> Fut<'a, HttpReply> {
        Box::pin(async move {
            if req.method != "GET" { return HttpReply::text("404 Not Found", "not found") }
            let n = { let w = self.0.world().await; running(w.threads.values().map(|t| t.turn.as_str())) };
            HttpReply { status: "200 OK", ctype: "application/json".into(), extra: "Cache-Control: no-store\r\n".into(), body: body(n) }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn only_a_running_turn_counts() {
        assert_eq!(running(["running", "completed", "interrupted", "failed", "", "running"].into_iter()), 2);
        assert_eq!(running(std::iter::empty()), 0);
    }
    #[test] fn the_answer_is_a_count_and_nothing_else() { assert_eq!(String::from_utf8(body(3)).unwrap(), r#"{"running":3}"#); }
}
