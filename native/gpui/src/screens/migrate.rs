//! A 0.14 home coming across: a card over the whole window, drawn from the hub's `migrate` (present/onboard.rs `import_overlay`): its title, body, the bar
//! (`pct`, none on a failure), its lines and its buttons. Every word and every button is the hub's. While it runs there is no close and Esc does nothing: the
//! hub serves meanwhile, and the card comes back on every client until the import ends; it then shows what came across until Continue.
use crate::{hub::Command, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

pub fn is_up(data: &Value) -> bool { data["migrate"].is_object() }

pub fn view(cx: &mut Context<Backplane>, light: bool, m: &Value) -> Option<AnyElement> {
    if !m.is_object() { return None }
    let c = |n: &str| theme::color(light, n);
    let mut card = div().id("migrate-card").w(px(640.)).max_w_full().flex().flex_col().gap_3().p_6().bg(c("bg")).border_1().border_color(c("line"))
        .child(div().text_size(px(22.)).font_weight(FontWeight::BOLD).child(string(m, "title")))
        .child(div().text_size(px(14.)).text_color(c("dim")).line_height(px(21.)).child(string(m, "body")));
    if let Some(p) = m["pct"].as_u64() {
        let p = p.min(100) as f32;
        card = card.child(div().id("migrate-bar").w_full().h(px(8.)).bg(c("raise")).child(div().h_full().w(relative(p / 100.)).bg(c("accent"))))
            .child(div().text_size(px(12.)).text_color(c("faint")).child(format!("{}%", p as u32)));
    }
    for l in m["lines"].as_array().into_iter().flatten().filter_map(Value::as_str) { card = card.child(div().text_size(px(13.)).text_color(c("dim")).child(l.to_string())); }
    let mut row = div().flex().gap_2().pt_2();
    for (i, b) in m["buttons"].as_array().into_iter().flatten().enumerate() {
        let (a, v, on) = (string(b, "action"), string(b, "value"), b["on"] == true);
        let id = format!("btn:{a}:{v}");
        let d = div().id(SharedString::from(format!("migrate-btn-{i}"))).h(px(30.)).px_4().flex().items_center().text_size(px(13.)).cursor_pointer();
        let d = if on { d.bg(c("accent")).text_color(c("on-accent")) } else { d.bg(c("raise")).hover(|s| s.bg(c("line"))) };
        // its rectangle for the e2e drivers (BACKPLANE_GPUI_RECTS=1 only)
        let d = if crate::art::rects_on() { d.relative().child(canvas(|_, _, _| (), move |r: Bounds<Pixels>, _, w: &mut Window, _| crate::art::report(&id, r, w.scale_factor())).absolute().top_0().left_0().size_full()) } else { d };
        row = row.child(d.child(string(b, "label")).on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action(a.clone(), v.clone())); cx.notify(); })));
    }
    card = card.child(row);
    // the window behind is dimmed and takes no input: nothing behind the card can be pressed into a half-made state
    Some(div().id("migrate").absolute().top_0().left_0().size_full().flex().items_center().justify_center().occlude()
        .child(div().absolute().top_0().left_0().size_full().bg(c("bg")).opacity(0.92)).child(card).into_any_element())
}
