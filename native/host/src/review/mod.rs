//! The automated design review (docs/design-review.md): one run of deterministic checkers over a project, each a findings source in
//! lane E's model (kicad/findings.rs): "drc" and "erc" (E's own runs, waited for when the scope needs them fresh), "fab" (the fab's
//! limits the DRC rules cannot say, and the DRC rules' fab section), "bom", "outputs", "assembly", "mech" (review/checks.rs). Agent-judged
//! checks (a datasheet against a footprint) are a separate class, never mixed in silently. A run is kept in the project as
//! `.backplane/design-review.json` (findings, what was skipped and why, the design's fingerprint) and written out as `.backplane/review.md`;
//! `order_review record` takes its verdict and must-changes from it, so the Buy flow rests on it. Published as info `review.<digest>`.
//! Triggers: policy.rs. Tool: `design_review {scope}`.
pub mod checks;
pub mod judged;
pub mod policy;

use crate::fab;
use crate::files::kicad::findings::{self as fd, Finding};
use crate::hub::Hub;
use serde_json::{json, Value};
use std::{path::{Path, PathBuf}, sync::Arc, time::Duration};

pub const FILE: &str = ".backplane/design-review.json";
/// The sources a deterministic run fills, in the order they are listed.
pub const SOURCES: &[&str] = &["drc", "erc", "fab", "bom", "outputs", "assembly", "mech"];
/// A severity in the review's words: error must change, warning should, info is a note.
pub fn level(sev: &str) -> &'static str { match sev { "error" => "must change", "warning" => "should", "excluded" => "excluded", _ => "note" } }

