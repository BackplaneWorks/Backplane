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
    /// when the person pressed this row's Install/Update, and when they last did anything in Backplane (a click, a key): the reopen after
    /// the install hands KiCad the keyboard only if they are still there for it (`kicad_pane::reattach_why`)
    pub pressed_at: Option<std::time::Instant>, pub person_at: Option<std::time::Instant>,
}

impl Update {
    /// BACKPLANE_GPUI_KICAD_UPDATE=<the kicad.update JSON>: an offer to start with (the e2e's: the row's place is checked without a hub
    /// that offers one); otherwise none until the hub says.
    pub fn from_env() -> Update { Update { offer: std::env::var("BACKPLANE_GPUI_KICAD_UPDATE").ok().map_or(Value::Null, |t| parse(&t)), ..Default::default() } }
}

/// What the row shows now.
#[derive(Clone, Debug, PartialEq)]
pub enum Row { None, Offer { to: String, words: String }, Progress { words: String, fraction: Option<f32> }, Failed(String), Reopen(String),
    /// no KiCad found, a package for this computer: Install KiCad (the person's press is the consent) and Set a path…
    Missing { words: String },
    /// no KiCad found and none published for this computer: why, and Set a path…
    NoPackage(String) }

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

/// The offer when no KiCad is found (`kicad.update` {missing, to, size, folder, admin}), in words. Pure.
pub fn missing_words(offer: &Value) -> String {
    let (v, size, folder) = (s(offer, "to"), s(offer, "size"), s(offer, "folder"));
    let key = if offer["admin"] == false { "gpui.kicad.update.missing" } else { "gpui.kicad.update.missing.admin" };
    crate::i18n::tf(key, &[("version", v.into()), ("size", size.into()), ("folder", folder.into())])
}
/// The row from the hub's two infos, whether "Not now" holds for its version now, and whether the pane's KiCad has unsaved edits.
/// `edited`: the person pressed Edit (or Open in KiCad, Try again) on the pane's page: only then is a missing KiCad offered (never on
/// open, an agent's show, an automatic attach or a tile layout restored).
pub fn row(offer: &Value, install: &Value, refused: &str, later: bool, unsaved: bool) -> Row { row_for(offer, install, refused, later, unsaved, true) }
pub fn row_for(offer: &Value, install: &Value, refused: &str, later: bool, unsaved: bool, edited: bool) -> Row {
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
    // no KiCad: offered only after the person's Edit, and "Not now" does not apply (there is nothing to edit with without it)
    if offer["missing"] == true {
        if !edited { return Row::None }
        if offer["nopackage"] == true { return Row::NoPackage(s(offer, "words")) }
        if !to.is_empty() { return Row::Missing { words: missing_words(offer) } }
        return Row::None
    }
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
    app.screens.kicad_update.pressed_at = Some(std::time::Instant::now());
    let _ = app.tx.try_send(Command::Rpc("kicad.install".into(), Secret(json!({"consent": true})), "kicad"));
}
/// Set a path…: Settings → CAD, where the KiCad path field is (no dialog of its own).
fn set_path(app: &mut Backplane, cx: &mut Context<Backplane>) {
    if !(app.screens.settings_ui.open_local || app.data["settings"].is_object()) { app.open_settings(cx); }
    crate::screens::settings::go_page(app, "cad");
}
/// Whether the person did anything in Backplane after the Install press (or the Install was not this row's: pressed in Settings). Pure.
pub fn turned_since(pressed: Option<std::time::Instant>, person: Option<std::time::Instant>) -> bool {
    match (pressed, person) { (None, _) => true, (Some(p), Some(a)) => a > p, (Some(_), None) => false }
}
/// A click or a key in Backplane (ui.rs): noted for `turned_since`.
pub fn person_acted(app: &mut Backplane) { app.screens.kicad_update.person_at = Some(std::time::Instant::now()); }
fn reopen(app: &mut Backplane, cx: &mut Context<Backplane>) {
    app.screens.kicad_update.reopened = true;
    if let Some(p) = app.screens.kicad.clone() {
        // a pane whose Edit had no KiCad in it (refused: "KiCad was not found", before the install) attaches with its own container: a
        // restart has no container to start KiCad in and started it in a window of its own (bp.13.3 first install). A pane on a running
        // KiCad keeps the restart, which keeps that session's container
        if p.read(cx).is_adrift() {
            use crate::viewers::kicad_pane::{reattach_why, AdriftRoad};
            let thread = app.data["thread"].clone();
            let on_screen = app.kicad_for(&thread, cx).as_ref() == Some(&p);
            let u = &app.screens.kicad_update;
            let turned = turned_since(u.pressed_at, u.person_at);
            let why = reattach_why(AdriftRoad::InstallReopen, on_screen, turned);
            // not its project on screen: nothing now, and not asked again (the pane says KiCad could not start; the person's Edit there, or its
            // Try again, works now)
            app.kicad_adrift(&p, why, false, cx);
            return
        }
        let project = p.read(cx).project.clone();
        let _ = app.tx.try_send(Command::Rpc("editor.kicad.restart".into(), Secret(json!({"project": project})), "kicad"));
    }
}

