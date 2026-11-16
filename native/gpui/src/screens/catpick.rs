//! The cat control (a row of `kind: "cat"`, present/onboard_uber.rs `cat_row`): setup's Your uberbot and a bot's Settings. The name box with
//! the cat beside it, the colours (its coat's own, the presets, a custom #rrggbb) and Different cat / The name's cat. The hub decides what
//! is kept; the window only works out, each letter, which look the typed name gives (crate::cat, the hub's own file), so the cat follows the
//! typing with no round trip, and animates the change.
//!
//! Animations, only while one runs (a frame is asked for each paint until it ends, then nothing): a new look fades in with a small pop
//! (`LETTER_MS`); the row's `spawn` naming a bot it did not name before plays the arrival (`SPAWN_MS`): the preview goes up in a puff, the
//! cat drops in, lands with a squash and a little dust, stretches and blinks. With reduced motion (crate::plat::reduce_motion) both are a
//! plain fade.
use super::settings::FieldSlot;
use crate::{art, cat, hub::Command, i18n::trs, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::{cell::RefCell, collections::HashMap, sync::Arc, time::Instant};

pub const LETTER_MS: f32 = 180.;
pub const SPAWN_MS: f32 = 1300.;
/// The cat's box: wide enough for the cat, tall enough for its drop.
const W: f32 = 112.; const H: f32 = 150.; const CAT: f32 = 96.;

/// What the control shows now and what it showed, per control (its name field).
#[derive(Clone, Debug, PartialEq)]
pub struct Shown { pub look: u64, pub color: String, pub prev: Option<(u64, String)>, pub at: Option<Instant>, pub spawn: String, pub spawn_at: Option<Instant> }
thread_local! {
    static SHOWN: RefCell<HashMap<String, Shown>> = RefCell::new(HashMap::new());
    static CATS: RefCell<art::Cats> = RefCell::new(art::Cats::new());
}

/// The state after a draw that shows (`look`, `color`) with the row's `spawn`: a new look starts the letter change; a spawn not seen
/// before starts the arrival, except on the first draw (a page opened on a cat already there plays nothing).
pub fn step(old: Option<Shown>, look: u64, color: &str, spawn: &str, now: Instant) -> Shown {
    let Some(mut s) = old else { return Shown { look, color: color.into(), prev: None, at: None, spawn: spawn.into(), spawn_at: None } };
    if s.look != look || s.color != color { s.prev = Some((s.look, s.color.clone())); s.look = look; s.color = color.into(); s.at = Some(now); }
    if s.spawn != spawn { if !spawn.is_empty() { s.spawn_at = Some(now) } s.spawn = spawn.into(); }
    s
}

/// The arrival at `ms` after it began: the cat's Fx, and the puff (0..1, 0 when none) and the dust (0..1) drawn around it.
pub fn spawn_fx(ms: f32, reduced: bool) -> (art::Fx, f32, f32) {
    let n = art::Fx::NONE;
    if reduced { return (art::Fx { alpha: (ms / 250.).clamp(0., 1.), ..n }, 0., 0.) }
    let sin = |x: f32| (std::f32::consts::PI * x.clamp(0., 1.)).sin();
    match ms {
        m if m < 160. => { let t = m / 160.; (art::Fx { sx: 1. - t * t, sy: 1. - t * t, alpha: 1. - t, ..n }, m / 400., 0.) }
        m if m < 560. => { let u = (m - 160.) / 400.; (art::Fx { dy: -36. * (1. - u * u), sx: 0.92, sy: 1.1, alpha: (u * 4.).min(1.), ..n }, m / 400., 0.) }
        m if m < 760. => { let k = sin((m - 560.) / 200.); (art::Fx { sx: 1. + 0.16 * k, sy: 1. - 0.2 * k, ..n }, 0., (m - 560.) / 300.) }
        m if m < 1000. => { let k = sin((m - 760.) / 240.); (art::Fx { dy: -1.5 * k, sx: 1. - 0.04 * k, sy: 1. + 0.07 * k, ..n }, 0., ((m - 560.) / 300.).min(1.01)) }
        m if m < 1250. => (art::Fx { blink: sin((m - 1000.) / 250.), ..n }, 0., 0.),
        _ => (n, 0., 0.),
    }
}
/// A new look `t` (0..1) into its change: the new cat's Fx and the old one's alpha.
pub fn letter_fx(t: f32, reduced: bool) -> (art::Fx, f32) {
    let t = t.clamp(0., 1.);
    if reduced { return (art::Fx { alpha: t, ..art::Fx::NONE }, 1. - t) }
    let back = 1. + 2.2 * (t - 1.).powi(3) + 1.2 * (t - 1.).powi(2); // ease out with a little overshoot
    (art::Fx { sx: 0.88 + 0.12 * back, sy: 0.88 + 0.12 * back, alpha: (t * 2.).min(1.), ..art::Fx::NONE }, (1. - t * 2.).max(0.))
}

fn rig(look: u64, color: &str) -> Option<Arc<art::Rig>> { CATS.with(|c| c.borrow_mut().rig(look, "idle", color)) }

/// The cat with its animations, in a W x H box.
fn cat_box(key: String, s: &Shown, light: bool) -> AnyElement {
    let (now, cur, prev) = (rig(s.look, &s.color), s.prev.as_ref().and_then(|(l, c)| rig(*l, c)), s.clone());
    let reduced = crate::plat::reduce_motion();
    let accent = theme::color(light, "accent"); let dust = theme::color(light, "faint");
    canvas(|_, _, _| (), move |b: Bounds<Pixels>, _, window: &mut Window, _| {
        art::report(&key, b, window.scale_factor());
        let cb = Bounds { origin: point(b.origin.x + px((W - CAT) / 2.), b.origin.y + px(H - CAT - 2.)), size: size(px(CAT), px(CAT)) };
        let mut running = false;
        let spawn_ms = prev.spawn_at.map(|t| t.elapsed().as_secs_f32() * 1000.).filter(|m| *m < SPAWN_MS);
        if let Some(ms) = spawn_ms {
            running = true;
            let (fx, puff, dusty) = spawn_fx(ms, reduced);
            // before the drop the puff shows where it went up; the cat drawn is the one that arrives (the same look)
            if let Some(r) = &now { art::paint_cat_fx(window, cb, r, 0., CAT, 1., fx); }
            let mid = point(cb.origin.x + px(CAT / 2.), cb.origin.y + px(CAT * 0.55));
            if puff > 0. && puff < 1. { for i in 0..8 { let a = i as f32 * std::f32::consts::TAU / 8.; let r = 10. + 40. * puff; let s = 5. * (1. - puff);
                let p = point(mid.x + px(a.cos() * r), mid.y + px(a.sin() * r));
                let mut col = accent; col.a = 1. - puff;
                window.paint_quad(fill(Bounds { origin: point(p.x - px(s), p.y - px(s * 0.25)), size: size(px(s * 2.), px(s * 0.5)) }, col));
                window.paint_quad(fill(Bounds { origin: point(p.x - px(s * 0.25), p.y - px(s)), size: size(px(s * 0.5), px(s * 2.)) }, col)); } }
            if dusty > 0. && dusty < 1. { for side in [-1f32, 1.] { for k in 0..3 { let d = 8. + 26. * dusty + k as f32 * 6.; let s = 4. * (1. - dusty) + 1.;
                let p = point(cb.origin.x + px(CAT / 2. + side * d), cb.origin.y + px(CAT * 0.97 - 6. * dusty - k as f32 * 3.));
                let mut col = dust; col.a = 0.8 * (1. - dusty);
                window.paint_quad(fill(Bounds { origin: point(p.x - px(s / 2.), p.y - px(s / 2.)), size: size(px(s), px(s)) }, col)); } } }
        } else {
            let t = prev.at.map(|t| t.elapsed().as_secs_f32() * 1000. / LETTER_MS).unwrap_or(1.);
            if t < 1. {
                running = true;
                let (fx, old) = letter_fx(t, reduced);
                if let Some(r) = &cur { art::paint_cat_fx(window, cb, r, 0., CAT, 1., art::Fx { alpha: old, ..art::Fx::NONE }); }
                if let Some(r) = &now { art::paint_cat_fx(window, cb, r, 0., CAT, 1., fx); }
            } else if let Some(r) = &now { art::paint_cat_fx(window, cb, r, 0., CAT, 1., art::Fx::NONE); }
        }
        if running { window.request_animation_frame(); }
    }).w(px(W)).h(px(H)).flex_none().into_any_element()
}

fn field(fields: &HashMap<String, FieldSlot>, light: bool, name: &str, w: f32) -> Option<AnyElement> {
    let slot = fields.get(name)?; let c = |n: &str| theme::color(light, n);
    Some(div().w(px(w)).px_2().py(px(6.)).bg(c("raise")).border_1().border_color(c("line")).child(slot.input.clone()).into_any_element())
}
/// A press: what is typed goes first (`settings::flush_fields`), then the row's action. `custom` and `go` carry the box's text after `\u{1f}`
/// for a page that keeps no drafts (a bot's Settings); setup reads its drafts and ignores it.
fn send(this: &mut Backplane, act: &str, value: String, cx: &mut Context<Backplane>) {
    super::settings::flush_fields(this); this.error.clear();
    let _ = this.tx.try_send(Command::Action(act.to_string(), value)); cx.notify();
}
fn text_of(this: &Backplane, name: &str, cx: &App) -> String { this.screens.fields.get(name).map(|s| s.input.read(cx).content.to_string()).unwrap_or_default() }

fn chip(cx: &mut Context<Backplane>, light: bool, id: String, label: String, on: bool, f: impl Fn(&mut Backplane, &mut Context<Backplane>) + 'static) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let d = div().id(SharedString::from(id)).px(px(10.)).py(px(4.)).text_size(crate::scale::tpx(12.)).cursor_pointer().whitespace_nowrap();
    let d = if on { d.bg(c("accent")).text_color(c("on-accent")) } else { d.bg(c("raise")).hover(|s| s.bg(c("line"))) };
    d.child(label).on_click(cx.listener(move |this, _, _, cx| f(this, cx))).into_any_element()
}

/// The control for a `kind: "cat"` row.
pub fn view(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, raw: &Value, avail: f32) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let fs: Vec<String> = raw["fields"].as_array().into_iter().flatten().map(|f| string(f, "name")).collect();
    let (name_f, custom_f) = (fs.first().cloned().unwrap_or_default(), fs.get(1).cloned().unwrap_or_default());
    let act = string(raw, "act"); let w = |k: &str| trs(&string(&raw["words"], k));
    // the look: a picked one, else the one the typed name gives, now
    let typed = fields.get(&name_f).map(|s| s.input.read(cx).content.to_string()).unwrap_or_else(|| string(&raw["fields"][0], "value"));
    let look = raw["look"].as_u64().unwrap_or_else(|| cat::look_of(&typed));
    let color = string(raw, "color");
    let shown = SHOWN.with(|m| { let mut m = m.borrow_mut(); let s = step(m.remove(&name_f), look, &color, &string(raw, "spawn"), Instant::now()); m.insert(name_f.clone(), s.clone()); s });
    let key = if crate::art::rects_on() { format!("catpick:{name_f}") } else { String::new() };

    // name and Save name
    let mut name_line = div().flex().items_center().gap_2().children(field(fields, light, &name_f, 220.));
    if let Some(g) = raw["go"].as_object() { let (a, nf) = (act.clone(), name_f.clone());
        name_line = name_line.child(chip(cx, light, format!("cat-go-{name_f}"), trs(&g.get("label").and_then(Value::as_str).unwrap_or("").to_string()), true, move |this, cx| { let t = text_of(this, &nf, cx); send(this, &a, format!("go\u{1f}{t}"), cx) })); }
    // colours: the coat's own, the presets as squares, Custom
    let mut sw = div().flex().flex_wrap().items_center().gap(px(6.)).child(div().text_size(crate::scale::tpx(12.)).text_color(c("dim")).mr_1().child(w("color")));
    { let a = act.clone(); sw = sw.child(chip(cx, light, format!("cat-own-{name_f}"), w("own"), color.is_empty(), move |this, cx| send(this, &a, "color\u{1f}".into(), cx))); }
    for (i, p) in raw["presets"].as_array().into_iter().flatten().enumerate() {
        let (hex, pname) = (string(p, "color"), string(p, "name")); let on = hex.eq_ignore_ascii_case(&color);
        let Some(col) = super::cards::hex(&hex) else { continue };
        let a = act.clone(); let v = format!("color\u{1f}{hex}");
        sw = sw.child(div().id(SharedString::from(format!("cat-color-{name_f}-{i}"))).size(px(22.)).bg(col).cursor_pointer().border_2().border_color(if on { c("text") } else { c("line") })
            .tooltip(crate::keys::tip(&trs(&pname), "", light))
            .on_click(cx.listener(move |this, _, _, cx| send(this, &a, v.clone(), cx))));
    }
    let custom_on = !color.is_empty() && !raw["presets"].as_array().into_iter().flatten().any(|p| string(p, "color").eq_ignore_ascii_case(&color));
    if custom_on { if let Some(col) = super::cards::hex(&color) { sw = sw.child(div().size(px(22.)).bg(col).border_2().border_color(c("text"))); } }
    sw = sw.child(div().text_size(crate::scale::tpx(12.)).text_color(c("dim")).ml_2().child(w("custom"))).children(field(fields, light, &custom_f, 92.));
    { let (a, cf) = (act.clone(), custom_f.clone()); sw = sw.child(chip(cx, light, format!("cat-custom-{name_f}"), w("use"), false, move |this, cx| {
        let t = text_of(this, &cf, cx);
        // a page with no drafts says nothing back for a colour it cannot read: said here
        if a != "ob-uber" && cat::parse_color(&t).is_none() { this.error = trs("Type a colour like #6fb3f2."); cx.notify(); return }
        send(this, &a, format!("custom\u{1f}{t}"), cx) })); }
    // Different cat, and back to the name's
    let mut picks = div().flex().gap_2();
    { let a = act.clone(); picks = picks.child(chip(cx, light, format!("cat-shuffle-{name_f}"), w("shuffle"), false, move |this, cx| send(this, &a, "shuffle".into(), cx))); }
    if raw["picked"] == true { let a = act.clone(); picks = picks.child(chip(cx, light, format!("cat-auto-{name_f}"), w("auto"), false, move |this, cx| send(this, &a, "auto".into(), cx))); }

    let note = string(raw, "note");
    let col = div().flex().flex_col().gap_3().min_w_0().flex_1().child(name_line).child(sw).child(picks)
        .when(!note.is_empty(), |d| d.child(div().text_size(crate::scale::tpx(12.)).text_color(c(super::cards::tone_token(&string(raw, "tone")))).child(trs(&note))));
    let stack = avail < 520.;
    div().id(SharedString::from(format!("catpick-{name_f}"))).w_full().flex().when(stack, |d| d.flex_col()).items_start().gap_4().py_2()
        .child(cat_box(key, &shown, light)).child(col).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn a_new_look_starts_a_change_and_a_new_spawn_its_arrival_but_not_on_the_first_draw() {
        let t = Instant::now();
        let s = step(None, 3, "", "b1", t); assert!(s.at.is_none() && s.spawn_at.is_none(), "opened on a cat already there: nothing plays");
        let s = step(Some(s), 3, "", "b1", t); assert!(s.at.is_none());
        let s = step(Some(s), 7, "", "b1", t); assert_eq!((s.prev.clone(), s.at.is_some()), (Some((3, String::new())), true), "a letter changed the look");
        let s = step(Some(Shown { at: None, ..s }), 7, "#6fb3f2", "b1", t); assert!(s.at.is_some(), "a colour is a change too");
        let s = step(Some(Shown { spawn: String::new(), ..s }), 7, "#6fb3f2", "b2", t); assert!(s.spawn_at.is_some(), "Say hello made it: it arrives");
    }
    #[test] fn the_arrival_ends_where_the_cat_sits_and_reduced_motion_only_fades() {
        assert_eq!(spawn_fx(SPAWN_MS, false).0, art::Fx::NONE);
        let (drop, _, _) = spawn_fx(200., false); assert!(drop.dy < -20. && drop.sy > 1., "falling, stretched: {drop:?}");
        let (land, _, dust) = spawn_fx(660., false); assert!(land.sy < 0.85 && land.sx > 1.1 && dust > 0., "squashed on landing, with dust: {land:?}");
        assert!(spawn_fx(1125., false).0.blink > 0.9, "a blink");
        for ms in [0., 100., 300., 700., 1100.] { let (f, p, d) = spawn_fx(ms, true); assert!(f.dx == 0. && f.dy == 0. && f.sx == 1. && f.sy == 1. && f.blink == 0. && p == 0. && d == 0., "reduced motion at {ms}: {f:?}"); }
        assert_eq!(spawn_fx(300., true).0.alpha, 1.);
        let (f, old) = letter_fx(1., false); assert!((f.sx - 1.).abs() < 1e-4 && f.alpha == 1. && old == 0.);
        assert!(letter_fx(0.8, false).0.sx > 1., "a little overshoot"); assert_eq!(letter_fx(0.5, true).0.sx, 1.);
    }
}
