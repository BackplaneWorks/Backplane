//! The Checks panel (screens/mgmt.rs hosts it; the hub's kicad/checkrun.rs publishes `mgmt.checks.<digest>`): DRC and ERC findings grouped by
//! rule, worst first, filtered by severity and words, exclusions dimmed. A press on a row (or up/down through them) selects and zooms its items in
//! the open KiCad, in the editor it belongs to (the board for DRC, the schematic for ERC: `kicad.checks.focus`), and lights them in the viewers;
//! "Fix" sends the finding, with everything an agent needs, to the thread. Run runs both checks now. Pure rules first, tested.
use super::composer::Composer;
use crate::{hub::Command, i18n::{tf, trs}, screens::settings::Secret, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};
use std::{cell::RefCell, collections::{BTreeMap, BTreeSet}};

/// The rows a shut group shows (the rest behind its "n more": a board's hundreds of missing connections stay one short group).
pub const SHUT_ROWS: usize = 3;
/// Answer tags (Command::Ask): a search, a group's rest, a fix's message.
pub const LIST_TAG: u64 = 0x6368_6b31;
pub const MORE_TAG: u64 = 0x6368_6b32;
pub const FIX_TAG: u64 = 0x6368_6b33;
/// The answer that finds the group of a finding a KiCad marker click named (`review.clicked.<digest>`).
pub const WANT_TAG: u64 = 0x6368_6b34;

