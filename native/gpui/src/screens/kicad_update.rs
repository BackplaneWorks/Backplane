//! The Backplane KiCad update at Edit (0.16.0): a row of its own above the KiCad pane, never drawn over the pane's area (on macOS KiCad's
//! frame is another process's window above Backplane's: anything over the pane is hidden under it). The hub says what is offered
//! (info `kicad.update`: {offer, from, to, reason, mb, size}, "" for none) and how an install goes (info `kicad.install`: {state:
//! running|failed|done, version, note, and while downloading phase, done, total, rate, eta}); its words are shown as given. Update sends
//! `kicad.install {consent: true}`; "Not now" is kept per version on this device; when the install is done the pane's KiCad is restarted
//! on the new runtime (`editor.kicad.restart`), at once when nothing in it is unsaved, else on the person's "Reopen".
use crate::{hub::Command, i18n::trs, screens::settings::Secret, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};

#[derive(Default)]
pub struct Update {
    /// the hub's offer and install progress, as published
    pub offer: Value, pub install: Value,
    /// an install the hub refused at once (its reply's words)
    pub refused: String,
    /// the restart onto the new runtime was asked for (once per install)
    pub reopened: bool,
}

impl Update {
    /// BACKPLANE_GPUI_KICAD_UPDATE=<the kicad.update JSON>: an offer to start with (the e2e's: the row's place is checked without a hub
    /// that offers one); otherwise none until the hub says.
    pub fn from_env() -> Update { Update { offer: std::env::var("BACKPLANE_GPUI_KICAD_UPDATE").ok().map_or(Value::Null, |t| parse(&t)), ..Default::default() } }
}

/// What the row shows now.
#[derive(Clone, Debug, PartialEq)]
pub enum Row { None, Offer { to: String, words: String }, Progress { words: String, fraction: Option<f32> }, Failed(String), Reopen(String) }

fn s(v: &Value, k: &str) -> String { v[k].as_str().unwrap_or("").to_string() }
fn parse(text: &str) -> Value { serde_json::from_str(text).unwrap_or(Value::Null) }

/// How long "Not now" puts the offer away: it comes back after a day (Settings → CAD offers it always).
pub const LATER_S: u64 = 24 * 3600;
/// Whether "Not now" (kept as "<version>|<epoch s>") still holds for `to` at `now` (epoch s).
pub fn put_off(later: &str, to: &str, now: u64) -> bool {
    let (v, at) = later.split_once('|').unwrap_or((later, "0"));
    v == to && at.parse::<u64>().is_ok_and(|t| now.saturating_sub(t) < LATER_S)
}
fn now_s() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) }

/// The row from the hub's two infos, whether "Not now" holds for its version now, and whether the pane's KiCad has unsaved edits.
pub fn row(offer: &Value, install: &Value, refused: &str, later: bool, unsaved: bool) -> Row {
    let to = s(offer, "to");
    match install["state"].as_str().unwrap_or("") {
        "running" => {
            let (done, total) = (install["done"].as_f64().unwrap_or(0.), install["total"].as_f64().unwrap_or(0.));
            let fraction = (install["phase"] == "download" && total > 0.).then(|| (done / total).clamp(0., 1.) as f32);
            return Row::Progress { words: s(install, "note"), fraction };
        }
        "failed" => return Row::Failed(s(install, "note")),
        // installed: KiCad reopens on it by itself, unless something in it is unsaved (the person's Reopen then)
        "done" if unsaved && offer["offer"] != true => return Row::Reopen(s(install, "version")),
        _ => {}
    }
    if !refused.is_empty() { return Row::Failed(refused.to_string()) }
    if offer["offer"] != true || to.is_empty() || later { return Row::None }
    let size = s(offer, "size");
    let words = if size.is_empty() { s(offer, "reason") } else { format!("{} ({size})", s(offer, "reason")) }; // i18n-ok: the hub's words joined
    Row::Offer { to, words }
}

/// The hub published `kicad.update` or `kicad.install`.
pub fn seen(app: &mut Backplane, key: &str, text: &str, cx: &mut Context<Backplane>) {
    let v = parse(text);
    let u = &mut app.screens.kicad_update;
    if key == "kicad.update" { u.offer = v; } else {
        let fresh = v["state"] == "running" && u.install["state"] != "running";
        if fresh { u.reopened = false; u.refused.clear(); }
        u.install = v;
    }
    // installed, nothing unsaved in the pane's KiCad: it is reopened on the new runtime now
    if app.screens.kicad_update.install["state"] == "done" && !app.screens.kicad_update.reopened && !unsaved(app, cx) { reopen(app, cx); }
    cx.notify();
}
/// The hub answered `kicad.install` (a refusal comes at once: "KiCad is being installed already").
pub fn replied(app: &mut Backplane, reply: &Value, cx: &mut Context<Backplane>) {
    app.screens.kicad_update.refused = if reply["ok"] == true { String::new() } else { crate::hub::rpc_error(reply) };
    cx.notify();
}

fn unsaved(app: &Backplane, cx: &App) -> bool {
    app.screens.kicad.as_ref().is_some_and(|p| p.read(cx).info["frames"].as_array().into_iter().flatten().any(|f| f["modified"] == true))
}
fn install(app: &mut Backplane) {
    app.screens.kicad_update.refused.clear();
    let _ = app.tx.try_send(Command::Rpc("kicad.install".into(), Secret(json!({"consent": true})), "kicad"));
}
fn reopen(app: &mut Backplane, cx: &mut Context<Backplane>) {
    app.screens.kicad_update.reopened = true;
    if let Some(p) = app.screens.kicad.as_ref() {
        let project = p.read(cx).project.clone();
        let _ = app.tx.try_send(Command::Rpc("editor.kicad.restart".into(), Secret(json!({"project": project})), "kicad"));
    }
}

