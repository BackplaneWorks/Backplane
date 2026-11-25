//! Pieces of a sidebar row that follow 0.14: the status dot (working blue, finished green, wanting the person red, waiting on its subagents
//! the waiting colour, idle grey; `style.css` .dot.* of 0.14) and the rename field drawn in the row itself (a right click's or the ▾ menu's
//! Rename: the hub's `renaming`, no page over the window). The hub decides the status word and what is being renamed; this only draws.
use super::settings::FieldSlot;
use crate::{hub::Command, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::collections::HashMap;

/// How a row's dot is drawn: its colour token, whether it is round (0.14: work, finished and idle are round; a block or a failure square) and
/// whether it is only an outline (a failed turn waits on nothing: r01).
#[derive(Debug, PartialEq)]
pub struct Dot { pub token: &'static str, pub round: bool, pub hollow: bool }

/// The dot for a row's `status` (the hub's signal word) and `state` (its turn: run, fail, stop, idle).
pub fn dot_of(status: &str, state: &str) -> Dot {
    let d = |token, round, hollow| Dot { token, round, hollow };
    match status {
        "blocked" => d("bad", false, false),
        "failed" => d("bad", false, true),
        "stalled" => d("warn", false, false),
        "active" | "working" => d("accent", true, false),
        // waiting on subagents or work it started: not finished, not idle
        "waiting" | "monitoring" => d("waiting", true, false),
        "finished" => d("ok", true, false),
        "inactive" | "settled" | "stopped" => d("faint", true, false),
        _ => match state { "run" => d("accent", true, false), "fail" => d("bad", false, true), _ => d("faint", true, false) },
    }
}

/// The 6 px dot of a row.
pub fn dot(light: bool, t: &Value) -> AnyElement {
    let k = dot_of(t["status"].as_str().unwrap_or(""), t["state"].as_str().unwrap_or(""));
    let col = theme::color(light, k.token);
    div().size(px(6.)).flex_shrink_0().when(k.round, |d| d.rounded_full()).when(k.hollow, |d| d.border_1().border_color(col)).when(!k.hollow, |d| d.bg(col)).into_any_element()
}

/// Whether the hub's open rename is this row's (`qid`, the row's qualified id) of this kind ("thread" or "project").
pub fn renaming(data: &Value, qid: &str, kind: &str) -> bool {
    let r = &data["renaming"];
    if !r.is_object() { return false }
    let k = string(r, "kind");
    let k = if k.is_empty() { if string(r, "title").contains("project") { "project".to_string() } else { "thread".to_string() } } else { k };
    let raw = |s: &str| s.rsplit('|').next().unwrap_or("").to_string();
    k == kind && raw(&string(r, "id")) == raw(qid)
}

/// Whether a row will draw the rename: the thread's or project's row is in the sidebar's picture (and the sidebar is open).
pub fn rename_in_sidebar(data: &Value, sidebar_open: bool) -> bool {
    let r = &data["renaming"];
    if !r.is_object() || !sidebar_open { return false }
    let id = string(r, "id"); let raw = id.rsplit('|').next().unwrap_or("").to_string();
    // the hub's sections draw a thread's title box in its row, never a project's or a group's (those take the bar)
    if crate::screens::sections::on(data) { return string(r, "kind") == "thread" && walk(&data["sections"], &raw) }
    fn walk(v: &Value, raw: &str) -> bool {
        match v {
            Value::Object(m) => m.get("id").and_then(Value::as_str).map_or(false, |id| id.rsplit('|').next() == Some(raw) && m.contains_key("title")) || m.values().any(|x| walk(x, raw)),
            Value::Array(a) => a.iter().any(|x| walk(x, raw)),
            _ => false,
        }
    }
    let projects = data["projects"].as_array().into_iter().flatten().any(|p| {
        let head = p["id"].as_str().map_or(false, |id| id.rsplit('|').next() == Some(raw.as_str()));
        head || (p["closed"] != true && walk(p, &raw)) // a folded project draws its head alone
    });
    projects || ["active", "activeSettled", "legacy"].iter().any(|k| walk(&data[*k], &raw))
}

/// The title box in place of a row's title: Enter keeps it, Esc leaves it (the window's own keys: settings::submit, close_question).
/// A press in it never selects or drags the row.
pub fn field(light: bool, fields: &HashMap<String, FieldSlot>, id: &str) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    div().id(SharedString::from(format!("rename-{id}"))).flex_1().min_w_0().h(crate::scale::tpx(22.)).px_1().flex().items_center().border_1().border_color(c("accent")).bg(c("view")).text_size(crate::scale::tpx(12.))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
        .on_click(|_, _, cx| cx.stop_propagation())
        .children(fields.get("title").map(|s| s.input.clone())).into_any_element()
}

/// The rename when no row can show it (its project folded, the sidebar shut): a small bar at the top of the main column, never the whole of it.
pub fn bar(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, data: &Value, sidebar_open: bool) -> Option<AnyElement> {
    let r = &data["renaming"];
    if !r.is_object() || rename_in_sidebar(data, sidebar_open) { return None }
    let c = |n: &str| theme::color(light, n);
    let id = string(r, "id");
    let btn = |id: &'static str, label: String| div().id(id).px_2().h(crate::scale::tpx(22.)).flex().items_center().text_size(crate::scale::tpx(12.)).cursor_pointer().border_1().border_color(c("line")).bg(c("raise")).hover(|s| s.bg(c("panel"))).child(label);
    Some(div().id("rename-bar").absolute().top_0().left_0().right_0().flex().items_center().gap_2().px_3().py_1().border_b_1().border_color(c("line")).bg(c("panel")).occlude()
        .child(div().text_size(crate::scale::tpx(12.)).text_color(c("dim")).child(string(r, "title")))
        .child(div().flex_1().max_w(px(520.)).flex().child(field(light, fields, "bar")))
        .child(btn("rename-bar-no", string(r, "no")).on_click(cx.listener(|this, _, _, cx| { let _ = this.tx.try_send(crate::hub::Command::Action("rename-no".into(), String::new())); cx.notify(); })))
        .child(btn("rename-bar-yes", string(r, "yes")).on_click(cx.listener(move |this, _, _, cx| {
            super::settings::flush_fields(this);
            let text = this.screens.fields.get("title").map(|s| s.input.read(cx).content.to_string()).unwrap_or_default();
            if let Some(c) = super::tools::rename_save(&id, &text) { let _ = this.tx.try_send(c); }
            cx.notify();
        }))).into_any_element())
}

/// The height of the sidebar's fold row (the fold chevron, the search box, Add project), and of the folded strip's unfold row, so the chevron
/// sits on the same spot folded and unfolded (0.14 style.css .side-fold / .side-strip).
pub const FOLD_ROW: f32 = 46.;
/// The project badges the folded strip shows: (raw project id, title, the project) in the sidebar's order.
/// The press a row's pin makes: Unpin on a pinned row, Pin on the others (the hub's `row-pin` / `row-unpin`, the context menu's too).
pub fn pin_press(t: &Value) -> (&'static str, String) { (if t["pinned"].as_bool().unwrap_or(false) { "row-unpin" } else { "row-pin" }, t["id"].as_str().unwrap_or("").to_string()) }
/// A thread row's pin: drawn on a pinned row, shown on hover on the others; a press pins or unpins and does not select the row.
pub fn pin_button(cx: &mut Context<Backplane>, light: bool, t: &Value, group: SharedString) -> AnyElement { pin_button_as(cx, light, t, group, "pin") }
/// The same pin under another control name (`<name>:<thread>`): a project page's row and the sidebar's row of one thread are both on screen.
pub fn pin_button_as(cx: &mut Context<Backplane>, light: bool, t: &Value, group: SharedString, name: &'static str) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let (action, value) = pin_press(t); let pinned = action == "row-unpin";
    /* i18n-ok: the context menu's words, drawn through trs */ let tip = crate::i18n::trs(if pinned { "Unpin" } else { "Pin" });
    let raw = value.rsplit('|').next().unwrap_or("").to_string();
    crate::ui::with_rect(div().id(SharedString::from(format!("{name}-{value}"))), move || format!("{name}:{raw}")).flex_shrink_0().w(px(16.)).h(px(16.)).flex().items_center().justify_center().cursor_pointer()
        .when(!pinned, |d| d.invisible().group_hover(group, |s| s.visible()))
        .tooltip(crate::keys::tip(&tip, "", light))
        .child(crate::art::icon("pin", 12., c(if pinned { "dim" } else { "faint" })))
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action(action.into(), value.clone())); cx.stop_propagation(); cx.notify(); }))
        .into_any_element()
}

