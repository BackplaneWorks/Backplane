//! A second opinion on DRC from stock KiCad 10.0, and the warning that stands while the runtime's DRC can miss a short.
//! The fork's DRC (upstream code since its merge base: bp.10 to bp.12, the CLI and the editor alike) can leave real zone-fill copper
//! uncovered by its check, so a foreign-net track inside a GND fill passes; the saved copper and Gerbers are right, only the check is
//! wrong. Fixed in bp.12.1, which says so with the capability `drc-zone-guard`. Until the runtime has it:
//!  - a board with filled zones gets one should-level finding (source "stockdrc"), and order_review's summary says the same, so a review
//!    never reads as clean on that DRC alone;
//!  - when a stock KiCad 10.0.x kicad-cli is on the machine (not one of Backplane's runtimes), a full or order review (never on save,
//!    under the memory guard) exports the project for 10.0 with the runtime's `project downgrade` and runs stock `pcb drc` on the copy.
//!    Each violation stock reports that the runtime's DRC did not (type, items, position, compared as sets) is a finding: an error is a
//!    must-change, a warning a should. Stock 10.0.8 reports the probe that bp.12 misses. When stock ran and added nothing, the warning
//!    drops to a note that says exactly what was run.
//! The stock cli runs on the copy only, in the hub's cache; nothing is written beside the design. Linux, macOS and Windows install paths.
use crate::files::kicad::findings::{self as fd, Anchor, Finding};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const SOURCE: &str = "stockdrc";
pub const LABEL: &str = "Stock DRC (second opinion)";
pub const CAP: &str = "drc-zone-guard";
/// The rule ids of the run's own findings (order_review reads them).
pub const RULE_WARN: &str = "drc_zone_guard_missing";
pub const RULE_NOTE: &str = "drc_second_opinion";
pub const WARN: &str = "This KiCad build's DRC can miss a short inside a zone fill (fixed in Backplane KiCad bp.12.1); get a second opinion before ordering";
/// Memory a stock DRC needs (MB, beside the margin), and the longest it may take.
const NEED_MB: u64 = 1200;
const SECS: u64 = 300;

/// Whether the runtime's DRC has the zone-fill guard (its capabilities, or its version line).
pub fn guarded(caps: &[String], version: &str) -> bool { caps.iter().any(|c| c == CAP) || version.contains(CAP) }
/// Whether the board has filled zones (the fill is what the faulty check reads).
pub fn has_fills(pcb_text: &str) -> bool { pcb_text.contains("(filled_polygon") }
/// A stock KiCad 10.0.x version from `kicad-cli --version` ("10.0.7"), None for anything else (a fork build, 9.x, 10.99).
pub fn stock_version(out: &str) -> Option<String> {
    let v = out.lines().map(str::trim).filter(|l| !l.is_empty()).last()?;
    let rest = v.strip_prefix("10.0.")?;
    (!rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())).then(|| v.to_string())
}
/// Whether a kicad-cli is one of Backplane's runtimes (a manifest.json beside its bin/, or the runtime in use).
fn ours(cli: &Path, runtime_cli: &str) -> bool {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    if !runtime_cli.is_empty() && canon(cli) == canon(Path::new(runtime_cli)) { return true }
    let c = canon(cli);
    c.parent().and_then(Path::parent).is_some_and(|r| r.join("manifest.json").is_file() || r.join("Contents/Resources/manifest.json").is_file())
}
/// Where a stock kicad-cli may be: `BACKPLANE_STOCK_KICAD_CLI` (tests; then only it), the usual install places, then PATH.
fn candidates() -> Vec<PathBuf> {
    if let Ok(p) = std::env::var("BACKPLANE_STOCK_KICAD_CLI") { return vec![PathBuf::from(p)] }
    let mut v: Vec<PathBuf> = ["/usr/bin/kicad-cli", "/usr/local/bin/kicad-cli", "/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli", r"C:\Program Files\KiCad\10.0\bin\kicad-cli.exe"].iter().map(PathBuf::from).collect();
    for d in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) { v.push(d.join(if cfg!(windows) { "kicad-cli.exe" } else { "kicad-cli" })) }
    v
}
/// The stock KiCad 10.0.x kicad-cli on this machine, with its version.
pub async fn find(runtime_cli: &str) -> Option<(PathBuf, String)> {
    let mut seen = std::collections::HashSet::new();
    for c in candidates() {
        if !c.is_file() || ours(&c, runtime_cli) || !seen.insert(std::fs::canonicalize(&c).unwrap_or(c.clone())) { continue }
        let o = tokio::time::timeout(Duration::from_secs(20), tokio::process::Command::new(&c).arg("--version").stdin(std::process::Stdio::null()).kill_on_drop(true).output()).await;
        if let Ok(Ok(o)) = o { if let Some(v) = stock_version(&String::from_utf8_lossy(&o.stdout)) { return Some((c, v)) } }
    }
    None
}

