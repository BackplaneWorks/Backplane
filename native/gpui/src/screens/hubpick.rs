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
/// Resolve the active hub from `list` to `(ws_url, token_file, paired)` for in-process switching.
/// Returns `None` if switching to the local hub and no local token file exists yet (so `restart` can launch it).
pub fn resolve_target(list: &[hubs::Hub]) -> Option<(String, std::path::PathBuf, bool)> {
    resolve_target_with(
        list,
        |k| hubs::credential_saved(k).filter(|p| p.is_file()),
        || crate::plat::user_home().map(|h| std::path::PathBuf::from(h).join(".backplane-gpui/token")).filter(|p| p.is_file()),
    )
}
fn resolve_target_with(
    list: &[hubs::Hub],
    cred_for: impl Fn(&str) -> Option<std::path::PathBuf>,
    default_local_token: impl Fn() -> Option<std::path::PathBuf>,
) -> Option<(String, std::path::PathBuf, bool)> {
    match hubs::target(list, |k| cred_for(k).is_some()) {
        hubs::Target::Remote { key, ws } => Some((ws, cred_for(&key)?, true)),
        hubs::Target::Local => {
            if let Some((origin, token)) = hubs::local_target(list) {
                return Some((crate::pairing::ws_of(&origin), token, false));
            }
            let token = default_local_token()?;
            Some(("ws://127.0.0.1:3799".into(), token, false))
        }
    }
}
/// Switch to the first hub in `this.hubs` in-process (zero OS relaunch) with 0 ms cached paint when
/// returning to a previously visited hub; falls back to `restart` only when a local hub must be launched.
fn switch_hub(this: &mut Backplane, prev_key: &str, cx: &mut Context<Backplane>) {
    if this.pair.is_none() {
        if let Some((ws, token_file, paired)) = resolve_target(&this.hubs) {
            let new_key = this.hubs.first().map(|h| h.key.clone()).unwrap_or_default();
            if !prev_key.is_empty() && this.data.is_object() {
                if !this.screens.offers.hub_cache.contains_key(prev_key) {
                    if this.screens.offers.hub_cache_order.len() >= 8 {
                        if let Some(old) = this.screens.offers.hub_cache_order.pop_front() {
                            this.screens.offers.hub_cache.remove(&old);
                        }
                    }
                    this.screens.offers.hub_cache_order.push_back(prev_key.to_string());
                }
                this.screens.offers.hub_cache.insert(prev_key.to_string(), this.data.clone());
            }
            crate::viewers::kicad_pane::release_all();
            this.screens.drafts.flush();
            // Retarget every per-hub HTTP target, credential-beside file, and watcher so attachments,
            // images, improvements, drafts, place and hubbar all follow the new hub (no leak to the old hub).
            let new_target = crate::screens::attach::Target::from(&ws, &token_file);
            let drafts_path = crate::screens::place::beside_credential(&token_file, "gpui-drafts.json");
            let saved_drafts = crate::screens::drafts::load(&drafts_path);
            this.screens.attacher.target = new_target.clone();
            this.screens.attacher.show("");
            this.screens.images = Default::default();
            crate::screens::place::retarget(&token_file);
            this.screens.drafts.retarget(drafts_path.clone(), saved_drafts.clone());
            this.drafts = saved_drafts;
            this.screens.imp.retarget(&drafts_path, new_target.clone());
            this.screens.unsent = crate::screens::unsent::Unsent::open(drafts_path.with_file_name("gpui-unsent.json"));
            crate::screens::hubbar::watch(cx, new_target, &token_file);
            this.paired = paired;
            this.screens.elsewhere = hubs::elsewhere(&ws);
            this.screens.offers.on_home = None;
            this.error.clear();
            this.note.clear();
            if let Some(cached) = this.screens.offers.hub_cache.get(&new_key).cloned() {
                this.data = cached;
                this.ui_epoch += 1;
            } else {
                this.data = serde_json::Value::Null;
                this.selected.clear();
            }
            /* i18n-ok: connection status, compared with Connected; drawn through trs */ this.status = "Connecting…".into();
            note("switch", &new_key);
            let _ = this.tx.try_send(crate::hub::Command::SwitchHub(ws, token_file, paired));
            cx.notify();
            return;
        }
    }
    restart(this, cx);
}
/// Start again on the saved list, whose first hub is the one to connect to. Returns only when that failed.
fn restart(this: &mut Backplane, cx: &mut Context<Backplane>) {
    note("restart", this.hubs.first().map_or("", |h| h.key.as_str()));
    let e = crate::pairing::relaunch();
    this.status = crate::i18n::tf("gpui.could.not.restart", &[("why", e.to_string().into())]); cx.notify();
}
pub fn act(this: &mut Backplane, a: Act, cx: &mut Context<Backplane>) {
    match a {
        Act::Menu => { this.hub_menu = !this.hub_menu; this.screens.offers.forget_arm = None; cx.notify(); }
        Act::Add => crate::screens::pair::add(this, cx),
        Act::Use(k) => {
            this.hub_menu = false;
            let prev = this.hubs.first().map(|h| h.key.clone()).unwrap_or_default();
            match hubs::use_saved(&k) {
                Ok(Some(list)) => { this.hubs = list; note("use", &k); switch_hub(this, &prev, cx) }
                Ok(None) => cx.notify(),
                Err(_) => { this.status = crate::i18n::trs("The choice could not be saved here."); cx.notify() }
            }
        }
        Act::Forget(k) => {
            let prev = this.hubs.first().map(|h| h.key.clone()).unwrap_or_default();
            this.screens.offers.hub_cache.remove(&k);
            match hubs::forget_saved(&k) {
                Ok(Some((list, moved))) => { this.hubs = list; note("forget", &k); if moved { switch_hub(this, &prev, cx) } else { cx.notify() } }
                Ok(None) => cx.notify(),
                Err(_) => { this.status = crate::i18n::trs("The hub could not be forgotten here."); cx.notify() }
            }
        }
    }
}

