//! Asks and approvals: `thread.asks` from the hub (Mob.asks in src/mobile/screen.bend).
//! Three shapes: the standard card (permission, plan, input: head, optional markdown body or detail,
//! buttons), the settings proposal (same card, Approve/Reject), and a buy thread's "Backplane needs" card
//! (choices, a typed box sent as `pre + text`, Skip). Every press is the view action `answer` with the
//! button's own value, as the phones send it.
use super::composer::{Composer, ComposerEvent};
use crate::{hub::Command, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct Button { pub label: String, pub value: String, pub primary: bool }

#[derive(Debug, Clone, PartialEq)]
pub struct Ask {
    pub id: String, pub kind: String, pub head: String, pub detail: String,
    pub blocks: Vec<Value>, pub buttons: Vec<Button>,
    pub needs: bool, pub typed: bool, pub pre: String, pub placeholder: String, pub go: String, pub skip: Option<Button>,
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
        })
    }).collect()
}

/// What pressing a button sends.
pub fn answer(value: &str) -> Command { Command::Action("answer".into(), value.to_string()) }

/// What the typed box sends: the ask's `pre` and the trimmed text, nothing for an empty box.
pub fn typed_answer(ask: &Ask, text: &str) -> Option<Command> {
    let t = text.trim();
    if t.is_empty() { None } else { Some(answer(&format!("{}{}", ask.pre, t))) }
}

fn pill(cx: &mut Context<Backplane>, light: bool, key: String, rect: String, b: &Button) -> Stateful<Div> {
    let c = |n: &str| theme::color(light, n);
    let value = b.value.clone();
    let d = crate::ui::with_rect(div().id(SharedString::from(key)), || rect).px_3().py_1().text_size(px(12.)).cursor_pointer().border_1();
    let d = if b.primary { d.bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")) } else { d.bg(c("raise")).border_color(c("line")).hover(|s| s.bg(c("panel"))) };
    d.child(b.label.clone()).on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(answer(&value)); cx.notify(); }))
}

fn card(cx: &mut Context<Backplane>, light: bool, input: Option<Entity<Composer>>, ask: &Ask, i: usize) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let mut col = div().flex().flex_col().gap_2().p_3().bg(c("panel")).border_1().border_color(if ask.needs { c("accent") } else { c("bad") });
    let mut head = div().flex().items_center().child(div().flex_1().font_weight(FontWeight::BOLD).text_size(px(13.)).text_color(if ask.needs { c("accent") } else { c("text") }).child(ask.head.clone()));
    if let Some(skip) = &ask.skip {
        let v = skip.value.clone();
        head = head.child(div().id(SharedString::from(format!("ask-{i}-skip"))).text_size(px(11.)).text_color(c("dim")).cursor_pointer().hover(|s| s.text_color(c("text"))).child(skip.label.clone())
            .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(answer(&v)); cx.notify(); })));
    }
    col = col.child(head);
    if !ask.blocks.is_empty() {
        col = col.child(div().id(SharedString::from(format!("ask-{i}-body"))).max_h(px(220.)).overflow_y_scroll().flex().flex_col().gap_1().children(ask.blocks.iter().map(|b| crate::ui::block(b, light, 0))));
    } else if !ask.detail.is_empty() {
        col = col.child(div().id(SharedString::from(format!("ask-{i}-detail"))).max_h(px(300.)).overflow_y_scroll().text_size(px(12.)).text_color(c("dim")).child(ask.detail.clone()));
    }
    let chips = div().flex().flex_wrap().gap_2().children(ask.buttons.iter().enumerate().map(|(j, b)| pill(cx, light, format!("ask-{i}-b{j}"), format!("ask:{i}:{j}"), b).into_any_element()).collect::<Vec<_>>());
    if let (true, Some(input)) = (ask.needs && !ask.pre.is_empty(), input) {
        let id = ask.id.clone();
        let boxed = div().flex().gap_2().items_center()
            .child(div().flex_1().px_2().py_1().border_1().border_color(c("line")).bg(c("view")).child(input))
            .child(div().id(SharedString::from(format!("ask-{i}-go"))).px_3().py_1().cursor_pointer().text_size(px(12.))
                .bg(if ask.typed { c("accent") } else { c("raise") }).text_color(if ask.typed { c("on-accent") } else { c("text") }).child(ask.go.clone())
                .on_click(cx.listener(move |this, _, _, cx| submit(this, &id, cx))));
        col = if ask.typed { col.child(boxed).child(chips) } else { col.child(chips).child(boxed) };
    } else {
        col = col.child(chips);
    }
    col.into_any_element()
}

/// The typed answer of an ask: its box's text, sent as `pre + text`; the box is cleared once sent.
pub fn submit(this: &mut Backplane, id: &str, cx: &mut Context<Backplane>) {
    let asks = parse(&this.data["thread"]["asks"]);
    let (Some(ask), Some(input)) = (asks.iter().find(|a| a.id == id), this.screens.ask_inputs.get(id).cloned()) else { return };
    let text = input.read(cx).content.to_string();
    if let Some(cmd) = typed_answer(ask, &text) { let _ = this.tx.try_send(cmd); input.update(cx, |i, cx| i.set(String::new(), cx)); }
    cx.notify();
}

/// One box per open typed ask (keyed by its id), made when the ask arrives and dropped when it goes.
pub fn sync(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let asks = parse(&this.data["thread"]["asks"]);
    this.screens.ask_inputs.retain(|id, _| asks.iter().any(|a| a.needs && !a.pre.is_empty() && a.id == *id));
    for a in asks.iter().filter(|a| a.needs && !a.pre.is_empty()) {
        if this.screens.ask_inputs.contains_key(&a.id) { continue; }
        let input = cx.new(|cx| Composer::with_context(cx, "AskInput"));
        let placeholder = a.placeholder.clone();
        input.update(cx, |i, _| i.set_placeholder(&placeholder));
        let id = a.id.clone();
        cx.subscribe(&input, move |this, _, ev: &ComposerEvent, cx| if matches!(ev, ComposerEvent::Submit) { submit(this, &id, cx); }).detach();
        this.screens.ask_inputs.insert(a.id.clone(), input);
    }
}

/// The asks card stack above the composer, scrolling when tall; none when the thread has no open ask.
pub fn view(cx: &mut Context<Backplane>, light: bool, inputs: &std::collections::HashMap<String, Entity<Composer>>, asks: &Value) -> Option<AnyElement> {
    let asks = parse(asks);
    if asks.is_empty() { return None; }
    let cards: Vec<AnyElement> = asks.iter().enumerate().map(|(i, a)| card(cx, light, inputs.get(&a.id).cloned(), a, i)).collect();
    Some(div().id("asks").max_h(px(420.)).overflow_y_scroll().flex().flex_col().gap_2().children(cards).into_any_element())
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
    #[test] fn keeps_every_ask_and_skips_malformed() {
        let many: Vec<Value> = (0..20).map(|i| serde_json::from_str(&format!(r#"{{"id":"x{i}","buttons":[]}}"#)).unwrap()).collect();
        assert_eq!(parse(&Value::Array(many)).len(), 20, "no ask may be hidden");
        assert!(parse(&serde_json::from_str(r#"[{"kind":"permission"}]"#).unwrap()).is_empty());
        assert!(parse(&Value::Null).is_empty());
    }
}
