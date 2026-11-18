//! What an agent is told: src/core/prompt's text assembly, with the
//! literals generated from source (prompt_text.rs, by test/agents/gen_prompt.py) so the words cannot drift.
//! The golden test compares the whole string with the argv the JS hub started a stand-in Claude with
//! (test/agents/fixtures/argv-default.json); names are kept on each fn.
use super::prompt_text as T;

/// What the host learned of KiCad: status ok|stock|old|broken|checking|missing, the CLI path, version, commit, why it broke.
#[derive(Clone, Debug, Default)]
pub struct Kicad { pub status: String, pub cli: String, pub version: String, pub commit: String, pub why: String }

/// Everything Hub.prompt reads, given by the hub that discovered it (FreeCAD, the KiCad runtime, the models line, secrets, settings).
#[derive(Clone, Debug)]
pub struct PromptEnv {
    pub fc: String, pub render: String, pub kicad: Kicad, pub kicad_ipc: bool, pub models: String, pub may_delegate: bool, pub orders_on: bool,
    /// The person turned on Plan before routing (plan/mod.rs `SETTING`).
    pub routing_plan: bool,
    pub root: String, pub pro: String, pub pcb: String, pub sch: String, pub all: String,
    /// Where this thread's design files and code go, and the document locks (`design_words`).
    pub design: String,
    pub keys: String, pub machines: String, pub buy: bool, pub lang: String,
    /// Backplane's own skills (bundled.rs): (name, description, SKILL.md path)
    pub skills: Vec<(String, String, String)>,
    /// KiStack's copy the agents get (kistack.rs); None when the hub has none, and then no prompt claims it
    pub kistack: Option<crate::agents::kistack::Copy>,
}
impl Default for PromptEnv {
    fn default() -> Self { PromptEnv { fc: String::new(), render: String::new(), kicad: Kicad { status: "missing".into(), ..Default::default() }, kicad_ipc: true, models: String::new(), may_delegate: true, orders_on: true, routing_plan: false,
        root: String::new(), pro: String::new(), pcb: String::new(), sch: String::new(), all: String::new(), design: String::new(), keys: String::new(), machines: String::new(), buy: false, lang: String::new(), skills: vec![], kistack: None } }
}

/// Prompt.runtime
pub fn runtime(harness: &str) -> String { let r = T::PROMPT_RUNTIME; format!("{}{}{}{}{}{}", r[0], harness, r[1], r[2], r[3], T::PROMPT_BOARD[0]) }

/// Prompt.mech.gui: FreeCAD's GUI program beside its console one.
pub fn mech_gui(fc: &str) -> String {
    let cut = |n: usize, add: &str| format!("{}{add}", &fc[..fc.len() - n]);
    if fc.ends_with("FreeCADCmd.exe") || fc.ends_with("freecadcmd.exe") { return cut(14, "FreeCAD.exe") }
    if fc.ends_with("/Resources/bin/freecadcmd") { cut(25, "/MacOS/FreeCAD") } else if fc.ends_with("freecadcmd") { cut(10, "freecad") } else if fc.ends_with("FreeCADCmd") { cut(10, "FreeCAD") } else { fc.to_string() }
}
/// Prompt.mech
pub fn mech(fc: &str, render: &str) -> String {
    let (f, l, m) = (T::PROMPT_MECH_FREECAD, T::PROMPT_MECH_LOOP, T::PROMPT_MECH);
    let free = if fc.is_empty() { f[0].to_string() } else { format!("{}{}{}{}{}", f[1], mech_gui(fc), f[2], fc, f[3]) };
    format!("{}{}{}{}{}{}", m[0], free, l[0], render, l[1], m[1])
}
/// Cfg.prompt: what the hub is configured with (Main.config)
pub fn config_prompt(fc: &str, render: &str) -> String { format!("{}\n\n{}", runtime("Backplane"), mech(fc, render)) }

