//! Improvements: `improvements.context {sourceThread}` and `improvements.report {thread}`.
//! Settings > Improvements lets the person start a thread that writes a problem report about the app; these two requests give the form what it
//! needs and read the finished report back as a draft. Nothing is published here: the client shows the draft and the person decides.
//! The thread itself is made by `thread.create` with a `creationKey` (the commands lane; `Improvements.key`/`thread` below are its rules).
//! Where it goes is the person's: `improvements.projects {q}` searches the projects of this machine and of the linked ones (a linked machine's project id is
//! `<link>~<id>`, which `thread.create` makes its thread on that machine) and says the last one chosen; `improvements.project {project}` remembers a choice.
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::sync::Arc;

/// Improvements.key: a creation key is 32 lowercase hex characters.
pub fn key(s: &str) -> bool { s.len() == 32 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) }
/// Improvements.thread
pub fn thread_for(key: &str) -> String { format!("improvement-{key}") }
/// Improvements.report.limit: a body of more than 120000 bytes is dropped (""), never cut.
pub fn limit(text: &str) -> String { if text.len() <= 120_000 { text.to_string() } else { String::new() } }

pub struct ImprovementsRpc;
impl RpcHandler for ImprovementsRpc {
    fn methods(&self) -> &'static [&'static str] { &["improvements.context", "improvements.report", "improvements.preview", "improvements.template", "improvements.prompt", "improvements.diagnostics", "improvements.projects", "improvements.project"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let data = |v: Value| vec![json!({"t":"reply","id":id,"ok":true,"result":v})];
            let t = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            match method {
                // the pure report policy (issue_report.rs), for clients without bridge: a `body` previews it as it is, else the parts build one
                "improvements.preview" => return data(if p.get("body").is_some() { super::issue_report::preview_body(&t("body")) } else { super::issue_report::preview(&t("text"), &t("os"), &t("version"), &t("details"), &t("diagnostics")) }),
                "improvements.template" => return data(json!({"template": super::issue_report::TEMPLATE})),
                "improvements.prompt" => return data(json!({"prompt": super::issue_report::prompt(&t("description"), &t("os"), &t("version"), &t("evidence"), &t("diagnostics"))})),
                // the elected diagnostics, finished: the local recorder's export allowlisted again (issue_report::safe_hub), no connection ring here
                "improvements.diagnostics" => { let doc = tokio::task::spawn_blocking(super::diagnostics::export).await.ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).unwrap_or(Value::Null); return data(json!({"report": super::issue_report::diagnostic_report(&doc)})) }
                _ => {}
            }
            if method == "improvements.projects" { return data(projects(hub, &t("q")).await) }
            if method == "improvements.project" {
                let project = t("project");
                if !all_projects(hub).await.iter().any(|r| r.id == project) { return vec![json!({"t":"reply","id":id,"ok":false,"error":"That project is not on this machine or a linked one."})] }
                let _ = hub.commit(&json!({"$":"SettingSet","key":LAST_KEY,"value":project,"at":crate::hub::now_ms() / 1000})).await;
                return data(json!({"project":project}))
            }
            if method == "improvements.context" { return data(context(hub, p.get("sourceThread").and_then(Value::as_str).unwrap_or("")).await) }
            data(report(hub, p.get("thread").and_then(Value::as_str).unwrap_or("")).await)
        })
    }
}

