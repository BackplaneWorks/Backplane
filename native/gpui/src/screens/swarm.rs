//! Swarms (docs/SWARM.md) from the hub's screen model: a project's swarm rows (`projects[].swarms`), the swarm page (a room page with
//! `bot.swarm`: member chips, the goal and state, the controls, the add picker, the board as one feed, the post box) and the New swarm form
//! (`newSwarm`). Every press is a view action the hub's presenter maps (present/swarmform.rs, `swarm-act`); nothing here decides what a swarm does.
use super::settings::{flush_fields, Field, FieldSlot};
use crate::{hub::Command, i18n::{tf, trs}, script::{Ctl, CtlEntry}, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::collections::HashMap;

const SEP: char = '\u{1f}';
fn arr<'a>(v: &'a Value, k: &str) -> impl Iterator<Item = &'a Value> { v[k].as_array().into_iter().flatten() }
fn act(a: &str, v: &str) -> Command { Command::Action(a.into(), v.into()) }

/// A sidebar row's `nav` as the view action that opens it: `@room:<id>` is the room's page.
/// A room's page is open over the window's thread (a swarm's page over the thread its viewer is drawn for): that thread has no header
/// and no sidebar row is marked; only its viewer shows, beside the page.
pub fn over_thread(data: &Value) -> bool { data["bot"]["kind"] == "room" }
pub fn nav_action(nav: &str) -> Option<(String, String)> { nav.strip_prefix("@room:").filter(|r| !r.is_empty()).map(|r| ("room".to_string(), r.to_string())) }
/// A member's or post's colour, "#rrggbb" from the hub.
pub fn hex(c: &str) -> Option<Rgba> { let h = c.strip_prefix('#')?; (h.len() == 6).then(|| u32::from_str_radix(h, 16).ok().map(rgb)).flatten() }
/// The dot for a member's live state or a swarm's state, as a thread row's dot is drawn (siderow::dot_of).
pub fn dot_of(state: &str) -> super::siderow::Dot {
    super::siderow::dot_of(match state { "paused" => "stalled", "idle" | "removed" | "bot" | "done" => "inactive", s => s }, "")
}
fn dot(light: bool, state: &str) -> AnyElement {
    let k = dot_of(state); let col = theme::color(light, k.token);
    div().size(px(6.)).flex_shrink_0().when(k.round, |d| d.rounded_full()).when(k.hollow, |d| d.border_1().border_color(col)).when(!k.hollow, |d| d.bg(col)).into_any_element()
}
/// The byte ranges of `@name` in a post's text for the members it mentions (`mentions` are member ids; `names` maps an id to its board name), whole names in any case.
pub fn mention_ranges(text: &str, mentions: &[String], names: &HashMap<String, String>) -> Vec<std::ops::Range<usize>> {
    let want: Vec<String> = mentions.iter().filter_map(|m| names.get(m)).map(|n| n.to_lowercase()).collect();
    let low = text.to_lowercase();
    if want.is_empty() || low.len() != text.len() { return vec![] }
    let word = |c: char| c.is_alphanumeric() || c == '-' || c == '_';
    let mut out = vec![];
    for (i, _) in text.match_indices('@') {
        if text[..i].chars().next_back().map_or(false, word) { continue }
        let end = text[i + 1..].find(|c: char| !word(c)).map_or(text.len(), |e| i + 1 + e);
        let end = if text[i + 1..end].ends_with('-') { end - 1 } else { end };
        if want.iter().any(|w| low[i + 1..end] == *w) { out.push(i..end) }
    }
    out
}
/// The add picker's Add: `add|<provider>|<model>|<role>`.
pub fn add_value(add: &Value, role: &str) -> String { format!("add|{}|{}|{}", string(add, "provider"), string(add, "model"), role.trim()) }

// ---- the sidebar ----

