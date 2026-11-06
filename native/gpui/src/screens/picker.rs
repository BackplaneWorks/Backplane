//! The model menu under the composer: the hub's `thread.picker` lists (providers, models, efforts) each with the one in force marked, and `thread.pickerAgents`, the same lists for
//! every other agent. A cursor moves over them on this client with no request per key (`key`, the old client's ModelPick.key); Enter or a click applies it (`plan`) and shuts the menu.
use crate::{hub::Command, string, theme, Backplane};
use super::composer::Composer;
use gpui::{prelude::*, *};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Choice { pub label: String, pub value: String, pub on: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct Section { pub title: &'static str, pub action: &'static str, pub choices: Vec<Choice> }

fn choices(v: &Value) -> Vec<Choice> {
    v.as_array().into_iter().flatten().map(|c| Choice { label: string(c, "label"), value: string(c, "value"), on: c["on"].as_bool().unwrap_or(false) }).collect()
}
pub fn parse(picker: &Value) -> Vec<Section> {
    [("Provider", "provider", "providers"), ("Model", "model", "models"), ("Effort", "effort", "efforts")].into_iter()
        .map(|(title, action, key)| Section { title, action, choices: choices(&picker[key]) }).filter(|s| !s.choices.is_empty()).collect()
}
/// "@claude" (a provider row's value) as the hub names the agent.
pub fn bare(provider: &str) -> &str { provider.strip_prefix('@').unwrap_or(provider) }
/// The sections with the cursor on `provider`: another agent's models and efforts in place of the thread's. Nothing in them is in force, so nothing is marked.
pub fn for_provider(picker: &Value, agents: &Value, provider: &str) -> Vec<Section> {
    let own = parse(picker);
    if provider.is_empty() || provider == provider_of(&own) { return own }
    let a = &agents[bare(provider)];
    let unmarked = |v: &Value| -> Vec<Choice> { choices(v).into_iter().map(|c| Choice { on: false, ..c }).collect() };
    let mut models = unmarked(&a["models"]);
    if models.is_empty() { models.push(Choice { label: "default".into(), value: String::new(), on: false }); }
    let mut out: Vec<Section> = own.into_iter().filter(|s| s.action == "provider").collect();
    out.push(Section { title: "Model", action: "model", choices: models });
    let efforts = unmarked(&a["efforts"]);
    if !efforts.is_empty() { out.push(Section { title: "Effort", action: "effort", choices: efforts }); }
    out
}

/// The columns, in the order Tab goes through them.
pub const COLS: [&str; 3] = ["provider", "model", "effort"];
pub fn col_of(action: &str) -> usize { COLS.iter().position(|c| *c == action).unwrap_or(1) }
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Key { Tab, BackTab, Up, Down, Left, Right }
/// What Enter would apply, and the column the keys are in; a draft until then.
#[derive(Clone, Debug, PartialEq)]
pub struct Cursor { pub col: usize, pub provider: String, pub model: String, pub effort: String }
impl Cursor {
    /// The thread's own agent, model and effort, in the model column.
    pub fn own(all: &[Section]) -> Self {
        let on = |a: &str| all.iter().find(|s| s.action == a).and_then(|s| s.choices.iter().find(|c| c.on)).map(|c| c.value.clone()).unwrap_or_default();
        Self { col: 1, provider: on("provider"), model: on("model"), effort: on("effort") }
    }
    fn value(&self, col: usize) -> &str { match col { 0 => &self.provider, 1 => &self.model, _ => &self.effort } }
    /// Another agent starts on its default model, the thread's agent on the thread's model; the effort stays.
    pub fn set(&mut self, col: usize, v: &str, own: &Cursor) {
        match col {
            0 => { if self.provider != v { self.model = if v == own.provider { own.model.clone() } else { String::new() }; self.provider = v.to_string(); } }
            1 => self.model = v.to_string(),
            _ => self.effort = v.to_string(),
        }
    }
}
/// The row of `s` the cursor is on: an effort this agent does not take reads as its default.
pub fn at(s: &Section, c: &Cursor) -> Option<usize> {
    let row = |v: &str| s.choices.iter().position(|x| x.value == v);
    row(c.value(col_of(s.action))).or_else(|| if s.action == "effort" { row("") } else { None })
}
/// What a key does to the cursor over the rows on show. Tab and Shift+Tab go round the columns there are; Up and Down go round the rows of the column; Left and Right step the effort from any
/// column and stop at its ends.
pub fn key(k: Key, c: &Cursor, rows: &[Section], own: &Cursor) -> Cursor {
    let mut n = c.clone();
    let section = |col: usize| rows.iter().find(|s| col_of(s.action) == col);
    match k {
        Key::Tab | Key::BackTab => {
            let step = if k == Key::Tab { 1 } else { COLS.len() - 1 };
            if let Some(col) = (1..=COLS.len()).map(|i| (c.col + i * step) % COLS.len()).find(|col| section(*col).is_some()) { n.col = col; }
        }
        Key::Up | Key::Down => if let Some(s) = section(c.col).or(rows.first()) {
            n.col = col_of(s.action); let len = s.choices.len() as i32; let d = if k == Key::Down { 1 } else { -1 };
            let to = match at(s, &n) { Some(i) => (i as i32 + d).rem_euclid(len), None if d > 0 => 0, None => len - 1 };
            n.set(n.col, &s.choices[to as usize].value, own);
        },
        Key::Left | Key::Right => if let Some(s) = section(2) {
            let to = match (at(s, c), k == Key::Right) { (Some(i), true) => (i + 1).min(s.choices.len() - 1), (Some(i), false) => i.saturating_sub(1), (None, _) => 0 };
            n.effort = s.choices[to].value.clone();
        },
    }
    n
}
/// The search changed: the cursor goes to the first match (a model when any matches), which is what Enter then takes.
pub fn settle(c: &Cursor, rows: &[Section], own: &Cursor) -> Cursor {
    let mut n = c.clone();
    if let Some(s) = rows.iter().find(|s| s.action == "model").or(rows.first()) { n.col = col_of(s.action); n.set(n.col, &s.choices[0].value, own); }
    n
}
/// The requests that apply a cursor against the thread's own: another agent is one switch carrying the model and any new effort, the same agent asks for what changed, nothing changed asks for nothing.
pub fn plan(c: &Cursor, now: &Cursor) -> Vec<Command> {
    if c.provider != now.provider {
        let mut v = format!("{}\u{1f}{}", bare(&c.provider), c.model);
        if c.effort != now.effort { v.push('\u{1f}'); v.push_str(&c.effort); }
        return vec![Command::Action("provider".into(), v)];
    }
    let mut out = vec![];
    if c.model != now.model { out.push(Command::Action("model".into(), c.model.clone())); }
    if c.effort != now.effort { out.push(Command::Action("effort".into(), c.effort.clone())); }
    out
}
/// The list's child that shows the cursor's row (each section is its title, then its rows).
pub fn child_of(rows: &[Section], c: &Cursor) -> Option<usize> {
    let mut ix = 0;
    for s in rows { if col_of(s.action) == c.col { return at(s, c).map(|r| ix + 1 + r); } ix += 1 + s.choices.len(); }
    None
}
/// The keycap in a column's head, read from the column the keys are in.
pub fn cap(col: usize, here: usize) -> &'static str {
    if col == 2 { "←→" } else if col == here { "↑↓" } else if col == (here + 1) % COLS.len() { "Tab" } else { "⇧Tab" }
}

