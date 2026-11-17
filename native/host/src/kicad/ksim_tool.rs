//! `kicad_sim` as an MCP tool (docs/kicad-sim.md): its definition, made from ksim.rs's op table and ksim_wave.rs's measures; the call,
//! each of its commands through `Agents::kicad_call` as `kicad_edit`'s are (the approval refusals, the document's lock, the save's
//! commit to main); and the simulation log (ksim_log.rs), with the turn's token cost when the turn ends.
use super::{canonical as kc, kedit, ksim, ksim_log};
use crate::agents::Agents;
use serde_json::{json, Value};
use std::sync::Arc;

fn fail(m: &str) -> (bool, Value) { (false, json!({"error": m})) }

/// The tool as an agent is shown it.
pub fn def() -> Value {
    let ops: String = ksim::OPS.iter().map(|(_, _, shape, what)| format!("- {shape}: {what}.")).collect::<Vec<_>>().join("\n");
    let measures: String = super::ksim_wave::MEASURES.iter().map(|(n, what)| format!("{n} ({what})")).collect::<Vec<_>>().join("; ");
    json!({"name": "kicad_sim", "description": format!("Simulate the schematic with KiCad's simulator (ngspice) in one call: {{path, ops}} opens the schematic, runs the ops in order, saves what they changed and answers numbers, never whole vectors. Parts go by reference, signals as the Simulator names them (\"V(out)\", \"I(R1)\", a net name; \"V(a)/V(b)\" and \"V(a)-V(b)\" of two).\n\
Ops:\n{ops}\n\
Measures ({{name: spec}}; a spec is \"corner V(out)\", {{\"corner\": \"V(out)\", ...parameters}}, or the rest of an ngspice meas statement after its name; `from`, `to` window any of them): {measures}.\n\
A failed simulation answers `failed` and `problems`, one line each with what to do next; the ops after it are not run. Sweeps and tunes change only their runs; the schematic changes only by model, exclude, include, source, probe, analysis, import with `to`, and tune with write.\n\
Models: {}.", ksim::VENDOR_RULE),
        "inputSchema": {"type": "object", "properties": {
            "path": {"type": "string", "description": "Absolute path of the schematic (.kicad_sch)."},
            "ops": {"type": "array", "items": {"type": "object"}, "description": "The steps, in order."},
            "save": {"type": "boolean", "description": "false leaves what the ops changed unsaved (default: saved)."},
            "person_seen": {"type": "boolean", "description": "Send true to repeat a call that was answered with what the person is doing in this KiCad, once your change does not touch it."}},
            "required": ["path", "ops"]}})
}
/// Whether the hub's KiCad (the one a thread's calls go to) has the simulation commands.
pub async fn available(ag: &Arc<Agents>) -> bool { super::ipc::runtime(&ag.hub).await.caps.iter().any(|c| c == ksim::CAP) }