/// The row over the pane (None: nothing to say). Its controls: kicad-update, kicad-update:go / later / retry / reopen.
pub fn view(app: &mut Backplane, cx: &mut Context<Backplane>) -> Option<AnyElement> {
    let unsaved = unsaved(app, cx);
    let u = &app.screens.kicad_update;
    let r = row(&u.offer, &u.install, &u.refused, put_off(&app.local.kicad_update_later, &s(&u.offer, "to"), now_s()), unsaved);
    if r == Row::None { return None }
    let light = app.light();
    let c = |n: &str| theme::color(light, n);
    let btn = |id: &'static str, label: String, primary: bool| crate::ui::with_rect(div().id(id), move || id.to_string()).flex_shrink_0().px_2().py(px(2.)).border_1()
        .border_color(c(if primary { "accent" } else { "line" })).bg(c(if primary { "accent" } else { "raise" })).text_color(c(if primary { "bg" } else { "text" }))
        .cursor_pointer().child(label);
    let mut row_el = crate::ui::with_rect(div().id("kicad-update"), || "kicad-update".to_string()).flex_shrink_0().flex().items_center().gap_2().px_2().py(px(4.))
        .border_b_1().border_color(c("line")).bg(c("panel")).text_size(crate::scale::tpx(12.));
    match r {
        Row::Offer { to, words } => {
            row_el = row_el.child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(words))
                .child(btn("kicad-update:go", trs("Update"), true).on_click(cx.listener(|this, _, _, cx| { install(this); cx.notify(); })))
                .child(btn("kicad-update:later", trs("Not now"), false).on_click(cx.listener(move |this, _, _, cx| { this.local.kicad_update_later = format!("{to}|{}", now_s()); this.local.save(); cx.notify(); })));
        }
        Row::Progress { words, fraction } => {
            row_el = row_el.child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(words))
                .children(fraction.map(|f| div().w(px(160.)).h(px(4.)).bg(c("raise")).child(div().h_full().bg(c("accent")).w(relative(f)))));
        }
        Row::Failed(why) => {
            row_el = row_el.child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(c("bad")).child(why))
                .child(btn("kicad-update:retry", trs("Try again"), true).on_click(cx.listener(|this, _, _, cx| { install(this); cx.notify(); })));
        }
        Row::Reopen(version) => {
            row_el = row_el.child(div().flex_1().min_w_0().child(crate::i18n::tf("gpui.kicad.update.installed", &[("version", version.into())])))
                .child(btn("kicad-update:reopen", trs("Reopen"), true).on_click(cx.listener(|this, _, _, cx| { reopen(this, cx); cx.notify(); })));
        }
        Row::None => {}
    }
    Some(row_el.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn offer() -> Value { json!({"offer": true, "from": "10.99.0-bp.12.4", "to": "10.99.0-bp.13", "reason": "Backplane KiCad bp.13 is out: panels that fit the pane", "mb": 1083, "size": "about 1.1 GB"}) }
    #[test] fn the_update_row_says_what_the_hub_says_and_goes_when_put_off() {
        assert_eq!(row(&offer(), &Value::Null, "", false, false), Row::Offer { to: "10.99.0-bp.13".into(), words: "Backplane KiCad bp.13 is out: panels that fit the pane (about 1.1 GB)".into() });
        assert_eq!(row(&offer(), &Value::Null, "", true, false), Row::None, "Not now");
        let t = 1_800_000_000;
        assert!(put_off(&format!("10.99.0-bp.13|{t}"), "10.99.0-bp.13", t + 3600), "within the day");
        assert!(!put_off(&format!("10.99.0-bp.13|{t}"), "10.99.0-bp.13", t + LATER_S), "a day later: offered again");
        assert!(!put_off(&format!("10.99.0-bp.13|{t}"), "10.99.0-bp.14", t + 60), "a newer one: offered at once");
        assert!(!put_off("10.99.0-bp.13", "10.99.0-bp.13", t), "an old setting without its day: offered");
        assert_eq!(row(&json!(""), &Value::Null, "", false, false), Row::None, "no offer");
        let dl = json!({"state": "running", "version": "10.99.0-bp.13", "note": "Downloading…", "phase": "download", "done": 250, "total": 1000});
        assert_eq!(row(&offer(), &dl, "", false, false), Row::Progress { words: "Downloading…".into(), fraction: Some(0.25) });
        assert_eq!(row(&offer(), &json!({"state": "running", "note": "Unpacking…"}), "", false, false), Row::Progress { words: "Unpacking…".into(), fraction: None });
        assert_eq!(row(&offer(), &json!({"state": "failed", "note": "no space"}), "", false, false), Row::Failed("no space".into()));
        assert_eq!(row(&offer(), &Value::Null, "KiCad is being installed already", false, false), Row::Failed("KiCad is being installed already".into()));
        let done = json!({"state": "done", "version": "10.99.0-bp.13", "note": "Installed"});
        assert_eq!(row(&json!(""), &done, "", false, false), Row::None, "installed, nothing unsaved: reopened by itself, nothing to say");
        assert_eq!(row(&json!(""), &done, "", false, true), Row::Reopen("10.99.0-bp.13".into()), "unsaved edits: the person reopens it");
    }
}