/// The run as kept and published.
pub fn run_json(at: u64, scope: &str, fingerprint: &str, rev: &Value, findings: &[Finding], skipped: &[String], took_ms: u64) -> Value {
    // must-changes first, then the order the sources were checked in
    let mut sorted: Vec<&Finding> = findings.iter().collect(); sorted.sort_by_key(|f| fd::rank(&f.severity));
    let findings: Vec<Finding> = sorted.into_iter().cloned().collect(); let findings = &findings[..];
    let c = fd::counts(findings);
    let by: serde_json::Map<String, Value> = SOURCES.iter().map(|s| (s.to_string(), fd::counts(&findings.iter().filter(|f| f.source == *s).cloned().collect::<Vec<_>>()).to_json())).collect();
    json!({"at": at, "scope": scope, "fingerprint": fingerprint, "rev": rev, "took_ms": took_ms, "counts": c.to_json(), "by_source": by, "skipped": skipped,
        "verdict": if c.error > 0 { "issues" } else { "ok" }, "findings": findings.iter().map(fd::to_json).collect::<Vec<_>>()})
}
/// One line per must-change (for order_review's issues and the Buy gate), at most `n`.
pub fn must_lines(run: &Value, n: usize) -> Vec<String> {
    run["findings"].as_array().into_iter().flatten().chain(run["judged"]["findings"].as_array().into_iter().flatten()).filter(|f| f["severity"] == "error").map(|f| { let refs: Vec<&str> = f["anchors"].as_array().into_iter().flatten().filter_map(|a| a["reference"].as_str()).take(3).collect();
        let t = f["title"].as_str().unwrap_or(""); if refs.is_empty() || refs.iter().any(|r| t.contains(r)) { t.to_string() } else { format!("{}: {t}", refs.join(", ")) } }).take(n).collect()
}
/// `.backplane/review.md` from a run: per severity the findings with where and the fix; what was skipped; the agent's own notes after.
pub fn md(run: &Value, notes: &str) -> String {
    let fs: Vec<&Value> = run["findings"].as_array().into_iter().flatten().collect();
    let mut s = format!("# Design review\n\n{} · scope {} · {} must change, {} should, {} notes\n", run["verdict"].as_str().unwrap_or(""), run["scope"].as_str().unwrap_or(""), run["counts"]["error"], run["counts"]["warning"], run["counts"]["info"]);
    for (sev, head) in [("error", "Must change"), ("warning", "Should"), ("info", "Notes")] {
        let xs: Vec<&&Value> = fs.iter().filter(|f| f["severity"] == sev).collect();
        if xs.is_empty() { continue }
        s += &format!("\n## {head}\n\n");
        for f in xs {
            let at: Vec<String> = f["anchors"].as_array().into_iter().flatten().take(4).map(|a| a["reference"].as_str().filter(|x| !x.is_empty()).or(a["net"].as_str().filter(|x| !x.is_empty())).or(a["text"].as_str()).unwrap_or("").to_string()).filter(|x| !x.is_empty()).collect();
            s += &format!("- **{}** ({}{}){}{}\n", f["title"].as_str().unwrap_or(""), f["source"].as_str().unwrap_or(""), if at.is_empty() { String::new() } else { format!(": {}", at.join(", ")) },
                f["detail"].as_str().filter(|d| !d.is_empty()).map(|d| format!(" {d}")).unwrap_or_default(), f["fix"].as_str().filter(|d| !d.is_empty()).map(|d| format!(" Fix: {d}.")).unwrap_or_default());
        }
    }
    let j = &run["judged"];
    if j.is_object() {
        s += &format!("\n## Agent-judged (datasheets)\n\nChecked: {}. Not checked: {}.\n\n", j["checked"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).unwrap_or_default(), j["unchecked"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).filter(|x| !x.is_empty()).unwrap_or("none".into()));
        for f in j["findings"].as_array().into_iter().flatten() { s += &format!("- **{}** ({}) {}\n", f["title"].as_str().unwrap_or(""), level(f["severity"].as_str().unwrap_or("")), f["detail"].as_str().unwrap_or("")) }
    }
    let sk: Vec<&str> = run["skipped"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    if !sk.is_empty() { s += "\n## Not checked\n\n"; for x in sk { s += &format!("- {x}\n") } }
    if !notes.trim().is_empty() { s += &format!("\n## The reviewer's notes\n\n{}\n", notes.trim()) }
    s
}

/// Everything a run reads that is not the hub's: the project's files.
fn read_files(root: &Path) -> (Vec<(String, String)>, String, String, String) {
    let files: Vec<(String, String)> = crate::settings::fab::find_fab(root).into_iter().map(|p| { let t = std::fs::read_to_string(root.join(&p)).unwrap_or_default(); (p, t) }).collect();
    let f = crate::files::kicad::project::read(&root.to_string_lossy());
    (files, std::fs::read_to_string(&f.pcb).unwrap_or_default(), f.pro, f.pcb)
}
/// The newest output set in mfg/ (a folder with Gerbers), its files and drill texts, and its placement file.
pub fn newest_outputs(root: &Path) -> (Option<checks::Outputs>, Option<(String, String)>) {
    let mut dirs: Vec<(std::time::SystemTime, PathBuf)> = vec![];
    fn walk(d: &Path, depth: usize, out: &mut Vec<(std::time::SystemTime, PathBuf)>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        let mut gerb = false;
        for e in rd.flatten() { let p = e.path(); if p.is_dir() && depth < 3 { walk(&p, depth + 1, out) } else if p.extension().map_or(false, |x| { let x = x.to_string_lossy().to_ascii_lowercase(); x == "gtl" || x == "gbr" || x == "gbl" }) { gerb = true } }
        if gerb { if let Ok(m) = std::fs::metadata(d).and_then(|m| m.modified()) { out.push((m, d.to_path_buf())) } }
    }
    walk(&root.join("mfg"), 0, &mut dirs);
    dirs.sort_by(|a, b| b.0.cmp(&a.0));
    let Some((_, d)) = dirs.into_iter().next() else { return (None, None) };
    let rel = |p: &Path| p.strip_prefix(root).unwrap_or(p).to_string_lossy().into_owned();
    let names: Vec<String> = std::fs::read_dir(&d).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    let drills = names.iter().filter(|n| n.ends_with(".drl") || n.ends_with(".xln")).map(|n| (n.clone(), std::fs::read_to_string(d.join(n)).unwrap_or_default())).collect();
    // the placement file: beside the Gerbers or one folder up (a CPL or -pos/-top-pos csv)
    let pos = [d.clone(), d.parent().map(Path::to_path_buf).unwrap_or_default()].iter().find_map(|dd| std::fs::read_dir(dd).ok()?.flatten().map(|e| e.path()).find(|p| { let n = p.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default(); n.ends_with(".csv") && (n.contains("cpl") || n.contains("pos")) }))
        .and_then(|p| Some((rel(&p), std::fs::read_to_string(&p).ok()?)));
    (Some(checks::Outputs { dir: rel(&d), files: names, drills }), pos)
}

/// The deterministic checkers that read only files (no kicad-cli): fab, bom, outputs, assembly, mech. `bom_lines` is the published BOM.
pub fn run_files(home: &Path, root: &Path, bom_lines: &[Value]) -> (Vec<Finding>, Vec<String>) {
    let (fab_files, pcb, pro, pcb_path) = read_files(root);
    let mut out: Vec<Finding> = vec![]; let mut skipped: Vec<String> = vec![];
    let c = fab::choice(root);
    let parsed: Vec<(String, Value)> = fab_files.iter().filter_map(|(p, t)| Some((p.clone(), serde_json::from_str::<Value>(t).ok()?))).collect();
    let board_fab = parsed.iter().find(|(_, j)| fab::model::KINDS.contains(&j["kind"].as_str().unwrap_or("")) && j["order"] != false);
    let u = if pcb.is_empty() { checks::Usage::default() } else { checks::usage(&pcb) };
    match fab::store::load(home, &c.fab) {
        Some(p) if !pcb_path.is_empty() => {
            if let Some((dru, plan)) = fab::drc_plan(root, &p, &c) { let rel = dru.strip_prefix(root).unwrap_or(&dru).to_string_lossy().into_owned(); out.extend(checks::drc_section(&plan, &std::fs::read_to_string(&dru).unwrap_or_default(), &rel)) }
            match board_fab { Some((path, j)) => out.extend(checks::fab_checks(&p, &c, j, path, &u)), None => skipped.push("Fab options and board size: the board has no fab file (<name>.fab.json) to check".into()) }
            let sel = fab::model::select(&p, &c.kind, &c.opts());
            let (o, pos) = newest_outputs(root);
            out.extend(checks::outputs(o.as_ref(), u.copper_layers, &sel));
            let assembled = c.kind == "pcb" && board_fab.map_or(false, |(_, j)| j["options"]["assemble"] != false);
            out.extend(checks::assembly(&u, pos.as_ref().map(|(n, t)| (n.as_str(), t.as_str())), assembled));
            if o.is_some() { skipped.push("Outputs against the board: a fresh export compared with mfg/ (on demand and before an order, not on save)".into()) }
        }
        Some(_) => skipped.push("Fab, outputs and assembly: no KiCad board in this project".into()),
        None => skipped.push(format!("Fab: no profile of the fab {:?}", c.fab)),
    }
    let boards = board_fab.and_then(|(_, j)| j["qty"].as_u64()).unwrap_or(5);
    if !pcb_path.is_empty() || !bom_lines.is_empty() { let (f, s) = checks::bom(bom_lines, boards); out.extend(f); skipped.extend(s) }
    let (f, s) = checks::mech(&parsed); out.extend(f); skipped.extend(s);
    let _ = pro;
    fd::assign_ids(&mut out);
    (out, skipped)
}

/// The newest outputs in mfg/ against a fresh export of the board (kicad-cli with the board's own plot settings, into the hub's cache):
/// the layers whose drawing differs make the outputs stale. Outputs written with another coordinate format are not compared (said).
pub async fn outputs_fresh(hub: &Arc<Hub>, root: &Path) -> (Vec<Finding>, Vec<String>) {
    let (o, _) = { let r = root.to_path_buf(); tokio::task::spawn_blocking(move || newest_outputs(&r)).await.unwrap_or((None, None)) };
    let Some(o) = o else { return (vec![], vec![]) };
    let pcb = crate::files::kicad::project::read(&root.to_string_lossy()).pcb;
    let cli = crate::files::kicad::ipc::runtime(hub).await.cli;
    if cli.is_empty() || pcb.is_empty() { return (vec![], vec!["Outputs against the board: no kicad-cli to export with".into()]) }
    let tmp = hub.cfg.home.join("cache").join("review").join(crate::files::kicad::kserve::digest(&pcb));
    let (dir, odir) = (root.join(&o.dir), o.dir.clone());
    let r = tokio::task::spawn_blocking(move || stale_vs_export(&cli, &pcb, &tmp, &dir)).await.unwrap_or(Err("failed".into()));
    match r {
        Ok((_, 0, _)) => (vec![], vec![format!("Outputs against the board: {odir} was written with other Gerber settings than the board's, so it was not compared")]),
        Ok((stale, _, other)) => {
            let skip = if other.is_empty() { vec![] } else { vec![format!("Outputs against the board: {odir} was made by KiCad {other}, whose apertures and fills differ from this KiCad's; only its outline was compared")] };
            if stale.is_empty() { (vec![], skip) } else { (vec![checks::finding("outputs", "warning", "outputs_stale", format!("The outputs in {odir} are older than the board: {} differ", stale.join(", ")), "A fresh export of the saved board draws these layers differently.".into(), vec![checks::file_anchor(&odir)], "Export the Gerbers and drill files again from the saved board (a buy thread exports fresh ones when it orders)")], skip) }
        }
        Err(e) => (vec![], vec![format!("Outputs against the board: the export failed ({e})")]),
    }
}
/// Export the board's Gerbers into `tmp` and compare them with the set in `kept`: (the layers that differ, how many were compared).
pub fn stale_vs_export(cli: &str, pcb: &str, tmp: &Path, kept: &Path) -> Result<(Vec<String>, usize, String), String> {
    let _ = std::fs::remove_dir_all(tmp); std::fs::create_dir_all(tmp).map_err(|e| e.to_string())?;
    let out = crate::proc::std_cmd(crate::proc::Kind::Local, cli).args(["pcb", "export", "gerbers", "--board-plot-params", "-o"]).arg(tmp).arg(pcb).output().map_err(|e| e.to_string())?;
    if !out.status.success() { return Err(String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("kicad-cli failed").to_string()) }
    let read = |d: &Path| -> Vec<(String, String)> { std::fs::read_dir(d).map(|r| r.flatten().filter_map(|e| { let n = e.file_name().to_string_lossy().into_owned(); (n.contains("_Cu") || n.contains("Edge_Cuts") || n.contains("_Mask")).then(|| Some((n, std::fs::read_to_string(e.path()).ok()?))).flatten() }).collect()).unwrap_or_default() };
    let (a, b) = (read(tmp), read(kept));
    let (stale, n) = checks::stale_layers(&a, &b);
    let mine = a.first().map(|x| checks::writer(&x.1)).unwrap_or_default();
    let other = b.iter().map(|x| checks::writer(&x.1)).find(|w| *w != mine).map(|w| w.rsplit(',').next().unwrap_or("").trim_end_matches("*%").to_string()).unwrap_or_default();
    let _ = std::fs::remove_dir_all(tmp);
    Ok((stale, n, other))
}

/// The project's BOM lines as lane D's BOM publishes them (`mgmt.bom.<digest>`): asked for here whether or not a panel shows it (its
/// watch), waited for up to `secs` while its stock lookups are still being checked; without a published BOM, the lines read from the saved
/// schematic (MPNs need no lookup). Never the parts service from here.
pub async fn bom_lines(hub: &Arc<Hub>, pro: &str, secs: u64) -> Vec<Value> {
    use crate::files::kicad::{bompanel, mgmt::{self, Panel}};
    let key = mgmt::data_key("bom", pro);
    let read = |hub: &Arc<Hub>| hub.info_now()[key.as_str()].as_str().and_then(|t| serde_json::from_str::<Value>(t).ok());
    bompanel::BomPanel.watch(hub, pro);
    for _ in 0..(secs * 2) {
        if let Some(v) = read(hub) { if v["checking"].as_u64().unwrap_or(0) == 0 { break } }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    if let Some(v) = read(hub) { if let Some(ls) = v["lines"].as_array() { return ls.clone() } }
    let (p, sch) = (pro.to_string(), bompanel::sch_of(pro));
    tokio::task::spawn_blocking(move || bompanel::lines(&bompanel::parts_now(&p, &sch).0)).await.unwrap_or_default()
}

/// The project's E-side DRC and ERC: their findings when fresh; with `wait` the run is asked for and waited on (a few minutes at most).
async fn drc_erc(hub: &Arc<Hub>, pro: &str, wait: bool) -> (Vec<Finding>, Vec<String>, Value) {
    use crate::files::kicad::checkrun as cr;
    let p = json!({"project": pro});
    if cr::request(hub, "kicad.checks.watch", &p).await.is_err() { return (vec![], vec!["DRC and ERC: this project's checks could not be read".into()], Value::Null) }
    let fresh = |s: &Value| ["drc", "erc"].iter().all(|k| s[*k].is_null() || s[*k]["state"] == "fresh");
    let mut sum = cr::request(hub, "kicad.checks.watch", &p).await.unwrap_or(Value::Null);
    if wait && !fresh(&sum) {
        let _ = cr::request(hub, "kicad.checks.run", &p).await;
        // until both are fresh or failed; a run that has not even started after 10 s is not waited on
        let busy = |s: &Value, i: u32| ["drc", "erc"].iter().any(|k| !s[*k].is_null() && match s[*k]["state"].as_str().unwrap_or("") { "running" | "deferred" => true, "stale" | "none" => i < 5, _ => false });
        for i in 0..150u32 { tokio::time::sleep(Duration::from_secs(2)).await; sum = cr::request(hub, "kicad.checks.watch", &p).await.unwrap_or(Value::Null); if fresh(&sum) || !busy(&sum, i) { break } }
    }
    let mut skipped = vec![]; let mut out = vec![];
    for k in ["drc", "erc"] {
        let s = &sum[k]; if s.is_null() { continue }
        if s["state"] != "fresh" { skipped.push(format!("{}: {} ({})", k.to_uppercase(), s["state"].as_str().unwrap_or("not run"), s["reason"].as_str().unwrap_or(""))); continue }
        let l = cr::request(hub, "kicad.checks.list", &json!({"project": pro, "source": k, "limit": 2000})).await.unwrap_or(Value::Null);
        out.extend(l["findings"].as_array().into_iter().flatten().map(fd::from_json));
    }
    (out, skipped, json!({"drc": sum["drc"]["rev"], "erc": sum["erc"]["rev"]}))
}

/// Fab.review.run: a run of the scope ("save": the file checks only; "full" and "order": DRC and ERC fresh too), kept, written and published.
pub async fn run(hub: &Arc<Hub>, root: &Path, scope: &str) -> Result<Value, String> {
    let t0 = crate::hub::now_ms();
    let pro = crate::files::kicad::project::read(&root.to_string_lossy()).pro;
    let bom_lines: Vec<Value> = if pro.is_empty() { vec![] } else { bom_lines(hub, &pro, if scope == "save" { 0 } else { 20 }).await };
    let (home, r) = (hub.cfg.home.clone(), root.to_path_buf());
    let (mut fs, mut skipped): (Vec<Finding>, Vec<String>) = tokio::task::spawn_blocking(move || run_files(&home, &r, &bom_lines)).await.map_err(|_| "the review failed".to_string())?;
    let mut rev = Value::Null;
    if !pro.is_empty() {
        if scope == "save" { skipped.push("DRC and ERC: run beside the design (the Checks panel); a full review waits for them fresh".into()) }
        else {
            let (d, s, r) = drc_erc(hub, &pro, true).await; let mut d = d; d.extend(fs); fs = d; skipped.extend(s); rev = r;
            let (o, s) = outputs_fresh(hub, root).await; fs.extend(o); skipped.extend(s);
            skipped.retain(|x| !x.starts_with("Outputs against the board: a fresh export"));
        }
    }
    if scope == "save" { skipped.push("Agent-judged checks (datasheets against footprints): on demand and before an order".into()) }
    let r = root.to_path_buf(); let (scope2, skipped2) = (scope.to_string(), skipped.clone());
    let v = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let fp = crate::settings::order::fingerprint(&r);
        let mut v = run_json(crate::agents::now_s(), &scope2, &fp, &rev, &fs, &skipped2, crate::hub::now_ms() - t0);
        // the agent-judged section stays while the design is the one it judged
        if let Some(old) = std::fs::read_to_string(r.join(FILE)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) { if old["judged"]["fingerprint"] == json!(fp) { v["judged"] = old["judged"].clone() } }
        if !crate::copy::on() {
            let d = r.join(".backplane"); std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
            std::fs::write(r.join(FILE), serde_json::to_string(&v).unwrap_or_default() + "\n").map_err(|e| e.to_string())?;
            std::fs::write(d.join("review.md"), md(&v, "")).map_err(|e| e.to_string())?;
        }
        Ok(v)
    }).await.map_err(|_| "the review failed".to_string())??;
    if !pro.is_empty() {
        let mut small = v.clone(); if let Some(a) = small["findings"].as_array_mut() { a.truncate(300) } let _ = hub.set_info(&info_key(&pro), &small.to_string()).await;
        publish_checks(hub, &pro, &v).await;
    }
    Ok(v)
}
/// The run's sources in the one Checks list (lane E's, kicad/checkrun.rs): each source's findings, labelled, with the run's when, scope
/// and what that source could not check. DRC and ERC are E's own and are not set from here.
pub const LABELS: &[(&str, &str)] = &[("fab", "Fab rules"), ("bom", "BOM"), ("outputs", "Outputs"), ("assembly", "Assembly"), ("mech", "Mechanical")];
pub async fn publish_checks(hub: &Arc<Hub>, pro: &str, run: &Value) {
    use crate::files::kicad::checkrun as cr;
    let _ = cr::request(hub, "kicad.checks.watch", &json!({"project": pro})).await;
    let all: Vec<Finding> = run["findings"].as_array().into_iter().flatten().map(fd::from_json).collect();
    let skipped: Vec<&str> = run["skipped"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    for (id, label) in LABELS {
        let fs: Vec<Finding> = all.iter().filter(|f| f.source == *id).cloned().collect();
        let mine: Vec<&&str> = skipped.iter().filter(|s| { let l = s.to_ascii_lowercase(); match *id { "fab" => l.starts_with("fab"), "bom" => l.starts_with("bom"), "outputs" => l.starts_with("outputs"), "assembly" => l.starts_with("assembly"), _ => l.contains(".fab.json") } }).collect();
        let meta = json!({"label": label, "review": {"at": run["at"], "scope": run["scope"], "verdict": run["verdict"], "skipped": mine}});
        if let Err(e) = cr::set_findings(hub, pro, id, fs, meta).await { eprintln!("review: the Checks list did not take {id}: {e}") }
    }
}
pub fn info_key(pro: &str) -> String { format!("review.{}", crate::files::kicad::kserve::digest(pro)) }
/// The last run kept in the project, whatever design it was of: its summary (no findings).
pub fn last(root: &Path) -> Value {
    let Some(v) = std::fs::read_to_string(root.join(FILE)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else { return Value::Null };
    json!({"at": v["at"], "scope": v["scope"], "verdict": v["verdict"], "counts": v["counts"], "by_source": v["by_source"], "skipped": v["skipped"], "took_ms": v["took_ms"]})
}
/// The last run kept in the project, if it is of the design as it is now.
pub fn current(root: &Path) -> Option<Value> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(root.join(FILE)).ok()?).ok()?;
    (v["fingerprint"].as_str()? == crate::settings::order::fingerprint(root)).then_some(v)
}