#[derive(Default)]
pub struct Ui {
    pub filter: Option<Entity<Composer>>,
    pub focus: Option<FocusHandle>,
    /// "" all, or "error" / "warning" / "excluded"
    pub sev: String,
    /// "" every source, else one ("drc", "erc", or another's id: the design review's "fab", "bom"...)
    pub src: String,
    pub sel: Option<String>,
    /// the hub's answer to the search now shown (severity and words, as asked) and the rows of groups opened past their first rows
    pub found: Option<(String, String, Vec<Value>)>,
    pub asked: Option<(String, String)>,
    pub more: BTreeMap<String, Vec<Value>>,
    pub open: BTreeSet<String>,
    /// the list as drawn (only its rows in view are built), and what drawing an item needs
    pub items: Vec<Item>,
    pub scroll: Option<UniformListScrollHandle>,
    pub light: bool,
    pub sources: Value,
    /// a finding to show (a click on its marker in KiCad): its id, and whether its group was asked for
    pub want: Option<(String, bool)>,
}
/// One line of the list: a group's heading, a finding, a group's "n more" / "Fewer", or a word (searching, nothing found...).
#[derive(Clone, Debug, PartialEq)]
pub enum Item { Head(Value), Row(Value), More(Value, u64), Msg(&'static str) }
thread_local! { static UI: RefCell<Ui> = RefCell::new(Ui::default()); }
fn ui<R>(f: impl FnOnce(&mut Ui) -> R) -> R { UI.with(|u| f(&mut u.borrow_mut())) }

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
/// A rule's id as a heading: "courtyards_overlap" -> "Courtyards overlap".
pub fn rule_label(r: &str) -> String { let t = r.replace('_', " "); let mut c = t.chars(); match c.next() { Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), None => String::new() } }
/// A group's key: its check, rule and whether it holds exclusions.
pub fn group_key(g: &Value) -> String { format!("{}:{}:{}", s(g, "source"), s(g, "rule"), g["excluded"] == true) }
/// The colour name of a severity's mark.
pub fn tone(sev: &str) -> &'static str { match sev { "error" => "bad", "warning" => "warn", "info" => "dim", _ => "faint" } }
/// Whether the hub must be asked (a search): a severity or words are set.
pub fn searching(sev: &str, q: &str) -> bool { !sev.is_empty() || !q.trim().is_empty() }
/// The rows shown, in order: the search's when searching, else each group's (its published rows, all of them once opened and fetched).
pub fn visible(d: &Value, search: Option<&[Value]>, more: &BTreeMap<String, Vec<Value>>, open: &BTreeSet<String>) -> Vec<Value> {
    if let Some(rows) = search { return rows.to_vec() }
    let mut out = vec![];
    for g in d["groups"].as_array().into_iter().flatten() {
        let k = group_key(g);
        let shut: Vec<Value> = g["rows"].as_array().cloned().unwrap_or_default();
        match (open.contains(&k), more.get(&k)) { (true, Some(all)) => out.extend(all.iter().cloned()), (true, None) => out.extend(shut), _ => out.extend(shut.into_iter().take(SHUT_ROWS)) }
    }
    out
}
/// The list's lines: a search's rows (or "Looking…" / "Nothing matches"), else each group's heading, its rows (all of them once opened and
/// fetched) and its "n more" (0: "Fewer"); "No findings" (or "Running the checks…" before a first result) when there are none.
pub fn items(d: &Value, search: Option<&[Value]>, waiting: bool, more: &BTreeMap<String, Vec<Value>>, open: &BTreeSet<String>) -> Vec<Item> {
    if waiting { return vec![Item::Msg("gpui.mgmt.checks.searching")] }
    if let Some(rows) = search { return if rows.is_empty() { vec![Item::Msg("gpui.mgmt.checks.nomatch")] } else { rows.iter().cloned().map(Item::Row).collect() } }
    let mut out = vec![];
    for g in d["groups"].as_array().into_iter().flatten() {
        let k = group_key(g); let n = g["n"].as_u64().unwrap_or(0); let is_open = open.contains(&k);
        let published: Vec<Value> = g["rows"].as_array().cloned().unwrap_or_default();
        let rows: Vec<Value> = match (is_open, more.get(&k)) { (true, Some(all)) => all.clone(), (true, None) => published, _ => published.into_iter().take(SHUT_ROWS).collect() };
        let shown = rows.len() as u64;
        out.push(Item::Head(g.clone()));
        out.extend(rows.into_iter().map(Item::Row));
        if n > shown { out.push(Item::More(g.clone(), n - shown)) } else if is_open { out.push(Item::More(g.clone(), 0)) }
    }
    if out.is_empty() { out.push(Item::Msg(if d["drc"]["has_result"] == true || d["erc"]["has_result"] == true { "gpui.mgmt.checks.none" } else { "gpui.mgmt.checks.reading" })) }
    out
}
/// The panel's data with only `src`'s groups ("" all).
pub fn by_source(d: &Value, src: &str) -> Value {
    if src.is_empty() { return d.clone() }
    let mut o = d.clone();
    o["groups"] = Value::Array(d["groups"].as_array().into_iter().flatten().filter(|g| s(g, "source") == src).cloned().collect());
    o
}
/// One item of the list, drawn (uniform_list asks only for the ones in view).
fn item_el(app: &mut Backplane, i: usize, cx: &mut Context<Backplane>) -> AnyElement {
    let (it, light, srcs, sel) = ui(|u| (u.items.get(i).cloned(), u.light, u.sources.clone(), u.sel.clone()));
    let c = |n: &str| theme::color(light, n);
    let d = json!({"sources": srcs});
    let _ = app;
    match it {
        None => div().w_full().h(crate::scale::tpx(24.)).into_any_element(),
        Some(Item::Msg(k)) => div().w_full().h(crate::scale::tpx(24.)).px_3().flex().items_center().text_color(c("faint")).child(tf(k, &[])).into_any_element(),
        Some(Item::Head(g)) => {
            let ex = g["excluded"] == true; let n = g["n"].as_u64().unwrap_or(0);
            let src = source_label(&d, s(&g, "source")); let jd = judged(&d, s(&g, "source"));
            div().w_full().h(crate::scale::tpx(24.)).flex().items_center().gap_2().px_2().border_b_1().border_color(c("line")).bg(c("panel")).text_size(crate::scale::tpx(11.))
                .child(div().flex_shrink_0().text_color(c("faint")).child(src))
                .when(jd, |x| x.child(div().flex_shrink_0().px(px(3.)).text_size(crate::scale::tpx(10.)).border_1().border_color(c("warn")).text_color(c("warn")).child(tf("gpui.mgmt.checks.judged", &[]))))
                .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(c(if ex { "faint" } else { "dim" }))
                    .child(if ex { format!("{} · {}", rule_label(s(&g, "rule")), tf("gpui.mgmt.checks.excluded", &[])) } else { rule_label(s(&g, "rule")) })) // i18n-ok: a join
                .child(div().flex_shrink_0().text_color(c(if ex { "faint" } else { tone(s(&g, "severity")) })).child(n.to_string()))
                .into_any_element()
        }
        Some(Item::More(g, left)) => {
            let k = group_key(&g);
            let label = if left == 0 { tf("gpui.mgmt.checks.fewer", &[]) } else { tf("gpui.mgmt.checks.more", &[("n", left.into())]) };
            crate::ui::with_rect(div().id(SharedString::from(format!("checks-more-{k}"))), { let k = k.clone(); move || format!("mgmt:checks:more:{k}") })
                .w_full().h(crate::scale::tpx(24.)).pl(px(14.)).flex().items_center().text_size(crate::scale::tpx(11.)).text_color(c("dim")).cursor_pointer().hover(|x| x.text_color(c("text")))
                .child(label).on_click(cx.listener(move |this, _, _, cx| open_group(this, &g, cx))).into_any_element()
        }
        Some(Item::Row(f)) => {
            let id = s(&f, "id").to_string(); let on = sel.as_deref() == Some(id.as_str()); let ex = s(&f, "severity") == "excluded";
            crate::ui::with_rect(div().id(SharedString::from(format!("checks-row-{id}"))), { let id = id.clone(); move || format!("mgmt:checks:row:{id}") })
                .group("checks-row").w_full().flex().items_center().gap_2().pl(px(14.)).pr_2().h(crate::scale::tpx(24.)).border_b_1().border_color(c("line")).cursor_pointer()
                .relative().when(on, |x| x.bg(c("raise")).child(div().absolute().left_0().top_0().bottom_0().w(px(2.)).bg(c("accent")))).hover(|x| x.bg(c("panel"))).when(ex, |x| x.text_color(c("faint")))
                .child(div().flex_shrink_0().size(px(7.)).bg(c(tone(s(&f, "severity")))))
                .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(s(&f, "title").to_string()))
                .when(judged(&d, s(&f, "source")), |x| x.child(div().flex_shrink_0().px(px(3.)).text_size(crate::scale::tpx(10.)).border_1().border_color(c("warn")).text_color(c("warn")).child(tf("gpui.mgmt.checks.judged", &[]))))
                .child(div().flex_shrink_0().max_w(relative(0.4)).overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(c(if ex { "faint" } else { "dim" })).child(s(&f, "names").to_string()))
                .child(crate::ui::with_rect(div().id(SharedString::from(format!("checks-fix-{id}"))), { let id = id.clone(); move || format!("mgmt:checks:fix:{id}") })
                    // over the row's right end, not beside it: hidden, it takes no room from the title
                    .absolute().right(px(6.)).top(px(3.)).px(px(5.)).border_1().border_color(c("line")).bg(c("raise")).text_size(crate::scale::tpx(11.)).text_color(c("dim")).invisible().group_hover("checks-row", |x| x.visible()).when(on, |x| x.visible())
                    .hover(|x| x.text_color(c("text")).bg(c("bg"))).child(tf(if s(&f, "fix_kind") == "reexport" { "gpui.mgmt.checks.reexport" } else { "gpui.mgmt.checks.fix" }, &[]))
                    .on_click({ let id = id.clone(); cx.listener(move |this, _, _, cx| { cx.stop_propagation(); fix(this, &id, cx) }) }))
                .on_click({ let id = id.clone(); cx.listener(move |this, _, window, cx| { if let Some(h) = ui(|u| u.focus.clone()) { window.focus(&h); } select(this, &id, cx) }) })
                .into_any_element()
        }
    }
}
/// Up (-1) or down (+1) from the selected row: the next row's id (the first when none is selected); None past either end.
pub fn step(ids: &[String], sel: Option<&str>, by: i32) -> Option<String> {
    if ids.is_empty() { return None }
    let Some(i) = sel.and_then(|x| ids.iter().position(|y| y == x)) else { return ids.first().cloned() };
    let j = i as i32 + by;
    (j >= 0 && (j as usize) < ids.len()).then(|| ids[j as usize].clone())
}
/// One check's state line: "DRC 555", with what is true of it now (running, changed since, waiting for memory, failed).
pub fn state_text(name: &str, c: &Value) -> (String, &'static str) {
    if c.is_null() { return (String::new(), "faint") }
    let n = ["error", "warning", "info"].iter().map(|k| c["counts"][*k].as_u64().unwrap_or(0)).sum::<u64>();
    let head = if c["has_result"] == true { format!("{name} {}", if n == 0 { "✓".to_string() } else { n.to_string() }) } else { format!("{name} …") }; // i18n-ok: DRC/ERC are the checks' names; numbers and joins
    let out = match s(c, "state") {
        "running" if !s(c, "behind").is_empty() => (format!("{head} · {}", tf("gpui.mgmt.checks.behind", &[("what", s(c, "behind").to_string().into())])), "dim"), // i18n-ok: DRC/ERC are the checks' names; numbers and joins
        "running" => (format!("{head} · {}", tf("gpui.mgmt.checks.running", &[])), "dim"), // i18n-ok: DRC/ERC are the checks' names; numbers and joins
        "stale" => (format!("{head} · {}", tf("gpui.mgmt.checks.stale", &[])), "warn"), // i18n-ok: DRC/ERC are the checks' names; numbers and joins
        "deferred" => (format!("{head} · {}", tf("gpui.mgmt.checks.waiting", &[("why", s(c, "reason").to_string().into())])), "warn"), // i18n-ok: DRC/ERC are the checks' names; numbers and joins
        "failed" => (format!("{head} · {}", tf("gpui.mgmt.checks.failed", &[("why", s(c, "reason").to_string().into())])), "bad"), // i18n-ok: DRC/ERC are the checks' names; numbers and joins
        _ => (head, if c["counts"]["error"].as_u64().unwrap_or(0) > 0 { "bad" } else if n > 0 { "warn" } else { "dim" }),
    };
    // a result of the open editor's unsaved edits says so (bp.13 embed-checks: the check ran in KiCad, not on the saved files)
    let out = if c["unsaved"] == true && s(c, "state") != "running" { (format!("{} · {}", out.0, tf("gpui.mgmt.checks.liveunsaved", &[])), out.1) } else { out }; // i18n-ok: a join
    // a result made on another of the person's machines says so
    let on = s(c, "ran_on");
    if on.is_empty() || s(c, "state") == "running" { out } else { (format!("{} · {}", out.0, tf("gpui.mgmt.checks.on", &[("host", on.to_string().into())])), out.1) } // i18n-ok: a join
}

