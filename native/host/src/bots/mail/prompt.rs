//! What a bot's agent is told (bothub.bend `Bots.prompt.*`, `Bots.turn`): who it is, how it works, the uberbot's part, a triage run's and a back office's role, and the memory a turn starts
//! with. The words are generated from the Bend source (prompt_text.rs).
use super::{escalate::PRIVATE_BRIEF, prompt_text as T, route::{mem_block, ref_uri, Ctx}};
use crate::bots::{clock::Zones, rpc::{now_s, Bots, HOP_MAX}, sched_text::zone_source, world::{Bot, World}};
use std::sync::Arc;

/// Bots.prompt.k
fn identity(w: &World, b: &Bot, home: &str) -> String {
    let z = Zones { settings: &w.settings }; let h = T::HEAD; let k = T::WORK;
    let persona = if b.persona.is_empty() { String::new() } else { format!("{}{}{}", T::PERSONA[0], b.persona, T::PERSONA[1]) };
    let uber = if w.uber_is(&b.id) { T::UBER } else { "" };
    format!("{}{}{}{}{home}/bots/{}{}{persona}{}{HOP_MAX}{}{}{}{}{}{uber}", h[0], b.name, h[1], h[2], b.id, h[3], k[0], k[1], z.person(), k[2], zone_source(&z), k[3])
}
impl<'a> Ctx<'a> {
    /// Bots.prompt.for
    pub fn bot_prompt(&self, thread: &str, home: &str) -> String {
        let bot = self.w.run_bot(thread).cloned();
        let who = |b: &Option<Bot>| b.as_ref().map_or(String::new(), |b| identity(self.w, b, home));
        if self.w.private_is(thread) { return format!("{}{}{PRIVATE_BRIEF}", who(&bot), self.triage_context(&self.w.private_owner(thread))) }
        if self.w.triage_is(thread) { return format!("{}{}{}{}\n", T::RUN[0], ref_uri("thread", &self.w.triage_thread()), T::RUN[1], super::route::EVIDENCE_TOOLS) }
        let assess = if thread == self.w.triage_thread() && !thread.is_empty() { format!("{}{}{}", T::ASSESS[0], self.triage_audit(&self.w.uber_bot()), T::ASSESS[1]) } else { String::new() };
        format!("{}{assess}", who(&bot))
    }
}
/// Bots.turn.k: the memory a turn of a bot's thread starts with (the words of the message pick the memories)
pub fn memory(w: &World, thread: &str, text: &str, now: u64) -> String {
    let Some(b) = w.run_bot(thread) else { return String::new() };
    let ms: Vec<_> = w.mems.values().filter(|m| m.bot == b.id).collect(); mem_block(&ms, text, now)
}
impl Bots {
    /// Bots.prompt (the language line is the base prompt's, once)
    pub(crate) async fn bot_prompt(self: &Arc<Self>, thread: &str) -> String {
        if !self.is_bot_thread(thread) { return String::new() }
        let w = self.world().await; let f = self.facts(&w, &[]).await; let home = self.hub.cfg.home.to_string_lossy().to_string();
        Ctx::new(&w, &f, now_s(), 0).bot_prompt(thread, &home)
    }
}
