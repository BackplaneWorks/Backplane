//! What an agent is told: src/core/prompt.bend's text assembly (Hub.prompt in src/server/server.bend), with the
//! literals generated from the Bend source (prompt_text.rs, by test/gpui/agents/gen_prompt.py) so the words cannot drift.
//! The golden test compares the whole string with the argv the JS hub started a stand-in Claude with
//! (test/gpui/agents/fixtures/argv-default.json); the Bend names are kept on each fn.
use super::prompt_text as T;

/// What the host learned of KiCad (kihealth.bend): status ok|stock|old|broken|checking|missing, the CLI path, version, commit, why it broke.
#[derive(Clone, Debug, Default)]
pub struct Kicad { pub status: String, pub cli: String, pub version: String, pub commit: String, pub why: String }

/// Everything Hub.prompt reads, given by the hub that discovered it (FreeCAD, the KiCad runtime, the models line, secrets, settings).
#[derive(Clone, Debug)]
pub struct PromptEnv {
    pub fc: String, pub render: String, pub kicad: Kicad, pub kicad_ipc: bool, pub models: String, pub may_delegate: bool, pub orders_on: bool,
    pub root: String, pub pro: String, pub pcb: String, pub sch: String, pub all: String,
    pub keys: String, pub machines: String, pub buy: bool, pub lang: String,
}
impl Default for PromptEnv {
    fn default() -> Self { PromptEnv { fc: String::new(), render: String::new(), kicad: Kicad { status: "missing".into(), ..Default::default() }, kicad_ipc: true, models: String::new(), may_delegate: true, orders_on: true,
        root: String::new(), pro: String::new(), pcb: String::new(), sch: String::new(), all: String::new(), keys: String::new(), machines: String::new(), buy: false, lang: String::new() } }
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
pub fn project(root: &str, pro: &str, pcb: &str, sch: &str, all: &str) -> String {
    let (p, k, ps) = (T::PROMPT_PROJECT, T::PROMPT_PROJECT_K, T::PROMPT_PROJECTS);
    let body = if pcb.is_empty() && sch.is_empty() { k[0].to_string() } else { format!("{}{}{}{}{}", k[1], path(&k[2], pro), path(&k[3], pcb), path(&k[4], sch), k[5]) };
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

/// Hub.prompt.base then Prompt.lang.add
pub fn build(e: &PromptEnv) -> String {
    let mut s = [config_prompt(&e.fc, &e.render), kicad(&e.kicad, e.kicad_ipc), project(&e.root, &e.pro, &e.pcb, &e.sch, &e.all), orchestration(e.may_delegate, &e.models), background().to_string(), order_project(e.orders_on)].join("\n\n");
    if !e.keys.is_empty() { s += "\n\n"; s += &e.keys }
    if !e.machines.is_empty() { s += "\n\n"; s += &e.machines }
    if e.buy { s += "\n\n"; s += T::ORDER_PROMPT_BUY[0] }
    let l = lang_line(&e.lang); if l.is_empty() { s } else { s + "\n" + l }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    const GOLD: &str = include_str!("../../../../test/gpui/agents/fixtures/argv-default.json");
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
        assert!(kicad(&k("missing"), true).contains("isn't installed") && kicad(&k("missing"), true).starts_with("<kicad_runtime>The selected KiCad CLI path is `/k`.")); assert!(kicad(&k("old"), true).contains(min_version()));
    }
    #[test] fn project_lists_the_files_it_has() {
        let p = project("/r", "a.kicad_pro", "a.kicad_pcb", "", "a.kicad_pro\nb/b.kicad_pro");
        assert!(p.contains("canonical KiCad files:\n- project: `a.kicad_pro`\n- board: `a.kicad_pcb`\nWork on these")); assert!(p.contains("several KiCad projects") && p.contains("\n- `b/b.kicad_pro`"));
        assert!(project("/r", "", "", "", "").contains("No KiCad project was found"));
    }
    #[test] fn a_delegated_task_at_the_depth_limit_may_not_delegate() { assert!(orchestration(false, "m").contains("depth limit") && !orchestration(false, "m").contains("Models available")); }
    #[test] fn language_and_buy_blocks_are_added_after() { let mut e = gold_env(); e.lang = "en".into(); e.buy = true; let s = build(&e); assert!(s.ends_with("Keep code, commands, paths and identifiers as they are.")); assert!(s.contains("<backplane_buy>")); }
    #[test] fn api_key_names_are_read_from_the_file_and_the_block_never_holds_a_value() {
        let names = key_names("# c\nOPENAI_API_KEY='sk-secret'\n\nbad name=1\n1X=2\nLONE\nGH_TOKEN='a=b'\n"); assert_eq!(names, ["OPENAI_API_KEY", "GH_TOKEN"]);
        let b = api_keys("/h/secrets/api.env", &names); assert!(b.starts_with("<api_keys>The user keeps API keys for you in `/h/secrets/api.env` (`NAME='value'` lines): `OPENAI_API_KEY`, `GH_TOKEN`. ")); assert!(b.contains("set -a; . '/h/secrets/api.env'; set +a") && b.contains("Bearer $OPENAI_API_KEY") && b.ends_with("</api_keys>")); assert!(!b.contains("sk-secret"));
        assert_eq!(api_keys("/p", &[]), ""); assert!(api_keys("/it's", &["A".into()]).contains("'/it'\\''s'"));
        assert_eq!(key_name("my-key"), "MY_KEY"); assert_eq!(key_name("9a"), "_9A");
    }
    #[test] fn secrets_name_what_is_set() { assert!(secrets(&["a".into(), "b".into()]).contains("Set now: `a`, `b`. ")); assert!(secrets(&[]).contains("None is set yet. ")); }
    #[test] fn the_freecad_gui_sits_beside_the_console_program() { assert_eq!(mech_gui("/x/freecadcmd"), "/x/freecad"); assert_eq!(mech_gui("/a/Resources/bin/freecadcmd"), "/a/MacOS/FreeCAD"); assert_eq!(mech_gui("C:\\F\\FreeCADCmd.exe"), "C:\\F\\FreeCAD.exe"); }
}