/// A project's swarm rows, above its threads: the name, the state's dot, "N members · M working" and the ago; a click opens its page.
pub fn side_rows(cx: &mut Context<Backplane>, light: bool, project: &Value, open_room: &str, ctl: &mut Vec<CtlEntry>) -> Vec<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    arr(project, "swarms").filter_map(|r| {
        let (action, room) = nav_action(&string(r, "nav"))?;
        let name = string(r, "name");
        ctl.push(CtlEntry { id: format!("swarm:{name}"), label: name.clone(), ctl: Ctl::Node(action.clone(), room.clone()) });
        let sel = room == open_room;
        let line = tf("gpui.swarm.row", &[("n", r["members"].as_u64().unwrap_or(0).into()), ("w", r["working"].as_u64().unwrap_or(0).into())]);
        let el = crate::ui::with_rect(div().id(SharedString::from(format!("swarm-{}", string(r, "id")))), { let n = name.clone(); move || format!("swarm:{n}") })
            .flex().items_center().gap_2().pl(px(30.)).pr_3().h(crate::scale::tpx(38.)).border_l_2().border_color(if sel { c("accent") } else { c("panel") })
            .bg(if sel { c("raise") } else { c("panel") }).hover(|s| s.bg(c("raise"))).cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| this.go(&action, room.clone(), cx)))
            .child(dot(light, &string(r, "state")))
            .child(div().flex_1().min_w_0().flex().flex_col().line_height(crate::scale::tpx(16.))
                .child(div().overflow_hidden().whitespace_nowrap().text_ellipsis().child(name))
                .child(div().text_size(crate::scale::tpx(11.)).text_color(c("faint")).overflow_hidden().whitespace_nowrap().child(line)))
            .child(div().text_size(crate::scale::tpx(11.)).text_color(c("faint")).child(string(r, "ago")));
        Some(el.into_any_element())
    }).collect()
}

// ---- typed fields ----

/// The typed fields these screens need (screens::settings::sync keeps one input each): the New swarm form's topic, goal, seed and each member's role;
/// the add picker's role (held here until Add).
pub fn form_fields(data: &Value) -> Vec<Field> {
    let f = |name: String, hint: &str, value: String, vault: bool| Field { name, hint: trs(hint), secret: false, value, vault };
    let mut out = vec![];
    let ns = &data["newSwarm"];
    if ns.is_object() {
        out.push(f("stopic".into(), "Topic", string(ns, "topic"), false)); // i18n-ok: a hint, drawn through trs in f
        out.push(f("sgoal".into(), "Goal, in a line", string(ns, "goal"), false)); // i18n-ok: a hint, drawn through trs in f
        out.push(f("sseed".into(), "First message to every member", string(ns, "seed"), false)); // i18n-ok: a hint, drawn through trs in f
        for (i, m) in arr(ns, "members").enumerate() { out.push(f(format!("srole.{i}"), "Role (optional)", string(m, "role"), false)); } // i18n-ok: a hint, drawn through trs in f
    }
    if data["bot"]["swarm"]["add"]["open"] == true { out.push(f("sarole".into(), "Role (optional)", String::new(), true)); } // i18n-ok: a hint, drawn through trs in f
    out
}
pub fn focus_target(data: &Value) -> Option<String> { data["newSwarm"].is_object().then(|| "stopic".to_string()) }
pub fn close_form(data: &Value) -> Option<Command> {
    if data["newSwarm"].is_object() { return Some(act("form-close", "@snew")) }
    (data["bot"]["swarm"]["add"]["open"] == true).then(|| act("form-close", "@sadd"))
}

// ---- controls ----