/// The project the person last sent an improvement to.
pub const LAST_KEY: &str = "improvements.project";
/// A project an improvement may go to: this machine's (`machine` "") or a linked machine's (its name; the id is `<link>~<id>`).
#[derive(Clone, Debug, PartialEq)]
pub struct Row { pub id: String, pub title: String, pub root: String, pub machine: String }
/// How many projects the picker shows at most.
pub const PICK_MAX: usize = 40;
/// Improvements.projects: the rows that have every word of `q` (any case) in their title, folder or machine name; the last choice first, then this machine's, then each
/// linked machine's, by title. `last` comes back when it is still one of them, else "".
pub fn pick_projects(rows: &[Row], q: &str, last: &str) -> (Vec<Row>, String) {
    let words: Vec<String> = q.split_whitespace().map(str::to_lowercase).collect();
    let mut xs: Vec<Row> = rows.iter().filter(|r| { let hay = format!("{} {} {}", r.title, r.root, r.machine).to_lowercase(); words.iter().all(|w| hay.contains(w.as_str())) }).cloned().collect();
    xs.sort_by(|a, b| (a.id != last, !a.machine.is_empty(), a.machine.to_lowercase(), a.title.to_lowercase(), a.id.clone()).cmp(&(b.id != last, !b.machine.is_empty(), b.machine.to_lowercase(), b.title.to_lowercase(), b.id.clone())));
    xs.truncate(PICK_MAX);
    (xs, if rows.iter().any(|r| r.id == last) { last.to_string() } else { String::new() })
}
/// Every project a request may go to: this machine's that are not removed and whose folder is there, and the shown linked machines'.
pub async fn all_projects(hub: &Arc<Hub>) -> Vec<Row> {
    let mut rows: Vec<Row> = { let m = hub.model().await; m.projects.iter().filter(|(id, p)| !p.removed && !p.root.is_empty() && id.as_str() != crate::sidebar::SCRATCH && std::path::Path::new(&p.root).is_dir()).map(|(id, p)| Row { id: id.clone(), title: p.title.clone(), root: p.root.clone(), machine: String::new() }).collect() };
    hub.mirrors().shown(|far| for (name, st) in far { for (id, p) in &st.projects { if !p.removed && !p.root.is_empty() && crate::net::far::is(id) { rows.push(Row { id: id.clone(), title: p.title.clone(), root: p.root.clone(), machine: name.clone() }) } } });
    rows
}
/// `improvements.projects`: the picker's rows and the choice to start from.
pub async fn projects(hub: &Arc<Hub>, q: &str) -> Value {
    let last = hub.setting(LAST_KEY).await.unwrap_or_default();
    let (xs, last) = pick_projects(&all_projects(hub).await, q, &last);
    json!({"project":last,"projects":xs.iter().map(|r| json!({"id":r.id,"title":r.title,"root":r.root,"machine":r.machine,"short":if r.machine.is_empty() { String::new() } else { crate::mirror::short_name(&r.machine) }})).collect::<Vec<_>>()})
}

/// Improvements.context: the app's version, the source thread's project and account binding, and every project that is not removed and whose folder (if it has one) is there.
pub async fn context(hub: &Arc<Hub>, thread: &str) -> Value {
    let (project, owns, mut projects) = {
        let m = hub.model().await;
        let t = m.threads.get(thread).filter(|t| !t.deleted);
        let mut ps: Vec<(&String, &crate::model::ProjectMeta)> = m.projects.iter().filter(|(_, p)| !p.removed && (p.root.is_empty() || std::path::Path::new(&p.root).is_dir())).collect();
        ps.sort_by(|a, b| a.0.cmp(b.0));
        (t.map(|t| t.project.clone()).unwrap_or_default(), t.map(|t| t.provider.clone()), ps.into_iter().map(|(id, p)| json!({"id":id,"title":p.title,"root":p.root})).collect::<Vec<_>>())
    };
    projects.shrink_to_fit();
    let binding = match &owns { Some(provider) => hub.setting(&format!("account.thread.{thread}.{provider}")).await.unwrap_or_default(), None => String::new() };
    json!({"appVersion":hub.cfg.version,"thread":thread,"project":project,"accountBinding":binding,"projects":projects})
}