/// Prompt.kicad.basics / fork / k / lead / health
fn kicad_basics(cli: &str) -> String { let b = T::PROMPT_KICAD_BASICS; let mut s = b[0].to_string(); for i in 1..7 { s += cli; s += b[i] } s }
fn kicad_fork(version: &str, commit: &str) -> String { let f = T::PROMPT_KICAD_FORK; format!("{}{}{}{}{}{}{}", f[0], version, f[1], commit, f[2], commit, f[3]) }
fn kicad_k(k: &Kicad, ipc: bool) -> String {
    let t = T::PROMPT_KICAD_K; let fb = T::PROMPT_KICAD_FALLBACK[0];
    match (k.status.as_str(), ipc) {
        ("ok", true) => kicad_fork(&k.version, &k.commit),
        ("ok", false) => format!("{}{}{}{fb}", t[2], k.version, t[3]),
        ("stock", _) => format!("{}{}{}{fb}", t[5], k.version, t[6]),
        ("old", _) => format!("{}{}{}{}{}{fb}", t[8], k.version, t[9], min_version(), t[10]),
        ("broken", _) => format!("{}{}{}{}{}{fb}", t[12], k.cli, t[13], why(&k.why), t[14]),
        ("checking", _) => format!("{}{fb}", t[16]),
        _ => format!("{}{fb}", t[17]),
    }
}
/// KH.min_version / KH.why
pub fn min_version() -> &'static str { "10.99.0-bp.4" }
pub fn why(text: &str) -> String { text.lines().next().unwrap_or("").trim().chars().take(160).collect() }
fn kicad_lead(k: &Kicad) -> String { match k.status.as_str() { "missing" | "broken" | "checking" => format!("{}{}{}", T::PROMPT_KICAD_LEAD[1], k.cli, T::PROMPT_KICAD_LEAD[2]), _ => kicad_basics(&k.cli) } }
pub fn kicad(k: &Kicad, ipc: bool) -> String { let h = T::PROMPT_KICAD_HEALTH; format!("{}{}{}{}", h[0], kicad_lead(k), kicad_k(k, ipc), h[1]) }