fn send(this: &mut Backplane, a: &str, v: &str, cx: &mut Context<Backplane>) { flush_fields(this); let _ = this.tx.try_send(act(a, v)); cx.notify(); }
fn button(cx: &mut Context<Backplane>, light: bool, id: String, label: String, on: bool, a: &'static str, v: String) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let d = div().id(SharedString::from(id)).px_2().py_1().text_size(crate::scale::tpx(12.)).cursor_pointer().border_1().whitespace_nowrap();
    let d = if on { d.bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")) } else { d.bg(c("raise")).border_color(c("line")).hover(|s| s.bg(c("panel"))) };
    d.child(label).on_click(cx.listener(move |this, _, _, cx| send(this, a, &v, cx))).into_any_element()
}
fn inert(light: bool, label: String) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    div().px_2().py_1().text_size(crate::scale::tpx(12.)).border_1().border_color(c("line")).bg(c("raise")).text_color(c("faint")).opacity(0.6).child(label).into_any_element()
}
fn field_box(light: bool, fields: &HashMap<String, FieldSlot>, name: &str, min_h: f32) -> Option<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    fields.get(name).map(|s| div().w_full().max_w(px(560.)).min_h(px(min_h)).px_2().py_1().border_1().border_color(c("line")).bg(c("view")).child(s.input.clone()).into_any_element())
}
/// An agent's and its models' choices: `pa`/`ma` are the actions, `pre` goes before each value (a member row's index).
fn picks(cx: &mut Context<Backplane>, light: bool, id: &str, row: &Value, pa: &'static str, ma: &'static str, pre: &str) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let provs = div().flex().flex_wrap().gap_1().children(arr(row, "providers").enumerate().map(|(i, p)| {
        let label = string(p, "label");
        if p["ok"] == false { return inert(light, label) }
        button(cx, light, format!("{id}-p{i}"), label, p["on"] == true, pa, format!("{pre}{}", string(p, "value")))
    }).collect::<Vec<_>>());
    let models = div().id(SharedString::from(format!("{id}-models"))).flex().flex_wrap().gap_1().max_h(px(58.)).overflow_y_scroll().children(arr(row, "models").enumerate().map(|(i, m)| {
        let (label, note) = (string(m, "label"), string(m, "note"));
        let label = if label == "default" { trs("default") } else { label };
        let el = button(cx, light, format!("{id}-m{i}"), label, m["on"] == true, ma, format!("{pre}{}", string(m, "value")));
        if note.is_empty() { el } else { div().id(SharedString::from(format!("{id}-mt{i}"))).tooltip(crate::keys::tip(&note, "", light)).child(el).into_any_element() }
    }).collect::<Vec<_>>());
    div().flex().flex_col().gap_1().child(provs).child(div().text_color(c("dim")).child(models)).into_any_element()
}

// ---- the swarm page ----

struct Who { color: Option<Rgba> }
thread_local! { /** the board's scroll, the swarm it shows and how many posts it last drew */ static BOARD: std::cell::RefCell<(ScrollHandle, String, usize)> = std::cell::RefCell::new((ScrollHandle::new(), String::new(), 0)); }
fn post_el(light: bool, p: &Value, who: &HashMap<String, Who>, names: &HashMap<String, String>, claims: &HashMap<String, String>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let text = string(p, "text");
    if p["guard"] == true {
        return div().px_3().py(px(3.)).text_size(crate::scale::tpx(11.)).text_color(c("faint")).child(text).into_any_element()
    }
    let tags: Vec<String> = arr(p, "tags").filter_map(|t| t.as_str().map(str::to_string)).collect();
    let synth = tags.iter().any(|t| t == "synthesis");
    let mine = p["mine"] == true;
    let col = if mine { Some(c("accent")) } else { hex(&string(p, "color")).or_else(|| who.get(&string(p, "thread")).and_then(|w| w.color)) };
    let edge = col.unwrap_or(c("rule"));
    let name = if mine { trs("You") } else { string(p, "from") };
    let mentions: Vec<String> = arr(p, "mentions").filter_map(|m| m.as_str().map(str::to_string)).collect();
    let hl = mention_ranges(&text, &mentions, names).into_iter().map(|r| (r, HighlightStyle { font_weight: Some(FontWeight::BOLD), color: Some(c("text").into()), ..Default::default() })).collect::<Vec<_>>();
    let body = StyledText::new(text).with_highlights(hl);
    let mut head = div().flex().items_center().gap_2().child(div().font_weight(FontWeight::BOLD).text_color(edge).child(name));
    for t in &tags { head = head.child(div().px_1().text_size(crate::scale::tpx(10.)).border_1().border_color(c("line")).text_color(c("dim")).child(trs(t))); }
    head = head.child(div().text_size(crate::scale::tpx(11.)).text_color(c("faint")).child(string(p, "ago")));
    let held = string(p, "held");
    if !held.is_empty() { head = head.child(div().text_size(crate::scale::tpx(10.)).text_color(c("faint")).child(tf("gpui.swarm.held", &[("why", held.clone().into())]))); }
    let files: Vec<String> = arr(p, "files").filter_map(|f| f.as_str().map(str::to_string)).collect();
    let claim = claims.get(&string(p, "id"));
    let files_el = (!files.is_empty()).then(|| div().flex().flex_wrap().gap_1().children(files.into_iter().map(|f| div().px_1().text_size(crate::scale::tpx(11.)).bg(c("raise")).border_1().border_color(c("line")).child(f)))
        .children(claim.filter(|s| s.as_str() == "waiting").map(|_| div().px_1().text_size(crate::scale::tpx(11.)).text_color(c("waiting")).border_1().border_color(c("waiting")).child(trs("waiting")))));
    let d = div().flex().flex_col().gap_1().pl_3().pr_3().py_2().border_l_2().border_color(edge).child(head).child(div().text_color(c("text")).child(body)).children(files_el);
    let d = if synth { d.mt_2().mb_2().border_1().border_l_4().bg(c("raise")) } else { d };
    d.when(!held.is_empty(), |d| d.opacity(0.5)).into_any_element()
}

