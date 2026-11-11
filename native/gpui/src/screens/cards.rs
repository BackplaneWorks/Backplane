//! Settings rows the hub draws as more than words and controls (`kind`, mcpx/page.rs on the hub): a `card` (a tile, a title with its status pill,
//! a monospace line, a meta line, a switch), a `head`, a `group` label, a `notice`, a `code` block, a `tool` line under a card, an `empty`
//! state, a `panel` (a form's title and its mode chips), a `paste` box and a line of `actions`. Pure layout: every press is the row's own
//! action (settings::on_press), so nothing here knows what a server is.
use super::settings::{on_press, Btn, FieldSlot, Row};
use crate::{i18n::trs, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::collections::HashMap;

/// "#rrggbb" as a colour; None for anything else.
pub fn hex(s: &str) -> Option<Rgba> { let h = s.strip_prefix('#')?; (h.len() == 6).then(|| u32::from_str_radix(h, 16).ok()).flatten().map(gpui::rgb) }
fn alpha(mut c: Rgba, a: f32) -> Rgba { c.a = a; c }
/// A tone's colour token: ok, warn, bad, busy (the accent), anything else dim.
pub fn tone_token(t: &str) -> &'static str { match t { "ok" => "ok", "warn" => "warn", "bad" => "bad", "busy" => "accent", _ => "dim" } }
fn btn_of(v: &Value) -> Option<Btn> { let a = string(v, "action"); (!a.is_empty()).then(|| Btn { label: string(v, "label"), action: a, value: string(v, "value"), on: v["on"].as_bool().unwrap_or(false) }) }
/// The buttons a card draws beside its switch: not the ones its switch or its press already are (an older client's stand-ins).
pub fn card_buttons(r: &Row) -> Vec<Btn> {
    let (t, p) = (btn_of(&r.raw["toggle"]), btn_of(&r.raw["press"]));
    r.buttons.iter().filter(|b| !t.as_ref().map_or(false, |t| t.action == b.action) && !p.as_ref().map_or(false, |p| p.action == b.action)).cloned().collect()
}

fn chip(cx: &mut Context<Backplane>, light: bool, b: &Btn, id: String, small: bool) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let b2 = b.clone();
    let d = div().id(SharedString::from(id)).px(px(if small { 8. } else { 12. })).py(px(if small { 2. } else { 4. })).text_size(px(if small { 11. } else { 12. })).cursor_pointer().whitespace_nowrap();
    let d = if b.on { d.bg(c("accent")).text_color(c("on-accent")) } else { d.bg(c("raise")).hover(|s| s.bg(c("line"))) };
    let d = if crate::art::rects_on() { let n = format!("btn:{}:{}", b.action, b.value); d.relative().child(canvas(|_, _, _| (), move |r: Bounds<Pixels>, _, w: &mut Window, _| crate::art::report(&n, r, w.scale_factor())).absolute().top_0().left_0().size_full()) } else { d };
    d.child(trs(&b.label)).on_click(cx.listener(move |this, _, _, cx| on_press(this, &b2, cx))).into_any_element()
}
fn chips(cx: &mut Context<Backplane>, light: bool, bs: &[Btn], i: usize, small: bool) -> AnyElement { chips_p(cx, light, bs, i, small, None) }
/// `primary`: the button drawn as the page's main one (outlined in the accent when it is not lit).
fn chips_p(cx: &mut Context<Backplane>, light: bool, bs: &[Btn], i: usize, small: bool, primary: Option<usize>) -> AnyElement {
    let c = theme::color(light, "accent");
    div().flex().flex_wrap().gap_1().children(bs.iter().enumerate().map(|(j, b)| { let e = chip(cx, light, b, format!("set-{i}-{j}"), small); if primary == Some(j) && !b.on { div().border_1().border_color(c).child(e).into_any_element() } else { e } }).collect::<Vec<_>>()).into_any_element()
}
/// A switch: a square track, the knob at the end it is set to.
fn switch(cx: &mut Context<Backplane>, light: bool, t: &Value, id: String) -> Option<AnyElement> {
    let b = btn_of(t)?; let on = t["on"] == true; let c = |n: &str| theme::color(light, n);
    let track = if t["style"] == "check" {
        // a pick (what to add, what to import): a box with a tick
        div().id(SharedString::from(id)).size(px(18.)).flex().items_center().justify_center().cursor_pointer().bg(if on { c("accent") } else { c("bg") }).border_1().border_color(if on { c("accent") } else { c("dim") })
            .hover(|s| s.border_color(c("text"))).when(on, |d| d.child(crate::art::icon("circle-check", 14., c("on-accent"))))
    } else {
        let knob = div().size(px(12.)).bg(if on { gpui::rgb(0xffffff) } else { c("dim") });
        div().id(SharedString::from(id)).w(px(34.)).h(px(20.)).p(px(3.)).flex().items_center().when(on, |d| d.justify_end()).cursor_pointer()
            .bg(if on { c("accent") } else { c("raise") }).border_1().border_color(if on { c("accent") } else { c("line") }).hover(|s| s.border_color(c("dim"))).child(knob)
    };
    let track = if crate::art::rects_on() { let n = format!("btn:{}:{}", b.action, b.value); track.relative().child(canvas(|_, _, _| (), move |r: Bounds<Pixels>, _, w: &mut Window, _| crate::art::report(&n, r, w.scale_factor())).absolute().top_0().left_0().size_full()) } else { track };
    Some(track.on_click(cx.listener(move |this, _, _, cx| { cx.stop_propagation(); on_press(this, &b, cx) })).into_any_element())
}
fn pill(light: bool, p: &Value) -> Option<AnyElement> {
    let text = string(p, "text"); if text.is_empty() { return None }
    let col = theme::color(light, tone_token(&string(p, "tone")));
    Some(div().flex_none().px(px(6.)).py(px(1.)).text_size(px(10.5)).text_color(col).bg(alpha(col, if light { 0.12 } else { 0.16 })).whitespace_nowrap().child(trs(&text)).into_any_element())
}
fn tile(light: bool, icon: &Value, size: f32) -> AnyElement {
    let bg = hex(&string(icon, "bg")).unwrap_or(theme::color(light, "raise"));
    let text = string(icon, "text");
    div().flex_none().size(px(size)).flex().items_center().justify_center().bg(bg).border_1().border_color(gpui::hsla(0., 0., 1., if light { 0. } else { 0.10 })).text_color(gpui::rgb(0xffffff)).font_weight(FontWeight::BOLD).text_size(px(if text.chars().count() > 1 { size * 0.34 } else { size * 0.44 }))
        .child(if text.is_empty() { "·".to_string() } else { text }).into_any_element()
}
fn field_box(fields: &HashMap<String, FieldSlot>, light: bool, raw: &Value, name: &str) -> Option<AnyElement> {
    let slot = fields.get(name)?; let c = |n: &str| theme::color(light, n);
    let lines = raw["fields"].as_array().into_iter().flatten().find(|f| f["name"] == name).and_then(|f| f["lines"].as_u64()).unwrap_or(1).max(1) as f32;
    let mono = lines > 1.;
    Some(div().w_full().px_2().py(px(6.)).bg(c("raise")).border_1().border_color(c("line")).when(mono, |d| d.min_h(px(lines * 17. + 12.)).text_size(px(12.))).child(slot.input.clone()).into_any_element())
}