/// The tool's answer: the run's summary and its findings (the first `max`), each with its id, severity in the review's words, where and the fix.
pub fn answer(v: &Value, max: usize) -> Value {
    let fs: Vec<Value> = v["findings"].as_array().into_iter().flatten().take(max).map(|f| { let mut f = f.clone(); f["level"] = json!(level(f["severity"].as_str().unwrap_or(""))); f }).collect();
    json!({"verdict": v["verdict"], "scope": v["scope"], "counts": v["counts"], "by_source": v["by_source"], "skipped": v["skipped"], "took_ms": v["took_ms"], "findings": fs,
        "more": v["findings"].as_array().map_or(0, |a| a.len().saturating_sub(max)), "written": [".backplane/review.md", FILE]})
}

/// The evidence an agent judges (scope "judged"): the parts worth checking against their datasheets, what to do, and how to answer.
pub async fn judged_evidence(root: &Path) -> Result<Value, String> {
    let r = root.to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let f = crate::files::kicad::project::read(&r.to_string_lossy());
        if f.pro.is_empty() { return Err("this project has no KiCad project".into()) }
        let sch = crate::files::kicad::bompanel::sch_of(&f.pro);
        let sheets = crate::files::kicad::bompanel::read_sheets(&sch);
        let texts: Vec<String> = sheets.iter().map(|s| String::from_utf8_lossy(&s.text).into_owned()).collect();
        let parts = crate::files::kicad::bom::parts(&crate::files::kicad::bompanel::project_name(&f.pro), &sheets);
        let pcb = std::fs::read_to_string(&f.pcb).unwrap_or_default();
        let (ev, more) = judged::evidence(&parts, &texts, &pcb, 24);
        Ok(json!({"scope": "judged", "parts": ev, "more": more, "how": judged::HOW, "answer_with": "design_review_judged {findings: [{reference, pin?, severity, title, detail, page, confidence, fix?}], checked: [references], unchecked: [references]}"}))
    }).await.map_err(|_| "failed".to_string())?
}
/// An agent's judged answer kept: in the run (its own section, for the design as it is now), review.md, and the Checks list as "judged".
pub async fn judged_record(hub: &Arc<Hub>, root: &Path, a: &Value) -> Result<Value, String> {
    if crate::copy::on() { return Err(crate::copy::HOME_ONLY.into()) }
    let mut fs = judged::findings_of(a)?; fd::assign_ids(&mut fs);
    let names = |k: &str| a[k].as_array().map(|x| x.iter().filter_map(Value::as_str).map(str::to_string).collect::<Vec<_>>()).unwrap_or_default();
    let (checked, unchecked) = (names("checked"), names("unchecked"));
    let (r, fs2, c2, u2) = (root.to_path_buf(), fs.clone(), checked.clone(), unchecked.clone());
    let v = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let fp = crate::settings::order::fingerprint(&r);
        let mut v = std::fs::read_to_string(r.join(FILE)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).unwrap_or(json!({"findings": [], "skipped": [], "counts": {}, "verdict": "ok"}));
        v["judged"] = json!({"at": crate::agents::now_s(), "fingerprint": fp, "findings": fs2.iter().map(fd::to_json).collect::<Vec<_>>(), "checked": c2, "unchecked": u2, "counts": fd::counts(&fs2).to_json()});
        std::fs::create_dir_all(r.join(".backplane")).map_err(|e| e.to_string())?;
        std::fs::write(r.join(FILE), serde_json::to_string(&v).unwrap_or_default() + "\n").map_err(|e| e.to_string())?;
        std::fs::write(r.join(".backplane/review.md"), md(&v, "")).map_err(|e| e.to_string())?;
        Ok(v)
    }).await.map_err(|_| "failed".to_string())??;
    let pro = crate::files::kicad::project::read(&root.to_string_lossy()).pro;
    if !pro.is_empty() {
        let _ = crate::files::kicad::checkrun::request(hub, "kicad.checks.watch", &json!({"project": pro})).await;
        let meta = json!({"label": "Datasheets", "judged": true, "checked": checked, "unchecked": unchecked});
        if let Err(e) = crate::files::kicad::checkrun::set_findings(hub, &pro, "judged", fs, meta).await { eprintln!("review: the Checks list did not take judged: {e}") }
    }
    Ok(json!({"recorded": true, "counts": v["judged"]["counts"], "checked": v["judged"]["checked"].as_array().map_or(0, Vec::len), "written": ".backplane/review.md"}))
}

