//! The BOM panel (screens/mgmt.rs hosts it; the hub's kicad/bompanel.rs publishes `mgmt.bom.<digest>`): the design's BOM lines, sortable by a
//! header press, filtered by what is typed, DNP lines dimmed. A press on a line selects its parts in the open KiCad and lights them in the
//! viewers (`editor.kicad.select`). Pure rules first (sorting, filtering, the footprint's short name), tested.
use super::composer::Composer;
use crate::{hub::Command, i18n::{tf, trs}, screens::settings::Secret, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Col { #[default] Refs, Qty, Value, Footprint, Mpn }
pub const COLS: &[(Col, &str, &str)] = &[(Col::Refs, "refs", "Refs"), (Col::Qty, "qty", "Qty"), (Col::Value, "value", "Value"), (Col::Footprint, "footprint", "Footprint"), (Col::Mpn, "mpn", "MPN")]; // i18n-ok: ids; labels go through trs

#[derive(Default)]
pub struct Ui { pub sort: Col, pub desc: bool, pub filter: Option<Entity<Composer>>, pub sel: Option<String> }

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
    let key = |l: &Value| -> String { match col { Col::Refs | Col::Qty => String::new(), Col::Value => s(l, "value").to_lowercase(), Col::Footprint => short_fp(s(l, "footprint")).to_lowercase(), Col::Mpn => s(l, "mpn").to_lowercase() } };
    v.sort_by(|(i, a), (j, b)| {
        let o = match col { Col::Refs => i.cmp(j), Col::Qty => a["qty"].as_u64().cmp(&b["qty"].as_u64()).then(i.cmp(j)), _ => key(a).cmp(&key(b)).then(i.cmp(j)) };
        if desc { o.reverse() } else { o }
    });
    v.into_iter().map(|(_, l)| l.clone()).collect()
}
/// What a header press does: the same column turns its order round, another sorts by it ascending.
pub fn press(cur: (Col, bool), col: Col) -> (Col, bool) { if cur.0 == col { (col, !cur.1) } else { (col, false) } }
/// The source line: where the parts come from.
pub fn source_text(d: &Value) -> String { if d["source"] == "kicad" { trs("live from KiCad") } else { trs("from the saved schematic") } }

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
pub fn act(app: &mut Backplane, a: &str, cx: &mut Context<Backplane>) {
    if let Some(id) = a.strip_prefix("row:") { select(app, id, cx) }
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
        .child(div().flex_shrink_0().text_color(c("faint")).child(source_text(d)))
        .child(div().flex_1().min_w(px(80.)).px_2().py(px(2.)).border_1().border_color(c("line")).bg(c("panel")).text_size(px(12.)).children(app.screens.mgmt.bom.filter.clone()));
    // the header: a press sorts by that column
    let w = |col: Col| -> Div { let d = div().flex_shrink_0().overflow_hidden().whitespace_nowrap().text_ellipsis(); match col { Col::Qty => d.w(px(30.)), Col::Refs => d.w(relative(0.27)), Col::Value => d.w(relative(0.18)), Col::Footprint => d.w(relative(0.22)), Col::Mpn => d.flex_1().min_w_0() } };
    let order = [Col::Qty, Col::Refs, Col::Value, Col::Footprint, Col::Mpn];
    let mut head = div().flex_shrink_0().flex().items_center().gap_2().px_2().h(px(22.)).border_b_1().border_color(c("line")).text_size(px(11.)).text_color(c("dim"));
    for k in order {
        let (_, id, label) = COLS.iter().find(|x| x.0 == k).copied().unwrap();
        let mark = if col == k { if desc { " ↓" } else { " ↑" } } else { "" };
        head = head.child(w(k).id(SharedString::from(format!("bom-h-{id}"))).cursor_pointer().hover(|s| s.text_color(c("text"))).child(format!("{}{mark}", trs(label)))
            .on_click(cx.listener(move |this, _, _, cx| { let (a, b) = press((this.screens.mgmt.bom.sort, this.screens.mgmt.bom.desc), k); this.screens.mgmt.bom.sort = a; this.screens.mgmt.bom.desc = b; cx.notify(); })));
    }
    let mut list = div().id("bom-rows").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().text_size(px(12.));
    if rows.is_empty() { list = list.child(div().p_3().text_color(c("faint")).child(if all.is_empty() { trs("No parts.") } else { trs("No line matches the filter.") })); }
    for l in rows {
        let id = s(&l, "id").to_string(); let dnp = l["dnp"] == true; let on = sel.as_deref() == Some(id.as_str());
        let mpn = s(&l, "mpn").to_string();
        let mpn_cell = if !mpn.is_empty() { div().child(mpn) } else if started && !dnp { div().text_color(c("warn")).child(trs("no MPN")) } else { div().text_color(c("faint")).child("—") };
        let row = crate::ui::with_rect(div().id(SharedString::from(format!("bom-row-{id}"))), { let id = id.clone(); move || format!("mgmt:bom:row:{id}") })
            .flex_shrink_0().flex().items_center().gap_2().px_2().h(px(24.)).border_b_1().border_color(c("line")).cursor_pointer()
            .when(on, |d| d.bg(c("raise"))).hover(|d| d.bg(c("panel"))).when(dnp, |d| d.text_color(c("faint")))
            .child(w(Col::Qty).text_color(c("dim")).child(l["qty"].as_u64().unwrap_or(0).to_string()))
            .child(w(Col::Refs).child(refs_text(&l)))
            .child(w(Col::Value).child(s(&l, "value").to_string()))
            .child(w(Col::Footprint).text_color(c(if dnp { "faint" } else { "dim" })).child(short_fp(s(&l, "footprint")).to_string()))
            .child(w(Col::Mpn).flex().items_center().gap_1().child(mpn_cell.overflow_hidden().text_ellipsis()).when(dnp, |d| d.child(div().flex_shrink_0().px(px(3.)).text_size(px(10.)).border_1().border_color(c("line")).child(trs("DNP")))))
            .on_click({ let id = id.clone(); cx.listener(move |this, _, _, cx| select(this, &id, cx)) });
        list = list.child(row);
    }
    if d["more"].as_u64().unwrap_or(0) > 0 { list = list.child(div().p_2().text_color(c("faint")).child(tf("gpui.mgmt.bom.more", &[("n", d["more"].as_u64().unwrap_or(0).into())]))); }
    div().size_full().flex().flex_col().child(top).child(head).child(list).into_any_element()
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
    #[test] fn short_names() { assert_eq!(short_fp("Resistor_SMD:R_0603"), "R_0603"); assert_eq!(short_fp("R_0603"), "R_0603"); assert_eq!(refs_text(&sample()[0]), "C1, C2"); }
}
