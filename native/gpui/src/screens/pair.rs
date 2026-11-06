//! The screen an unpaired or revoked client starts at: a pairing link, or a hub address and short code. A link that came from outside
//! (the `backplane://` scheme, a command line) never pairs by itself: it names the hub it would pair with, and Cancel is the way out.
//! A desktop has no camera to speak of, so a QR code is for the phones; every decision here is `crate::pairing`'s.
use crate::{input::TextInput, pairing::{self, Invitation}, theme, Backplane};
use gpui::*;

pub struct Ui { pub link: Entity<TextInput>, pub addr: Entity<TextInput>, pub code: Entity<TextInput>, pub repair: bool, pub status: String, pub busy: bool, pub ask: Option<Invitation>, focus: bool }

fn field(hint: &str, cx: &mut Context<Backplane>) -> Entity<TextInput> { let h: SharedString = hint.to_string().into(); cx.new(|cx| TextInput::new(cx).hint(h)) }
impl Ui {
    pub fn new(repair: bool, cx: &mut Context<Backplane>) -> Ui {
        Ui { link: field("http://host:port/pair#invite=…", cx), addr: field("host:port", cx), code: field("ABCD-2345", cx), repair, status: String::new(), busy: false, ask: None, focus: true }
    }
}
/// A link from outside this window: the question that names its hub, or why it is no link.
fn asked(ui: &mut Ui, link: &str) {
    match pairing::invitation(link) { Some(i) => { ui.ask = Some(i); ui.status.clear() } None => { ui.ask = None; ui.status = "That is not a pairing link. Copy it again from Settings → Devices.".into() } }
}
pub fn start(unpaired: bool, link: Option<&str>, cx: &mut Context<Backplane>) -> Option<Ui> {
    if !unpaired && link.is_none() { return None }
    let mut ui = Ui::new(false, cx);
    if let Some(l) = link { asked(&mut ui, l) }
    Some(ui)
}
/// A link the system handed the running app.
pub fn incoming(this: &mut Backplane, link: &str, cx: &mut Context<Backplane>) {
    if this.pair.is_none() { this.pair = Some(Ui::new(false, cx)); }
    if let Some(ui) = &mut this.pair { asked(ui, link) }
    cx.notify();
}
/// The hub refused this device's credential: it was revoked.
pub fn revoked(this: &mut Backplane, cx: &mut Context<Backplane>) { this.pair = Some(Ui::new(true, cx)); cx.notify(); }
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
    match pairing::submit(&l, &a, &c) { Ok(i) => pair(this, i, cx), Err(e) => { if let Some(ui) = &mut this.pair { ui.status = e.into() } cx.notify() } }
}
/// The question's Pair.
pub fn confirm(this: &mut Backplane, cx: &mut Context<Backplane>) {
    if let Some(i) = this.pair.as_mut().and_then(|u| u.ask.take()) { pair(this, i, cx) }
}
/// The question's Cancel: the form again, or, on a client that has a hub, back to it.
pub fn cancel(this: &mut Backplane, cx: &mut Context<Backplane>) {
    if this.paired { this.pair = None } else if let Some(ui) = &mut this.pair { ui.ask = None }
    cx.notify();
}
fn pair(this: &mut Backplane, i: Invitation, cx: &mut Context<Backplane>) {
    if let Some(ui) = &mut this.pair { ui.busy = true; ui.status = format!("Pairing with {}…", i.origin) }
    cx.notify();
    cx.spawn(async move |weak, cx| {
        let o = i.origin.clone();
        let r = cx.background_executor().spawn(async move { let t = pairing::exchange(&i, &pairing::name())?; pairing::save(&i.origin, &t).map_err(|_| "This pairing could not be saved here.".to_string()) }).await;
        // a failed exchange leaves any existing pairing as it was
        let _ = weak.update(cx, |this, cx| {
            let why = match r { Ok(()) => { eprintln!("GPUI_LOG {}", serde_json::json!({"paired": o})); format!("Could not restart: {}", pairing::relaunch()) } Err(e) => e };
            if let Some(ui) = &mut this.pair { ui.busy = false; ui.status = why }
            cx.notify();
        });
    }).detach();
}

fn btn(id: &'static str, label: &'static str, strong: bool, light: bool) -> Stateful<Div> {
    let c = |n: &str| theme::color(light, n);
    let b = div().id(id).px_3().py_1().border_1().border_color(c("line")).cursor_pointer().text_size(px(13.)).child(label);
    if strong { b.bg(c("raise")) } else { b.text_color(c("dim")) }
}
fn boxed(input: &Entity<TextInput>, light: bool) -> Div { div().h(px(32.)).px_2().flex().items_center().border_1().border_color(theme::color(light, "line")).child(input.clone()) }

pub fn view(this: &mut Backplane, light: bool, window: &mut Window, cx: &mut Context<Backplane>) -> (AnyElement, Vec<crate::script::CtlEntry>) {
    use crate::script::{Ctl, CtlEntry};
    let c = |n: &str| theme::color(light, n);
    let ctl = |id: &str, label: &str, ctl: Ctl| CtlEntry { id: id.into(), label: label.into(), ctl };
    let ui = this.pair.as_mut().expect("the pair screen has its state");
    if ui.focus { ui.focus = false; window.focus(&ui.link.focus_handle(cx)); }
    let (repair, status, busy, ask) = (ui.repair, ui.status.clone(), ui.busy, ui.ask.clone());
    let (link, addr, code) = (ui.link.clone(), ui.addr.clone(), ui.code.clone());
    let mut col = div().w(px(460.)).flex().flex_col().gap_3();
    let mut ctls = vec![];
    if let Some(i) = ask {
        col = col.child(div().text_size(px(18.)).child("Pair with this hub?")).child(div().text_size(px(13.)).child(i.origin.clone()))
            .child(div().text_size(px(12.)).text_color(c("dim")).child("A link opened this. Pair only with a hub you know."))
            .child(div().flex().gap_2()
                .child(btn("pair-cancel", "Cancel", true, light).on_click(cx.listener(|this, _, _, cx| cancel(this, cx))))
                .child(btn("pair-yes", "Pair", false, light).on_click(cx.listener(|this, _, _, cx| confirm(this, cx)))));
        ctls.push(ctl("pair:cancel", "Cancel", Ctl::PairCancel)); ctls.push(ctl("pair:yes", "Pair", Ctl::PairYes));
    } else {
        col = col.child(div().text_size(px(18.)).child(if repair { "Re-pair this device" } else { "Connect to your Backplane hub" }))
            .child(div().text_size(px(12.)).text_color(c("dim")).child(if repair { "This device was revoked. Make a new link in Settings → Devices on the hub." } else { "On a paired machine: Settings → Devices → Add a device." }))
            .child(boxed(&link, light))
            .child(div().text_size(px(12.)).text_color(c("dim")).child("Or the hub's address and short code"))
            .child(div().flex().gap_2().child(boxed(&addr, light).flex_1()).child(boxed(&code, light).w(px(130.))))
            .child(div().flex().child(btn("pair-go", "Pair", true, light).on_click(cx.listener(|this, _, _, cx| go(this, cx)))));
        ctls.push(ctl("pair:go", "Pair", Ctl::PairGo));
    }
    if !status.is_empty() { col = col.child(div().text_size(px(12.)).text_color(c(if busy { "dim" } else { "text" })).child(status)); }
    (div().size_full().flex().items_center().justify_center().bg(c("bg")).text_color(c("text")).child(col).into_any_element(), ctls)
}
