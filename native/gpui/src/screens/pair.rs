//! The screen an unpaired or revoked client starts at: a pairing link, or a hub address and short code. A link that came from outside
//! (the `backplane://` scheme, a command line) never pairs by itself: it names the hub it would pair with, and Cancel is the way out.
//! A desktop has no camera to speak of, so a QR code is for the phones; every decision here is `crate::pairing`'s.
use crate::{i18n::trs, input::TextInput, pairing::{self, Invitation}, theme, Backplane};
use gpui::{prelude::*, *};

/* i18n-ok: a comment's words */ pub struct Ui { pub link: Entity<TextInput>, pub addr: Entity<TextInput>, pub code: Entity<TextInput>, pub repair: bool, pub status: String, pub busy: bool, pub ask: Option<Invitation>, /** opened from "Add a hub": the form can be left */ pub adding: bool, focus: bool, /** the right-click menu: which field (0 link, 1 address, 2 code) and where */ menu: Option<(usize, Point<Pixels>)>, subs: Vec<Subscription> }

fn field(hint: &str, cx: &mut Context<Backplane>) -> Entity<TextInput> { let h: SharedString = hint.to_string().into(); cx.new(|cx| TextInput::new(cx).hint(h).field()) }
impl Ui {
    pub fn new(repair: bool, cx: &mut Context<Backplane>) -> Ui {
        /* i18n-ok: example input, not words */ Ui { link: field("Paste a pairing link (…/pair#invite=…)", cx), addr: field("Hub address, e.g. https://desktop.tail9e0cf2.ts.net:8451", cx), code: field("Code", cx), repair, status: String::new(), busy: false, ask: None, adding: false, focus: true, menu: None, subs: vec![] }
    }
}
/// What a rig reads of this screen (never a credential: a link's secret stays out of it).
fn note(what: &str, text: &str) { eprintln!("GPUI_LOG {}", serde_json::json!({"pair": what, "text": text})); }
/// A link from outside this window: the question that names its hub, or why it is no link.
fn asked(ui: &mut Ui, link: &str) {
    match pairing::invitation(link) { Some(i) => { note("ask", &i.origin); ui.ask = Some(i); ui.status.clear() } None => { ui.ask = None; /* i18n-ok: status is English (the rig reads it), drawn through trs */ ui.status = "That is not a pairing link. Copy it again from Settings → Devices.".into(); note("refused", &ui.status) } }
}
pub fn start(unpaired: bool, link: Option<&str>, cx: &mut Context<Backplane>) -> Option<Ui> {
    if !unpaired && link.is_none() { return None }
    note("screen", "connect");
    let mut ui = Ui::new(false, cx);
    if let Some(l) = link { asked(&mut ui, l) }
    Some(ui)
}
/// A link the system handed the running app.
pub fn incoming(this: &mut Backplane, link: &str, cx: &mut Context<Backplane>) {
    // a thread or project to open (one of another machine too): the hub says what it opens
    if pairing::is_open_link(link) { let _ = this.tx.try_send(crate::hub::Command::Action("link-open".into(), link.to_string())); cx.notify(); return }
    if this.pair.is_none() { this.pair = Some(Ui::new(false, cx)); }
    if let Some(ui) = &mut this.pair { asked(ui, link) }
    cx.notify();
}
/// "Add a hub": the pairing form over the running window; Cancel returns.
pub fn add(this: &mut Backplane, cx: &mut Context<Backplane>) {
    note("screen", "add");
    let mut ui = Ui::new(false, cx); ui.adding = true; this.pair = Some(ui); this.hub_menu = false; cx.notify();
}
/// The hub refused this device's credential: it was revoked.
pub fn revoked(this: &mut Backplane, cx: &mut Context<Backplane>) { note("screen", "re-pair"); this.pair = Some(Ui::new(true, cx)); cx.notify(); }
pub fn fill(this: &mut Backplane, link: &str, addr: &str, code: &str, cx: &mut Context<Backplane>) {
    let Some(ui) = &this.pair else { return };
    let (l, a, c) = (ui.link.clone(), ui.addr.clone(), ui.code.clone());
    l.update(cx, |i, cx| i.set(link.to_string(), cx)); a.update(cx, |i, cx| i.set(addr.to_string(), cx)); c.update(cx, |i, cx| i.set(code.to_string(), cx));
}
/// The form's Pair.
pub fn go(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let Some(ui) = &this.pair else { return };
    if ui.busy { return }
    let (l, a, c) = (ui.link.read(cx).content.to_string(), ui.addr.read(cx).content.to_string(), ui.code.read(cx).content.to_string());
    eprintln!("GPUI_LOG {}", serde_json::json!({"pair": "submit", "link": l.trim().len(), "address": a.trim().len(), "code": c.trim().len()}));
    match pairing::submit(&l, &a, &c) { Ok(i) => pair(this, i, cx), Err(e) => { note("refused", e); if let Some(ui) = &mut this.pair { ui.status = e.into() } cx.notify() } }
}
/// The question's Pair.
pub fn confirm(this: &mut Backplane, cx: &mut Context<Backplane>) {
    if let Some(i) = this.pair.as_mut().and_then(|u| u.ask.take()) { pair(this, i, cx) }
}
/// The question's Cancel: the form again, or, on a client that has a hub, back to it.
pub fn cancel(this: &mut Backplane, cx: &mut Context<Backplane>) {
    note("cancelled", "");
    if this.has_hub { this.pair = None } else if let Some(ui) = &mut this.pair { ui.ask = None }
    cx.notify();
}
fn pair(this: &mut Backplane, i: Invitation, cx: &mut Context<Backplane>) {
    /* i18n-ok: status is English (the rig reads it), drawn through trs */ if let Some(ui) = &mut this.pair { ui.busy = true; ui.status = format!("Pairing with {}…", i.origin) }
    cx.notify();
    cx.spawn(async move |weak, cx| {
        let o = i.origin.clone();
        /* i18n-ok: status is English (the rig reads it), drawn through trs */ let r = cx.background_executor().spawn(async move { let (at, t) = pairing::exchange(&i, &pairing::name())?; crate::hubs::adopt_saved(&at, &t).map(|_| ()).map_err(|_| "This pairing could not be saved here.".to_string()) }).await;
        // a failed exchange leaves any existing pairing as it was
        let _ = weak.update(cx, |this, cx| {
            let why = match r { Ok(()) => { eprintln!("GPUI_LOG {}", serde_json::json!({"paired": o})); format!("Could not restart: {}", pairing::relaunch()) } Err(e) => e };
            note("refused", &why);
            if let Some(ui) = &mut this.pair { ui.busy = false; ui.status = why }
            cx.notify();
        });
    }).detach();
}