impl Agents {
    /// The MCP tool `kicad_sim` (kicad/ksim.rs).
    pub async fn tool_kicad_sim(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        struct Via<'a> { ag: &'a Arc<Agents>, thread: &'a str, path: &'a str, person: std::sync::Mutex<Value>, caps: Vec<String> }
        impl kedit::Call for Via<'_> {
            fn call<'b>(&'b self, ty: &'b str, msg: Value) -> kedit::Fut<'b> { Box::pin(async move {
                // the person's KiCad has the project: a document it has not opened is theirs to open (no window is opened or raised for an agent)
                if ty.ends_with(".OpenDocument") && super::embed::gui_route(&super::ipc::pro_of(self.path)).is_some() {
                    return Err(format!("{} is not open in the person's KiCad, which has this project: ask them to open it there (Backplane opens no window for an agent)", self.path))
                }
                let v = self.ag.kicad_call(self.thread, self.path, ty, msg).await?;
                if !v["person"].is_null() { *self.person.lock().unwrap() = v["person"].clone() }
                let mut m = v["message"].clone();
                for k in ["not_saved", "format_upgrade", "kicad_load_changes", "detached_kicad"] { if v[k].is_string() { if !m.is_object() { m = json!({}) } m[k] = v[k].clone() } }
                Ok(m)
            }) }
            fn caps(&self) -> Vec<String> { self.caps.clone() }
        }
        let path = a["path"].as_str().unwrap_or("");
        if !crate::plat::fs::is_abs(path) { return fail("path must be the absolute path of the schematic (.kicad_sch) to simulate") }
        let pro = super::ipc::pro_of(path); let env = super::ipc::runtime(&self.hub).await;
        // the KiCad this call goes to: the person's when it has the project, else the hub's headless one
        let (caps, version, gui) = match super::embed::gui_route(&pro) { Some(g) => (g.caps.clone(), g.version.clone(), true), None => (env.caps.clone(), env.version.clone(), false) };
        if !caps.iter().any(|c| c == ksim::CAP) { return fail(&ksim::no_sim(&version)) }
        if ksim::writes(&a["ops"]) && a["person_seen"].as_bool() != Some(true) {
            if let Some(p) = super::coedit::person_now(&pro) { return (false, json!({"error": kedit::person_first(&p), "person": p})) }
        }
        let routed = kc::route(path);
        let project_dir = std::path::Path::new(if pro.is_empty() { routed.as_str() } else { pro.as_str() }).parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let libs: Vec<(std::path::PathBuf, &'static str)> = [("share/kicad/symbols", "kicad"), ("share/kicad/simulation", "kicad"), ("share/kicad/backplane-library", "library")].into_iter()
            .map(|(d, what)| (std::path::Path::new(&env.root).join(d), what)).filter(|(d, _)| d.is_dir()).collect();
        let log = ksim_log::file(&self.hub.cfg.home);
        let ctx = ksim::Ctx { project_dir, libs, now: crate::agents::now_s(), log: Some(log.clone()), gui };
        let via = Via { ag: self, thread, path, person: std::sync::Mutex::new(Value::Null), caps };
        let (r, entries) = ksim::run(&via, path, a, &ctx).await;
        if ksim::writes(&a["ops"]) { super::coedit::agent_wrote(&pro) }
        // the log: one line per op, under the turn that asked (the time it started); the turn's cost joins it when the turn ends
        if !entries.is_empty() && !crate::copy::on() {
            let (turn, ring) = { let w = self.world().await; (w.threads.get(thread).map_or(0, |t| t.running_at), ring_of(&w, thread)) };
            ksim_log::mark(thread, turn, &ring);
            let lines = ksim_log::lines(&entries, crate::agents::now_ms(), thread, turn, &routed);
            let _ = tokio::task::spawn_blocking(move || for l in lines { ksim_log::append(&log, &l) }).await;
        }
        match r {
            Ok(mut v) => { let p = via.person.lock().unwrap().clone(); if !p.is_null() { v["person"] = p }
                if v["saved"].as_str() == Some(path) && a["show"].as_bool().unwrap_or(true) { if self.viewer_show(thread, path, "", false).await.0 { v["shown"] = json!(true) } }
                (true, v) }
            Err(e) => fail(&e),
        }
    }
    /// A turn ended: when it simulated, its token cost is written beside its actions in the simulation log. The reading is the thread's
    /// newest "tokens/cost/context" (state's turn ring), waited for briefly: the provider's usage line and the turn's end come close together.
    pub async fn sim_turn_end(self: &Arc<Self>, thread: &str) {
        let Some((turn, before)) = ksim_log::take(thread) else { return };
        let mut ring = String::new();
        for _ in 0..30 { ring = ring_of(&*self.world().await, thread); if ring != before { break } tokio::time::sleep(std::time::Duration::from_millis(100)).await }
        let last = if ring != before { ring.rsplit(' ').next().unwrap_or("").to_string() } else { String::new() };
        let (log, line) = (ksim_log::file(&self.hub.cfg.home), ksim_log::turn_line(crate::agents::now_ms(), thread, turn, &last));
        let _ = tokio::task::spawn_blocking(move || ksim_log::append(&log, &line)).await;
    }
}
/// The thread's last turns as state keeps them ("tokens/cost/context" each, state/fam.rs `turn_usage`).
fn ring_of(w: &crate::agents::world::World, thread: &str) -> String {
    let ep = format!("{}{}", w.setting("tok.epoch"), w.setting("tok.recount"));
    w.setting(&format!("{}tr.{thread}", if ep.is_empty() { "tok.".to_string() } else { format!("tok.{ep}.") })).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_definition_is_made_from_the_tables() {
        let d = def(); let text = d["description"].as_str().unwrap();
        for (name, _, shape, _) in ksim::OPS { assert!(text.contains(shape), "{name}") }
        for (m, _) in super::super::ksim_wave::MEASURES { assert!(text.contains(&format!("{m} (")), "{m}") }
        assert!(text.contains("stop and ask the person") && text.contains("a login, a licence acceptance or a purchase") && d["name"] == "kicad_sim" && d["inputSchema"]["required"] == json!(["path", "ops"]));
        assert!(text.len() < 9000, "{} characters", text.len());
    }
}