pub struct Tools(pub Arc<Hub>);
pub const TOOL: &str = "design_review";
pub const JUDGED: &str = "design_review_judged";
pub fn judged_def() -> Value {
    json!({"name": JUDGED, "description": "Record the agent-judged part of the design review (after design_review with scope \"judged\" handed you the parts and their evidence): each mismatch between a part's symbol or footprint and its datasheet, with the datasheet page it rests on and your confidence. They are kept apart from Backplane's deterministic findings, marked agent-judged, in .backplane/review.md and the Checks list.",
        "inputSchema": {"type": "object", "properties": {"findings": {"type": "array", "items": {"type": "object"}, "description": "[{reference, pin?, severity: must change|should|note, title, detail, page (datasheet page and table or figure), confidence (0-1), fix?}]; none when everything matched"},
            "checked": {"type": "array", "items": {"type": "string"}, "description": "the references you checked"}, "unchecked": {"type": "array", "items": {"type": "string"}, "description": "the references you could not check (no datasheet)"}}, "required": ["findings", "checked"]}})
}
pub fn tool_def() -> Value {
    json!({"name": TOOL, "description": "Run Backplane's design review of this project: deterministic checks, each finding with its severity (must change, should, note), where it is (references, nets, files) and how to fix it. Sources: drc and erc (KiCad's, waited for fresh), fab (the project's fab limits DRC rules cannot say: options, board size, the narrowest track and smallest via against the free and costs-more limits, and whether the DRC rules hold the fab's section), bom (MPNs; stock and lifecycle where the BOM has them), outputs (Gerbers and drill files in mfg/: layers, outline, drill sizes), assembly (sides, fiducials, the placement file), mech (fab files' own DFM results). It writes .backplane/review.md and keeps the run; order_review record then takes its verdict and must-changes. scope: full (default) or order (the same, before ordering), or save (file checks only, quick); judged hands you the parts to check against their datasheets (symbol pins, footprint pads and pitch) with how to answer (design_review_judged): run it on request and before an order.",
        "inputSchema": {"type": "object", "properties": {"scope": {"type": "string", "description": "full (default), order, save or judged"}, "max": {"type": "number", "description": "findings in the answer (default 60); all are in .backplane/review.md"}}, "required": []}})
}
impl crate::agents::mcp::ToolSet for Tools {
    fn list(&self, _thread: &str) -> Vec<Value> { vec![tool_def(), judged_def()] }
    fn owns(&self, name: &str) -> bool { name == TOOL || name == JUDGED }
    fn call<'a>(&'a self, thread: &'a str, name: &'a str, a: &'a Value) -> crate::api::Fut<'a, (bool, Value)> {
        Box::pin(async move {
            let Some(r) = crate::files::root_of(&self.0, thread).await else { return (false, json!({"error": "this thread has no project folder"})) };
            let root = PathBuf::from(crate::files::kicad::canonical::design_root(&r.to_string_lossy()));
            if name == JUDGED { return match judged_record(&self.0, &root, a).await { Ok(v) => (true, v), Err(e) => (false, json!({"error": e})) } }
            if a["scope"] == "judged" { return match judged_evidence(&root).await { Ok(v) => (true, v), Err(e) => (false, json!({"error": e})) } }
            let scope = match a["scope"].as_str().unwrap_or("") { "save" => "save", "order" => "order", _ => "full" };
            match run(&self.0, &root, scope).await { Ok(v) => (true, answer(&v, a["max"].as_u64().unwrap_or(60) as usize)), Err(e) => (false, json!({"error": e})) }
        })
    }
}
pub fn start(hub: &Arc<Hub>, ag: Option<&Arc<crate::agents::Agents>>) {
    if let Some(ag) = ag { ag.add_tools(Arc::new(Tools(hub.clone()))) }
    policy::start(hub);
}

