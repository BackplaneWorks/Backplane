//! Asks and approvals: `thread.asks` from the hub.
//! Four shapes: proposed plan cards (`kind == "plan"` with elevated header, rich Markdown body,
//! Copy / Expand / Implement / inline Revise / Dismiss), standard permission/input cards, settings
//! proposals, and buy-thread "Backplane needs" cards.
use super::composer::{Composer, ComposerEvent};
use crate::{hub::Command, i18n::trs, string, theme, ui::{ic, with_rect}, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub struct Button { pub label: String, pub value: String, pub primary: bool }

#[derive(Debug, Clone, PartialEq)]
pub struct Ask {
    pub id: String, pub kind: String, pub head: String, pub detail: String,
    pub blocks: Vec<Value>, pub buttons: Vec<Button>,
    pub needs: bool, pub typed: bool, pub pre: String, pub placeholder: String, pub go: String, pub skip: Option<Button>,
    pub revise: bool, pub revise_pre: String, pub raw: String,
}

fn button(v: &Value) -> Option<Button> {
    let value = v["value"].as_str()?;
    Some(Button { label: string(v, "label"), value: value.to_string(), primary: v["primary"].as_bool().unwrap_or(false) })
}

/// Parse the hub's asks array. Nothing is dropped: an ask that is not drawn can never be answered.
pub fn parse(asks: &Value) -> Vec<Ask> {
    asks.as_array().into_iter().flatten().filter_map(|a| {
        let id = a["id"].as_str()?.to_string();
        Some(Ask {
            id, kind: string(a, "kind"), head: string(a, "head"), detail: string(a, "detail"),
            blocks: a["blocks"].as_array().cloned().unwrap_or_default(),
            buttons: a["buttons"].as_array().into_iter().flatten().filter_map(button).collect(),
            needs: a["needs"].as_bool().unwrap_or(false), typed: a["typed"].as_bool().unwrap_or(false),
            pre: string(a, "pre"), placeholder: string(a, "placeholder"),
            go: string(a, "go"),
            skip: button(&a["skip"]),
            revise: a["revise"] == true,
            revise_pre: string(a, "revisePre"),
            raw: string(a, "raw"),
        })
    }).collect()
}

/// What pressing a button sends.
pub fn answer(value: &str) -> Command { Command::Action("answer".into(), value.to_string()) }

/// What the typed box sends: the ask's `pre` or `revise_pre` and the trimmed text, nothing for empty text.
pub fn typed_answer(ask: &Ask, text: &str) -> Option<Command> {
    let t = text.trim();
    if t.is_empty() { return None }
    let prefix = if !ask.pre.is_empty() { &ask.pre } else { &ask.revise_pre };
    if prefix.is_empty() { None } else { Some(answer(&format!("{prefix}{t}"))) }
}

fn pill(cx: &mut Context<Backplane>, light: bool, key: String, rect: String, b: &Button, quiet: bool) -> Stateful<Div> {
    let c = |n: &str| theme::color(light, n);
    let value = b.value.clone();
    let d = with_rect(div().id(SharedString::from(key)), || rect).px_3().py_1().text_size(crate::scale::tpx(12.)).cursor_pointer().border_1();
    let d = if b.primary {
        d.bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")).hover(|s| s.opacity(0.9))
    } else if quiet {
        d.bg(c("panel")).border_color(c("line")).text_color(c("dim")).hover(|s| s.bg(c("raise")).text_color(c("text")))
    } else {
        d.bg(c("raise")).border_color(c("line")).text_color(c("text")).hover(|s| s.bg(c("panel")))
    };
    d.child(trs(&b.label)).on_click(cx.listener(move |this, _, _, cx| {
        if this.screens.plan_revising.as_deref() == Some(value.split('|').next().unwrap_or("")) {
            this.screens.plan_revising = None;
        }
        let _ = this.tx.try_send(answer(&value));
        cx.notify();
    }))
}

fn copy_text_of(ask: &Ask) -> String {
    if !ask.raw.is_empty() { return ask.raw.clone() }
    if !ask.detail.is_empty() { return ask.detail.clone() }
    let parsed = super::md::parse(&ask.blocks);
    parsed.iter().map(super::md::block_text).collect::<Vec<_>>().join("\n\n")
}

fn plan_card(cx: &mut Context<Backplane>, light: bool, input: Option<Entity<Composer>>, ask: &Ask, i: usize, expanded: bool, revising: bool) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let aid = ask.id.clone();
    let aid_exp = aid.clone();
    let aid_rev = aid.clone();
    let raw = copy_text_of(ask);
    let copy_id = SharedString::from(format!("ask-{i}-copy"));
    let copied = super::copyable::just_copied(&copy_id);

    let left = div().flex().items_center().gap_2().flex_1().min_w(px(140.))
        .child(ic(light, "file-text", 14., "input"))
        .child(div().font_weight(FontWeight::BOLD).text_size(crate::scale::tpx(13.)).text_color(c("text")).child(trs(&ask.head)))
        .child(div().px(px(5.)).py(px(1.)).border_1().border_color(c("input")).bg(c("raise")).text_size(crate::scale::tpx(10.)).text_color(c("input")).child(trs("Ready for review")));

    let mut hdr_btns = div().flex().items_center().gap_1().flex_shrink_0();
    if !raw.is_empty() {
        hdr_btns = hdr_btns.child(div().id(copy_id.clone()).flex().items_center().gap_1().px_2().py(px(2.)).border_1().border_color(c("line")).bg(c("panel")).text_size(crate::scale::tpx(11.)).text_color(c(if copied { "ok" } else { "dim" })).cursor_pointer().hover(|s| s.bg(c("raise")).text_color(c("text")))
            .child(ic(light, if copied { "check" } else { "copy" }, 11., if copied { "ok" } else { "dim" }))
            .child(if copied { trs("Copied") } else { trs("Copy") })
            .on_click(cx.listener(move |_, _, window, cx| { super::copyable::copy(&copy_id, &raw, window, cx); })));
    }
    hdr_btns = hdr_btns.child(div().id(SharedString::from(format!("ask-{i}-expand"))).px_2().py(px(2.)).border_1().border_color(c("line")).bg(c("panel")).text_size(crate::scale::tpx(11.)).text_color(c("dim")).cursor_pointer().hover(|s| s.bg(c("raise")).text_color(c("text")))
        .child(if expanded { trs("Collapse") } else { trs("Expand") })
        .on_click(cx.listener(move |this, _, _, cx| {
            if !this.screens.plan_expanded.remove(&aid_exp) { this.screens.plan_expanded.insert(aid_exp.clone()); }
            cx.notify();
        })));
    let head = div().flex().flex_wrap().items_center().justify_between().gap_2().child(left).child(hdr_btns);

    let body_el = if !ask.blocks.is_empty() {
        let parsed = super::md::parse(&ask.blocks);
        let reg: super::timeline::Reg = Default::default();
        {
            let mut r = reg.borrow_mut();
            r.app = Some(cx.entity().downgrade());
            r.enter(0, &format!("plan-ask-{}", ask.id));
        }
        div().flex().flex_col().gap(px(6.)).children(super::timeline::blocks(cx, &reg, light, &parsed, &format!("plan-ask-{}", ask.id)))
    } else {
        div().text_size(crate::scale::tpx(12.5)).text_color(c("text")).child(ask.detail.clone())
    };
    let body = div().id(SharedString::from(format!("ask-{i}-body")))
        .max_h(px(if expanded { 560. } else { 280. }))
        .overflow_y_scroll().p_3().border_1().border_color(c("line")).bg(c("view"))
        .child(body_el);

    let mut actions = div().flex().flex_wrap().items_center().gap_2();
    let implement_btn = ask.buttons.first();
    let rest_btns = ask.buttons.iter().skip(1);
    if let Some(b) = implement_btn {
        actions = actions.child(pill(cx, light, format!("ask-{i}-b0"), format!("ask:{i}:0"), b, false));
    }
    if ask.revise && !ask.revise_pre.is_empty() {
        actions = actions.child(with_rect(div().id(SharedString::from(format!("ask-{i}-revise"))), || format!("ask:{i}:revise"))
            .px_3().py_1().text_size(crate::scale::tpx(12.)).cursor_pointer().border_1()
            .border_color(c(if revising { "input" } else { "line" }))
            .bg(c(if revising { "raise" } else { "panel" }))
            .text_color(c(if revising { "input" } else { "text" }))
            .hover(|s| s.bg(c("raise")))
            .flex().items_center().gap_1()
            .child(ic(light, "square-pen", 12., if revising { "input" } else { "dim" }))
            .child(trs("Revise"))
            .on_click(cx.listener(move |this, _, window, cx| {
                if this.screens.plan_revising.as_deref() == Some(aid_rev.as_str()) {
                    this.screens.plan_revising = None;
                } else {
                    this.screens.plan_revising = Some(aid_rev.clone());
                    if let Some(inp) = this.screens.ask_inputs.get(&aid_rev) {
                        inp.focus_handle(cx).focus(window);
                    }
                }
                cx.notify();
            })));
    }
    for (idx, b) in rest_btns.enumerate() {
        let j = idx + 1;
        actions = actions.child(pill(cx, light, format!("ask-{i}-b{j}"), format!("ask:{i}:{j}"), b, true));
    }

    let mut content = div().flex_1().min_w_0().flex().flex_col().gap_2().p_3()
        .child(head).child(body).child(actions);

    if revising {
        if let Some(input) = input {
            let sid = ask.id.clone();
            let can_send = !input.read(cx).content.trim().is_empty();
            content = content.child(div().pt_2().border_t_1().border_color(c("line")).flex().flex_wrap().items_center().gap_2()
                .child(div().flex_1().min_w(px(220.)).px_2().py_1().border_1().border_color(c("input")).bg(c("view")).child(input))
                .child(div().id(SharedString::from(format!("ask-{i}-revise-send"))).px_3().py_1().text_size(crate::scale::tpx(12.)).border_1()
                    .border_color(c(if can_send { "input" } else { "line" }))
                    .bg(c(if can_send { "input" } else { "raise" }))
                    .text_color(c(if can_send { "on-accent" } else { "faint" }))
                    .when(can_send, |d| d.cursor_pointer().hover(|s| s.opacity(0.9)))
                    .child(trs("Send revision"))
                    .on_click(cx.listener(move |this, _, _, cx| submit(this, &sid, cx))))
                .child(div().id(SharedString::from(format!("ask-{i}-revise-cancel"))).px_2().py_1().text_size(crate::scale::tpx(12.)).text_color(c("dim")).cursor_pointer().hover(|s| s.text_color(c("text")))
                    .child(trs("Cancel"))
                    .on_click(cx.listener(|this, _, _, cx| { this.screens.plan_revising = None; cx.notify(); }))));
        }
    }

    div().w_full().flex().bg(c("panel")).border_1().border_color(c("input"))
        .child(div().w(px(3.)).flex_shrink_0().bg(c("input")))
        .child(content)
        .into_any_element()
}

