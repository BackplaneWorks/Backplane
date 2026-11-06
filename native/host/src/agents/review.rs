//! review_comment / review_comments: review comments as Acts of tone `review` filed under the thread; the pure rules are the files lane's (kicad/review.rs).
use super::{hub_id, lines::s, now_s, Agents};
use crate::files::kicad::{bj::J, review as rv};
use serde_json::{json, Value};
use std::sync::Arc;

impl Agents {
    /// review_comment {op: add|resolve|reopen|edit|drop, text, id, ...anchor}: one op, logged as an Act.
    pub async fn tool_review_comment(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        let op = s(a, "op").to_string(); let p = J::of(a);
        if !rv::op_ok(&op, &p) { return (false, json!({"error":"review_comment needs op (add, resolve, reopen, edit, drop), text for add and edit, id for the rest"})) }
        let (now, by) = (now_s(), self.world().await.threads.get(thread).map(|t| t.provider.clone()).unwrap_or_default());
        let aid = hub_id("r", now, self.seq_n());
        let id = if op == "add" { aid.clone() } else { s(a, "id").to_string() };
        self.commit_events(vec![json!({"$":"ActivityLogged","thread":thread,"act":aid,"tone":rv::TONE,"kind":op,"summary":rv::op_summary(&op, &aid, &p, &by),"at":now})]).await;
        (true, json!({"id":id}))
    }
    /// review_comments: the open comments as the agent would read them, and the counts.
    pub async fn tool_review_comments(self: &Arc<Self>, thread: &str) -> (bool, Value) {
        let ops: Vec<rv::Op> = { let w = self.world().await; w.reviews.get(thread).map(|v| v.iter().filter_map(|(k, sm, at)| rv::of(rv::TONE, k, sm, *at)).collect()).unwrap_or_default() };
        let cs = rv::fold(&ops); let text = rv::prompt(&cs);
        (true, json!({"open":rv::count_open(&cs),"resolved":rv::resolved(&cs).len(),"text":if text.is_empty() { "No open comments.".to_string() } else { text }}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn review_ops_fold_into_open_and_resolved_comments() {
        let mut ops = vec![]; let by = "claude";
        let add = J::of(&json!({"text":"fix the trace","file":"b.kicad_pcb","layer":"F.Cu","x":1,"y":2}));
        ops.push(rv::of("review", "add", &rv::op_summary("add", "r1", &add, by), 1).unwrap());
        ops.push(rv::of("review", "add", &rv::op_summary("add", "r2", &J::of(&json!({"text":"second"})), by), 2).unwrap());
        ops.push(rv::of("review", "resolve", &rv::op_summary("resolve", "x", &J::of(&json!({"id":"r2"})), by), 3).unwrap());
        let cs = rv::fold(&ops); assert_eq!(rv::count_open(&cs), 1); assert_eq!(rv::resolved(&cs).len(), 1); assert!(rv::prompt(&cs).contains("fix the trace"));
        assert!(!rv::op_ok("add", &J::of(&json!({"text":"  "}))) && !rv::op_ok("drop", &J::of(&json!({}))) && !rv::op_ok("nope", &J::of(&json!({"id":"x"}))));
    }
}