/// Prompt.path / project.k / projects / project
fn path(label: &str, p: &str) -> String { if p.is_empty() { String::new() } else { let t = T::PROMPT_PATH; format!("{}{label}{}{p}{}", t[1], t[2], t[3]) } }
pub fn project(root: &str, pro: &str, pcb: &str, sch: &str, all: &str) -> String { project_locked(root, pro, pcb, sch, all, "") }
/// `design`: where design files and code go and the documents' locks (`design_words`), after the files.
pub fn project_locked(root: &str, pro: &str, pcb: &str, sch: &str, all: &str, design: &str) -> String {
    let (p, k, ps) = (T::PROMPT_PROJECT, T::PROMPT_PROJECT_K, T::PROMPT_PROJECTS);
    let held = if design.is_empty() { String::new() } else { format!(" {design}") };
    let body = if pcb.is_empty() && sch.is_empty() { k[0].to_string() } else { format!("{}{}{}{}{}{held}", k[1], path(&k[2], pro), path(&k[3], pcb), path(&k[4], sch), k[5]) };
    let ls: Vec<&str> = all.lines().collect();
    let several = if ls.len() > 1 { let l = T::PROMPT_LIST; format!("{}{}{}", ps[0], ls.iter().map(|x| format!("{}{x}{}", l[1], l[2])).collect::<String>(), ps[1]) } else { String::new() };
    format!("{}{root}{}{body}{several}{}", p[0], p[1], p[2])
}
/// Prompt.orchestration / background
pub fn orchestration(may: bool, models: &str) -> String { let (k, o) = (T::PROMPT_ORCHESTRATION_K, T::PROMPT_ORCHESTRATION); if may { format!("{}{}{models}{}{}", o[0], k[0], k[1], o[1]) } else { format!("{}{}{}", o[0], k[2], o[1]) } }
pub fn background() -> &'static str { T::PROMPT_BACKGROUND[0] }
/// Order.prompt.project.k
pub fn order_project(on: bool) -> String {
    let (k, i, l, f) = (T::ORDER_PROMPT_PROJECT_K, T::ORDER_PROMPT_INTENT[0], T::ORDER_PROMPT_LAYOUT[0], T::ORDER_PROMPT_FAB[0]);
    if on { format!("{}{i}{}{l}{}{f}{}{}{}{}", k[0], k[1], k[2], k[3], T::ORDER_PROMPT_ON[0], k[4], T::ORDER_PROMPT_REVIEW[0]) }
    else { format!("{}{i}{}{l}{}{}", k[5], k[6], k[7], T::ORDER_PROMPT_OFF[0]) }
}
/// Sec.prompt: the secrets block, names only
pub fn secrets(names: &[String]) -> String {
    let t = T::SEC_PROMPT; let set = if names.is_empty() { t[1].to_string() } else { format!("{}{}{}", t[2], names.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", "), t[4]) };
    format!("{}{set}{}{}", t[0], t[5], t[6])
}
/// Keys.name: an environment variable name (upper case, `_` for anything else, a leading digit gets a `_`), at most 64 characters.
pub fn key_name(raw: &str) -> String {
    let s: String = raw.trim().chars().take(64).map(|c| { let u = c.to_ascii_uppercase(); if u.is_ascii_uppercase() || u.is_ascii_digit() || u == '_' { u } else { '_' } }).collect();
    if s.chars().next().map_or(false, |c| c.is_ascii_digit()) { format!("_{s}") } else { s }
}
/// Keys.names: the names an api.env file sets, in its order (blank lines, comments and anything else are none).
pub fn key_names(text: &str) -> Vec<String> {
    text.lines().filter(|l| !l.trim().is_empty()).filter_map(|l| { let mut p = l.trim().split('='); let n = p.next()?; p.next()?; if !n.is_empty() && key_name(n) == n { Some(n.to_string()) } else { None } }).collect()
}
fn shell_quote(s: &str) -> String { format!("'{}'", s.split('\'').collect::<Vec<_>>().join("'\\''")) }
/// Keys.prompt: the api_keys block (names only, never the file); nothing when there are none.
pub fn api_keys(path: &str, names: &[String]) -> String {
    if names.is_empty() { return String::new() }
    let k = T::KEYS_PROMPT_K; let list = names.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ");
    format!("{}{path}{}{list}{}{}{}{}{}{}{}{}{}{}", k[1], k[2], k[3], k[4], shell_quote(path), k[5], names.first().map_or("NAME", |s| s.as_str()), k[7], k[8], k[9], shell_quote(path), k[10])
}

/// Prompt.lang
pub fn lang_line(setting: &str) -> &'static str { match setting { "ja" => T::PROMPT_LANG_K[0], "en" => T::PROMPT_LANG_K[1], "es" => PROMPT_LANG_ES, _ => "" } }
/// Prompt.lang for Spanish (prompt_text.rs is generated and has ja and en only).
const PROMPT_LANG_ES: &str = "Reply in Spanish (español) unless the person writes in another language or asks otherwise. Keep code, commands, paths and identifiers as they are.";

