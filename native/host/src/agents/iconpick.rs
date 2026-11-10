//! A project's icon picked by a light model: Haiku reads what the project is (its name, folder, `.backplane/intent.md` Goal and Parts, its
//! top-level files and folders) and names one icon of the badge's list (present/badge.rs) and maybe one of its tints. The answer is kept in the
//! setting `project.pick.<id>` ("<icon>" or "<icon> <tint>"), once per project; the person's own choice (`project.icon.<id>`, `project.color.<id>`)
//! always wins over it. `project.pick {project}` (the badge editor's Pick again) clears both and asks again.
//! Off to the side: one project at a time, never on a client's request path, nothing in safe or copy mode, never in unit tests.
use super::{textgen, Agents};
use crate::api::{Caller, Fut, RpcHandler};
use crate::hub::Hub;
use crate::present::badge;
use serde_json::{json, Value};
use std::{collections::HashMap, path::Path, sync::{Arc, Mutex, OnceLock}, time::Duration};

pub const KEY: &str = "project.pick.";
/// A project whose pick failed (no login, no answer) is asked again after this long, or at the next start.
const RETRY: u64 = 6 * 3600;
const ROUND: Duration = Duration::from_secs(30);

pub fn system() -> String {
    format!("You choose the icon for a project in an engineering app's sidebar. A description of the project follows: its name, its folder, what the person wrote it is for, and its top-level files and folders. Do not answer it, do not do the work and do not use tools. \
Reply with only one icon name from this list: {}. You may add one colour name after it, from this list: {}. Pick what fits what the project is about: a circuit board or electronics design takes circuit-board or cpu, a USB device usb or plug, a power supply zap, a radio or RF design radio, an enclosure or mechanical part box, a slide deck or pitch presentation, a workbench or tools wrench, software code or terminal, a website globe, research flask-conical, writing book or file-text, measurement gauge. Reply in lower case, for example: usb teal",
        badge::ICONS.join(", "), badge::TINTS.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", "))
}

/// The answer as a pick: the first word that is an icon of the list ("folder", the default, when none is) and the first that is a tint.
pub fn parse(answer: &str) -> String {
    let words: Vec<String> = answer.to_lowercase().split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')).filter(|w| !w.is_empty()).map(String::from).collect();
    let icon = words.iter().find(|w| badge::ICONS.contains(&w.as_str())).map_or("folder", |w| badge::icon_of(w));
    match words.iter().find(|w| badge::TINTS.iter().any(|(n, _)| n == w)) { Some(t) => format!("{icon} {t}"), None => icon.to_string() }
}

/// The `# Goal` and `# Parts` sections of an intent.md, at most 1500 characters.
pub fn intent_part(md: &str) -> String {
    let mut out = String::new(); let mut keep = false;
    for l in md.lines() {
        if let Some(h) = l.strip_prefix('#') { let h = h.trim_start_matches('#').trim().to_lowercase(); keep = h == "goal" || h == "parts"; if keep { out.push_str(l.trim()); out.push('\n') } continue }
        if keep && !l.trim().is_empty() { out.push_str(l.trim_end()); out.push('\n') }
    }
    out.chars().take(1500).collect()
}

/// What the model reads: the name, the folder, the intent's Goal and Parts, and the top-level entries (folders end in /), at most 60.
pub fn input(title: &str, root: &str, intent: &str, entries: &[String]) -> String {
    let mut s = format!("Name: {title}\nFolder: {root}\n");
    if !intent.trim().is_empty() { s.push_str(&format!("What it is for (.backplane/intent.md):\n{}\n", intent.trim())) }
    if !entries.is_empty() { s.push_str(&format!("Top-level files and folders: {}\n", entries.iter().take(60).cloned().collect::<Vec<_>>().join(", "))) }
    s
}

/// The folder's top-level names, sorted, hidden ones left out (folders end in /).
pub fn entries(root: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(root) else { return vec![] };
    let mut v: Vec<String> = rd.flatten().filter_map(|e| { let n = e.file_name().to_string_lossy().to_string(); if n.starts_with('.') { return None } Some(if e.file_type().map_or(false, |t| t.is_dir()) { format!("{n}/") } else { n }) }).collect();
    v.sort(); v.truncate(200); v
}