/// A row's status label (the hub's `label` and `tone`): a short word in its tone's colour, filled when it asks the person (Approve, Input,
/// Sign in), outlined otherwise. Read at a glance where a 6 px dot was not. `None` for a row with nothing to say.
pub struct Pill { text: String, token: &'static str, filled: bool, light: bool }
pub fn pill(light: bool, t: &Value) -> Option<Pill> {
    let text = string(t, "label"); if text.is_empty() { return None }
    let token = pill_token(t["tone"].as_str().unwrap_or(""));
    Some(Pill { filled: t["word"] == "needs", text, token, light })
}
/// The colour token of a label's tone.
pub fn pill_token(tone: &str) -> &'static str { match tone { "bad" => "bad", "run" => "accent", "wait" => "waiting", "ok" => "ok", _ => "dim" } }
pub trait PillEl { fn clone_el(&self) -> Option<AnyElement>; }
impl PillEl for Option<Pill> {
    fn clone_el(&self) -> Option<AnyElement> {
        let p = self.as_ref()?; let col = theme::color(p.light, p.token);
        Some(div().flex_shrink_0().h(px(15.)).px(px(4.)).flex().items_center().text_size(crate::scale::tpx(10.)).line_height(px(13.)).border_1().border_color(col)
            .when(p.filled, |d| d.bg(col).text_color(theme::color(p.light, "bg"))).when(!p.filled, |d| d.text_color(col))
            .child(crate::i18n::trs(&p.text)).into_any_element())
    }
}
/// Snooze, under the pointer: a clock that opens the hub's spans (`snooze`: 1 hour, Tomorrow, 1 week); on a snoozed row (`wake`) it wakes it in
/// one press. None for a row that can do neither.
pub fn snooze_button(cx: &mut Context<Backplane>, light: bool, t: &Value, group: SharedString) -> Option<AnyElement> {
    let (wake, opts) = (string(t, "wake"), t["snooze"].as_array().cloned().unwrap_or_default());
    if wake.is_empty() && opts.is_empty() { return None }
    let c = |n: &str| theme::color(light, n);
    let id = string(t, "id"); let raw = id.rsplit('|').next().unwrap_or("").to_string();
    /* i18n-ok: the menu's words, drawn through trs */ let tip = crate::i18n::trs(if wake.is_empty() { "Snooze" } else { "Wake" });
    Some(crate::ui::with_rect(div().id(SharedString::from(format!("snooze-{id}"))), move || format!("snooze:{raw}")).flex_shrink_0().w(px(16.)).h(px(16.)).flex().items_center().justify_center().cursor_pointer()
        .invisible().group_hover(group, |s| s.visible()).hover(|s| s.bg(c("line")))
        .tooltip(crate::keys::tip(&tip, "", light))
        .child(crate::art::icon(if wake.is_empty() { "clock" } else { "bell" }, 12., c("dim")))
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |this, e: &ClickEvent, _, cx| { cx.stop_propagation();
            if !wake.is_empty() { let _ = this.tx.try_send(Command::Action("row-snooze".into(), wake.clone())); cx.notify(); return }
            /* i18n-ok: menu labels, drawn through trs */ let items = opts.iter().map(|o| crate::screens::ctxmenu::Item { label: format!("Snooze {}", string(o, "label").to_lowercase()), action: "row-snooze".into(), value: string(o, "value"), confirm: false, note: String::new() }).collect();
            this.ctx_open(e.position(), items, cx); }))
        .into_any_element())
}