/// The swarm page: what a room page is when the hub sends `swarm` with it.
pub fn page(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, bot: &Value) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let sw = &bot["swarm"]; let room = &sw["room"];
    let ended = sw["ended"] == true;
    let members: Vec<&Value> = arr(sw, "members").collect();
    let who: HashMap<String, Who> = members.iter().map(|m| (string(m, "thread"), Who { color: hex(&string(m, "color")) })).collect();
    let names: HashMap<String, String> = members.iter().map(|m| (string(m, "member"), string(m, "name"))).collect();
    let claims: HashMap<String, String> = arr(sw, "claims").map(|c| (string(c, "post"), string(c, "state"))).collect();
    // the head: the name, then the controls the hub offers and Add member
    let mut head = div().h(crate::scale::tpx(36.)).flex_shrink_0().px_3().flex().items_center().gap_2().border_b_1().border_color(c("line"))
        .child(div().font_weight(FontWeight::BOLD).child(bot["title"].as_str().map(str::to_string).unwrap_or_else(|| string(bot, "name"))))
        .children(super::badge::place(light, &bot["mark"], "", "room-head")).child(div().flex_1());
    for (i, a) in arr(sw, "actions").enumerate() { head = head.child(button(cx, light, format!("swarm-act-{i}"), trs(&string(a, "label")), false, "swarm-act", string(a, "value"))); }
    let add = &sw["add"];
    if add.is_object() && add["open"] != true { head = head.child(button(cx, light, "swarm-add".into(), trs("Add member"), false, "swarm-add-open", String::new())); }
    // the goal and the state
    let state = string(room, "state"); let reason = string(room, "reason");
    let state_word = if reason.is_empty() { trs(&state) } else { tf("gpui.swarm.state.why", &[("state", trs(&state).into()), ("why", trs(&reason).into())]) };
    let goal = string(room, "goal");
    let line = div().flex().items_center().gap_2().px_3().py_1().text_size(crate::scale::tpx(12.)).border_b_1().border_color(c("line"))
        .child(dot(light, &state)).child(div().text_color(c("dim")).child(state_word))
        .children((!goal.is_empty()).then(|| div().min_w_0().text_color(c("text")).child(goal)));
    // the members: colour, name, live state; a click opens the thread, × removes it
    let chips = div().flex().flex_wrap().gap_2().px_3().py_2().border_b_1().border_color(c("line")).children(members.iter().enumerate().map(|(i, m)| {
        let (thread, state) = (string(m, "thread"), string(m, "state"));
        let col = hex(&string(m, "color")).unwrap_or(c("rule"));
        let gone = state == "removed";
        let tip = { let t = string(m, "title"); let pm = [string(m, "provider"), string(m, "model")].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" / "); [t, pm, trs(&state)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ") };
        let mut chip = div().id(SharedString::from(format!("swarm-member-{i}"))).flex().items_center().gap_2().pl_2().pr_1().py_1().border_1().border_color(c("line")).border_l_4().border_color(col).bg(c("raise")).text_size(crate::scale::tpx(12.))
            .when(gone, |d| d.opacity(0.5)).tooltip(crate::keys::tip(&tip, "", light))
            .child(dot(light, &state)).child(div().text_color(col).font_weight(FontWeight::BOLD).child(string(m, "name")));
        let role = string(m, "role"); if !role.is_empty() && role != string(m, "name") { chip = chip.child(div().text_color(c("faint")).child(role)); }
        if !thread.is_empty() && !gone {
            let t = thread.clone();
            chip = chip.cursor_pointer().hover(|s| s.bg(c("panel"))).on_click(cx.listener(move |this, _, window, cx| this.select(t.clone(), window, cx)));
            if !ended { let v = format!("remove|{thread}");
                chip = chip.child(div().id(SharedString::from(format!("swarm-rm-{i}"))).px_1().text_color(c("faint")).cursor_pointer().hover(|s| s.text_color(c("bad"))).tooltip(crate::keys::tip("Remove from the swarm", "", light)).child("×")
                    .on_click(cx.listener(move |this, _, _, cx| { cx.stop_propagation(); send(this, "swarm-act", &v, cx) }))); }
        } else { chip = chip.pr_2(); }
        chip.into_any_element()
    }).collect::<Vec<_>>());
    // the add picker: an agent, its model and a role
    let picker = (add["open"] == true).then(|| {
        let a1 = add.clone();
        div().flex().flex_col().gap_2().px_3().py_2().border_b_1().border_color(c("line")).bg(c("panel"))
            .child(picks(cx, light, "swarm-add", add, "swarm-add-provider", "swarm-add-model", ""))
            .child(div().flex().gap_2().items_center().children(field_box(light, fields, "sarole", 0.))
                .child(div().id("swarm-add-cancel").px_2().py_1().text_size(crate::scale::tpx(12.)).cursor_pointer().border_1().bg(c("raise")).border_color(c("line")).child(trs("Cancel")).on_click(cx.listener(|this, _, _, cx| send(this, "form-close", "@sadd", cx))))
                .child(div().id("swarm-add-go").px_2().py_1().text_size(crate::scale::tpx(12.)).cursor_pointer().border_1().bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")).child(trs("Add")).on_click(cx.listener(move |this, _, _, cx| {
                    let role = this.screens.fields.get("sarole").map(|s| s.input.read(cx).content.to_string()).unwrap_or_default();
                    send(this, "swarm-act", &add_value(&a1, &role), cx);
                }))))
    });
    // the board
    let posts: Vec<&Value> = arr(sw, "posts").collect();
    // the newest post in view: on opening, and as posts come while the board is read at its end (as subview.rs follows its steps)
    let h = BOARD.with(|b| { let mut b = b.borrow_mut(); let (n, key) = (posts.len(), string(room, "id"));
        let at_end = { let (o, m) = (b.0.offset().y, b.0.max_offset().height); -o >= m - px(40.) };
        if b.1 != key || (b.2 != n && at_end) { b.0.scroll_to_bottom(); } b.1 = key; b.2 = n; b.0.clone() });
    let mut list = div().id("swarm-board").flex_1().min_h_0().overflow_y_scroll().track_scroll(&h).flex().flex_col().gap_1().py_2();
    if posts.is_empty() { list = list.child(div().px_3().text_color(c("dim")).child(trs("No posts yet."))); }
    for p in posts { list = list.child(post_el(light, p, &who, &names, &claims)); }
    let foot = if ended { div().px_3().py_2().border_t_1().border_color(c("line")).text_size(crate::scale::tpx(12.)).text_color(c("dim")).child(trs("This swarm is over.")).into_any_element() } else {
        div().flex().gap_2().items_end().p_3().border_t_1().border_color(c("line"))
            .child(div().flex_1().px_2().py_1().border_1().border_color(c("line")).bg(c("view")).children(fields.get("rpost").map(|s| s.input.clone())))
            .child(div().id("talk-send").px_2().py_1().text_size(crate::scale::tpx(12.)).cursor_pointer().border_1().bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")).child(trs("Send")).on_click(cx.listener(|this, _, _, cx| super::bots::send_post(this, "rpost", "room-post", cx)))).into_any_element()
    };
    div().id("room-page").absolute().inset_0().occlude().bg(c("bg")).flex().flex_col().child(head).child(line).child(chips).children(picker).child(list).child(foot).into_any_element()
}

