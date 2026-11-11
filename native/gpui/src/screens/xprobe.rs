//! Cross-probing from the chat and the viewers (the hub's kicad/xprobe.rs): a reference designator, pin or net name of the thread's design in a
//! message is a link (`kicad:<word>`); clicking it selects and zooms it in the open KiCad and lights it in Backplane's viewers (opening the Board
//! page when nothing shows the design). A piece tapped in a viewer is selected in KiCad too. The pure rules are here; the window calls them.
use super::md::{Link, Span, Style};
use crate::{hub::Command, screens::settings::Secret, Backplane};
use serde_json::{json, Value};
use std::collections::HashSet;

/// A design link's href prefix.
pub const HREF: &str = "kicad:";

/// The names of the thread's design (`kicad.names`): its parts' references and its nets.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Design { pub refs: HashSet<String>, pub nets: HashSet<String> }
impl Design {
    pub fn from_reply(v: &Value) -> Design {
        let r = if v["result"].is_object() { &v["result"] } else { v };
        let set = |k: &str| r[k].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|s| s.len() >= 2 && s.len() <= 64).map(str::to_string).take(5000).collect();
        Design { refs: set("references"), nets: set("nets") }
    }
    pub fn is_empty(&self) -> bool { self.refs.is_empty() && self.nets.is_empty() }
    /// What a word names: "ref", "pin" (`U3.4` of a known part) or "net"; None for none.
    pub fn kind(&self, w: &str) -> Option<&'static str> {
        if self.refs.contains(w) { return Some("ref") }
        if let Some((r, n)) = w.split_once('.') { if self.refs.contains(r) && !n.is_empty() && n.len() <= 8 && n.chars().all(|c| c.is_ascii_alphanumeric()) { return Some("pin") } }
        if self.nets.contains(w) { return Some("net") }
        None
    }
}

/// The trimmed word at each whitespace-separated token: (start, end) byte ranges without the punctuation round it.
fn tokens(s: &str) -> Vec<(usize, usize)> {
    let mut out = vec![]; let mut i = 0; let b = s.as_bytes();
    while i < b.len() {
        while i < b.len() && b[i].is_ascii_whitespace() { i += 1 }
        let st = i;
        while i < b.len() && !b[i].is_ascii_whitespace() { i += 1 }
        if st == i { continue }
        let w = &s[st..i];
        let lead = w.len() - w.trim_start_matches(['(', '[', '"', '\'', '`', '*', '<']).len();
        let core = w.trim_start_matches(['(', '[', '"', '\'', '`', '*', '<']).trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '"', '\'', '`', '*', '>']);
        if !core.is_empty() { out.push((st + lead, st + lead + core.len())); }
    }
    out
}

/// Spans with the design's words as links (`kicad:<word>`): a code span that is one word, or each word of a plain, bold or italic span. Links stay.
pub fn links(spans: Vec<Span>, d: &Design) -> Vec<Span> {
    if d.is_empty() { return spans }
    let link = |w: &str| Some(Link { href: format!("{HREF}{w}"), file: false });
    let mut out = Vec::with_capacity(spans.len());
    for s in spans {
        if s.style.link.is_some() { out.push(s); continue }
        if s.style.code {
            let w = s.text.trim();
            if d.kind(w).is_some() && w == s.text { out.push(Span { style: Style { link: link(w), ..s.style.clone() }, text: s.text }); } else { out.push(s); }
            continue
        }
        let mut at = 0;
        for (a, b) in tokens(&s.text) {
            let w = &s.text[a..b];
            if d.kind(w).is_none() { continue }
            if a > at { out.push(Span { text: s.text[at..a].to_string(), style: s.style.clone() }); }
            out.push(Span { text: w.to_string(), style: Style { link: link(w), ..s.style.clone() } }); at = b;
        }
        if at == 0 { out.push(s); } else if at < s.text.len() { out.push(Span { text: s.text[at..].to_string(), style: s.style.clone() }); }
    }
    out
}

/// `editor.kicad.select`'s names for a clicked word.
pub fn names_for_word(w: &str, d: &Design) -> Option<Value> {
    Some(match d.kind(w)? { "ref" => json!({"references": [w]}), "pin" => json!({"pins": [w]}), _ => json!({"nets": [w]}) })
}