/// What the menu keeps between frames: the search box, the scroll of the list, the cursor once a key moved it (with its thread: a cursor moved on another thread is not this one's).
#[derive(Default)]
pub struct Pick { pub search: Option<Entity<Composer>>, pub q: String, pub scroll: ScrollHandle, pub cur: Option<(String, Cursor)> }

/// The provider in force (the marked choice of the provider section), "" when the hub lists none.
pub fn provider_of(sections: &[Section]) -> String {
    sections.iter().find(|s| s.action == "provider").and_then(|s| s.choices.iter().find(|c| c.on)).map(|c| c.value.clone()).unwrap_or_default()
}
pub fn fav_key(provider: &str, value: &str) -> String { format!("{provider}/{value}") }

/// The sections for a search text and the starred models of `provider`: a choice stays when its label or value holds the text (any case); the models the person starred come first, in the hub's order otherwise.
pub fn arrange(sections: &[Section], provider: &str, q: &str, favs: &[String]) -> Vec<Section> {
    let q = q.trim().to_lowercase();
    sections.iter().filter_map(|s| {
        let mut ch: Vec<Choice> = s.choices.iter().filter(|c| q.is_empty() || c.label.to_lowercase().contains(&q) || c.value.to_lowercase().contains(&q)).cloned().collect();
        if s.action == "model" { ch.sort_by_key(|c| !favs.iter().any(|f| *f == fav_key(provider, &c.value))); } // stable: the rest keep the hub's order
        (!ch.is_empty()).then(|| Section { choices: ch, ..s.clone() })
    }).collect()
}
/// The menu as it stands: the thread's own cursor, the cursor, every row for its agent, and the rows on show.
pub struct Shown { pub own: Cursor, pub cur: Cursor, pub all: Vec<Section>, pub rows: Vec<Section> }
pub fn shown(thread: &Value, pick: &Pick, favs: &[String]) -> Option<Shown> {
    let mine = parse(&thread["picker"]);
    if mine.is_empty() { return None }
    let own = Cursor::own(&mine); let id = string(thread, "id");
    let cur = pick.cur.as_ref().filter(|(t, _)| *t == id).map_or_else(|| own.clone(), |(_, c)| c.clone());
    let all = for_provider(&thread["picker"], &thread["pickerAgents"], &cur.provider);
    let rows = arrange(&all, &cur.provider, &pick.q, favs);
    Some(Shown { own, cur, all, rows })
}

