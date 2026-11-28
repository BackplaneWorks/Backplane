//! Swarms (docs/SWARM.md) from the hub's screen model: a project's swarm rows (`projects[].swarms`), the swarm page (a room page with
//! `bot.swarm`: member chips, the goal and state, the controls, the add picker, the board as one feed, the post box) and the New swarm form
//! (`newSwarm`). Every press is a view action the hub's presenter maps (present/swarmform.rs, `swarm-act`); nothing here decides what a swarm does.
use super::{md::{self, Block, Inline, Span}, settings::{flush_fields, Field, FieldSlot}};
use crate::{art, hub::Command, i18n::{tf, trs}, script::{Ctl, CtlEntry}, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::{collections::HashMap, rc::Rc, sync::atomic::{AtomicBool, Ordering::Relaxed}};

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

/// Whether a swarm row should show under a project row in the sidebar:
/// settled or finished swarms never show; under a collapsed project (`folded`),
/// only actively working/monitoring (unpaused) or attention-needing swarms show.
pub fn visible_under_project(r: &Value, folded: bool) -> bool {
    if r["settled"] == true { return false }
    let state = string(r, "state");
    if matches!(state.as_str(), "done" | "stopped") { return false }
    if !folded { return true }
    let working = r["working"].as_u64().unwrap_or(0);
    let monitoring = r["monitoring"] == true;
    let needs = r["need"].is_object() || matches!(r["word"].as_str(), Some("needs"));
    ((working > 0 || monitoring) && state != "paused") || needs
}

/// Context-menu items for a swarm row (hub-provided `menu` when present, else Pause/Resume + Stop swarm).
pub fn menu_items(r: &Value) -> Vec<crate::screens::ctxmenu::Item> {
    let from_hub = crate::screens::ctxmenu::section_items(&r["menu"]);
    if !from_hub.is_empty() { return from_hub }
    let room = nav_action(&string(r, "nav")).map(|(_, id)| id).unwrap_or_default();
    let state = string(r, "state");
    if room.is_empty() || matches!(state.as_str(), "done" | "stopped") { return vec![] }
    let item = |label: &str, action: &str, value: String, confirm: bool| crate::screens::ctxmenu::Item {
        label: label.into(), action: action.into(), value, confirm, note: String::new(),
    };
    let mut out = vec![];
    if state == "paused" { out.push(item("Resume", "swarm-need", format!("{room}\u{1f}resume\u{1f}"), false)); } // i18n-ok: menu label, drawn through trs in render
    else if state == "active" { out.push(item("Pause", "swarm-need", format!("{room}\u{1f}pause\u{1f}"), false)); } // i18n-ok: menu label, drawn through trs in render
    out.push(item("Stop swarm", "swarm-need", format!("{room}\u{1f}stop\u{1f}"), true)); // i18n-ok: menu label, drawn through trs in render
    out
}

/// A project's swarm rows, above its threads: the name, the state's dot, "N members · M working" and the ago; a click opens its page.
pub fn side_rows(cx: &mut Context<Backplane>, light: bool, project: &Value, open_room: &str, ctl: &mut Vec<CtlEntry>) -> Vec<AnyElement> {
    render_side_rows(cx, light, arr(project, "swarms"), open_room, ctl)
}

/// A project's swarm rows under its sidebar entry, applying collapsed-project visibility rules.
pub fn project_side_rows(cx: &mut Context<Backplane>, light: bool, project: &Value, open_room: &str, ctl: &mut Vec<CtlEntry>) -> Vec<AnyElement> {
    let folded = project["folded"] == true;
    render_side_rows(cx, light, arr(project, "swarms").filter(|r| visible_under_project(r, folded)), open_room, ctl)
}

fn render_side_rows<'a>(cx: &mut Context<Backplane>, light: bool, rows: impl Iterator<Item = &'a Value>, open_room: &str, ctl: &mut Vec<CtlEntry>) -> Vec<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    rows.filter_map(|r| {
        let (action, room) = nav_action(&string(r, "nav"))?;
        let name = { let n = string(r, "name"); if n.is_empty() { string(r, "title") } else { n } };
        ctl.push(CtlEntry { id: format!("swarm:{name}"), label: name.clone(), ctl: Ctl::Node(action.clone(), room.clone()) });
        let sel = room == open_room;
        // what it wants of the person, in the hub's one line (present/swarm_need.rs), with the buttons that resolve it; else its members at work
        let need = &r["need"];
        let line = if need.is_object() { string(need, "line") } else { tf("gpui.swarm.row", &[("n", r["members"].as_u64().unwrap_or(0).into()), ("w", r["working"].as_u64().unwrap_or(0).into())]) };
        let rid = string(r, "id");
        let btns: Vec<AnyElement> = need_buttons(r).into_iter().enumerate().map(|(i, (label, v))| {
            ctl.push(CtlEntry { id: format!("swarm-need:{name}:{i}"), label: label.clone(), ctl: Ctl::Node("swarm-need".into(), v.clone()) });
            need_button(cx, light, format!("swarm-need-{rid}-{i}"), trs(&label), i == 0, v) }).collect();
        let has = !btns.is_empty();
        let settle_val = string(r, "settle");
        let items = menu_items(r);
        let group = SharedString::from(format!("swarm-row-{rid}"));
        let settle_btn = (!has && !settle_val.is_empty()).then(|| {
            let raw = settle_val.rsplit('|').next().unwrap_or("").to_string();
            /* i18n-ok: a ctl label */ ctl.push(CtlEntry { id: format!("swarm-settle:{raw}"), label: "Settle".into(), ctl: Ctl::Node("swarm-settle".into(), settle_val.clone()) });
            let sv = settle_val.clone();
            crate::ui::with_rect(div().id(SharedString::from(format!("swarm-settle-{rid}"))), move || format!("swarm-settle:{raw}"))
                .flex_shrink_0().w(px(16.)).h(px(16.)).flex().items_center().justify_center().cursor_pointer()
                .invisible().group_hover(group.clone(), |s| s.visible()).hover(|s| s.bg(c("line")))
                .tooltip(crate::keys::tip(&trs("Settle"), "", light))
                .child(crate::art::icon("check", 12., c("dim")))
                .on_click(cx.listener(move |this, _, _, cx| { cx.stop_propagation(); this.go("swarm-settle", sv.clone(), cx); }))
                .into_any_element()
        });
        let el = crate::ui::with_rect(div().id(SharedString::from(format!("swarm-{rid}"))), { let n = name.clone(); move || format!("swarm:{n}") })
            .group(group).flex().items_center().gap_2().pl(px(30.)).pr_3().when(!has, |d| d.h(crate::scale::tpx(38.))).when(has, |d| d.py_1()).border_l_2().border_color(if sel { c("accent") } else { c("panel") })
            .bg(if sel { c("raise") } else { c("panel") }).hover(|s| s.bg(c("raise"))).cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| this.go(&action, room.clone(), cx)))
            .when(!items.is_empty(), |d| d.on_mouse_down(MouseButton::Right, cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                this.ctx_open(e.position, items.clone(), cx);
            })))
            .child(dot(light, &string(r, "state")))
            .child(div().flex_1().min_w_0().flex().flex_col().line_height(crate::scale::tpx(16.))
                .child(div().overflow_hidden().whitespace_nowrap().text_ellipsis().child(name))
                .child(div().text_size(crate::scale::tpx(11.)).text_color(if has { c("warn") } else { c("faint") }).overflow_hidden().whitespace_nowrap().text_ellipsis().child(line))
                .when(has, |d| d.child(div().pt_1().flex().gap_1().children(btns))))
            .children(settle_btn)
            .when(!has, |d| d.child(div().text_size(crate::scale::tpx(11.)).text_color(c("faint")).child(string(r, "ago"))));
        Some(el.into_any_element())
    }).collect()
}
/// A swarm row's buttons (its `need.actions`): each one's label and the `swarm-need` value the hub's presenter maps ("<room>\u{1f}<act>\u{1f}<thread>").
pub fn need_buttons(r: &Value) -> Vec<(String, String)> {
    arr(&r["need"], "actions").filter(|a| !string(a, "act").is_empty()).map(|a| (string(a, "label"), format!("{}{SEP}{}{SEP}{}", string(a, "room"), string(a, "act"), string(a, "thread")))).collect()
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
/// A Needs you row's button: as `button`, smaller, and its press is not the row's (which opens the swarm's page).
fn need_button(cx: &mut Context<Backplane>, light: bool, id: String, label: String, on: bool, v: String) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let d = div().id(SharedString::from(id)).px_2().text_size(crate::scale::tpx(11.)).cursor_pointer().border_1().whitespace_nowrap();
    let d = if on { d.bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")) } else { d.bg(c("raise")).border_color(c("line")).hover(|s| s.bg(c("panel"))) };
    d.child(label).on_click(cx.listener(move |this, _, _, cx| { cx.stop_propagation(); send(this, "swarm-need", &v, cx) })).into_any_element()
}
fn inert(light: bool, label: String) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    div().px_2().py_1().text_size(crate::scale::tpx(12.)).border_1().border_color(c("line")).bg(c("raise")).text_color(c("faint")).opacity(0.6).child(label).into_any_element()
}
fn field_box(light: bool, fields: &HashMap<String, FieldSlot>, name: &str, min_h: f32) -> Option<AnyElement> { field_box_w(light, fields, name, min_h, Some(560.)) }
/// `field_box` that spans its parent (`max` None): the New swarm form's boxes all end on the column's right edge.
fn field_box_w(light: bool, fields: &HashMap<String, FieldSlot>, name: &str, min_h: f32, max: Option<f32>) -> Option<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    fields.get(name).map(|s| div().w_full().when_some(max, |d, m| d.max_w(px(m))).min_h(px(min_h)).px_2().py_1().border_1().border_color(c("line")).bg(c("view")).child(s.input.clone()).into_any_element())
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

