//! kicad.export10 and the agents' kicad_export10: a project downgraded by kicad-cli into <home>/exports/..., through
//! hub's guard script (scripts/kexport_guard.sh), with the report's words as consent. The rules are kicad/kexport.rs. A deadline
//! returns to the caller without signalling the child: the command keeps its .busy folder until it exits.
use super::git::{proc, Ran};
use super::kicad::bj::{self, J};
use super::kicad::{ipc, kexport as kx, project};
use super::threadpath;
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};

const GUARD: &str = include_str!("scripts/kexport_guard.sh");
const OPEN: &str = include_str!("scripts/kexport_open.sh");
const DEADLINE: Duration = Duration::from_secs(600);

fn result(home: &str, pro: &str, report: &J, state: &str, text: &str) -> J {
    J::obj(&[("export10", J::str(state)), ("path", J::Str(kx::paths(home, pro).output)), ("project", J::str(pro)), ("report", report.clone()), ("text", J::str(text))])
}
/// KExport.run
async fn within(work: impl std::future::Future<Output = Ran>) -> Ran {
    tokio::time::timeout(DEADLINE, work).await.unwrap_or(Ran { code: 124, out: "KiCad export timed out after ten minutes. Its export folder stays locked until the command exits.".into() })
}
/// KExport.command: only a marked, managed folder is cleared
async fn command(home: &str, pro: &str, cli: &str, mode: &str, flags: &[&str]) -> Ran {
    let base = kx::paths(home, pro).base;
    let marker = std::path::Path::new(&format!("{base}/.backplane-export")).is_file();
    let mode = if mode == "export" && !kx::may_clear_in(home, &base, marker) { "refuse" } else { mode };
    let mut a = vec!["-c", GUARD, "sh", home, &base, pro, mode, cli]; a.extend_from_slice(flags);
    within(super::git::proc_ro("sh", &a, "/", &[])).await
}
fn report_at(home: &str, pro: &str) -> Option<J> { std::fs::read(kx::paths(home, pro).report).ok().and_then(|b| bj::parse(&super::kicad::sexpr::decode(&b))) }
/// KExport.project
async fn project_export(home: &str, pro: &str, cli: &str, anyway: bool, expected: &str, agent: bool) -> J {
    if !(pro.starts_with('/') && pro.ends_with(".kicad_pro") && !pro.starts_with(&format!("{home}/exports/"))) { return result(home, pro, &J::Null, "error", "Export needs an absolute .kicad_pro path outside the managed export folder.") }
    let r = command(home, pro, cli, "dry", &["--dry-run"]).await;
    let report = report_at(home, pro).unwrap_or(J::Null);
    if r.code != 0 { return result(home, pro, &report, "error", &r.out) }
    if !kx::valid(&report) { return result(home, pro, &report, "error", "KiCad did not write a valid compatibility report.") }
    let rep = kx::report(&report);
    let consent = anyway && (agent || kx::signature(&bj::parse(expected).unwrap_or(J::Null)) == kx::signature(&report));
    if !(rep.empty() || consent) { return result(home, pro, &report, "confirm", "Review the changes before exporting.") }
    let r = command(home, pro, cli, "export", &kx::flags(&rep, anyway)).await;
    let last = report_at(home, pro).unwrap_or(report);
    if r.code == 0 { result(home, pro, &last, "done", &kx::success(&kx::paths(home, pro).output)) } else { result(home, pro, &last, "error", &r.out) }
}
/// KExport.export: `anyway` accepts the reported changes (a person's when `expected` is the report they saw, an agent's outright)
pub async fn export(rt: &ipc::Env, home: &str, pro: &str, anyway: bool, expected: &str, agent: bool) -> J {
    if rt.status != "ok" || !rt.caps.iter().any(|c| c == "export-v10") {
        return J::obj(&[("kicad", J::str(&rt.status)), ("export10", J::str("unavailable")), ("text", J::str("KiCad 10 export needs a healthy Backplane KiCad runtime with export-v10 capability."))])
    }
    project_export(home, pro, &rt.cli, anyway, expected, agent).await
}
/// KH.put.obj
fn put(o: J, key: &str, v: J) -> J {
    match o { J::Obj(f) => { let mut n = vec![(key.to_string(), v)]; n.extend(f.into_iter().filter(|(k, _)| k != key)); J::Obj(n) } _ => J::obj(&[(key, v)]) }
}
/// KExport.open: the copy opened in stock KiCad 10 (or the file opener); a hub on a copied log opens nothing
async fn open(o: J, pro: &str) -> J {
    if crate::copy::on() { return o }
    let path = o.get_str("path", "").to_string();
    let file = pro.rsplit('/').next().unwrap_or("project.kicad_pro");
    let r = within(proc("sh", &["-c", OPEN, "sh", &path, &format!("{path}{file}")], "/", &[])).await;
    put(o, "openError", J::Str(if r.code == 0 { String::new() } else { bj::trim(&r.out).to_string() }))
}
/// The MCP tool kicad_export10 {path, force} (KExportMcp.run): the result, and whether it is a good answer (done or confirm). Never opens a GUI.
pub async fn mcp(hub: &Arc<Hub>, path: &str, force: bool) -> (bool, Value) {
    let rt = ipc::runtime(hub).await; let home = hub.cfg.home.to_string_lossy().into_owned();
    let r = export(&rt, &home, path, force, "", true).await;
    (matches!(r.get_str("export10", ""), "done" | "confirm"), r.value())
}

