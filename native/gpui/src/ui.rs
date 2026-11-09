//! The window: layout and colours follow the web client (src/web/style.css tokens).
use crate::script::{Ctl, CtlEntry};
use crate::{art, hub::Command, string, theme, Backplane, Send};
use gpui::{prelude::*, *};
use serde_json::Value;

fn pager_row(cx: &mut Context<Backplane>, id: String, pager: &Value, light: bool, ctl: &mut Vec<CtlEntry>) -> Option<AnyElement> {
    let c = |name: &str| theme::color(light, name);
    let mut row = div().flex().gap_2().px_3().py_1().text_size(px(11.));
    let mut any = false;
    for (name, key) in [("Newer", "previous"), ("Older", "next")] {
        if let Some(value) = pager[key]["value"].as_str() {
            any = true;
            let value = value.to_string();
            ctl.push(CtlEntry { id: format!("{id}-{key}"), label: name.to_string(), ctl: Ctl::Page(value.clone()) });
            row = row.child(with_rect(div().id(SharedString::from(format!("{id}-{key}"))), || format!("{id}-{key}")).cursor_pointer().text_color(c("accent")).hover(|s| s.bg(c("raise"))).px_1()
                .child(format!("{} ({})", name, pager["total"]))
                .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Page(value.clone())); cx.notify(); })));
        }
    }
    if any { Some(row.into_any_element()) } else { None }
}

fn inline_text(node: &Value, out: &mut String) {
    if let Some(t) = node["text"].as_str() { out.push_str(t); }
    for k in node["kids"].as_array().into_iter().flatten() { inline_text(k, out); }
}

pub(crate) fn block(node: &Value, light: bool, depth: usize) -> AnyElement {
    let c = |name: &str| theme::color(light, name);
    let tag = node["tag"].as_str().unwrap_or("p");
    let mut text = String::new();
    match tag {
        "ul" | "ol" => {
            let mut list = div().flex().flex_col().gap_1().pl(px(16.));
            for (i, item) in node["kids"].as_array().into_iter().flatten().enumerate() {
                let mut t = String::new(); inline_text(item, &mut t);
                let mark = if tag == "ol" { format!("{}.", i + 1) } else { "•".to_string() };
                list = list.child(div().flex().gap_2().child(div().text_color(c("dim")).child(mark)).child(div().flex_1().child(t)));
            }
            list.into_any_element()
        }
        "pre" => { inline_text(node, &mut text); div().font(art::mono_font()).w_full().bg(c("raise")).border_1().border_color(c("line")).p_2().text_size(px(12.)).child(text).into_any_element() }
        t if t.starts_with('h') && t.len() == 2 => { inline_text(node, &mut text); div().font_weight(FontWeight::BOLD).text_size(px(15.)).child(text).into_any_element() }
        "blockquote" => {
            let mut q = div().border_l_2().border_color(c("line")).pl_3().text_color(c("dim"));
            if depth < 4 { for k in node["kids"].as_array().into_iter().flatten() { q = q.child(block(k, light, depth + 1)); } }
            q.into_any_element()
        }
        _ => { inline_text(node, &mut text); div().child(text).into_any_element() }
    }
}

/// The hub's project-home cards: generic nodes {tag, className, label, action, payload, kids|text}.
fn node_text(n: &Value, out: &mut String) {
    if let Some(t) = n["text"].as_str() { out.push_str(t); }
    for k in n["kids"].as_array().into_iter().flatten() { node_text(k, out); }
}
fn node_el(cx: &mut Context<Backplane>, n: &Value, light: bool, depth: usize, path: &str, ctl: &mut Vec<CtlEntry>) -> AnyElement {
    let c = |name: &str| theme::color(light, name);
    if let Some(t) = n["text"].as_str() { return div().child(t.to_string()).into_any_element(); }
    if depth > 14 { return div().into_any_element(); }
    let tag = n["tag"].as_str().unwrap_or("div");
    let class = n["className"].as_str().unwrap_or("");
    let kids = |cx: &mut Context<Backplane>, ctl: &mut Vec<CtlEntry>| -> Vec<AnyElement> {
        n["kids"].as_array().into_iter().flatten().take(200).enumerate().map(|(i, k)| node_el(cx, k, light, depth + 1, &format!("{path}.{i}"), ctl)).collect()
    };
    match tag {
        "svg" | "path" | "img" => div().into_any_element(),
        "button" => {
            let (action, payload) = (string(n, "action"), string(n, "payload"));
            let mut label = String::new(); node_text(n, &mut label);
            if label.is_empty() { label = string(n, "label"); }
            if !action.is_empty() && !label.is_empty() { ctl.push(CtlEntry { id: format!("node:{path}"), label: label.clone(), ctl: Ctl::Node(action.clone(), payload.clone()) }); }
            if action.is_empty() || label.is_empty() { return div().flex().gap_1().children(kids(cx, ctl)).into_any_element(); }
            with_rect(div().id(SharedString::from(format!("n{path}"))), || format!("node:{path}")).px_2().py_1().border_1().border_color(c("line")).bg(c("raise")).text_size(px(12.)).cursor_pointer().hover(|s| s.bg(c("panel")))
                .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action(action.clone(), payload.clone())); cx.notify(); })).child(label).into_any_element()
        }
        "h1" => { let mut t = String::new(); node_text(n, &mut t); div().text_size(px(22.)).font_weight(FontWeight::BOLD).child(t).into_any_element() }
        "h2" | "h3" => { let mut t = String::new(); node_text(n, &mut t); div().font_weight(FontWeight::BOLD).text_size(px(13.)).child(t).into_any_element() }
        "span" | "p" | "a" | "label" | "code" => {
            let mut t = String::new(); node_text(n, &mut t);
            let mut d = div().child(t);
            if class.contains("eyebrow") || class.contains("faint") || class.contains("dim") { d = d.text_size(px(11.)).text_color(c("faint")); }
            if class.contains("error") { d = d.text_color(c("bad")); }
            d.into_any_element()
        }
        "section" if class.contains("card") => div().flex().flex_col().gap_2().p_3().border_1().border_color(c("line")).bg(c("panel")).children(kids(cx, ctl)).into_any_element(),
        _ => {
            let row = class.contains("name") || class.contains("row") || class.contains("head") || class.contains("meta");
            let d = div().flex().gap_2();
            (if row { d.items_center() } else { d.flex_col() }).children(kids(cx, ctl)).into_any_element()
        }
    }
}

/// In check mode the control records its window rectangle under its id (the e2e driver aims clicks with it); otherwise nothing is added.
pub(crate) fn with_rect<E: ParentElement + Styled>(e: E, id: impl FnOnce() -> String) -> E { if art::rects_on() { e.relative().child(art::probe_rect(id())) } else { e } }
pub(crate) fn ic(light: bool, name: &'static str, size: f32, tok: &str) -> AnyElement { art::icon(name, size, theme::color(light, tok)) }
fn chip(light: bool, label: String) -> Div {
    let c = |name: &str| theme::color(light, name);
    div().px_3().py_1().flex().items_center().justify_center().border_1().border_color(c("line")).bg(c("panel")).text_size(px(12.)).whitespace_nowrap().child(label)
}

/// A message's references as little cards under it: a bot, thread, project or room goes there on a click; a model is only named.
fn ref_cards(cx: &mut Context<Backplane>, light: bool, mid: &str, refs: &Value) -> Option<Div> {
    let list = refs.as_array().filter(|r| !r.is_empty())?;
    let c = |n: &str| theme::color(light, n);
    let mut row = div().flex().flex_wrap().gap_2();
    for (i, r) in list.iter().enumerate() {
        let (kind, id, label, word) = (string(r, "kind"), string(r, "id"), string(r, "label"), string(r, "word"));
        let mark = word.chars().next().map(String::from).unwrap_or_default();
        let go = !id.is_empty();
        let mut card = div().id(SharedString::from(format!("ref-{mid}-{i}"))).flex().items_center().gap_2().px_2().py_1().border_1().border_color(c("line")).bg(c("panel")).text_size(px(12.))
            .child(div().text_color(c(if kind == "bot" { "accent" } else { "ok" })).font_weight(FontWeight::BOLD).child(mark))
            .child(div().max_w(px(220.)).overflow_hidden().whitespace_nowrap().text_ellipsis().child(label))
            .child(div().text_size(px(10.)).text_color(c("faint")).child(kind.clone()));
        if go {
            card = card.cursor_pointer().hover(|s| s.bg(c("raise"))).on_click(cx.listener(move |this, _, window, cx| {
                match kind.as_str() {
                    "bot" => { let _ = this.tx.try_send(Command::Action("bot".into(), id.clone())); }
                    "room" => { let _ = this.tx.try_send(Command::Action("room".into(), id.clone())); }
                    "project" => { this.open_project(id.rsplit('|').next().unwrap_or(&id).to_string(), cx); }
                    _ => { this.select(id.clone(), window, cx); }
                }
                cx.notify();
            }));
        }
        row = row.child(card);
    }
    Some(row)
}

/// "Replying to ..." over the composer: its first line, a click goes to the message, × drops it.
fn reply_chip(cx: &mut Context<Backplane>, light: bool, replying: &Value) -> Option<AnyElement> {
    if !replying.is_object() { return None }
    let c = |n: &str| theme::color(light, n);
    let (target, line) = (string(replying, "id"), string(replying, "text"));
    Some(div().flex().items_center().gap_2().px_2().py_1().mb_1().border_1().border_color(c("line")).bg(c("panel")).text_size(px(12.))
        .child(div().id("reply-chip").flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(c("dim")).cursor_pointer().child(format!("\u{21a9} Replying to: {line}"))
            .on_click(cx.listener(move |this, _, _, cx| { this.timeline_goto(&target); cx.notify(); })))
        .child(div().id("reply-drop").px_1().cursor_pointer().text_color(c("dim")).hover(|s| s.text_color(c("bad"))).child("\u{00d7}")
            .on_click(cx.listener(|this, _, _, cx| { let _ = this.tx.try_send(Command::Action("reply-drop".into(), String::new())); cx.notify(); }))).into_any_element())
}

/// Copy and Reply under a message (a reply puts its first line over the composer and the whole text ahead of what is sent, for the agent).
fn msg_tools(cx: &mut Context<Backplane>, light: bool, id: &str, text: &str) -> Div {
    let c = |n: &str| theme::color(light, n);
    let (copy, reply, rid) = (text.to_string(), format!("{id}\u{1f}{text}"), id.to_string());
    let btn = |tag: &str, label: &'static str| div().id(SharedString::from(format!("{tag}-{rid}"))).px_1().cursor_pointer().text_size(px(11.)).text_color(c("faint")).hover(|s| s.text_color(c("text"))).child(label);
    div().flex().gap_2().child(btn("copy", "Copy").on_click(cx.listener(move |_, _, _, cx| { cx.write_to_clipboard(ClipboardItem::new_string(copy.clone())); })))
        .child(btn("reply", "Reply").on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("reply".into(), reply.clone())); cx.notify(); })))
}