/// Close any open inline Plan Revise drawer on Escape and return focus to the main composer.
pub fn escape_revise(app: &mut Backplane, cx: &mut Context<Backplane>) -> bool {
    if app.screens.plan_revising.is_none() { return false }
    app.screens.plan_revising = None;
    app.want_focus = true;
    cx.notify();
    true
}

fn card(cx: &mut Context<Backplane>, light: bool, input: Option<Entity<Composer>>, ask: &Ask, i: usize, expanded: bool, revising: bool) -> AnyElement {
    if ask.kind == "plan" { return plan_card(cx, light, input, ask, i, expanded, revising) }
    let c = |n: &str| theme::color(light, n);
    let rail = if ask.needs { "accent" } else { "bad" };
    let mut col = div().flex_1().min_w_0().flex().flex_col().gap_2().p_3();
    let mut head = div().flex().items_center().child(div().flex_1().font_weight(FontWeight::BOLD).text_size(crate::scale::tpx(13.)).text_color(if ask.needs { c("accent") } else { c("text") }).child(trs(&ask.head)));
    if let Some(skip) = &ask.skip {
        let v = skip.value.clone();
        head = head.child(div().id(SharedString::from(format!("ask-{i}-skip"))).text_size(crate::scale::tpx(11.)).text_color(c("dim")).cursor_pointer().hover(|s| s.text_color(c("text"))).child(trs(&skip.label))
            .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(answer(&v)); cx.notify(); })));
    }
    col = col.child(head);
    if !ask.blocks.is_empty() {
        col = col.child(div().id(SharedString::from(format!("ask-{i}-body"))).max_h(px(220.)).overflow_y_scroll().flex().flex_col().gap_1().children(ask.blocks.iter().map(|b| crate::ui::block(b, light, 0))));
    } else if !ask.detail.is_empty() {
        col = col.child(div().id(SharedString::from(format!("ask-{i}-detail"))).max_h(px(300.)).overflow_y_scroll().text_size(crate::scale::tpx(12.)).text_color(c("dim")).child(ask.detail.clone()));
    }
    let chips = div().flex().flex_wrap().gap_2().children(ask.buttons.iter().enumerate().map(|(j, b)| pill(cx, light, format!("ask-{i}-b{j}"), format!("ask:{i}:{j}"), b, false).into_any_element()).collect::<Vec<_>>());
    if let (true, Some(input)) = (ask.needs && !ask.pre.is_empty(), input) {
        let id = ask.id.clone();
        let boxed = div().flex().gap_2().items_center()
            .child(with_rect(div(), || format!("ask:{i}:box")).flex_1().px_2().py_1().border_1().border_color(c("line")).bg(c("view")).child(input))
            .child(with_rect(div().id(SharedString::from(format!("ask-{i}-go"))), || format!("ask:{i}:go")).px_3().py_1().cursor_pointer().text_size(crate::scale::tpx(12.))
                .bg(if ask.typed { c("accent") } else { c("raise") }).text_color(if ask.typed { c("on-accent") } else { c("text") }).child(trs(&ask.go))
                .on_click(cx.listener(move |this, _, _, cx| submit(this, &id, cx))));
        col = if ask.typed { col.child(boxed).child(chips) } else { col.child(chips).child(boxed) };
    } else {
        col = col.child(chips);
    }
    div().w_full().flex().bg(c("panel")).border_1().border_color(c("line"))
        .child(div().w(px(3.)).flex_shrink_0().bg(c(rail)))
        .child(col)
        .into_any_element()
}