/// The row drawn by its kind; None for a kind this client does not know (it is then drawn as a plain row).
pub fn el(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, r: &Row, i: usize, avail: f32) -> Option<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    let raw = &r.raw;
    let note = |d: Div, t: &str, tone: &str| if t.is_empty() { d } else { d.child(div().text_size(px(12.)).text_color(c(tone)).child(trs(t))) };
    Some(match r.kind.as_str() {
        "head" => div().flex().items_start().justify_between().gap_4().pb_3().mb_1().border_b_1().border_color(c("line"))
            .child(note(div().flex().flex_col().gap_1().min_w_0().flex_1(), &r.note, "dim"))
            .child(chips_p(cx, light, &r.buttons, i, false, raw["primary"].as_u64().map(|x| x as usize))).into_any_element(),
        "group" => div().pt_4().pb(px(4.)).flex().items_baseline().gap_2().min_w_0().child(div().flex_none().text_size(px(11.)).text_color(c("dim")).child(trs(&r.label).to_uppercase()))
            .when(!r.note.is_empty(), |d| d.child(div().min_w_0().text_size(px(11.)).text_color(c("faint")).whitespace_nowrap().overflow_hidden().text_ellipsis().child(r.note.clone()))).into_any_element(),
        "notice" => {
            let col = c(tone_token(&r.tone));
            div().flex().items_center().gap_3().my_1().px_3().py_2().border_l_2().border_color(col).bg(alpha(col, 0.07))
                .child(div().flex().flex_col().gap(px(2.)).flex_1().min_w_0().child(div().text_size(px(12.)).text_color(if r.tone == "ok" || r.tone == "busy" { c("text") } else { col }).child(trs(&r.label)))
                    .when(!r.note.is_empty(), |d| d.child(div().text_size(px(11.)).text_color(c("dim")).child(trs(&r.note)))))
                .when(!r.buttons.is_empty(), |d| d.child(chips(cx, light, &r.buttons, i, true))).into_any_element()
        }
        "code" => div().my_1().px_3().py_2().bg(c("view")).border_1().border_color(c("line")).text_size(px(11.)).text_color(c("dim")).flex().flex_col()
            .children(string(raw, "text").lines().take(14).map(|l| div().whitespace_nowrap().overflow_hidden().text_ellipsis().child(l.to_string())).collect::<Vec<_>>()).into_any_element(),
        "empty" => div().my_2().p_6().flex().flex_col().items_center().gap_2().border_1().border_dashed().border_color(c("line"))
            .child(crate::art::icon("plug", 22., c("faint"))).child(div().text_size(px(14.)).font_weight(FontWeight::BOLD).child(trs(&r.label)))
            .child(div().max_w(px(460.)).text_size(px(12.)).text_color(c("dim")).text_center().child(trs(&r.note))).child(div().pt_2().child(chips(cx, light, &r.buttons, i, false))).into_any_element(),
        "panel" => div().mt_3().pt_3().pb_2().flex().items_center().justify_between().gap_3().border_t_1().border_color(c("accent"))
            .child(note(div().flex().flex_col().gap_1().min_w_0().flex_1().child(div().text_size(px(13.)).font_weight(FontWeight::BOLD).child(trs(&r.label))), &r.note, "dim"))
            .child(chips(cx, light, &r.buttons, i, false)).into_any_element(),
        "paste" => div().py_2().flex().flex_col().gap_2()
            .child(div().text_size(px(12.)).text_color(c("dim")).child(trs(&r.note)))
            .children(r.fields.iter().filter_map(|f| field_box(fields, light, raw, &f.name))).into_any_element(),
        "actions" => div().py_3().flex().items_center().justify_between().gap_3()
            .child(div().text_size(px(11.)).text_color(c(if r.note.contains("again") { "bad" } else { "dim" })).child(trs(&r.note)))
            .child(chips(cx, light, &r.buttons, i, false)).into_any_element(),
        "tool" => div().ml(px(48.)).py(px(6.)).flex().items_center().gap_3().border_b_1().border_color(c("line"))
            .child(div().flex_1().min_w_0().flex().flex_col().gap(px(1.))
                .child(div().text_size(px(12.)).text_color(if raw["toggle"]["on"] == false { c("faint") } else { c("text") }).child(r.label.clone()))
                .when(!r.note.is_empty(), |d| d.child(div().text_size(px(11.)).text_color(c("dim")).whitespace_nowrap().overflow_hidden().text_ellipsis().child(r.note.clone()))))
            .children(switch(cx, light, &raw["toggle"], format!("set-{i}-t"))).into_any_element(),
        "card" => {
            let open = raw["open"] == true;
            let narrow = avail < 560.;
            let title = string(raw, "title");
            let mut head = div().flex().items_center().gap_2().min_w_0().child(div().text_size(px(13.)).font_weight(FontWeight::BOLD).whitespace_nowrap().overflow_hidden().text_ellipsis().child(trs(if title.is_empty() { &r.label } else { &title })));
            let badge = string(raw, "badge");
            if !badge.is_empty() { head = head.child(div().flex_none().px(px(5.)).text_size(px(10.)).text_color(c("dim")).border_1().border_color(c("line")).child(trs(&badge))) }
            if let Some(p) = pill(light, &raw["pill"]) { head = head.child(p) }
            let (sub, meta) = (string(raw, "sub"), string(raw, "meta"));
            let body = div().flex_1().min_w_0().flex().flex_col().gap(px(3.)).child(head)
                .when(!sub.is_empty(), |d| d.child(div().text_size(px(11.5)).text_color(c("dim")).whitespace_nowrap().overflow_hidden().text_ellipsis().child(sub.clone())))
                .when(!meta.is_empty(), |d| d.child(div().text_size(px(11.)).text_color(c("faint")).child(trs(&meta))));
            let extra = card_buttons(r);
            let mut right = div().flex_none().flex().items_center().gap_2();
            if !extra.is_empty() { right = right.child(chips(cx, light, &extra, i, true)) }
            if let Some(s) = switch(cx, light, &raw["toggle"], format!("set-{i}-t")) { right = right.child(s) }
            let field = r.fields.iter().find_map(|f| field_box(fields, light, raw, &f.name));
            let press = btn_of(&raw["press"]);
            let compact = raw["compact"] == true; let ts = if compact { 28. } else { 36. };
            let mut card = div().id(SharedString::from(format!("card-{i}"))).mt(px(if open { 10. } else { 6. })).px_3().py(px(if compact { 7. } else { 10. })).flex().flex_col().gap_2().bg(c("panel")).border_1().border_color(if open { c("accent") } else { c("line") })
                .child(if narrow { div().flex().flex_col().gap_2().child(div().flex().gap_3().items_start().child(tile(light, &raw["icon"], ts)).child(body)).child(right).into_any_element() }
                    else { div().flex().gap_3().items_center().child(tile(light, &raw["icon"], ts)).child(body).child(right).into_any_element() })
                .children(field);
            if let Some(p) = press { card = card.cursor_pointer().hover(|s| s.border_color(c("dim"))).on_click(cx.listener(move |this, _, _, cx| on_press(this, &p, cx))) }
            if crate::art::rects_on() { let n = format!("card:{}", string(raw, "id")); card = card.relative().child(canvas(|_, _, _| (), move |r: Bounds<Pixels>, _, w: &mut Window, _| crate::art::report(&n, r, w.scale_factor())).absolute().top_0().left_0().size_full()) }
            card.into_any_element()
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn colours_tones_and_the_buttons_a_card_keeps() {
        assert!(hex("#24292f").is_some() && hex("24292f").is_none() && hex("#zzz000").is_none());
        assert_eq!(tone_token("busy"), "accent"); assert_eq!(tone_token(""), "dim");
        let v: Value = serde_json::json!({"rows":[{"kind":"card","label":"fetch","note":"","toggle":{"on":true,"action":"mcp-toggle","value":"fetch"},"press":{"action":"mcp-view","value":"open:fetch"},
            "buttons":[{"label":"on","action":"mcp-toggle","value":"fetch","on":true},{"label":"Open","action":"mcp-view","value":"open:fetch","on":false},{"label":"Docs","action":"open-url","value":"https://x","on":false}]}]});
        let rows = super::super::settings::parse(&v); assert_eq!(rows[0].kind, "card");
        assert_eq!(card_buttons(&rows[0]).iter().map(|b| b.label.as_str()).collect::<Vec<_>>(), ["Docs"], "the switch and the press are not drawn twice");
        let plain = super::super::settings::parse(&serde_json::json!({"rows":[{"label":"x","note":"","buttons":[]}]})); assert!(plain[0].kind.is_empty() && plain[0].raw.is_null());
    }
}

/// A row drawn inside the opened card above it: the accent frame continued down its sides (and closed under the last).
pub fn inside(light: bool, e: AnyElement, last: bool) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    div().px_3().bg(c("panel")).border_l_1().border_r_1().border_color(c("accent")).when(last, |d| d.border_b_1().mb_2()).child(e).into_any_element()
}
