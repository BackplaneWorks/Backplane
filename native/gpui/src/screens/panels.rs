//! The thread's to-do list and subagents: two cells in the header bar ("To-do 1/4", "Subagents · 2 at work") that open their rows in a dock under it (client
//! `Todos.*`, subs `Subs.*`). The rows come from the hub (`thread.todos`, `thread.subs`); whether a panel is open is this device's: to-do stays shut until opened,
//! subagents are open while any is at work and fold to their head once all are done (a click on the head holds the choice).
use crate::{hub::Command, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Default)]
pub struct Panels { pub todos_open: HashMap<String, bool>, pub subs_open: HashMap<String, bool>, pub sub_open: std::collections::HashSet<String> }

/// A subagent row's drill-down is open: this device's choice, by thread and the call that started it (a click shows it at once: the rows carry it).
pub fn sub_is_open(p: &Panels, thread: &Value, key: &str) -> bool { !key.is_empty() && p.sub_open.contains(&format!("{}\u{1f}{key}", string(thread, "id"))) }

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

fn field_none(fields: &std::collections::HashMap<String, super::settings::FieldSlot>, id: &str) -> bool { !fields.contains_key(&format!("todo.{id}")) }
/// The fields the to-do list types into: each step's text and the new one's (typed through `bfield`, sent on Enter).
pub fn form_fields(data: &Value) -> Vec<super::settings::Field> {
    let t = &data["thread"]["todos"];
    if !t.is_object() { return vec![] }
    let f = |name: String, hint: &str, value: String| super::settings::Field { name, hint: hint.into(), secret: false, value, vault: false };
    let mut out = vec![f("todonew".into(), "Add an item", String::new())];
    for r in t["rows"].as_array().into_iter().flatten() { out.push(f(format!("todo.{}", string(r, "id")), "To-do", string(r, "text"))); }
    out
}
/// The dock under the bar: the open panels' rows.
pub fn dock(cx: &mut Context<Backplane>, light: bool, fields: &std::collections::HashMap<String, super::settings::FieldSlot>, p: &Panels, thread: &Value) -> Option<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    let (to, su) = (todos_is_open(p, thread), subs_is_open(p, thread));
    if !to && !su { return None }
    let mut col = div().id("panels-dock").flex_shrink_0().max_h(px(300.)).overflow_y_scroll().flex().flex_col().border_b_1().border_color(c("line")).bg(c("panel")).px_3().py_1().text_size(px(12.));
    if to {
        col = col.child(div().text_size(px(11.)).text_color(c("faint")).child(string(&thread["todos"], "head").to_uppercase()));
        let to_rows: Vec<Value> = thread["todos"]["rows"].as_array().cloned().unwrap_or_default();
        for r in to_rows.iter() {
            let st = string(r, "status");
            // s05: a done step a green tick, an open one an empty square, the one in progress an accent square with brighter text
            let mark: AnyElement = match st.as_str() { "completed" => div().text_color(c("ok")).child("✓").into_any_element(),
                "in_progress" => div().size(px(8.)).bg(c("accent")).into_any_element(), _ => div().size(px(8.)).border_1().border_color(c("faint")).into_any_element() };
            let id = string(r, "id"); let n = to_rows.len();
            let (i, row_id) = (to_rows.iter().position(|x| string(x, "id") == id).unwrap_or(0), id.clone());
            let btn = |cx: &mut Context<Backplane>, key: String, label: AnyElement, action: &'static str, value: String| div().id(SharedString::from(key)).px_1().flex().items_center().justify_center().cursor_pointer().text_color(c("dim")).hover(|s| s.text_color(c("text"))).child(label)
                .on_click(cx.listener(move |this, _, _, cx| { crate::screens::settings::flush_fields(this); let _ = this.tx.try_send(Command::Action(action.into(), value.clone())); cx.notify(); }));
            // the reorder and remove buttons show on the row the pointer is on (they keep their place, so nothing moves)
            let group = SharedString::from(format!("todo-row-{id}"));
            let quiet = |cx: &mut Context<Backplane>, key: String, label: &'static str, action: &'static str, value: String| btn(cx, key, div().child(label).into_any_element(), action, value).invisible().group_hover(group.clone(), |s| s.visible());
            let text_c = match st.as_str() { "in_progress" => c("text"), "completed" => c("faint"), _ => c("dim") };
            let field = fields.get(&format!("todo.{id}")).map(|s| s.input.clone());
            col = col.child(div().id(SharedString::from(format!("todo-{id}"))).group(group.clone()).flex().gap_2().items_center().py(px(1.)).hover(|s| s.bg(c("raise")))
                .child(btn(cx, format!("todo-check-{id}"), mark, "todo-check", row_id.clone()).w(px(18.)).h(px(16.)))
                .child(div().flex_1().min_w_0().text_color(text_c).when(st == "completed", |d| d.line_through()).children(field).when(field_none(fields, &id), |d| d.child(string(r, "text"))))
                .child(quiet(cx, format!("todo-up-{id}"), "\u{2191}", "todo-move", format!("{id}\u{1f}{}", i.saturating_sub(1))))
                .child(quiet(cx, format!("todo-down-{id}"), "\u{2193}", "todo-move", format!("{id}\u{1f}{}", (i + 1).min(n.saturating_sub(1)))))
                .child(quiet(cx, format!("todo-del-{id}"), "\u{d7}", "todo-del", row_id)));
        }
        col = col.child(div().flex().gap_2().items_center().py(px(2.)).child(div().flex_1().min_w_0().children(fields.get("todonew").map(|s| s.input.clone())))
            .child(div().id("todo-add").px_2().cursor_pointer().text_color(c("accent")).child("Add").on_click(cx.listener(|this, _, _, cx| crate::screens::settings::send_todo(this, cx)))));
    }
    if su {
        col = col.child(div().mt_1().text_size(px(11.)).text_color(c("faint")).child(format!("SUBAGENTS · {}", string(&thread["subs"], "word").to_uppercase())));
        let tid = string(thread, "id");
        for r in thread["subs"]["rows"].as_array().into_iter().flatten() {
            let working = string(r, "state") == "working"; let doing = string(r, "doing"); let key = string(r, "key");
            let detail = r["detail"].is_object(); let open = detail && sub_is_open(p, thread, &key);
            let state = { let t = string(r, "took"); if t.is_empty() { string(r, "state") } else { format!("{} · {t}", string(r, "state")) } };
            let mark_c = if working { c("accent") } else { c("faint") };
            let mark: AnyElement = if detail { crate::art::icon(if open { "chevron-down" } else { "chevron-right" }, 12., mark_c) } else { div().child(if working { "▸" } else { "✓" }).into_any_element() };
            let mut head = div().id(SharedString::from(format!("sub-{}", if key.is_empty() { string(r, "title") } else { key.clone() }))).flex().gap_2().py(px(1.)).items_center()
                .child(div().w(px(14.)).flex_shrink_0().flex().items_center().text_color(mark_c).child(mark))
                .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(c("text")).child(string(r, "title")))
                .child(div().flex_shrink_0().text_color(c("faint")).child(if detail { state.clone() } else if doing.is_empty() { format!("{} · {}", string(r, "kind"), state) } else { format!("{} · {}", string(r, "kind"), doing) }));
            if detail {
                let k = format!("{tid}\u{1f}{key}");
                head = head.cursor_pointer().hover(|s| s.bg(c("raise"))).on_click(cx.listener(move |this, _, _, cx| { let o = &mut this.screens.panels.sub_open; if !o.remove(&k) { o.insert(k.clone()); } cx.notify(); }));
            }
            let head = head.when(!string(r, "act").is_empty(), |d| { let (act, value) = (string(r, "act"), string(r, "value"));
                d.child(div().id(SharedString::from(format!("{act}-{value}"))).flex_shrink_0().px_1().cursor_pointer().text_color(c("accent")).child(string(r, "label"))
                    .on_click(cx.listener(move |this, _, _, cx| { cx.stop_propagation(); let _ = this.tx.try_send(Command::Action(act.clone(), value.clone())); cx.notify(); }))) });
            let line = |t: String, col_: Rgba| div().w_full().pl(px(22.)).flex().text_size(px(11.)).text_color(col_).child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(t));
            let mut row = div().w_full().flex().flex_col().py(px(2.)).child(head);
            if detail {
                // what it is (harness and type, model, steps, tokens) and what it is doing now (Claude's word for it, else its newest step)
                let meta = string(r, "meta"); if !meta.is_empty() { row = row.child(line(meta, c("faint"))); }
                let now = if !doing.is_empty() { doing.clone() } else { string(r, "last") }; if !now.is_empty() && !open { row = row.child(line(now, c("dim"))); }
            }
            if open {
                let d = &r["detail"]; let label = |l: &'static str, t: String| div().w_full().pl(px(22.)).flex().gap_2().text_size(px(11.)).child(div().flex_shrink_0().w(px(64.)).text_color(c("faint")).child(l)).child(div().flex_1().min_w_0().text_color(c("dim")).child(t));
                row = row.child(label("Runs", string(d, "where")));
                if !doing.is_empty() { row = row.child(label("Doing", doing.clone())); }
                if !string(d, "ask").is_empty() { row = row.child(label("Asked", string(d, "ask"))); }
                let more = d["more"].as_u64().unwrap_or(0);
                let steps: Vec<Value> = d["steps"].as_array().cloned().unwrap_or_default();
                if steps.is_empty() { row = row.child(label("Steps", "none reported yet".into())); }
                for (n, st) in steps.iter().enumerate() {
                    let t = format!("{} {}", string(st, "kind"), string(st, "text"));
                    row = row.child(label(if n == 0 { "Steps" } else { "" }, if n == 0 && more > 0 { format!("({more} earlier) {t}") } else { t }));
                }
                if !string(d, "outcome").is_empty() { row = row.child(label("Reported", string(d, "outcome"))); }
            }
            col = col.child(row);
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