/// One hover/row action button on a sidebar thread row, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowAction { Wake, Snooze, Settle, Pin, Unpin }

impl RowAction {
    /// Icon name in `art::icon` for this action.
    pub fn icon(self) -> &'static str {
        match self { Self::Wake => "bell", Self::Snooze => "clock", Self::Settle => "check", Self::Pin | Self::Unpin => "pin" }
    }
    /// Tooltip label for this action (drawn through `crate::i18n::trs` by the button builders).
    pub fn tip(self) -> &'static str {
        match self { Self::Wake => "Wake", Self::Snooze => "Snooze", Self::Settle => "Settle", Self::Pin => "Pin", Self::Unpin => "Unpin" } /* i18n-ok: tooltip words drawn through trs by the button builders */
    }
}

/// Pure planner for a sidebar thread row's hover action strip: emits at most one action per slot, never duplicating Settle or Wake/Snooze.
pub fn row_actions(t: &Value) -> Vec<RowAction> {
    let mut out = Vec::with_capacity(3);
    if !string(t, "wake").is_empty() { out.push(RowAction::Wake); }
    else if t["snooze"].as_array().is_some_and(|a| !a.is_empty()) { out.push(RowAction::Snooze); }
    if !string(t, "settle").is_empty() { out.push(RowAction::Settle); }
    out.push(if t["pinned"].as_bool().unwrap_or(false) { RowAction::Unpin } else { RowAction::Pin });
    out
}