impl Backplane {
    /// One thread row of the sidebar (a project's, or the Active view's: `sub` names the project there, `faded` is a settled one).
    pub(crate) fn side_thread(&self, cx: &mut Context<Self>, light: bool, t: &Value, pid: &str, sub: &str, faded: bool, ctl: &mut Vec<CtlEntry>) -> AnyElement {
        let c = |name: &str| theme::color(light, name);
                let id = string(t, "id");
                let raw = id.rsplit_once('|').map_or(id.as_str(), |(_, i)| i);
                let selected = raw == self.selected && self.project_page.is_none(); // a project's page on show: its head is the one highlighted, no row
                ctl.push(CtlEntry { id: format!("thread:{raw}"), label: string(t, "title"), ctl: Ctl::Select(id.clone()) });
                let (depth, kids, open) = (t["depth"].as_u64().unwrap_or(0).min(6) as f32, t["children"].as_u64().unwrap_or(0), t["expanded"].as_bool().unwrap_or(false));
                let toggle = (kids > 0).then(|| {
                    let tid = id.clone();
                    div().id(SharedString::from(format!("tree-{id}"))).flex_shrink_0().w(px(16.)).h_full().flex().items_center().cursor_pointer().child(ic(light, if open { "chevron-down" } else { "chevron-right" }, 14., "dim"))
                        .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("thread-tree".into(), format!("{tid}|{}", if open { 0 } else { 1 }))); cx.stop_propagation(); cx.notify(); }))
                });
                if kids > 0 { ctl.push(CtlEntry { id: format!("tree:{raw}"), label: format!("{} subthreads", kids), ctl: Ctl::Node("thread-tree".into(), format!("{id}|{}", if open { 0 } else { 1 })) }); }
                with_rect(div().id(SharedString::from(format!("thread-{id}"))), || format!("thread:{raw}")).flex().items_center().gap_2().pl(px(6. + depth * 14.)).pr_3().h(px(32.)).when(faded, |d| d.opacity(0.55))
                    .border_l_2().border_color(if selected { c("accent") } else { c("panel") })
                    .bg(if selected { c("raise") } else { c("panel") }).hover(|s| s.bg(c("raise"))).cursor_pointer()
                    .on_click(cx.listener({ let sid = id.clone(); move |this, _, window, cx| this.select(sid.clone(), window, cx) }))
                    .when(sub.is_empty(), |d| d.on_drag(DragRow { id: raw.to_string(), label: string(t, "title") }, |d, _, _, cx| { let l = d.label.clone(); cx.new(|_| Ghost(l)) })
                        .drag_over::<DragRow>({ let a = c("accent"); move |s, _, _, _| s.border_t_2().border_color(a) }))
                    .on_drop(cx.listener({ let target = raw.to_string(); move |this, d: &DragRow, _, cx| { let _ = this.tx.try_send(Command::Action("reorder".into(), format!("{}\u{1f}{}", d.id, target))); cx.notify(); } }))
                    .on_mouse_down(MouseButton::Right, { let (row, pid2) = (t.clone(), pid.to_string()); cx.listener(move |this, e: &MouseDownEvent, _, cx| { let items = crate::screens::ctxmenu::thread_items(&row, &pid2); this.ctx_open(e.position, items, cx); }) })
                    .child(div().w(px(16.)).flex_shrink_0().children(toggle)).child(crate::screens::siderow::dot(light, t)).child(if crate::screens::siderow::renaming(&self.data, &id, "thread") { crate::screens::siderow::field(light, &self.screens.fields, &id) } else { div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().flex().gap_2().relative().child(div().overflow_hidden().text_ellipsis().child(string(t, "title"))).children((!sub.is_empty()).then(|| div().flex_shrink_0().max_w(px(80.)).overflow_hidden().text_ellipsis().text_size(px(11.)).text_color(c("faint")).child(sub.to_string())))
                        // held Ctrl: this row's Ctrl+digit; held Alt, in the Active view: its project's Alt+digit; over the title's end, so nothing moves (screens/hints.rs)
                        .children(crate::screens::hints::digit_of(&self.screens.hold_threads, &id).or_else(|| (!sub.is_empty()).then(|| crate::screens::hints::digit_of(&self.screens.hold_projects, pid)).flatten()).map(|n| crate::screens::hints::over_end(crate::screens::hints::digit(light, n)))).into_any_element() })
                    .children(crate::screens::badge::machine_mark(light, &t["mark"], false, &id)) // (another machine's thread: its name, dim; none for this machine's)
                    .child(div().text_size(px(11.)).text_color(c("faint")).child(if kids > 0 && string(t, "agents").is_empty() { format!("{kids}") } else { string(t, "ago") })).into_any_element()
    }
}