fn mentions_fingerprint(mentions: &[String], names: &HashMap<String, String>) -> u64 {
    if mentions.is_empty() { return 0; }
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    mentions.len().hash(&mut h);
    for m in mentions { m.hash(&mut h); names.get(m).hash(&mut h); }
    h.finish()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostSig {
    pub id: String,
    pub hash: u64,
    pub held: String,
    pub tags_len: usize,
    pub blocks_len: usize,
    pub imgs_len: usize,
}

pub fn post_sigs(posts: &[Value], claims: &HashMap<String, String>) -> Vec<PostSig> {
    use std::hash::{Hash, Hasher};
    posts.iter().map(|p| {
        let id = string(p, "id");
        let mut h = std::collections::hash_map::DefaultHasher::new();
        p["text"].as_str().unwrap_or("").hash(&mut h);
        for f in arr(p, "files").filter_map(|v| v.as_str()) {
            f.hash(&mut h);
        }
        claims.get(&id).map(String::as_str).unwrap_or("").hash(&mut h);
        PostSig {
            id,
            hash: h.finish(),
            held: string(p, "held"),
            tags_len: p["tags"].as_array().map_or(0, Vec::len),
            blocks_len: p["blocks"].as_array().map_or(0, Vec::len),
            imgs_len: p["images"].as_array().map_or(0, Vec::len),
        }
    }).collect()
}

pub fn diff_range(old: &[PostSig], new: &[PostSig]) -> Option<(std::ops::Range<usize>, usize)> {
    if old == new { return None; }
    let mut p = 0;
    while p < old.len() && p < new.len() && old[p] == new[p] { p += 1; }
    let mut s = 0;
    while s < old.len() - p && s < new.len() - p && old[old.len() - 1 - s] == new[new.len() - 1 - s] { s += 1; }
    Some((p..old.len() - s, new.len() - p - s))
}

fn highlight_spans(spans: &[Span], mentions: &[String], names: &HashMap<String, String>) -> Vec<Span> {
    let mut out = Vec::with_capacity(spans.len());
    for sp in spans {
        if sp.style.code || sp.style.kbd || sp.style.link.is_some() || sp.text.is_empty() {
            out.push(sp.clone());
            continue;
        }
        let ranges = mention_ranges(&sp.text, mentions, names);
        if ranges.is_empty() {
            out.push(sp.clone());
            continue;
        }
        let mut at = 0usize;
        for r in ranges {
            if r.start > at {
                out.push(Span { text: sp.text[at..r.start].to_string(), style: sp.style.clone() });
            }
            let mut st = sp.style.clone();
            st.bold = true;
            out.push(Span { text: sp.text[r.clone()].to_string(), style: st });
            at = r.end;
        }
        if at < sp.text.len() {
            out.push(Span { text: sp.text[at..].to_string(), style: sp.style.clone() });
        }
    }
    out
}

pub fn highlight_mentions(blocks: &[Block], mentions: &[String], names: &HashMap<String, String>) -> Vec<Block> {
    if mentions.is_empty() || names.is_empty() { return blocks.to_vec(); }
    blocks.iter().map(|b| match b {
        Block::Para(s) => Block::Para(highlight_spans(s, mentions, names)),
        Block::Heading(l, s) => Block::Heading(*l, highlight_spans(s, mentions, names)),
        Block::Quote(s) => Block::Quote(highlight_spans(s, mentions, names)),
        Block::List { ordered, start, items } => Block::List {
            ordered: *ordered,
            start: *start,
            items: items.iter().map(|it| md::Item {
                spans: highlight_spans(&it.spans, mentions, names),
                sub: highlight_mentions(&it.sub, mentions, names),
                checked: it.checked,
            }).collect(),
        },
        Block::Table { head, rows } => Block::Table {
            head: head.iter().map(|c| highlight_spans(c, mentions, names)).collect(),
            rows: rows.iter().map(|r| r.iter().map(|c| highlight_spans(c, mentions, names)).collect()).collect(),
        },
        Block::Flow(parts, l) => Block::Flow(parts.iter().map(|p| match p {
            Inline::Text(s) => Inline::Text(highlight_spans(s, mentions, names)),
            other => other.clone(),
        }).collect(), *l),
        Block::Aligned(a, bs) => Block::Aligned(*a, highlight_mentions(bs, mentions, names)),
        Block::Callout(k, bs) => Block::Callout(k.clone(), highlight_mentions(bs, mentions, names)),
        Block::Details { summary, open, blocks } => Block::Details {
            summary: highlight_spans(summary, mentions, names),
            open: *open,
            blocks: highlight_mentions(blocks, mentions, names),
        },
        other => other.clone(),
    }).collect()
}

pub struct BoardMeta {
    pub who: HashMap<String, Option<Rgba>>,
    pub names: HashMap<String, String>,
    pub claims: HashMap<String, String>,
}

static SWARM_FAR: AtomicBool = AtomicBool::new(false);
fn note_swarm_far(list: &ListState) -> bool {
    let (max, y) = (f32::from(list.max_offset_for_scrollbar().height), -f32::from(list.scroll_px_offset_for_scrollbar().y));
    let far = super::glue::far_from(max, y);
    SWARM_FAR.swap(far, Relaxed) != far
}

impl Backplane {
    pub(crate) fn swarm_part(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let light = self.light();
        let c = |n: &str| theme::color(light, n);
        let reg = self.screens.reg.clone();
        {
            let mut r = reg.borrow_mut();
            r.begin_rows();
            r.app = Some(cx.entity().downgrade());
        }
        super::xprobe::want_names(self);
        reg.borrow_mut().design = self.screens.xprobe.design.clone();

        let bot = &self.data["bot"];
        let sw = &bot["swarm"];
        let room = &sw["room"];
        let room_id = string(room, "id");
        let ended = sw["ended"] == true;
        let members: Vec<&Value> = arr(sw, "members").collect();
        let meta = Rc::new(BoardMeta {
            who: members.iter().map(|m| (string(m, "thread"), hex(&string(m, "color")))).collect(),
            names: members.iter().map(|m| (string(m, "member"), string(m, "name"))).collect(),
            claims: arr(sw, "claims").map(|cl| (string(cl, "post"), string(cl, "state"))).collect(),
        });

        let posts_slice = sw["posts"].as_array().map_or(&[][..], |a| a.as_slice());
        let sigs = post_sigs(posts_slice, &meta.claims);
        if self.screens.swarm_list_key != room_id {
            self.screens.swarm_list.reset(posts_slice.len());
            self.screens.swarm_list_key = room_id.clone();
            SWARM_FAR.store(false, Relaxed);
        } else if let Some((range, count)) = diff_range(&self.screens.swarm_post_sigs, &sigs) {
            super::glue::splice_keeping(&self.screens.swarm_list, range, count);
        }
        self.screens.swarm_post_sigs = sigs;

        // the head: the name, then the controls the hub offers and Add member
        let mut head = div().h(crate::scale::tpx(36.)).flex_shrink_0().px_3().flex().items_center().gap_2().border_b_1().border_color(c("line"))
            .child(div().font_weight(FontWeight::BOLD).child(bot["title"].as_str().map(str::to_string).unwrap_or_else(|| string(bot, "name"))))
            .children(super::badge::place(light, &bot["mark"], "", "room-head")).child(div().flex_1());
        for (i, a) in arr(sw, "actions").enumerate() {
            head = head.child(button(cx, light, format!("swarm-act-{i}"), trs(&string(a, "label")), false, "swarm-act", string(a, "value")));
        }
        let add = &sw["add"];
        if add.is_object() && add["open"] != true {
            head = head.child(button(cx, light, "swarm-add".into(), trs("Add member"), false, "swarm-add-open", String::new()));
        }

        // the goal and the state
        let state = string(room, "state");
        let reason = string(room, "reason");
        let state_word = if reason.is_empty() { trs(&state) } else { tf("gpui.swarm.state.why", &[("state", trs(&state).into()), ("why", trs(&reason).into())]) };
        let goal = string(room, "goal");
        let line = div().flex().items_center().gap_2().px_3().py_1().text_size(crate::scale::tpx(12.)).border_b_1().border_color(c("line"))
            .child(dot(light, &state)).child(div().flex_shrink_0().text_color(c("dim")).child(state_word))
            .children((!goal.is_empty()).then(|| div().flex_1().min_w_0().text_color(c("text")).child(goal)));

        // the members: crisp 3px colored bar inside a clean line-bordered chip; a click opens the thread, × removes it
        let chips = div().flex().flex_wrap().gap_2().px_3().py_2().border_b_1().border_color(c("line")).children(members.iter().enumerate().map(|(i, m)| {
            let (thread, mstate) = (string(m, "thread"), string(m, "state"));
            let col = hex(&string(m, "color")).unwrap_or(c("rule"));
            let gone = mstate == "removed";
            let tip = {
                let t = string(m, "title");
                let pm = [string(m, "provider"), string(m, "model")].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" / ");
                [t, pm, trs(&mstate)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")
            };
            let mut chip = div().id(SharedString::from(format!("swarm-member-{i}"))).flex().gap_2().pr_1().border_1().border_color(c("line")).bg(c("raise")).text_size(crate::scale::tpx(12.))
                .when(gone, |d| d.opacity(0.5)).tooltip(crate::keys::tip(&tip, "", light))
                .child(div().w(px(3.)).bg(col))
                .child(div().flex().items_center().gap_2().py_1().child(dot(light, &mstate)).child(div().text_color(col).font_weight(FontWeight::BOLD).child(string(m, "name"))));
            let role = string(m, "role");
            if !role.is_empty() && role != string(m, "name") { chip = chip.child(div().text_color(c("faint")).child(role)); }
            if !thread.is_empty() && !gone {
                let t = thread.clone();
                chip = chip.cursor_pointer().hover(|s| s.bg(c("panel"))).on_click(cx.listener(move |this, _, window, cx| this.select(t.clone(), window, cx)));
                if !ended {
                    let v = format!("remove|{thread}");
                    chip = chip.child(div().id(SharedString::from(format!("swarm-rm-{i}"))).px_1().text_color(c("faint")).cursor_pointer().hover(|s| s.text_color(c("bad"))).tooltip(crate::keys::tip("Remove from the swarm", "", light)).child("×")
                        .on_click(cx.listener(move |this, _, _, cx| { cx.stop_propagation(); send(this, "swarm-act", &v, cx) })));
                } else { chip = chip.pr_2(); }
            } else { chip = chip.pr_2(); }
            chip.into_any_element()
        }).collect::<Vec<_>>());

        // the add picker: an agent, its model and a role
        let picker = (add["open"] == true).then(|| {
            let a1 = add.clone();
            div().flex().flex_col().gap_2().px_3().py_2().border_b_1().border_color(c("line")).bg(c("panel"))
                .child(picks(cx, light, "swarm-add", add, "swarm-add-provider", "swarm-add-model", ""))
                .child(div().flex().gap_2().items_center().children(field_box(light, &self.screens.fields, "sarole", 0.))
                    .child(div().id("swarm-add-cancel").px_2().py_1().text_size(crate::scale::tpx(12.)).cursor_pointer().border_1().bg(c("raise")).border_color(c("line")).child(trs("Cancel")).on_click(cx.listener(|this, _, _, cx| send(this, "form-close", "@sadd", cx))))
                    .child(div().id("swarm-add-go").px_2().py_1().text_size(crate::scale::tpx(12.)).cursor_pointer().border_1().bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")).child(trs("Add")).on_click(cx.listener(move |this, _, _, cx| {
                        let role = this.screens.fields.get("sarole").map(|s| s.input.read(cx).content.to_string()).unwrap_or_default();
                        send(this, "swarm-act", &add_value(&a1, &role), cx);
                    }))))
        });

        let no_posts = posts_slice.is_empty();
        let hover_text = reg.borrow().hover.clone();
        let list_el = if no_posts {
            div().flex_1().flex().justify_center().items_start().child(div().w_full().max_w(px(930.)).px_4().py_4().text_color(c("dim")).child(trs("No posts yet."))).into_any_element()
        } else {
            let meta = meta.clone();
            div().flex_1().min_h_0().relative().flex().flex_col()
                .child(list(self.screens.swarm_list.clone(), cx.processor(move |this, ix, _, cx| this.swarm_row(ix, &meta, cx))).flex_1().min_h_0())
                .child(super::scrollbar::bar("swarm", super::scrollbar::Src::List(self.screens.swarm_list.clone()), light)).into_any_element()
        };

        let board = div().id("swarm-board").flex_1().min_h_0().relative().flex().flex_col()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, e, w, cx| this.timeline_down(e, w, cx)))
            .on_mouse_down(MouseButton::Right, cx.listener(|this, e, w, cx| this.timeline_down(e, w, cx)))
            .on_mouse_move(cx.listener(|this, e, _, cx| this.timeline_move(e, cx)))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, e, w, cx| this.timeline_up(e, w, cx)))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, e, w, cx| this.timeline_up(e, w, cx)))
            .child(list_el)
            .child(canvas({
                let (me, reg) = (cx.entity().downgrade(), reg.clone());
                move |_, _, cx| {
                    let Some(this) = me.upgrade() else { return };
                    let rows = this.read(cx).active_rows();
                    reg.borrow_mut().resolve_by(|id| rows.iter().position(|e| e["id"].as_str() == Some(id)));
                }
            }, |_, _, _, _| {}).absolute().size_0())
            .child(canvas({
                let me = cx.entity().downgrade();
                move |_, _, cx| {
                    let Some(this) = me.upgrade() else { return };
                    if note_swarm_far(&this.read(cx).screens.swarm_list) {
                        cx.defer(move |cx| this.update(cx, |_, cx| cx.notify()));
                    }
                }
            }, |_, _, _, _| {}).absolute().size_0())
            .when(!no_posts && SWARM_FAR.load(Relaxed), |d| {
                d.child(div().absolute().bottom_3().left_0().right_0().flex().justify_center()
                    .child(div().id("swarm-to-latest").flex().items_center().gap_1().px_3().py_1().border_1().border_color(c("line")).bg(c("raise")).cursor_pointer().hover(|s| s.bg(c("panel")))
                        .text_size(crate::scale::tpx(12.)).text_color(c("text")).tooltip(crate::keys::tip("Go to the newest post", "", light))
                        .child(art::icon("chevron-down", 14., c("dim"))).child(trs("Latest"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            let n = this.screens.swarm_list.item_count();
                            this.screens.swarm_list.scroll_to(ListOffset { item_ix: n, offset_in_item: px(0.) });
                            SWARM_FAR.store(false, Relaxed);
                            cx.notify();
                        }))))
            })
            .when(!hover_text.is_empty(), |d| {
                d.child(crate::ui::link_address(light, &hover_text, self.screens.swarm_list.viewport_bounds().size.width))
            });

        if !self.screens.img_poll && self.screens.images.loading() {
            self.screens.img_poll = true;
            super::timeline::poll(window, cx);
        }

        let foot = if ended {
            div().border_t_1().border_color(c("line")).flex().justify_center()
                .child(div().w_full().max_w(px(930.)).px_4().py_2().text_size(crate::scale::tpx(12.)).text_color(c("dim")).child(trs("This swarm is over."))).into_any_element()
        } else {
            div().border_t_1().border_color(c("line")).flex().justify_center()
                .child(div().w_full().max_w(px(930.)).px_4().py_3().flex().gap_2().items_end()
                    .child(div().flex_1().px_2().py_1().border_1().border_color(c("line")).bg(c("view")).children(self.screens.fields.get("rpost").map(|s| s.input.clone())))
                    .child(div().id("talk-send").px_3().py_1().text_size(crate::scale::tpx(12.)).cursor_pointer().border_1().bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")).child(trs("Send"))
                        .on_click(cx.listener(|this, _, _, cx| super::bots::send_post(this, "rpost", "room-post", cx))))).into_any_element()
        };

        div().id("room-page").size_full().occlude().bg(c("bg")).flex().flex_col().child(head).child(line).child(chips).children(picker).child(board).child(foot).into_any_element()
    }

    pub(crate) fn swarm_row(&mut self, ix: usize, meta: &BoardMeta, cx: &mut Context<Self>) -> AnyElement {
        let light = self.light();
        let c = |n: &str| theme::color(light, n);
        let reg = self.screens.reg.clone();
        let Some(p) = self.data["bot"]["swarm"]["posts"].as_array().and_then(|a| a.get(ix)).cloned() else {
            return div().into_any_element();
        };
        let id = string(&p, "id");
        let text = string(&p, "text");
        reg.borrow_mut().enter(ix, &id);

        if p["guard"] == true {
            let body = super::timeline::user_text(&reg, light, &text, &[]);
            return div().w_full().flex().justify_center()
                .child(div().w_full().max_w(px(930.)).px_4().py(px(3.)).text_size(crate::scale::tpx(11.)).text_color(c("faint")).child(body))
                .into_any_element();
        }

        let tags: Vec<String> = arr(&p, "tags").filter_map(|t| t.as_str().map(str::to_string)).collect();
        let synth = tags.iter().any(|t| t == "synthesis");
        let mine = p["mine"] == true;
        let col = if mine { Some(c("accent")) } else { hex(&string(&p, "color")).or_else(|| meta.who.get(&string(&p, "thread")).and_then(|w| *w)) };
        let edge = col.unwrap_or(c("rule"));
        let name = if mine { trs("You") } else { string(&p, "from") };
        let mentions: Vec<String> = arr(&p, "mentions").filter_map(|m| m.as_str().map(str::to_string)).collect();

        let mhash = mentions_fingerprint(&mentions, &meta.names);
        let body_els: Vec<AnyElement> = match p["blocks"].as_array() {
            Some(blocks) if !blocks.is_empty() => {
                let blocks_rc = self.screens.parsed.get_with(&id, &text, blocks, mhash, |bs| {
                    *bs = highlight_mentions(bs, &mentions, &meta.names);
                });
                super::timeline::blocks(cx, &reg, light, &blocks_rc, &id)
            }
            _ => {
                let bs = highlight_mentions(&md::plain_fallback(&text), &mentions, &meta.names);
                super::timeline::blocks(cx, &reg, light, &bs, &id)
            }
        };

        let imgs_el = super::timeline::images(cx, light, &p["images"], &id, self.screens.attacher.target.as_ref(), &self.screens.images);

        let group = SharedString::from(format!("sp-{id}"));
        let cid = SharedString::from(format!("swarm-msg-{id}"));
        let done = super::copyable::just_copied(&cid);
        let copy_txt = text.clone();
        let copy_btn = div().id(SharedString::from(format!("sp-copy-{id}"))).px_1().cursor_pointer()
            .text_size(crate::scale::tpx(11.)).text_color(c(if done { "ok" } else { "faint" })).hover(|s| s.text_color(c("text")))
            .when(!done, |d| d.invisible().group_hover(group.clone(), |s| s.visible()))
            .flex().items_center().gap_1()
            .child(art::icon(if done { "check" } else { "copy" }, 11., c(if done { "ok" } else { "faint" })))
            /* i18n-ok: drawn through trs */ .child(trs(if done { "Copied" } else { "Copy" }))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |_, window, cx| { cx.stop_propagation(); super::copyable::copy(&cid, &copy_txt, window, cx); });

        let mut head = div().flex().items_center().gap_2().flex_wrap()
            .child(div().font_weight(FontWeight::BOLD).text_color(edge).child(name));
        for t in &tags {
            if synth && t == "synthesis" {
                head = head.child(div().px_1().text_size(crate::scale::tpx(10.)).font_weight(FontWeight::BOLD).border_1().border_color(edge).text_color(edge).child(trs(t)));
            } else {
                head = head.child(div().px_1().text_size(crate::scale::tpx(10.)).border_1().border_color(c("line")).text_color(c("dim")).child(trs(t)));
            }
        }
        head = head.child(div().text_size(crate::scale::tpx(11.)).text_color(c("faint")).child(string(&p, "ago")));
        let held = string(&p, "held");
        if !held.is_empty() {
            head = head.child(div().text_size(crate::scale::tpx(10.)).text_color(c("faint")).child(tf("gpui.swarm.held", &[("why", held.clone().into())])));
        }
        head = head.child(div().flex_1()).child(copy_btn);

        let files: Vec<String> = arr(&p, "files").filter_map(|f| f.as_str().map(str::to_string)).collect();
        let claim = meta.claims.get(&id);
        let files_el = (!files.is_empty()).then(|| {
            div().flex().flex_wrap().gap_1().pt_1()
                .children(files.into_iter().map(|f| div().px_1().text_size(crate::scale::tpx(11.)).bg(c("raise")).border_1().border_color(c("line")).text_color(c("dim")).child(f)))
                .children(claim.filter(|s| s.as_str() == "waiting").map(|_| div().px_1().text_size(crate::scale::tpx(11.)).text_color(c("waiting")).border_1().border_color(c("waiting")).child(trs("waiting"))))
        });

        let mut content = div().flex_1().min_w_0().flex().flex_col().gap_2().child(head);
        for el in body_els { content = content.child(el); }
        if let Some(im) = imgs_el { content = content.child(im); }
        if let Some(fe) = files_el { content = content.child(fe); }

        let card = if synth {
            div().my_2().border_1().border_color(c("line")).bg(c("raise")).flex()
                .child(div().w(px(4.)).flex_shrink_0().bg(edge))
                .child(content.p_3())
        } else {
            div().py_2().flex().gap_3()
                .child(div().w(px(3.)).flex_shrink_0().bg(edge))
                .child(content)
        };

        div().id(SharedString::from(format!("spost-{id}"))).group(group).w_full().flex().justify_center()
            .child(div().w_full().max_w(px(930.)).px_4().when(!held.is_empty(), |d| d.opacity(0.5)).child(card))
            .into_any_element()
    }
}