/// The row's hover action strip (`Snooze`/`Wake`, `Settle`, `Pin`/`Unpin`), built once from `row_actions(t)` so no icon can appear twice.
pub fn action_strip(cx: &mut Context<Backplane>, light: bool, t: &Value, group: SharedString) -> Div {
    let mut strip = div().flex().items_center().gap_1();
    for act in row_actions(t) {
        match act {
            RowAction::Wake | RowAction::Snooze => { strip = strip.children(snooze_button(cx, light, t, group.clone())); }
            RowAction::Settle => { strip = strip.children(settle_button(cx, light, t, group.clone())); }
            RowAction::Pin | RowAction::Unpin => { strip = strip.child(pin_button(cx, light, t, group.clone())); }
        }
    }
    strip
}

/// Settle in one press: a check on a row the hub says can be settled (`settle`: its id), shown under the pointer. None for any other row.
pub fn settle_button(cx: &mut Context<Backplane>, light: bool, t: &Value, group: SharedString) -> Option<AnyElement> {
    let value = string(t, "settle"); if value.is_empty() { return None }
    let c = |n: &str| theme::color(light, n);
    let raw = value.rsplit('|').next().unwrap_or("").to_string();
    Some(crate::ui::with_rect(div().id(SharedString::from(format!("settle-{value}"))), move || format!("settle:{raw}")).flex_shrink_0().w(px(16.)).h(px(16.)).flex().items_center().justify_center().cursor_pointer()
        .invisible().group_hover(group, |s| s.visible()).hover(|s| s.bg(c("line")))
        /* i18n-ok: the context menu's word, drawn through trs */ .tooltip(crate::keys::tip(&crate::i18n::trs("Settle"), "", light))
        .child(crate::art::icon("check", 12., c("dim")))
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("row-settle".into(), value.clone())); cx.stop_propagation(); cx.notify(); }))
        .into_any_element())
}