impl Backplane {
    /// One row of the timeline, as the list asks for it: only rows on screen are drawn.
    pub(crate) fn timeline_row(&mut self, ei: usize, cx: &mut Context<Self>) -> AnyElement {
        let light = self.light();
        let c = |name: &str| theme::color(light, name);
        let reg = self.screens.reg.clone();
        let e = self.data["thread"]["entries"][ei].clone();
        let id = string(&e, "id");
        reg.borrow_mut().enter(ei, &id);
        let kind = e["kind"].as_str().unwrap_or("");
        let text = string(&e, "text");
        let stamp = string(&e, "stamp");
        let row: AnyElement = match kind {
            "user" => {
                let wide = crate::screens::timeline::bubble_full_width(&text);
                let ref_words: Vec<String> = e["refs"].as_array().into_iter().flatten().map(|r| string(r, "word")).collect();
                let body = crate::screens::timeline::user_text(&reg, light, &text, &ref_words);
                let atts = crate::screens::timeline::attachments(cx, light, &e["attachments"], &id, self.screens.attacher.target.as_ref(), &self.screens.images);
                let tools = msg_tools(cx, light, &id, &text);
                let cards = ref_cards(cx, light, &id, &e["refs"]);
                let stamp_row = div().flex().justify_end().items_center().gap_3().text_size(px(11.)).text_color(c("faint")).child(tools).child(stamp);
                let reply_to = e["replyTo"].is_object().then(|| {
                    let (target, quote) = (string(&e["replyTo"], "id"), string(&e["replyTo"], "quote"));
                    div().id(SharedString::from(format!("rt-{id}"))).mb_1().max_w_full().overflow_hidden().whitespace_nowrap().text_ellipsis().text_size(px(11.)).text_color(c("dim")).cursor_pointer().hover(|s| s.text_color(c("text")))
                        .child(format!("\u{21a9} In reply to: {quote}")).on_click(cx.listener(move |this, _, _, cx| { this.timeline_goto(&target); cx.notify(); }))
                });
                let row = div().id(SharedString::from(format!("e-{id}"))).w_full().py_2();
                // A text that may wrap sits in block layout like a reply's (a shrink-to-fit flex bubble measured it at one width and painted it at another);
                // a message of short lines keeps the snug flex bubble.
                if wide {
                    row.child(div().w_full().max_w(px(640.)).ml_auto().px_3().py_2().bg(c("raise")).border_1().border_color(c("line")).children(reply_to).child(body).children(cards.map(|k| k.mt_1())).children(atts.into_iter().map(|a| div().mt_1().child(a))).child(div().mt_1().child(stamp_row))).into_any_element()
                } else {
                    row.flex().justify_end().child(div().max_w(px(640.)).px_3().py_2().bg(c("raise")).border_1().border_color(c("line")).flex().flex_col().gap_1().children(reply_to).child(body).children(cards).children(atts).child(stamp_row)).into_any_element()
                }
            }
            "fold" => {
                let value = string(&e, "value");
                self.screens.parts.rows_ctl.push(CtlEntry { id: format!("fold:{id}"), label: text.clone(), ctl: Ctl::Fold(value.clone()) });
                // the label is one clipped line beside the chevron, never wider than the row; the chevron is centred on it
                div().id(SharedString::from(format!("e-{id}"))).w_full().py_2().child(
                    with_rect(div().id(SharedString::from(format!("f-{id}"))), || format!("fold:{id}")).w_full().min_w_0().overflow_hidden().flex().items_center().gap_2().px_2().py_1().border_1().border_color(c("line")).text_size(px(12.)).text_color(c("dim")).cursor_pointer()
                        .hover(|s| s.bg(c("raise")))
                        .on_click(cx.listener(move |this, _, _, cx| { this.fold_click(value.clone()); cx.notify(); }))
                        .child(div().flex_shrink_0().flex().items_center().child(ic(light, if e["open"] == true { "chevron-down" } else { "chevron-right" }, 14., "dim")))
                        .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(text))).into_any_element()
            }
            // the agent still working: a green square that breathes beside the word (the one animation on screen, only while this row is)
            "assistant" if id.starts_with("@live:") || id.starts_with("@working:") => {
                // drawn at crate::part::PULSE's rate, not every display frame: each step draws this part (the visible messages) and the root's shell again
                let (d, dots) = crate::part::pulse(self.started.elapsed());
                self.screens.parts.tick(Backplane::timeline_part, crate::part::PULSE, cx);
                let square = div().size(px(12.)).flex_shrink_0().flex().items_center().justify_center().child(div().size(px(5. + 7. * d)).opacity(0.45 + 0.55 * d).bg(c("ok")));
                div().id(SharedString::from(format!("e-{id}"))).w_full().py_2().flex().items_center().gap_2().text_color(c("dim")).child(square)
                    // the dots count up and start over: "Working." "Working.." "Working..." (a fixed width so the line does not jitter)
                    .child(div().min_w(px(86.)).child(format!("Working{}", ".".repeat(dots)))).into_any_element()
            }
            "assistant" => {
                let mut col = div().id(SharedString::from(format!("e-{id}"))).w_full().py_2().flex().flex_col().gap_2();
                match e["blocks"].as_array() {
                    Some(blocks) if !blocks.is_empty() => { let parsed = self.screens.parsed.get(&id, &text, blocks); for el in crate::screens::timeline::blocks(cx, &reg, light, &parsed, &id) { col = col.child(el); } }
                    _ => { col = col.child(crate::screens::timeline::user_text(&reg, light, &text, &[])); }
                }
                if let Some(im) = crate::screens::timeline::images(cx, light, &e["images"], &id, self.screens.attacher.target.as_ref(), &self.screens.images) { col = col.child(im); }
                let tools = msg_tools(cx, light, &id, &text);
                col.child(div().flex().items_center().gap_3().text_size(px(11.)).text_color(c("faint")).child(stamp).child(tools)).into_any_element()
            }
            "term" => self.term_card(&e, light, cx),
            _ if !text.is_empty() => div().id(SharedString::from(format!("e-{id}"))).flex().gap_2().py_1().font(art::mono_font()).text_size(px(12.)).text_color(c("dim")).child(string(&e, "label")).child(text).into_any_element(),
            _ => div().into_any_element(),
        };
        let inner = div().w_full().max_w(px(930.)).px_4().child(row);
        // A row that arrives while the thread is on screen (a sent message, the reply starting) fades and slides in over 220 ms instead of popping in.
        // Only the newest rows, only within a moment of their first draw, so scrolling back to a row or opening a thread never replays it.
        let len = self.data["thread"]["entries"].as_array().map_or(0, |r| r.len());
        let settled = self.screens.shown_at.elapsed().as_millis() > 800;
        let first = *self.screens.born.entry(id.clone()).or_insert_with(std::time::Instant::now);
        let fresh = settled && ei + 3 >= len && first.elapsed().as_millis() < 260;
        let inner: AnyElement = if fresh {
            inner.with_animation(SharedString::from(format!("arrive-{id}")), Animation::new(std::time::Duration::from_millis(220)).with_easing(ease_out_quint()),
                |el, d| el.opacity(d).mt(px(10. * (1. - d)))).into_any_element()
        } else { inner.into_any_element() };
        div().w_full().flex().justify_center().child(inner).into_any_element()
    }
    /// The sidebar's rows, a part of the window (part.rs): the uberbot, projects and their threads, the Active view, pagers, conversation history, Older, BOTS and ROOMS.
    pub(crate) fn sidebar_part(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let light = self.light();
        let c = |name: &str| theme::color(light, name);
        // bots: (name, mood, note, look seed) copied out so the cat cache can be filled
        // the uberbot (flagged by the hub) is drawn in its own spot above the projects, so it is kept apart from the BOTS list
        let uber_data: Vec<(String, String, String, u64, String, bool)> = self.data["bots"].as_array().into_iter().flatten().filter(|b| b["uber"].as_bool().unwrap_or(false))
            .map(|b| (string(b, "name"), string(b, "mood"), string(b, "note"), b["look"].as_u64().unwrap_or(0), string(b, "id"), b["sel"].as_bool().unwrap_or(false))).collect();
        let uber_rigs: Vec<_> = uber_data.iter().map(|(_, mood, _, look, _, _)| self.cats.rig(*look, mood)).collect();
        let bot_data: Vec<(String, String, String, u64, String, bool)> = self.data["bots"].as_array().into_iter().flatten().filter(|b| !b["uber"].as_bool().unwrap_or(false))
            .map(|b| (string(b, "name"), string(b, "mood"), string(b, "note"), b["look"].as_u64().unwrap_or(0), string(b, "id"), b["sel"].as_bool().unwrap_or(false))).collect();
        let bot_rigs: Vec<_> = bot_data.iter().map(|(_, mood, _, look, _, _)| self.cats.rig(*look, mood)).collect();
        let started = self.started;
        let mut rows: Vec<AnyElement> = Vec::new();
        if let Some(r) = crate::screens::onboard::resume_row(cx, light, &self.data) { rows.push(r); }
        let mut ctl: Vec<CtlEntry> = Vec::new();
        for ((bname, mood, note, _, bid, bsel), rig) in uber_data.iter().zip(uber_rigs.iter()) {
            let note_color = match mood.as_str() { "wait" | "error" => c("bad"), "work" | "talk" => c("accent"), _ => c("faint") };
            ctl.push(CtlEntry { id: format!("bot:{bname}"), label: bname.clone(), ctl: Ctl::Node("bot".into(), bid.clone()) });
            let bid2 = bid.clone();
            rows.push(with_rect(div().id(SharedString::from(format!("bot-{bid}"))), || format!("bot:{bname}")).flex().items_center().gap_3().px_3().py(px(10.)).border_b_1().border_color(c("line")).cursor_pointer().when(*bsel, |d| d.bg(c("raise"))).hover(|s| s.bg(c("raise")))
                .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("bot".into(), bid2.clone())); cx.notify(); }))
                .child(art::cat(rig.clone(), mood, 56., started, if art::rects_on() { format!("cat:{bname}") } else { String::new() }))
                .child(div().flex().flex_col().min_w_0().line_height(px(22.)).child(div().text_size(px(16.)).overflow_hidden().whitespace_nowrap().child(bname.clone()))
                    .child(div().text_size(px(11.)).text_color(note_color).child(note.clone()))).into_any_element());
        }
        let new_here = crate::keys::new_target(&self.data, &self.selected, self.project_page.as_deref());
        for project in self.data["projects"].as_array().into_iter().flatten() {
            let pid = string(project, "id"); let pid = pid.rsplit_once('|').map_or(pid.as_str(), |(_, r)| r).to_string();
            ctl.push(CtlEntry { id: format!("project:{pid}"), label: string(project, "title"), ctl: Ctl::Home(pid.clone()) });
            let (pid_new, pid_new2, pid_thr, pid_set) = (pid.clone(), pid.clone(), pid.clone(), pid.clone());
            let (on_page, closed, qpid) = (self.project_page.as_deref() == Some(pid_new.as_str()), project["closed"].as_bool().unwrap_or(false), string(project, "id"));
            let fold_value = format!("project-closed:{pid}");
            ctl.push(CtlEntry { id: format!("fold:{pid}"), label: format!("{} {}", if closed { "Unfold" } else { "Fold" }, string(project, "title")), ctl: Ctl::Node("flag".into(), fold_value.clone()) });
            rows.push(with_rect(div().id(SharedString::from(format!("proj-{pid}"))), || format!("project:{pid}")).flex().items_center().gap_2().pl(px(6.)).pr_3().pt_3().pb_1().text_size(px(11.)).text_color(if on_page { c("text") } else { c("dim") }).border_l_2().border_color(if on_page { c("accent") } else { c("bg") }).when(on_page, |d| d.bg(c("raise"))).cursor_pointer().hover(|s| s.text_color(c("text")))
                .on_click(cx.listener(move |this, _, _, cx| { this.open_project(pid.clone(), cx); }))
                .on_drag(DragProj { id: pid_new.clone(), label: string(project, "title") }, |d, _, _, cx| { let l = d.label.clone(); cx.new(|_| Ghost(l)) })
                .drag_over::<DragProj>({ let a = c("accent"); move |s, _, _, _| s.border_t_2().border_color(a) })
                .on_drop(cx.listener({ let target = pid_new.clone(); move |this, d: &DragProj, _, cx| { let _ = this.tx.try_send(Command::Action("reorder-project".into(), format!("{}\u{1f}{}", d.id, target))); cx.notify(); } }))
                .on_mouse_down(MouseButton::Right, { let pidm = pid_new.clone(); cx.listener(move |this, e: &MouseDownEvent, _, cx| { let items = crate::screens::ctxmenu::project_items(&pidm); this.ctx_open(e.position, items, cx); }) })
                .child(with_rect(div().id(SharedString::from(format!("fold-{pid_new}"))), || format!("fold:{pid_new}")).w(px(16.)).flex_shrink_0().flex().items_center().cursor_pointer().hover(|s| s.bg(c("raise"))).child(ic(light, if closed { "chevron-right" } else { "chevron-down" }, 14., "dim"))
                    .on_click(cx.listener(move |this, _, _, cx| { cx.stop_propagation(); let _ = this.tx.try_send(Command::Action("flag".into(), fold_value.clone())); cx.notify(); })))
                .child(crate::screens::badge::mark(light, project)).child(div().flex_1().min_w_0().flex().items_center().gap_2().relative().child(if crate::screens::siderow::renaming(&self.data, &qpid, "project") { crate::screens::siderow::field(light, &self.screens.fields, &qpid) } else { div().child(string(project, "title").to_uppercase()).into_any_element() }).children(crate::screens::badge::place(light, &project["mark"], project["also"].as_str().unwrap_or(""), &format!("proj-{pid_new}")))
                    .children(crate::screens::hints::digit_of(&self.screens.hold_projects, &pid_new).map(|n| crate::screens::hints::over_end(crate::screens::hints::digit(light, n))))).child(with_rect(div().id(SharedString::from(format!("new-{pid_new}"))), || "ui:New thread".to_string()).cursor_pointer().tooltip(crate::keys::tip("New thread", &if pid_new == new_here { crate::keys::hint(&self.local, "new") } else { String::new() }, light)).on_click(cx.listener(move |this, _, _, cx| { cx.stop_propagation(); this.arm_new(); let _ = this.tx.try_send(Command::Action("new-thread".into(), pid_new2.clone())); cx.notify(); })).child(ic(light, "plus", 16., "dim"))).into_any_element());
            if project["look"].is_object() {
                for v in crate::screens::badge::choices(&project["look"], "colors").into_iter().chain(crate::screens::badge::choices(&project["look"], "icons")) { ctl.push(CtlEntry { id: format!("look:{}", v.1), label: v.0, ctl: Ctl::Node("look-set".into(), v.1) }); }
                rows.push(crate::screens::badge::editor(cx, light, &pid_thr, &project["look"]));
            }
            if closed { continue } // folded: the head alone (0.14 project-closed:<id>)
            for t in project["threads"].as_array().into_iter().flatten() { rows.push(self.side_thread(cx, light, t, &pid_thr, "", false, &mut ctl)); }
            // the Settled shelf: a drop-down at the foot of the project; the hub builds its rows only while it is open
            let settled_n = project["settledCount"].as_u64().unwrap_or_else(|| project["settled"].as_array().map_or(0, |a| a.len() as u64));
            let settled_open = project["open"].as_bool().unwrap_or(false);
            if settled_n > 0 || settled_open {
                let value = string(project, "id");
                ctl.push(CtlEntry { id: format!("settled:{pid_set}"), label: format!("Settled ({settled_n})"), ctl: Ctl::Node("flag".into(), value.clone()) });
                rows.push(with_rect(div().id(SharedString::from(format!("settled-{pid_set}"))), || format!("settled:{pid_set}")).flex().items_center().gap_1().pl(px(6.)).pr_3().h(px(26.)).text_size(px(11.)).text_color(c("faint")).cursor_pointer().hover(|s| s.bg(c("raise")))
                    .child(div().w(px(16.)).flex_shrink_0().child(ic(light, if settled_open { "chevron-down" } else { "chevron-right" }, 14., "dim"))).child(format!("Settled ({settled_n})"))
                    .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("flag".into(), value.clone())); cx.notify(); })).into_any_element());
                if settled_open { for t in project["settled"].as_array().into_iter().flatten() { rows.push(self.side_thread(cx, light, t, &pid_thr, "", true, &mut ctl)); } }
            }
        }
        // the Active view: threads that need the person first, then the settled ones folded under "Settled N"
        for t in self.data["active"].as_array().into_iter().flatten() {
            let pid = string(t, "projectId"); let pid = pid.rsplit_once('|').map_or(pid.as_str(), |(_, r)| r).to_string();
            rows.push(self.side_thread(cx, light, t, &pid, &string(t, "project"), false, &mut ctl));
        }
        if let Some(n) = self.data["activeSettled"]["count"].as_u64().filter(|n| *n > 0) {
            let open = self.data["activeSettled"]["open"].as_bool().unwrap_or(false);
            ctl.push(CtlEntry { id: "side-settled".into(), label: "Settled".into(), ctl: Ctl::Node("side-settled".into(), String::new()) });
            rows.push(with_rect(div().id("side-settled"), || "side-settled".to_string()).flex().items_center().gap_2().px_3().py_1().mt_2().text_size(px(11.)).text_color(c("accent")).cursor_pointer().hover(|s| s.bg(c("raise")))
                .child(ic(light, if open { "chevron-down" } else { "chevron-right" }, 14., "dim")).child(format!("Settled {n}"))
                .on_click(cx.listener(|this, _, _, cx| { let _ = this.tx.try_send(Command::Action("side-settled".into(), String::new())); cx.notify(); })).into_any_element());
            for t in self.data["activeSettled"]["rows"].as_array().into_iter().flatten() {
                let pid = string(t, "projectId"); let pid = pid.rsplit_once('|').map_or(pid.as_str(), |(_, r)| r).to_string();
                rows.push(self.side_thread(cx, light, t, &pid, &string(t, "project"), true, &mut ctl));
            }
        }
        for page in self.data["sidebarPages"].as_array().into_iter().flatten() {
            for key in ["projects", "active", "legacy"] { if let Some(row) = pager_row(cx, format!("pg-{key}"), &page[key], light, &mut ctl) { rows.push(row); } }
        }
        // Conversation history: the old Scratch chats. Nothing starts here; a chat that wants attention always has its row, the rest open from this row or from a search
        let history = &self.data["conversationHistory"];
        if history["toggle"] == true {
            let (n, open) = (history["count"].as_u64().unwrap_or(0), history["open"] == true);
            ctl.push(CtlEntry { id: "side-history".into(), label: "Conversation history".into(), ctl: Ctl::Node("flag".into(), "conversation-history".into()) });
            rows.push(with_rect(div().id("side-history"), || "side-history".to_string()).flex().items_center().gap_2().px_3().py_1().text_size(px(11.)).text_color(c("accent")).cursor_pointer().hover(|s| s.bg(c("raise")))
                .child(ic(light, if open { "chevron-down" } else { "chevron-right" }, 14., "dim")).child(format!("Conversation history ({n})"))
                .on_click(cx.listener(|this, _, _, cx| { let _ = this.tx.try_send(Command::Action("flag".into(), "conversation-history".into())); cx.notify(); })).into_any_element());
        }
        for t in self.data["legacy"].as_array().into_iter().flatten() { rows.push(self.side_thread(cx, light, t, "", "", false, &mut ctl)); }
        // the Older group of projects (no live thread, quiet for a while): a row that opens and shuts it
        if let Some(n) = self.data["olderProjects"]["count"].as_u64() {
            let open = self.data["olderProjects"]["open"].as_bool().unwrap_or(false);
            ctl.push(CtlEntry { id: "side-older".into(), label: "Older projects".into(), ctl: Ctl::Node("side-older".into(), String::new()) });
            rows.push(with_rect(div().id("side-older"), || "side-older".to_string()).flex().items_center().gap_2().px_3().py_1().text_size(px(11.)).text_color(c("accent")).cursor_pointer().hover(|s| s.bg(c("raise")))
                .child(ic(light, if open { "chevron-down" } else { "chevron-right" }, 14., "dim")).child(if open { format!("Older ({n}) · hide") } else { format!("Older ({n})") })
                .on_click(cx.listener(|this, _, _, cx| { let _ = this.tx.try_send(Command::Action("side-older".into(), String::new())); cx.notify(); })).into_any_element());
        }
        for name in ["BOTS", "ROOMS"] {
            let (plus_id, plus_action) = if name == "BOTS" { ("ui:New bot", "bot-new") } else { ("ui:New room", "room-new") };
            ctl.push(CtlEntry { id: plus_id.into(), label: plus_id.trim_start_matches("ui:").into(), ctl: Ctl::Node(plus_action.into(), String::new()) });
            rows.push(div().flex().justify_between().px_3().pt_4().text_size(px(11.)).text_color(c("dim")).child(name)
                .child(with_rect(div().id(plus_id), || plus_id.to_string()).cursor_pointer().on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action(plus_action.into(), String::new())); cx.notify(); })).child(ic(light, "plus", 16., "dim"))).into_any_element());
            if name == "BOTS" {
                for ((bname, mood, note, _, bid, bsel), rig) in bot_data.iter().zip(bot_rigs.iter()) {
                    let note_color = match mood.as_str() { "wait" | "error" => c("bad"), "work" | "talk" => c("accent"), _ => c("faint") };
                    ctl.push(CtlEntry { id: format!("bot:{bname}"), label: bname.clone(), ctl: Ctl::Node("bot".into(), bid.clone()) });
                    let bid2 = bid.clone();
                    rows.push(with_rect(div().id(SharedString::from(format!("bot-{bid}"))), || format!("bot:{bname}")).flex().items_center().gap_2().px_3().py(px(3.)).cursor_pointer().when(*bsel, |d| d.bg(c("raise"))).hover(|s| s.bg(c("raise")))
                        .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("bot".into(), bid2.clone())); cx.notify(); }))
                        .child(art::cat(rig.clone(), mood, 28., started, if art::rects_on() { format!("cat:{bname}") } else { String::new() }))
                        .child(div().flex().flex_col().min_w_0().line_height(px(18.)).child(div().overflow_hidden().whitespace_nowrap().child(bname.clone()))
                            .child(div().text_size(px(11.)).text_color(note_color).child(note.clone()))).into_any_element());
                }
            } else {
                for r in self.data["rooms"].as_array().into_iter().flatten() {
                    let (rid, rname, sel) = (string(r, "id"), string(r, "name"), r["sel"].as_bool().unwrap_or(false));
                    ctl.push(CtlEntry { id: format!("room:{rname}"), label: rname.clone(), ctl: Ctl::Node("room".into(), rid.clone()) });
                    let rid2 = rid.clone();
                    rows.push(with_rect(div().id(SharedString::from(format!("room-{rid}"))), || format!("room:{rname}")).flex().items_center().gap_2().px_3().py_1().cursor_pointer().when(sel, |d| d.bg(c("raise"))).hover(|s| s.bg(c("raise")))
                        .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("room".into(), rid2.clone())); cx.notify(); }))
                        .child(div().text_color(c("faint")).child("#")).child(div().flex_1().overflow_hidden().whitespace_nowrap().child(rname.clone()))
                        .child(div().text_size(px(11.)).text_color(c("faint")).child(format!("{}", r["members"]))).into_any_element());
                }
            }
        }
        self.screens.parts.side_ctl = ctl;
        div().id("sidebar").size_full().overflow_y_scroll().children(rows).into_any_element()
    }
    /// The viewer's page, a part of the window (part.rs): a board or schematic is drawn again only when the page itself changed, not for
    /// a hover in the sidebar or the caret's blink.
    pub(crate) fn viewer_part(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let light = self.light();
        let reg = self.screens.reg.clone(); reg.borrow_mut().begin_outside();
        let thread = &self.data["thread"];
        let el = crate::screens::viewer::page(cx, &reg, light, &self.screens.files, &self.screens.images, self.screens.attacher.target.as_ref(), thread, &self.screens.plot, &mut self.screens.plotview, &mut self.screens.split_pv, self.screens.solid.as_ref(), self.screens.browser.as_ref().map(|(_, b)| b));
        { let rows = &self.data["thread"]["entries"]; reg.borrow_mut().resolve_by(|id| rows.as_array().and_then(|r| r.iter().position(|e| e["id"].as_str() == Some(id)))); }
        el
    }
    /// The thread's messages, a part of the window (part.rs): a list that draws only the rows on screen.
    pub(crate) fn timeline_part(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let light = self.light();
        let c = |name: &str| theme::color(light, name);
        self.screens.reg.borrow_mut().begin_rows(); self.screens.parts.rows_ctl.clear();
        let thread = &self.data["thread"];
        let has_thread = thread.is_object();
        let no_rows = thread["entries"].as_array().map_or(true, |e| e.is_empty());
        div().id("timeline").size_full().relative().flex().flex_col()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, e, _, cx| this.timeline_down(e, cx))).on_mouse_down(MouseButton::Right, cx.listener(|this, e, _, cx| this.timeline_down(e, cx)))
            .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, _, _| this.scroll_note(e)))
            .on_mouse_move(cx.listener(|this, e, _, cx| this.timeline_move(e, cx))).on_mouse_up(MouseButton::Left, cx.listener(|this, e, _, cx| this.timeline_up(e, cx))).on_mouse_up_out(MouseButton::Left, cx.listener(|this, e, _, cx| this.timeline_up(e, cx)))
            .when(no_rows, |d| d.child(div().flex_1().flex().justify_center().items_end().child(div().w_full().max_w(px(930.)).px_4().py_6().text_color(c("dim")).child(if has_thread && thread["loading"] == true { "Loading…" } else if has_thread { "Start a conversation." } else { "Pick a thread, or start one with + next to a project." }))))
            .when(!no_rows, |d| d.child(list(self.timeline.clone(), cx.processor(|this, ix, _, cx| this.timeline_row(ix, cx))).flex_1().min_h_0()))
            .children(thread["live"].as_array().into_iter().flatten().map(|live| div().w_full().flex().justify_center().child(div().w_full().max_w(px(930.)).px_4().py_2().child(string(live, "text")))))
            // after the list has drawn its rows: put them in reading order, and see whether history is wanted
            .child(canvas({ let (me, reg) = (cx.entity().downgrade(), self.screens.reg.clone()); move |_, _, cx| {
                let Some(this) = me.upgrade() else { return };
                { let rows = &this.read(cx).data["thread"]["entries"]; reg.borrow_mut().resolve_by(|id| rows.as_array().and_then(|r| r.iter().position(|e| e["id"].as_str() == Some(id)))); }
                cx.defer(move |cx| this.update(cx, |this, _| this.history_want()));
            } }, |_, _, _, _| {}).absolute().size_0()).into_any_element()
    }
}