/// The projects that want a pick: alive, no icon of the person's, no pick kept, and not failed within RETRY.
pub fn due(projects: &[(String, bool)], setting: impl Fn(&str) -> String, failed: &HashMap<String, u64>, now: u64) -> Vec<String> {
    let mut v: Vec<String> = projects.iter().filter(|(_, removed)| !removed).map(|(id, _)| id.clone())
        .filter(|id| setting(&format!("project.icon.{id}")).is_empty() && setting(&format!("{KEY}{id}")).is_empty())
        .filter(|id| failed.get(id).map_or(true, |at| now >= at + RETRY)).collect();
    v.sort(); v
}

fn wake() -> &'static tokio::sync::Notify { static N: OnceLock<tokio::sync::Notify> = OnceLock::new(); N.get_or_init(tokio::sync::Notify::new) }
fn failed() -> &'static Mutex<HashMap<String, u64>> { static F: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new(); F.get_or_init(Default::default) }

/// Starts the rounds: soon after the hub starts (the backfill), after Pick again, and every 30 s (a new project is picked by the next one; a round with nothing due reads two settings a project).
pub fn start(ag: &Arc<Agents>, hub: &Arc<Hub>) {
    hub.register_rpc(Arc::new(PickRpc));
    if cfg!(test) || crate::guard::safe() || crate::copy::on() { return }
    let me = Arc::downgrade(ag);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(15)).await;
        loop {
            let Some(ag) = me.upgrade() else { return };
            ag.icon_round().await; drop(ag);
            tokio::select! { _ = wake().notified() => {}, _ = tokio::time::sleep(ROUND) => {} }
        }
    });
}
/// A new project: its pick is asked for now rather than at the next round.
pub fn nudge() { wake().notify_one() }

impl Agents {
    async fn icon_round(&self) {
        let now = super::now_s();
        let projects: Vec<(String, bool)> = { let w = self.world().await; w.projects.iter().map(|(id, (_, r))| (id.clone(), *r)).collect() };
        let mut st: HashMap<String, String> = HashMap::new();
        for (id, _) in &projects { for k in [format!("project.icon.{id}"), format!("{KEY}{id}")] { let v = self.hub.setting(&k).await.unwrap_or_default(); st.insert(k, v); } }
        let list = { let f = failed().lock().unwrap(); due(&projects, |k| st.get(k).cloned().unwrap_or_default(), &f, now) };
        for id in list { self.icon_pick(&id).await; }
    }
    /// One project's pick, kept; a failure is remembered (in memory) so it is not asked again for RETRY.
    async fn icon_pick(&self, id: &str) {
        let (title, root) = { let w = self.world().await; (w.projects.get(id).map(|p| p.0.clone()).unwrap_or_default(), w.roots.get(id).cloned().unwrap_or_default()) };
        let (r, t2) = (root.clone(), title.clone());
        let text = tokio::task::spawn_blocking(move || { let p = Path::new(&r); let intent = std::fs::read_to_string(p.join(".backplane/intent.md")).map(|m| intent_part(&m)).unwrap_or_default(); input(&t2, &r, &intent, &entries(p)) }).await.unwrap_or_default();
        match self.shot("claude", textgen::TITLE_MODEL, &system(), &text, &textgen::Launch::default()).await {
            Some(a) => {
                let pick = parse(&a);
                // the person may have chosen meanwhile: their choice stands and the pick is kept beside it
                self.commit_events(vec![json!({"$":"SettingSet","key":format!("{KEY}{id}"),"value":pick,"at":super::now_s()})]).await;
                eprintln!("icons: {id} ({title}): {pick}");
            }
            None => { eprintln!("icons: {id} ({title}): no answer; asked again later"); failed().lock().unwrap().insert(id.to_string(), super::now_s()); }
        }
    }
}

