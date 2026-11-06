//! The thread's to-do list and subagents: two cells in the header bar ("To-do 1/4", "Subagents · 2 at work") that open their rows in a dock under it (client.bend
//! `Todos.*`, subs.bend `Subs.*`). The rows come from the hub (`thread.todos`, `thread.subs`); whether a panel is open is this device's: to-do stays shut until opened,
//! subagents are open while any is at work and fold to their head once all are done (a click on the head holds the choice).
use crate::{string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Default)]
pub struct Panels { pub todos_open: HashMap<String, bool>, pub subs_open: HashMap<String, bool> }

pub fn todos_is_open(p: &Panels, thread: &Value) -> bool { thread["todos"].is_object() && p.todos_open.get(&string(thread, "id")).copied().unwrap_or(false) }
/// Open while any is at work, unless the person chose.
pub fn subs_is_open(p: &Panels, thread: &Value) -> bool {
    thread["subs"].is_object() && p.subs_open.get(&string(thread, "id")).copied().unwrap_or_else(|| thread["subs"]["live"].as_u64().unwrap_or(0) > 0)
}

fn cell(light: bool, id: &'static str, label: String, detail: String, open: bool) -> Stateful<Div> {
    let c = |n: &str| theme::color(light, n);
    div().id(id).px_3().h_full().flex().items_center().gap_2().border_l_1().border_color(c("line")).text_size(px(11.)).cursor_pointer().hover(|s| s.bg(c("raise"))).when(open, |d| d.bg(c("raise")))
        .child(div().text_color(c("faint")).child(label)).child(detail).child(crate::art::icon(if open { "chevron-down" } else { "chevron-right" }, 12., c("faint")))
}

/// The two header cells (those the thread has).
pub fn cells(cx: &mut Context<Backplane>, light: bool, p: &Panels, thread: &Value) -> Vec<AnyElement> {
    let mut out = vec![]; let tid = string(thread, "id");
    if thread["todos"].is_object() {
        let t = tid.clone(); let open = todos_is_open(p, thread);
        out.push(cell(light, "todos-cell", "TO-DO".into(), string(&thread["todos"], "count"), open).on_click(cx.listener(move |this, _, _, cx| { let o = this.screens.panels.todos_open.entry(t.clone()).or_insert(false); *o = !*o; cx.notify(); })).into_any_element());
    }
    if thread["subs"].is_object() {
        let t = tid.clone(); let open = subs_is_open(p, thread);
        let word = string(&thread["subs"], "word"); let live = thread["subs"]["live"].as_u64().unwrap_or(0) > 0;
        let (dc, ac) = (theme::color(light, "accent"), theme::color(light, "faint"));
        out.push(cell(light, "subs-cell", "SUBAGENTS".into(), word, open).child(div().size(px(6.)).bg(if live { dc } else { ac }))
            .on_click(cx.listener(move |this, _, _, cx| { let now = subs_is_open(&this.screens.panels, &this.data["thread"]); this.screens.panels.subs_open.insert(t.clone(), !now); cx.notify(); })).into_any_element());
    }
    out
}

/// The dock under the bar: the open panels' rows.
pub fn dock(light: bool, p: &Panels, thread: &Value) -> Option<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    let (to, su) = (todos_is_open(p, thread), subs_is_open(p, thread));
    if !to && !su { return None }
    let mut col = div().id("panels-dock").flex_shrink_0().max_h(px(220.)).overflow_y_scroll().flex().flex_col().border_b_1().border_color(c("line")).bg(c("panel")).px_3().py_1().text_size(px(12.));
    if to {
        col = col.child(div().text_size(px(11.)).text_color(c("faint")).child(string(&thread["todos"], "head").to_uppercase()));
        for r in thread["todos"]["rows"].as_array().into_iter().flatten() {
            let st = string(r, "status");
            let (mark, col_) = match st.as_str() { "completed" => ("✓", c("ok")), "in_progress" => ("▸", c("accent")), _ => ("○", c("faint")) };
            col = col.child(div().flex().gap_2().py(px(1.)).child(div().w(px(14.)).text_color(col_).child(mark)).child(div().flex_1().min_w_0().when(st == "completed", |d| d.text_color(c("faint")).line_through()).child(string(r, "text"))));
        }
    }
    if su {
        col = col.child(div().mt_1().text_size(px(11.)).text_color(c("faint")).child(format!("SUBAGENTS · {}", string(&thread["subs"], "word").to_uppercase())));
        for r in thread["subs"]["rows"].as_array().into_iter().flatten() {
            let working = string(r, "state") == "working"; let doing = string(r, "doing");
            col = col.child(div().flex().gap_2().py(px(1.)).child(div().w(px(14.)).text_color(if working { c("accent") } else { c("faint") }).child(if working { "▸" } else { "✓" }))
                .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(string(r, "title")))
                .child(div().flex_shrink_0().text_color(c("faint")).child(if doing.is_empty() { format!("{} · {}", string(r, "kind"), string(r, "state")) } else { format!("{} · {}", string(r, "kind"), doing) })));
        }
    }
    Some(col.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use core::prelude::v1::test;
    #[test] fn to_do_stays_shut_until_opened_and_subagents_are_open_while_any_works_unless_chosen() {
        let thread = json!({"id": "h|t1", "todos": {"count": "1/3"}, "subs": {"live": 2}});
        let mut p = Panels::default();
        assert!(!todos_is_open(&p, &thread) && subs_is_open(&p, &thread));
        p.todos_open.insert("h|t1".into(), true); p.subs_open.insert("h|t1".into(), false);
        assert!(todos_is_open(&p, &thread) && !subs_is_open(&p, &thread), "the person's choice holds");
        let done = json!({"id": "h|t1", "subs": {"live": 0}}); assert!(!subs_is_open(&Panels::default(), &done), "all done: folded");
        assert!(!todos_is_open(&p, &json!({"id": "h|t1"})) && !subs_is_open(&p, &json!({"id": "h|t1"})), "no list, no panel");
    }
}