fn project(app: &Backplane) -> String { app.screens.mgmt.project.clone() }
fn filter_text(cx: &App) -> String { ui(|u| u.filter.clone()).map_or(String::new(), |f| f.read(cx).content.to_string()) }
fn log(ev: &str, v: Value) { let mut o = v; o["event"] = json!(ev); eprintln!("GPUI_LOG {o}"); }

/// A row pressed (or reached with up/down): its items selected and zoomed in KiCad's right editor, lit in the viewers.
pub fn select(app: &mut Backplane, id: &str, cx: &mut Context<Backplane>) {
    ui(|u| u.sel = Some(id.to_string()));
    log("checks_select", json!({"id": id}));
    // a file's finding (Gerbers vs board): that comparison on its layer; a difference (a place) lit and zoomed on, the whole (a stale
    // export) as it is
    let row = ui(|u| u.items.iter().find_map(|i| if let Item::Row(f) = i { (s(f, "id") == id).then(|| f.clone()) } else { None }));
    if let Some(f) = row.filter(|f| s(f, "doc") == "file") {
        let viewer = app.data["thread"]["viewer"].clone();
        if viewer["gerbersVsBoard"] != true { let _ = app.tx.try_send(Command::Action("view".into(), "gerbers\tboard".into())); }
        let layer = s(&f, "layer").to_string();
        if let Some(r) = viewer["layerList"].as_array().into_iter().flatten().find(|r| s(r, "name") == layer || s(r, "name").starts_with(&format!("{layer} ·"))) { // i18n-ok: matches the hub's row name
            if r["active"] != true { let _ = app.tx.try_send(Command::Action("view-layer-pick".into(), r["layer"].as_u64().unwrap_or(0).to_string())); }
        }
        if s(&f, "at").is_empty() { let _ = app.tx.try_send(Command::Action("view-probe".into(), String::new())); app.ui_epoch += 1; cx.notify(); return }
        let probe = format!("difference;at={}", s(&f, "at")); // i18n-ok: a probe's text, not drawn
        let _ = app.tx.try_send(Command::Action("view-probe".into(), probe.clone()));
        // zoomed on once the page shows that probe (xprobe::zoom_pending)
        app.screens.xprobe.zoom = Some((probe, std::time::Instant::now()));
        app.ui_epoch += 1; cx.notify(); return
    }
    let _ = app.tx.try_send(Command::Rpc("kicad.checks.focus".into(), Secret(json!({"project": project(app), "id": id})), "xsel"));
    if app.screens.kicad.is_none() { app.screens.xprobe.zoom = Some((String::new(), std::time::Instant::now())); }
    app.ui_epoch += 1; cx.notify();
}
/// "Fix": the finding's message (from the hub) goes to the thread as the person's message.
pub fn fix(app: &mut Backplane, id: &str, cx: &mut Context<Backplane>) {
    log("checks_fix", json!({"id": id}));
    let _ = app.tx.try_send(Command::Ask("kicad.checks.fix".into(), json!({"project": project(app), "id": id}), FIX_TAG));
    cx.notify();
}
pub fn run(app: &mut Backplane, cx: &mut Context<Backplane>) {
    let _ = app.tx.try_send(Command::Rpc("kicad.checks.run".into(), Secret(json!({"project": project(app)})), "checks-run"));
    cx.notify();
}
/// The e2e harness's controls (screens/mgmt.rs lists them): Run, Run all, the severity filters, and a row and its Fix each.
pub fn controls(m: &super::mgmt::Mgmt) -> Vec<(String, String, String)> {
    if m.peek()["open"] != "checks" { return vec![] }
    let d = m.data("checks");
    let mut v: Vec<(String, String, String)> = vec![("mgmt:checks:run".into(), "Run".into(), "checks:run".into()), ("mgmt:checks:full".into(), "Run all checks".into(), "checks:full".into())]; // i18n-ok: harness ids and labels
    let (more, open) = ui(|u| (u.more.clone(), u.open.clone()));
    let found = ui(|u| u.found.clone()).map(|x| x.2);
    v.push(("mgmt:checks:scroll".into(), "Scroll".into(), "checks:scroll".into())); // i18n-ok: a harness id
    for k in ["", "drc", "erc"].into_iter().map(str::to_string).chain(d["order"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string))) { v.push((format!("mgmt:checks:src:{k}"), source_label(&d, &k), format!("checks:src:{k}"))); }
    for g in d["groups"].as_array().into_iter().flatten() { let k = group_key(g); v.push((format!("mgmt:checks:more:{k}"), "More".into(), format!("checks:more:{k}"))); } // i18n-ok: harness ids
    for f in visible(&d, found.as_deref(), &more, &open) {
        let id = s(&f, "id");
        v.push((format!("mgmt:checks:row:{id}"), s(&f, "title").to_string(), format!("checks:row:{id}")));
        v.push((format!("mgmt:checks:fix:{id}"), "Fix with agent".into(), format!("checks:fix:{id}"))); // i18n-ok: a harness label
    }
    v
}
/// A press of one of `controls`.
pub fn act(app: &mut Backplane, a: &str, cx: &mut Context<Backplane>) {
    if let Some(id) = a.strip_prefix("row:") { select(app, id, cx) } else if let Some(id) = a.strip_prefix("fix:") { fix(app, id, cx) }
    else if let Some(k) = a.strip_prefix("src:") { ui(|u| u.src = k.to_string()); app.ui_epoch += 1; cx.notify() }
    else if a == "run" { run(app, cx) } else if a == "full" { run_full(app, cx) }
    else if let Some(k) = a.strip_prefix("more:") { if let Some(g) = app.screens.mgmt.data("checks")["groups"].as_array().into_iter().flatten().find(|g| group_key(g) == k).cloned() { open_group(app, &g, cx) } }
    else if a == "scroll" {
        // a screen's worth down the list (the e2e's wheel: a synthetic wheel is not delivered to the window)
        if let Some(h) = ui(|u| u.scroll.clone()) { let b = &h.0.borrow().base_handle; let o = b.offset(); b.set_offset(point(o.x, o.y - px(240.))); }
        app.ui_epoch += 1; cx.notify();
    }
}
/// "Run all checks": both checks with the library checks too (more memory: the hub's guard still decides when).
pub fn run_full(app: &mut Backplane, cx: &mut Context<Backplane>) {
    let _ = app.tx.try_send(Command::Rpc("kicad.checks.run".into(), Secret(json!({"project": project(app), "full": true})), "checks-run"));
    cx.notify();
}
/// The search's first key: the severity, and the source after a bar when one is picked ("error|fab").
pub fn search_key(sev: &str, src: &str) -> String { if src.is_empty() { sev.to_string() } else { format!("{sev}|{src}") } }
/// A source's name in the panel: DRC and ERC, else the label it gave, marked when an agent judged it.
pub fn source_label(d: &Value, src: &str) -> String {
    match src { "drc" => "DRC".into(), "erc" => "ERC".into(), _ => { let l = s(&d["sources"][src], "label"); if l.is_empty() { src.to_string() } else { l.to_string() } } } // i18n-ok: the checks' names
}
pub fn judged(d: &Value, src: &str) -> bool { d["sources"][src]["judged"] == true }
fn ask_search(app: &mut Backplane, sev: &str, q: &str) {
    let key = (sev.to_string(), q.trim().to_string());
    let (sev_only, src) = key.0.split_once('|').map_or((key.0.clone(), String::new()), |(a, b)| (a.to_string(), b.to_string()));
    if ui(|u| u.asked.as_ref() == Some(&key)) { return }
    ui(|u| u.asked = Some(key.clone()));
    let _ = app.tx.try_send(Command::Ask("kicad.checks.list".into(), json!({"project": project(app), "severity": sev_only, "source": src, "q": key.1, "rows": true, "limit": 1000}), LIST_TAG));
}
fn open_group(app: &mut Backplane, g: &Value, cx: &mut Context<Backplane>) {
    let k = group_key(g);
    let now_open = ui(|u| if u.open.remove(&k) { false } else { u.open.insert(k.clone()); true });
    if now_open && !ui(|u| u.more.contains_key(&k)) {
        let sev = if g["excluded"] == true { "excluded" } else { "" };
        let _ = app.tx.try_send(Command::Ask("kicad.checks.list".into(), json!({"project": project(app), "source": s(g, "source"), "rule": s(g, "rule"), "severity": sev, "rows": true, "limit": 2000, "tag": k}), MORE_TAG));
    }
    cx.notify();
}