fn btn(id: &'static str, label: &'static str, strong: bool, light: bool) -> Stateful<Div> {
    let c = |n: &str| theme::color(light, n);
    let b = div().id(id).px_3().py_1().border_1().border_color(c("line")).cursor_pointer().text_size(crate::scale::tpx(13.)).child(trs(label));
    if strong { b.bg(c("raise")) } else { b.text_color(c("dim")) }
}
/// A field's box: its own colour and border, the accent border while it has the keyboard, a click anywhere in it focuses it, and a right-click opens the Paste menu.
fn boxed(this: &Entity<TextInput>, i: usize, focused: bool, light: bool, cx: &mut Context<Backplane>) -> Stateful<Div> {
    let c = |n: &str| theme::color(light, n);
    let h = this.focus_handle(cx);
    div().id(SharedString::from(format!("pair-field-{i}"))).h(crate::scale::tpx(34.)).px_2().flex().items_center().bg(c("panel")).border_1().border_color(c(if focused { "accent" } else { "rule" })).cursor_text()
        .on_mouse_down(MouseButton::Left, move |_, window, cx| { window.focus(&h); cx.stop_propagation(); })
        .on_mouse_down(MouseButton::Right, cx.listener(move |this, e: &MouseDownEvent, window, cx| {
            if let Some(ui) = &mut this.pair { let f = [&ui.link, &ui.addr, &ui.code][i].focus_handle(cx); ui.menu = Some((i, e.position)); window.focus(&f); }
            cx.stop_propagation(); cx.notify();
        }))
        .child(this.clone())
}
/// The menu's Paste, Select all and Clear on a field.
fn menu_do(this: &mut Backplane, i: usize, what: &str, window: &mut Window, cx: &mut Context<Backplane>) {
    let Some(ui) = &mut this.pair else { return };
    ui.menu = None;
    let f = [ui.link.clone(), ui.addr.clone(), ui.code.clone()][i].clone();
    match what {
        "paste" => f.update(cx, |t, cx| t.paste_clipboard(window, cx)),
        "all" => f.update(cx, |t, cx| t.select_everything(cx)),
        _ => f.update(cx, |t, cx| t.set(String::new(), cx)),
    }
    window.focus(&f.focus_handle(cx)); cx.notify();
}
/// Which field has the keyboard (0 link, 1 address, 2 code), for the Tab key and the focus ring.
fn focused_field(ui: &Ui, window: &Window, cx: &App) -> Option<usize> { [&ui.link, &ui.addr, &ui.code].iter().position(|f| f.focus_handle(cx).is_focused(window)) }

