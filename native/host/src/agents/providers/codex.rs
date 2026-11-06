//! Codex: the reference driver. `codex exec --json` takes the whole turn as arguments and already speaks in shapes, so a line passes through.
use super::{Driver, Made, Reaction, TurnIn};
use serde_json::Value;
use crate::agents::codex as c;

pub struct CodexDriver;
impl Driver for CodexDriver {
    fn argv(&self, a: &TurnIn) -> Vec<String> {
        c::args(&c::ArgsIn { model: &a.model, session: &a.session, runtime: &a.runtime, effort: &a.effort, think: a.think, images: &a.images, mcp_url: &a.mcp_url, pcbparts: &a.pcbparts_url, prompt: &a.prompt, text: &a.text })
    }
    fn on_line(&mut self, line: &Value) -> Reaction { if line.is_object() { Reaction { send: vec![], shapes: vec![line.clone()] } } else { Reaction::default() } }
}
pub fn driver(provider: &str) -> Option<Made> { if provider == "codex" { Some(Made { cli: "codex", driver: Box::new(CodexDriver) }) } else { None } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn codex_passes_its_own_shapes_through_and_ignores_noise() {
        let mut d = CodexDriver; assert_eq!(d.on_line(&serde_json::json!({"type":"item.delta","text":"x"})).shapes.len(), 1); assert!(d.on_line(&Value::String("warning".into())).shapes.is_empty());
        assert!(super::super::driver("codex").is_some());
    }
}