/// The outputs of a project folder (REVIEW_ROOT, a copy) against a fresh export with KICAD_CLI: by hand only.
#[cfg(test)]
#[test] #[ignore] fn review_outputs_on_a_copy() {
    let (Ok(root), Ok(cli)) = (std::env::var("REVIEW_ROOT"), std::env::var("KICAD_CLI")) else { return };
    let r = Path::new(&root); let (o, _) = newest_outputs(r); let o = o.unwrap(); let pcb = crate::files::kicad::project::read(&root).pcb;
    let t = crate::plat::tmp::dir("review-export"); let t0 = std::time::Instant::now();
    println!("OUTPUTS {} {:?} in {} ms", o.dir, stale_vs_export(&cli, &pcb, &t.join("x"), &r.join(&o.dir)), t0.elapsed().as_millis());
}
/// The judged evidence of a project folder (REVIEW_ROOT, a copy), printed: by hand only.
#[cfg(test)]
#[test] #[ignore] fn review_judged_on_a_copy() {
    let Ok(root) = std::env::var("REVIEW_ROOT") else { return };
    let rt = tokio::runtime::Runtime::new().unwrap(); let v = rt.block_on(judged_evidence(Path::new(&root))).unwrap();
    println!("JUDGED {} parts, {} more", v["parts"].as_array().unwrap().len(), v["more"]);
    for p in v["parts"].as_array().unwrap().iter().take(6) { println!("  {} {} pins {} pads {} pitch {} ds {}", p["reference"], p["mpn"], p["symbol_pins"].as_array().unwrap().len(), p["pads"]["count"], p["pads"]["pitch_mm"], !p["datasheet"].as_str().unwrap_or("").is_empty()) }
}
/// A run of the file checks on a project folder named by REVIEW_ROOT (a copy), printed: by hand only.
#[cfg(test)]
#[test] #[ignore] fn review_run_on_a_copy() {
    let Ok(root) = std::env::var("REVIEW_ROOT") else { return };
    let pro = crate::files::kicad::project::read(&root).pro; let sch = crate::files::kicad::bompanel::sch_of(&pro);
    let lines = crate::files::kicad::bompanel::lines(&crate::files::kicad::bompanel::parts_now(&pro, &sch).0);
    let h = crate::plat::tmp::dir("review-copy"); let (fs, sk) = run_files(&h, Path::new(&root), &lines);
    let v = run_json(0, "save", "", &Value::Null, &fs, &sk, 0);
    println!("REVIEW {} {}", v["counts"], v["by_source"]); for f in v["findings"].as_array().unwrap() { println!("  {} [{}] {} | {}", f["severity"], f["source"], f["title"], f["anchors"].as_array().map_or(0, Vec::len)) } for s in sk { println!("  skipped: {s}") }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn put(p: PathBuf, t: &str) { std::fs::create_dir_all(p.parent().unwrap()).unwrap(); std::fs::write(p, t).unwrap() }
    #[test] fn review_run_the_files_of_a_project_give_findings_by_source_and_say_what_they_skipped() {
        let t = crate::plat::tmp::dir("review-run"); let r: &Path = &t; let h = crate::plat::tmp::dir("review-home");
        put(r.join("b/b.kicad_pro"), r#"{"board":{"design_settings":{"rules":{"min_clearance":0.0}}},"net_settings":{"classes":[{"name":"Default","clearance":0.2}]}}"#);
        put(r.join("b/b.kicad_pcb"), "(kicad_pcb\n\t(general\n\t\t(thickness 1.6)\n\t)\n\t(layers\n\t\t(0 \"F.Cu\" signal)\n\t\t(2 \"B.Cu\" signal)\n\t)\n\t(segment\n\t\t(start 1 2)\n\t\t(end 3 4)\n\t\t(width 0.05)\n\t\t(layer \"F.Cu\")\n\t\t(net \"A\")\n\t)\n)\n");
        put(r.join("b/b.fab.json"), r#"{"kind":"pcb","name":"B","qty":5,"options":{"layers":2},"features":{"width_mm":30,"height_mm":20}}"#);
        put(r.join("mech/c.fab.json"), r#"{"kind":"3dp","name":"Case"}"#);
        let lines = vec![json!({"refs": ["R1"], "qty": 1, "mpn": "", "dnp": false})];
        let (fs, sk) = run_files(&h, r, &lines);
        let src = |s: &str| fs.iter().filter(|f| f.source == s).map(|f| f.rule.as_str()).collect::<Vec<_>>();
        assert!(src("fab").contains(&"fab_rules_section") && src("fab").contains(&"fab_limit"), "{:?}", src("fab"));
        assert_eq!(src("bom"), ["bom_no_mpn"]); assert_eq!(src("outputs"), ["outputs_none"]);
        assert!(fs.iter().all(|f| !f.id.is_empty() && f.id.starts_with(&f.source)), "ids from E's model");
        assert!(sk.iter().any(|s| s.contains("stock")) && sk.iter().any(|s| s.contains("walls")), "{sk:?}");
        let v = run_json(1, "save", "fp", &Value::Null, &fs, &sk, 3);
        assert_eq!(v["verdict"], "issues"); assert_eq!(v["by_source"]["bom"]["error"], 1);
        let lines = must_lines(&v, 8); assert!(lines.iter().any(|l| l.starts_with("R1: no manufacturer part number") || l.contains("R1")), "{lines:?}");
        let m = md(&v, "Checked by hand: the antenna keepout."); assert!(m.contains("## Must change") && m.contains("## Not checked") && m.contains("antenna keepout"), "{m}");
        assert_eq!(answer(&v, 1)["findings"].as_array().unwrap().len(), 1); assert_eq!(answer(&v, 1)["findings"][0]["level"], "must change");
    }
}