/// `editor.kicad.select`'s names for a piece tapped in a viewer (kicad.info's card): its pin, else its part, else its net.
pub fn names_for_card(c: &super::plot::Card) -> Option<Value> {
    let get = |k: &str| c.rows.iter().find(|r| r.0 == k).map(|r| r.1.clone()).filter(|v| !v.is_empty());
    let part = get("ref").or_else(|| get("symbol"));
    let num = c.title.strip_prefix("pad ").or_else(|| c.title.strip_prefix("pin ")).map(str::trim).filter(|n| !n.is_empty());
    match (part, num) {
        (Some(r), Some(n)) => Some(json!({"pins": [format!("{r}.{n}")], "references": []})),
        (Some(r), None) => Some(json!({"references": [r]})),
        (None, _) => get("net").map(|n| json!({"nets": [n]})),
    }
}

/// The thread whose design names the timeline uses; asked of the hub when the thread changes or the names are a minute old.
pub fn want_names(app: &mut Backplane) {
    let tid = crate::screens::attach::raw_id(app.data["thread"]["id"].as_str().unwrap_or("")).to_string();
    if tid.is_empty() { return }
    let x = &mut app.screens.xprobe;
    if x.asked.0 == tid && x.asked.1.map_or(false, |t| t.elapsed() < std::time::Duration::from_secs(60)) { return }
    if x.asked.0 != tid { x.design = Default::default(); }
    x.asked = (tid.clone(), Some(std::time::Instant::now()));
    let _ = app.tx.try_send(Command::Rpc("kicad.names".into(), Secret(json!({"thread": tid})), "names"));
}
/// `kicad.names`' answer: kept when it is the thread's still.
pub fn got_names(app: &mut Backplane, v: &Value) {
    let r = if v["result"].is_object() { &v["result"] } else { v };
    if r["thread"].as_str().unwrap_or("") != app.screens.xprobe.asked.0 { return }
    app.screens.xprobe.design = std::rc::Rc::new(Design::from_reply(v));
}

/// A design word clicked in the chat: selected in KiCad (the hub), lit in the viewers; with no editor in the pane, the Board page opens and
/// zooms on it once the hub's probe comes back.
pub fn clicked(app: &mut Backplane, word: &str) {
    let d = app.screens.xprobe.design.clone();
    let Some(mut p) = names_for_word(word, &d) else { return };
    let tid = crate::screens::attach::raw_id(app.data["thread"]["id"].as_str().unwrap_or("")).to_string();
    p["thread"] = json!(tid); p["zoom"] = json!(true);
    let editing = app.screens.kicad.is_some();
    eprintln!("GPUI_LOG {}", json!({"event": "xprobe_click", "word": word, "editing": editing}));
    let _ = app.tx.try_send(Command::Rpc("editor.kicad.select".into(), Secret(p), "xsel"));
    if editing { return }
    let open = app.data["thread"]["viewer"]["open"].as_str().unwrap_or("");
    if !matches!(open, "board" | "schematic" | "split") { let _ = app.tx.try_send(super::viewer::view_kind("board")); }
    app.screens.xprobe.zoom = Some((String::new(), std::time::Instant::now()));
}
/// `editor.kicad.select`'s answer to a click: the probe it set is the one to zoom on.
pub fn got_select(app: &mut Backplane, v: &Value) {
    let r = if v["result"].is_object() { &v["result"] } else { v };
    eprintln!("GPUI_LOG {}", json!({"event": "xprobe_select", "ok": v["ok"], "kicad": r["kicad"], "via": r["via"], "found": r["found"], "not_found": r["not_found"], "probe": r["probe"], "error": v["error"]}));
    if let Some(z) = app.screens.xprobe.zoom.as_mut() { if z.0.is_empty() { z.0 = r["probe"].as_str().unwrap_or("").to_string(); } }
    if r["probe"].as_str().map_or(true, str::is_empty) { app.screens.xprobe.zoom = None; }
}
/// While drawing the viewer: a zoom waiting for the hub's probe, done once the page shows it with a drawing and a size (given up after 15 s).
pub fn zoom_pending(app: &mut Backplane) {
    let Some((want, at)) = app.screens.xprobe.zoom.clone() else { return };
    if at.elapsed() > std::time::Duration::from_secs(15) { app.screens.xprobe.zoom = None; return }
    let v = app.data["thread"]["viewer"].clone();
    if want.is_empty() || v["probe"].as_str() != Some(want.as_str()) { return }
    let panes = v["panes"].as_array().cloned().unwrap_or_default();
    let slots: Vec<(usize, String)> = if panes.len() >= 3 { vec![(1, crate::string(&panes[0], "key")), (2, crate::string(&panes[1], "key"))] }
        else { vec![(0, app.screens.plot.frame.as_ref().map(|f| f.key.clone()).unwrap_or_default())] };
    if slots.iter().any(|(s, k)| k.is_empty() || app.screens.plot.frame_for(k).is_none() || app.screens.pv(*s).bounds.is_none()) { return }
    app.screens.xprobe.zoom = None;
    eprintln!("GPUI_LOG {}", json!({"event": "xprobe_zoom", "probe": want, "slots": slots.iter().map(|s| s.0).collect::<Vec<_>>()}));
    let card = super::plot::card(&want);
    for (s, _) in slots { app.screens.pv(s).card = card.clone(); super::plotview::bring_into_view(app, s); }
}
/// A piece tapped in a viewer, selected in the KiCad open for the project (the hub skips it when none is; the viewers keep their own probe).
pub fn tapped(app: &mut Backplane, card: &super::plot::Card) {
    if !app.screens.kicad_infos.values().any(|i| i["state"] == "ready") { return }
    let Some(mut p) = names_for_card(card) else { return };
    let tid = crate::screens::attach::raw_id(app.data["thread"]["id"].as_str().unwrap_or("")).to_string();
    p["thread"] = json!(tid); p["zoom"] = json!(true); p["from"] = json!("viewer");
    let _ = app.tx.try_send(Command::Rpc("editor.kicad.select".into(), Secret(p), "xsel-tap"));
}