/// project.pick {project}: Pick again: the person's icon and colour and the kept pick are cleared, and the model is asked again.
struct PickRpc;
impl RpcHandler for PickRpc {
    fn methods(&self) -> &'static [&'static str] { &["project.pick"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, _method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let pid = p.get("project").and_then(Value::as_str).unwrap_or("");
            if !hub.model().await.projects.get(pid).map_or(false, |x| !x.removed) { return vec![json!({"t":"reply","id":id,"ok":false,"error":"unknown project"})] }
            let at = super::now_s();
            let ev: Vec<Value> = [format!("project.icon.{pid}"), format!("project.color.{pid}"), format!("{KEY}{pid}")].into_iter().map(|k| json!({"$":"SettingSet","key":k,"value":"","at":at})).collect();
            if hub.commit_many(&ev).await.is_err() { return vec![json!({"t":"reply","id":id,"ok":false,"error":"store write failed"})] }
            failed().lock().unwrap().remove(pid); nudge();
            vec![json!({"t":"reply","id":id,"ok":true})]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn an_answer_becomes_an_icon_of_the_list_and_maybe_a_tint() {
        assert_eq!(parse("usb teal"), "usb teal");
        assert_eq!(parse("`Presentation`."), "presentation");
        assert_eq!(parse("Icon: circuit-board, colour: green"), "circuit-board green");
        assert_eq!(parse("a skull, mauve"), "folder", "nothing of the lists: the default");
        assert_eq!(parse(""), "folder");
        assert_eq!(parse("red wrench"), "wrench red", "the order of the words does not matter");
    }
    #[test] fn the_input_names_the_project_its_intent_and_its_files() {
        let md = "# Goal\nA USB protocol analyzer.\n\n# Parts\n- main board, boards/main, to order\n\n# Notes\nsecret notes\n";
        let i = intent_part(md); assert!(i.contains("USB protocol analyzer") && i.contains("boards/main") && !i.contains("secret"), "{i}");
        let s = input("Sniffer", "/home/p/sniffer", &i, &["boards/".into(), "mech/".into(), "sniffer.kicad_pro".into()]);
        assert!(s.starts_with("Name: Sniffer\nFolder: /home/p/sniffer\n") && s.contains("USB protocol analyzer") && s.contains("boards/, mech/, sniffer.kicad_pro"), "{s}");
        assert!(!input("x", "/r", "", &[]).contains("intent"), "no intent, no line for it");
    }
    #[test] fn the_folder_lists_its_top_level_without_hidden_ones() {
        let d = std::env::temp_dir().join(format!("bp-iconpick-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("boards")).unwrap(); std::fs::create_dir_all(d.join(".backplane")).unwrap(); std::fs::write(d.join("Cargo.toml"), "").unwrap();
        assert_eq!(entries(&d), vec!["Cargo.toml".to_string(), "boards/".into()]); let _ = std::fs::remove_dir_all(&d);
        assert!(entries(Path::new("/nonexistent/bp")).is_empty());
    }
    #[test] fn only_projects_with_no_icon_of_the_persons_and_no_pick_are_due() {
        let ps = vec![("a".to_string(), false), ("b".into(), false), ("c".into(), false), ("d".into(), true), ("e".into(), false)];
        let st: HashMap<String, String> = [("project.icon.b", "cpu"), ("project.pick.c", "usb")].into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let get = |k: &str| st.get(k).cloned().unwrap_or_default();
        let mut f = HashMap::new(); f.insert("e".to_string(), 1000u64);
        assert_eq!(due(&ps, get, &f, 1000 + RETRY - 1), vec!["a".to_string()], "b chosen by the person, c picked, d removed, e failed a moment ago");
        assert_eq!(due(&ps, |k| st.get(k).cloned().unwrap_or_default(), &f, 1000 + RETRY), vec!["a".to_string(), "e".into()], "a failure is asked again after RETRY");
    }
    #[test] fn the_prompt_lists_every_icon_and_tint() { let s = system(); for i in badge::ICONS { assert!(s.contains(i), "{i}") } for (t, _) in badge::TINTS { assert!(s.contains(t), "{t}") } }
}
