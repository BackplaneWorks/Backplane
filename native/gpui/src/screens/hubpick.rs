//! Switching hubs: the picker at the top of the sidebar, and the Hubs page of Settings. Every decision (the order, what can be forgotten, what a
//! choice does) is `crate::hubs`'s; this draws it and carries it out. Switching keeps the choice, closes this window's socket by starting the client
//! again on the chosen hub (the way a new pairing already does), so nothing shown from the old hub is left.
use crate::{hubs::{self, Entry}, screens::settings::{Btn, Row}, theme, Backplane};
use gpui::{prelude::*, *};

/// `hub:menu`, `hub:add`, `hub:use:<key>`, `hub:forget:<key>`: the actions the picker, the page and a script name.
#[derive(Debug, PartialEq, Clone)]
pub enum Act { Menu, Add, Use(String), Forget(String) }
pub fn parse(action: &str) -> Option<Act> {
    let a = action.strip_prefix("hub:")?;
    match a { "menu" => Some(Act::Menu), "add" => Some(Act::Add), _ => a.strip_prefix("use:").map(|k| Act::Use(k.into())).or_else(|| a.strip_prefix("forget:").map(|k| Act::Forget(k.into()))) }
}
fn note(what: &str, text: &str) { eprintln!("GPUI_LOG {}", serde_json::json!({"hub": what, "text": text})); }
/// Start again on the saved list, whose first hub is the one to connect to. Returns only when that failed.
fn restart(this: &mut Backplane, cx: &mut Context<Backplane>) {
    note("restart", this.hubs.first().map_or("", |h| h.key.as_str()));
    let e = crate::pairing::relaunch();
    this.status = crate::i18n::tf("gpui.could.not.restart", &[("why", e.to_string().into())]); cx.notify();
}
pub fn act(this: &mut Backplane, a: Act, cx: &mut Context<Backplane>) {
    match a {
        Act::Menu => { this.hub_menu = !this.hub_menu; cx.notify(); }
        Act::Add => crate::screens::pair::add(this, cx),
        Act::Use(k) => {
            this.hub_menu = false;
            match hubs::use_saved(&k) {
                Ok(Some(list)) => { this.hubs = list; note("use", &k); restart(this, cx) }
                Ok(None) => cx.notify(),
                Err(_) => { this.status = crate::i18n::trs("The choice could not be saved here."); cx.notify() }
            }
        }
        Act::Forget(k) => match hubs::forget_saved(&k) {
            Ok(Some((list, moved))) => { this.hubs = list; note("forget", &k); if moved { restart(this, cx) } else { cx.notify() } }
            Ok(None) => cx.notify(),
            Err(_) => { this.status = crate::i18n::trs("The hub could not be forgotten here."); cx.notify() }
        },
    }
}

/// The Hubs page of Settings.
pub fn rows(list: &[hubs::Hub]) -> Vec<Row> {
    let b = |label: &str, action: String, on: bool| Btn { label: label.into(), action, value: String::new(), on };
    let mut out: Vec<Row> = entries_rows(&hubs::entries(list));
    /* i18n-ok: settings rows, drawn through trs by row_el */ out.push(Row { label: "Add a hub".into(), note: "Pair this device with a hub on another machine.".into(), buttons: vec![b("Add a hub", "hub:add".into(), false)], fields: vec![], qr: vec![], qrnote: String::new(), local: true });
    out
}
fn entries_rows(es: &[Entry]) -> Vec<Row> {
    es.iter().map(|e| {
        let mut buttons = vec![];
        /* i18n-ok: settings rows, drawn through trs by row_el */ if e.in_use { buttons.push(Btn { label: "In use".into(), action: String::new(), value: String::new(), on: true }) } else { buttons.push(Btn { label: "Use".into(), action: format!("hub:use:{}", e.key), value: String::new(), on: false }) }
        /* i18n-ok: settings rows, drawn through trs by row_el */ if e.can_forget { buttons.push(Btn { label: "Forget".into(), action: format!("hub:forget:{}", e.key), value: String::new(), on: false }) }
        /* i18n-ok: settings rows, drawn through trs by row_el */ Row { label: e.label.clone(), note: if e.key == hubs::LOCAL { "The hub on this computer.".into() } else { String::new() }, buttons, fields: vec![], qr: vec![], qrnote: String::new(), local: true }
    }).collect()
}

