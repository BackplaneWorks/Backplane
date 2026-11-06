//! Grok (`grok agent stdio`, ACP): server's Grok.args/Grok.perm/Grok.asks/Grok.go and MS.Proto's PAcp (see acp.rs for the protocol).
//! One process per turn: initialize, session/new or session/load, session/prompt; the turn's end ends the process (the actor does).
//! Stop is `AgentStop` alone (law interrupt_grok_stops): no session/cancel is written.
use super::acp::{self, Acp, AcpTurn};
use super::{Driver, Made, Reaction, TurnIn};
use serde_json::Value;

/// Effort.args for grok: `--reasoning-effort <e>` for low, medium, high; none for the default.
pub fn effort_args(e: &str) -> Vec<String> { if ["low", "medium", "high"].contains(&e) { vec!["--reasoning-effort".into(), e.into()] } else { vec![] } }
/// Grok.perm
pub fn perm(runtime: &str) -> Vec<String> {
    let v: &[&str] = match runtime { "approval-required" => &["--permission-mode", "default", "agent"], "auto-accept-edits" => &["--permission-mode", "acceptEdits", "agent"], "auto" => &["--permission-mode", "auto", "agent"], _ => &["agent", "--always-approve"] };
    v.iter().map(|x| x.to_string()).collect()
}
/// Grok.args: the permission mode, the model (when named), the effort, `stdio`.
pub fn args(runtime: &str, model: &str, effort: &str) -> Vec<String> {
    let mut v = perm(runtime); if !model.is_empty() { v.push("-m".into()); v.push(model.into()) } v.extend(effort_args(effort)); v.push("stdio".into()); v
}
/// Grok.asks: whether tool requests go to the person (every mode but full access).
pub fn asks(runtime: &str) -> bool { matches!(runtime, "approval-required" | "auto-accept-edits" | "auto") }
/// MS.Proto.gone / install for grok
pub const GONE: &str = "grok exited before the turn ended (is it signed in? run `grok login`)";
pub const INSTALL: &str = "Install Grok Build with `curl -fsSL https://x.ai/cli/install.sh | bash`, then `grok login`.";

pub struct GrokDriver { pub turn: AcpTurn, pub a: Acp }
impl GrokDriver { pub fn new() -> GrokDriver { GrokDriver { turn: AcpTurn::default(), a: Acp::new() } } }
impl Driver for GrokDriver {
    fn argv(&self, a: &TurnIn) -> Vec<String> { args(&a.runtime, &a.model, &a.effort) }
    fn start(&mut self, a: &TurnIn) -> Vec<String> {
        self.turn = AcpTurn { cwd: a.cwd.clone(), mcp: a.mcp_url.clone(), prompt: a.prompt.clone(), text: a.text.clone(), session: a.session.clone(), ask: asks(&a.runtime) };
        vec![acp::init()]
    }
    fn on_line(&mut self, line: &Value) -> Reaction { let o = acp::line(&mut self.a, &self.turn, line); Reaction { send: o.send, shapes: o.shapes } }
    fn ask_info(&self, r: &Value) -> Option<super::AskInfo> { acp::ask_of(r) }
    fn answer(&mut self, r: &Value, answer: &str) -> Vec<String> { acp::answer_of(r, answer) }
    fn ends_on_turn_end(&self) -> bool { true }
    fn exit_why(&self, _code: i32) -> Option<String> { Some(GONE.into()) } // Proto.gone; the actor adds (exit N | signal N), OOM wording and the stderr tail (Exit.say)
    fn install_hint(&self) -> Option<String> { Some(INSTALL.into()) }
}
pub fn driver(provider: &str) -> Option<Made> { if provider == "grok" { Some(Made { cli: "grok", driver: Box::new(GrokDriver::new()) }) } else { None } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_command_line_follows_the_runtime_mode_model_and_effort() {
        assert_eq!(args("approval-required", "", ""), ["--permission-mode", "default", "agent", "stdio"]);
        assert_eq!(args("auto-accept-edits", "grok-4", "high"), ["--permission-mode", "acceptEdits", "agent", "-m", "grok-4", "--reasoning-effort", "high", "stdio"]);
        assert_eq!(args("auto", "", "xhigh"), ["--permission-mode", "auto", "agent", "stdio"], "an effort grok does not take is dropped");
        assert_eq!(args("full-access", "", "low"), ["agent", "--always-approve", "--reasoning-effort", "low", "stdio"]);
        assert!(!asks("full-access") && asks("approval-required") && asks("auto") && asks("auto-accept-edits"));
    }
    #[test] fn a_process_that_exits_early_says_signed_in_and_how_to_install() {
        let d = GrokDriver::new(); assert_eq!(d.exit_why(1).unwrap(), GONE); assert!(d.install_hint().unwrap().contains("grok login"));
        let o = super::super::opencode::OcDriver::new(); assert!(o.exit_why(1).unwrap().contains("opencode auth login") && o.install_hint().unwrap().contains("opencode.ai"));
        assert_eq!(crate::agents::rules::exit_say(&d.exit_why(137).unwrap(), 137, ""), "agent was killed (signal 9, likely out of memory) (signal 9)");
    }
    // law interrupt_grok_stops: Stop on a Grok thread is AgentStop alone: nothing is written to the process
    #[test] fn interrupt_grok_stops() { let mut d = GrokDriver::new(); assert!(d.on_stop().is_empty()); assert!(driver("grok").is_some() && driver("opencode").is_none()) }
    #[test] fn the_first_line_is_initialize_and_a_turn_through_the_driver_ends() {
        let a = TurnIn { thread: "t".into(), cwd: "/tmp/p".into(), runtime: "full-access".into(), text: "hi".into(), prompt: "R".into(), mcp_url: "http://u".into(), ..Default::default() };
        let mut d = GrokDriver::new(); let st = d.start(&a); assert_eq!(st.len(), 1); assert!(st[0].contains(r#""method":"initialize""#));
        let r = d.on_line(&serde_json::json!({"jsonrpc":"2.0","id":1,"result":{}})); assert!(r.send[0].contains("session/new") && r.send[0].contains(r#""url":"http://u""#));
        d.on_line(&serde_json::json!({"jsonrpc":"2.0","id":2,"result":{"sessionId":"S"}})); assert!(!d.a.done);
        let e = d.on_line(&serde_json::json!({"jsonrpc":"2.0","id":3,"result":{}})); assert_eq!(e.shapes.last().unwrap()["type"], "turn.completed"); assert!(d.a.done);
    }
}
