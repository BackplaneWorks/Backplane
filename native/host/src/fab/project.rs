//! A project's fab, picked once: `.backplane/fab-profile.json` in the project's design root, `{fab, kind, options, rev_seen}`. Until it is
//! picked the fab is the one Backplane orders from and the kind and options are the project's board fab file's (the first board kind
//! among its `*.fab.json`), else a rigid board with the copper layers and thickness of its KiCad board. A sidecar of Backplane's own:
//! nothing is written into a KiCad file.
use super::model::{Opts, KINDS};
use serde_json::{json, Value};
use std::path::Path;

pub const FILE: &str = ".backplane/fab-profile.json";

#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub fab: String,
    pub kind: String,
    pub options: Value,
    /// where it comes from: "picked" (the file), "fab.json" (a board fab file), "board" (the KiCad board), "default"
    pub by: String,
    /// the profile revision the project last acknowledged (0: none), for "changed since" marks
    pub rev_seen: u64,
    /// the fab file it came from, if any
    pub from: String,
    /// which limits DRC holds the board to: "free" (the fab's base price, the default) or "extra" (what it makes for more)
    pub tier: String,
}
impl Choice {
    pub fn opts(&self) -> Opts { Opts::from(&self.options) }
    pub fn json(&self) -> Value { json!({"fab": self.fab, "kind": self.kind, "options": self.opts().json(), "by": self.by, "rev_seen": self.rev_seen, "from": self.from, "tier": self.tier}) }
}

/// Copper layers and thickness of a KiCad board (its `(layers ...)` and `(general (thickness ...))`), read from its head.
pub fn board_opts(text: &str) -> Value {
    let mut o = json!({});
    if let Some(i) = text.find("(layers") {
        let block = &text[i..]; let end = block.find("\n\t)").or_else(|| block.find("\n  )")).unwrap_or(block.len().min(20000));
        let n = block[..end].lines().filter(|l| { let l = l.trim(); l.starts_with('(') && l.split('"').nth(1).map_or(false, |n| n.ends_with(".Cu")) }).count();
        if n > 0 { o["layers"] = json!(n) }
    }
    if let Some(i) = text.find("(thickness ") { if let Some(v) = text[i + 11..].split(')').next().and_then(|s| s.trim().parse::<f64>().ok()) { o["thickness_mm"] = json!(v) } }
    o
}

/// The project's choice. `fab_files` are its fab files as (path, text) (settings::fab::find_fab), `board` its KiCad board's text.
pub fn choice_in(root: &Path, fab_files: &[(String, String)], board: &str, default_fab: &str) -> Choice {
    if let Some(v) = std::fs::read_to_string(root.join(FILE)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        let fab = v["fab"].as_str().unwrap_or("").to_string(); let kind = v["kind"].as_str().unwrap_or("pcb");
        if !fab.is_empty() {
            return Choice { fab, kind: if KINDS.contains(&kind) { kind.into() } else { "pcb".into() }, options: v.get("options").cloned().unwrap_or(json!({})), by: "picked".into(), rev_seen: v["rev_seen"].as_u64().unwrap_or(0), from: FILE.into(), tier: tier_of(v["tier"].as_str().unwrap_or("")) }
        }
    }
    for (path, text) in fab_files {
        let Ok(v) = serde_json::from_str::<Value>(text) else { continue };
        let kind = v["kind"].as_str().unwrap_or("");
        if KINDS.contains(&kind) && v["order"] != json!(false) {
            return Choice { fab: default_fab.into(), kind: kind.into(), options: v.get("options").cloned().unwrap_or(json!({})), by: "fab.json".into(), rev_seen: 0, from: path.clone(), tier: "free".into() }
        }
    }
    let b = board_opts(board);
    let by = if b.as_object().map_or(true, |m| m.is_empty()) { "default" } else { "board" };
    Choice { fab: default_fab.into(), kind: "pcb".into(), options: b, by: by.into(), rev_seen: 0, from: String::new(), tier: "free".into() }
}

pub fn tier_of(s: &str) -> String { if s == "extra" { "extra".into() } else { "free".into() } }

/// Keep the person's pick (and what revision they have seen). Refused on a copy.
pub fn write(root: &Path, fab: &str, kind: &str, options: &Value, tier: &str, rev_seen: u64) -> Result<(), String> {
    if crate::copy::on() { return Err(crate::copy::HOME_ONLY.into()) }
    if !KINDS.contains(&kind) { return Err(format!("{kind} is not a board kind (one of {})", KINDS.join(", "))) }
    let p = root.join(FILE);
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(&json!({"fab": fab, "kind": kind, "options": options, "tier": tier_of(tier), "rev_seen": rev_seen})).unwrap() + "\n";
    std::fs::write(&p, text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    const BOARD: &str = "(kicad_pcb\n\t(version 20241229)\n\t(general\n\t\t(thickness 1.2)\n\t)\n\t(layers\n\t\t(0 \"F.Cu\" signal)\n\t\t(4 \"In1.Cu\" signal)\n\t\t(6 \"In2.Cu\" power)\n\t\t(2 \"B.Cu\" signal)\n\t\t(9 \"F.Adhes\" user \"F.Adhesive\")\n\t\t(25 \"Edge.Cuts\" user)\n\t)\n)";
    #[test] fn fab_project_the_choice_is_the_pick_else_the_fab_file_else_the_board() {
        let t = crate::plat::tmp::dir("fab-proj"); let r: &Path = &t;
        assert_eq!(board_opts(BOARD), json!({"layers": 4, "thickness_mm": 1.2}));
        let c = choice_in(r, &[], BOARD, "jlcpcb"); assert_eq!((c.by.as_str(), c.opts().layers, c.opts().thickness_mm), ("board", 4.0, 1.2));
        assert_eq!(choice_in(r, &[], "", "jlcpcb").by, "default");
        let files = vec![("mech/case.fab.json".to_string(), r#"{"kind":"3dp"}"#.to_string()), ("boards/a/a.fab.json".to_string(), r#"{"kind":"flex","options":{"layers":2}}"#.to_string())];
        let c = choice_in(r, &files, BOARD, "jlcpcb"); assert_eq!((c.by.as_str(), c.kind.as_str(), c.from.as_str(), c.opts().layers), ("fab.json", "flex", "boards/a/a.fab.json", 2.0));
        write(r, "jlcpcb", "pcb", &json!({"layers": 6}), "extra", 3).unwrap();
        let c = choice_in(r, &files, BOARD, "jlcpcb"); assert_eq!((c.by.as_str(), c.kind.as_str(), c.opts().layers, c.rev_seen, c.tier.as_str()), ("picked", "pcb", 6.0, 3, "extra"));
        assert!(write(r, "jlcpcb", "3dp", &json!({}), "", 0).is_err(), "a profile is a board's");
    }
}