/// The top of the sidebar: the hub in use, and, open, the others and "Add a hub". Registers its controls for scripts.
pub fn picker(this: &Backplane, cx: &mut Context<Backplane>, light: bool, ctl: &mut Vec<crate::script::CtlEntry>) -> Div {
    use crate::script::{Ctl, CtlEntry};
    let c = |n: &str| theme::color(light, n);
    let es = hubs::entries(&this.hubs);
    // the hub the window is on, by its address when that is another computer's (a `--url` start too, whatever the list says)
    let away = this.screens.elsewhere.clone();
    let cur = away.as_ref().map(|e| e.1.clone()).or_else(|| es.first().map(|e| e.label.clone())).unwrap_or_default();
    /* i18n-ok: ctl labels are what drivers press */ ctl.push(CtlEntry { id: "hub:menu".into(), label: "Hub".into(), ctl: Ctl::Hub(Act::Menu) });
    let mut col = div().flex().flex_col().border_b_1().border_color(c("rule")).child(
        // the thread header's row height (less this column's rule, which the header's row counts in its own), so the two rules are one line
        div().id("hub-picker").flex().items_center().gap_2().px_3().h(px(super::head::ROW_H - 1.)).cursor_pointer().text_size(px(12.)).hover(|s| s.bg(c("panel")))
            .children(away.as_ref().map(|_| crate::art::icon("link", 12., c("accent"))))
            .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().when(away.is_some(), |d| d.text_color(c("accent"))).child(crate::i18n::trs(&cur))).child(crate::art::icon(if this.hub_menu { "chevron-down" } else { "chevron-right" }, 14., c("dim")))
            .on_click(cx.listener(|this, _, _, cx| act(this, Act::Menu, cx))));
    if this.hub_menu {
        for e in es.iter().skip(1) {
            ctl.push(CtlEntry { id: format!("hub:use:{}", e.key), label: e.label.clone(), ctl: Ctl::Hub(Act::Use(e.key.clone())) });
            let k = e.key.clone();
            col = col.child(div().id(SharedString::from(format!("hub-use-{}", e.key))).px_3().py_1().cursor_pointer().text_size(px(12.)).hover(|s| s.bg(c("panel"))).child(crate::i18n::trs(&e.label)).on_click(cx.listener(move |this, _, _, cx| act(this, Act::Use(k.clone()), cx))));
        }
        /* i18n-ok: ctl labels are what drivers press */ ctl.push(CtlEntry { id: "hub:add".into(), label: "Add a hub".into(), ctl: Ctl::Hub(Act::Add) });
        col = col.child(div().id("hub-add").px_3().py_1().cursor_pointer().text_size(px(12.)).text_color(c("dim")).hover(|s| s.bg(c("panel"))).child(crate::i18n::trs("Add a hub")).on_click(cx.listener(|this, _, _, cx| act(this, Act::Add, cx))));
    }
    col
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn actions_are_read_by_name() {
        assert_eq!(parse("hub:menu"), Some(Act::Menu)); assert_eq!(parse("hub:add"), Some(Act::Add));
        assert_eq!(parse("hub:use:b.ts.net:8451"), Some(Act::Use("b.ts.net:8451".into()))); assert_eq!(parse("hub:forget:a:1"), Some(Act::Forget("a:1".into())));
        assert_eq!(parse("local:theme"), None); assert_eq!(parse("hub:nonsense"), None);
    }
    #[test] fn the_page_offers_use_for_the_others_forget_for_paired_ones_and_add() {
        let l = vec![hubs::Hub::remote("http://a:1"), hubs::Hub::remote("http://b:2"), hubs::Hub::local()];
        let r = rows(&l);
        let acts = |i: usize| r[i].buttons.iter().map(|b| (b.label.as_str(), b.action.as_str())).collect::<Vec<_>>();
        assert_eq!(acts(0), [("In use", ""), ("Forget", "hub:forget:a:1")]);
        assert_eq!(acts(1), [("Use", "hub:use:b:2"), ("Forget", "hub:forget:b:2")]);
        assert_eq!(acts(2), [("Use", "hub:use:local")], "this computer's hub is never forgotten");
        assert_eq!(acts(3), [("Add a hub", "hub:add")]);
    }
}