/// A provider's mark: a square of its colour with its initial. (The hub sends no artwork; these stand in.)
pub fn mark(provider: &str) -> (u32, &'static str) {
    match provider { "claude" => (0xd97757, "A"), "codex" => (0x10a37f, "O"), "grok" => (0x6b7280, "X"), "muse" => (0x8b5cf6, "M"), "opencode" => (0x2563eb, "C"),
        "pi" => (0xdb2777, "π"), "omp" => (0xc026d3, "Ω"), "antigravity" | "agy" => (0x4285f4, "G"), _ => (0x64748b, "?") }
}
pub fn logo(provider: &str) -> Div {
    let (rgb_, letter) = mark(bare(provider));
    div().w(px(16.)).h(px(16.)).flex_shrink_0().flex().items_center().justify_center().bg(rgb(rgb_)).text_color(rgb(0xffffff)).text_size(px(10.)).font_weight(FontWeight::BOLD).child(letter)
}

/// The thumb of a scroll bar: (top, height) in px inside a track of `track` px, for a view `view` px tall holding `max + view` px, scrolled `y` (>= 0).
pub fn thumb(track: f32, view: f32, max: f32, y: f32) -> Option<(f32, f32)> {
    if max <= 0.5 || track <= 0. { return None }
    let h = (track * view / (view + max)).clamp(24., track); Some(((track - h) * (y / max).clamp(0., 1.), h))
}

pub fn view(cx: &mut Context<Backplane>, light: bool, thread: &Value, open: bool, pick: &Pick, favs: &[String]) -> Option<AnyElement> {
    if !open { return None; }
    let c = |n: &str| theme::color(light, n);
    let Shown { cur, rows: sections, .. } = shown(thread, pick, favs)?;
    let provider = cur.provider.clone();
    let cap_h = 320.;
    let mut col = div().id("picker-list").track_scroll(&pick.scroll).max_h(px(cap_h)).overflow_y_scroll().flex().flex_col().py_1().pr(px(10.));
    if sections.is_empty() { col = col.child(div().px_3().py_1().text_color(c("faint")).child("Nothing matches")); }
    for (si, s) in sections.iter().enumerate() {
        let n = col_of(s.action); let here_col = n == cur.col; let row_at = at(s, &cur);
        col = col.child(div().px_3().pt_1().flex().justify_between().text_size(px(11.)).text_color(if here_col { c("accent") } else { c("faint") }).child(s.title.to_uppercase()).child(cap(n, cur.col)));
        for (ci, ch) in s.choices.iter().enumerate() {
            let value = ch.value.clone();
            let fav = fav_key(&provider, &ch.value);
            let starred = s.action == "model" && favs.iter().any(|f| *f == fav);
            let (picked, here) = (row_at == Some(ci), here_col && row_at == Some(ci));
            let mut row = div().flex().items_center().hover(|st| st.bg(c("raise"))).when(here, |d| d.bg(c("raise"))).when(picked, |d| d.text_color(c("accent")))
                .child(div().id(SharedString::from(format!("pick-{si}-{ci}"))).flex_1().min_w_0().flex().items_center().gap_2().px_3().py_1().cursor_pointer()
                    .child(div().w(px(14.)).text_color(c("accent")).child(if ch.on { "✓" } else { "" }))
                    .when(s.action == "provider", |d| d.child(logo(&ch.value)))
                    .child(ch.label.clone())
                    .on_click(cx.listener(move |this, _, _, cx| this.picker_press(n, &value, cx))));
            if s.action == "model" && !ch.value.is_empty() {
                let f2 = fav.clone();
                row = row.child(div().id(SharedString::from(format!("fav-{si}-{ci}"))).px_2().py_1().cursor_pointer().text_color(if starred { c("accent") } else { c("faint") })
                    .child(if starred { "★" } else { "☆" })
                    .on_click(cx.listener(move |this, _, _, cx| { this.local_press("fav", &f2, cx); })));
            }
            col = col.child(row);
        }
    }
    // the scroll bar: a thin track at the right edge with a thumb, drawn from the list's own scroll state
    let (view_h, max, y) = (f32::from(pick.scroll.bounds().size.height), f32::from(pick.scroll.max_offset().height), -f32::from(pick.scroll.offset().y));
    let bar = thumb(view_h, view_h, max, y).map(|(top, h)| div().absolute().top(px(top)).right(px(2.)).w(px(6.)).h(px(h)).bg(c("faint")).opacity(0.7));
    let search = pick.search.clone().map(|s| div().px_3().py_1().border_b_1().border_color(c("line")).flex().items_center().gap_2().child(div().text_color(c("faint")).child("⌕")).child(div().flex_1().child(s)));
    Some(div().id("picker").flex().flex_col().border_1().border_color(c("line")).bg(c("panel")).children(search)
        .child(div().relative().child(col).children(bar)).into_any_element())
}