// ---- New swarm ----

/// What Start waits for, from the hub's own enable condition (`ok`: a topic, and two to eight members): None when Start is on, else the first thing
/// missing, in English (drawn through trs).
pub fn start_waits(ns: &Value) -> Option<&'static str> {
    if ns["ok"] == true { return None }
    if string(ns, "topic").trim().is_empty() { return Some("Needs a topic") } // i18n-ok: drawn through trs in form
    Some("Needs two to eight members") // i18n-ok: drawn through trs in form
}

/// The New swarm form over the main area, in the page column (as a project's page): topic, goal, seed; two to eight members, each an agent, a model and a
/// role; Cancel and Start, and beside a grey Start what it waits for. Every box and rule ends on the column's right edge.
pub fn form(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, data: &Value) -> Option<AnyElement> {
    let ns = &data["newSwarm"]; if !ns.is_object() { return None }
    let c = |n: &str| theme::color(light, n);
    let can_rm = ns["canRemove"] == true;
    let rows: Vec<AnyElement> = arr(ns, "members").enumerate().map(|(i, m)| {
        div().flex().gap_3().py_2().border_t_1().border_color(c("line"))
            // the indent: the member's number, and its × under it, so the chips and the Role box end where the boxes above do
            .child(div().w(px(22.)).flex_shrink_0().flex().flex_col().gap_1().child(div().text_color(c("faint")).child(format!("{}", i + 1)))
                .children(can_rm.then(|| button(cx, light, format!("srow-del-{i}"), "×".into(), false, "swarm-row-del", i.to_string()))))
            .child(div().flex_1().min_w_0().flex().flex_col().gap_1().child(picks(cx, light, &format!("srow-{i}"), m, "swarm-row-provider", "swarm-row-model", &format!("{i}{SEP}")))
                .children(field_box_w(light, fields, &format!("srole.{i}"), 0., None)))
            .into_any_element()
    }).collect();
    let add = if ns["canAdd"] == true { button(cx, light, "srow-add".into(), trs("Add member"), false, "swarm-row-add", String::new()) } else { inert(light, trs("Add member")) };
    let go = if ns["ok"] == true { button(cx, light, "form-go".into(), trs("Start"), true, "swarm-start", String::new()) } else { inert(light, trs("Start")) };
    let waits = start_waits(ns).map(|w| div().id("form-go-waits").text_size(crate::scale::tpx(12.)).text_color(c("faint")).whitespace_nowrap().child(trs(w)));
    Some(div().id("swarm-form").absolute().inset_0().occlude().bg(c("bg")).flex().justify_center()
        .child(div().w_full().max_w(px(930.)).min_h_0().px_4().py_4().flex().flex_col().gap_2()
            .child(div().flex().items_baseline().gap_2().child(div().font_weight(FontWeight::BOLD).text_size(crate::scale::tpx(15.)).child(trs("New swarm"))).child(div().text_size(crate::scale::tpx(12.)).text_color(c("dim")).child(string(ns, "title"))))
            .children(field_box_w(light, fields, "stopic", 0., None)).children(field_box_w(light, fields, "sgoal", 0., None)).children(field_box_w(light, fields, "sseed", 48., None))
            .child(div().id("swarm-form-rows").min_h_0().overflow_y_scroll().flex().flex_col().children(rows).child(div().pt_2().flex().child(add)))
            .child(div().flex().items_center().gap_2().child(button(cx, light, "form-cancel".into(), trs("Cancel"), false, "form-close", "@snew".into())).child(go).children(waits)))
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
        let r = serde_json::json!({"need":{"line":"Member gemini failed: Gemini sign-in expired","actions":[{"label":"Sign in to Gemini","act":"signin","room":"h|r1","thread":"h|t2"},{"label":"Move to Claude","act":"move","room":"h|r1","thread":"h|t2"}]}});
        assert_eq!(need_buttons(&r), vec![("Sign in to Gemini".to_string(), "h|r1\u{1f}signin\u{1f}h|t2".to_string()), ("Move to Claude".to_string(), "h|r1\u{1f}move\u{1f}h|t2".to_string())]);
        assert!(need_buttons(&serde_json::json!({"need":null})).is_empty(), "a swarm that needs nothing has no buttons");
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
    #[test] fn swarm_form_says_what_start_waits_for() {
        assert_eq!(start_waits(&json!({"ok": true, "topic": "Motor"})), None);
        assert_eq!(start_waits(&json!({"ok": false, "topic": "  "})), Some("Needs a topic"));
        assert_eq!(start_waits(&json!({"ok": false, "topic": "Motor"})), Some("Needs two to eight members"));
        for w in ["Needs a topic", "Needs two to eight members"] { assert!(crate::i18n::catalog().japanese(w).is_some(), "{w}"); }
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
    #[test] fn collapsed_project_hides_idle_swarms_and_keeps_active_or_needing_ones() {
        let idle = json!({"nav": "@room:r1", "state": "active", "working": 0, "monitoring": false});
        let working = json!({"nav": "@room:r2", "state": "active", "working": 2});
        let monitoring = json!({"nav": "@room:r3", "state": "active", "working": 0, "monitoring": true});
        let paused_working = json!({"nav": "@room:r4", "state": "paused", "working": 1});
        let needs = json!({"nav": "@room:r5", "state": "paused", "working": 0, "need": {"line": "Paused"}});
        let settled = json!({"nav": "@room:r6", "state": "active", "working": 0, "settled": true});
        let done = json!({"nav": "@room:r7", "state": "done", "working": 0});
        let stopped = json!({"nav": "@room:r8", "state": "stopped", "working": 0});
        assert!(!visible_under_project(&idle, true), "collapsed project hides 0-working idle swarm");
        assert!(visible_under_project(&idle, false), "expanded project shows live 0-working swarm");
        assert!(visible_under_project(&working, true), "collapsed project keeps working > 0 swarm");
        assert!(visible_under_project(&monitoring, true), "collapsed project keeps monitoring swarm");
        assert!(!visible_under_project(&paused_working, true), "paused swarm without need is hidden when collapsed");
        assert!(visible_under_project(&needs, true), "collapsed project keeps swarm needing user");
        assert!(!visible_under_project(&settled, false) && !visible_under_project(&settled, true));
        assert!(!visible_under_project(&done, false) && !visible_under_project(&stopped, false));
        let hub_menu = json!({"nav": "@room:r1", "state": "active", "menu": [{"id": "settle", "label": "Settle", "action": "swarm-settle", "value": "r1"}]});
        assert_eq!(menu_items(&hub_menu).len(), 1);
        assert_eq!(menu_items(&idle).iter().map(|i| i.label.as_str()).collect::<Vec<_>>(), ["Pause", "Stop swarm"]);
        assert_eq!(menu_items(&done).len(), 0);
    }
    #[test] fn post_sigs_and_diff_range_compute_minimal_splice() {
        let mut claims = HashMap::new();
        let a = post_sigs(&[json!({"id":"p1","text":"hi"}), json!({"id":"p2","text":"there"})], &claims);
        assert_eq!(diff_range(&a, &a), None);
        let b = post_sigs(&[json!({"id":"p1","text":"hi"}), json!({"id":"p2","text":"there"}), json!({"id":"p3","text":"new"})], &claims);
        assert_eq!(diff_range(&a, &b), Some((2..2, 1)));
        let c = post_sigs(&[json!({"id":"p1","text":"hi"}), json!({"id":"p2","text":"there!","held":"paused"})], &claims);
        assert_eq!(diff_range(&a, &c), Some((1..2, 1)));
        let d = post_sigs(&[json!({"id":"p1","text":"hi"}), json!({"id":"p2","text":"therx"})], &claims);
        assert_eq!(diff_range(&a, &d), Some((1..2, 1)), "same-length text edit invalidates sig");
        claims.insert("p1".into(), "waiting".into());
        let e = post_sigs(&[json!({"id":"p1","text":"hi"}), json!({"id":"p2","text":"there"})], &claims);
        assert_eq!(diff_range(&a, &e), Some((0..1, 1)), "claim change invalidates sig");
    }
    #[test] fn highlight_mentions_bolds_names_inside_markdown_blocks_and_preserves_code_links() {
        let names: HashMap<String, String> = [("thread:1", "opus"), ("thread:2", "neon")].into_iter().map(|(a, b)| (a.into(), b.into())).collect();
        let blocks = md::parse(&[json!({
            "tag": "p",
            "kids": [
                {"text": "Ask @Opus and "},
                {"tag": "code", "kids": [{"text": "@opus"}]},
                {"text": " plus "},
                {"tag": "a", "href": "https://x.io/@opus", "file": false, "kids": [{"text": "@opus"}]}
            ]
        }), json!({
            "tag": "ul",
            "kids": [{"tag": "li", "kids": [{"text": "cc @neon here"}]}]
        })]);
        let out = highlight_mentions(&blocks, &["thread:1".into(), "thread:2".into()], &names);
        let Block::Para(ps) = &out[0] else { panic!() };
        assert_eq!(ps[0].text, "Ask "); assert!(!ps[0].style.bold);
        assert_eq!(ps[1].text, "@Opus"); assert!(ps[1].style.bold);
        assert_eq!(ps[3].text, "@opus"); assert!(ps[3].style.code && !ps[3].style.bold, "code span unchanged");
        assert_eq!(ps[5].text, "@opus"); assert!(ps[5].style.link.is_some() && !ps[5].style.bold, "link unchanged");
        let Block::List { items, .. } = &out[1] else { panic!() };
        assert_eq!(items[0].spans[1].text, "@neon"); assert!(items[0].spans[1].style.bold);
    }
}