// ---- the home hub's offer (docs/pairing.md, Home hub): seen in info `hub.offer`, carried out with no dialog, reported with hub.offer.done ----

/// The tags of hub.offer.take's and hub.offer.done's answers.
pub const TAKE: u64 = 0x0f_0ff3_0001;
pub const DONE: u64 = 0x0f_0ff3_0002;
/// How often an offer is tried before it is reported failed, and the wait before each try again.
const TRIES: u32 = 3;
fn backoff(tried: u32) -> std::time::Duration { std::time::Duration::from_secs(2u64 << tried.min(4)) }
#[derive(Default)]
pub struct Offers {
    /// the offer being carried out, and how many times it was tried
    pub doing: Option<(hubs::Offer, u32)>,
    /// offers answered with hub.offer.done this run
    pub done: Vec<String>,
    /// the home hub's key while the connection failed over to it
    pub on_home: Option<String>,
    /// the picker's Forget pressed once on this hub: a second press forgets it
    pub forget_arm: Option<String>,
    /// last rendered picture per hub key, so switching back to a hub paints at 0 ms while reconnecting
    pub hub_cache: std::collections::HashMap<String, serde_json::Value>,
    /// FIFO insertion order for bounded `hub_cache` eviction
    pub hub_cache_order: std::collections::VecDeque<String>,
}
/// The device token this client holds for the hub in use: a paired hub's credential; None on this computer's own hub (its home token: the client takes a link).
fn token_in_use(this: &Backplane) -> Option<String> {
    let h = this.hubs.first().filter(|h| !h.is_local())?;
    if this.screens.elsewhere.as_ref().map_or(false, |e| !h.key.starts_with(&e.1)) { return None }
    hubs::credential_saved(&h.key).and_then(|p| std::fs::read_to_string(p).ok()).map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}
