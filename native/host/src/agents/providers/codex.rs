//! Codex: the reference driver. On Unix, `codex exec --json` takes the turn as arguments as before.
//! On Windows, CreateProcessW caps the command line at 32,767 characters (os error 206), so instructions and MCP config
//! go in a private per-turn profile file (`$CODEX_HOME/bp-<unique>.config.toml`, removed when the turn ends) and the
//! prompt/handoff goes over stdin (`-`), keeping argv short and the full context intact.
use super::{Driver, Made, Reaction, TurnIn};
use serde_json::Value;
use crate::agents::{codex as c, proc};

pub struct CodexDriver { windows: bool, profile: Option<String>, profile_file: Option<proc::RunFile> }
impl CodexDriver {
    pub fn new() -> Self { Self { windows: cfg!(windows), profile: None, profile_file: None } }
    pub fn for_windows() -> Self { Self { windows: true, profile: None, profile_file: None } }
    fn extra(a: &TurnIn) -> Vec<String> { [a.mcp.codex_args.clone(), crate::mcpx::gen::codex_approve(&a.mcp.ids, &a.runtime), a.speed_args.clone()].concat() }
    fn args_in<'a>(a: &'a TurnIn, extra: &'a [String]) -> c::ArgsIn<'a> {
        c::ArgsIn { model: &a.model, session: &a.session, runtime: &a.runtime, effort: &a.effort, think: a.think, images: &a.images, mcp_url: &a.mcp_url, pcbparts: &a.pcbparts_url, prompt: &a.prompt, text: &a.text, extra }
    }
}
impl Default for CodexDriver { fn default() -> Self { Self::new() } }
impl Driver for CodexDriver {
    fn prepare(&mut self, a: &TurnIn) -> Result<(), String> {
        if !self.windows { return Ok(()) }
        let extra = Self::extra(a);
        let toml = c::profile_toml(&Self::args_in(a, &extra));
        let dir = if !a.codex_home.is_empty() { std::path::PathBuf::from(&a.codex_home) } else { proc::run_dir(std::path::Path::new(&a.home)) };
        let name = format!("bp-{}", proc::unique());
        let file = proc::RunFile::private_in(&dir, &format!("{name}.config.toml"), &toml).map_err(|e| format!("could not write Codex profile config: {e}"))?;
        self.profile = Some(name);
        self.profile_file = Some(file);
        Ok(())
    }
    fn argv(&self, a: &TurnIn) -> Vec<String> {
        let extra = Self::extra(a);
        let ai = Self::args_in(a, &extra);
        match &self.profile {
            Some(name) => c::args_with_profile(&ai, name),
            None => c::args(&ai),
        }
    }
    /// a remote server's secret headers (mcpx gen::codex: env_http_headers)
    fn env(&self, a: &TurnIn) -> Vec<(String, String)> { [a.mcp.codex_env.clone(), c::bearer_env(&a.mcp_url)].concat() }
    fn start(&mut self, a: &TurnIn) -> Vec<String> {
        if self.windows { vec![if a.text.is_empty() { " ".into() } else { a.text.clone() }] } else { vec![] }
    }
    fn on_line(&mut self, line: &Value) -> Reaction { if line.is_object() { Reaction { send: vec![], shapes: vec![line.clone()] } } else { Reaction::default() } }
    fn closes_stdin(&self) -> bool { true }
    fn take_files(&mut self) -> Vec<proc::RunFile> { self.profile_file.take().into_iter().collect() }
}
pub fn driver(provider: &str) -> Option<Made> { if provider == "codex" { Some(Made { cli: "codex", driver: Box::new(CodexDriver::new()) }) } else { None } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn codex_passes_its_own_shapes_through_and_ignores_noise() {
        let mut d = CodexDriver::new(); assert_eq!(d.on_line(&serde_json::json!({"type":"item.delta","text":"x"})).shapes.len(), 1); assert!(d.on_line(&Value::String("warning".into())).shapes.is_empty());
        assert!(super::super::driver("codex").is_some());
    }
}