/// A violation's key parts: its type, its items (KIIDs), its places (0.01 mm).
fn items(f: &Finding) -> Vec<String> { let mut v: Vec<String> = f.anchors.iter().filter(|a| !a.uuid.is_empty()).map(|a| a.uuid.clone()).collect(); v.sort(); v }
fn places(f: &Finding) -> Vec<(i64, i64)> { let mut v: Vec<(i64, i64)> = f.anchors.iter().filter(|a| a.at).map(|a| ((a.x * 100.0).round() as i64, (a.y * 100.0).round() as i64)).collect(); v.sort(); v }
/// The violations stock reports that the runtime's DRC did not: same type and the same items, or (KIIDs differ after the export) the
/// same type at the same places, count as the same. Unconnected items, library checks and exclusions are left out (not this class).
pub fn extra(stock: &[Finding], fork: &[Finding]) -> Vec<Finding> {
    stock.iter().filter(|s| s.rule != "unconnected_items" && !fd::library_rule("drc", &s.rule) && matches!(s.severity.as_str(), "error" | "warning"))
        .filter(|s| !fork.iter().any(|f| f.rule == s.rule && ((!items(s).is_empty() && items(f) == items(s)) || (!places(s).is_empty() && places(f) == places(s)))))
        .cloned().collect()
}
fn mine(severity: &str, rule: &str, title: String, detail: String, fix: &str) -> Finding {
    Finding { id: String::new(), source: SOURCE.into(), severity: severity.into(), rule: rule.into(), title, detail, anchors: vec![Anchor { doc: "pcb".into(), ..Default::default() }], fix: fix.into() }
}
/// The stock run's extra violations as review findings (stock's words, items and places kept).
pub fn as_findings(extra: Vec<Finding>, version: &str) -> Vec<Finding> {
    extra.into_iter().map(|mut f| {
        f.source = SOURCE.into();
        f.title = format!("Stock KiCad {version} DRC reports what this KiCad's DRC did not: {}", f.title);
        f.fix = "Fix it in the board (stock KiCad checked the same copper, exported for 10.0); then run the review again".into();
        f
    }).collect()
}

/// What the review adds for DRC trust (pure): `stock` is None when no stock run happened (with why in `skipped`), Some(Ok((version,
/// extras))) when it ran, Some(Err(why)) when it failed.
pub fn findings(guard: bool, fills: bool, stock: Option<Result<(String, Vec<Finding>), String>>) -> (Vec<Finding>, Vec<String>) {
    let mut out = vec![]; let mut skipped = vec![];
    let warn = || mine("warning", RULE_WARN, WARN.into(), "The check can leave real fill copper out, so a track of another net inside a filled zone passes it; the copper and Gerbers are right, only the check is wrong.".into(),
        "Install stock KiCad 10.0 (its kicad-cli) for the review's second opinion, or update Backplane KiCad to bp.12.1 or later");
    match stock {
        Some(Ok((v, ex))) => {
            let n = ex.len();
            out.extend(as_findings(ex, &v));
            if !guard && fills {
                if n == 0 { out.push(mine("info", RULE_NOTE, format!("Second opinion (stock KiCad {v} DRC on the exported project): no additional violations"),
                    "This KiCad build's DRC can miss a short inside a zone fill (fixed in bp.12.1); stock KiCad's DRC ran on a copy exported for 10.0 and reported nothing this one did not.".into(), "")) }
                else { out.push(warn()) }
            }
        }
        Some(Err(why)) => { skipped.push(format!("Stock DRC (second opinion): {why}")); if !guard && fills { out.push(warn()) } }
        None => if !guard && fills { out.push(warn()) },
    }
    (out, skipped)
}