impl Backplane {
    /// The model chip or its key: open or shut the menu; opening clears the search and puts the keyboard in it.
    pub(crate) fn picker_toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.screens.picker_open { self.picker_close(cx); return; }
        self.screens.picker_open = true; self.screens.pick.cur = None;
        if self.screens.pick.search.is_none() {
            let input = cx.new(|cx| Composer::with_context(cx, "PickInput"));
            input.update(cx, |i, _| i.set_placeholder("Search models"));
            cx.observe(&input, |this, input, cx| {
                let q = input.read(cx).content.to_string();
                if q != this.screens.pick.q { this.screens.pick.q = q; this.picker_searched(); }
                cx.notify();
            }).detach();
            self.screens.pick.search = Some(input);
        }
        let s = self.screens.pick.search.clone().unwrap();
        s.update(cx, |i, cx| i.set(String::new(), cx));
        self.screens.pick.q.clear();
        window.focus(&s.focus_handle(cx));
        self.picker_reveal();
        cx.notify();
    }
    /// The menu has the arrow keys only while nothing is open over it.
    pub(crate) fn picker_keys(&self) -> bool { self.screens.picker_open && !self.screens.palette.open && self.screens.lightbox.is_none() && !self.screens.acct.menu && !super::settings::is_open(self) }
    /// Shut the menu: the cursor and the search go, the keyboard goes back to the composer.
    pub(crate) fn picker_close(&mut self, cx: &mut Context<Self>) {
        self.screens.picker_open = false; self.screens.pick.cur = None; self.want_focus = true; cx.notify();
    }
    fn picker_shown(&self) -> Option<Shown> { shown(&self.data["thread"], &self.screens.pick, &self.local.favs) }
    fn picker_store(&mut self, c: Cursor) { self.screens.pick.cur = Some((string(&self.data["thread"], "id"), c)); self.picker_reveal(); }
    /// Scroll the cursor's row into view.
    fn picker_reveal(&mut self) {
        let Some(s) = self.picker_shown() else { return };
        if let Some(ix) = child_of(&s.rows, &s.cur) { self.screens.pick.scroll.scroll_to_item(ix); }
        self.m("pick", serde_json::json!({"col": COLS[s.cur.col], "provider": s.cur.provider, "model": s.cur.model, "effort": s.cur.effort, "child": child_of(&s.rows, &s.cur), "y": -f32::from(self.screens.pick.scroll.offset().y)}));
    }
    fn picker_searched(&mut self) {
        let Some(s) = self.picker_shown() else { return };
        if !self.screens.pick.q.trim().is_empty() { let c = settle(&s.cur, &s.rows, &s.own); self.picker_store(c); }
    }
    /// A key moved the cursor: here, with no request. The list scrolls while it paints, so the scroll bar is drawn again from the next frame (once).
    pub(crate) fn picker_key(&mut self, k: Key, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = self.picker_shown() else { return };
        self.picker_store(key(k, &s.cur, &s.rows, &s.own));
        cx.on_next_frame(window, |_, _, cx| cx.notify());
        cx.notify();
    }
    /// Left and Right: the search's caret while it holds text, else the effort.
    pub(crate) fn picker_side(&mut self, k: Key, window: &mut Window, cx: &mut Context<Self>) {
        let search = self.screens.pick.search.clone().filter(|s| !self.screens.pick.q.is_empty() && s.focus_handle(cx).is_focused(window));
        match search { Some(s) => s.update(cx, |i, cx| i.caret(if k == Key::Left { -1 } else { 1 }, cx)), None => self.picker_key(k, window, cx) }
    }
    /// Enter: the cursor applies; with nothing on show it shuts and asks for nothing.
    pub(crate) fn picker_enter(&mut self, cx: &mut Context<Self>) {
        if let Some(s) = self.picker_shown() { if !s.rows.is_empty() { self.picker_apply(s.cur, &s.all, &s.own); } }
        self.picker_close(cx);
    }
    /// A row pressed: its value goes on the cursor, which applies.
    fn picker_press(&mut self, col: usize, value: &str, cx: &mut Context<Self>) {
        if let Some(s) = self.picker_shown() {
            let mut c = s.cur.clone(); c.set(col, value, &s.own);
            let all = for_provider(&self.data["thread"]["picker"], &self.data["thread"]["pickerAgents"], &c.provider);
            self.picker_apply(c, &all, &s.own);
        }
        self.picker_close(cx);
    }
    fn picker_apply(&self, mut c: Cursor, all: &[Section], own: &Cursor) {
        if let Some(s) = all.iter().find(|s| s.action == "effort") { if let Some(i) = at(s, &c) { c.effort = s.choices[i].value.clone(); } } // an effort the agent does not take is its default
        for cmd in plan(&c, own) { let _ = self.tx.try_send(cmd); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn fixture() -> (Value, Value) {
        let picker = serde_json::json!({"providers": [{"label": "claude", "value": "@claude", "on": true}, {"label": "codex", "value": "@codex"}, {"label": "grok", "value": "@grok"}],
            "models": [{"label": "default", "value": ""}, {"label": "opus", "value": "opus", "on": true}, {"label": "sonnet", "value": "sonnet"}],
            "efforts": [{"label": "auto", "value": ""}, {"label": "low", "value": "low"}, {"label": "high", "value": "high", "on": true}]});
        let agents = serde_json::json!({"codex": {"models": [{"label": "default", "value": "", "on": true}, {"label": "gpt", "value": "gpt"}], "efforts": [{"label": "auto", "value": ""}, {"label": "low", "value": "low"}]},
            "grok": {"models": [{"label": "default", "value": ""}], "efforts": []}});
        (picker, agents)
    }
    fn acts(v: Vec<Command>) -> Vec<(String, String)> { v.into_iter().map(|c| match c { Command::Action(a, v) => (a, v), c => panic!("{c:?}") }).collect() }
    fn pair(a: &str, v: &str) -> (String, String) { (a.to_string(), v.to_string()) }
    #[test] fn sections_in_tab_order_with_the_marked_choice_and_empty_ones_dropped() {
        let p: Value = serde_json::from_str(r#"{"label":"x","models":[{"label":"Opus","value":"opus","on":true},{"label":"Haiku","value":"haiku","on":false}],"efforts":[{"label":"auto","value":"","on":true}],"providers":[]}"#).unwrap();
        let s = parse(&p);
        assert_eq!(s.iter().map(|x| x.action).collect::<Vec<_>>(), ["model", "effort"]);
        assert!(s[0].choices[0].on && !s[0].choices[1].on);
        assert!(parse(&Value::Null).is_empty());
        let (p, _) = fixture(); assert_eq!(parse(&p).iter().map(|x| x.action).collect::<Vec<_>>(), COLS);
    }
    #[test] fn search_filters_and_starred_models_lead() {
        let p: Value = serde_json::from_str(r#"{"models":[{"label":"Opus","value":"opus","on":true},{"label":"Haiku","value":"haiku"},{"label":"Sonnet","value":"sonnet"}],"providers":[{"label":"Claude","value":"claude","on":true},{"label":"Codex","value":"codex"}]}"#).unwrap();
        let s = parse(&p);
        assert_eq!(provider_of(&s), "claude");
        let a = arrange(&s, "claude", "", &["claude/sonnet".to_string()]);
        assert_eq!(a[1].choices.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(), ["sonnet", "opus", "haiku"]);
        let b = arrange(&s, "claude", "HAI", &[]); assert_eq!(b[0].choices.len(), 1); assert_eq!(b.len(), 1, "providers that do not match are dropped");
        assert!(arrange(&s, "claude", "zzz", &[]).is_empty());
        assert_eq!(arrange(&s, "claude", "co", &[]).iter().map(|x| x.action).collect::<Vec<_>>(), ["provider"], "Codex matches by name");
    }
    #[test] fn the_cursor_starts_on_the_threads_own_in_the_model_column() {
        let (p, _) = fixture(); let c = Cursor::own(&parse(&p));
        assert_eq!(c, Cursor { col: 1, provider: "@claude".into(), model: "opus".into(), effort: "high".into() });
    }
    #[test] fn up_and_down_go_round_the_column() {
        let (p, _) = fixture(); let s = parse(&p); let own = Cursor::own(&s);
        let c = key(Key::Down, &own, &s, &own); assert_eq!(c.model, "sonnet");
        let c = key(Key::Down, &c, &s, &own); assert_eq!(c.model, "", "past the last it wraps to the first");
        let c = key(Key::Up, &c, &s, &own); assert_eq!(c.model, "sonnet", "and back up over the top");
        let mut e = own.clone(); e.col = 2; assert_eq!(key(Key::Down, &e, &s, &own).effort, "", "the effort column wraps as well");
    }
    #[test] fn tab_goes_round_the_columns_and_skips_one_not_on_show() {
        let (p, _) = fixture(); let s = parse(&p); let own = Cursor::own(&s);
        let c = key(Key::Tab, &own, &s, &own); assert_eq!(c.col, 2, "model to effort");
        let c = key(Key::Tab, &c, &s, &own); assert_eq!(c.col, 0, "effort to provider");
        assert_eq!(key(Key::BackTab, &c, &s, &own).col, 2); assert_eq!(key(Key::BackTab, &own, &s, &own).col, 0, "shift+tab: model to provider");
        let no_effort: Vec<Section> = s.iter().filter(|x| x.action != "effort").cloned().collect();
        assert_eq!(key(Key::Tab, &own, &no_effort, &own).col, 0); assert_eq!((own.model.as_str(), key(Key::Tab, &own, &s, &own).model.as_str()), ("opus", "opus"), "Tab moves no choice");
    }
    #[test] fn left_and_right_step_the_effort_from_any_column_and_stop_at_its_ends() {
        let (p, _) = fixture(); let s = parse(&p); let own = Cursor::own(&s);
        assert_eq!(key(Key::Right, &own, &s, &own).effort, "high", "no wrapping at the top");
        let c = key(Key::Left, &own, &s, &own); assert_eq!((c.effort.as_str(), c.col), ("low", 1), "the column stays");
        let c = key(Key::Left, &c, &s, &own); assert_eq!(c.effort, ""); assert_eq!(key(Key::Left, &c, &s, &own).effort, "");
    }
    #[test] fn another_agent_shows_its_own_lists_and_starts_on_its_default() {
        let (p, a) = fixture(); let s = parse(&p); let own = Cursor::own(&s);
        let mut c = own.clone(); c.col = 0;
        let c = key(Key::Down, &c, &s, &own); assert_eq!((c.provider.as_str(), c.model.as_str(), c.effort.as_str()), ("@codex", "", "high"));
        let rows = for_provider(&p, &a, &c.provider);
        assert_eq!(rows[1].choices.iter().map(|x| x.value.as_str()).collect::<Vec<_>>(), ["", "gpt"]);
        assert!(rows[1].choices.iter().all(|x| !x.on) && rows[0].choices[0].on, "only what is in force is marked");
        assert_eq!(at(&rows[2], &c), Some(0), "an effort it does not take reads as its default");
        let g = for_provider(&p, &a, "@grok"); assert_eq!(g.iter().map(|x| x.action).collect::<Vec<_>>(), ["provider", "model"], "an agent with no efforts has no such column");
        assert_eq!(for_provider(&p, &Value::Null, "@codex")[1].choices.len(), 1, "unknown lists: its default");
        let back = key(Key::Up, &c, &rows, &own); assert_eq!(back.model, "opus", "back on the thread's agent: the thread's model");
    }
    #[test] fn a_search_puts_the_cursor_on_the_first_match_a_model_first() {
        let (p, _) = fixture(); let s = parse(&p); let own = Cursor::own(&s);
        let c = settle(&own, &arrange(&s, "@claude", "son", &[]), &own); assert_eq!((c.col, c.model.as_str()), (1, "sonnet"));
        let c = settle(&own, &arrange(&s, "@claude", "cod", &[]), &own); assert_eq!((c.col, c.provider.as_str(), c.model.as_str()), (0, "@codex", ""));
        assert_eq!(settle(&own, &[], &own), own, "nothing matches: it stays");
        let c = settle(&own, &arrange(&s, "@claude", "o", &["@claude/sonnet".into()]), &own); assert_eq!(c.model, "sonnet", "a star leads");
    }
    #[test] fn enter_asks_for_what_changed_and_another_agent_is_one_switch() {
        let (p, _) = fixture(); let own = Cursor::own(&parse(&p));
        assert!(plan(&own, &own).is_empty());
        let mut c = own.clone(); c.model = "sonnet".into(); assert_eq!(acts(plan(&c, &own)), [pair("model", "sonnet")]);
        c.effort = "".into(); assert_eq!(acts(plan(&c, &own)), [pair("model", "sonnet"), pair("effort", "")]);
        let d = Cursor { col: 0, provider: "@codex".into(), model: "gpt".into(), effort: "high".into() }; assert_eq!(acts(plan(&d, &own)), [pair("provider", "codex\u{1f}gpt")]);
        let d = Cursor { effort: "low".into(), ..d }; assert_eq!(acts(plan(&d, &own)), [pair("provider", "codex\u{1f}gpt\u{1f}low")]);
    }
    #[test] fn the_row_to_scroll_to_counts_the_titles() {
        let (p, _) = fixture(); let s = parse(&p); let own = Cursor::own(&s);
        assert_eq!(child_of(&s, &own), Some(4 + 1 + 1), "provider title and rows, model title, default, then opus");
        let mut c = own.clone(); c.col = 0; assert_eq!(child_of(&s, &c), Some(1)); c.col = 2; assert_eq!(child_of(&s, &c), Some(4 + 4 + 1 + 2));
        assert_eq!(child_of(&[], &own), None);
    }
    #[test] fn the_keycaps_read_from_the_column_the_keys_are_in() {
        assert_eq!((cap(0, 1), cap(1, 1), cap(2, 1)), ("⇧Tab", "↑↓", "←→")); assert_eq!((cap(0, 2), cap(1, 2)), ("Tab", "⇧Tab")); assert_eq!(cap(1, 0), "Tab");
    }
    #[test] fn a_cursor_moved_on_another_thread_is_not_this_ones() {
        let (p, a) = fixture(); let t = serde_json::json!({"id": "h|t2", "picker": p, "pickerAgents": a});
        let far = Cursor { col: 0, provider: "@codex".into(), model: "gpt".into(), effort: "low".into() };
        let pick = Pick { cur: Some(("h|t1".into(), far.clone())), ..Default::default() };
        let s = shown(&t, &pick, &[]).unwrap(); assert_eq!(s.cur, s.own);
        let pick = Pick { cur: Some(("h|t2".into(), far.clone())), ..Default::default() };
        let s = shown(&t, &pick, &[]).unwrap(); assert_eq!(s.cur, far); assert_eq!(s.rows[1].choices.len(), 2, "its agent's rows");
    }
    #[test] fn a_provider_value_names_its_mark() { assert_eq!(mark(bare("@claude")), mark("claude")); assert_eq!(bare("codex"), "codex"); }
    #[test] fn the_thumb_follows_the_scroll_and_hides_without_overflow() {
        assert_eq!(thumb(300., 300., 0., 0.), None);
        let (top, h) = thumb(300., 300., 300., 0.).unwrap(); assert_eq!((top, h), (0., 150.));
        let (top, h) = thumb(300., 300., 300., 300.).unwrap(); assert_eq!(top + h, 300.);
        assert!(thumb(300., 300., 1_000_000., 0.).unwrap().1 >= 24.);
    }
    #[test] fn every_provider_has_a_mark() { for p in ["claude", "codex", "grok", "muse", "opencode", "pi", "omp", "antigravity", "mystery"] { assert!(!mark(p).1.is_empty()); } }
}