/// The typed answer of an ask: its box's text, sent as `pre + text`; the box is cleared once sent.
pub fn submit(this: &mut Backplane, id: &str, cx: &mut Context<Backplane>) {
    let asks = parse(&this.data["thread"]["asks"]);
    let (Some(ask), Some(input)) = (asks.iter().find(|a| a.id == id), this.screens.ask_inputs.get(id).cloned()) else { return };
    let text = input.read(cx).content.to_string();
    if let Some(cmd) = typed_answer(ask, &text) {
        let _ = this.tx.try_send(cmd);
        input.update(cx, |i, cx| i.set(String::new(), cx));
        if this.screens.plan_revising.as_deref() == Some(id) { this.screens.plan_revising = None; }
    }
    cx.notify();
}

/// One box per open typed ask or plan-revise drawer (keyed by its id), made when the ask arrives and dropped when it goes.
pub fn sync(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let asks = parse(&this.data["thread"]["asks"]);
    let needs_box = |a: &Ask| (a.needs && !a.pre.is_empty()) || (a.revise && !a.revise_pre.is_empty());
    this.screens.ask_inputs.retain(|id, _| asks.iter().any(|a| needs_box(a) && a.id == *id));
    this.screens.plan_expanded.retain(|id| asks.iter().any(|a| a.id == *id));
    if let Some(ref rev) = this.screens.plan_revising {
        if !asks.iter().any(|a| a.revise && a.id == *rev) { this.screens.plan_revising = None; }
    }
    for a in asks.iter().filter(|a| needs_box(a)) {
        if this.screens.ask_inputs.contains_key(&a.id) { continue; }
        let input = cx.new(|cx| Composer::with_context(cx, "AskInput"));
        let placeholder = if a.revise { trs("What should change in the plan?") } else { trs(&a.placeholder) };
        input.update(cx, |i, _| i.set_placeholder(&placeholder));
        let id = a.id.clone();
        cx.subscribe(&input, move |this, _, ev: &ComposerEvent, cx| if matches!(ev, ComposerEvent::Submit) { submit(this, &id, cx); }).detach();
        if a.revise { cx.observe(&input, |_, _, cx| cx.notify()).detach(); }
        this.screens.ask_inputs.insert(a.id.clone(), input);
    }
}