/// The hub's offer, as it comes (at connect and on change): what it asks is decided by `hubs::plan`, and done here without a word to the person.
pub fn offered(this: &mut Backplane, text: &str, cx: &mut Context<Backplane>) {
    if this.screens.offers.on_home.is_some() || this.screens.offers.doing.is_some() { return } // only the hub in use offers; one at a time
    let Some(o) = hubs::offer(text) else { return };
    let token = token_in_use(this);
    let p = hubs::plan(text, &this.hubs, token.as_deref(), &this.screens.offers.done, |h| Some(h.key.clone()));
    note("offer", &format!("{} {:?}", o.url, std::mem::discriminant(&p)));
    match p {
        hubs::Plan::None => {}
        hubs::Plan::Known(k) => {
            if o.why == "home" { if let Ok(Some(l)) = hubs::home_saved(&k) { this.hubs = l; note("home", &k); } }
            done(this, &o.id, true);
            cx.notify();
        }
        hubs::Plan::Same { .. } => { let t = token.unwrap_or_default(); this.screens.offers.doing = Some((o.clone(), 0)); same(o, t, 0, cx) }
        hubs::Plan::Take(_) => { this.screens.offers.doing = Some((o.clone(), 0)); take(this, &o) }
    }
}
fn take(this: &mut Backplane, o: &hubs::Offer) { let _ = this.tx.try_send(crate::hub::Command::Ask("hub.offer.take".into(), serde_json::json!({"id": o.id}), TAKE)); }
fn done(this: &mut Backplane, id: &str, ok: bool) {
    this.screens.offers.doing = None;
    if !this.screens.offers.done.iter().any(|d| d == id) { this.screens.offers.done.push(id.to_string()) }
    note(if ok { "offer-done" } else { "offer-failed" }, id);
    let _ = this.tx.try_send(crate::hub::Command::Ask("hub.offer.done".into(), serde_json::json!({"id": id, "ok": ok}), DONE));
}
/// The same device token at the offered hub: kept once that hub takes it.
fn same(o: hubs::Offer, token: String, tried: u32, cx: &mut Context<Backplane>) {
    cx.spawn(async move |weak, cx| {
        if tried > 0 { cx.background_executor().timer(backoff(tried)).await; }
        let (url, t, home) = (o.url.clone(), token.clone(), o.why == "home");
        let r = cx.background_executor().spawn(async move { crate::pairing::takes(&url, &t)?; hubs::add_saved(&url, &t, home).map_err(|e| e.to_string()) }).await;
        let _ = weak.update(cx, |this, cx| finish(this, o, r, Some(token), tried, cx));
    }).detach();
}
/// The link hub.offer.take answered, redeemed as a scanned code is (pairing::exchange), kept without switching.
fn redeem(o: hubs::Offer, link: String, tried: u32, cx: &mut Context<Backplane>) {
    cx.spawn(async move |weak, cx| {
        let home = o.why == "home";
        let r = cx.background_executor().spawn(async move {
            /* i18n-ok: logged, never shown */ let i = crate::pairing::invitation(&link).ok_or_else(|| "not a pairing link".to_string())?;
            let (at, t) = crate::pairing::exchange(&i, &crate::pairing::name())?;
            hubs::add_saved(&at, &t, home).map_err(|e| e.to_string())
        }).await;
        let _ = weak.update(cx, |this, cx| finish(this, o, r, None, tried, cx));
    }).detach();
}
/// One try's end: kept (done ok), or tried again later (a take asks for a new link: each is single-use), or after the last try reported failed.
fn finish(this: &mut Backplane, o: hubs::Offer, r: Result<Vec<hubs::Hub>, String>, token: Option<String>, tried: u32, cx: &mut Context<Backplane>) {
    match r {
        Ok(l) => { this.hubs = l; note("added", &o.url); done(this, &o.id, true) }
        Err(e) if tried + 1 < TRIES => {
            note("offer-retry", &e);
            this.screens.offers.doing = Some((o.clone(), tried + 1));
            match token {
                Some(t) => same(o, t, tried + 1, cx),
                None => cx.spawn(async move |weak, cx| { cx.background_executor().timer(backoff(tried + 1)).await; let _ = weak.update(cx, |this, _| take(this, &o)); }).detach(),
            }
        }
        Err(e) => { note("offer-error", &e); done(this, &o.id, false) }
    }
    cx.notify();
}
/// hub.offer.take's and hub.offer.done's answers; false for any other.
pub fn answered(this: &mut Backplane, tag: u64, reply: &serde_json::Value, cx: &mut Context<Backplane>) -> bool {
    match tag {
        DONE => true,
        TAKE => {
            let Some((o, tried)) = this.screens.offers.doing.clone() else { return true };
            match reply["result"]["link"].as_str().filter(|_| reply["ok"] == true) {
                Some(link) => redeem(o, link.to_string(), tried, cx),
                None => finish(this, o, Err(crate::hub::rpc_error(reply)), None, tried, cx),
            }
            true
        }
        _ => false,
    }
}

