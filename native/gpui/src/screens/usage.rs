//! The Context panel: the composer's token chip opens it, floating over the thread from the chip, so the composer never moves. It draws the hub's picture
//! as it is (host present/tokstats.rs `usage`): the context meter, what feeds a bot's turns, this turn and the last split four ways, the thread's totals with
//! a sparkline of its last turns, a bot's lanes, the account the thread runs on (`thread.combo`), and what the panel offers (Compact, Switch model, Accounts).
//! Exact figures are in each figure's hover. No decision lives here: the colours of the four parts of a turn are the only thing it picks.
use crate::{i18n::trs, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::{cell::Cell, rc::Rc};

/// The panel's width at 100% text (it keeps to the window).
pub const W: f32 = 404.;
/// The four parts of a turn and the colour each is drawn in (the same in the bar and its legend).
pub fn part_color(key: &str) -> &'static str { match key { "input" => "input", "cread" => "accent", "cwrite" => "queued", _ => "ok" } }
/// The meter's colour: the hub's tone, else the accent.
pub fn tone_color(tone: &str) -> &'static str { match tone { "bad" => "bad", "warn" => "warn", _ => "accent" } }
/// A sparkline's bars: each value's height in px, at least 1 px so a quiet turn still shows.
pub fn spark_heights(v: &[f64], h: f32) -> Vec<f32> { v.iter().map(|x| ((*x as f32).clamp(0., 1.) * h).max(1.)).collect() }

/// What a press in the panel asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act { Compact, Model, Accounts }
pub fn act_of(id: &str) -> Option<Act> { match id { "compact" => Some(Act::Compact), "model" => Some(Act::Model), "accounts" => Some(Act::Accounts), _ => None } }

/// The panel, anchored over the composer's box at the chip's right edge (`chip`, `top`: their bounds as last drawn), or None while shut.
pub fn view(cx: &mut Context<Backplane>, light: bool, this: &Backplane, chip: &Rc<Cell<Bounds<Pixels>>>, top: &Rc<Cell<Bounds<Pixels>>>, win: (f32, f32)) -> Option<AnyElement> {
    if !this.screens.ctx_open { return None }
    let thread = &this.data["thread"];
    let u = &this.data["usage"];
    let c = |n: &str| theme::color(light, n);
    let s = (this.local.text_pct as f32 / 100.).clamp(0.7, 1.6);
    let z = |v: f32| px(v * s);
    let w = (W * s).min(win.0 - 24.);
    let b = chip.get();
    // over the composer's box (never over what is being typed), its right edge on the chip's, kept inside the window; the room above is its most height
    let above = { let t = top.get(); if t.size.height > px(0.) && t.top() < b.top() { t.top() } else { b.top() } };
    let room = (f32::from(above) - 16.).max(200.);
    let head = |t: String| div().text_size(z(10.5)).text_color(c("faint")).child(t.to_uppercase());
    let tip = |id: String, el: Div, t: &str| { let t = t.to_string(); el.id(SharedString::from(id)).when(!t.is_empty(), |d| d.tooltip(crate::keys::tip(&t, "", light))) };
    let rule = || div().h(px(1.)).bg(c("line"));
    let sect = || div().px(z(14.)).py(z(9.)).flex().flex_col().gap(z(6.));
    let mut card = div().flex().flex_col();

    // the model and the context meter
    let model = string(u, "model"); let window = string(u, "window");
    let mut top = sect().pt(z(12.)).child(div().flex().items_baseline().gap(z(8.)).child(div().text_size(z(13.)).font_weight(FontWeight::BOLD).text_color(c("text")).child(trs(&model)))
        .child(div().text_size(z(11.5)).text_color(c("dim")).child(trs(&window))).child(div().flex_1())
        .when(u["kind"] == "bot", |d| d.child(div().text_size(z(10.5)).text_color(c("faint")).child(trs("Bot chat")))));
    let ctx = &u["ctx"];
    if ctx.is_object() {
        let tone = tone_color(ctx["tone"].as_str().unwrap_or(""));
        let frac = ctx["frac"].as_f64().unwrap_or(0.) as f32;
        top = top.child(tip("usage-ctx".into(), div().flex().flex_col().gap(z(6.)).pt(z(2.)), &string(ctx, "tip"))
            .child(div().flex().items_baseline().gap(z(8.)).child(div().text_size(z(22.)).font_weight(FontWeight::BOLD).text_color(c(tone)).child(string(ctx, "big")))
                .child(div().text_size(z(12.)).text_color(c("dim")).child(trs(&string(ctx, "line")))).child(div().flex_1())
                .child(div().text_size(z(11.5)).text_color(c("faint")).child(trs(&string(ctx, "room")))))
            .child(div().h(z(6.)).w_full().bg(c("line")).child(div().h_full().w(relative(frac.clamp(0., 1.))).bg(c(tone))))
            .when(!string(ctx, "hint").is_empty(), |d| d.child(div().text_size(z(11.)).text_color(c("dim")).child(trs(&string(ctx, "hint"))))));
    } else {
        top = top.child(div().text_size(z(12.)).text_color(c("dim")).child(trs(&string(u, "ctxNote"))));
    }
    card = card.child(top);

    // a bot: its session and what each turn starts with
    let session = u["session"].as_array().cloned().unwrap_or_default(); let feeds = u["feeds"].as_array().cloned().unwrap_or_default();
    if !session.is_empty() || !feeds.is_empty() {
        let mut sec = sect().child(head(trs("Each turn starts with")));
        for (i, f) in feeds.iter().enumerate() {
            let frac = f["frac"].as_f64(); let sub = string(f, "sub"); let meter = string(f, "meter");
            sec = sec.child(tip(format!("usage-feed-{i}"), div().flex().items_center().gap(z(8.)).text_size(z(12.)), &string(f, "tip"))
                .child(div().w(z(86.)).flex_shrink_0().text_color(c("dim")).child(trs(&string(f, "label"))))
                .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(c("text")).child(trs(&string(f, "value")))
                    .when(!sub.is_empty(), |d| d.child(div().text_color(c("faint")).child(format!(" · {}", trs(&sub))))).flex())
                .when(!meter.is_empty(), |d| d.child(div().flex_shrink_0().text_size(z(11.)).text_color(c("faint")).child(meter)))
                .children(frac.map(|x| div().w(z(40.)).h(z(4.)).flex_shrink_0().bg(c("line")).child(div().h_full().w(relative((x as f32).clamp(0.02, 1.))).bg(c("dim"))))));
        }
        if !session.is_empty() {
            sec = sec.child(div().flex().flex_wrap().gap_x(z(14.)).gap_y(z(2.)).pt(z(2.)).text_size(z(11.)).children(session.iter().enumerate().map(|(i, r)|
                tip(format!("usage-sess-{i}"), div().flex().gap(z(5.)), &string(r, "tip")).child(div().text_color(c("faint")).child(trs(&string(r, "label")))).child(div().text_color(c("dim")).child(trs(&string(r, "value")))))));
        }
        card = card.child(rule()).child(sec);
    }

    // this turn so far and the last one: a bar split four ways and its legend
    let turns = u["turns"].as_array().cloned().unwrap_or_default();
    for (ti, t) in turns.iter().enumerate() {
        let live = t["live"] == true;
        let parts = t["parts"].as_array().cloned().unwrap_or_default();
        let mut sec = sect().child(div().flex().items_center().gap(z(8.))
            .when(live, |d| d.child(div().size(z(6.)).bg(c("accent"))))
            .child(head(trs(&string(t, "title")))).child(div().flex_1())
            .child(tip(format!("usage-turn-total-{ti}"), div().text_size(z(12.)).text_color(c("text")), &string(t, "totalTip")).child(string(t, "total")))
            .when(!string(t, "cost").is_empty(), |d| d.child(div().text_size(z(12.)).text_color(c("faint")).child("·"))
                .child(tip(format!("usage-turn-cost-{ti}"), div().text_size(z(12.)).text_color(c("text")), &string(t, "costTip")).child(string(t, "cost")))));
        if !parts.is_empty() {
            sec = sec.child(div().h(z(6.)).w_full().flex().bg(c("line")).children(parts.iter().filter(|p| p["frac"].as_f64().unwrap_or(0.) > 0.).map(|p|
                div().h_full().w(relative(p["frac"].as_f64().unwrap_or(0.) as f32)).bg(c(part_color(p["key"].as_str().unwrap_or("")))))));
            sec = sec.child(div().flex().flex_wrap().gap_y(z(3.)).children(parts.iter().enumerate().map(|(i, p)|
                tip(format!("usage-part-{ti}-{i}"), div().w(relative(0.5)).flex().items_center().gap(z(6.)).text_size(z(12.)), &string(p, "exact"))
                    .child(div().size(z(7.)).flex_shrink_0().bg(c(part_color(p["key"].as_str().unwrap_or("")))))
                    .child(div().text_color(c("dim")).child(trs(&string(p, "label"))))
                    .child(div().flex_1()).child(div().pr(z(if i % 2 == 0 { 18. } else { 0. })).text_color(c("text")).child(string(p, "short"))))));
        }
        if !string(t, "meta").is_empty() { sec = sec.child(div().text_size(z(11.)).text_color(c("faint")).overflow_hidden().whitespace_nowrap().text_ellipsis().child(trs(&string(t, "meta")))) }
        card = card.child(rule()).child(sec);
    }

    // the totals: this thread (with its last turns) and today, side by side
    let tots = u["totals"].as_array().cloned().unwrap_or_default();
    if !tots.is_empty() {
        let tile = |i: usize, t: &Value| {
            let spark = spark_heights(&t["spark"].as_array().map(|a| a.iter().filter_map(Value::as_f64).collect::<Vec<_>>()).unwrap_or_default(), 18. * s);
            div().flex_1().min_w_0().flex().flex_col().gap(z(3.))
                .child(head(trs(&string(t, "title"))))
                .child(tip(format!("usage-tot-{i}"), div().flex().items_baseline().gap(z(8.)), &string(t, "tip"))
                    .child(div().text_size(z(17.)).font_weight(FontWeight::BOLD).text_color(c("text")).child(string(t, "tokens")))
                    .child(div().text_size(z(12.)).text_color(c("dim")).child(if string(t, "tokens").is_empty() { String::new() } else { trs("tokens") }))
                    .child(div().flex_1()).child(div().text_size(z(13.)).text_color(c("text")).child(string(t, "cost"))))
                .child(div().text_size(z(11.)).text_color(c("faint")).overflow_hidden().whitespace_nowrap().text_ellipsis().child(trs(&string(t, "sub"))))
                .when(!spark.is_empty(), |d| d.child(tip(format!("usage-spark-{i}"), div().h(z(18.)).flex().items_end().gap(px(2.)), &string(t, "sparkTip"))
                    .children(spark.into_iter().map(|h| div().flex_1().max_w(z(7.)).h(px(h)).bg(c("accent")).opacity(0.6)))))
        };
        let mut row = div().flex().gap(z(18.));
        for (i, t) in tots.iter().enumerate() { if i > 0 { row = row.child(div().w(px(1.)).bg(c("line"))) } row = row.child(tile(i, t)) }
        card = card.child(rule()).child(sect().child(row));
    }

    // a bot's lanes today, each with its account
    let lanes = u["lanes"].as_array().cloned().unwrap_or_default();
    let lanes_note = string(u, "lanesNote");
    if !lanes.is_empty() || !lanes_note.is_empty() {
        let mut sec = sect().child(head(trs("Side lanes today")));
        for (i, l) in lanes.iter().enumerate() {
            let acct = string(l, "account");
            sec = sec.child(tip(format!("usage-lane-{i}"), div().flex().items_center().gap(z(8.)).text_size(z(12.)), &string(l, "tip"))
                .child(div().flex_1().min_w_0().flex().gap(z(6.)).overflow_hidden().whitespace_nowrap().child(div().flex_shrink_0().text_color(c("text")).child(trs(&string(l, "name"))))
                    .child(div().min_w_0().overflow_hidden().text_ellipsis().text_size(z(11.)).text_color(c("faint")).child(format!("{}{}", trs(&string(l, "sub")), if acct.is_empty() { String::new() } else { format!(" · {acct}") }) /* i18n-ok: a separator and an account's name */)))
                .child(div().w(z(54.)).flex().justify_end().text_color(c("text")).child(string(l, "tokens"))).child(div().w(z(54.)).flex().justify_end().text_color(c("dim")).child(string(l, "cost"))));
        }
        if !lanes_note.is_empty() { sec = sec.child(div().text_size(z(11.)).text_color(c("faint")).child(trs(&lanes_note))) }
        card = card.child(rule()).child(sec);
    }

    // more than one model this session
    let models = u["models"].as_array().cloned().unwrap_or_default();
    if !models.is_empty() {
        card = card.child(rule()).child(sect().child(head(trs("This session by model"))).children(models.iter().enumerate().map(|(i, m)|
            tip(format!("usage-model-{i}"), div().flex().gap(z(8.)).text_size(z(12.)), &string(m, "tip")).child(div().flex_1().text_color(c("dim")).child(trs(&string(m, "name"))))
                .child(div().text_color(c("text")).child(string(m, "tokens"))).child(div().w(z(62.)).flex().justify_end().text_color(c("dim")).child(string(m, "cost"))))));
    }

    // the account and what the panel offers
    let combo = &thread["combo"];
    let acct = string(combo, "account");
    let mut foot = sect().gap(z(8.));
    if !acct.is_empty() {
        let left = combo["left"].as_u64();
        let tone = super::combo::tone(combo);
        foot = foot.child(div().id("usage-account").flex().items_center().gap(z(8.)).text_size(z(12.)).cursor_pointer().tooltip(crate::keys::tip("Accounts in Settings", "", light))
            .on_click(cx.listener(|this, _, window, cx| act(this, Act::Accounts, window, cx)))
            .child(crate::ui::ic(light, "user", 13., "dim")).child(div().text_color(c("text")).child(acct))
            .children(left.map(|l| div().text_color(c(if tone == "text" { "dim" } else { tone })).child(crate::i18n::tf("gpui.combo.left", &[("pct", l.into())]))))
            .child(div().flex_1()).child(div().text_color(c("faint")).child(trs("Accounts"))).child(crate::ui::ic(light, "chevron-right", 12., "faint")));
    }
    let actions = u["actions"].as_array().cloned().unwrap_or_default();
    let note = string(u, "note");
    foot = foot.child(div().flex().items_center().gap(z(6.))
        .children(actions.iter().filter_map(|a| { let id = string(a, "id"); let what = act_of(&id)?;
            Some(div().id(SharedString::from(format!("usage-act-{id}"))).px(z(9.)).py(z(3.)).border_1().border_color(c("line")).bg(c("raise")).text_size(z(12.)).text_color(c("text")).cursor_pointer()
                .hover(|d| d.border_color(c("rule"))).tooltip(crate::keys::tip(&string(a, "tip"), "", light)).child(trs(&string(a, "label")))
                .on_click(cx.listener(move |this, _, window, cx| act(this, what, window, cx)))) }))
        .child(div().flex_1()));
    card = card.child(rule()).child(foot).when(!note.is_empty(), |d| d.child(div().px(z(14.)).pb(z(9.)).mt(z(-2.)).text_size(z(10.5)).text_color(c("faint")).child(trs(&note))));

    let card = div().id("usage-panel").occlude().w(px(w)).max_h(px(room)).overflow_y_scroll().border_1().border_color(c("line")).bg(c("panel")).shadow_lg().text_color(c("text"))
        .child(crate::art::probe("usage")).child(card)
        // a press anywhere else shuts it (the chip's own press toggles it)
        .on_mouse_down_out(cx.listener({ let chip = chip.clone(); move |this, e: &MouseDownEvent, _, cx| { if !chip.get().contains(&e.position) { this.screens.ctx_open = false; cx.notify(); } } }));
    let at = point(b.right(), above - px(6.));
    Some(deferred(anchored().position(at).anchor(Corner::BottomRight).snap_to_window_with_margin(px(8.)).child(card)).with_priority(1).into_any_element())
}