/// The asks card stack above the composer, scrolling when tall; none when the thread has no open ask.
pub fn view(cx: &mut Context<Backplane>, light: bool, inputs: &std::collections::HashMap<String, Entity<Composer>>, asks: &Value, expanded: &HashSet<String>, revising: Option<&str>) -> Option<AnyElement> {
    let asks = parse(asks);
    if asks.is_empty() { return None; }
    let cards: Vec<AnyElement> = asks.iter().enumerate().map(|(i, a)| card(cx, light, inputs.get(&a.id).cloned(), a, i, expanded.contains(&a.id), revising == Some(a.id.as_str()))).collect();
    Some(div().id("asks").max_h(px(620.)).overflow_y_scroll().flex().flex_col().gap_2().children(cards).into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test; // gpui::* also exports a `test` macro
    fn std_ask() -> Value { serde_json::from_str(r#"[{"id":"a1","kind":"permission","head":"Run Bash?","detail":"ls -la","blocks":[],
        "buttons":[{"label":"Allow","value":"a1|accept","primary":true},{"label":"Session","value":"a1|session","primary":false},{"label":"Deny","value":"a1|decline","primary":false}]}]"#).unwrap() }
    #[test] fn parses_a_permission_ask() {
        let a = parse(&std_ask());
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].head, "Run Bash?");
        assert_eq!(a[0].buttons.iter().map(|b| b.value.as_str()).collect::<Vec<_>>(), ["a1|accept", "a1|session", "a1|decline"]);
        assert!(a[0].buttons[0].primary && !a[0].needs);
    }
    #[test] fn pressing_sends_answer_with_the_buttons_value() {
        match answer("a1|decline") { Command::Action(a, v) => assert_eq!((a.as_str(), v.as_str()), ("answer", "a1|decline")), c => panic!("{c:?}") }
    }
    #[test] fn needs_card_sends_pre_plus_text_and_nothing_when_empty() {
        let a = parse(&serde_json::from_str(r#"[{"id":"q","kind":"input","needs":true,"head":"Backplane needs","detail":"Ship where?","blocks":[],"typed":true,"pre":"q|",
            "placeholder":"Type it here","go":"Continue","skip":{"label":"Skip","value":"q|","primary":false},"buttons":[{"label":"US","value":"q|US","primary":true}]}]"#).unwrap());
        assert!(a[0].needs && a[0].typed);
        assert_eq!(a[0].skip.as_ref().unwrap().value, "q|");
        assert!(typed_answer(&a[0], "   ").is_none());
        match typed_answer(&a[0], " US east ").unwrap() { Command::Action(n, v) => assert_eq!((n.as_str(), v.as_str()), ("answer", "q|US east")), c => panic!("{c:?}") }
    }
    #[test] fn plan_card_parses_revise_pre_and_sends_prefixed_feedback() {
        let a = parse(&serde_json::from_str(r##"[{"id":"kp1","kind":"plan","head":"Proposed plan","detail":"","revise":true,"revisePre":"kp1|revise:","raw":"# Steps\n1. A",
            "blocks":[{"tag":"h1","kids":[{"text":"Steps"}]}],"buttons":[{"label":"Implement","value":"kp1|implement","primary":true},{"label":"Dismiss","value":"kp1|dismiss","primary":false}]}]"##).unwrap());
        assert!(a[0].revise && a[0].revise_pre == "kp1|revise:");
        assert_eq!(copy_text_of(&a[0]), "# Steps\n1. A");
        assert!(typed_answer(&a[0], " ").is_none());
        match typed_answer(&a[0], " Keep LDO ").unwrap() {
            Command::Action(n, v) => assert_eq!((n.as_str(), v.as_str()), ("answer", "kp1|revise:Keep LDO")),
            c => panic!("{c:?}"),
        }
    }
    #[test] fn keeps_every_ask_and_skips_malformed() {
        let many: Vec<Value> = (0..20).map(|i| serde_json::from_str(&format!(r#"{{"id":"x{i}","buttons":[]}}"#)).unwrap()).collect();
        assert_eq!(parse(&Value::Array(many)).len(), 20, "no ask may be hidden");
        assert!(parse(&serde_json::from_str(r#"[{"kind":"permission"}]"#).unwrap()).is_empty());
        assert!(parse(&Value::Null).is_empty());
    }
}