/// The Hubs page of Settings.
pub fn rows(list: &[hubs::Hub]) -> Vec<Row> { rows_on(list, None) }
/// The page with the hub connected to now (`on_home`: the home's key while the client failed over to it).
pub fn rows_on(list: &[hubs::Hub], on_home: Option<&str>) -> Vec<Row> {
    let b = |label: &str, action: String, on: bool| Btn { label: label.into(), action, value: String::new(), on };
    let mut out: Vec<Row> = entries_rows(&hubs::entries_on(list, on_home));
    /* i18n-ok: settings rows, drawn through trs by row_el */ out.push(Row { label: "Add a hub".into(), note: "Pair this device with a hub on another machine.".into(), buttons: vec![b("Add a hub", "hub:add".into(), false)], fields: vec![], qr: vec![], qrnote: String::new(), local: true, tone: String::new(), ..Default::default() });
    out
}
fn entries_rows(es: &[Entry]) -> Vec<Row> {
    es.iter().map(|e| {
        let mut buttons = vec![];
        /* i18n-ok: settings rows, drawn through trs by row_el */ if e.on { buttons.push(Btn { label: "In use".into(), action: String::new(), value: String::new(), on: true }) } else { buttons.push(Btn { label: "Use".into(), action: format!("hub:use:{}", e.key), value: String::new(), on: false }) }
        /* i18n-ok: settings rows, drawn through trs by row_el */ if e.can_forget { buttons.push(Btn { label: "Forget".into(), action: format!("hub:forget:{}", e.key), value: String::new(), on: false }) }
        /* i18n-ok: settings rows, drawn through trs by row_el */ Row { label: e.label.clone(), note: note_of(e).into(), buttons, fields: vec![], qr: vec![], qrnote: String::new(), local: true, tone: String::new(), ..Default::default() }
    }).collect()
}