// ---- New swarm ----

/// The New swarm form over the main area: topic, goal, seed; two to eight members, each an agent, a model and a role; Cancel and Start.
pub fn form(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, data: &Value) -> Option<AnyElement> {
    let ns = &data["newSwarm"]; if !ns.is_object() { return None }
    let c = |n: &str| theme::color(light, n);
    let can_rm = ns["canRemove"] == true;
    let rows: Vec<AnyElement> = arr(ns, "members").enumerate().map(|(i, m)| {
        div().flex().gap_3().py_2().border_t_1().border_color(c("line"))
            .child(div().w(px(18.)).text_color(c("faint")).child(format!("{}", i + 1)))
            .child(div().flex_1().min_w_0().flex().flex_col().gap_1().child(picks(cx, light, &format!("srow-{i}"), m, "swarm-row-provider", "swarm-row-model", &format!("{i}{SEP}")))
                .children(field_box(light, fields, &format!("srole.{i}"), 0.)))
            .children(can_rm.then(|| button(cx, light, format!("srow-del-{i}"), "×".into(), false, "swarm-row-del", i.to_string())))
            .into_any_element()
    }).collect();
    let add = if ns["canAdd"] == true { button(cx, light, "srow-add".into(), trs("Add member"), false, "swarm-row-add", String::new()) } else { inert(light, trs("Add member")) };
    let go = if ns["ok"] == true { button(cx, light, "form-go".into(), trs("Start"), true, "swarm-start", String::new()) } else { inert(light, trs("Start")) };
    Some(div().id("swarm-form").absolute().inset_0().occlude().bg(c("bg")).flex().flex_col().p_4().gap_2()
        .child(div().flex().items_baseline().gap_2().child(div().font_weight(FontWeight::BOLD).text_size(crate::scale::tpx(15.)).child(trs("New swarm"))).child(div().text_size(crate::scale::tpx(12.)).text_color(c("dim")).child(string(ns, "title"))))
        .children(field_box(light, fields, "stopic", 0.)).children(field_box(light, fields, "sgoal", 0.)).children(field_box(light, fields, "sseed", 48.))
        .child(div().id("swarm-form-rows").min_h_0().overflow_y_scroll().flex().flex_col().max_w(px(760.)).children(rows).child(div().pt_2().flex().child(add)))
        .child(div().flex().gap_2().child(button(cx, light, "form-cancel".into(), trs("Cancel"), false, "form-close", "@snew".into())).child(go))
        .into_any_element())
}