pub struct Export10;
impl RpcHandler for Export10 {
    fn methods(&self) -> &'static [&'static str] { &["kicad.export10"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, who: &'a Caller, id: &'a Value, _method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let j = J::of(p);
            let thread = j.get_str("thread", "");
            let Some(cwd) = super::root_of(hub, thread).await else { return vec![threadpath::refusal(id)] };
            let (rt, home, pro) = (ipc::runtime(hub).await, hub.cfg.home.to_string_lossy().into_owned(), project::read(&cwd.to_string_lossy()).pro);
            let r = export(&rt, &home, &pro, j.get_bool("anyway", false), j.get_str("report", ""), false).await;
            let state = r.get_str("export10", "").to_string();
            let text = r.get_str("text", "").to_string();
            let out = if state == "done" { open(r, &pro).await } else { r };
            if state == "done" { // KExportJob.activity
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                let _ = hub.commit(&json!({"$":"ActivityLogged","thread":thread,"act":format!("export10:{}:{id}:{now}", who.client),"tone":"info","kind":"kicad.export10","summary":text,"at":now})).await;
            }
            vec![json!({"t":"reply","id":id,"ok":state != "error","result":put(out, "thread", J::str(thread)).value()})]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(n: &str) -> String { let p = std::env::temp_dir().join(format!("bp-kx-{}-{n}", std::process::id())); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); std::fs::canonicalize(p).unwrap().to_string_lossy().into_owned() }
    /// a stand-in kicad-cli for `project downgrade`: the report it writes comes from $REPORT
    fn rt(dir: &str, report: &str) -> ipc::Env {
        let p = format!("{dir}/kicad-cli");
        std::fs::write(&p, format!("#!/bin/sh\nrep=; out=; prev=; for a in \"$@\"; do [ \"$prev\" = --report-json ] && rep=$a; [ \"$prev\" = -o ] && out=$a; prev=$a; done\nprintf '%s' '{report}' > \"$rep\"\ncase \" $* \" in *' --dry-run '*) ;; *) mkdir -p \"$out\"; echo \"$*\" > \"$out/args\";; esac\n")).unwrap();
        use crate::plat::fs::PermissionsExt; std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        ipc::Env { cli: p, status: "ok".into(), caps: vec!["export-v10".into()], ..Default::default() }
    }
    const LOSSY: &str = r#"{"entries":[{"category":"lower","feature":"Text","detail":"Converted","count":2}]}"#;
    #[tokio::test] async fn consent_follows_the_report_and_agents_may_force() {
        let (home, bin, proj) = (tmp("home"), tmp("bin"), tmp("proj")); let pro = format!("{proj}/My Board.kicad_pro"); std::fs::write(&pro, "{}").unwrap();
        let env = rt(&bin, LOSSY);
        let a = export(&env, &home, &pro, false, "", false).await; assert_eq!(a.get_str("export10", ""), "confirm"); assert_eq!(a.get_str("text", ""), "Review the changes before exporting.");
        let seen = a.get_obj("report").show();
        let no = export(&env, &home, &pro, true, r#"{"entries":[]}"#, false).await; assert_eq!(no.get_str("export10", ""), "confirm"); // a stale consent is no consent
        let yes = export(&env, &home, &pro, true, &seen, false).await; assert_eq!(yes.get_str("export10", ""), "done", "{yes:?}");
        let out = yes.get_str("path", ""); assert!(out.starts_with(&format!("{home}/exports/My-Board-")) && out.ends_with("/kicad10/"));
        assert!(std::fs::read_to_string(format!("{out}args")).unwrap().contains("--force")); assert_eq!(yes.get_str("text", ""), kx::success(out));
        assert_eq!(export(&env, &home, &pro, true, "", true).await.get_str("export10", ""), "done"); // an agent's force needs no signature
    }
    #[tokio::test] async fn an_empty_report_exports_at_once_and_a_bad_one_is_an_error() {
        let (home, bin, proj) = (tmp("home2"), tmp("bin2"), tmp("proj2")); let pro = format!("{proj}/b.kicad_pro"); std::fs::write(&pro, "{}").unwrap();
        assert_eq!(export(&rt(&bin, r#"{"entries":[]}"#), &home, &pro, false, "", false).await.get_str("export10", ""), "done");
        let bad = export(&rt(&bin, r#"{"future":1}"#), &home, &pro, false, "", false).await; assert_eq!((bad.get_str("export10", ""), bad.get_str("text", "")), ("error", "KiCad did not write a valid compatibility report."));
    }
    #[tokio::test] async fn refusals_before_anything_runs() {
        let (home, bin) = (tmp("home3"), tmp("bin3")); let env = rt(&bin, LOSSY);
        for pro in ["relative.kicad_pro", "/a/b.txt", &format!("{home}/exports/x/y.kicad_pro")] { assert_eq!(export(&env, &home, pro, false, "", false).await.get_str("text", ""), "Export needs an absolute .kicad_pro path outside the managed export folder.") }
        assert!(!std::path::Path::new(&format!("{home}/exports")).exists());
        let sick = ipc::Env { status: "broken".into(), ..Default::default() }; let u = export(&sick, &home, "/a/b.kicad_pro", false, "", false).await; assert_eq!((u.get_str("export10", ""), u.get_str("kicad", "")), ("unavailable", "broken"));
        let nocap = ipc::Env { status: "ok".into(), ..Default::default() }; assert_eq!(export(&nocap, &home, "/a/b.kicad_pro", false, "", false).await.get_str("export10", ""), "unavailable");
    }
    #[tokio::test] async fn an_unmarked_folder_is_never_adopted_and_a_link_never_followed() {
        let (home, bin, proj) = (tmp("home4"), tmp("bin4"), tmp("proj4")); let pro = format!("{proj}/b.kicad_pro"); std::fs::write(&pro, "{}").unwrap(); let env = rt(&bin, r#"{"entries":[]}"#);
        let base = kx::paths(&home, &pro).base; std::fs::create_dir_all(format!("{base}/kicad10")).unwrap(); std::fs::write(format!("{base}/kicad10/keep"), "x").unwrap();
        let r = export(&env, &home, &pro, false, "", false).await; assert_eq!(r.get_str("export10", ""), "error"); assert!(r.get_str("text", "").contains("Unmarked export folder refused"), "{r:?}"); assert!(std::path::Path::new(&format!("{base}/kicad10/keep")).exists());
        let (h2, victim) = (tmp("home5"), tmp("victim")); crate::plat::fs::symlink(&victim, format!("{h2}/exports")).unwrap();
        let r = export(&env, &h2, &pro, false, "", false).await; assert_eq!(r.get_str("export10", ""), "error"); assert!(r.get_str("text", "").contains("Symlinked exports folder refused"), "{r:?}"); assert!(std::fs::read_dir(&victim).unwrap().next().is_none());
    }
    #[test] fn put_replaces_at_the_front() { assert_eq!(put(J::obj(&[("a", J::str("1")), ("b", J::str("2"))]), "b", J::str("3")).show(), r#"{"b":"3","a":"1"}"#); assert_eq!(put(J::Null, "k", J::Null).show(), r#"{"k":null}"#) }
}
