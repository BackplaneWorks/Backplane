//! The BOM panel (screens/mgmt.rs hosts it; the hub's kicad/bompanel.rs publishes `mgmt.bom.<digest>`): the design's BOM lines, sortable by a
//! header press, filtered by what is typed, DNP lines dimmed. A press on a line selects its parts in the open KiCad and lights them in the
//! viewers (`editor.kicad.select`). Pure rules first (sorting, filtering, the footprint's short name), tested.
use super::composer::{Composer, ComposerEvent};
use crate::{hub::Command, i18n::{tf, trs}, screens::settings::Secret, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Col { #[default] Refs, Qty, Value, Footprint, Mpn, Stock, Price, Class }
pub const COLS: &[(Col, &str, &str)] = &[(Col::Refs, "refs", "Refs"), (Col::Qty, "qty", "Qty"), (Col::Value, "value", "Value"), (Col::Footprint, "footprint", "Footprint"), (Col::Mpn, "mpn", "MPN"), (Col::Stock, "stock", "Stock"), (Col::Price, "price", "Price"), (Col::Class, "class", "Class")]; // i18n-ok: ids; labels go through trs

#[derive(Default)]
pub struct Ui { pub sort: Col, pub desc: bool, pub filter: Option<Entity<Composer>>, pub sel: Option<String>,
    /// the selected line's fields (EDIT order), the line they hold, and what the last edit said (ok, words)
    pub fields: Vec<Entity<Composer>>, pub filled: String, pub filled_vals: Vec<String>,
    /// the alternatives asked for a line (its id, the hub's answer; Null while asked)
    pub alts: Option<(String, Value)>,
    /// the last edit's answer (its field, old value and who takes it back), and whether the edit on its way is the panel's undo
    pub last: Option<Value>, pub undoing: bool, pub said: Option<(bool, String)>, pub sending: bool }
/// The fields a line's editor has: (field, label, placeholder).
pub const EDIT: &[(&str, &str, &str)] = &[("mpn", "MPN", "Manufacturer part number"), ("fab", "Fab PN", "The fab's part number"), ("value", "Value", "Value"), ("footprint", "Footprint", "Library:Footprint")]; // i18n-ok: field ids; labels go through trs

fn s<'a>(l: &'a Value, k: &str) -> &'a str { l[k].as_str().unwrap_or("") }
/// "Resistor_SMD:R_0603_1608Metric" as a line shows it: the footprint's name without its library.
pub fn short_fp(fp: &str) -> &str { fp.rsplit_once(':').map_or(fp, |(_, n)| n) }
/// A line's references as shown: "R1, R2, R5".
pub fn refs_text(l: &Value) -> String { l["refs"].as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>().join(", ") }
/// Whether a line matches the filter: every word of it is in the line's references, value, footprint, MPN, maker or fab number (any case);
/// the word `dnp` matches DNP lines.
pub fn matches(l: &Value, q: &str) -> bool {
    let hay = format!("{} {} {} {} {} {}", refs_text(l), s(l, "value"), s(l, "footprint"), s(l, "mpn"), s(l, "mfr"), s(l, "fab")).to_lowercase();
    q.split_whitespace().all(|w| { let w = w.to_lowercase(); (w == "dnp" && l["dnp"] == true) || hay.contains(&w) })
}
/// The lines in a column's order (natural order of the first reference for Refs; the hub's order breaks ties).
pub fn sorted(lines: &[Value], col: Col, desc: bool) -> Vec<Value> {
    let mut v: Vec<(usize, &Value)> = lines.iter().enumerate().collect();
    let key = |l: &Value| -> String { match col { Col::Refs | Col::Qty => String::new(), Col::Stock => stock_rank(l).to_string(),
        Col::Price => format!("{:020.6}", l["stock"]["price"].as_f64().unwrap_or(f64::MAX / 1e300)), Col::Class => l["stock"]["class"].as_str().unwrap_or("~").to_string(), Col::Value => s(l, "value").to_lowercase(), Col::Footprint => short_fp(s(l, "footprint")).to_lowercase(), Col::Mpn => s(l, "mpn").to_lowercase() } };
    v.sort_by(|(i, a), (j, b)| {
        let o = match col { Col::Refs => i.cmp(j), Col::Qty => a["qty"].as_u64().cmp(&b["qty"].as_u64()).then(i.cmp(j)), _ => key(a).cmp(&key(b)).then(i.cmp(j)) };
        if desc { o.reverse() } else { o }
    });
    v.into_iter().map(|(_, l)| l.clone()).collect()
}
/// Stock order: what cannot be ordered first (out, low), then unknown and checking, then in stock, then lines with nothing to look up.
pub fn stock_rank(l: &Value) -> u8 { match l["stock"]["status"].as_str() { Some("out") => 0, Some("low") => 1, Some("unknown") => 2, Some("checking") => 3, Some("in") => 4, _ => 5 } }
/// A price in dollars: cents for a dollar and over, up to four places under it ($1.20, $0.50, $0.0029).
pub fn money(v: f64) -> String {
    if v >= 1.0 { return format!("${v:.2}") }
    let t = format!("{v:.4}"); let t = t.trim_end_matches('0'); let dec = t.split('.').nth(1).map_or(0, str::len);
    if dec < 2 { format!("${v:.2}") } else { format!("${t}") }
}
/// 1060770 as "1.1M", 12400 as "12k", 320 as "320".
pub fn abbrev(n: i64) -> String {
    let f = |x: f64, u: &str| { let r = if x < 10.0 { format!("{:.1}", x) } else { format!("{:.0}", x) }; format!("{}{u}", r.trim_end_matches(".0")) };
    match n { n if n < 0 => "?".into(), n if n < 1000 => n.to_string(), n if n < 1_000_000 => f(n as f64 / 1e3, "k"), n => f(n as f64 / 1e6, "M") }
}
/// A line's stock cell: its words, its tone (a theme colour) and its tooltip; None for a line with nothing to look up.
pub fn stock_view(st: &Value) -> Option<(String, &'static str, String)> {
    let status = st["status"].as_str()?;
    let price = st["price"].as_f64().map(money);
    let class = st["class"].as_str().filter(|c| !c.is_empty()).map(trs);
    let detail = [class, price.map(|p| tf("gpui.mgmt.bom.each", &[("price", p.into()), ("n", st["need"].as_u64().unwrap_or(1).into())]))].into_iter().flatten().collect::<Vec<_>>().join(" · ");
    let n = st["stock"].as_i64().unwrap_or(-1);
    Some(match status {
        "checking" => ("…".into(), "faint", trs("Checking the fab's stock…")),
        "unknown" => ("?".into(), "faint", st["why"].as_str().map(trs).unwrap_or_else(|| trs("Not known"))),
        "out" => (abbrev(n), "bad", [tf("gpui.mgmt.bom.out", &[("n", (n.max(0) as u64).into()), ("need", st["need"].as_u64().unwrap_or(1).into())]), detail].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ")),
        "low" => (abbrev(n), "warn", [tf("gpui.mgmt.bom.low", &[("n", (n.max(0) as u64).into())]), detail].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ")),
        _ => (abbrev(n), "ok", [tf("gpui.mgmt.bom.in", &[("n", (n.max(0) as u64).into())]), detail].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ")),
    })
}
/// The BOM's parts cost for one board, once every line that can be priced is ("≈ $1.23 a board"); with some lines unpriced, said as "at least".
pub fn cost_text(d: &Value) -> Option<String> {
    let (priced, looked, cost) = (d["priced"].as_u64().unwrap_or(0), d["lookups"].as_u64().unwrap_or(0), d["cost"].as_f64().unwrap_or(0.0));
    if priced == 0 || d["checking"].as_u64().unwrap_or(0) > 0 { return None }
    let v = format!("${cost:.2}");
    Some(if priced < looked { tf("gpui.mgmt.bom.cost.least", &[("cost", v.into())]) } else { tf("gpui.mgmt.bom.cost", &[("cost", v.into())]) })
}
/// A footprint's package as people say it: the case size of a chip part ("C_0402_1005Metric" → "0402"), or the package family and pin
/// count ("QFN-48-1EP_6x6mm_P0.4mm" → "QFN-48"); None when it is not recognisable (the name is shown, shortened).
pub fn package(fp: &str) -> Option<String> {
    let n = short_fp(fp);
    for part in n.split('_') { if part.len() == 4 && part.chars().all(|c| c.is_ascii_digit()) && ["0201", "0402", "0603", "0805", "1206", "1210", "1812", "2010", "2512", "01005"].contains(&part) { return Some(part.to_string()) } }
    /* i18n-ok: package family names */ const FAMILIES: [&str; 17] = ["QFN", "DFN", "SOT", "SOIC", "SOP", "SSOP", "TSSOP", "MSOP", "QFP", "LQFP", "TQFP", "BGA", "WLCSP", "SOD", "TO", "SON", "LGA"];
    let head = n.split('_').next().unwrap_or(n);
    let fam: String = head.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    if FAMILIES.contains(&fam.to_ascii_uppercase().as_str()) {
        let mut it = head.splitn(3, '-'); let (a, b) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
        return Some(if !b.is_empty() && b.chars().all(|c| c.is_ascii_digit()) { format!("{a}-{b}") } else { a.to_string() })
    }
    None
}
/// The columns a panel `width` px wide shows, each with its width (None: the rest, the MPN). The MPN is the one that matters: it takes
/// what is left; narrower, Class, then Price, then Footprint go, in that order.
pub fn layout(width: f32) -> Vec<(Col, Option<f32>)> {
    let mut v = vec![(Col::Qty, Some(26.)), (Col::Refs, Some(if width >= 700. { 150. } else { 104. })), (Col::Value, Some(if width >= 700. { 84. } else { 66. }))];
    if width >= 700. { v.push((Col::Footprint, Some(132.))) }
    v.push((Col::Mpn, None));
    v.push((Col::Stock, Some(62.)));
    if width >= 520. { v.push((Col::Price, Some(58.))) }
    if width >= 700. { v.push((Col::Class, Some(64.))) }
    v
}
/// How many characters fit in `px` at the panel's 12 px monospace (7.2 px a character).
pub fn chars_in(px: f32) -> usize { (px / 7.2).floor().max(4.0) as usize }
/// A long part number shortened in its middle ("GRM033R6…E47D"), both ends kept: they tell parts apart.
pub fn mid_ellipsis(s: &str, max: usize) -> String {
    let cs: Vec<char> = s.chars().collect();
    if cs.len() <= max || max < 5 { return s.to_string() }
    let head = (max - 1 + 1) / 2; let tail = max - 1 - head;
    format!("{}…{}", cs[..head].iter().collect::<String>(), cs[cs.len() - tail..].iter().collect::<String>())
}
/// A line's references in `max` characters: as many as fit, then "+n" ("C2, C4, C37 +1").
pub fn refs_fit(refs: &[String], max: usize) -> String {
    let all = refs.join(", ");
    if all.chars().count() <= max { return all }
    let mut out = String::new();
    for (i, r) in refs.iter().enumerate() {
        let more = format!(" +{}", refs.len() - i);
        let next = if out.is_empty() { r.clone() } else { format!("{out}, {r}") }; // i18n-ok: a list of references
        if next.chars().count() + format!(" +{}", refs.len() - i - 1).chars().count() > max { return if out.is_empty() { format!("{}{more}", refs[0].chars().take(max.saturating_sub(more.len())).collect::<String>()) } else { format!("{out}{more}") } }
        out = next;
    }
    out
}
/// The stock cell's words: the count, with what it means said in words where it is not plain "in stock" (never colour alone).
pub fn stock_words(st: &Value) -> Option<String> {
    let n = st["stock"].as_i64().unwrap_or(-1);
    Some(match st["status"].as_str()? { "checking" => "…".into(), "unknown" => "?".into(), "out" => tf("gpui.mgmt.bom.stock.out", &[("n", abbrev(n.max(0)).into())]), "low" => tf("gpui.mgmt.bom.stock.low", &[("n", abbrev(n).into())]), _ => abbrev(n) })
}
/// What a header press does: the same column turns its order round, another sorts by it ascending.
pub fn press(cur: (Col, bool), col: Col) -> (Col, bool) { if cur.0 == col { (col, !cur.1) } else { (col, false) } }
/// The source line: where the parts come from.
pub fn source_text(d: &Value) -> String { if d["source"] == "kicad" { trs("Live from the open KiCad: its unsaved edits are in it") } else { trs("From the saved schematic: KiCad is not open on it") } }

fn filter_text(app: &Backplane, cx: &App) -> String { app.screens.mgmt.bom.filter.as_ref().map_or(String::new(), |f| f.read(cx).content.to_string()) }
/// A line pressed: its parts selected in KiCad and lit in the viewers (zoomed on there, with no editor in the pane).
pub fn select(app: &mut Backplane, id: &str, cx: &mut Context<Backplane>) {
    let d = app.screens.mgmt.data("bom");
    let Some(l) = d["lines"].as_array().into_iter().flatten().find(|l| l["id"] == id) else { return };
    let refs = l["refs"].clone();
    app.screens.mgmt.bom.sel = Some(id.to_string());
    let tid = crate::screens::attach::raw_id(app.data["thread"]["id"].as_str().unwrap_or("")).to_string();
    eprintln!("GPUI_LOG {}", json!({"event": "bom_select", "id": id, "refs": refs}));
    let _ = app.tx.try_send(Command::Rpc("editor.kicad.select".into(), Secret(json!({"thread": tid, "references": refs, "zoom": true})), "xsel"));
    if app.screens.kicad.is_none() { app.screens.xprobe.zoom = Some((String::new(), std::time::Instant::now())); }
    app.ui_epoch += 1; cx.notify();
}
/// The e2e harness's controls: a line each (`mgmt:bom:row:<id>`).
pub fn controls(m: &super::mgmt::Mgmt) -> Vec<(String, String, String)> {
    if m.peek()["open"] != "bom" { return vec![] }
    m.data("bom")["lines"].as_array().into_iter().flatten().map(|l| (format!("mgmt:bom:row:{}", s(l, "id")), s(l, "id").to_string(), format!("bom:row:{}", s(l, "id")))).collect()
}
/// The line's `field` set to `value` (the hub writes it: into the open KiCad as an undoable commit, else the saved schematic).
pub fn edit(app: &mut Backplane, field: &str, value: &str, cx: &mut Context<Backplane>) {
    let d = app.screens.mgmt.data("bom");
    let Some(id) = app.screens.mgmt.bom.sel.clone() else { return };
    let Some(l) = d["lines"].as_array().into_iter().flatten().find(|l| l["id"] == id.as_str()).cloned() else { return };
    if edit_value(&l, field) == value { return }
    send_edit(app, &id, l["refs"].clone(), field, value, cx);
}
fn send_edit(app: &mut Backplane, id: &str, refs: Value, field: &str, value: &str, cx: &mut Context<Backplane>) {
    let p = json!({"project": app.screens.mgmt.project, "refs": refs, "field": field, "value": value});
    eprintln!("GPUI_LOG {}", json!({"event": "bom_edit", "id": id, "field": field, "value": value}));
    let _ = app.tx.try_send(Command::Ask("mgmt.bom.edit".into(), p, EDIT_TAG));
    app.screens.mgmt.bom.sending = true; app.screens.mgmt.bom.said = None; cx.notify();
}
/// What the editor says after an edit: (tone, words, whether it offers its own Undo). KiCad's Ctrl+Z is promised only where the hub said
/// KiCad takes it back (`undo: "kicad"`); the panel's own Undo (the old value written back) is offered after every edit.
pub fn edit_note(said: Option<&(bool, String)>, last: Option<&Value>) -> Option<(&'static str, String, bool)> {
    let (ok, what) = said?;
    if !ok { return Some(("bad", what.clone(), false)) }
    if what == "undone" { return Some(("ok", trs("Put back."), false)) }
    let undo = last.map_or("", |l| l["undo"].as_str().unwrap_or("")); let changed = last.map_or(0, |l| l["changed"].as_u64().unwrap_or(0));
    if changed == 0 { return Some(("dim", trs("Nothing to change."), false)) }
    Some(match (what.as_str(), undo) {
        ("kicad", "kicad") => ("ok", trs("Changed in KiCad: Ctrl+Z there takes it back; save there to keep it."), true),
        ("kicad", _) => ("ok", trs("Changed in KiCad; save there to keep it."), true),
        _ => ("ok", trs("Saved in the schematic."), true),
    })
}
/// The tag of `mgmt.bom.edit`'s answer (Command::Ask).
pub const EDIT_TAG: u64 = 0x6d67_6564;
/// The panel's own undo of its last edit (offered where KiCad's Ctrl+Z does not take it back): the old value written back the same way.
pub fn undo(app: &mut Backplane, cx: &mut Context<Backplane>) {
    let Some(last) = app.screens.mgmt.bom.last.take() else { return };
    if last["changed"].as_u64().unwrap_or(0) == 0 { return }
    let (field, was) = (last["field"].as_str().unwrap_or("").to_string(), last["was"].as_str().unwrap_or("").to_string());
    let id = last["refs"][0].as_str().unwrap_or("").to_string();
    app.screens.mgmt.bom.undoing = true;
    send_edit(app, &id, last["refs"].clone(), &field, &was, cx);
}
/// A line's value for one of the edited fields.
pub fn edit_value(l: &Value, field: &str) -> String { match field { "dnp" => if l["dnp"] == true { "yes".into() } else { "no".into() }, f => s(l, f).to_string() } }
/// The tag of `mgmt.bom.alts`' answer (Command::Ask).
pub const ALTS_TAG: u64 = 0x6d67_616c;
fn ask_alts(app: &mut Backplane, id: &str, fab: &str, need: u64, cx: &mut Context<Backplane>) {
    app.screens.mgmt.bom.alts = Some((id.to_string(), Value::Null));
    let _ = app.tx.try_send(Command::Ask("mgmt.bom.alts".into(), json!({"project": app.screens.mgmt.project, "fab": fab, "need": need}), ALTS_TAG));
    cx.notify();
}
pub fn alts_answered(app: &mut Backplane, reply: &Value, cx: &mut Context<Backplane>) {
    let Some((id, _)) = app.screens.mgmt.bom.alts.clone() else { return };
    let v = if reply["ok"] == false { json!({"error": crate::hub::rpc_error(reply)}) } else if reply["result"].is_object() { reply["result"].clone() } else { reply.clone() };
    eprintln!("GPUI_LOG {}", json!({"event": "bom_alts", "id": id, "n": v["alternatives"].as_array().map_or(0, Vec::len), "error": v["error"]}));
    app.screens.mgmt.bom.alts = Some((id, v)); cx.notify();
}
/// The hub's answer to an edit: where it went and who can take it back, or why not.
pub fn edited(app: &mut Backplane, reply: &Value, cx: &mut Context<Backplane>) {
    let ok = reply["ok"] != false; let why = if ok { String::new() } else { crate::hub::rpc_error(reply) };
    let r = if reply["result"].is_object() { reply["result"].clone() } else { reply.clone() };
    eprintln!("GPUI_LOG {}", json!({"event": "bom_edited", "ok": ok, "why": why, "via": r["via"], "undo": r["undo"], "was": r["was"]}));
    let b = &mut app.screens.mgmt.bom; b.sending = false;
    // an undo's own answer offers no undo of itself
    b.last = if ok && !b.undoing { Some(r.clone()) } else { None };
    b.said = Some((ok, if ok { if b.undoing { "undone".into() } else { r["via"].as_str().unwrap_or("").to_string() } } else { why.to_string() }));
    b.undoing = false;
    if !ok { b.filled.clear(); } // the fields show the line as it is again
    cx.notify();
}
pub fn act(app: &mut Backplane, a: &str, cx: &mut Context<Backplane>) {
    if let Some(id) = a.strip_prefix("row:") { select(app, id, cx) }
    else if let Some((f, v)) = a.strip_prefix("set:").and_then(|x| x.split_once('=')) { edit(app, f, v, cx) }
    else if a == "undo" { undo(app, cx) }
    else if a == "alts" {
        let d = app.screens.mgmt.data("bom"); let Some(id) = app.screens.mgmt.bom.sel.clone() else { return };
        let Some(l) = d["lines"].as_array().into_iter().flatten().find(|l| l["id"] == id.as_str()).cloned() else { return };
        let fab = l["stock"]["fab"].as_str().filter(|f| !f.is_empty()).unwrap_or(s(&l, "fab")).to_string();
        if !fab.is_empty() { ask_alts(app, &id, &fab, l["stock"]["need"].as_u64().unwrap_or(1), cx) }
    }
    else if let Some(q) = a.strip_prefix("filter:") { if let Some(f) = app.screens.mgmt.bom.filter.clone() { f.update(cx, |i, cx| i.set(q.to_string(), cx)); } }
}

pub fn body(app: &mut Backplane, d: &Value, light: bool, cx: &mut Context<Backplane>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    if app.screens.mgmt.bom.filter.is_none() {
        let f = cx.new(|cx| Composer::with_context(cx, "AskInput"));
        f.update(cx, |i, _| i.set_placeholder(&trs("Filter")));
        cx.observe(&f, |_, _, cx| cx.notify()).detach();
        app.screens.mgmt.bom.filter = Some(f);
    }
    if d.is_null() { return div().p_3().text_size(px(12.)).text_color(c("faint")).child(trs("Reading the schematic…")).into_any_element() }
    let all: Vec<Value> = d["lines"].as_array().cloned().unwrap_or_default();
    let q = filter_text(app, cx);
    let (col, desc) = (app.screens.mgmt.bom.sort, app.screens.mgmt.bom.desc);
    let rows: Vec<Value> = sorted(&all, col, desc).into_iter().filter(|l| matches(l, &q)).collect();
    let started = all.iter().any(|l| !s(l, "mpn").is_empty() || !s(l, "fab").is_empty());
    let sel = app.screens.mgmt.bom.sel.clone();
    // the panel's top row, on the same band as the page's bar: what it holds, where from, and the filter
    let summary = tf("gpui.mgmt.bom.summary", &[("parts", d["parts"].as_u64().unwrap_or(0).into()), ("lines", (all.len() as u64).into())]);
    let top = div().flex_shrink_0().min_h(px(super::head::ROW_H)).flex().items_center().gap_2().px_2().border_b_1().border_color(c("rule")).text_size(px(11.))
        .child(div().flex_shrink_0().text_color(c("dim")).child(summary))
        // where the lines come from, as a state: live from the open KiCad (its unsaved edits too), or the saved file
        .child({ let live = d["source"] == "kicad"; div().id("bom-source").flex_shrink_0().flex().items_center().gap_1().px(px(5.)).h(px(18.)).border_1().border_color(c(if live { "accent" } else { "line" }))
            .text_size(px(10.)).text_color(c(if live { "accent" } else { "dim" })).tooltip(crate::keys::tip(&source_text(d), "", light))
            .child(if live { trs("Live") } else { trs("Saved") }) })
        .children(cost_text(d).map(|t| div().flex_shrink_0().text_color(c("dim")).child(t)))
        // the parts search asked to slow down: the lookups wait, quietly
        .when(d["paused"] == true, |r| r.child(div().id("bom-paused").flex_shrink_0().text_color(c("faint")).tooltip(crate::keys::tip(&trs("The parts search asked Backplane to slow down; the lookups go on by themselves shortly."), "", light)).child(trs("Stock check paused"))))
        .child(div().flex_1().min_w(px(80.)).px_2().py(px(2.)).border_1().border_color(c("line")).bg(c("panel")).text_size(px(12.)).children(app.screens.mgmt.bom.filter.clone()));
    // the columns this width shows (MPN first in priority), the same gutter between every two
    let width = app.screens.mgmt.panel_w.max(240.);
    let cols = layout(width);
    const GUTTER: f32 = 8.0;
    let fixed: f32 = cols.iter().filter_map(|c| c.1).sum::<f32>() + GUTTER * (cols.len() as f32 - 1.0) + 16.0;
    let mpn_chars = chars_in(width - fixed);
    let cell = |w: Option<f32>, right: bool| -> Div { let d = div().flex_shrink_0().overflow_hidden().whitespace_nowrap(); let d = match w { Some(x) => d.w(px(x)), None => d.flex_1().min_w_0() }; if right { d.flex().justify_end() } else { d } };
    let right = |k: Col| matches!(k, Col::Qty | Col::Stock | Col::Price);
    let mut head = div().flex_shrink_0().flex().items_center().gap(px(GUTTER)).px_2().h(px(22.)).border_b_1().border_color(c("line")).text_size(px(11.)).text_color(c("dim"));
    for (k, wd) in cols.iter().copied() {
        let (_, id, label) = COLS.iter().find(|x| x.0 == k).copied().unwrap();
        let mark = if col == k { if desc { " ↓" } else { " ↑" } } else { "" };
        head = head.child(cell(wd, right(k)).id(SharedString::from(format!("bom-h-{id}"))).cursor_pointer().hover(|s| s.text_color(c("text"))).child(format!("{}{mark}", trs(label)))
            .on_click(cx.listener(move |this, _, _, cx| { let (a, b) = press((this.screens.mgmt.bom.sort, this.screens.mgmt.bom.desc), k); this.screens.mgmt.bom.sort = a; this.screens.mgmt.bom.desc = b; cx.notify(); })));
    }
    let mut list = div().id("bom-rows").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().text_size(px(12.));
    if rows.is_empty() { list = list.child(div().p_3().text_color(c("faint")).child(if all.is_empty() { trs("No parts.") } else { trs("No line matches the filter.") })); }
    for l in rows {
        let id = s(&l, "id").to_string(); let dnp = l["dnp"] == true; let on = sel.as_deref() == Some(id.as_str());
        let refs: Vec<String> = l["refs"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
        let st = &l["stock"];
        let mut row = crate::ui::with_rect(div().id(SharedString::from(format!("bom-row-{id}"))), { let id = id.clone(); move || format!("mgmt:bom:row:{id}") })
            .flex_shrink_0().flex().items_center().gap(px(GUTTER)).px_2().h(px(24.)).border_b_1().border_color(c("line")).cursor_pointer()
            .when(on, |d| d.bg(c("raise"))).hover(|d| d.bg(c("panel"))).when(dnp, |d| d.text_color(c("faint")));
        for (k, wd) in cols.iter().copied() {
            let base = cell(wd, right(k));
            let chars = wd.map_or(mpn_chars, chars_in);
            let el: AnyElement = match k {
                Col::Qty => base.text_color(c("dim")).child(l["qty"].as_u64().unwrap_or(0).to_string()).into_any_element(),
                Col::Refs => { let t = refs_fit(&refs, chars); let full = refs.join(", "); base.id(SharedString::from(format!("bom-refs-{id}"))).when(t != full, |d| d.tooltip(crate::keys::tip(&full, "", light))).child(t).into_any_element() }
                Col::Value => base.child(mid_ellipsis(s(&l, "value"), chars)).into_any_element(),
                Col::Footprint => { let full = s(&l, "footprint").to_string(); let t = package(&full).unwrap_or_else(|| mid_ellipsis(short_fp(&full), chars));
                    base.id(SharedString::from(format!("bom-fp-{id}"))).text_color(c(if dnp { "faint" } else { "dim" })).when(!full.is_empty(), |d| d.tooltip(crate::keys::tip(&full, "", light))).child(t).into_any_element() }
                Col::Mpn => {
                    let mpn = s(&l, "mpn").to_string();
                    if !mpn.is_empty() { let t = mid_ellipsis(&mpn, chars); base.id(SharedString::from(format!("bom-mpn-{id}"))).when(t != mpn, |d| d.tooltip(crate::keys::tip(&mpn, "", light))).child(t).into_any_element() }
                    else if started && !dnp { base.text_color(c("warn")).child(trs("no MPN")).into_any_element() } else { base.text_color(c("faint")).child("—").into_any_element() }
                }
                // a DNP line is not ordered: its stock cell says DNP, in its own place
                Col::Stock if dnp => base.child(div().px(px(3.)).text_size(px(10.)).border_1().border_color(c("line")).child(trs("DNP"))).into_any_element(),
                Col::Stock => match (stock_words(st), stock_view(st)) {
                    (Some(t), Some((_, tone, tip))) => base.id(SharedString::from(format!("bom-stock-{id}"))).text_color(c(tone)).tooltip(crate::keys::tip(&tip, "", light)).child(t).into_any_element(),
                    _ => base.into_any_element() },
                Col::Price => base.text_color(c("dim")).child(st["price"].as_f64().map(money).unwrap_or_default()).into_any_element(),
                Col::Class => base.text_color(c(if st["class"] == "basic" { "dim" } else { "faint" })).child(st["class"].as_str().map(trs).unwrap_or_default()).into_any_element(),
            };
            row = row.child(el);
        }
        list = list.child(row.on_click({ let id = id.clone(); cx.listener(move |this, _, _, cx| select(this, &id, cx)) }));
    }
    if d["more"].as_u64().unwrap_or(0) > 0 { list = list.child(div().p_2().text_color(c("faint")).child(tf("gpui.mgmt.bom.more", &[("n", d["more"].as_u64().unwrap_or(0).into())]))); }
    let editor = sel.as_ref().and_then(|id| all.iter().find(|l| s(l, "id") == id.as_str())).map(|l| line_editor(app, l, light, cx));
    // docked at the bottom the panel is short and wide: the line's editor sits beside the list instead of under it
    if app.screens.mgmt.peek()["side"] == "bottom" {
        let left = div().flex_1().min_w_0().h_full().flex().flex_col().child(head).child(list);
        return div().size_full().flex().flex_col().child(top).child(div().flex_1().min_h_0().flex().flex_row().child(left)
            .children(editor.map(|e| div().id("bom-editor").w(px(380.)).flex_shrink_0().h_full().border_l_1().border_color(c("rule")).overflow_y_scroll().child(e)))).into_any_element()
    }
    div().size_full().flex().flex_col().child(top).child(head).child(list).children(editor).into_any_element()
}

/// The selected line's editor, under the list: its fields (Enter writes one), DNP, and where the last edit went.
fn line_editor(app: &mut Backplane, l: &Value, light: bool, cx: &mut Context<Backplane>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    if app.screens.mgmt.bom.fields.is_empty() {
        for (i, (_, _, ph)) in EDIT.iter().enumerate() {
            let f = cx.new(|cx| Composer::with_context(cx, "AskInput"));
            f.update(cx, |x, _| { x.set_placeholder(ph); x.max_lines = 2.0; });
            cx.subscribe(&f, move |this, e, ev: &ComposerEvent, cx| if matches!(ev, ComposerEvent::Submit) { let v = e.read(cx).content.to_string(); edit(this, EDIT[i].0, v.trim(), cx) }).detach();
            app.screens.mgmt.bom.fields.push(f);
        }
    }
    let id = s(l, "id").to_string();
    // the fields hold the line as it is: filled when another line is picked, or the hub's data moved under an idle field
    let fresh = app.screens.mgmt.bom.filled != id;
    if fresh { app.screens.mgmt.bom.filled_vals = vec![String::new(); EDIT.len()]; }
    for (i, (k, _, _)) in EDIT.iter().enumerate() {
        let f = app.screens.mgmt.bom.fields[i].clone(); let want = edit_value(l, k);
        let had = app.screens.mgmt.bom.filled_vals[i].clone(); let now = f.read(cx).content.to_string();
        // a field nobody is typing in follows the design (an edit here, in KiCad or by an agent)
        if fresh || (want != had && now == had) { f.update(cx, |x, cx| x.set(want.clone(), cx)); app.screens.mgmt.bom.filled_vals[i] = want; }
    }
    if fresh { app.screens.mgmt.bom.filled = id.clone(); app.screens.mgmt.bom.said = None; }
    let dnp = l["dnp"] == true;
    let mut rows = div().flex_shrink_0().flex().flex_col().gap_1().px_2().py_2().border_t_1().border_color(c("rule")).bg(c("panel")).text_size(px(11.));
    rows = rows.child(div().flex().items_center().gap_2().child(div().text_color(c("text")).child(refs_text(l))).child(div().flex_1())
        .child(crate::ui::with_rect(div().id("bom-dnp"), || "mgmt:bom:dnp".to_string()).px_2().h(px(20.)).flex().items_center().border_1().border_color(c(if dnp { "warn" } else { "line" })).bg(c("raise")).cursor_pointer()
            .text_color(c(if dnp { "warn" } else { "dim" })).hover(|d| d.bg(c("bg"))).tooltip(crate::keys::tip(&trs("Do not place these parts"), "", light)).child(trs("DNP"))
            .on_click(cx.listener(move |this, _, _, cx| edit(this, "dnp", if dnp { "no" } else { "yes" }, cx)))));
    for (i, (_, label, _)) in EDIT.iter().enumerate() {
        rows = rows.child(div().flex().items_center().gap_2().child(div().w(px(64.)).flex_shrink_0().text_color(c("dim")).child(trs(label)))
            .child(div().flex_1().min_w_0().px_2().py(px(2.)).border_1().border_color(c("line")).bg(c("bg")).text_size(px(12.)).child(app.screens.mgmt.bom.fields[i].clone())));
    }
    let b = &app.screens.mgmt.bom;
    let note = if b.sending { Some(("dim", trs("Writing…"), false)) } else { edit_note(b.said.as_ref(), b.last.as_ref()) };
    rows = rows.children(note.map(|(tone, t, offer)| div().flex().items_center().gap_2().child(div().flex_1().min_w_0().text_color(c(tone)).child(t))
        .when(offer, |d| d.child(crate::ui::with_rect(div().id("bom-undo"), || "mgmt:bom:undo".to_string()).flex_shrink_0().px_2().h(px(20.)).flex().items_center().border_1().border_color(c("line")).bg(c("raise")).cursor_pointer().hover(|d| d.bg(c("bg")))
            .tooltip(crate::keys::tip(&trs("Put the old value back"), "", light)).child(trs("Undo")).on_click(cx.listener(|this, _, _, cx| undo(this, cx)))))));
    // the fab's stock for the line, and parts that could stand in for it
    let st = &l["stock"];
    let fab = st["fab"].as_str().filter(|f| !f.is_empty()).unwrap_or(s(l, "fab")).to_string();
    if let Some((t, tone, tip)) = stock_view(st) {
        let id = s(l, "id").to_string();
        rows = rows.child(div().flex().items_center().gap_2().child(div().w(px(64.)).flex_shrink_0().text_color(c("dim")).child(trs("Stock")))
            .child(div().flex_1().min_w_0().text_color(c(tone)).overflow_hidden().text_ellipsis().whitespace_nowrap().child(tip.clone()))
            .when(!fab.is_empty(), |d| d.child(crate::ui::with_rect(div().id("bom-alts"), || "mgmt:bom:alts".to_string()).flex_shrink_0().px_2().h(px(20.)).flex().items_center().border_1().border_color(c("line")).bg(c("raise")).cursor_pointer().hover(|d| d.bg(c("bg")))
                .child(trs("Alternatives")).on_click({ let (fab, id) = (fab.clone(), id.clone()); let need = st["need"].as_u64().unwrap_or(1); cx.listener(move |this, _, _, cx| ask_alts(this, &id, &fab, need, cx)) }))));
    }
    let id = s(l, "id").to_string();
    if let Some((_, a)) = app.screens.mgmt.bom.alts.clone().filter(|(i, _)| *i == id) {
        if a.is_null() { rows = rows.child(div().text_color(c("faint")).child(trs("Looking for alternatives…"))) }
        else if let Some(e) = a["error"].as_str() { rows = rows.child(div().text_color(c("bad")).child(e.to_string())) }
        else {
            let list: Vec<Value> = a["alternatives"].as_array().cloned().unwrap_or_default();
            if list.is_empty() { rows = rows.child(div().text_color(c("faint")).child(trs("No alternatives in the fab's stock."))) }
            for (i, x) in list.iter().enumerate() {
                let (xf, xm) = (s(x, "fab").to_string(), s(x, "model").to_string());
                rows = rows.child(div().id(SharedString::from(format!("bom-alt-{i}"))).flex().items_center().gap_2().px_1().cursor_pointer().hover(|d| d.bg(c("raise")))
                    .tooltip(crate::keys::tip(&trs("Use this part: its MPN and fab part number go on the line"), "", light))
                    .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().child(format!("{xm} · {}", s(x, "mfr"))))
                    .child(div().flex_shrink_0().text_color(c("dim")).child(s(x, "class").to_string()))
                    .child(div().w(px(44.)).flex().justify_end().flex_shrink_0().text_color(c("dim")).child(abbrev(x["stock"].as_i64().unwrap_or(-1))))
                    .on_click(cx.listener(move |this, _, _, cx| { edit(this, "mpn", &xm, cx); edit(this, "fab", &xf, cx); this.screens.mgmt.bom.alts = None; })));
            }
        }
    }
    rows.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn l(id: &str, refs: &[&str], qty: u64, v: &str, fp: &str, mpn: &str, dnp: bool) -> Value { json!({"id": id, "refs": refs, "qty": qty, "value": v, "footprint": fp, "mpn": mpn, "mfr": "", "fab": "", "dnp": dnp}) }
    fn ids(v: &[Value]) -> Vec<&str> { v.iter().map(|l| l["id"].as_str().unwrap()).collect() }
    fn sample() -> Vec<Value> { vec![l("C1", &["C1", "C2"], 2, "100n", "Capacitor_SMD:C_0402", "GRM155", false), l("R1", &["R1"], 1, "10k", "Resistor_SMD:R_0603", "", false), l("U1", &["U1"], 1, "nRF54L15", "Package_DFN_QFN:QFN-48", "NRF54L15-QFAA-R", false), l("R9", &["R9"], 1, "0R", "Resistor_SMD:R_0402", "", true)] }
    #[test] fn sorting_by_each_column() {
        let v = sample();
        assert_eq!(ids(&sorted(&v, Col::Refs, false)), ["C1", "R1", "U1", "R9"]);
        assert_eq!(ids(&sorted(&v, Col::Refs, true)), ["R9", "U1", "R1", "C1"]);
        assert_eq!(ids(&sorted(&v, Col::Qty, true)), ["C1", "R9", "U1", "R1"]);
        assert_eq!(ids(&sorted(&v, Col::Value, false)), ["R9", "C1", "R1", "U1"]);
        assert_eq!(ids(&sorted(&v, Col::Footprint, false)), ["C1", "U1", "R9", "R1"]);
        assert_eq!(ids(&sorted(&v, Col::Mpn, false)), ["R1", "R9", "C1", "U1"]);
        assert_eq!(press((Col::Refs, false), Col::Refs), (Col::Refs, true)); assert_eq!(press((Col::Refs, true), Col::Mpn), (Col::Mpn, false));
    }
    #[test] fn filtering_by_any_field_and_dnp() {
        let v = sample();
        let f = |q: &str| ids(&v.iter().filter(|x| matches(x, q)).cloned().collect::<Vec<_>>()).iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(f(""), ["C1", "R1", "U1", "R9"]); assert_eq!(f("c2"), ["C1"]); assert_eq!(f("0603"), ["R1"]); assert_eq!(f("qfaa"), ["U1"]);
        assert_eq!(f("dnp"), ["R9"]); assert_eq!(f("resistor 0402"), ["R9"]); assert!(f("zzz").is_empty());
    }
    #[test] fn a_lines_edited_values() {
        let v = sample(); assert_eq!(edit_value(&v[3], "dnp"), "yes"); assert_eq!(edit_value(&v[0], "dnp"), "no"); assert_eq!(edit_value(&v[0], "mpn"), "GRM155"); assert_eq!(edit_value(&v[1], "fab"), "");
        assert_eq!(EDIT.iter().map(|e| e.0).collect::<Vec<_>>(), ["mpn", "fab", "value", "footprint"]);
    }
    #[test] fn stock_cells() {
        assert_eq!(money(1.2), "$1.20"); assert_eq!(money(0.5), "$0.50"); assert_eq!(money(0.0029), "$0.0029"); assert_eq!(money(0.01), "$0.01"); assert_eq!(money(0.012), "$0.012");
        assert_eq!(abbrev(1060770), "1.1M"); assert_eq!(abbrev(12400), "12k"); assert_eq!(abbrev(1500), "1.5k"); assert_eq!(abbrev(320), "320"); assert_eq!(abbrev(-1), "?");
        let (t, tone, tip) = stock_view(&json!({"status": "out", "stock": 3, "need": 10, "class": "basic", "price": 0.2})).unwrap();
        assert_eq!((t.as_str(), tone), ("3", "bad")); assert!(tip.contains("10") && tip.contains("$0.2"), "{tip}");
        assert_eq!(stock_view(&json!({"status": "checking"})).unwrap().0, "…"); assert_eq!(stock_view(&Value::Null), None);
        let mut v = sample(); v[0]["stock"] = json!({"status": "in"}); v[1]["stock"] = json!({"status": "out"});
        assert_eq!(ids(&sorted(&v, Col::Stock, false))[..2], ["R1", "C1"]);
        assert_eq!(cost_text(&json!({"priced": 2, "lookups": 2, "cost": 1.234, "checking": 0})).unwrap(), "≈ $1.23 a board");
        assert_eq!(cost_text(&json!({"priced": 1, "lookups": 2, "cost": 1.0, "checking": 0})).unwrap(), "at least $1.00 a board");
        assert_eq!(cost_text(&json!({"priced": 1, "lookups": 2, "cost": 1.0, "checking": 1})), None);
    }
    #[test] fn after_an_edit_ctrl_z_is_promised_only_where_kicad_takes_it_back() {
        let ok = |w: &str| (true, w.to_string());
        let k = json!({"undo": "kicad", "changed": 2}); let p = json!({"undo": "panel", "changed": 2});
        let kn = edit_note(Some(&ok("kicad")), Some(&k)).unwrap(); assert!(kn.1.contains("Ctrl+Z") && kn.2, "Ctrl+Z said, and the panel's Undo kept");
        let n = edit_note(Some(&ok("kicad")), Some(&p)).unwrap(); assert!(n.2 && !n.1.contains("Ctrl+Z"), "{}", n.1);
        assert!(edit_note(Some(&ok("file")), Some(&p)).unwrap().2);
        assert!(!edit_note(Some(&ok("undone")), None).unwrap().2);
        assert_eq!(edit_note(Some(&(false, "locked".into())), None).unwrap(), ("bad", "locked".to_string(), false));
        assert!(!edit_note(Some(&ok("file")), Some(&json!({"undo": "panel", "changed": 0}))).unwrap().2); assert_eq!(edit_note(None, None), None);
    }
    #[test] fn columns_fit_the_width_and_the_mpn_shows_whole_or_shortened_in_its_middle() {
        let ids = |w: f32| layout(w).iter().map(|c| c.0).collect::<Vec<_>>();
        assert_eq!(ids(400.), [Col::Qty, Col::Refs, Col::Value, Col::Mpn, Col::Stock]);
        assert_eq!(ids(560.), [Col::Qty, Col::Refs, Col::Value, Col::Mpn, Col::Stock, Col::Price]);
        assert_eq!(ids(800.), [Col::Qty, Col::Refs, Col::Value, Col::Footprint, Col::Mpn, Col::Stock, Col::Price, Col::Class]);
        assert_eq!(mid_ellipsis("GRM033R60J225ME47D", 12), "GRM033…ME47D"); assert_eq!(mid_ellipsis("CL05A475MP5NRNC", 15), "CL05A475MP5NRNC"); assert_eq!(mid_ellipsis("GRM033R60J225ME47D", 12).chars().count(), 12);
        let r = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(refs_fit(&r(&["C2", "C4", "C37", "C40"]), 14), "C2, C4, C37 +1"); assert_eq!(refs_fit(&r(&["C1", "C36"]), 14), "C1, C36");
        assert!(refs_fit(&r(&["C12", "C32", "C33", "C34", "C35", "C40", "C41"]), 14).chars().count() <= 14);
        assert_eq!(stock_words(&json!({"status": "low", "stock": 12})).unwrap(), "low 12"); assert_eq!(stock_words(&json!({"status": "out", "stock": 0})).unwrap(), "out 0"); assert_eq!(stock_words(&json!({"status": "in", "stock": 717000})).unwrap(), "717k");
    }
    #[test] fn packages_as_people_say_them() {
        assert_eq!(package("Capacitor_SMD:C_0402_1005Metric").as_deref(), Some("0402")); assert_eq!(package("Resistor_SMD:R_0201_0603Metric").as_deref(), Some("0201"));
        assert_eq!(package("Package_DFN_QFN:QFN-48-1EP_6x6mm_P0.4mm").as_deref(), Some("QFN-48")); assert_eq!(package("Package_TO_SOT_SMD:SOT-23-5").as_deref(), Some("SOT-23"));
        assert_eq!(package("Connector_JST:JST_PH_S2B-PH-SM4-TB_1x02-1MP_P2.00mm_Horizontal"), None); assert_eq!(package(""), None);
    }
    #[test] fn short_names() { assert_eq!(short_fp("Resistor_SMD:R_0603"), "R_0603"); assert_eq!(short_fp("R_0603"), "R_0603"); assert_eq!(refs_text(&sample()[0]), "C1, C2"); }
}
