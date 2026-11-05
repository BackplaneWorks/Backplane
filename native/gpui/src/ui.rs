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
            row = row.child(div().id(SharedString::from(format!("{id}-{key}"))).cursor_pointer().text_color(c("accent")).hover(|s| s.bg(c("raise"))).px_1()
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
            div().id(SharedString::from(format!("n{path}"))).px_2().py_1().border_1().border_color(c("line")).bg(c("raise")).text_size(px(12.)).cursor_pointer().hover(|s| s.bg(c("panel")))
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

fn ic(light: bool, name: &'static str, size: f32, tok: &str) -> AnyElement { art::icon(name, size, theme::color(light, tok)) }
fn chip(light: bool, label: String) -> Div {
    let c = |name: &str| theme::color(light, name);
    div().px_3().py_1().border_1().border_color(c("line")).bg(c("panel")).text_size(px(12.)).whitespace_nowrap().child(label)
}

impl Render for Backplane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t_render = std::time::Instant::now();
        if self.want_focus { self.want_focus = false; self.input.focus_handle(cx).focus(window); }
        self.frame_marks(window, cx);
        let light = self.theme_override.unwrap_or(self.data["theme"] == "light");
        if self.swatches { return crate::swatch::view(light).into_any_element(); }
        if self.gallery { return crate::swatch::gallery(light).into_any_element(); }
        if self.fonts { return crate::swatch::fonts(light).into_any_element(); }
        let c = |name: &str| theme::color(light, name);
        self.screens.reg.borrow_mut().begin(); self.screens.images.frame();
        let reg = self.screens.reg.clone();
        if self.input.read(cx).light != light { self.input.update(cx, |i, _| i.light = light); for i in self.screens.ask_inputs.values() { i.update(cx, |i, _| i.light = light); } }
        // bots: (name, mood, note, look seed) copied out so the cat cache can be filled
        let bot_data: Vec<(String, String, String, u64)> = self.data["bots"].as_array().into_iter().flatten()
            .map(|b| (string(b, "name"), string(b, "mood"), string(b, "note"), b["look"].as_u64().unwrap_or(0))).collect();
        let bot_rigs: Vec<_> = bot_data.iter().map(|(_, mood, _, look)| self.cats.rig(*look, mood)).collect();
        let started = self.started;
        let thread = &self.data["thread"];
        let has_thread = thread.is_object();
        // sidebar rows
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut ctl: Vec<CtlEntry> = Vec::new(); // the controls drawn this frame (the e2e harness's inventory)
        for project in self.data["projects"].as_array().into_iter().flatten() {
            let pid = string(project, "id"); let pid = pid.rsplit_once('|').map_or(pid.as_str(), |(_, r)| r).to_string();
            ctl.push(CtlEntry { id: format!("project:{pid}"), label: string(project, "title"), ctl: Ctl::Home(pid.clone()) });
            rows.push(div().id(SharedString::from(format!("proj-{pid}"))).flex().items_center().gap_2().px_3().pt_3().pb_1().text_size(px(11.)).text_color(c("dim")).cursor_pointer().hover(|s| s.text_color(c("text")))
                .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("select".into(), format!("project-home-{pid}"))); cx.notify(); }))
                .child(ic(light, "chevron-down", 14., "dim")).child(ic(light, "folder", 14., "ok")).child(div().flex_1().child(string(project, "title").to_uppercase())).child(div().opacity(0.45).child(ic(light, "plus", 16., "dim"))).into_any_element());
            for t in project["threads"].as_array().into_iter().flatten().chain(project["settled"].as_array().into_iter().flatten()) {
                let id = string(t, "id");
                let raw = id.rsplit_once('|').map_or(id.as_str(), |(_, i)| i);
                let selected = raw == self.selected;
                let dot = match t["state"].as_str().unwrap_or("") { "running" => c("accent"), "failed" => c("bad"), _ => c("ok") };
                ctl.push(CtlEntry { id: format!("thread:{raw}"), label: string(t, "title"), ctl: Ctl::Select(id.clone()) });
                rows.push(div().id(SharedString::from(format!("thread-{id}"))).flex().items_center().gap_2().pl(px(22.)).pr_3().h(px(32.))
                    .border_l_2().border_color(if selected { c("ok") } else { c("panel") })
                    .bg(if selected { c("raise") } else { c("panel") }).hover(|s| s.bg(c("raise"))).cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| this.select(id.clone(), window, cx)))
                    .child(div().size(px(5.)).bg(dot)).child(div().flex_1().overflow_hidden().whitespace_nowrap().child(string(t, "title")))
                    .child(div().text_size(px(11.)).text_color(c("faint")).child(string(t, "ago"))).into_any_element());
            }
        }
        for page in self.data["sidebarPages"].as_array().into_iter().flatten() {
            for key in ["projects", "active", "legacy"] { if let Some(row) = pager_row(cx, format!("pg-{key}"), &page[key], light, &mut ctl) { rows.push(row); } }
        }
        for name in ["BOTS", "ROOMS"] {
            rows.push(div().flex().justify_between().px_3().pt_4().text_size(px(11.)).text_color(c("dim")).child(name).child(div().opacity(0.45).child(ic(light, "plus", 16., "dim"))).into_any_element());
            if name == "BOTS" {
                for ((bname, mood, note, _), rig) in bot_data.iter().zip(bot_rigs.iter()) {
                    let note_color = match mood.as_str() { "wait" | "error" => c("bad"), "work" | "talk" => c("accent"), _ => c("faint") };
                    rows.push(div().flex().items_center().gap_2().px_3().py(px(3.)).child(art::cat(rig.clone(), mood, 28., started, if art::rects_on() { format!("cat:{bname}") } else { String::new() }))
                        .child(div().flex().flex_col().min_w_0().line_height(px(18.)).child(div().overflow_hidden().whitespace_nowrap().child(bname.clone()))
                            .child(div().text_size(px(11.)).text_color(note_color).child(note.clone()))).into_any_element());
                }
            }
        }
        // timeline
        let mut entries: Vec<AnyElement> = Vec::new();
        for (ei, e) in thread["entries"].as_array().into_iter().flatten().enumerate() {
            { let mut r = reg.borrow_mut(); r.entry = ei; r.entry_id = string(e, "id"); }
            let kind = e["kind"].as_str().unwrap_or("");
            let id = string(e, "id");
            let text = string(e, "text");
            let stamp = string(e, "stamp");
            match kind {
                "user" => entries.push(div().id(SharedString::from(format!("e-{id}"))).w_full().flex().justify_end().py_2().child(
                    div().max_w(px(640.)).px_3().py_2().bg(c("raise")).border_1().border_color(c("line")).flex().flex_col().gap_1()
                        .child(crate::screens::timeline::user_text(&reg, light, &text)).child(div().flex().justify_end().text_size(px(11.)).text_color(c("faint")).child(stamp))).into_any_element()),
                "fold" => {
                    let value = string(e, "value");
                    ctl.push(CtlEntry { id: format!("fold:{id}"), label: text.clone(), ctl: Ctl::Fold(value.clone()) });
                    entries.push(div().id(SharedString::from(format!("e-{id}"))).py_2().child(
                        div().id(SharedString::from(format!("f-{id}"))).flex().gap_2().px_2().py_1().border_1().border_color(c("line")).text_size(px(12.)).text_color(c("dim")).cursor_pointer()
                            .hover(|s| s.bg(c("raise")))
                            .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("fold".into(), value.clone())); cx.notify(); }))
                            .child(ic(light, if e["open"] == true { "chevron-down" } else { "chevron-right" }, 14., "dim")).child(text)).into_any_element());
                }
                "assistant" => {
                    let mut col = div().id(SharedString::from(format!("e-{id}"))).w_full().py_2().flex().flex_col().gap_2();
                    match e["blocks"].as_array() {
                        Some(blocks) if !blocks.is_empty() => { let parsed = self.screens.parsed.get(&id, &text, blocks); for el in crate::screens::timeline::blocks(cx, &reg, light, &parsed, &id) { col = col.child(el); } }
                        _ => { col = col.child(crate::screens::timeline::user_text(&reg, light, &text)); }
                    }
                    if let Some(im) = crate::screens::timeline::images(light, &e["images"], &id, self.screens.attacher.target.as_ref(), &self.screens.images) { col = col.child(im); }
                    entries.push(col.child(div().text_size(px(11.)).text_color(c("faint")).child(stamp)).into_any_element());
                }
                _ => if !text.is_empty() {
                    entries.push(div().id(SharedString::from(format!("e-{id}"))).flex().gap_2().py_1().font(art::mono_font()).text_size(px(12.)).text_color(c("dim")).child(string(e, "label")).child(text).into_any_element());
                }
            }
        }
        reg.borrow_mut().resolve();
        for live in thread["live"].as_array().into_iter().flatten() { entries.push(div().py_2().child(string(live, "text")).into_any_element()); }
        // project home replaces the thread when the hub selected it
        let home = &self.data["projectHome"];
        let has_home = home.is_object();
        let home_els: Vec<AnyElement> = if has_home {
            home["cards"].as_array().into_iter().flatten().take(60).enumerate().map(|(i, k)| node_el(cx, k, light, 0, &format!("h{i}"), &mut ctl)).collect()
        } else { Vec::new() };
        // popup
        // the popup is the hub's: rows are shown when thread.skills is non-empty (no client-side rows)
        let skills = thread["skills"].as_array();
        let mut popup = div().flex().flex_col().border_1().border_color(c("line")).bg(c("panel")).py_1();
        for (index, skill) in skills.into_iter().flatten().take(8).enumerate() {
            let word = skill.get("word").or_else(|| skill.get("name")).and_then(Value::as_str).unwrap_or("").to_string();
            let label = skill.get("label").and_then(Value::as_str).unwrap_or(&word).to_string();
            let desc = ["text", "hint", "desc", "detail", "about"].iter().find_map(|k| skill.get(*k).and_then(Value::as_str)).unwrap_or("").to_string();
            ctl.push(CtlEntry { id: format!("skill:{word}"), label: label.clone(), ctl: Ctl::Skill(word.clone()) });
            popup = popup.child(div().id(("skill", index)).flex().gap_3().px_3().py_1().cursor_pointer().hover(|s| s.bg(c("raise")))
                .child(div().font_weight(FontWeight::BOLD).flex_shrink_0().child(label)).child(div().flex_1().overflow_hidden().whitespace_nowrap().text_color(c("dim")).child(desc))
                .on_click(cx.listener(move |this, _, window, cx| {
                    let _ = this.tx.try_send(Command::Action("skill".into(), word.clone())); this.await_completion(cx);
                    this.input.focus_handle(cx).focus(window); cx.notify();
                })));
        }
        let show_popup = skills.map_or(false, |s| !s.is_empty());
        // chips
        let model = thread["picker"]["models"].as_array().and_then(|m| m.iter().find(|x| x["on"] == true)).map(|m| string(m, "label")).unwrap_or_default();
        let model_short = if model.starts_with("Claude Code") { "default".to_string() } else { model.split(" · ").next().unwrap_or("").to_lowercase() };
        let effort = thread["picker"]["efforts"].as_array().and_then(|m| m.iter().find(|x| x["on"] == true)).map(|m| string(m, "label")).unwrap_or_default();
        // Thread permissions are read-only here and are not drawn in the composer: the web's composer chip beside the model chip is the account chip.
        let ctx = string(&thread["picker"], "label").split("ctx ").nth(1).map(|s| format!("ctx {}", s.split(' ').next().unwrap_or(""))).unwrap_or_default();
        let todo = thread["todos"]["count"].as_str().unwrap_or("").to_string();
        let rail: Vec<AnyElement> = [("Terminal", "terminal"), ("Commands", "search"), ("", ""), ("Board", "circuit-board"), ("Schematic", "cpu"), ("3D", "box"), ("Split", "layout-panel-left"), ("Mech", "layers"), ("", ""), ("Files", "folder"), ("Diff", "diff"), ("Review", "circle-check"), ("Browser", "globe")].iter().map(|(l, i)| {
            if l.is_empty() { div().h(px(12.)).into_any_element() } else { div().opacity(0.45).flex().flex_col().items_center().h(px(48.)).justify_center().gap_1().text_size(px(10.)).text_color(c("dim")).child(ic(light, i, 18., "dim")).child(*l).into_any_element() }
        }).collect();
        // controls with no handler yet are registered too, so the harness finds them dead instead of missing
        ctl.push(CtlEntry { id: "send".into(), label: "Send".into(), ctl: Ctl::Send });
        for l in ["Terminal", "Commands", "Board", "Schematic", "3D", "Split", "Mech", "Files", "Diff", "Review", "Browser"] { ctl.push(CtlEntry { id: format!("rail:{l}"), label: l.into(), ctl: Ctl::None }); }
        for l in ["Jump or run a command", "Improvements", "Search", "Settings", "Add project", "New thread", "New bot", "New room", "Account", "Edit", "Context", "Dictate"] { ctl.push(CtlEntry { id: format!("ui:{l}"), label: l.into(), ctl: Ctl::None }); }
        ctl.push(CtlEntry { id: "ui:Fold sidebar".into(), label: "Fold sidebar".into(), ctl: Ctl::ToggleSidebar });
        ctl.push(CtlEntry { id: "ui:Attach".into(), label: "Attach".into(), ctl: Ctl::PickFiles });
        if !self.sidebar_open { ctl.push(CtlEntry { id: "ui:Unfold sidebar".into(), label: "Unfold sidebar".into(), ctl: Ctl::ToggleSidebar }); }
        let connected = self.status == "Connected";
        if !self.screens.img_poll && self.screens.images.loading() { self.screens.img_poll = true; crate::screens::timeline::poll(cx); }

        let el = div().size_full().flex().flex_col().bg(c("bg")).text_color(c("text")).text_size(px(13.)).font_family("Noto Sans Mono CJK JP").line_height(px(20.))
            .on_action(cx.listener(Self::send)).on_action(cx.listener(Self::close_popup))
            // top bar
            .child(div().h(px(36.)).flex_shrink_0().relative().child(art::probe("topbar")).flex().items_center().justify_between().px_3().border_b_1().border_color(c("line")).bg(c("bg"))
                .child(div().flex().items_center().gap_2()
                    .when(!self.sidebar_open, |d| d.child(div().id("unfold-sidebar").cursor_pointer().child(ic(light, "chevron-right", 16., "dim")).on_click(cx.listener(|this, _, _, cx| { this.sidebar_open = true; this.ui_epoch += 1; cx.notify(); }))))
                    .child(div().relative().size(px(18.)).child(art::logo(light, 18., "logo"))
                        .child(div().absolute().right(px(-2.)).bottom(px(-1.)).size(px(6.)).bg(if connected { c("ok") } else { c("bad") })))
                    .child(div().font_weight(FontWeight::BOLD).child("Backplane"))
                    .child(div().text_color(c("faint")).child("/")).child(div().text_color(c("dim")).child(if has_thread { "Project".to_string() } else { "Projects".to_string() }))
                    .when(has_thread, |d| d.child(div().text_color(c("faint")).child("/")).child(div().font_weight(FontWeight::BOLD).child(string(thread, "title"))).child(ic(light, "chevron-down", 14., "dim"))))
                .child(div().flex().items_center().gap_3()
                    .child(div().opacity(0.45).w(px(260.)).px_2().flex().items_center().gap_2().border_1().border_color(c("line")).text_size(px(12.)).text_color(c("faint")).child(ic(light, "chevron-right", 13., "faint")).child("Jump or run a command"))
                    .child(div().opacity(0.45).px_2().flex().items_center().gap_2().border_1().border_color(c("line")).text_size(px(12.)).text_color(c("dim")).child(ic(light, "message-square", 15., "dim")).child("Improvements"))
                    .child(div().opacity(0.45).child(ic(light, "search", 15., "dim"))).child(div().opacity(0.45).child(ic(light, "settings", 15., "dim")))))
            .child(div().flex_1().min_h_0().flex()
                // sidebar
                .child(div().w(px(if self.sidebar_open { 264. } else { 0. })).overflow_hidden().h_full().flex_shrink_0().relative().child(art::probe("sidebar")).bg(c("bg")).border_r_1().border_color(c("line")).flex().flex_col()
                    .child(div().flex().items_center().gap_2().p_2().child(div().id("fold-sidebar").cursor_pointer().child(ic(light, "chevron-left", 16., "dim")).on_click(cx.listener(|this, _, _, cx| { this.sidebar_open = false; this.ui_epoch += 1; cx.notify(); }))).child(div().flex_1().px_2().py_1().border_1().border_color(c("line")).bg(c("panel")).text_size(px(12.)).text_color(c("faint")).child("Search projects")).child(div().opacity(0.45).child(ic(light, "plus", 16., "dim"))))
                    .child(div().id("sidebar").overflow_y_scroll().flex_1().children(rows))
                    .child(div().px_3().py_1().border_t_1().border_color(c("line")).text_size(px(11.)).text_color(c("dim")).child(self.status.clone())))
                // main
                .child(div().flex_1().min_w_0().h_full().relative().child(art::probe("main")).flex().flex_col()
                    .when(has_thread, |d| d.child(div().h(px(32.)).flex_shrink_0().flex().items_center().border_b_1().border_color(c("line"))
                        .child(div().px_3().flex().gap_2().text_size(px(11.)).child(div().text_color(c("faint")).child("STATE")).child(div().size(px(6.)).mt(px(7.)).bg(c("ok"))).child(string(thread, "phase").split(':').next().unwrap_or("").to_lowercase()))
                        .when(!todo.is_empty(), |d| d.child(div().px_3().h_full().flex().items_center().gap_2().border_l_1().border_color(c("line")).text_size(px(11.)).child(div().text_color(c("faint")).child("TO-DO")).child(todo.clone()).child(ic(light, "chevron-down", 12., "faint"))))))
                    .when(has_home, |d| d.child(div().id("home").overflow_y_scroll().flex_1().flex().justify_center().child(div().w_full().max_w(px(930.)).p_4().flex().flex_col().gap_3().children(home_els))))
                    .when(!has_home, |d| d.child(div().id("timeline").overflow_y_scroll().track_scroll(&self.scroll)
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, e, _, cx| this.timeline_down(e, cx))).on_mouse_down(MouseButton::Right, cx.listener(|this, e, _, cx| this.timeline_down(e, cx)))
                        .on_mouse_move(cx.listener(|this, e, _, cx| this.timeline_move(e, cx))).on_mouse_up(MouseButton::Left, cx.listener(|this, e, _, cx| this.timeline_up(e, cx))).on_mouse_up_out(MouseButton::Left, cx.listener(|this, e, _, cx| this.timeline_up(e, cx))).flex_1().flex().justify_center().child(div().w_full().max_w(px(930.)).px_4().flex().flex_col().justify_end().min_h_full().children({
                        if entries.is_empty() { entries.push(div().py_6().text_color(c("dim")).child(if has_thread && thread["loading"] == true { "Loading…" } else if has_thread { "Start a conversation." } else { "Pick a thread, or start one with + next to a project." }).into_any_element()); }
                        entries
                    }))))
                    .when(!self.error.is_empty(), |d| d.child(div().px_5().py_2().text_color(c("bad")).child(self.error.clone())))
                    .child(div().flex_shrink_0().relative().child(art::probe("composer")).border_t_1().border_color(c("line")).flex().justify_center().child(div().w_full().max_w(px(930.)).px_4().py_3().flex().flex_col().gap_2()
                        .when(show_popup, |d| d.child(popup))
                        .children(crate::screens::asks::view(cx, light, &self.screens.ask_inputs, &thread["asks"]))
                        .children(crate::screens::attach::chips(cx, light, &self.screens.attacher.previews, thread))
                        .child(div().border_1().border_color(c("accent")).bg(c("view")).px_3().py_2().min_h(px(60.)).child(self.input.clone()))
                        .child(div().flex().gap_2().items_center()
                            .child(div().flex_1().px_3().py_1().border_1().border_color(c("line")).bg(c("panel")).text_size(px(12.)).font(art::mono_font()).child(format!("claude / {} / {}", if model_short.is_empty() { "default" } else { &model_short }, if effort.is_empty() { "auto" } else { &effort })))
                            .child(chip(light, String::new()).opacity(0.45).flex().items_center().gap_2().child(ic(light, "user", 16., "text")).child("Default"))
                            .child(chip(light, String::new()).opacity(0.45).child(ic(light, "square-pen", 16., "dim")))
                            .child(chip(light, String::new()).opacity(0.45).flex().items_center().gap_2().child(ic(light, "gauge", 14., "dim")).child(ctx))
                            .child(chip(light, String::new()).id("attach").cursor_pointer().on_click(cx.listener(|this, _, _, cx| this.pick_files(cx))).child(ic(light, "paperclip", 16., "dim")))
                            .child(chip(light, String::new()).opacity(0.45).child(ic(light, "mic", 16., "dim")))
                            .child(div().id("send").px_4().py_1().bg(c("accent")).text_color(c("on-accent")).cursor_pointer().child("Send").on_click(cx.listener(|this, _, window, cx| this.send(&Send, window, cx)))))))
                )
                // right rail
                .child(div().w(px(72.)).h_full().flex_shrink_0().relative().child(art::probe("rail")).border_l_1().border_color(c("line")).flex().flex_col().py_2().children(rail)))
            .into_any_element();
        self.controls = ctl;
        self.render_us = t_render.elapsed().as_micros() as u64;
        el
    }
}