pub fn strip_projects(data: &Value) -> Vec<(String, String, &Value)> {
    data["projects"].as_array().into_iter().flatten().map(|p| { let id = string(p, "id"); (id.rsplit('|').next().unwrap_or("").to_string(), string(p, "title"), p) }).filter(|(id, _, _)| !id.is_empty()).collect()
}

impl Backplane {
    /// The folded sidebar: a 44 px strip (0.14's View.strip) with the unfold chevron where the fold chevron was, over the projects' badges,
    /// each in its colour; a badge opens its project's page.
    pub(crate) fn side_strip(&self, cx: &mut Context<Self>, light: bool) -> Vec<AnyElement> {
        let c = |n: &str| theme::color(light, n);
        let mut out = vec![
            // the hub picker's row, empty and as tall as it (hubpick.rs: its ROW_H - 1 row and the rule under it): the unfold chevron lands on the fold chevron's height
            div().flex_shrink_0().h(crate::scale::tpx(super::head::ROW_H)).border_b_1().border_color(c("rule")).into_any_element(),
            div().h(crate::scale::tpx(FOLD_ROW)).px_2().flex().items_center().child(crate::ui::with_rect(div().id("unfold-sidebar"), || /* i18n-ok: a control id */ "ui:Unfold sidebar".to_string()).cursor_pointer()
                .tooltip(crate::keys::tip(&crate::i18n::trs("Unfold the sidebar"), &crate::keys::hint(&self.local, "sidebar"), light)).child(crate::ui::ic(light, "chevron-right", 16., "dim"))
                .on_click(cx.listener(|this, _, _, cx| this.side_flip(cx)))).into_any_element(),
        ];
        for (pid, title, p) in strip_projects(&self.data) {
            let on = self.project_page.as_deref() == Some(pid.as_str());
            out.push(div().id(SharedString::from(format!("strip-{pid}"))).w_full().h(crate::scale::tpx(30.)).flex().items_center().justify_center().cursor_pointer().hover(|s| s.bg(c("raise"))).when(on, |d| d.bg(c("raise")))
                .tooltip(crate::keys::tip(&title, "", light)).child(super::badge::mark(light, p))
                .on_click(cx.listener(move |this, _, _, cx| this.open_project(pid.clone(), cx))).into_any_element());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*; use core::prelude::v1::test; use serde_json::json;
    #[test] fn a_rows_pin_unpins_a_pinned_thread_and_pins_the_others() {
        assert_eq!(pin_press(&serde_json::json!({"id":"http://h:1|t7","pinned":true})), ("row-unpin", "http://h:1|t7".to_string()));
        assert_eq!(pin_press(&serde_json::json!({"id":"t1"})), ("row-pin", "t1".to_string()));
    }
    #[test] fn the_dots_are_0_14s_colours() {
        assert_eq!(dot_of("active", "run").token, "accent", "working is blue");
        assert_eq!(dot_of("finished", "idle"), Dot { token: "ok", round: true, hollow: false }, "finished is the green dot");
        assert_eq!(dot_of("waiting", "idle").token, "waiting", "waiting on subagents is neither finished nor working");
        assert_eq!(dot_of("blocked", "idle"), Dot { token: "bad", round: false, hollow: false }, "wanting the person: a red square");
        assert_eq!(dot_of("failed", "fail"), Dot { token: "bad", round: false, hollow: true }, "a failed turn: the red square drawn hollow");
        for s in ["inactive", "settled", "stopped"] { assert_eq!(dot_of(s, "idle").token, "faint", "{s}"); }
        assert_eq!(dot_of("", "run").token, "accent"); assert_eq!(dot_of("", "").token, "faint");
        for s in ["active", "waiting", "finished", "blocked", "failed", "inactive"] { assert_eq!(dot_of(s, "").token == "ok", s == "finished", "only finished is green: {s}"); }
    }
    #[test] fn the_rename_is_drawn_in_its_own_row_or_in_a_bar_when_no_row_shows() {
        let d = json!({"renaming": {"id": "http://h:1|t7", "kind": "thread", "title": "Rename thread"}, "projects": [{"id": "http://h:1|p1", "title": "P", "threads": [{"id": "http://h:1|t7", "title": "x"}]}]});
        assert!(renaming(&d, "http://h:1|t7", "thread") && !renaming(&d, "http://h:1|t8", "thread") && !renaming(&d, "http://h:1|t7", "project"));
        assert!(rename_in_sidebar(&d, true) && !rename_in_sidebar(&d, false));
        let p = json!({"renaming": {"id": "h|p1", "title": "Rename project"}, "projects": [{"id": "h|p1", "title": "P", "threads": []}]});
        assert!(renaming(&p, "h|p1", "project"), "an older hub's rename says its kind in its title");
        let hidden = json!({"renaming": {"id": "h|t9", "kind": "thread"}, "projects": [{"id": "h|p1", "title": "P", "threads": []}]});
        assert!(!rename_in_sidebar(&hidden, true));
        assert!(!renaming(&Value::Null, "h|t1", "thread"));
        assert_eq!(strip_projects(&d).iter().map(|x| (x.0.as_str(), x.1.as_str())).collect::<Vec<_>>(), [("p1", "P")], "the folded strip's badges: each project by its raw id");
        let mut folded = d.clone(); folded["projects"][0]["closed"] = json!(true); assert!(!rename_in_sidebar(&folded, true), "a folded project shows no row");
    }
    #[test] fn row_actions_never_duplicate_settle_or_icons() {
        let snz = json!([{"label":"1 hour","value":"h|t1|1h"}]);
        let done = json!({"id":"h|t1","snooze":snz,"settle":"h|t1","pinned":false});
        let acts = row_actions(&done);
        assert_eq!(acts, vec![RowAction::Snooze, RowAction::Settle, RowAction::Pin]);
        assert_eq!(acts.iter().map(|a| a.icon()).collect::<Vec<_>>(), vec!["clock", "check", "pin"]);
        assert_eq!(acts.iter().map(|a| a.tip()).collect::<Vec<_>>(), vec!["Snooze", "Settle", "Pin"]);
        assert_eq!(acts.iter().filter(|a| a.icon() == "check").count(), 1);

        let snoozed_pinned = json!({"id":"h|t2","wake":"h|t2|off","snooze":snz,"settle":"h|t2","pinned":true});
        assert_eq!(row_actions(&snoozed_pinned), vec![RowAction::Wake, RowAction::Settle, RowAction::Unpin], "Wake suppresses Snooze");

        let running = json!({"id":"h|t3","snooze":snz,"settle":"","pinned":false});
        assert_eq!(row_actions(&running), vec![RowAction::Snooze, RowAction::Pin]);

        let needs = json!({"id":"h|t4","snooze":[],"settle":"","pinned":false});
        assert_eq!(row_actions(&needs), vec![RowAction::Pin]);

        for has_wake in [false, true] {
            for has_snooze in [false, true] {
                for has_settle in [false, true] {
                    for pinned in [false, true] {
                        let row = json!({
                            "id": "h|t",
                            "wake": if has_wake { "h|t|off" } else { "" },
                            "snooze": if has_snooze { snz.clone() } else { json!([]) },
                            "settle": if has_settle { "h|t" } else { "" },
                            "pinned": pinned,
                        });
                        let got = row_actions(&row);
                        let mut uniq_acts = std::collections::HashSet::new();
                        let mut uniq_icons = std::collections::HashSet::new();
                        let mut uniq_tips = std::collections::HashSet::new();
                        for a in &got {
                            assert!(uniq_acts.insert(format!("{a:?}")), "duplicate action in {got:?}");
                            assert!(uniq_icons.insert(a.icon()), "duplicate icon {} in {got:?}", a.icon());
                            assert!(uniq_tips.insert(a.tip()), "duplicate tip {} in {got:?}", a.tip());
                        }
                    }
                }
            }
        }
    }
}
