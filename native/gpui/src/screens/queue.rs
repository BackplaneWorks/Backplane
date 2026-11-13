//! The queue above the composer (0.14's Qu.*): the messages waiting for the agent, as the hub words them (thread `queue` rows `{msg, tag, text, buttons}`
//! and `queued`, the rows not shown). A button is a composer action the hub turns into its queue request (session/composer.rs `act`): Steer or Send,
//! Edit (back into the draft), Up, Remove, and Resume on the first row of a held queue.
use crate::{hub::Command, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};

#[derive(Debug, PartialEq)]
pub struct Row { pub msg: String, pub tag: String, pub text: String, pub buttons: Vec<(String, String, String)> }
pub fn rows(thread: &Value) -> Vec<Row> {
    thread["queue"].as_array().into_iter().flatten().filter(|r| r["msg"].is_string()).map(|r| Row { msg: string(r, "msg"), tag: string(r, "tag"), text: string(r, "text"),
        buttons: r["buttons"].as_array().into_iter().flatten().map(|b| (string(b, "label"), string(b, "action"), string(b, "value"))).filter(|b| !b.1.is_empty()).collect() }).collect()
}

pub fn strip(cx: &mut Context<Backplane>, light: bool, thread: &Value, unsent: &[Value]) -> Option<AnyElement> {
    // this window's own unsent messages (unsent.rs) first: they were written before anything the hub holds
    let mut rs = rows(&json!({"queue": unsent})); rs.extend(rows(thread));
    if rs.is_empty() { return None }
    let c = |n: &str| theme::color(light, n);
    let more = string(thread, "queued");
    Some(div().id("queue-strip").border_1().border_color(c("line")).bg(c("panel")).flex().flex_col().text_size(crate::scale::tpx(12.))
        .children(rs.into_iter().enumerate().map(|(i, r)| {
            div().id(("queue-row", i)).flex().items_center().gap_2().px_2().py(px(3.)).when(i > 0, |d| d.border_t_1().border_color(c("line")))
                .child(div().flex_shrink_0().w(crate::scale::tpx(56.)).text_size(crate::scale::tpx(10.)).text_color(c("faint")).child(crate::i18n::trs(&r.tag).to_uppercase()))
                // w_0: a long line must not size the composer's column by its unwrapped width (the draft box then stopped growing and its text ran under the chips)
                .child(div().flex_1().w_0().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(r.text))
                .children(r.buttons.into_iter().map(|(label, action, value)| {
                    div().id(SharedString::from(format!("q-{i}-{action}"))).flex_shrink_0().px_2().border_1().border_color(c("line")).text_size(crate::scale::tpx(11.)).text_color(c("dim")).cursor_pointer()
                        .hover(|s| s.bg(c("raise")).text_color(c("text"))).child(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            // the window's own (unsent.rs): Edit puts the text back after the draft, Remove drops it; nothing goes to the hub
                            if action == "u-edit" || action == "u-remove" {
                                if let Some(text) = this.screens.unsent.remove(&value) { if action == "u-edit" { this.unsent_to_draft(&text, cx) } }
                                cx.notify(); return;
                            }
                            // Edit: the hub's reply carries the draft with the message in it; take it once, and the keyboard goes there
                            if action == "q-edit" { this.await_completion(cx); this.want_focus = true; }
                            let _ = this.tx.try_send(Command::Action(action.clone(), value.clone())); cx.notify();
                        }))
                })).into_any_element()
        }).collect::<Vec<_>>())
        .when(!more.is_empty(), |d| d.child(div().px_2().py(px(2.)).border_t_1().border_color(c("line")).text_size(crate::scale::tpx(11.)).text_color(c("faint")).child(more)))
        .into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    use serde_json::json;
    #[test] fn the_hubs_queue_rows_are_read_and_a_thread_without_one_has_none() {
        let t = json!({"queue": [{"msg": "m1", "tag": "Held", "text": "first", "buttons": [{"label": "Send", "action": "q-steer", "value": "m1"}, {"label": "x", "action": "", "value": ""}]}], "queued": "2 more"});
        assert_eq!(rows(&t), vec![Row { msg: "m1".into(), tag: "Held".into(), text: "first".into(), buttons: vec![("Send".into(), "q-steer".into(), "m1".into())] }]);
        assert!(rows(&json!({"queue": []})).is_empty() && rows(&Value::Null).is_empty());
    }
}