/// Memory available now (MB; 0 unknown). A test hub sets it (BACKPLANE_CHECKS_FREE_MB, as the checks' guard reads).
fn free_mb() -> u64 {
    if let Some(n) = std::env::var("BACKPLANE_CHECKS_FREE_MB").ok().and_then(|v| v.parse().ok()) { return n }
    std::fs::read_to_string("/proc/meminfo").ok().and_then(|m| m.lines().find(|l| l.starts_with("MemAvailable:"))?.split_whitespace().nth(1)?.parse::<u64>().ok()).map_or(0, |kb| kb / 1024)
}
async fn run_cmd(argv: &[String]) -> Result<std::process::Output, String> {
    let mut c = tokio::process::Command::new(&argv[0]); c.args(&argv[1..]).stdin(std::process::Stdio::null()).kill_on_drop(true);
    match tokio::time::timeout(Duration::from_secs(SECS), c.output()).await { Ok(Ok(o)) => Ok(o), Ok(Err(e)) => Err(format!("could not start {}: {e}", argv[0])), Err(_) => Err(format!("took longer than {SECS} s")) }
}
fn last_line(o: &std::process::Output) -> String { let e = String::from_utf8_lossy(&o.stderr); let s = String::from_utf8_lossy(&o.stdout); e.trim().lines().last().or_else(|| s.trim().lines().last()).unwrap_or("").to_string() }

/// What the export for 10.0 left out (its "[drop]" lines), and what that makes of stock's extras: a short stays a must-change (no rule
/// makes a short allowed), anything else may come from a dropped rule and is a should, saying so.
pub fn soften(extra: Vec<Finding>, dropped: &[String]) -> Vec<Finding> {
    if dropped.is_empty() { return extra }
    extra.into_iter().map(|mut f| { if f.rule != "shorting_items" && f.severity == "error" { f.severity = "warning".into() }
        if f.rule != "shorting_items" { f.detail = format!("{}{}The export for KiCad 10.0 left out: {}; this may come from that.", f.detail, if f.detail.is_empty() { "" } else { " " }, dropped.join("; ")) } f }).collect()
}
fn dropped_of(o: &std::process::Output) -> Vec<String> {
    let all = format!("{}\n{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    all.lines().map(str::trim).filter_map(|l| l.strip_prefix("[drop]")).map(|l| l.trim().split(": ").next().unwrap_or("").to_string()).filter(|l| !l.is_empty()).collect()
}
/// Export the board for 10.0 with the runtime's cli (its .kicad_pro, .kicad_pcb and .kicad_dru copied first, so nothing else of the
/// folder goes in; forced when a feature must be dropped, and that said) and run stock DRC on the copy: its findings and what was dropped.
pub async fn stock_drc(cache: &Path, runtime_cli: &str, stock: &Path, pro: &str) -> Result<(Vec<Finding>, Vec<String>), String> {
    let free = free_mb();
    if !fd::admit(free, &[], NEED_MB) { return Err(format!("not run: low memory ({free} MB free)")) }
    let dir = cache.join("stock-drc").join(crate::files::kicad::kserve::digest(pro));
    let _ = std::fs::remove_dir_all(&dir);
    let r = async {
        let (src, out) = (dir.join("in"), dir.join("project"));
        std::fs::create_dir_all(&src).map_err(|e| e.to_string())?;
        let p = Path::new(pro); let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        for ext in ["kicad_pro", "kicad_pcb", "kicad_dru"] { let f = p.with_extension(ext); if f.is_file() { std::fs::copy(&f, src.join(format!("{stem}.{ext}"))).map_err(|e| e.to_string())?; } }
        let staged = src.join(format!("{stem}.kicad_pro"));
        let dg = |force: bool| { let mut a: Vec<String> = vec![runtime_cli.into(), "project".into(), "downgrade".into(), "--target".into(), "10.0".into()]; if force { a.push("--force".into()) } a.extend(["-o".into(), out.to_string_lossy().into(), staged.to_string_lossy().into()]); a };
        let mut o = run_cmd(&dg(false)).await?;
        let mut dropped = vec![];
        if !o.status.success() && format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).contains("--force") { dropped = dropped_of(&o); o = run_cmd(&dg(true)).await? }
        let pcb = out.join(format!("{stem}.kicad_pcb"));
        if !pcb.is_file() { return Err(format!("the export for KiCad 10.0 failed (exit {}){}", o.status.code().unwrap_or(-1), { let l = last_line(&o); if l.is_empty() { l } else { format!(": {l}") } })) }
        let report = dir.join("drc.json");
        let o = run_cmd(&[stock.to_string_lossy().into(), "pcb".into(), "drc".into(), "--format".into(), "json".into(), "--severity-all".into(), "--units".into(), "mm".into(), "-o".into(), report.to_string_lossy().into(), pcb.to_string_lossy().into()]).await?;
        let fs = fd::drc(&std::fs::read_to_string(&report).unwrap_or_default()).ok_or_else(|| format!("stock kicad-cli DRC failed (exit {}){}", o.status.code().unwrap_or(-1), { let l = last_line(&o); if l.is_empty() { l } else { format!(": {l}") } }))?;
        Ok((fs, dropped))
    }.await;
    let _ = std::fs::remove_dir_all(&dir);
    r
}