/// Improvements.report: only an explicit report thread (`improvement-…`, a live project or Scratch) supplies a draft; `done` once its turn
/// completed, then the body is its newest assistant message (looking back at most 200 entries).
pub async fn report(hub: &Arc<Hub>, thread: &str) -> Value {
    let refuse = || json!({"error":"Choose the report-writing thread first."});
    let (turn, project) = {
        let m = hub.model().await;
        let Some(t) = m.threads.get(thread).filter(|t| !t.deleted) else { return refuse() };
        if !thread.starts_with("improvement-") { return refuse() }
        let live = m.projects.get(&t.project).map_or(false, |p| !p.removed && !p.root.is_empty());
        if !(live || t.project == "scratch") { return refuse() }
        (t.turn.clone(), t.project.clone())
    };
    let _ = project;
    let done = turn == "completed";
    let mut body = String::new();
    if done {
        let (mut before, mut seen) = (0u64, 0usize);
        'pages: while seen < 200 {
            let page = { let m = hub.model().await; match m.page(&hub.store, thread, before, 32) { Ok(p) => p, Err(_) => break } };
            if page.rows.is_empty() { break }
            let oldest = page.rows.iter().map(|r| r.ord).min().unwrap_or(0);
            for r in page.rows.iter().filter(|r| before == 0 || r.ord < before) { seen += 1; if seen > 200 { break 'pages } if r.kind == "msg" && r.role == "assistant" { body = limit(&r.text); break 'pages } }
            if oldest <= 1 || page.below == 0 { break }
            before = oldest;
        }
    }
    json!({"thread":thread,"done":done,"body":body})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hub::Cfg, presenter::Canned};
    #[test] fn a_creation_key_is_32_lowercase_hex() { assert!(key(&"a1".repeat(16))); assert!(!key(&"A1".repeat(16)) && !key("abc") && !key(&"g".repeat(32))); assert_eq!(thread_for("ab"), "improvement-ab"); }
    #[test] fn the_picker_searches_every_machines_projects_with_the_last_choice_first() {
        let r = |id: &str, title: &str, root: &str, m: &str| Row { id: id.into(), title: title.into(), root: root.into(), machine: m.into() };
        let rows = vec![r("l1~p9", "Backplane", "/home/x/Backplane", "boat.ts.net"), r("p2", "Keyboard", "/w/kbd", ""), r("p1", "Backplane", "/w/Backplane", ""), r("l1~p3", "Antenna", "/w/ant", "boat.ts.net")];
        let ids = |xs: &[Row]| xs.iter().map(|x| x.id.clone()).collect::<Vec<_>>();
        let (xs, last) = pick_projects(&rows, "", "");
        assert_eq!((ids(&xs), last.as_str()), (vec!["p1".to_string(), "p2".into(), "l1~p3".into(), "l1~p9".into()], ""), "this machine's first, then each linked one's, by title");
        assert_eq!(ids(&pick_projects(&rows, "BACKPLANE", "").0), ["p1", "l1~p9"], "any case");
        assert_eq!(ids(&pick_projects(&rows, "backplane boat", "").0), ["l1~p9"], "every word, the machine's name too");
        assert_eq!(ids(&pick_projects(&rows, "kbd", "").0), ["p2"], "or the folder");
        let (xs, last) = pick_projects(&rows, "", "l1~p3"); assert_eq!((xs[0].id.as_str(), last.as_str()), ("l1~p3", "l1~p3"), "the last choice first");
        assert_eq!(pick_projects(&rows, "", "gone").1, "", "a project no longer there is no choice");
    }
    #[test] fn a_body_over_the_bound_is_dropped_not_cut() { assert_eq!(limit("short"), "short"); assert_eq!(limit(&"x".repeat(120_001)), ""); assert_eq!(limit(&"x".repeat(120_000)).len(), 120_000); }
    #[tokio::test(flavor = "multi_thread")] async fn context_lists_live_projects_and_report_reads_only_a_finished_report_thread() {
        let home = std::env::temp_dir().join(format!("bp-imp-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap();
        let k = "ab".repeat(16); let th = thread_for(&k); let board = home.join("board"); std::fs::create_dir_all(&board).unwrap(); let board = board.to_string_lossy().into_owned();
        let log = format!("{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
            format!(r#"{{"$":"ProjectCreated","id":"p1","title":"Board","root":"{board}","at":1}}"#), r#"{"$":"ProjectCreated","id":"p3","title":"Deleted folder","root":"/nonexistent/bp-imp-gone","at":1}"#, r#"{"$":"ProjectCreated","id":"p2","title":"Gone","root":"/work/gone","at":1}"#, r#"{"$":"ProjectRemoved","id":"p2","at":2}"#,
            format!(r#"{{"$":"ThreadCreated","id":"{th}","project":"p1","title":"Report","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":3}}"#),
            format!(r#"{{"$":"MessagePosted","thread":"{th}","msg":"m1","role":"assistant","text":"The draft report.","at":4}}"#),
            r#"{"$":"SettingSet","key":"account.thread.improvement-X.claude","value":"{}","at":4}"#);
        std::fs::write(home.join("events.jsonl"), log).unwrap();
        let hub = Hub::start(Cfg { home: home.clone(), version: "9.9.9".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let c = context(&hub, &th).await;
        assert_eq!((c["appVersion"].clone(), c["thread"].clone(), c["project"].clone()), (json!("9.9.9"), json!(th), json!("p1")));
        assert_eq!(c["projects"], json!([{"id":"p1","title":"Board","root":board}]), "a removed project, or one whose folder is gone, is not offered");
        assert_eq!(all_projects(&hub).await.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["p1"], "nor in the picker");
        assert_eq!(report(&hub, "t-not-a-report").await["error"], "Choose the report-writing thread first.");
        assert_eq!(report(&hub, &th).await, json!({"thread":th,"done":false,"body":""}), "no draft until the turn completed");
        hub.commit(&json!({"$":"TurnChanged","thread":th,"state":"completed","at":5})).await.unwrap();
        let r = report(&hub, &th).await; assert_eq!((r["done"].clone(), r["body"].clone()), (json!(true), json!("The draft report.")), "{r}");
        let _ = std::fs::remove_dir_all(&home);
    }
}
