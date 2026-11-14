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
    pub root: String, pub pro: String, pub pcb: String, pub sch: String, pub all: String,
    /// Where this thread's design files and code go, and the document locks (`design_words`).
    pub design: String,
    pub keys: String, pub machines: String, pub buy: bool, pub lang: String,
    /// Backplane's own skills (bundled.rs): (name, description, SKILL.md path)
    pub skills: Vec<(String, String, String)>,
}
impl Default for PromptEnv {
    fn default() -> Self { PromptEnv { fc: String::new(), render: String::new(), kicad: Kicad { status: "missing".into(), ..Default::default() }, kicad_ipc: true, models: String::new(), may_delegate: true, orders_on: true,
        root: String::new(), pro: String::new(), pcb: String::new(), sch: String::new(), all: String::new(), design: String::new(), keys: String::new(), machines: String::new(), buy: false, lang: String::new(), skills: vec![] } }
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
pub fn lang_line(setting: &str) -> &'static str { match setting { "ja" => T::PROMPT_LANG_K[0], "en" => T::PROMPT_LANG_K[1], _ => "" } }

/// The backplane_skills block: one line per bundled skill, its SKILL.md named, so any provider can read it.
pub fn bundled_skills(ss: &[(String, String, String)]) -> String {
    let (b, l) = (T::PROMPT_BUNDLED, T::PROMPT_SKILL_LINE);
    format!("{}{}{}", b[0], ss.iter().map(|(n, d, p)| format!("{}{n}{}{d}{}{p}{}", l[0], l[1], l[2], l[3])).collect::<Vec<_>>().join("\n"), b[1])
}
/// Where a hardware thread's work goes. `code`: its code worktree and branch when it works in the
/// main checkout ("" for a thread still in an older worktree); `locks`: (document, its lock).
pub fn design_words(main: &str, code: Option<(&str, &str)>, locks: &[(String, String)]) -> String {
    let ls = locks.iter().map(|(d, l)| format!("`{}`: `{l}`", d.rsplit('/').next().unwrap_or(d))).collect::<Vec<_>>().join(", ");
    let one = if ls.is_empty() { String::new() } else { format!(" Document locks ({ls}): take one with lock_acquire only when kicad_ipc or a file write asks for it (whole-document commands); while KiCad has the project open, edits through kicad_ipc need none and land live beside the person's. Never ask the person to close KiCad or wait. Your document locks are released when your turn ends.") };
    match code {
        Some((wt, branch)) => format!("Design files (KiCad boards, schematics, projects, symbol and footprint libraries, 3D models, mech/, mfg/, fab files) live in the main checkout, `{main}`, your working folder: edit them there. Backplane commits them straight to main at the end of each turn and on each SaveDocument, and pushes when main has an upstream. Code (firmware, src/, scripts, docs, build files) goes in your worktree `{wt}` on branch `{branch}`, never in the main checkout: cd there for code, which lands on main the normal way. That worktree has no design files.{one}"),
        None => format!("These live once, in the project's main checkout `{main}`: never edit a worktree copy of them; code stays in your worktree.{one}"),
    }
}
/// Hub.prompt.base then Prompt.lang.add
pub fn build(e: &PromptEnv) -> String {
    let mut s = [config_prompt(&e.fc, &e.render), kicad(&e.kicad, e.kicad_ipc), project_locked(&e.root, &e.pro, &e.pcb, &e.sch, &e.all, &e.design), orchestration(e.may_delegate, &e.models), background().to_string(), order_project(e.orders_on)].join("\n\n");
    if !e.skills.is_empty() { s += "\n\n"; s += &bundled_skills(&e.skills) }
    if !e.keys.is_empty() { s += "\n\n"; s += &e.keys }
    if !e.machines.is_empty() { s += "\n\n"; s += &e.machines }
    // a board on Backplane KiCad: plan before copper (plan/mod.rs)
    if !e.pcb.is_empty() && crate::plan::plans(&e.kicad.status, &e.kicad.version, &crate::files::kicad::ipc::read_runtime(std::path::Path::new(&e.kicad.cli)).caps) { s += "\n\n"; s += &crate::plan::guidance() }
    // RunAction's target editor (bp.13 batch 6, runaction-frame): what an agent's kicad_ipc may name, or where a shared action goes without it
    if e.kicad.status == "ok" && e.kicad_ipc { s += "\n\n"; s += crate::files::kicad::ipc::runaction_guidance(&crate::files::kicad::ipc::read_runtime(std::path::Path::new(&e.kicad.cli)).caps) }
    if e.buy { s += "\n\n"; s += &T::ORDER_PROMPT_BUY.concat() } // every piece: the opening line alone ([0]) left a buy thread without its steps
    let l = lang_line(&e.lang); if l.is_empty() { s } else { s + "\n" + l }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
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
        { let mut e = PromptEnv { pcb: "/p/b.kicad_pcb".into(), ..Default::default() }; e.kicad.status = "ok".into(); e.kicad.version = "10.99.0-bp.11".into(); assert!(build(&e).contains("<backplane_routing_plan>")); e.kicad.version = "10.99.0-bp.10".into(); assert!(!build(&e).contains("<backplane_routing_plan>"), "the plan API came with bp.11"); e.kicad.status = "stock".into(); assert!(!build(&e).contains("<backplane_routing_plan>"), "stock KiCad has no plan API"); }
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
    #[test] fn language_and_buy_blocks_are_added_after() { let mut e = gold_env(); e.lang = "en".into(); e.buy = true; let s = build(&e); assert!(s.ends_with("Keep code, commands, paths and identifiers as they are.")); assert!(s.contains("<backplane_buy>")); }
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