/// The hub's answers to this panel's asks (main.rs routes them here first).
pub fn answered(app: &mut Backplane, id: u64, reply: &Value, cx: &mut Context<Backplane>) -> bool {
    if ![LIST_TAG, MORE_TAG, FIX_TAG, WANT_TAG].contains(&id) { return false }
    let r = if reply["result"].is_object() { &reply["result"] } else { reply };
    if reply["ok"] == false { app.screens.mgmt.error = crate::hub::rpc_error(reply); cx.notify(); return true }
    let rows = r["findings"].as_array().cloned().unwrap_or_default();
    match id {
        LIST_TAG => ui(|u| if let Some((a, b)) = u.asked.clone() { u.found = Some((a, b, rows)) }),
        // the wanted finding's group: opened (its rows fetched), so the row is there to select
        WANT_TAG => {
            let want = ui(|u| u.want.as_ref().map(|w| w.0.clone())).unwrap_or_default();
            if let Some(f) = rows.iter().find(|f| s(f, "id") == want) {
                let g = json!({"source": s(f, "source"), "rule": s(f, "rule"), "excluded": s(f, "severity") == "excluded"});
                if !ui(|u| u.open.contains(&group_key(&g))) { open_group(app, &g, cx) }
            } else { ui(|u| u.want = None) } // gone (the checks ran again): nothing to show
        }
        MORE_TAG => {
            // the group these rows are: the rule and check of the first one (all of one group)
            if let Some(f) = rows.first() { let k = format!("{}:{}:{}", s(f, "source"), s(f, "rule"), s(f, "severity") == "excluded"); ui(|u| { u.more.insert(k, rows.clone()); }); }
        }
        _ => {
            let text = s(r, "text").to_string();
            let tid = crate::screens::attach::raw_id(app.data["thread"]["id"].as_str().unwrap_or("")).to_string();
            if !text.is_empty() && !tid.is_empty() {
                let msg = format!("m{}-chk", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis()));
                let _ = app.tx.try_send(Command::Rpc("turn.start".into(), Secret(json!({"thread": tid, "text": text, "msg": msg})), "checks-fix"));
            }
        }
    }
    app.ui_epoch += 1; cx.notify(); true
}