/// The window's cross-probe state.
#[derive(Default)]
pub struct State {
    /// The thread the names were asked for, and when.
    pub asked: (String, Option<std::time::Instant>),
    pub design: std::rc::Rc<Design>,
    /// A zoom waiting for the hub's probe ("" until the select answers), and since when.
    pub zoom: Option<(String, std::time::Instant)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn d() -> Design { Design::from_reply(&json!({"result": {"references": ["U1", "R12", "C3"], "nets": ["GND", "/SDA", "+3V3"]}})) }
    fn plain(t: &str) -> Span { Span { text: t.into(), style: Style::default() } }
    fn hrefs(s: &[Span]) -> Vec<(String, String)> { s.iter().filter_map(|x| x.style.link.as_ref().map(|l| (x.text.clone(), l.href.clone()))).collect() }

    #[test] fn a_designs_words_are_links_in_a_message() {
        let out = links(vec![plain("Check U1 (pin U1.4) and R12, then GND; not U10 or gnd.")], &d());
        assert_eq!(hrefs(&out), vec![("U1".into(), "kicad:U1".into()), ("U1.4".into(), "kicad:U1.4".into()), ("R12".into(), "kicad:R12".into()), ("GND".into(), "kicad:GND".into())]);
        assert_eq!(super::super::md::plain(&out), "Check U1 (pin U1.4) and R12, then GND; not U10 or gnd.", "the text is unchanged");
        // a code span that is one name, a net with a slash and a plus; a link stays a link; nothing known: the spans as they were
        let code = Span { text: "/SDA".into(), style: Style { code: true, ..Style::default() } };
        let web = Span { text: "U1".into(), style: Style { link: Some(Link { href: "https://x".into(), file: false }), ..Style::default() } };
        let out = links(vec![code, plain(" on +3V3 "), web.clone()], &d());
        assert_eq!(hrefs(&out), vec![("/SDA".into(), "kicad:/SDA".into()), ("+3V3".into(), "kicad:+3V3".into()), ("U1".into(), "https://x".into())]);
        assert!(out[0].style.code, "the code span keeps its look");
        assert_eq!(links(vec![plain("U1")], &Design::default()), vec![plain("U1")]);
        assert_eq!(links(vec![plain("**U1**")], &d())[1].text, "U1");
    }

    #[test] fn a_click_and_a_tap_name_what_to_select() {
        assert_eq!(names_for_word("U1", &d()), Some(json!({"references": ["U1"]})));
        assert_eq!(names_for_word("U1.4", &d()), Some(json!({"pins": ["U1.4"]})));
        assert_eq!(names_for_word("/SDA", &d()), Some(json!({"nets": ["/SDA"]})));
        assert_eq!(names_for_word("Q9", &d()), None);
        let card = |t: &str| super::super::plot::card(t).unwrap();
        assert_eq!(names_for_card(&card("footprint;ref=U1;value=x")), Some(json!({"references": ["U1"]})));
        assert_eq!(names_for_card(&card("pad 4;net=GND;type=smd;ref=U1")), Some(json!({"pins": ["U1.4"], "references": []})));
        assert_eq!(names_for_card(&card("pin 2;name=A;symbol=R12")), Some(json!({"pins": ["R12.2"], "references": []})));
        assert_eq!(names_for_card(&card("track;net=/SDA;width=0.2")), Some(json!({"nets": ["/SDA"]})));
        assert_eq!(names_for_card(&card("line")), None);
    }
}