fn act(this: &mut Backplane, what: Act, window: &mut Window, cx: &mut Context<Backplane>) {
    this.screens.ctx_open = false;
    match what {
        /* i18n-ok: Claude's own command, sent as the person would type it */ Act::Compact => { let _ = this.tx.try_send(crate::hub::Command::Send("/compact".into())); }
        Act::Model => this.combo_model(window, cx),
        Act::Accounts => { this.want_focus = true; this.open_settings(cx); this.screens.settings_ui.device = false; this.screens.settings_ui.accounts = true; super::accounts::open_page(this, cx); }
    }
    cx.notify();
}

#[cfg(test)]
mod tests {
    use super::{act_of, part_color, spark_heights, tone_color, Act};
    #[test]
    fn parts_tones_sparks_and_acts() {
        assert_eq!(part_color("input"), "input"); assert_eq!(part_color("cread"), "accent"); assert_eq!(part_color("cwrite"), "queued"); assert_eq!(part_color("output"), "ok");
        assert_eq!(tone_color(""), "accent"); assert_eq!(tone_color("warn"), "warn"); assert_eq!(tone_color("bad"), "bad");
        assert_eq!(spark_heights(&[0., 0.5, 1., 2.], 10.), vec![1., 5., 10., 10.]);
        assert_eq!(act_of("compact"), Some(Act::Compact)); assert_eq!(act_of("model"), Some(Act::Model)); assert_eq!(act_of("accounts"), Some(Act::Accounts)); assert_eq!(act_of("x"), None);
    }
}