/// The backplane_skills block: one line per bundled skill, its SKILL.md named, so any provider can read it.
pub fn bundled_skills(ss: &[(String, String, String)]) -> String {
    let (b, l) = (T::PROMPT_BUNDLED, T::PROMPT_SKILL_LINE);
    format!("{}{}{}", b[0], ss.iter().map(|(n, d, p)| format!("{}{n}{}{d}{}{p}{}", l[0], l[1], l[2], l[3])).collect::<Vec<_>>().join("\n"), b[1])
}
/// The kistack_skills block: KiStack's copy, each skill and its SKILL.md, and that it is the one to use. Every provider gets it;
/// Claude (`--plugin-dir`, as `kistack:<name>`) and Pi (`--skill`) also load the same folder as skills of their own.
pub fn kistack_skills(c: &crate::agents::kistack::Copy) -> String {
    let l = T::PROMPT_SKILL_LINE;
    let which = if c.pinned { format!("pinned with this Backplane (KiStack commit {})", &c.rev[..12.min(c.rev.len())]) } else { format!("from BACKPLANE_KISTACK (KiStack {})", if c.rev.is_empty() { "of no recorded commit" } else { &c.rev }) };
    let list = c.skills.iter().map(|(n, d, p)| format!("{}{n}{}{d}{}{p}{}", l[0], l[1], l[2], l[3])).collect::<Vec<_>>().join("\n");
    format!("<kistack_skills>KiStack's KiCad skills, {which}, are in `{}`. Before KiCad work one of them covers (schematics, symbols, footprints, layout, BOM, exports, Gerbers, panels, renders), read its SKILL.md completely and follow it. Where your CLI lists them as skills of its own (Claude: `kistack:<name>`, e.g. `kistack:kicad-schematic`), they are these same files. Use only these copies: any other copy of these skills on this machine (in ~/.agents/skills, ~/.claude/skills, ~/.codex/skills, ~/.skills or a project folder) is older and not to be used.\n{list}</kistack_skills>", c.dir)
}
/// Where a hardware thread's work goes. `code`: its code worktree and branch when it works in the
/// main checkout ("" for a thread still in an older worktree); `locks`: (document, its lock).
pub fn design_words(main: &str, code: Option<(&str, &str)>, locks: &[(String, String)]) -> String {
    let ls = locks.iter().map(|(d, l)| format!("`{}`: `{l}`", d.rsplit('/').next().unwrap_or(d))).collect::<Vec<_>>().join(", ");
    let one = if ls.is_empty() { String::new() } else { format!(" Document locks ({ls}): a write through kicad_edit or kicad_ipc takes a free one by itself; call lock_acquire only when a tool says to (a direct file write, or to queue for a document another thread holds); while KiCad has the project open, edits through kicad_edit and kicad_ipc need none and land live beside the person's. Never ask the person to close KiCad or wait. Your document locks are released when your turn ends.") };
    match code {
        Some((wt, branch)) => format!("Design files (KiCad boards, schematics, projects, symbol and footprint libraries, 3D models, mech/, mfg/, fab files) live in the main checkout, `{main}`, your working folder: edit them there. Backplane commits them straight to main at the end of each turn and on each SaveDocument, and pushes when main has an upstream. Code (firmware, src/, scripts, docs, build files) goes in your worktree `{wt}` on branch `{branch}`, never in the main checkout: cd there for code, which lands on main the normal way. That worktree has no design files.{one}"),
        None => format!("These live once, in the project's main checkout `{main}`: never edit a worktree copy of them; code stays in your worktree.{one}"),
    }
}
/// An Altium Designer project (its .PrjPcb is the project's file): Altium's words replace KiCad's editing and runtime blocks.
pub fn is_altium(pro: &str) -> bool { pro.to_ascii_lowercase().ends_with(".prjpcb") }
/// Prompt.altium: how an agent works on an Altium project (altium/live.rs; the bundled `altium` skill holds the verified recipes)
pub fn altium(windows: bool, kistack: bool) -> String {
    let write = "New footprints (.PcbLib), symbols (.SchLib) and schematic sheets, and parts, wires, labels and power ports added to a sheet, are written with the backplane tool altium_write: a JSON spec, no Altium needed, the result read back.";
    let reach = if windows { format!("{write} Everything else goes through the person's running Altium Designer with the backplane tools altium_status (what Altium has open; call it first) and altium_script (a DelphiScript run inside Altium: Emit(S) prints a line; errors come back as results). If Altium is not running, either tool starts it and waits for it (about a minute): never ask the person to open Altium or click through its dialogs. Before either, read the bundled `altium` skill's SKILL.md completely: its recipes are tested against Altium and show how to open a project and documents by path, read and edit schematics and boards, run ERC and DRC, and save.") }
        else { format!("{write} Read the bundled `altium` skill's SKILL.md first. Altium Designer itself runs on Windows and this hub is not on Windows, so altium_script cannot reach it here: board edits, ERC and DRC need Altium on a Windows machine; say so when a change needs it.") };
    format!("<backplane_altium>This is an Altium Designer project: its .PrjPcb lists the schematic sheets (.SchDoc) and the board (.PcbDoc); libraries are .SchLib/.PcbLib/.IntLib. These are binary files: never write them with scripts of your own or any other tool than altium_write, and never ask the person to close Altium. {reach} Address every document by its full path (never the focused document: the person may be working in another), wrap edits in PreProcess/PostProcess so the person can undo them in Altium, and save each document right after each change that matters (the ECO, placement, routing), before any other step: unsaved work is lost if a box or a closed document interrupts (Backplane's Board and Schematic viewers draw the saved files, and Backplane commits them to main at each turn's end); then show it with viewer_show. Update the board from the schematic yourself with the skill's pcb_update_from_sch.pas recipe: never ask the person to run Update PCB or redo any step a script can do; ask them only for what no script can reach, such as online part downloads. Keep each script small: inspect, change, verify by reading back. The KiCad tools (kicad_edit, kicad_ipc, kicad-cli) do not apply to this project.{ks}</backplane_altium>", ks = if kistack { " KiStack's skills (below) are written for KiCad: adapt them for Altium at your discretion." } else { "" })
}
/// Hub.prompt.base then Prompt.lang.add
pub fn build(e: &PromptEnv) -> String {
    let orch = orchestration(e.may_delegate, &e.models);
    let alt = is_altium(&e.pro);
    let has_kistack = e.kistack.as_ref().is_some_and(|k| !k.skills.is_empty());
    let orch = match &e.kistack { Some(k) if !k.skills.is_empty() => orch, _ => orch.replace(" (KiStack's KiCad skills and the person's)", " (the person's)") }; // KiStack is claimed only when the agent has it
    let base = if alt { config_prompt(&e.fc, &e.render).replace(T::PROMPT_BOARD[0], "") } else { config_prompt(&e.fc, &e.render) };
    let cad = if alt { altium(cfg!(windows), has_kistack) } else { kicad(&e.kicad, e.kicad_ipc) };
    let mut s = [base, cad, project_locked(&e.root, &e.pro, &e.pcb, &e.sch, &e.all, &e.design), orch, background().to_string(), order_project(e.orders_on)].join("\n\n");
    if let Some(k) = e.kistack.as_ref().filter(|k| !k.skills.is_empty()) { s += "\n\n"; s += &kistack_skills(k) }
    if !e.skills.is_empty() { s += "\n\n"; s += &bundled_skills(&e.skills) }
    if !e.keys.is_empty() { s += "\n\n"; s += &e.keys }
    if !e.machines.is_empty() { s += "\n\n"; s += &e.machines }
    // a board on Backplane KiCad: plan before copper when the person turned it on, else route as asked (plan/mod.rs)
    if !e.pcb.is_empty() && !alt && crate::plan::plans(&e.kicad.status, &e.kicad.version, &crate::files::kicad::ipc::read_runtime(std::path::Path::new(&e.kicad.cli)).caps) { s += "\n\n"; s += &if e.routing_plan { crate::plan::guidance() } else { crate::plan::guidance_off().to_string() } }
    // RunAction's target editor (bp.13 batch 6, runaction-frame): what an agent's kicad_ipc may name, or where a shared action goes without it
    if e.kicad.status == "ok" && e.kicad_ipc && !alt { s += "\n\n"; s += crate::files::kicad::ipc::runaction_guidance(&crate::files::kicad::ipc::read_runtime(std::path::Path::new(&e.kicad.cli)).caps) }
    if e.buy { s += "\n\n"; s += &T::ORDER_PROMPT_BUY.concat() } // every piece: the opening line alone ([0]) left a buy thread without its steps
    let l = lang_line(&e.lang); if l.is_empty() { s } else { s + "\n" + l }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    #[test] fn an_altium_project_gets_altiums_words_not_kicads() {
        let mut e = PromptEnv::default(); e.root = "/p".into(); e.pro = "/p/Board.PrjPcb".into(); e.pcb = "/p/Board.PcbDoc".into(); e.sch = "/p/Top.SchDoc".into();
        let s = build(&e);
        assert!(s.contains("<backplane_altium>") && !s.contains("<backplane_kicad_board_editing>") && !s.contains("<kicad_runtime>"), "{s}");
        e.pro = "/p/b.kicad_pro".into(); let k = build(&e);
        assert!(!k.contains("<backplane_altium>") && k.contains("<backplane_kicad_board_editing>"));
        assert!(altium(true, false).contains("altium_script") && altium(false, false).contains("not on Windows") && altium(false, false).contains("altium_write"));
        assert!(s.contains("pcb_update_from_sch.pas") && s.contains("never ask the person to run Update PCB") && s.contains("right after each change"), "{s}");
        assert!(s.contains("(kicad_edit, kicad_ipc, kicad-cli) do not apply") && !s.contains("<kistack_skills>") && !s.contains("KiStack's skills (below)"), "no copy, no claim");
        e.pro = "/p/Board.PrjPcb".into(); e.kistack = Some(crate::agents::kistack::Copy { dir: "/h/kistack".into(), rev: crate::agents::kistack::REV.into(), pinned: true,
            skills: vec![("kicad-schematic".into(), "Draw schematics.".into(), "/h/kistack/skills/schematic/SKILL.md".into())] });
        let a = build(&e);
        assert!(a.contains("<kistack_skills>") && a.contains("KiStack's skills (below) are written for KiCad: adapt them for Altium at your discretion.") && a.contains("(KiStack's KiCad skills and the person's)"), "{a}");
        assert!(a.contains("<backplane_altium>") && !a.contains("<backplane_kicad_board_editing>") && !a.contains("<kicad_runtime>"), "KiCad's tool blocks stay out");
    }
    const GOLD: &str = include_str!("../../../../test/agents/fixtures/argv-default.json");
    pub fn gold() -> (String, serde_json::Value) { let a: Vec<String> = serde_json::from_str(GOLD).unwrap(); let i = a.iter().position(|x| x == "--append-system-prompt").unwrap(); (a[i + 1].clone(), serde_json::json!(a)) }
    pub fn gold_env() -> PromptEnv {
        PromptEnv { fc: "/home/h/.local/bin/freecadcmd".into(), render: "/home/h/Documents/i2cjak/backplane-gpui/build/gpui/backplane-serve-current".into(),
            kicad: Kicad { status: "stock".into(), cli: "/home/h/.local/bin/kicad-cli".into(), version: "10.0.7".into(), ..Default::default() }, kicad_ipc: true,
            models: "claude (fable, opus, sonnet, haiku); codex (gpt-6.1-sol, gpt-6-astra, gpt-6.1-terra, gpt-6-sol, gpt-6-luna, gpt-5.6-sol, gpt-5.6-terra, gpt-5.6-luna)".into(),
            root: "/home/h/Documents/i2cjak/backplane-gpui/build/gpui/results/agents/argv2/work/fixroot".into(), keys: secrets(&[]), ..Default::default() }
    }
    #[test] fn the_prompt_equals_the_one_the_js_hub_gave_a_stand_in_claude() {
        let (want, _) = gold(); let got = build(&gold_env());
        if got != want { let i = got.chars().zip(want.chars()).position(|(a, b)| a != b).unwrap_or(got.len().min(want.len())); panic!("differs at {i} (rust {} chars, js {}):\nrust …{}\njs   …{}", got.len(), want.len(), got.chars().skip(i.saturating_sub(40)).take(120).collect::<String>(), want.chars().skip(i.saturating_sub(40)).take(120).collect::<String>()) }
    }
    #[test] fn kicad_words_follow_the_status() {
        let k = |s: &str| Kicad { status: s.into(), cli: "/k".into(), version: "1".into(), commit: "abc".into(), why: "no\nmore".into() };
        assert!(kicad(&k("ok"), true).contains("blob/abc/BACKPLANE_IPC.md")); assert!(kicad(&k("ok"), false).contains("turned off")); assert!(kicad(&k("broken"), true).contains("doesn't run (no)"));
        { let mut e = PromptEnv { pcb: "/p/b.kicad_pcb".into(), ..Default::default() }; e.kicad.status = "ok".into(); e.kicad.version = "10.99.0-bp.11".into(); assert!(build(&e).contains("<backplane_routing_plan>"));
            assert!(build(&e).contains("Plan before routing is off") && !build(&e).contains("plan_approve"), "off unless the person turns it on: route as asked");
            e.routing_plan = true; assert!(build(&e).contains("plan_approve") && !build(&e).contains("is off")); e.routing_plan = false; e.kicad.version = "10.99.0-bp.10".into(); assert!(!build(&e).contains("<backplane_routing_plan>"), "the plan API came with bp.11"); e.kicad.status = "stock".into(); assert!(!build(&e).contains("<backplane_routing_plan>"), "stock KiCad has no plan API"); }
        assert!(kicad(&k("missing"), true).contains("isn't installed") && kicad(&k("missing"), true).starts_with("<kicad_runtime>The selected KiCad CLI path is `/k`.")); assert!(kicad(&k("old"), true).contains(min_version()));
    }
    #[test] fn project_lists_the_files_it_has() {
        let p = project("/r", "a.kicad_pro", "a.kicad_pcb", "", "a.kicad_pro\nb/b.kicad_pro");
        assert!(p.contains("canonical KiCad files:\n- project: `a.kicad_pro`\n- board: `a.kicad_pcb`\nWork on these")); assert!(p.contains("several KiCad projects") && p.contains("\n- `b/b.kicad_pro`"));
        assert!(project("/r", "", "", "", "").contains("No KiCad project was found"));
        let locks = vec![("/r/a.kicad_pcb".to_string(), "kicad-a-pcb-01234567".to_string())];
        let p = project_locked("/r", "a.kicad_pro", "a.kicad_pcb", "", "", &design_words("/r", Some(("/r/.backplane/worktrees/x", "backplane/x")), &locks));
        assert!(p.contains("your working folder") && p.contains("`/r/.backplane/worktrees/x` on branch `backplane/x`") && p.contains("`a.kicad_pcb`: `kicad-a-pcb-01234567`") && p.contains("released when your turn ends"), "{p}");
        assert!(design_words("/r", None, &locks).contains("never edit a worktree copy"));
    }
    #[test] fn a_delegated_task_at_the_depth_limit_may_not_delegate() { assert!(orchestration(false, "m").contains("depth limit") && !orchestration(false, "m").contains("Models available")); }
    #[test] fn the_bundled_skills_are_named_with_their_files() {
        let mut e = gold_env(); e.skills = vec![("sendcutsend".into(), "Design parts for SendCutSend.".into(), "/h/bundled/skills/sendcutsend/SKILL.md".into())];
        let s = build(&e);
        assert!(s.contains("<backplane_skills>Backplane's own skills, for every agent whatever its provider: before work one of them covers, read its SKILL.md completely and follow it.\n- sendcutsend: Design parts for SendCutSend. Read \"/h/bundled/skills/sendcutsend/SKILL.md\"</backplane_skills>"), "{s}");
        assert!(!build(&gold_env()).contains("<backplane_skills>"));
        assert!(order_project(true).contains("`sendcutsend` block") && !order_project(false).contains("sendcutsend"), "the review knows SendCutSend's check");
    }
    #[test] fn kistack_is_named_with_its_files_only_when_the_agent_has_it() {
        let e = gold_env(); let s = build(&e);
        assert!(!s.contains("<kistack_skills>") && !s.contains("KiStack's KiCad skills and") && s.contains("the same skills as you (the person's)"), "no copy, no claim");
        let mut e = gold_env(); e.kistack = Some(crate::agents::kistack::Copy { dir: "/h/kistack".into(), rev: crate::agents::kistack::REV.into(), pinned: true,
            skills: vec![("kicad-schematic".into(), "Draw schematics.".into(), "/h/kistack/skills/schematic/SKILL.md".into())] });
        let s = build(&e);
        assert!(s.contains(&format!("<kistack_skills>KiStack's KiCad skills, pinned with this Backplane (KiStack commit {}), are in `/h/kistack`.", &crate::agents::kistack::REV[..12])), "{s}");
        assert!(s.contains("- kicad-schematic: Draw schematics. Read \"/h/kistack/skills/schematic/SKILL.md\"</kistack_skills>") && s.contains("is older and not to be used"));
        assert!(s.contains("the same skills as you (KiStack's KiCad skills and the person's)"));
        e.kistack.as_mut().unwrap().skills.clear(); assert!(!build(&e).contains("<kistack_skills>"), "an empty copy is no copy");
    }
    #[test] fn language_and_buy_blocks_are_added_after() { let mut e = gold_env(); e.lang = "en".into(); e.buy = true; let s = build(&e); assert!(s.ends_with("Keep code, commands, paths and identifiers as they are.")); assert!(s.contains("<backplane_buy>")); e.lang = "es".into(); assert!(build(&e).contains("Reply in Spanish")); e.lang = "fr".into(); assert!(!build(&e).contains("Reply in")); }
    /// e2e n1110: Buy it started a thread that exported Gerbers, guessed at a manifest and gave up ("I don't know the format it expects"): its prompt was the
    /// block's opening line only. The whole block is there: the gate, the questions, the export layout, the quote and the closing rules.
    #[test] fn a_buy_thread_gets_every_step_of_its_instructions() {
        let mut e = gold_env(); e.buy = true; let s = build(&e);
        let block = &s[s.find("<backplane_buy>").unwrap()..];
        assert!(block.ends_with("</backplane_buy>"), "the block is closed: {}", &block[block.len().saturating_sub(120)..]);
        for step in ["1. Read `.backplane/intent.md`", "2. Call the backplane tool `order_review`", "3. Ask only what is missing", "`order_ask`", "4. Export from the saved files", "manifest.json", "5. Call the backplane tool `order_quote`", "Only order_quote ever sends anything"] {
            assert!(block.contains(step), "the buy instructions have {step:?}");
        }
        assert_eq!(block, T::ORDER_PROMPT_BUY.concat(), "the pieces in order, nothing between them (each ends its own line)");
        e.buy = false; assert!(!build(&e).contains("<backplane_buy>"), "only a buy thread has it");
    }
    #[test] fn api_key_names_are_read_from_the_file_and_the_block_never_holds_a_value() {
        let names = key_names("# c\nOPENAI_API_KEY='sk-secret'\n\nbad name=1\n1X=2\nLONE\nGH_TOKEN='a=b'\n"); assert_eq!(names, ["OPENAI_API_KEY", "GH_TOKEN"]);
        let b = api_keys("/h/secrets/api.env", &names); assert!(b.starts_with("<api_keys>The user keeps API keys for you in `/h/secrets/api.env` (`NAME='value'` lines): `OPENAI_API_KEY`, `GH_TOKEN`. ")); assert!(b.contains("set -a; . '/h/secrets/api.env'; set +a") && b.contains("Bearer $OPENAI_API_KEY") && b.ends_with("</api_keys>")); assert!(!b.contains("sk-secret"));
        assert_eq!(api_keys("/p", &[]), ""); assert!(api_keys("/it's", &["A".into()]).contains("'/it'\\''s'"));
        assert_eq!(key_name("my-key"), "MY_KEY"); assert_eq!(key_name("9a"), "_9A");
    }
    #[test] fn secrets_name_what_is_set() { assert!(secrets(&["a".into(), "b".into()]).contains("Set now: `a`, `b`. ")); assert!(secrets(&[]).contains("None is set yet. ")); }
    #[test] fn the_freecad_gui_sits_beside_the_console_program() { assert_eq!(mech_gui("/x/freecadcmd"), "/x/freecad"); assert_eq!(mech_gui("/a/Resources/bin/freecadcmd"), "/a/MacOS/FreeCAD"); assert_eq!(mech_gui("C:\\F\\FreeCADCmd.exe"), "C:\\F\\FreeCAD.exe"); }
}