impl Render for Backplane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.screens.plotview.layers_key = crate::keys::hint(&self.local, "layers");
        let t_render = std::time::Instant::now();
        if crate::perf::on() { crate::perf::frame_start(); cx.defer(|_| crate::perf::frame_end()); }
        crate::screens::improvements::tick(self, window, cx);
        // a 3D page not drawn this frame gives its pictures back to the GPU (they stayed in the atlas until the window closed)
        if let Some(s) = self.screens.solid.clone() { if s.read(cx).atlas_images() > 0 && !crate::screens::viewer::shows_solid(&self.data["thread"]) { s.update(cx, |p, _| p.put_away(window)); } }
        crate::screens::desk::tick(self, window, cx);
        self.term_layout(window, cx);
        self.term_script(window, cx);
        if self.want_focus && self.screens.pad.is_none() { self.want_focus = false; self.input.focus_handle(cx).focus(window); }
        if self.models_pending { self.models_pending = false; self.picker_toggle(window, cx); }
        if let Some(w) = self.screens.projects.focus.take() { let e = if w == "folder" { self.screens.projects.folder.clone() } else { self.screens.projects.search.clone() }; if let Some(e) = e { window.focus(&e.focus_handle(cx)); } }
        if let Some(n) = self.screens.focus_field.take() { if let Some(s) = self.screens.fields.get(&n) { window.focus(&s.input.focus_handle(cx)); } }
        if self.screens.focus_settings { self.screens.focus_settings = false; window.focus(&self.screens.settings_focus); }
        if self.screens.setup.want_focus { self.screens.setup.want_focus = false; window.focus(&self.screens.setup_focus); }
        if self.screens.palette.focus { self.screens.palette.focus = false; if let Some(i) = self.screens.palette.input.clone() { window.focus(&i.focus_handle(cx)); } }
        self.frame_marks(window, cx);
        self.hints_frame(window, cx);
        let light = self.light();
        if self.swatches { return crate::swatch::view(light).into_any_element(); }
        if self.gallery { return crate::swatch::gallery(light).into_any_element(); }
        if self.fonts { return crate::swatch::fonts(light).into_any_element(); }
        if self.pair.is_some() { let (el, ctl) = crate::screens::pair::view(self, light, window, cx); self.controls = ctl; return el; }
        let c = |name: &str| theme::color(light, name);
        // the selectable text is the rows' part's and the viewer part's (part.rs); with the viewer shut, what its page drew is gone
        if !crate::screens::viewer::is_open(&self.data["thread"]) { self.screens.reg.borrow_mut().begin_outside(); }
        self.screens.images.frame();
        if self.input.read(cx).light != light { self.input.update(cx, |i, _| i.light = light); for i in self.screens.ask_inputs.values() { i.update(cx, |i, _| i.light = light); } }
        let thread = &self.data["thread"];
        let dictating = self.screens.voice.views.get(&self.selected).map_or(false, |v| v["on"] == true);
        let has_thread = thread.is_object();
        let mut ctl: Vec<CtlEntry> = Vec::new(); // the controls drawn this frame (the e2e harness's inventory)
        // project home replaces the thread when the hub selected it
        let home = &self.data["projectHome"];
        let page = self.project_page.clone().filter(|p| crate::screens::projects::find(&self.data, p).is_some());
        let has_home = home.is_object() || page.is_some();
        // the project home replaces the rows' part: what it drew last is not on screen (its controls, its text for selection)
        if has_home { self.screens.parts.rows_ctl.clear(); self.screens.reg.borrow_mut().begin_rows(); }
        let home_els: Vec<AnyElement> = if let (false, Some(p)) = (home.is_object(), page.as_ref()) { crate::screens::projects::page(cx, light, &self.data, &self.screens.projects.home, p, &mut ctl) } else if has_home {
            home["cards"].as_array().into_iter().flatten().take(60).enumerate().map(|(i, k)| node_el(cx, k, light, 0, &format!("h{i}"), &mut ctl)).collect()
        } else { Vec::new() };
        // popup
        // the popup is the hub's: rows are shown when thread.skills is non-empty (no client-side rows)
        let skills = thread["skills"].as_array();
        // the highlighted row starts at the first for each new set of rows; the composer is told so Up/Down/Tab drive the popup while it shows
        { let key = self.popup_rows().join("\u{1f}"); if key != self.screens.popup_key { self.screens.popup_key = key; self.screens.popup_ix = 0; self.screens.popup_scroll.scroll_to_item(0); } }
        let popup_ix = self.screens.popup_ix.min(skills.map_or(0, |s| s.len()).saturating_sub(1));
        // eight rows show; the rest scroll, and the highlighted row is kept in view (glue.rs)
        let mut popup = div().id("popup-list").track_scroll(&self.screens.popup_scroll).max_h(px(8. * 28.)).overflow_y_scroll().flex().flex_col().py_1();
        let mut group = String::new();
        for (index, skill) in skills.into_iter().flatten().enumerate() {
            // a source's rows sit under its name (the hub groups them; rows without one draw as before); the name is drawn in its first row's box,
            // so the list keeps one child per row and scroll_to_item(popup_ix) lands on the highlighted row
            let g = skill.get("group").and_then(Value::as_str).unwrap_or("").to_string();
            let head = (!g.is_empty() && g != group).then(|| div().px_3().pt_1().text_xs().text_color(c("dim")).child(g.clone()));
            group = g;
            let word = skill.get("word").or_else(|| skill.get("name")).and_then(Value::as_str).unwrap_or("").to_string();
            let label = skill.get("label").and_then(Value::as_str).unwrap_or(&word).to_string();
            let desc = ["text", "hint", "desc", "detail", "about"].iter().find_map(|k| skill.get(*k).and_then(Value::as_str)).unwrap_or("").to_string();
            let desc = match skill.get("note").and_then(Value::as_str) { Some(n) if !n.is_empty() => format!("[{n}] {desc}"), _ => desc };
            ctl.push(CtlEntry { id: format!("skill:{word}"), label: label.clone(), ctl: Ctl::Skill(word.clone()) });
            popup = popup.child(div().flex().flex_col().children(head).child(with_rect(div().id(("skill", index)), || format!("skill:{word}")).flex().gap_3().px_3().py_1().cursor_pointer().when(index == popup_ix, |d| d.bg(c("raise"))).hover(|s| s.bg(c("raise")))
                .child(div().font_weight(FontWeight::BOLD).flex_shrink_0().child(label)).child(div().flex_1().overflow_hidden().whitespace_nowrap().text_color(c("dim")).child(desc))
                .on_click(cx.listener(move |this, _, window, cx| {
                    let _ = this.tx.try_send(Command::Action("skill".into(), word.clone())); this.await_completion(cx);
                    this.input.focus_handle(cx).focus(window); cx.notify();
                }))));
        }
        // the reference words the hub has offered become chips in the draft (it knows them as models, bots, threads...)
        { let fresh: Vec<String> = skills.into_iter().flatten().filter_map(|s| s.get("word").and_then(Value::as_str)).filter(|w| w.len() > 1 && w.starts_with(|c| matches!(c, '@' | '>' | '%' | '#' | '&' | '~' | '$')) && !w.starts_with("$install")).map(String::from).collect();
          let mut changed = false;
          for w in fresh { if !self.screens.known_refs.contains(&w) { self.screens.known_refs.push(w); changed = true; } }
          if changed { self.screens.known_refs.sort_by_key(|w| std::cmp::Reverse(w.len())); self.screens.known_refs.truncate(300); let k = self.screens.known_refs.clone(); self.input.update(cx, |i, cx| { i.chips = k; cx.notify(); }); } }
        let shut = self.screens.popup_off.as_deref().map_or(false, |d| d == self.input.read(cx).content.as_ref());
        if !shut { self.screens.popup_off = None; }
        self.screens.popup_shut = shut;
        let show_popup = !shut && skills.map_or(false, |s| !s.is_empty());
        if self.input.read(cx).popup_open != show_popup { self.input.update(cx, |c, _| c.popup_open = show_popup); }
        let (pv, pm, py) = (f32::from(self.screens.popup_scroll.bounds().size.height), f32::from(self.screens.popup_scroll.max_offset().height), -f32::from(self.screens.popup_scroll.offset().y));
        let popup_bar = crate::screens::picker::thumb(pv, pv, pm, py).map(|(top, h)| div().absolute().top(px(top)).right(px(2.)).w(px(6.)).h(px(h)).bg(c("faint")).opacity(0.7));
        // chips
        // Thread permissions are read-only here and are not drawn in the composer: the web's composer chip beside the model chip is the account chip.
        let ctx = string(&thread["picker"], "label").split("ctx ").nth(1).map(|s| format!("ctx {}", s.split(' ').next().unwrap_or(""))).unwrap_or_default();
        // the one chip (screens/combo.rs): the row's width last frame, less its other controls, is what the chip may use
        let bar_w = self.screens.acct.bar_w.get();
        let combo_avail = if bar_w > 0. { bar_w - crate::screens::combo::OTHERS } else { 0. };
        let tools_strip = crate::screens::tools::strip(cx, light, thread, &self.local);
        let mut rail: Vec<AnyElement> = Vec::new();
        // held Ctrl: each rail button's bare key in its corner (the old rail's keycaps), lit on the view on show
        let keycaps = self.screens.held == crate::keys::Held::Threads;
        let keycap = move |k: &str, lit: bool| (keycaps && !k.is_empty()).then(|| crate::screens::hints::cap(light, &k.to_uppercase(), lit).absolute().top(px(3.)).right(px(3.)));
        crate::screens::viewer::follow(&self.tx, thread);
        let active_kind = if thread["diff"].is_object() { String::new() } else { crate::screens::viewer::active_kind(&thread["viewer"], self.screens.rail_pending.as_deref()) };
        for (l, i) in [("Terminal", "terminal"), ("Commands", "search"), ("", ""), ("Board", "circuit-board"), ("Schematic", "cpu"), ("3D", "box"), ("Split", "layout-panel-left"), ("Mech", "layers"), ("", ""), ("Files", "folder"), ("Diff", "diff"), ("Review", "circle-check"), ("Browser", "globe")].iter() {
            if l.is_empty() { rail.push(div().h(px(12.)).into_any_element()); continue; }
            // the viewer's kinds are the hub's view-kind values; Diff is its own action
            let act: Option<(&'static str, &'static str)> = match *l { "Board" => Some(("view-kind", "board")), "Schematic" => Some(("view-kind", "schematic")), "3D" => Some(("view-kind", "3d")), "Split" => Some(("view-kind", "split")), "Mech" => Some(("view-kind", "mech")), "Files" => Some(("view-kind", "files")), "Review" => Some(("view-kind", "review")), "Browser" => Some(("view-kind", "browser")), "Diff" => Some(("diff", "")), _ => None };
            match act {
                Some((a, v)) => {
                    let active = a == "view-kind" && v == active_kind;
                    ctl.push(CtlEntry { id: format!("rail:{l}"), label: l.to_string(), ctl: if a == "view-kind" { Ctl::Rail(v.into()) } else { Ctl::Node(a.into(), v.into()) } });
                    rail.push(with_rect(div().id(SharedString::from(format!("rail-{l}"))), || format!("rail:{l}")).cursor_pointer().on_click(cx.listener(move |this, _, _, cx| this.rail_press(a, v, cx))).tooltip({ let letter = crate::keys::rail_letter(if a == "diff" { "diff" } else { v }).to_uppercase(); let chord = match (a, v) { ("diff", _) => crate::keys::hint(&self.local, "diff"), (_, "files") => crate::keys::hint(&self.local, "files"), _ => String::new() }; let key = [letter, chord].into_iter().filter(|k| !k.is_empty()).collect::<Vec<_>>().join(" or "); crate::keys::tip(l, &key, light) })
                        .relative().flex().flex_col().items_center().h(px(48.)).justify_center().gap_1().text_size(px(10.)).when(active, |d| d.bg(c("raise")).border_l_2().border_color(c("accent"))).text_color(c(if active { "text" } else { "dim" })).hover(|s| s.text_color(c("text"))).child(ic(light, i, 18., if active { "accent" } else { "dim" })).child(*l)
                        .children(keycap(crate::keys::rail_letter(if a == "diff" { "diff" } else { v }), active)).into_any_element());
                }
                None if *l == "Terminal" => { let on = self.screens.term.open; rail.push(with_rect(div().id("rail-Terminal").cursor_pointer().on_click(cx.listener(|this, _, window, cx| this.term_toggle(window, cx))).tooltip(crate::keys::tip("Terminal", &crate::keys::hint(&self.local, "term"), light)), || "rail:Terminal".to_string()).relative().flex().flex_col().items_center().h(px(48.)).justify_center().gap_1().text_size(px(10.)).when(on, |d| d.bg(c("raise")).border_l_2().border_color(c("accent"))).text_color(c(if on { "text" } else { "dim" })).hover(|s| s.bg(c("raise"))).child(ic(light, i, 18., if on { "accent" } else { "dim" })).child(*l).children(keycap("`", on)).into_any_element()) }
                None if *l == "Commands" => rail.push(with_rect(div().id("rail-Commands").cursor_pointer().on_click(cx.listener(|this, _, _, cx| this.palette_open(cx))).tooltip(crate::keys::tip("Commands", &crate::keys::hint(&self.local, "palette"), light)), || "rail:Commands".to_string()).flex().flex_col().items_center().h(px(48.)).justify_center().gap_1().text_size(px(10.)).text_color(c("dim")).hover(|s| s.bg(c("raise"))).child(ic(light, i, 18., "dim")).child(*l).into_any_element()),
                None => rail.push(with_rect(div().opacity(0.45), || format!("rail:{l}")).flex().flex_col().items_center().h(px(48.)).justify_center().gap_1().text_size(px(10.)).text_color(c("dim")).child(ic(light, i, 18., "dim")).child(*l).into_any_element()),
            }
        }
        // the open asks' buttons are controls too (the harness answers them by index: ask:<i>:<j>)
        for (i, a) in crate::screens::asks::parse(&thread["asks"]).into_iter().enumerate() {
            for (j, b) in a.buttons.iter().enumerate() { ctl.push(CtlEntry { id: format!("ask:{i}:{j}"), label: b.label.clone(), ctl: Ctl::Node("answer".into(), b.value.clone()) }); }
            if let Some(s) = &a.skip { ctl.push(CtlEntry { id: format!("ask:{i}:skip"), label: s.label.clone(), ctl: Ctl::Node("answer".into(), s.value.clone()) }); }
            // a "Backplane needs" card's typed box and its Continue (the harness clicks the box and types, as a person does)
            if a.needs && !a.pre.is_empty() { ctl.push(CtlEntry { id: format!("ask:{i}:box"), label: a.placeholder.clone(), ctl: Ctl::None }); ctl.push(CtlEntry { id: format!("ask:{i}:go"), label: a.go.clone(), ctl: Ctl::None }); }
        }
        // the header's tools (Buy it is tool:fab-buy) and the Order card's Buy now are controls too
        for t in crate::screens::tools::tools(thread) { ctl.push(CtlEntry { id: format!("tool:{}", t.action), label: t.label.clone(), ctl: Ctl::Node(t.action.clone(), t.value.clone()) }); }
        if let Some(o) = crate::screens::order::parse(&thread["order"]).filter(crate::screens::order::payable) { ctl.push(CtlEntry { id: "ui:Buy now".into(), label: if o.label.is_empty() { "Buy now".into() } else { o.label }, ctl: Ctl::None }); }
        // the Mechanical page's parts are controls too (the harness presses them by index)
        for (i, p) in crate::screens::viewer::mech_parts(&thread["viewer"]).into_iter().enumerate() { ctl.push(CtlEntry { id: format!("mech-part-{i}"), label: p.label, ctl: Ctl::Node("mech-part".into(), p.value) }); }
        // the viewer bar's Open in KiCad, and the Subagents panel's row buttons (Release, Cancel, Stop: `<act>-<value>`, as the rows name them)
        if crate::screens::viewer::is_open(thread) && crate::screens::viewer::has_design(&thread["viewer"]) { ctl.push(CtlEntry { id: "ui:Open in KiCad".into(), label: "Open in KiCad".into(), ctl: Ctl::Node("kicad-open".into(), String::new()) }); }
        // the Diff page's Commit, PR and Revert (and the Revert question's two buttons), by the ids the page draws
        for (id, label, a, v) in crate::screens::viewer::diff_controls(&thread["diff"]) { ctl.push(CtlEntry { id: id.into(), label: label.into(), ctl: Ctl::Node(a.into(), v.into()) }); }
        for r in thread["subs"]["rows"].as_array().into_iter().flatten().filter(|r| !string(r, "act").is_empty()) { ctl.push(CtlEntry { id: format!("{}-{}", string(r, "act"), string(r, "value")), label: string(r, "label"), ctl: Ctl::Node(string(r, "act"), string(r, "value")) }); }
        // controls with no handler yet are registered too, so the harness finds them dead instead of missing
        for (id, label, c) in crate::screens::tools::controls(&self.data) { ctl.push(CtlEntry { id, label, ctl: c }); }
        // the bot form, a bot's tabs and its Webhooks page are controls too (screens::bots::controls)
        for (id, label, c) in crate::screens::bots::controls(&self.data) { ctl.push(CtlEntry { id, label, ctl: c }); }
        if has_thread { ctl.push(CtlEntry { id: "ui:Thread menu".into(), label: "Thread actions".into(), ctl: Ctl::ThreadMenu }); }
        ctl.push(CtlEntry { id: "ui:Settings".into(), label: "Settings".into(), ctl: Ctl::Node("flag".into(), "settings".into()) });
        if self.data["settings"].is_object() { for (page, title) in &self.screens.settings_ui.nav { ctl.push(CtlEntry { id: format!("set-nav:{page}"), label: title.clone(), ctl: Ctl::SettingsPage(page.clone()) }); } }
        // Settings > CAD's Install KiCad buttons (Install KiCad, Install, Cancel, Try again): `set:<action>`
        for b in crate::screens::settings::parse(&self.data["settings"]).into_iter().flat_map(|r| r.buttons).filter(|b| b.action.starts_with("kicad-install")) { ctl.push(CtlEntry { id: format!("set:{}", b.action), label: b.label, ctl: Ctl::Node(b.action, b.value) }); }
        if let Some(p) = self.data["projects"].as_array().and_then(|p| p.first()) { let id = string(p, "id"); let id = id.rsplit_once('|').map_or(id.as_str(), |(_, r)| r).to_string(); ctl.push(CtlEntry { id: "ui:New thread".into(), label: "New thread".into(), ctl: Ctl::Node("new-thread".into(), id) }); }
        ctl.push(CtlEntry { id: "send".into(), label: "Send".into(), ctl: Ctl::Send });
        ctl.push(CtlEntry { id: "rail:Terminal".into(), label: "Terminal".into(), ctl: Ctl::None });
        ctl.push(CtlEntry { id: "rail:Commands".into(), label: "Commands".into(), ctl: Ctl::OpenPalette });
        ctl.push(CtlEntry { id: "ui:Context".into(), label: "Context".into(), ctl: Ctl::ToggleContext });
        ctl.push(CtlEntry { id: "ui:Jump or run a command".into(), label: "Jump or run a command".into(), ctl: Ctl::OpenPalette });
        for l in ["Account"] { ctl.push(CtlEntry { id: format!("ui:{l}"), label: l.into(), ctl: Ctl::None }); }
        ctl.push(CtlEntry { id: "ui:Dictate".into(), label: "Dictate".into(), ctl: Ctl::Dictate });
        crate::screens::improvements::controls(self, &mut ctl);
        for (id, label, c) in crate::screens::projects::controls(&self.data) { ctl.push(CtlEntry { id, label, ctl: c }); }
        ctl.push(CtlEntry { id: "ui:Search".into(), label: "Search".into(), ctl: Ctl::FocusSearch });
        ctl.push(CtlEntry { id: "ui:Fold sidebar".into(), label: "Fold sidebar".into(), ctl: Ctl::ToggleSidebar });
        ctl.push(CtlEntry { id: "ui:Attach".into(), label: "Attach".into(), ctl: Ctl::PickFiles });
        ctl.push(CtlEntry { id: "ui:Draw".into(), label: "Draw".into(), ctl: Ctl::Pad });
        if !self.sidebar_open { ctl.push(CtlEntry { id: "ui:Unfold sidebar".into(), label: "Unfold sidebar".into(), ctl: Ctl::ToggleSidebar }); }
        let connected = self.status == "Connected";
        if !self.screens.img_poll && self.screens.images.loading() { self.screens.img_poll = true; crate::screens::timeline::poll(cx); }

        let el = crate::keys::attach(div().size_full().flex().flex_col().bg(c("bg")),self.picker_keys(),cx)
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| this.attach_event(&crate::screens::composer::ComposerEvent::Files(paths.paths().to_vec()), cx))).text_color(c("text")).text_size(px(13. * self.local.text_pct as f32 / 100.)).font_family(self.local.font.clone()).line_height(px(20. * self.local.text_pct as f32 / 100.))
            .relative().child(div().absolute().size_0().track_focus(&self.screens.root_focus)).on_action(cx.listener(|this, _: &crate::OpenPaletteKey, _, cx| if !this.palette_close(cx) { this.palette_open(cx) })).on_action(cx.listener(|this, _: &crate::screens::improvements::Toggle, _, cx| crate::screens::improvements::toggle(this, cx))).on_action(cx.listener(|this, _: &crate::ToggleSidebarKey, _, cx| this.sidebar_toggle(cx))).on_action(cx.listener(|this, _: &crate::FocusSearchKey, _, cx| this.focus_search(cx))).on_action(cx.listener(Self::send)).on_action(cx.listener(Self::close_popup)).on_action(cx.listener(Self::copy_selection))
            // the dev build strip, only while a build is being made or waits to relaunch
            .children(self.screens.devbar.clone().map(|b| crate::screens::devbar::strip(cx, &b, light, self.screens.devbar_confirm.is_some())))
            .children({ let s = std::mem::take(&mut self.screens.hubbar); let e = crate::screens::hubbar::strip(cx, &s, light); self.screens.hubbar = s; e })
            // the GitHub star suggestion, while the hub says it is out
            .children(crate::screens::star::strip(cx, self.screens.star.clone().as_ref(), light))
            // a window on another computer's hub: a thin accent line along the top the whole time, and "on desktop" beside the name
            .when(self.screens.elsewhere.is_some(), |d| d.child(div().id("elsewhere-strip").h(px(2.)).flex_shrink_0().bg(c("accent"))))
            // top bar
            .child(div().h(px(36.)).flex_shrink_0().relative().child(art::probe("topbar")).children(crate::perf::mark("topbar")).flex().items_center().justify_between().px_3().border_b_1().border_color(c("line")).bg(c("bg"))
                .child(div().flex().items_center().gap_2()
                    .when(!self.sidebar_open, |d| d.child(with_rect(div().id("unfold-sidebar"), || "ui:Unfold sidebar".to_string()).cursor_pointer().tooltip(crate::keys::tip("Unfold the sidebar", &crate::keys::hint(&self.local, "sidebar"), light)).child(ic(light, "chevron-right", 16., "dim")).on_click(cx.listener(|this, _, _, cx| { if !this.sidebar_open { this.sidebar_toggle(cx); } }))))
                    .child(div().relative().size(px(18.)).child(art::logo(light, 18., "logo"))
                        .child(div().absolute().right(px(-2.)).bottom(px(-1.)).size(px(6.)).bg(if connected { c("ok") } else { c("bad") })))
                    .child(div().font_weight(FontWeight::BOLD).child("Backplane"))
                    .children(self.screens.elsewhere.clone().map(|(short, full)| div().id("elsewhere").flex().items_center().gap(px(3.)).px_1().border_1().border_color(c("accent")).text_size(px(11.)).text_color(c("accent"))
                        .tooltip(crate::keys::tip(&format!("This window is on {full}, not this computer"), "", light)).child(ic(light, "link", 11., "accent")).child(format!("on {short}"))))
                    .child(div().text_color(c("faint")).child("/")).child(div().text_color(c("dim")).child(if has_thread { "Project".to_string() } else { "Projects".to_string() }))
                    // another machine's thread: "Project · on desktop" (this machine's looks as always)
                    .when(has_thread, |d| d.children(crate::screens::badge::place(light, &thread["mark"], "", "crumb").map(|p| div().flex().items_center().gap_2().child(div().text_color(c("faint")).child("·")).child(p))))
                    // a buy thread's title is marked an Order with the cart, as 0.14's was (the hub sends its Order card only for a buy thread)
                    .when(has_thread && thread["order"].is_object(), |d| d.child(div().text_color(c("faint")).child("/")).child(div().id("order-mark").flex().items_center().gap(px(3.)).text_size(px(11.)).text_color(c("accent"))
                        .tooltip(crate::keys::tip("This thread orders the project's parts", "", light)).child(ic(light, "shopping-cart", 12., "accent")).child("Order")))
                    .when(has_thread, |d| d.child(div().text_color(c("faint")).child("/")).child(div().id("thread-title").font_weight(FontWeight::BOLD).child(string(thread, "title"))
                        // the name is not a button (Rename is in its menu): a right click on it, or the ▾ beside it, opens the thread's menu
                        .on_mouse_down(MouseButton::Right, cx.listener(|this, e: &MouseDownEvent, _, cx| { let items = crate::screens::ctxmenu::title_items(&this.data); if !items.is_empty() { this.ctx_open(e.position, items, cx); } })))
                        .child(with_rect(div().id("thread-menu"), || "ui:Thread menu".to_string()).px_1().h_full().flex().items_center().cursor_pointer().hover(|s| s.bg(c("raise")))
                            // the menu drops from under the ▾
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, _, cx| { cx.stop_propagation(); let items = crate::screens::ctxmenu::title_items(&this.data); if !items.is_empty() { this.ctx_open(point(e.position.x - px(8.), e.position.y + px(14.)), items, cx); } }))
                            .child(ic(light, "chevron-down", 14., "dim")))))
                .child(div().flex().items_center().gap_3()
                    .child(with_rect(div().id("palette-bar").cursor_pointer().on_click(cx.listener(|this, _, _, cx| this.palette_open(cx))).tooltip(crate::keys::tip("Jump or run a command", &crate::keys::hint(&self.local, "palette"), light)), || "ui:Jump or run a command".to_string()).w(px(260.)).px_2().flex().items_center().gap_2().border_1().border_color(c("line")).text_size(px(12.)).text_color(c("faint")).child(ic(light, "chevron-right", 13., "faint")).child("Jump or run a command")
                        // held Ctrl: its key at the end of the bar (0.14's "Ctrl K"), in room the bar always has
                        .when(self.screens.held == crate::keys::Held::Threads, |d| d.child(div().flex_1()).child(div().flex().gap(px(2.)).children(crate::keys::caps(&crate::keys::hint(&self.local, "palette")).iter().map(|k| crate::screens::hints::cap(light, k, false))))))
                    .child(with_rect(div().id("improvements-btn"), || "ui:Improvements".to_string()).cursor_pointer().on_click(cx.listener(|this, _, _, cx| crate::screens::improvements::toggle(this, cx))).px_2().flex().items_center().gap_2().border_1().border_color(c("line")).text_size(px(12.)).text_color(c("dim")).child(ic(light, "message-square", 15., "dim")).child("Improvements"))
                    .child(with_rect(div().id("search-btn"), || "ui:Search".to_string()).cursor_pointer().tooltip(crate::keys::tip("Search threads", &crate::keys::hint(&self.local, "search"), light)).on_click(cx.listener(|this, _, _, cx| this.focus_search(cx))).child(ic(light, "search", 15., "dim"))).child(with_rect(div().id("gear"), || "ui:Settings".to_string()).cursor_pointer().on_click(cx.listener(|this, _, _, cx| { this.open_settings(cx); })).child(ic(light, "settings", 15., "dim")))))
            .children(crate::screens::strip::view(light, &self.data))
            .child(div().flex_1().min_h_0().flex()
                // sidebar
                .child(div().w(px(if self.sidebar_open { self.local.sidebar_px as f32 } else { 0. })).overflow_hidden().h_full().flex_shrink_0().relative().child(art::probe("sidebar")).children(crate::perf::mark("sidebar")).bg(c("bg")).border_r_1().border_color(c("line")).flex().flex_col()
                    .child(crate::screens::hubpick::picker(self, cx, light, &mut ctl))
                    .child(div().flex().items_center().gap_2().p_2().child(with_rect(div().id("fold-sidebar"), || "ui:Fold sidebar".to_string()).cursor_pointer().tooltip(crate::keys::tip("Fold the sidebar", &crate::keys::hint(&self.local, "sidebar"), light)).child(ic(light, "chevron-left", 16., "dim")).on_click(cx.listener(|this, _, _, cx| { if this.sidebar_open { this.sidebar_toggle(cx); } }))).child(crate::screens::projects::search_box(light, &self.screens.projects)).child(with_rect(div().id("add-project"), || "ui:Add project".to_string()).cursor_pointer().on_click(cx.listener(|this, _, _, cx| { let _ = this.tx.try_send(crate::screens::projects::open_cmd()); cx.notify(); })).child(ic(light, "plus", 16., "dim"))))
                    .child(self.screens.parts.el(Backplane::sidebar_part, StyleRefinement::default().flex_1().min_h_0(), cx))
                    .child(div().px_3().py_1().border_t_1().border_color(c("line")).text_size(px(11.)).text_color(c("dim")).child(self.status.clone()))
                    // held Ctrl: the short legend over the sidebar's foot (0.14's keys-legend)
                    .when(self.screens.held == crate::keys::Held::Threads, |d| d.child(crate::screens::hints::legend(cx, light, &self.local))))
                // main
                .child(div().flex_1().min_w(px(crate::screens::viewer::THREAD_MIN)).h_full().relative().child(art::probe("main")).children(crate::perf::mark("main")).flex().flex_col()
                    .children(crate::screens::bots::bar(cx, light, &self.data["bot"]))
                    .when(has_thread, |d| d.child(div().id("thread-head").h(px(32.)).flex_shrink_0().flex().items_center().border_b_1().border_color(c("line"))
                        .on_mouse_down(MouseButton::Right, { let items = crate::screens::tools::context_items(thread); cx.listener(move |this, e: &MouseDownEvent, _, cx| { if !items.is_empty() { this.ctx_open(e.position, items.clone(), cx); } }) })
                        .child(div().px_3().flex().gap_2().text_size(px(11.)).child(div().text_color(c("faint")).child("STATE")).child(div().size(px(6.)).mt(px(7.)).bg(c("ok"))).child(string(thread, "phase").split(':').next().unwrap_or("").to_lowercase()))
                        .children(crate::screens::panels::cells(cx, light, &self.screens.panels, thread))
                        .children(tools_strip)))
                    .children(crate::screens::panels::dock(cx, light, &self.screens.fields, &self.screens.panels, thread))
                    .when(has_home, |d| d.child(div().id("home").overflow_y_scroll().flex_1().flex().justify_center().child(div().w_full().max_w(px(930.)).p_4().flex().flex_col().gap_3().children(home_els))))
                    // a list: only the rows on screen are drawn, however much of the thread is held, and it keeps the reader's place as rows land above them
                    .when(!has_home, |d| d.child(self.screens.parts.el(Backplane::timeline_part, StyleRefinement::default().flex_1().min_h_0(), cx)))
                    .when(!self.error.is_empty(), |d| d.child(div().px_5().py_2().text_color(c("bad")).child(self.error.clone())))
                    .when(!self.note.is_empty(), |d| d.child(div().px_5().py_2().text_color(c("dim")).child(self.note.clone())))
                    .child(div().flex_shrink_0().relative().child(art::probe("composer")).children(crate::perf::mark("composer")).border_t_1().border_color(c("line")).flex().justify_center().child(div().w_full().max_w(px(930.)).px_4().py_3().flex().flex_col().gap_2()
                        .when(show_popup, |d| d.child(div().relative().border_1().border_color(c("line")).bg(c("panel")).child(popup).children(popup_bar)))
                        .children(self.screens.ctx_open.then(|| div().id("ctx-panel").max_h(px(340.)).overflow_y_scroll().px_3().py_2().border_1().border_color(c("line")).bg(c("panel")).text_size(px(12.)).flex().flex_col().gap_1()
                            .child(div().font_weight(FontWeight::BOLD).child("Context"))
                            .child(div().text_color(c("dim")).child(if ctx.is_empty() { "Nothing reported by the provider yet.".to_string() } else { format!("Held now: {}", ctx.trim_start_matches("ctx ")) }))
                            .child(div().text_color(c("faint")).child(string(&thread["picker"], "label")))
                            .children(self.data["stats"].as_array().into_iter().flatten().map(|sec| div().flex().flex_col().gap_1().mt_1()
                                .child(div().font_weight(FontWeight::BOLD).text_color(c("dim")).child(string(sec, "title")))
                                .children(sec["rows"].as_array().into_iter().flatten().map(|r| div().flex().gap_2().child(div().w(px(110.)).text_color(c("dim")).child(string(r, "label"))).child(div().flex_1().min_w_0().child(string(r, "value")))).collect::<Vec<_>>())).collect::<Vec<_>>())))
                        .children(crate::screens::order::view(cx, light, &thread["order"]))
                        .children(crate::screens::asks::view(cx, light, &self.screens.ask_inputs, &thread["asks"]))
                        .children(self.screens.voice.views.get(&self.selected).cloned().and_then(|v| crate::screens::voice::view(cx, light, &v, self.screens.voice.level, self.screens.voice.fix.clone().as_ref())))
                        .children(crate::screens::sidebox::btw(cx, light, &thread["btw"]))
                        .children(crate::screens::sidebox::secret(cx, light, &self.screens.fields, &thread["secretBox"]))
                        .children(reply_chip(cx, light, &thread["replying"]))
                        .children(crate::screens::attach::chips(cx, light, &self.screens.attacher.previews, thread))
                        .child(div().id("composer-box").border_1().border_color(c("accent")).bg(c("view")).px_3().py_2().min_h(px(60.)).cursor(CursorStyle::IBeam).on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| { this.input.focus_handle(cx).focus(window); })).child(self.input.clone()))
                        // another machine's thread: where an agent sent from here runs
                        .children(crate::screens::badge::runs_text(&thread["mark"]).map(|r| div().id("runs-on").flex().items_center().gap(px(3.)).text_size(px(11.)).text_color(c("dim")).child(ic(light, "link", 11., "dim")).child(r)))
                        .child(div().relative().flex().gap_2().items_center().w_full().min_w_0()
                            // the row as drawn: its width sizes the chip's words; the menu floats from where it is (another frame when it moved under an open menu)
                            .child(canvas({ let (me, cell, at, open) = (cx.entity().downgrade(), self.screens.acct.bar_w.clone(), self.screens.pick.row.clone(), self.screens.picker_open); move |b, _, cx| {
                                let moved = at.get() != b; at.set(b);
                                let w = f32::from(b.size.width); let resized = (cell.get() - w).abs() >= 1.;
                                if resized { cell.set(w); }
                                if resized || (moved && open) { if let Some(this) = me.upgrade() { cx.defer(move |cx| this.update(cx, |_, cx| cx.notify())) } }
                            } }, |_, _, _, _| {}).absolute().size_full())
                            // the chip's menu (screens/combo.rs): out of the row's flow, floating over the thread from the row; hidden while something covers the page
                            .children((!self.chip_menu_covered()).then(|| crate::screens::combo::menu(cx, light, &thread, self, bar_w, (f32::from(window.viewport_size().width), f32::from(window.viewport_size().height)))).flatten())
                            .child(crate::screens::bar::fit(with_rect(crate::screens::combo::chip(cx, light, thread, combo_avail, (&crate::keys::hint(&self.local, "models"), &crate::keys::hint(&self.local, "account")), &self.screens.pick.chip), || "ui:Model".to_string()))
                                // held Ctrl: the menu's key over the chip's end (0.14's "Ctrl M"), so the chip keeps its width
                                .when(self.screens.held == crate::keys::Held::Threads, |d| d.relative().child(div().absolute().right(px(4.)).top_0().bottom_0().flex().items_center().gap(px(2.)).children(crate::keys::caps(&crate::keys::hint(&self.local, "models")).iter().map(|k| crate::screens::hints::cap(light, k, false))))))
                            .child(crate::screens::bar::fit(with_rect(chip(light, String::new()).id("draw"), || "ui:Draw".to_string())).flex_shrink_0().cursor_pointer().tooltip(crate::keys::tip("Draw a sketch and attach it as an image", "", light)).on_click(cx.listener(|this, _, window, cx| this.pad_open(window, cx))).child(ic(light, "square-pen", 16., "dim")))
                            .child(crate::screens::bar::fit(with_rect(chip(light, String::new()).id("ctx-chip"), || "ui:Context".to_string())).min_w_0().overflow_hidden().cursor_pointer().tooltip(crate::keys::tip("Tokens, cost and context: the thread's numbers", "", light)).on_click(cx.listener(|this, _, _, cx| { this.screens.ctx_open = !this.screens.ctx_open; cx.notify(); })).flex().items_center().gap_2().child(ic(light, "gauge", 14., "dim"))
                                // the hub's words for the thread's tokens, cost and context (0.14's token button), else the context alone
                                .child(div().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(Some(string(&self.data, "tokChip")).filter(|t| !t.is_empty()).unwrap_or_else(|| ctx.clone()))))
                            .child(crate::screens::bar::fit(with_rect(chip(light, String::new()).id("attach"), || "ui:Attach".to_string())).flex_shrink_0().cursor_pointer().on_click(cx.listener(|this, _, _, cx| this.pick_files(cx))).child(ic(light, "paperclip", 16., "dim")))
                            .child(crate::screens::bar::fit(with_rect(chip(light, String::new()).id("dictate"), || "ui:Dictate".to_string())).flex_shrink_0().cursor_pointer().on_click(cx.listener(|this, _, _, cx| this.voice_press(cx))).child(ic(light, "mic", 16., if dictating { "accent" } else { "dim" })))
                            .child(crate::screens::bar::fit(with_rect(div().id("send"), || "send".to_string())).flex_shrink_0().px_4().py_1().bg(c("accent")).text_color(c("on-accent")).cursor_pointer().child(crate::screens::sidebox::send_label(thread)).on_click(cx.listener(|this, _, window, cx| this.send(&Send, window, cx)))))))
                    .children(crate::screens::bots::page(cx, light, &self.screens.fields, &self.data["bot"], self.screens.browser.as_ref().map(|(_, b)| b)))
                    .children(crate::screens::tools::questions(cx, light, &self.screens.fields, &self.data, self.sidebar_open))
                    .children(crate::screens::projects::view(cx, light, &self.screens.projects, &self.data))
                    .children(crate::screens::bots::forms(cx, light, &self.screens.fields, &self.data))
                    .children(crate::screens::palette::view(cx, light, &self.data, &self.screens.palette, &self.local, &self.selected))
                    .children(crate::screens::images::lightbox(cx, self.screens.lightbox.as_ref()))
                )
                .children(crate::perf::mark("viewer"))
                .children(crate::screens::viewer::is_open(thread).then(|| self.screens.parts.el(Backplane::viewer_part, StyleRefinement::default().flex_1().min_h_0(), cx)).and_then(|page| crate::screens::viewer::pane(cx, light, thread, page)))
                // right rail
                .child(div().w(px(72.)).h_full().flex_shrink_0().relative().child(art::probe("rail")).children(crate::perf::mark("rail")).border_l_1().border_color(c("line")).flex().flex_col().py_2().children(rail)))
            .children(crate::perf::mark("over"))
            // Settings over the whole window: its dialog is sized from the window (settings::fit), so it must not sit in the thread column
            .children(crate::screens::settings::view(cx, light, &self.screens.fields, &self.data["settings"], &self.screens.settings_focus, &self.screens.settings_ui, &self.local, &self.screens.acct, crate::screens::settings::fit(f32::from(window.viewport_size().width), f32::from(window.viewport_size().height)),&self.hubs))
            .children(crate::screens::improvements::view(cx, light, &self.screens.imp))
            .children(crate::screens::improvements::indicator(cx, light, &self.screens.imp))
            .children({ let title = crate::screens::badge::from_machine(&self.data["thread"]["mark"], &string(&self.data["thread"], "title")); crate::screens::term::view(cx, window, light, &self.local, &title, &self.screens.term) })
            .children(crate::screens::padview::view(cx, window, light, &self.local, &mut self.screens.pad, &mut ctl))
            .children(crate::screens::onboard::view(cx, light, &self.screens.fields, &self.data["tour"], &self.screens.acct, &self.screens.setup_focus, &self.screens.setup.scroll, f32::from(window.viewport_size().width)))
            // a 0.14 home coming across: over setup and everything else of the window
            .children(crate::screens::migrate::view(cx, light, &self.data["migrate"]))
            // the right-click menu of a sidebar row, over everything
            .children(self.screens.ctx.clone().map(|m| crate::screens::ctxmenu::render(cx, light, &m)))
            .children(crate::perf::on().then(|| crate::perf::overlay(light, &crate::keys::hint(&self.local, "perf"))))
            .into_any_element();
        self.controls = ctl;
        // the selection's ends as ordinals of the blocks known now (the rows' part does the same when it draws)
        { let rows = &self.data["thread"]["entries"]; self.screens.reg.borrow_mut().resolve_by(|id| rows.as_array().and_then(|r| r.iter().position(|e| e["id"].as_str() == Some(id)))); }
        if crate::perf::on() { crate::perf::built(); }
        self.render_us = t_render.elapsed().as_micros() as u64;
        el
    }
}

/// A thread row being dragged (the sidebar's reorder: dropped on another row of its shelf), and a project header being dragged.
#[derive(Clone)] pub struct DragRow { pub id: String, pub label: String }
#[derive(Clone)] pub struct DragProj { pub id: String, pub label: String }
/// What follows the pointer while dragging: the label in a small box.
pub struct Ghost(pub String);
impl Render for Ghost { fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement { div().px_3().py_1().bg(gpui::rgb(0x2a2f3a)).border_1().border_color(gpui::rgb(0x4a5060)).text_color(gpui::rgb(0xe6e8ee)).text_size(px(12.)).child(self.0.clone()) } }
