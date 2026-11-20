//! POST /dev/account-retry: the outside watchdog's guard (scripts/hub-watchdog.sh `account_check`): every turn whose start failed on an account error and that nobody has sent
//! again starts again now (`Agents::account_retry_failed`); the answer is `{"retried":[threads]}`. Behind the same token.
//! GET /dev/turns: how many turns this hub is running, and how many threads at rest still wait on background work of their agent (`background`: a shell or a
//! subagent a restart would stop, world.rs `bg_waiting`) and how many running turns the person is waiting on (`waited`), as counts and nothing else. scripts/dev-reload.sh asks before it restarts the hub for a new
//! test build: a restart cuts a running turn, and on a copy nothing resumes it. The hub checks the pairing token first (default-deny).
use super::Agents;
use crate::{api::{Fut, HttpHandler, HttpReply}, auth::Req, hub::Hub};
use std::sync::Arc;

pub struct DevTurns(pub Arc<Agents>);
pub fn running<'a>(turns: impl Iterator<Item = &'a str>) -> usize { turns.filter(|t| *t == "running").count() }
pub fn body(n: usize, bg: usize, waited: usize) -> Vec<u8> { format!("{{\"running\":{n},\"background\":{bg},\"waited\":{waited}}}").into_bytes() }

/// `waited`: running turns the person is waiting on, those of threads they wrote to in the last `WAITED_S` (Bub answering them, a thread they started or
/// steered). fresh-watch.sh holds a new build for these only, and never longer than its cap: every other turn carries on after the restart.
pub const WAITED_S: u64 = 600;
static PERSON_AT: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, u64>>> = std::sync::OnceLock::new();
fn person_at() -> std::sync::MutexGuard<'static, std::collections::HashMap<String, u64>> { PERSON_AT.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner()) }
/// The person's own message reached this thread now (agents/mod.rs, where a message is known to be theirs).
pub fn person_sent(thread: &str, now: u64) { let mut m = person_at(); m.retain(|_, at| now.saturating_sub(*at) < WAITED_S); m.insert(thread.to_string(), now); }
/// Of these running threads, how many the person wrote to within `WAITED_S` of `now`.
pub fn waited<'a>(running: impl Iterator<Item = &'a str>, now: u64) -> usize { let m = person_at(); running.filter(|t| m.get(*t).map_or(false, |at| now.saturating_sub(*at) < WAITED_S)).count() }
impl HttpHandler for DevTurns {
    fn routes(&self) -> &'static [&'static str] { &["/dev/turns"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, req: &'a Req) -> Fut<'a, HttpReply> {
        Box::pin(async move {
            if req.method != "GET" { return HttpReply::text("404 Not Found", "not found") }
            let (n, bg, wt) = { let w = self.0.world().await; let now = crate::agents::now_s();
                (running(w.threads.values().map(|t| t.turn.as_str())), w.bg_waiting().len(), waited(w.threads.iter().filter(|(_, t)| t.turn == "running").map(|(id, _)| id.as_str()), now)) };
            HttpReply { status: "200 OK", ctype: "application/json".into(), extra: "Cache-Control: no-store\r\n".into(), body: body(n, bg, wt) }
        })
    }
}

pub struct AccountRetry(pub Arc<Agents>);
impl HttpHandler for AccountRetry {
    fn routes(&self) -> &'static [&'static str] { &["/dev/account-retry"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, req: &'a Req) -> Fut<'a, HttpReply> {
        Box::pin(async move {
            if req.method != "POST" { return HttpReply::text("405 Method Not Allowed", "POST only") }
            let ths = self.0.account_retry_failed().await;
            HttpReply { status: "200 OK", ctype: "application/json".into(), extra: "Cache-Control: no-store\r\n".into(), body: serde_json::json!({"retried":ths}).to_string().into_bytes() }
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
    #[test] fn the_answer_is_a_count_and_nothing_else() { assert_eq!(String::from_utf8(body(3, 1, 2)).unwrap(), r#"{"running":3,"background":1,"waited":2}"#); }
    #[test] fn a_turn_is_waited_on_for_ten_minutes_after_the_person_wrote() {
        person_sent("tw-a", 1000); person_sent("tw-b", 1000 - WAITED_S);
        assert_eq!(waited(["tw-a", "tw-b", "tw-c"].into_iter(), 1000), 1, "only the thread written to in the window");
        assert_eq!(waited(["tw-a"].into_iter(), 1000 + WAITED_S), 0, "ten minutes on: no longer waited on");
    }
}
