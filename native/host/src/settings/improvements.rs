//! Improvements (core/hub.bend `Improvements.*`, core/improvements.bend): `improvements.context {sourceThread}` and `improvements.report {thread}`.
//! Settings > Improvements lets the person start a thread that writes a problem report about the app; these two requests give the form what it
//! needs and read the finished report back as a draft. Nothing is published here: the client shows the draft and the person decides.
//! The thread itself is made by `thread.create` with a `creationKey` (the commands lane; `Improvements.key`/`thread` below are its rules).
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
    fn methods(&self) -> &'static [&'static str] { &["improvements.context", "improvements.report", "improvements.preview", "improvements.template", "improvements.prompt", "improvements.diagnostics"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let data = |v: Value| vec![json!({"t":"reply","id":id,"ok":true,"result":v})];
            let t = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            match method {
                // the pure report policy (issue_report.rs), for clients without the Bend bridge: a `body` previews it as it is, else the parts build one
                "improvements.preview" => return data(if p.get("body").is_some() { super::issue_report::preview_body(&t("body")) } else { super::issue_report::preview(&t("text"), &t("os"), &t("version"), &t("details"), &t("diagnostics")) }),
                "improvements.template" => return data(json!({"template": super::issue_report::TEMPLATE})),
                "improvements.prompt" => return data(json!({"prompt": super::issue_report::prompt(&t("description"), &t("os"), &t("version"), &t("evidence"), &t("diagnostics"))})),
                // the elected diagnostics, finished: the local recorder's export allowlisted again (issue_report::safe_hub), no connection ring here
                "improvements.diagnostics" => { let doc = tokio::task::spawn_blocking(super::diagnostics::export).await.ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).unwrap_or(Value::Null); return data(json!({"report": super::issue_report::diagnostic_report(&doc)})) }
                _ => {}
            }
            if method == "improvements.context" { return data(context(hub, p.get("sourceThread").and_then(Value::as_str).unwrap_or("")).await) }
            data(report(hub, p.get("thread").and_then(Value::as_str).unwrap_or("")).await)
        })
    }
}

/// Improvements.context: the app's version, the source thread's project and account binding, and every project that is not removed.
pub async fn context(hub: &Arc<Hub>, thread: &str) -> Value {
    let (project, owns, mut projects) = {
        let m = hub.model().await;
        let t = m.threads.get(thread).filter(|t| !t.deleted);
        let mut ps: Vec<(&String, &crate::model::ProjectMeta)> = m.projects.iter().filter(|(_, p)| !p.removed).collect();
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
    #[test] fn a_body_over_the_bound_is_dropped_not_cut() { assert_eq!(limit("short"), "short"); assert_eq!(limit(&"x".repeat(120_001)), ""); assert_eq!(limit(&"x".repeat(120_000)).len(), 120_000); }
    #[tokio::test(flavor = "multi_thread")] async fn context_lists_live_projects_and_report_reads_only_a_finished_report_thread() {
        let home = std::env::temp_dir().join(format!("bp-imp-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap();
        let k = "ab".repeat(16); let th = thread_for(&k);
        let log = format!("{}\n{}\n{}\n{}\n{}\n{}\n",
            r#"{"$":"ProjectCreated","id":"p1","title":"Board","root":"/work/board","at":1}"#, r#"{"$":"ProjectCreated","id":"p2","title":"Gone","root":"/work/gone","at":1}"#, r#"{"$":"ProjectRemoved","id":"p2","at":2}"#,
            format!(r#"{{"$":"ThreadCreated","id":"{th}","project":"p1","title":"Report","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":3}}"#),
            format!(r#"{{"$":"MessagePosted","thread":"{th}","msg":"m1","role":"assistant","text":"The draft report.","at":4}}"#),
            r#"{"$":"SettingSet","key":"account.thread.improvement-X.claude","value":"{}","at":4}"#);
        std::fs::write(home.join("events.jsonl"), log).unwrap();
        let hub = Hub::start(Cfg { home: home.clone(), version: "9.9.9".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let c = context(&hub, &th).await;
        assert_eq!((c["appVersion"].clone(), c["thread"].clone(), c["project"].clone()), (json!("9.9.9"), json!(th), json!("p1")));
        assert_eq!(c["projects"], json!([{"id":"p1","title":"Board","root":"/work/board"}]), "a removed project is not offered");
        assert_eq!(report(&hub, "t-not-a-report").await["error"], "Choose the report-writing thread first.");
        assert_eq!(report(&hub, &th).await, json!({"thread":th,"done":false,"body":""}), "no draft until the turn completed");
        hub.commit(&json!({"$":"TurnChanged","thread":th,"state":"completed","at":5})).await.unwrap();
        let r = report(&hub, &th).await; assert_eq!((r["done"].clone(), r["body"].clone()), (json!(true), json!("The draft report.")), "{r}");
        let _ = std::fs::remove_dir_all(&home);
    }
}