/// The row over the pane (None: nothing to say). Its controls: kicad-update, kicad-update:go / later / retry / reopen.
pub fn view(app: &mut Backplane, cx: &mut Context<Backplane>) -> Option<AnyElement> {
    let unsaved = unsaved(app, cx);
    // the person's own Edit on this pane (a deliberate pane): a missing KiCad is offered only then
    let edited = app.screens.kicad.as_ref().is_some_and(|p| p.read(cx).why == crate::viewers::kicad_pane::ShowBy::Deliberate);
    let u = &app.screens.kicad_update;
    let r = row_for(&u.offer, &u.install, &u.refused, put_off(&app.local.kicad_update_later, &s(&u.offer, "to"), now_s()), unsaved, edited);
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
        Row::Missing { words } => {
            row_el = row_el.child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(words))
                .child(btn("kicad-update:install", crate::i18n::t("gpui.kicad.update.install"), true).on_click(cx.listener(|this, _, _, cx| { install(this); cx.notify(); })))
                .child(btn("kicad-update:path", crate::i18n::t("gpui.kicad.update.set.path"), false).on_click(cx.listener(|this, _, _, cx| { set_path(this, cx); cx.notify(); })));
        }
        Row::NoPackage(why) => {
            row_el = row_el.child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(why))
                .child(btn("kicad-update:path", crate::i18n::t("gpui.kicad.update.set.path"), true).on_click(cx.listener(|this, _, _, cx| { set_path(this, cx); cx.notify(); })));
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
    #[test] fn the_person_turned_away_from_the_install_if_they_did_anything_since_its_press() {
        let t0 = std::time::Instant::now(); let later = t0 + std::time::Duration::from_secs(5); let before = t0 - std::time::Duration::from_secs(5);
        assert!(!turned_since(Some(t0), None), "nothing done since the press: still there");
        assert!(!turned_since(Some(t0), Some(before)), "the last act was before the press (the press's own click comes first)");
        assert!(turned_since(Some(t0), Some(later)), "a click or a key after the press: turned away");
        assert!(turned_since(None, Some(later)), "no press of this row's (an Install in Settings, then closing it): taken as turned");
        assert!(turned_since(None, None));
    }
    use core::prelude::v1::test;
    fn offer() -> Value { json!({"offer": true, "from": "10.99.0-bp.12.4", "to": "10.99.0-bp.13", "reason": "Backplane KiCad bp.13 is out: panels that fit the pane", "mb": 1083, "size": "about 1.1 GB"}) }
    #[test] fn a_missing_kicad_is_offered_only_after_the_persons_edit_and_only_their_press_installs() {
        let m = json!({"missing": true, "to": "10.99.0-bp.13.3", "size": "about 970 MB", "mb": 972, "folder": "~/.backplane/kicad", "admin": false});
        // an automatic attach, an agent's show, a pre-warm, a tile restore: the pane is not the person's (`edited` false): no row, nothing sent
        assert_eq!(row_for(&m, &Value::Null, "", false, false, false), Row::None);
        let Row::Missing { words } = row_for(&m, &Value::Null, "", false, false, true) else { panic!("after Edit: the offer") };
        assert!(words.contains("10.99.0-bp.13.3") && words.contains("about 970 MB") && words.contains("~/.backplane/kicad") && words.contains("administrator"), "{words}");
        assert!(matches!(row_for(&m, &Value::Null, "", true, false, true), Row::Missing { .. }), "Not now does not put a missing KiCad away");
        // where an administrator's password would be needed the offer does not say "no administrator password"
        let Row::Missing { words: w2 } = row_for(&json!({"missing": true, "to": "v", "size": "s", "folder": "f", "admin": true}), &Value::Null, "", false, false, true) else { panic!() };
        assert!(!w2.contains("no administrator"), "{w2}");
        // no package: why, and Set a path
        assert_eq!(row_for(&json!({"missing": true, "nopackage": true, "words": "No Backplane KiCad is published for this computer (linux aarch64)"}), &Value::Null, "", false, false, true),
            Row::NoPackage("No Backplane KiCad is published for this computer (linux aarch64)".into()));
        // pressed: running, refused for space or offline, failed: the hub's words and Try again
        assert!(matches!(row_for(&m, &json!({"state": "running", "note": "Downloading", "phase": "download", "done": 1, "total": 4}), "", false, false, true), Row::Progress { fraction: Some(f), .. } if (f - 0.25).abs() < 1e-6));
        assert_eq!(row_for(&m, &Value::Null, "Not enough free space for KiCad: it needs about 6.4 GB in ~/.backplane/kicad, and 2.0 GB is free", false, false, true),
            Row::Failed("Not enough free space for KiCad: it needs about 6.4 GB in ~/.backplane/kicad, and 2.0 GB is free".into()));
        assert_eq!(row_for(&m, &json!({"state": "failed", "note": "The download stopped: the network is unreachable"}), "", false, false, true), Row::Failed("The download stopped: the network is unreachable".into()), "offline");
        assert_eq!(row_for(&m, &json!({"state": "failed", "note": "KiCad install failed: checksum"}), "", false, false, true), Row::Failed("KiCad install failed: checksum".into()));
        // an install running started elsewhere (Settings) still shows its progress on a pane that is not the person's
        assert!(matches!(row_for(&m, &json!({"state": "running", "note": "Unpacking"}), "", false, false, false), Row::Progress { .. }));
    }
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