/// The review's DRC-trust findings for the project: the capability and fills always; the stock run when `run` (full and order scopes)
/// and the runtime's own DRC is known (`fork`: its fresh findings).
pub async fn check(hub: &std::sync::Arc<crate::hub::Hub>, pro: &str, pcb: &str, fork: Option<&[Finding]>, run: bool) -> (Vec<Finding>, Vec<String>) {
    let rt = crate::files::kicad::ipc::runtime(hub).await;
    let guard = guarded(&rt.caps, &rt.version);
    let fills = std::fs::read_to_string(pcb).map(|t| has_fills(&t)).unwrap_or(false);
    let stock = if !run { None } else {
        match find(&rt.cli).await {
            None => None,
            Some(_) if rt.cli.is_empty() => Some(Err("not run: no Backplane KiCad to export the project for 10.0".into())),
            Some(_) if fork.is_none() => Some(Err("not run: this KiCad's own DRC is not fresh, so there is nothing to compare".into())),
            Some((cli, v)) => Some(stock_drc(&hub.cfg.home.join("cache"), &rt.cli, &cli, pro).await.map(|(fs, dropped)| (v.clone(), soften(extra(&fs, fork.unwrap_or(&[])), &dropped)))),
        }
    };
    let (f, mut s) = findings(guard, fills, stock);
    if !run && !guard && fills { s.push("Stock DRC (second opinion): on demand and before an order".into()) }
    (f, s)
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn v(rule: &str, sev: &str, uuids: &[&str], at: Option<(f64, f64)>) -> Finding {
        Finding { id: String::new(), source: "drc".into(), severity: sev.into(), rule: rule.into(), title: rule.into(), detail: String::new(), fix: String::new(),
            anchors: uuids.iter().map(|u| Anchor { doc: "pcb".into(), uuid: (*u).into(), x: at.map_or(0.0, |p| p.0), y: at.map_or(0.0, |p| p.1), at: at.is_some(), ..Default::default() }).collect() }
    }
    /// The whole stock run with stand-ins: a runtime cli whose `project downgrade` copies the board, and a stock cli (10.0.8) whose DRC
    /// reports the short the runtime missed; found by `find` (not one of ours), its extras compared against the runtime's findings.
    #[cfg(unix)]
    #[tokio::test] async fn stockdrc_stand_in_stock_cli_finds_the_missed_short() {
        use std::os::unix::fs::PermissionsExt;
        let t = crate::plat::tmp::dir("stockdrc"); let r: &Path = &t;
        let proj = r.join("p"); std::fs::create_dir_all(&proj).unwrap();
        std::fs::write(proj.join("b.kicad_pro"), "{}").unwrap(); std::fs::write(proj.join("b.kicad_pcb"), "(kicad_pcb (zone (filled_polygon)))").unwrap();
        let rt = r.join("rt/bin"); std::fs::create_dir_all(&rt).unwrap(); std::fs::write(r.join("rt/manifest.json"), r#"{"version":"10.99.0-bp.12"}"#).unwrap();
        let script = |p: &Path, body: &str| { std::fs::write(p, format!("#!/bin/sh\n{body}\n")).unwrap(); std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap(); };
        script(&rt.join("kicad-cli"), r#"[ "$1" = "--version" ] && { echo 10.0.8; exit 0; }
while [ $# -gt 1 ]; do [ "$1" = "-o" ] && out=$2; shift; done; mkdir -p "$out"; cp "$(dirname "$1")"/b.kicad_pcb "$out"/"#);
        let stock = r.join("stock-kicad-cli");
        script(&stock, r#"[ "$1" = "--version" ] && { echo 10.0.8; exit 0; }
while [ $# -gt 1 ]; do [ "$1" = "-o" ] && out=$2; shift; done; [ -f "$1" ] || exit 3
echo '{"coordinate_units":"mm","violations":[{"type":"shorting_items","severity":"error","description":"Items shorting two nets","items":[{"uuid":"t1","description":"Track [SIG] on F.Cu","pos":{"x":5.0,"y":5.0}},{"uuid":"z1","description":"Zone [GND] on F.Cu","pos":{"x":5.0,"y":5.0}}]},{"type":"clearance","severity":"error","description":"Clearance violation","items":[{"uuid":"a","description":"Pad","pos":{"x":1.0,"y":2.0}}]}],"unconnected_items":[],"schematic_parity":[]}' > "$out""#);
        // the runtime is never taken for stock, even when it says 10.0.x
        assert!(ours(&rt.join("kicad-cli"), "")); assert!(!ours(&stock, &rt.join("kicad-cli").to_string_lossy()));
        std::env::set_var("BACKPLANE_STOCK_KICAD_CLI", &stock);
        let (cli, v) = find(&rt.join("kicad-cli").to_string_lossy()).await.expect("the stand-in stock cli is found");
        std::env::remove_var("BACKPLANE_STOCK_KICAD_CLI");
        assert_eq!((cli.as_path(), v.as_str()), (stock.as_path(), "10.0.8"));
        let (fs, dropped) = stock_drc(&r.join("cache"), &rt.join("kicad-cli").to_string_lossy(), &cli, &proj.join("b.kicad_pro").to_string_lossy()).await.unwrap();
        assert!(dropped.is_empty());
        let own = vec![v_("clearance", &["a"], (1.0, 2.0))];
        let ex = extra(&fs, &own);
        assert_eq!(ex.iter().map(|f| f.rule.as_str()).collect::<Vec<_>>(), ["shorting_items"], "only what the runtime missed");
        let (f, _) = findings(false, true, Some(Ok((v, ex))));
        assert_eq!((f[0].severity.as_str(), f[0].source.as_str()), ("error", SOURCE));
        assert!(!r.join("cache/stock-drc").read_dir().map_or(false, |mut d| d.next().is_some()), "the copy is removed after the run");
        fn v_(rule: &str, u: &[&str], at: (f64, f64)) -> Finding { super::tests::v(rule, "error", u, Some(at)) }
    }
    /// Live: STOCKDRC_PRO (a copy), STOCKDRC_RT (the runtime's kicad-cli), the machine's stock kicad-cli. Prints the extras.
    #[tokio::test] #[ignore] async fn stockdrc_live() {
        let (pro, rt) = (std::env::var("STOCKDRC_PRO").unwrap(), std::env::var("STOCKDRC_RT").unwrap());
        let (cli, v) = find(&rt).await.expect("a stock 10.0 kicad-cli");
        let t = crate::plat::tmp::dir("stockdrc-live"); let r: &Path = &t;
        let (fs, dropped) = stock_drc(r, &rt, &cli, &pro).await.unwrap();
        let own = r.join("own.json"); let o = std::process::Command::new(&rt).args(["pcb", "drc", "--format", "json", "--severity-all", "--units", "mm", "-o"]).arg(&own).arg(Path::new(&pro).with_extension("kicad_pcb")).output().unwrap(); assert!(o.status.success() || own.is_file());
        let mine = fd::drc(&std::fs::read_to_string(&own).unwrap()).unwrap();
        let ex = soften(extra(&fs, &mine), &dropped);
        println!("STOCKDRC {} {v}: stock {} own {} dropped {:?} extra {:?}", cli.display(), fs.len(), mine.len(), dropped, ex.iter().map(|f| (&f.rule, &f.severity)).collect::<Vec<_>>());
    }
    #[test] fn stockdrc_extra_violations_only_and_the_warning_until_the_guard() {
        assert_eq!(stock_version("10.0.7\n").as_deref(), Some("10.0.7"));
        for no in ["10.99.0-bp.13", "9.0.4", "10.0.7-bp.12", "10.1.0", ""] { assert_eq!(stock_version(no), None, "{no}") }
        assert!(guarded(&["drc-zone-guard".into()], "")); assert!(guarded(&[], "10.99.0-bp.12.1 drc-zone-guard")); assert!(!guarded(&["embed".into()], "10.99.0-bp.12"));
        assert!(has_fills("(zone (net 1) (filled_polygon (layer F.Cu) (pts")); assert!(!has_fills("(zone (net 1))"));
        let fork = vec![v("clearance", "error", &["a", "b"], Some((1.0, 2.0))), v("silk_overlap", "warning", &["s"], None)];
        let stock = vec![v("clearance", "error", &["a", "b"], Some((1.0, 2.0))), // the same
            v("shorting_items", "error", &["t", "z"], Some((5.0, 5.0))),             // the short the runtime missed
            v("silk_overlap", "warning", &["s2"], Some((9.0, 9.0))),                 // ids changed by the export, a new place: extra
            v("clearance", "error", &["x", "y"], Some((1.0, 2.0))),                  // ids changed, same place: the same
            v("unconnected_items", "error", &["u"], None), v("lib_footprint_mismatch", "warning", &["l"], None), v("courtyards_overlap", "excluded", &["e"], None)];
        let ex = extra(&stock, &fork);
        assert_eq!(ex.iter().map(|f| f.rule.as_str()).collect::<Vec<_>>(), ["shorting_items", "silk_overlap"]);
        // a stock run that adds a short: a must-change, and the warning stands
        let (f, _) = findings(false, true, Some(Ok(("10.0.8".into(), ex.clone()))));
        assert_eq!(f.iter().map(|x| (x.rule.as_str(), x.severity.as_str())).collect::<Vec<_>>(), [("shorting_items", "error"), ("silk_overlap", "warning"), (RULE_WARN, "warning")]);
        assert!(f.iter().all(|x| x.source == SOURCE)); assert!(f[0].title.starts_with("Stock KiCad 10.0.8 DRC reports what this KiCad's DRC did not"));
        // stock ran and added nothing: a note saying exactly that, never "clean"
        let (f, _) = findings(false, true, Some(Ok(("10.0.8".into(), vec![]))));
        assert_eq!((f.len(), f[0].severity.as_str(), f[0].title.as_str()), (1, "info", "Second opinion (stock KiCad 10.0.8 DRC on the exported project): no additional violations"));
        // no stock cli, or it failed: the warning; with the guard or without fills: nothing
        assert_eq!(findings(false, true, None).0[0].title, WARN);
        let (f, s) = findings(false, true, Some(Err("not run: low memory (100 MB free)".into()))); assert_eq!((f[0].rule.as_str(), s[0].as_str()), (RULE_WARN, "Stock DRC (second opinion): not run: low memory (100 MB free)"));
        let s = soften(ex.clone(), &["Custom DRC rule 'U1 fanout area' (1)".into()]);
        assert_eq!(s.iter().map(|x| (x.rule.as_str(), x.severity.as_str())).collect::<Vec<_>>(), [("shorting_items", "error"), ("silk_overlap", "warning")], "a short stays a must-change whatever was dropped");
        assert!(s[1].detail.contains("left out: Custom DRC rule 'U1 fanout area' (1)"));
        assert_eq!(soften(vec![v("clearance", "error", &["q"], None)], &["x".into()])[0].severity, "warning");
        assert!(findings(true, true, None).0.is_empty()); assert!(findings(false, false, None).0.is_empty());
        assert_eq!(findings(true, true, Some(Ok(("10.0.8".into(), ex)))).0.len(), 2, "with the guard, stock's extras still count");
    }
}