/// The registered controls of the swarm page and form (the e2e harness's `click`), named as their elements are.
pub fn controls(data: &Value) -> Vec<(String, String, Ctl)> {
    let a = |a: &str, v: &str| Ctl::Bot("act".into(), format!("{a}{SEP}{v}"));
    let mut out = vec![];
    let ns = &data["newSwarm"];
    if ns.is_object() {
        /* i18n-ok: e2e control labels */ out.push(("form-go".into(), "Start".into(), a("swarm-start", ""))); out.push(("form-cancel".into(), "Cancel".into(), a("form-close", "@snew"))); out.push(("srow-add".into(), "Add member".into(), a("swarm-row-add", "")));
        for (i, m) in arr(ns, "members").enumerate() { for (j, p) in arr(m, "providers").enumerate() { out.push((format!("srow-{i}-p{j}"), string(p, "label"), a("swarm-row-provider", &format!("{i}{SEP}{}", string(p, "value"))))); } }
    }
    let sw = &data["bot"]["swarm"];
    if sw.is_object() {
        for (i, x) in arr(sw, "actions").enumerate() { out.push((format!("swarm-act-{i}"), string(x, "label"), a("swarm-act", &string(x, "value")))); }
        /* i18n-ok: e2e control labels */ if sw["add"].is_object() { out.push(("swarm-add".into(), "Add member".into(), a("swarm-add-open", ""))); }
        for (i, m) in arr(sw, "members").enumerate() { let t = string(m, "thread"); if !t.is_empty() { out.push((format!("swarm-member-{i}"), string(m, "name"), Ctl::Select(t.clone()))); out.push((format!("swarm-rm-{i}"), string(m, "name"), a("swarm-act", &format!("remove|{t}")))); } }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*; use core::prelude::v1::test;
    use serde_json::json;
    #[test] fn a_rows_nav_opens_its_room_and_nothing_else() {
        assert_eq!(nav_action("@room:r7"), Some(("room".into(), "r7".into())));
        assert!(over_thread(&json!({"bot": {"kind": "room", "swarm": {}}, "thread": {"id": "h|t1"}}))); assert!(!over_thread(&json!({"bot": {"kind": "bot"}, "thread": {"id": "h|t1"}}))); assert!(!over_thread(&json!({"bot": null})));
        for n in ["", "@room:", "h|t1", "@remote:b"] { assert_eq!(nav_action(n), None, "{n}"); }
    }
    #[test] fn colours_and_dots_follow_the_hubs_words() {
        assert_eq!(hex("#ff8000"), Some(rgb(0xff8000))); assert_eq!(hex("ff8000"), None); assert_eq!(hex("#fff"), None);
        assert_eq!(dot_of("working").token, "accent"); assert_eq!(dot_of("waiting").token, "waiting"); assert_eq!(dot_of("failed").token, "bad"); assert!(dot_of("failed").hollow);
        assert_eq!(dot_of("idle").token, "faint"); assert_eq!(dot_of("paused").token, "warn"); assert_eq!(dot_of("active").token, "accent");
    }
    #[test] fn mentions_are_the_named_members_whole_names_in_any_case() {
        let names: HashMap<String, String> = [("thread:1", "opus"), ("thread:2", "codex"), ("thread:3", "codex-2")].into_iter().map(|(a, b)| (a.into(), b.into())).collect();
        let t = "@Opus look; cc @codex-2. mail x@opus.com, @gemini";
        let r = mention_ranges(t, &["thread:1".into(), "thread:3".into()], &names);
        assert_eq!(r.iter().map(|r| &t[r.clone()]).collect::<Vec<_>>(), ["@Opus", "@codex-2"]);
        assert!(mention_ranges(t, &[], &names).is_empty(), "the hub names who is mentioned");
        assert_eq!(mention_ranges("@codex-", &["thread:2".into()], &names), [0..6]);
    }
    #[test] fn the_add_value_is_what_the_presenter_maps() {
        assert_eq!(add_value(&json!({"provider": "codex", "model": "gpt-6"}), " critic "), "add|codex|gpt-6|critic");
        assert_eq!(add_value(&json!({"provider": "claude", "model": ""}), ""), "add|claude||");
    }
    #[test] fn the_form_and_page_ask_for_their_fields_and_escape_shuts_them() {
        let d = json!({"newSwarm": {"topic": "t", "members": [{"role": "critic"}, {"role": ""}]}});
        let f = form_fields(&d); assert_eq!(f.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["stopic", "sgoal", "sseed", "srole.0", "srole.1"]); assert_eq!(f[3].value, "critic");
        assert_eq!(focus_target(&d).as_deref(), Some("stopic"));
        assert!(matches!(close_form(&d), Some(Command::Action(a, v)) if a == "form-close" && v == "@snew"));
        let p = json!({"bot": {"kind": "room", "swarm": {"add": {"open": true}}}});
        let f = form_fields(&p); assert!(f.len() == 1 && f[0].name == "sarole" && f[0].vault, "the role is held here until Add");
        assert!(matches!(close_form(&p), Some(Command::Action(_, v)) if v == "@sadd")); assert!(close_form(&json!({"bot": {"swarm": {}}})).is_none());
    }
    #[test] fn the_page_and_form_register_their_controls() {
        let d = json!({"bot": {"kind": "room", "swarm": {"actions": [{"label": "Pause", "value": "pause"}], "add": {"open": false}, "members": [{"thread": "t1", "name": "opus"}, {"thread": "", "name": "cat"}]}},
            "newSwarm": {"members": [{"providers": [{"label": "codex", "value": "codex"}]}]}});
        let c = controls(&d); let ids: Vec<&str> = c.iter().map(|x| x.0.as_str()).collect();
        for want in ["form-go", "srow-add", "srow-0-p0", "swarm-act-0", "swarm-add", "swarm-member-0", "swarm-rm-0"] { assert!(ids.contains(&want), "{want}: {ids:?}"); }
        assert!(!ids.contains(&"swarm-member-1"), "a bot member has no thread to open");
        assert!(c.iter().any(|x| x.0 == "swarm-rm-0" && matches!(&x.2, Ctl::Bot(k, v) if k == "act" && v == "swarm-act\u{1f}remove|t1")));
        assert!(c.iter().any(|x| x.0 == "srow-0-p0" && matches!(&x.2, Ctl::Bot(_, v) if v == "swarm-row-provider\u{1f}0\u{1f}codex")));
    }
}