/// A new result from the hub: what was fetched for it is old.
fn sync(d: &Value) {
    let stamp = format!("{}/{}", d["drc"]["at"], d["erc"]["at"]);
    thread_local! { static SEEN: RefCell<String> = RefCell::new(String::new()); }
    let changed = SEEN.with(|x| { let mut x = x.borrow_mut(); let c = *x != stamp; *x = stamp; c });
    if changed { ui(|u| { u.more.clear(); u.found = None; u.asked = None; }) }
}

pub fn body(app: &mut Backplane, d: &Value, light: bool, cx: &mut Context<Backplane>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    if ui(|u| u.filter.is_none()) {
        let f = cx.new(|cx| Composer::with_context(cx, "AskInput"));
        f.update(cx, |i, _| i.set_placeholder(&trs("Filter")));
        cx.observe(&f, |_, _, cx| cx.notify()).detach();
        let h = cx.focus_handle();
        ui(|u| { u.filter = Some(f); u.focus = Some(h); });
    }
    if d.is_null() { return div().p_3().text_size(crate::scale::tpx(12.)).text_color(c("faint")).child(tf("gpui.mgmt.checks.reading", &[])).into_any_element() }
    sync(d);
    let q = filter_text(cx);
    let src_now = ui(|u| u.src.clone());
    let sev = search_key(&ui(|u| u.sev.clone()), &src_now);
    // a source alone keeps the groups (each rule a heading with its count, its first rows): only a severity or words ask the hub
    let search = searching(&ui(|u| u.sev.clone()), &q);
    if search { ask_search(app, &sev, &q) }
    let found = ui(|u| u.found.clone()).filter(|(a, b, _)| *a == sev && *b == q.trim()).map(|x| x.2);
    let (more, open) = ui(|u| (u.more.clone(), u.open.clone()));

    // the top row: each check's state, Run, the severity filter and the words
    let mut top = div().flex_shrink_0().min_h(crate::scale::tpx(super::head::ROW_H)).flex().items_center().gap_2().px_2().border_b_1().border_color(c("rule")).text_size(crate::scale::tpx(11.));
    for (name, k) in [("DRC", "drc"), ("ERC", "erc")] { // i18n-ok: the checks' names
        let (t, col) = state_text(name, &d[k]);
        if !t.is_empty() { top = top.child(div().id(SharedString::from(format!("checks-state-{k}"))).flex_shrink_0().max_w(px(260.)).overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(c(col)).child(t)); }
    }
    let running = d["drc"]["state"] == "running" || d["erc"]["state"] == "running";
    top = top.child(crate::ui::with_rect(div().id("checks-run"), || "mgmt:checks:run".into()).flex_shrink_0().px_2().border_1().border_color(c("line")).cursor_pointer().hover(|x| x.bg(c("panel")))
        .text_color(c(if running { "faint" } else { "text" })).child(tf("gpui.mgmt.checks.run", &[])).on_click(cx.listener(|this, _, _, cx| run(this, cx))));
    let mut sevs = div().flex_shrink_0().flex().border_1().border_color(c("line"));
    let sev_only = ui(|u| u.sev.clone());
    for (k, label) in [("", "gpui.mgmt.checks.all"), ("error", "gpui.mgmt.checks.errors"), ("warning", "gpui.mgmt.checks.warnings"), ("excluded", "gpui.mgmt.checks.excluded")] {
        let on = sev_only == k;
        sevs = sevs.child(crate::ui::with_rect(div().id(SharedString::from(format!("checks-sev-{k}"))), move || format!("mgmt:checks:sev:{k}")).px(px(6.)).cursor_pointer()
            .when(on, |x| x.bg(c("raise")).text_color(c("text"))).when(!on, |x| x.text_color(c("dim")).hover(|x| x.bg(c("panel"))))
            .child(tf(label, &[])).on_click(cx.listener(move |_, _, _, cx| { ui(|u| u.sev = k.to_string()); cx.notify(); })));
    }
    // the sources, when there are more than DRC and ERC: one list, narrowed to one of them by a press
    let order: Vec<String> = d["order"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect();
    let srcs = (!order.is_empty()).then(|| {
        let mut row = div().flex_shrink_0().flex().flex_wrap().items_center().gap_1().px_2().py(px(3.)).border_b_1().border_color(c("line")).text_size(crate::scale::tpx(11.));
        let all: Vec<(String, String)> = std::iter::once((String::new(), tf("gpui.mgmt.checks.allsources", &[]))).chain(["drc", "erc"].iter().map(|x| (x.to_string(), source_label(d, x)))).chain(order.iter().map(|x| (x.clone(), source_label(d, x)))).collect();
        for (k, label) in all {
            let on = src_now == k; let stale = d["sources"][&k]["state"] == "stale"; let kk = k.clone();
            let label = if judged(d, &k) { format!("{label} · {}", tf("gpui.mgmt.checks.judged", &[])) } else { label }; // i18n-ok: a join
            row = row.child(crate::ui::with_rect(div().id(SharedString::from(format!("checks-src-{k}"))), { let k = k.clone(); move || format!("mgmt:checks:src:{k}") })
                .px(px(6.)).border_1().border_color(c("line")).cursor_pointer().when(on, |x| x.bg(c("raise")).text_color(c("text"))).when(!on, |x| x.text_color(c(if stale { "faint" } else { "dim" })).hover(|x| x.bg(c("panel"))))
                .child(if stale { format!("{label} · {}", tf("gpui.mgmt.checks.stale", &[])) } else { label }) // i18n-ok: a join
                .on_click(cx.listener(move |_, _, _, cx| { ui(|u| u.src = kk.clone()); cx.notify(); })));
        }
        row
    });
    top = top.child(sevs).child(div().flex_1().min_w(px(80.)).px_2().py(px(2.)).border_1().border_color(c("line")).bg(c("panel")).text_size(crate::scale::tpx(12.)).children(ui(|u| u.filter.clone())));

    // the list: groups (a heading, its rows, "n more"), or the search's rows
    // the list: only the rows in view are built (a board's hundreds of findings scroll at the frame rate); every item is one row high
    let of_src = by_source(d, &src_now);
    let its = items(&of_src, if search { Some(found.as_deref().unwrap_or(&[])) } else { None }, search && found.is_none(), &more, &open);
    let n = its.len();
    let scroll = ui(|u| u.scroll.get_or_insert_with(UniformListScrollHandle::new).clone());
    // a finding named by a KiCad marker click: selected and scrolled to once its row is in the list; else its group is found and opened
    if let Some((want, asked)) = ui(|u| u.want.clone()) {
        if let Some(ix) = its.iter().position(|i| matches!(i, Item::Row(f) if s(f, "id") == want)) {
            ui(|u| { u.want = None; u.sel = Some(want.clone()); });
            scroll.scroll_to_item(ix, ScrollStrategy::Center);
            log("checks_reveal", json!({"id": want}));
        } else if !asked {
            ui(|u| u.want = Some((want.clone(), true)));
            // a word typed in the filter could hide it too
            if let Some(f) = ui(|u| u.filter.clone()) { f.update(cx, |i, cx| i.set(String::new(), cx)); }
            let src = want.split(':').next().unwrap_or("").to_string();
            let _ = app.tx.try_send(Command::Ask("kicad.checks.list".into(), json!({"project": project(app), "source": src, "rows": true, "limit": 2000}), WANT_TAG));
        }
    }
    ui(|u| { u.items = its.clone(); u.light = light; u.sources = d["sources"].clone(); });
    let list = uniform_list("checks-rows", n, cx.processor(|this, range: std::ops::Range<usize>, _w, cx| range.map(|i| item_el(this, i, cx)).collect::<Vec<_>>()))
        .track_scroll(scroll.clone()).flex_1().min_h_0().text_size(crate::scale::tpx(12.));
    // what these results leave out, said plainly: the background runs skip the library checks, so the counts can differ from KiCad's own DRC
    let left_out = ["drc", "erc"].iter().any(|k| !d[*k].is_null() && !s(&d[*k], "note").is_empty());
    let full_ran = ["drc", "erc"].iter().any(|k| d[*k]["full"] == true);
    let foot = (left_out || full_ran).then(|| div().flex_shrink_0().flex().items_center().gap_2().px_2().py(px(4.)).border_t_1().border_color(c("line")).text_size(crate::scale::tpx(11.)).text_color(c("faint"))
        .child(div().flex_1().min_w_0().child(if left_out { tf("gpui.mgmt.checks.library", &[]) } else { tf("gpui.mgmt.checks.withlib", &[]) }))
        .when(left_out, |x| x.child(crate::ui::with_rect(div().id("checks-full"), || "mgmt:checks:full".into()).flex_shrink_0().px_2().border_1().border_color(c("line")).text_color(c("dim")).cursor_pointer()
            .hover(|x| x.text_color(c("text")).bg(c("panel"))).child(tf("gpui.mgmt.checks.fullrun", &[])).on_click(cx.listener(|this, _, _, cx| run_full(this, cx))))));

    // up and down move through the rows, each one shown in KiCad as it is reached (and scrolled into view)
    let ids: Vec<String> = its.iter().filter_map(|i| if let Item::Row(f) = i { Some(s(f, "id").to_string()) } else { None }).collect();
    let focus = ui(|u| u.focus.clone());
    div().size_full().flex().flex_col().when_some(focus, |x, h| x.track_focus(&h).key_context("ChecksPanel"))
        .on_key_down(cx.listener(move |this, e: &KeyDownEvent, _, cx| {
            let by = match e.keystroke.key.as_str() { "down" => 1, "up" => -1, _ => return };
            let Some(id) = step(&ids, ui(|u| u.sel.clone()).as_deref(), by) else { return };
            cx.stop_propagation();
            if let Some(ix) = ui(|u| u.items.iter().position(|i| matches!(i, Item::Row(f) if s(f, "id") == id))) { scroll.scroll_to_item(ix, ScrollStrategy::Center) }
            select(this, &id, cx)
        }))
        .child(top).children(srcs).child(list).children(foot).into_any_element()
}

// The chip in the editor's row ("DRC 12 · ERC 3", coloured only when not clean): every project's, from `mgmt.checks.<digest>` as it arrives.
thread_local! { static CHIPS: RefCell<std::collections::HashMap<String, (String, &'static str)>> = RefCell::new(Default::default()); }
/// The chip's colour: errors red, warnings amber, a failed check red, else quiet.
pub fn chip_tone(d: &Value) -> &'static str {
    let n = |k: &str, sev: &str| d[k]["counts"][sev].as_u64().unwrap_or(0);
    if d["drc"]["state"] == "failed" || d["erc"]["state"] == "failed" || n("drc", "error") + n("erc", "error") > 0 { "bad" }
    else if n("drc", "warning") + n("erc", "warning") + n("drc", "info") + n("erc", "info") > 0 { "warn" } else { "faint" }
}
/// A `mgmt.*` info arrived (main.rs): the chip of its project, when it is the Checks panel's.
pub fn seen(key: &str, text: &str) {
    // a click on a review marker in KiCad (bp.13; lane J's review/markers.rs): that finding is the one to show
    if key.starts_with("review.clicked.") {
        let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
        if let Some(id) = v["id"].as_str().filter(|i| !i.is_empty()) { ui(|u| { u.want = Some((id.to_string(), false)); u.sev.clear(); u.src.clear(); u.found = None; u.asked = None; }); }
        return
    }
    if !key.starts_with("mgmt.checks.") { return }
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let (pro, chip) = (s(&v, "project").to_string(), s(&v, "chip").to_string());
    if pro.is_empty() { return }
    CHIPS.with(|m| { let mut m = m.borrow_mut(); if chip.is_empty() { m.remove(&pro); } else { m.insert(pro, (chip, chip_tone(&v))); } });
}
/// The chip for a project (its .kicad_pro), once the hub has published its checks.
/// `project`: the editor's (its .kicad_pro, board or schematic, or the project's folder).
pub fn chip_for(project: &str) -> Option<(String, &'static str)> {
    CHIPS.with(|m| { let m = m.borrow(); m.get(project).cloned().or_else(|| m.iter().find(|(k, _)| same_project(k, project)).map(|(_, v)| v.clone())) })
}
/// Whether a path names the project of a .kicad_pro: the file itself, a file beside it with its name (board, schematic), or its folder.
pub fn same_project(pro: &str, p: &str) -> bool {
    let (pro, p) = (pro.replace('\\', "/"), p.replace('\\', "/"));
    let p = p.trim_end_matches('/');
    let stem = pro.rsplit_once('.').map_or(pro.as_str(), |x| x.0);
    let dir = pro.rsplit_once('/').map_or("", |x| x.0);
    p == pro || p.rsplit_once('.').map_or(false, |x| x.0 == stem) || (!dir.is_empty() && p == dir)
}
/// The chip as the editor's row draws it; its press opens the Checks panel (`open_panel`).
pub fn chip_el(text: &str, tone: &'static str, light: bool) -> Stateful<Div> {
    let c = |n: &str| theme::color(light, n);
    crate::ui::with_rect(div().id("kicad-checks-chip"), || "kicad:checks-chip".into()).ml_2().flex_shrink_0().px(px(6.)).text_size(crate::scale::tpx(11.)).border_1().border_color(c("line")).cursor_pointer()
        .text_color(c(if tone == "faint" { "dim" } else { tone })).hover(|x| x.bg(c("raise"))).child(text.to_string())
}
/// The chip pressed: the Checks panel opens (left open when it already is).
pub fn open_panel(app: &mut Backplane, cx: &mut Context<Backplane>) {
    if app.screens.mgmt.peek()["open"] != "checks" { super::mgmt::toggle(app, "checks", cx) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn r(id: &str) -> Value { json!({"id": id, "severity": "error", "title": "t", "names": "U1"}) }
    fn data() -> Value { json!({"total": 5, "groups": [{"source": "drc", "rule": "clearance", "excluded": false, "n": 3, "rows": [r("a"), r("b")]}, {"source": "erc", "rule": "pin_not_driven", "excluded": false, "n": 2, "rows": [r("c"), r("d")]}]}) }
    #[test] fn rows_in_order_and_opened_groups() {
        let d = data();
        let ids = |v: Vec<Value>| v.iter().map(|x| x["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert_eq!(ids(visible(&d, None, &BTreeMap::new(), &BTreeSet::new())), ["a", "b", "c", "d"]);
        let k = "drc:clearance:false".to_string();
        let more: BTreeMap<String, Vec<Value>> = [(k.clone(), vec![r("a"), r("b"), r("e")])].into();
        assert_eq!(ids(visible(&d, None, &more, &[k.clone()].into())), ["a", "b", "e", "c", "d"]);
        assert_eq!(ids(visible(&d, None, &more, &BTreeSet::new())), ["a", "b", "c", "d"], "a shut group shows its first rows");
        assert_eq!(ids(visible(&d, Some(&[r("z")]), &more, &BTreeSet::new())), ["z"]);
        assert_eq!(group_key(&d["groups"][0]), k);
    }
    #[test] fn the_chip() {
        let d = |de: u64, dw: u64, ee: u64, st: &str| json!({"project": "/p/a.kicad_pro", "chip": "DRC 1 · ERC 0", "drc": {"state": st, "counts": {"error": de, "warning": dw}}, "erc": {"state": "fresh", "counts": {"error": ee, "warning": 0}}});
        assert_eq!(chip_tone(&d(1, 0, 0, "fresh")), "bad"); assert_eq!(chip_tone(&d(0, 4, 0, "fresh")), "warn"); assert_eq!(chip_tone(&d(0, 0, 0, "fresh")), "faint");
        assert_eq!(chip_tone(&d(0, 0, 0, "failed")), "bad"); assert_eq!(chip_tone(&d(0, 0, 2, "fresh")), "bad");
        seen("mgmt.checks.abc", &d(0, 2, 0, "fresh").to_string());
        assert_eq!(chip_for("/p/a.kicad_pro"), Some(("DRC 1 · ERC 0".to_string(), "warn")));
        assert!(chip_for("/p/a.kicad_pcb").is_some() && chip_for("/p").is_some() && chip_for("/p/").is_some() && chip_for("/q/a.kicad_pcb").is_none());
        assert!(same_project("C:\\p\\a.kicad_pro", "C:/p/a.kicad_sch") && !same_project("/p/a.kicad_pro", "/p/ab.kicad_pcb"));
        seen("mgmt.bom.abc", "{}"); assert!(chip_for("/p/b.kicad_pro").is_none());
    }
    #[test] fn sources_by_name() {
        let d = json!({"sources": {"fab": {"label": "Fab rules", "judged": false}, "datasheet": {"label": "Datasheets", "judged": true}}});
        assert_eq!(source_label(&d, "drc"), "DRC"); assert_eq!(source_label(&d, "fab"), "Fab rules"); assert_eq!(source_label(&d, "bom"), "bom");
        assert!(judged(&d, "datasheet") && !judged(&d, "fab"));
        let d = json!({"groups": [{"source": "drc", "rule": "clearance", "n": 1, "rows": []}, {"source": "gerbers", "rule": "B.Cu", "n": 21, "rows": []}]});
        assert_eq!(by_source(&d, "gerbers")["groups"].as_array().unwrap().iter().map(|g| s(g, "rule")).collect::<Vec<_>>(), ["B.Cu"], "a source alone: its groups, headed and counted");
        assert_eq!(by_source(&d, "")["groups"].as_array().unwrap().len(), 2);
        assert_eq!(search_key("error", ""), "error"); assert_eq!(search_key("", "fab"), "|fab"); assert!(searching(&search_key("", "fab"), ""));
    }
    #[test] fn a_shut_group_shows_three() {
        let d = json!({"groups": [{"source": "drc", "rule": "unconnected_items", "excluded": false, "n": 251, "rows": [r("a"), r("b"), r("c"), r("d"), r("e")]}]});
        let ids = |v: Vec<Item>| v.iter().filter_map(|i| if let Item::Row(f) = i { Some(f["id"].as_str().unwrap().to_string()) } else { None }).collect::<Vec<_>>();
        assert_eq!(ids(items(&d, None, false, &BTreeMap::new(), &BTreeSet::new())), ["a", "b", "c"]);
        assert!(matches!(items(&d, None, false, &BTreeMap::new(), &BTreeSet::new()).last(), Some(Item::More(_, 248))));
        let k: BTreeSet<String> = ["drc:unconnected_items:false".to_string()].into();
        assert_eq!(ids(items(&d, None, false, &BTreeMap::new(), &k)).len(), 5, "opened, before the rest arrives: what was published");
        assert_eq!(visible(&d, None, &BTreeMap::new(), &BTreeSet::new()).len(), 3);
    }
    #[test] fn the_list_lines() {
        let d = data();
        let kinds = |v: &[Item]| v.iter().map(|i| match i { Item::Head(g) => format!("h:{}", g["rule"].as_str().unwrap()), Item::Row(f) => f["id"].as_str().unwrap().to_string(), Item::More(_, n) => format!("+{n}"), Item::Msg(k) => k.to_string() }).collect::<Vec<_>>();
        assert_eq!(kinds(&items(&d, None, false, &BTreeMap::new(), &BTreeSet::new())), ["h:clearance", "a", "b", "+1", "h:pin_not_driven", "c", "d"]);
        let k = "drc:clearance:false".to_string();
        let more: BTreeMap<String, Vec<Value>> = [(k.clone(), vec![r("a"), r("b"), r("e")])].into();
        assert_eq!(kinds(&items(&d, None, false, &more, &[k].into())), ["h:clearance", "a", "b", "e", "+0", "h:pin_not_driven", "c", "d"]);
        assert_eq!(kinds(&items(&d, Some(&[]), false, &more, &BTreeSet::new())), ["gpui.mgmt.checks.nomatch"]);
        assert_eq!(kinds(&items(&d, None, true, &more, &BTreeSet::new())), ["gpui.mgmt.checks.searching"]);
        assert_eq!(kinds(&items(&json!({"groups": [], "drc": {"has_result": true}}), None, false, &more, &BTreeSet::new())), ["gpui.mgmt.checks.none"]);
    }
    #[test] fn a_marker_click_names_the_finding_to_show() {
        ui(|u| { u.sev = "error".into(); u.src = "drc".into(); u.want = None; });
        seen("review.clicked.abc", r#"{"id":"review:1f","at":5}"#);
        assert_eq!(ui(|u| (u.want.clone(), u.sev.clone(), u.src.clone())), (Some(("review:1f".to_string(), false)), String::new(), String::new()), "the filters that could hide it are cleared");
        seen("review.clicked.abc", r#"{"id":""}"#); assert!(ui(|u| u.want.is_some()), "an empty one changes nothing");
        ui(|u| u.want = None);
    }
    #[test] fn up_and_down() {
        let ids: Vec<String> = ["a", "b", "c"].iter().map(|x| x.to_string()).collect();
        assert_eq!(step(&ids, None, 1).as_deref(), Some("a")); assert_eq!(step(&ids, Some("a"), 1).as_deref(), Some("b"));
        assert_eq!(step(&ids, Some("c"), 1), None); assert_eq!(step(&ids, Some("a"), -1), None); assert_eq!(step(&ids, Some("gone"), -1).as_deref(), Some("a"));
        assert_eq!(step(&[], None, 1), None);
    }
    #[test] fn words() {
        assert_eq!(rule_label("courtyards_overlap"), "Courtyards overlap"); assert_eq!(rule_label(""), "");
        assert!(searching("error", "") && searching("", "gnd") && !searching("", "  "));
        assert_eq!(tone("error"), "bad"); assert_eq!(tone("excluded"), "faint");
        let (t, col) = state_text("DRC", &json!({"state": "fresh", "has_result": true, "counts": {"error": 2, "warning": 3, "info": 0}}));
        assert_eq!((t.as_str(), col), ("DRC 5", "bad"));
        assert_eq!(state_text("ERC", &json!({"state": "fresh", "has_result": true, "counts": {"error": 0, "warning": 0}})).0, "ERC ✓");
        assert_eq!(state_text("DRC", &json!({"state": "fresh", "has_result": true, "unsaved": true, "counts": {"error": 2, "warning": 0}})).0, "DRC 2 · live, unsaved");
        assert_eq!(state_text("DRC", &json!({"state": "running", "has_result": true, "behind": "ERC", "counts": {"error": 2, "warning": 0}})).0, "DRC 2 · after ERC");
        assert!(state_text("DRC", &json!({"state": "stale", "has_result": true, "counts": {"warning": 1}})).0.starts_with("DRC 1 · "));
        assert_eq!(state_text("DRC", &Value::Null).0, "");
        assert!(state_text("DRC", &json!({"state": "fresh", "has_result": true, "counts": {"error": 1}, "ran_on": "h@backplane"})).0.contains("h@backplane"));
    }
}