/* i18n-ok: settings notes, drawn through trs by row_el */
fn note_of(e: &Entry) -> &'static str {
    match (e.key == hubs::LOCAL, e.home, e.on && !e.in_use, e.in_use && !e.on) {
        /* i18n-ok: a note, drawn through trs */ (_, true, true, _) => "Home hub. In use while the hub you chose cannot be reached.",
        /* i18n-ok: a note, drawn through trs */ (_, true, _, _) => "Home hub. Used when the hub in use cannot be reached.",
        /* i18n-ok: a note, drawn through trs */ (_, _, _, true) => "Cannot be reached: the home hub is in use until it answers.",
        /* i18n-ok: a note, drawn through trs */ (true, ..) => "The hub on this computer.",
        _ => "",
    }
}
/// The top of the sidebar: the hub in use, and, open, the others and "Add a hub". Registers its controls for scripts.
pub fn picker(this: &Backplane, cx: &mut Context<Backplane>, light: bool, ctl: &mut Vec<crate::script::CtlEntry>) -> Div {
    use crate::script::{Ctl, CtlEntry};
    let c = |n: &str| theme::color(light, n);
    let es = hubs::entries_on(&this.hubs, this.screens.offers.on_home.as_deref());
    // the hub the window is on, by its address when that is another computer's (a `--url` start too, whatever the list says)
    let away = this.screens.elsewhere.clone();
    let home = es.iter().find(|e| e.on && !e.in_use).map(|e| e.label.clone());
    let cur = home.clone().or_else(|| away.as_ref().map(|e| e.1.clone())).or_else(|| es.first().map(|e| e.label.clone())).unwrap_or_default();
    /* i18n-ok: ctl labels are what drivers press */ ctl.push(CtlEntry { id: "hub:menu".into(), label: "Hub".into(), ctl: Ctl::Hub(Act::Menu) });
    let mut col = div().flex().flex_col().border_b_1().border_color(c("rule")).child(
        // the thread header's row height (less this column's rule, which the header's row counts in its own), so the two rules are one line
        div().id("hub-picker").flex().items_center().gap_2().px_3().h(crate::scale::tpx(super::head::ROW_H - 1.)).cursor_pointer().text_size(crate::scale::tpx(12.)).hover(|s| s.bg(c("panel")))
            .children(away.as_ref().map(|_| crate::art::icon("link", 12., c("accent"))))
            .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().when(away.is_some(), |d| d.text_color(c("accent"))).child(crate::i18n::trs(&cur))).child(crate::art::icon(if this.hub_menu { "chevron-down" } else { "chevron-right" }, 14., c("dim")))
            .on_click(cx.listener(|this, _, _, cx| act(this, Act::Menu, cx))));
    if this.hub_menu {
        // every hub but the one shown above to switch to, and a Forget on each paired one (the one in use too): pressed once it asks
        // ("Forget it"), again it forgets (Settings → Hubs has the same, but a device's person looks here first)
        let armed = this.screens.offers.forget_arm.clone();
        for e in es.iter() {
            if e.in_use && !e.can_forget { continue }
            let mut row = div().id(SharedString::from(format!("hub-row-{}", e.key))).flex().items_center().gap_2().px_3().py_1().text_size(crate::scale::tpx(12.));
            let name = div().flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().child(crate::i18n::trs(&e.label));
            if e.in_use {
                /* i18n-ok: drawn through trs */ row = row.child(name.text_color(c("dim"))).child(div().text_color(c("faint")).child(crate::i18n::trs("In use")));
            } else {
                ctl.push(CtlEntry { id: format!("hub:use:{}", e.key), label: e.label.clone(), ctl: Ctl::Hub(Act::Use(e.key.clone())) });
                let k = e.key.clone();
                row = row.child(name.id(SharedString::from(format!("hub-use-{}", e.key))).cursor_pointer().hover(|s| s.text_color(c("accent"))).on_click(cx.listener(move |this, _, _, cx| act(this, Act::Use(k.clone()), cx))));
            }
            if e.can_forget {
                let ask = armed.as_deref() == Some(e.key.as_str());
                let k = e.key.clone();
                /* i18n-ok: drawn through trs */ let label = if ask { "Forget it" } else { "Forget" };
                row = row.child(div().id(SharedString::from(format!("hub-forget-{}", e.key))).flex_shrink_0().cursor_pointer().text_color(c(if ask { "accent" } else { "faint" })).hover(|s| s.text_color(c("accent"))).child(crate::i18n::trs(label))
                    .on_click(cx.listener(move |this, _, _, cx| { if this.screens.offers.forget_arm.as_deref() == Some(k.as_str()) { this.screens.offers.forget_arm = None; this.hub_menu = false; act(this, Act::Forget(k.clone()), cx) } else { this.screens.offers.forget_arm = Some(k.clone()); cx.notify() } })));
            }
            col = col.child(row);
        }
        /* i18n-ok: ctl labels are what drivers press */ ctl.push(CtlEntry { id: "hub:add".into(), label: "Add a hub".into(), ctl: Ctl::Hub(Act::Add) });
        col = col.child(div().id("hub-add").px_3().py_1().cursor_pointer().text_size(crate::scale::tpx(12.)).text_color(c("dim")).hover(|s| s.bg(c("panel"))).child(crate::i18n::trs("Add a hub")).on_click(cx.listener(|this, _, _, cx| act(this, Act::Add, cx))));
    }
    col
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn the_page_shows_the_home_and_which_hub_is_used() {
        let mut boat = hubs::Hub::remote("https://boat-hub.ts.net"); boat.home = true;
        let l = vec![hubs::Hub::remote("http://a:1"), boat, hubs::Hub::local()];
        let r = rows(&l);
        assert_eq!((r[0].buttons[0].label.as_str(), r[1].buttons[0].label.as_str()), ("In use", "Use"));
        assert_eq!(r[1].note, "Home hub. Used when the hub in use cannot be reached.");
        let r = rows_on(&l, Some("boat-hub.ts.net:443"));
        assert_eq!((r[0].buttons[0].label.as_str(), r[1].buttons[0].label.as_str()), ("Use", "In use"), "failed over: the home is the one used");
        assert_eq!(r[0].note, "Cannot be reached: the home hub is in use until it answers.");
    }
    #[test] fn an_offer_is_tried_three_times_with_a_longer_wait_each_time() {
        assert_eq!(TRIES, 3); assert!(backoff(1) < backoff(2)); assert_eq!(backoff(1).as_secs(), 4);
    }
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
    #[test] fn resolve_target_picks_paired_remote_or_local_without_relaunch() {
        let remote = hubs::Hub::remote("https://box.ts.net:8451");
        let list = vec![remote.clone(), hubs::Hub::local()];
        let cred = std::path::PathBuf::from("/tmp/box.token");
        let got = resolve_target_with(&list, |k| (k == remote.key).then(|| cred.clone()), || None);
        assert_eq!(got, Some(("wss://box.ts.net:8451".into(), cred, true)));
        // Missing remote credential falls back to local; without local token returns None (triggers restart to launch local hub)
        assert_eq!(resolve_target_with(&list, |_| None, || None), None);
        let local_tok = std::path::PathBuf::from("/tmp/local.token");
        assert_eq!(resolve_target_with(&[hubs::Hub::local()], |_| None, || Some(local_tok.clone())), Some(("ws://127.0.0.1:3799".into(), local_tok, false)));
    }
}