pub fn view(this: &mut Backplane, light: bool, window: &mut Window, cx: &mut Context<Backplane>) -> (AnyElement, Vec<crate::script::CtlEntry>) {
    use crate::script::{Ctl, CtlEntry};
    let c = |n: &str| theme::color(light, n);
    let ctl = |id: &str, label: &str, ctl: Ctl| CtlEntry { id: id.into(), label: label.into(), ctl };
    this.term_script(window, cx);
    let ui = this.pair.as_mut().expect("the pair screen has its state");
    if ui.subs.is_empty() {
        for (i, f) in [ui.link.clone(), ui.addr.clone(), ui.code.clone()].iter().enumerate() {
            let h = f.focus_handle(cx);
            ui.subs.push(cx.on_focus_in(&h, window, move |_, _, cx| { note("focus", ["link", "address", "code"][i]); cx.notify() }));
            ui.subs.push(cx.on_focus_out(&h, window, |_, _, _, cx| cx.notify()));
        }
    }
    // (a script's steps start at a hub's first view; with no hub this screen starts them)
    if ui.focus { ui.focus = false; window.focus(&ui.link.focus_handle(cx)); cx.spawn(async move |weak, cx| { let _ = weak.update(cx, |this, cx| this.pump(cx)); }).detach(); }
    let (repair, status, busy, ask, adding) = (ui.repair, ui.status.clone(), ui.busy, ui.ask.clone(), ui.adding);
    let (link, addr, code) = (ui.link.clone(), ui.addr.clone(), ui.code.clone());
    let (focused, menu) = (focused_field(ui, window, cx), ui.menu);
    let mut col = div().w(crate::scale::tpx(560.)).flex().flex_col().gap_3();
    let mut ctls = vec![];
    if let Some(i) = ask {
        col = col.child(div().text_size(crate::scale::tpx(18.)).child(trs("Pair with this hub?"))).child(div().text_size(crate::scale::tpx(13.)).child(i.origin.clone()))
            .child(div().text_size(crate::scale::tpx(12.)).text_color(c("dim")).child(trs("A link opened this. Pair only with a hub you know.")))
            .child(div().flex().gap_2()
                /* i18n-ok: button labels drawn through trs in btn; ctl labels are what drivers press */ .child(btn("pair-cancel", "Cancel", true, light).on_click(cx.listener(|this, _, _, cx| cancel(this, cx))))
                /* i18n-ok: button labels drawn through trs in btn; ctl labels are what drivers press */ .child(btn("pair-yes", "Pair", false, light).on_click(cx.listener(|this, _, _, cx| confirm(this, cx)))));
        /* i18n-ok: button labels drawn through trs in btn; ctl labels are what drivers press */ ctls.push(ctl("pair:cancel", "Cancel", Ctl::PairCancel)); ctls.push(ctl("pair:yes", "Pair", Ctl::PairYes));
    } else {
        /* i18n-ok: drawn through trs */ col = col.child(div().text_size(crate::scale::tpx(18.)).child(trs(if repair { "Re-pair this device" } else { if adding { "Add a hub" } else { "Connect to your Backplane hub" } })))
            /* i18n-ok: drawn through trs */ .child(div().text_size(crate::scale::tpx(12.)).text_color(c("dim")).child(trs(if repair { "This device was revoked. Make a new link in Settings → Devices on the hub." } else { "On the hub's machine: Settings → Devices → Add a device, then paste its link here." })))
            .child(boxed(&link, 0, focused == Some(0), light, cx))
            .child(div().text_size(crate::scale::tpx(12.)).text_color(c("dim")).child(trs("Or the hub's address and short code")))
            .child(div().flex().gap_2().child(boxed(&addr, 1, focused == Some(1), light, cx).flex_1()).child(boxed(&code, 2, focused == Some(2), light, cx).w(crate::scale::tpx(110.))))
            /* i18n-ok: button labels drawn through trs in btn; ctl labels are what drivers press */ .child(div().flex().gap_2().child(btn("pair-go", "Pair", true, light).bg(c("accent")).text_color(c("on-accent")).on_click(cx.listener(|this, _, _, cx| go(this, cx))))
                /* i18n-ok: button labels drawn through trs in btn; ctl labels are what drivers press */ .when(adding, |d| d.child(btn("pair-back", "Cancel", false, light).on_click(cx.listener(|this, _, _, cx| { this.pair = None; cx.notify(); })))));
        /* i18n-ok: button labels drawn through trs in btn; ctl labels are what drivers press */ ctls.push(ctl("pair:go", "Pair", Ctl::PairGo));
    }
    if !status.is_empty() { col = col.child(div().id("pair-status").text_size(crate::scale::tpx(12.)).text_color(c(if busy { "dim" } else { "bad" })).child(trs(&status))); }
    let menu_el = menu.map(|(i, at)| {
        let item = |id: &'static str, label: &'static str, what: &'static str| div().id(id).px_3().py_1().cursor_pointer().text_size(crate::scale::tpx(13.)).hover(|s| s.bg(c("raise"))).child(trs(label))
            .on_click(cx.listener(move |this, _, window, cx| menu_do(this, i, what, window, cx)));
        /* i18n-ok: menu labels drawn through trs */ deferred(anchored().position(at).snap_to_window().child(div().id("pair-menu").flex().flex_col().min_w(crate::scale::tpx(140.)).py_1().bg(c("panel")).border_1().border_color(c("rule"))
            .on_mouse_down_out(cx.listener(|this, _, _, cx| { if let Some(ui) = &mut this.pair { ui.menu = None } cx.notify() }))
            /* i18n-ok: menu labels drawn through trs in item */ .child(item("pair-paste", "Paste", "paste")).child(item("pair-all", "Select all", "all")).child(item("pair-clear", "Clear", "clear")))).with_priority(1)
    });
    let key = cx.listener(move |this, e: &KeyDownEvent, window, cx| {
        let Some(ui) = &mut this.pair else { return };
        if ui.ask.is_some() { return }
        match e.keystroke.key.as_str() {
            "enter" => { ui.menu = None; go(this, cx); cx.stop_propagation() }
            "escape" => { if ui.menu.take().is_some() { cx.notify() } else if ui.adding { this.pair = None; cx.notify() } }
            "tab" => {
                let fs = [ui.link.clone(), ui.addr.clone(), ui.code.clone()];
                let at = focused_field(ui, window, cx).unwrap_or(0);
                let to = if e.keystroke.modifiers.shift { (at + 2) % 3 } else { (at + 1) % 3 };
                window.focus(&fs[to].focus_handle(cx)); cx.stop_propagation(); cx.notify()
            }
            _ => {}
        }
    });
    (div().id("pair-root").size_full().flex().items_center().justify_center().bg(c("bg")).text_color(c("text")).on_key_down(key).child(col).children(menu_el).into_any_element(), ctls)
}
