//! The model picker over the composer, as 0.14 drew it (src/web/view.bend's View.mp.*, src/core/modelpick.bend): three columns, agent | model | effort.
//! The hub sends `thread.picker` (the thread's own agent, model and effort, each list with the one in force marked; each agent with what it is and whether it
//! can be used) and `thread.pickerAgents` (every agent's models, each with what its CLI calls it, and the efforts each model takes). A cursor moves over them on
//! this client with no request per key (Tab and Shift+Tab go from column to column, Up and Down within one, Left and Right move the effort from any column);
//! Enter applies it and shuts the picker, Escape only shuts it. A click applies at once and the picker stays open (`plan`). The model column has a search and
//! shows at most `CAP` rows (the search narrows the rest); the effort column is a meter, a bar a step, the steps the model does not take drawn but disabled.
use crate::{hub::Command, string, theme, Backplane};
use super::composer::Composer;
use gpui::{prelude::*, *};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Choice { pub label: String, pub value: String, pub on: bool, pub note: String, pub ok: bool, pub efforts: Option<Vec<String>> }
#[derive(Clone, Debug, PartialEq)]
pub struct Section { pub title: &'static str, pub action: &'static str, pub choices: Vec<Choice> }

fn choices(v: &Value) -> Vec<Choice> {
    v.as_array().into_iter().flatten().map(|c| Choice { label: string(c, "label"), value: string(c, "value"), on: c["on"].as_bool().unwrap_or(false), note: string(c, "note"), ok: c["ok"].as_bool().unwrap_or(true),
        efforts: c["efforts"].as_array().map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()) }).collect()
}
pub fn parse(picker: &Value) -> Vec<Section> {
    [("Agent", "provider", "providers"), ("Model", "model", "models"), ("Effort", "effort", "efforts")].into_iter()
        .map(|(title, action, key)| Section { title, action, choices: choices(&picker[key]) }).filter(|s| !s.choices.is_empty()).collect()
}
/// "@claude" (a provider row's value) as the hub names the agent.
pub fn bare(provider: &str) -> &str { provider.strip_prefix('@').unwrap_or(provider) }
/// The sections with the cursor on `provider`: another agent's models and efforts in place of the thread's. Nothing in them is in force, so nothing is marked.
pub fn for_provider(picker: &Value, agents: &Value, provider: &str) -> Vec<Section> {
    let own = parse(picker);
    let unmarked = |v: &Value| -> Vec<Choice> { choices(v).into_iter().map(|c| Choice { on: false, ..c }).collect() };
    if provider.is_empty() || provider == provider_of(&own) {
        // the thread's own lists; the effort column is the agent's (its default model's) when the hub sent it: the thread's own model's are on its row only when they differ
        let Some(es) = agents[bare(&provider_of(&own))]["efforts"].as_array().map(|_| unmarked(&agents[bare(&provider_of(&own))]["efforts"])) else { return own };
        let mut out: Vec<Section> = own.into_iter().filter(|s| s.action != "effort").collect();
        if !es.is_empty() { out.push(Section { title: "Effort", action: "effort", choices: es }); }
        return out
    }
    let a = &agents[bare(provider)];
    let mut models = unmarked(&a["models"]);
    let ok = a["ok"].as_bool().unwrap_or(true);
    if models.is_empty() && ok { models.push(Choice { label: "default".into(), value: String::new(), on: false, note: String::new(), ok: true, efforts: None }); }
    let mut out: Vec<Section> = own.into_iter().filter(|s| s.action == "provider").collect();
    out.push(Section { title: "Model", action: "model", choices: models });
    let efforts = unmarked(&a["efforts"]);
    if !efforts.is_empty() { out.push(Section { title: "Effort", action: "effort", choices: efforts }); }
    out
}
/// Why the cursor's agent cannot be used, or "" (the hub says: its CLI is not installed, or it lists no models).
pub fn agent_why(rows: &[Section], provider: &str, agents: &Value) -> String {
    let said = string(&agents[bare(provider)], "why");
    if !said.is_empty() { return said }
    rows.iter().find(|s| s.action == "provider").and_then(|s| s.choices.iter().find(|c| c.value == provider)).filter(|c| !c.ok).map(|c| c.note.clone()).unwrap_or_default()
}
pub fn agent_ok(rows: &[Section], provider: &str) -> bool {
    rows.iter().find(|s| s.action == "provider").and_then(|s| s.choices.iter().find(|c| c.value == provider)).map_or(true, |c| c.ok)
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

// ---- the effort meter (0.14's ModelPick.steps / .bars) ----
/// The scale the meter draws its steps in: auto (the agent's own default, ""), then the levels from none to max.
const SCALE: [&str; 10] = ["", "off", "none", "auto", "minimal", "low", "medium", "high", "xhigh", "max"];
/// The steps always drawn, whether the model takes them or not.
const COMMON: [&str; 6] = ["", "low", "medium", "high", "xhigh", "max"];
#[derive(Clone, Debug, PartialEq)]
pub struct Step { pub level: String, pub on: bool }
/// The levels the cursor's model takes ("" first; none when it takes none): its own list when the hub sent one, else its agent's.
pub fn levels(rows: &[Section], model: &str) -> Vec<String> {
    let own = rows.iter().find(|s| s.action == "model").and_then(|s| s.choices.iter().find(|c| c.value == model)).and_then(|c| c.efforts.clone());
    own.unwrap_or_else(|| rows.iter().find(|s| s.action == "effort").map(|s| s.choices.iter().map(|c| c.value.clone()).collect()).unwrap_or_default())
}
/// The meter for levels `es`: the common steps and any other level the agent takes, in the scale's order; a step not in `es` is drawn but cannot be picked.
pub fn steps(es: &[String]) -> Vec<Step> {
    let has = |x: &str| es.iter().any(|e| e == x);
    let mut v: Vec<Step> = SCALE.iter().filter(|x| COMMON.contains(x) || has(x)).map(|x| Step { level: x.to_string(), on: has(x) }).collect();
    v.extend(es.iter().filter(|e| !SCALE.contains(&e.as_str())).map(|e| Step { level: e.clone(), on: true }));
    v
}
/// The step lit for effort `e`: `e` when the meter takes it, else the agent's default; None when the model takes no effort.
pub fn lit(ss: &[Step], e: &str) -> Option<String> {
    let on: Vec<&Step> = ss.iter().filter(|s| s.on).collect();
    if on.iter().any(|s| s.level == e) { Some(e.to_string()) } else if on.iter().any(|s| s.level.is_empty()) { Some(String::new()) } else { None }
}
/// A step's word under its bar (short: the column is narrow).
pub fn step_word(e: &str) -> &str { match e { "" => "auto", "medium" => "med", "minimal" => "min", x => x } }
/// A step's word on this meter: the agent's own default is "def" where the agent also has a level called auto (Oh My Pi).
pub fn word<'a>(e: &'a str, ss: &[Step]) -> &'a str { if e.is_empty() && ss.iter().any(|s| s.level == "auto") { "def" } else { step_word(e) } }
/// More (Up, Right) or less (Down, Left) effort from the step lit, stopping at the ends; a meter with nothing to pick leaves it.
fn step(c: &Cursor, ss: &[Step], more: bool) -> String {
    let on: Vec<&str> = ss.iter().filter(|s| s.on).map(|s| s.level.as_str()).collect();
    let Some(now) = lit(ss, &c.effort) else { return c.effort.clone() };
    let i = on.iter().position(|l| *l == now).unwrap_or(0);
    on[if more { (i + 1).min(on.len().saturating_sub(1)) } else { i.saturating_sub(1) }].to_string()
}

/// The row of `s` the cursor is on.
pub fn at(s: &Section, c: &Cursor) -> Option<usize> { s.choices.iter().position(|x| x.value == c.value(col_of(s.action))) }
/// What a key does to the cursor over the rows on show. Tab and Shift+Tab go round the three columns; Up and Down go round the rows of the agent and model columns
/// and step the effort in its own (Up is more); Left and Right step the effort from any column and stop at its ends.
pub fn key(k: Key, c: &Cursor, rows: &[Section], own: &Cursor) -> Cursor {
    let mut n = c.clone();
    let ss = steps(&levels(rows, &c.model));
    match k {
        Key::Tab => n.col = (c.col + 1) % COLS.len(),
        Key::BackTab => n.col = (c.col + COLS.len() - 1) % COLS.len(),
        Key::Up | Key::Down if c.col == 2 => n.effort = step(c, &ss, k == Key::Up),
        Key::Up | Key::Down => if let Some(s) = rows.iter().find(|s| col_of(s.action) == c.col).filter(|s| !s.choices.is_empty()) {
            let len = s.choices.len() as i32; let d = if k == Key::Down { 1 } else { -1 };
            let to = match at(s, &n) { Some(i) => (i as i32 + d).rem_euclid(len), None if d > 0 => 0, None => len - 1 };
            n.set(n.col, &s.choices[to as usize].value, own);
        },
        Key::Left | Key::Right => n.effort = step(c, &ss, k == Key::Right),
    }
    n
}
/// The search changed: the cursor goes to the first model it finds, which is what Enter then takes.
pub fn settle(c: &Cursor, rows: &[Section], own: &Cursor) -> Cursor {
    let mut n = c.clone();
    if let Some(m) = rows.iter().find(|s| s.action == "model").and_then(|s| s.choices.first()) { n.col = 1; n.set(1, &m.value, own); }
    n
}
/// The effort a cursor applies: the one lit (an effort the model does not take is the agent's default; none when it takes none).
pub fn effort_applied(c: &Cursor, rows: &[Section]) -> String { lit(&steps(&levels(rows, &c.model)), &c.effort).unwrap_or_else(|| c.effort.clone()) }
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
/// The model list's row that shows the cursor's model (the list holds model rows only).
pub fn child_of(rows: &[Section], c: &Cursor) -> Option<usize> { rows.iter().find(|s| s.action == "model").and_then(|s| at(s, c)) }
/// The keycap in a column's head, read from the column the keys are in.
pub fn cap(col: usize, here: usize) -> &'static str {
    if col == 2 { "←→" } else if col == here { "↑↓" } else if col == (here + 1) % COLS.len() { "Tab" } else { "⇧Tab" }
}

/// What the menu keeps between frames: the search box, the scroll of the model list, the cursor once a key moved it (with its thread: a cursor moved on another thread is not this one's).
#[derive(Default)]
pub struct Pick { pub search: Option<Entity<Composer>>, pub q: String, pub scroll: ScrollHandle, pub cur: Option<(String, Cursor)> }

/// The provider in force (the marked choice of the provider section), "" when the hub lists none.
pub fn provider_of(sections: &[Section]) -> String {
    sections.iter().find(|s| s.action == "provider").and_then(|s| s.choices.iter().find(|c| c.on)).map(|c| c.value.clone()).unwrap_or_default()
}
pub fn fav_key(provider: &str, value: &str) -> String { format!("{provider}/{value}") }

/// The model list's height: the same for every agent, so the picker does not jump as the cursor goes from one to another.
const LIST_H: f32 = 300.;
/// The model column shows at most this many rows (0.14's Models.cap): the search narrows the rest.
pub const CAP: usize = 50;
/// The model rows for a search text and the starred models of `provider`: a model stays when every word of the text is in its id or its label (any case); the starred
/// ones lead, the hub's order otherwise; at most `CAP`, with the cursor's model kept. The other columns are never filtered.
pub fn arrange(sections: &[Section], provider: &str, q: &str, favs: &[String], keep: &str) -> (Vec<Section>, usize) {
    let words: Vec<String> = q.trim().to_lowercase().split_whitespace().map(str::to_string).collect();
    let mut hidden = 0;
    let out = sections.iter().map(|s| {
        if s.action != "model" { return s.clone() }
        let mut ch: Vec<Choice> = s.choices.iter().filter(|c| { let hay = format!("{} {}", c.value, c.note).to_lowercase(); words.iter().all(|w| hay.contains(w.as_str())) }).cloned().collect();
        ch.sort_by_key(|c| !favs.iter().any(|f| *f == fav_key(provider, &c.value))); // stable: the rest keep the hub's order
        if ch.len() > CAP { hidden = ch.len() - CAP; let kept = ch.iter().skip(CAP).find(|c| c.value == keep).cloned(); ch.truncate(CAP); ch.extend(kept); }
        Section { choices: ch, ..s.clone() }
    }).collect();
    (out, hidden)
}
/// The menu as it stands: the thread's own cursor, the cursor, every row for its agent, the rows on show and how many models the cap left out.
pub struct Shown { pub own: Cursor, pub cur: Cursor, pub all: Vec<Section>, pub rows: Vec<Section>, pub hidden: usize }
pub fn shown(thread: &Value, pick: &Pick, favs: &[String]) -> Option<Shown> {
    let mine = parse(&thread["picker"]);
    if mine.is_empty() { return None }
    let own = Cursor::own(&mine); let id = string(thread, "id");
    let cur = pick.cur.as_ref().filter(|(t, _)| *t == id).map_or_else(|| own.clone(), |(_, c)| c.clone());
    let all = for_provider(&thread["picker"], &thread["pickerAgents"], &cur.provider);
    let (rows, hidden) = arrange(&all, &cur.provider, &pick.q, favs, &cur.model);
    Some(Shown { own, cur, all, rows, hidden })
}

/// The chip under the composer: the hub's words for the thread's agent, model and effort ("codex / gpt-6.1-sol / high"), and the agent for its mark.
pub fn chip_text(picker: &Value) -> (String, String) {
    let provider = { let p = string(picker, "provider"); if p.is_empty() { bare(&provider_of(&parse(picker))).to_string() } else { p } };
    let chip = string(picker, "chip");
    (if chip.is_empty() { format!("{provider} / default") } else { chip }, provider)
}

/// A provider's colour and fallback initial: a square of its colour with its initial. (The hub sends no artwork; these stand in.)
pub fn mark(provider: &str) -> (u32, &'static str) {
    match provider { "claude" => (0xd97757, "A"), "codex" => (0x10a37f, "O"), "grok" => (0x6b7280, "X"), "muse" => (0x8b5cf6, "M"), "opencode" => (0x2563eb, "C"),
        "pi" => (0xdb2777, "π"), "omp" => (0xc026d3, "Ω"), "antigravity" | "agy" => (0x4285f4, "G"), _ => (0x64748b, "?") }
}
pub fn logo(provider: &str) -> AnyElement {
    let provider = bare(provider);
    let (rgb_, letter) = mark(provider);
    if let Some(segs) = crate::art::mark_path(provider) { return crate::art::mark(segs, 16., rgb_).into_any_element(); }
    div().w(px(16.)).h(px(16.)).flex_shrink_0().flex().items_center().justify_center().bg(rgb(rgb_)).text_color(rgb(0xffffff)).text_size(px(10.)).font_weight(FontWeight::BOLD).child(letter).into_any_element()
}

/// The thumb of a scroll bar: (top, height) in px inside a track of `track` px, for a view `view` px tall holding `max + view` px, scrolled `y` (>= 0).
pub fn thumb(track: f32, view: f32, max: f32, y: f32) -> Option<(f32, f32)> {
    if max <= 0.5 || track <= 0. { return None }
    let h = (track * view / (view + max)).clamp(24., track); Some(((track - h) * (y / max).clamp(0., 1.), h))
}

/// The note under the meter: what the thread's agent runs at, or that the cursor's model takes no effort.
pub fn meter_note(ss: &[Step], provider: &str, own: &Cursor) -> String {
    if ss.iter().all(|s| !s.on) { format!("{} takes no effort setting", bare(provider)) } else { format!("Now: {}", step_word(&own.effort)) }
}

pub fn view(cx: &mut Context<Backplane>, light: bool, thread: &Value, open: bool, pick: &Pick, favs: &[String]) -> Option<AnyElement> {
    if !open { return None; }
    let c = |n: &str| theme::color(light, n);
    let Shown { cur, own, rows, hidden, .. } = shown(thread, pick, favs)?;
    let provider = cur.provider.clone();
    let head = |col: usize, title: &str| div().px_3().pt_2().pb_1().flex().justify_between().text_size(px(11.)).text_color(if col == cur.col { c("accent") } else { c("faint") }).child(title.to_uppercase()).child(cap(col, cur.col));
    // the agent column: each agent, what it is dim beside it; the cursor's row raised with the accent edge, the thread's own ticked
    let mut agents = div().id("picker-agents").w(px(236.)).flex_shrink_0().flex().flex_col().border_r_1().border_color(c("line")).child(head(0, "Agent"));
    for (ci, ch) in rows.iter().find(|s| s.action == "provider").map(|s| s.choices.clone()).unwrap_or_default().into_iter().enumerate() {
        let (here, value) = (ch.value == cur.provider, ch.value.clone());
        agents = agents.child(div().id(SharedString::from(format!("pick-a-{ci}"))).flex().items_center().gap_2().px_3().py_1().cursor_pointer().hover(|st| st.bg(c("raise")))
            .when(here, |d| d.bg(c("raise")).border_l_2().border_color(c("accent"))).when(!ch.ok, |d| d.opacity(0.55))
            .child(div().w(px(10.)).text_color(c("accent")).child(if ch.value == own.provider { "✓" } else { "" }))
            .child(logo(&ch.value)).child(div().when(here, |d| d.text_color(c("accent"))).child(ch.label.clone()))
            .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_size(px(11.)).text_color(c("faint")).child(ch.note.clone()))
            .on_click(cx.listener(move |this, _, _, cx| this.picker_press(0, &value, cx))));
    }
    // the model column: the search, then each model's name and what its CLI calls it
    let why = agent_why(&rows, &provider, &thread["pickerAgents"]);
    let models = rows.iter().find(|s| s.action == "model").map(|s| s.choices.clone()).unwrap_or_default();
    let namew = models.iter().map(|m| m.label.chars().count()).max().unwrap_or(7).clamp(7, 34) as f32 * 7.3 + 8.;
    let mut list = div().id("picker-list").track_scroll(&pick.scroll).h(px(LIST_H)).overflow_y_scroll().flex().flex_col().pb_1().pr(px(10.));
    if models.is_empty() { list = list.child(div().px_3().py_1().text_color(c("faint")).child(if !why.is_empty() { why.clone() } else if pick.q.trim().is_empty() { "No models".to_string() } else { "Nothing matches".to_string() })); }
    let own_agent = cur.provider == own.provider;
    for (ci, ch) in models.iter().enumerate() {
        let value = ch.value.clone();
        let fav = fav_key(&provider, &ch.value);
        let starred = favs.iter().any(|f| *f == fav);
        let here = ch.value == cur.model;
        let mut row = div().flex().items_center().hover(|st| st.bg(c("raise"))).when(here, |d| d.bg(c("raise")))
            .child(div().id(SharedString::from(format!("pick-m-{ci}"))).flex_1().min_w_0().flex().items_center().gap_2().px_3().py_1().cursor_pointer()
                .when(here && cur.col == 1, |d| d.border_l_2().border_color(c("accent")))
                .child(div().w(px(10.)).flex_shrink_0().text_color(c("accent")).child(if own_agent && ch.value == own.model { "✓" } else { "" }))
                .child(div().w(px(namew)).flex_shrink_0().overflow_hidden().whitespace_nowrap().when(here, |d| d.text_color(c("accent"))).child(ch.label.clone()))
                .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_size(px(11.)).text_color(c("faint")).child(ch.note.clone()))
                .on_click(cx.listener(move |this, _, _, cx| this.picker_press(1, &value, cx))));
        if !ch.value.is_empty() {
            let f2 = fav.clone();
            row = row.child(div().id(SharedString::from(format!("fav-{ci}"))).px_2().py_1().cursor_pointer().text_color(if starred { c("accent") } else { c("faint") })
                .child(if starred { "★" } else { "☆" })
                .on_click(cx.listener(move |this, _, _, cx| { this.local_press("fav", &f2, cx); })));
        }
        list = list.child(row);
    }
    if hidden > 0 { list = list.child(div().px_3().py_1().text_size(px(11.)).text_color(c("faint")).child(format!("{hidden} more: search to find them"))); }
    let (view_h, max, y) = (f32::from(pick.scroll.bounds().size.height), f32::from(pick.scroll.max_offset().height), -f32::from(pick.scroll.offset().y));
    let bar = thumb(view_h, view_h, max, y).map(|(top, h)| div().absolute().top(px(top)).right(px(2.)).w(px(6.)).h(px(h)).bg(c("faint")).opacity(0.7));
    let search = pick.search.clone().map(|s| div().mx_3().mb_1().px_2().py_1().border_1().border_color(c("line")).flex().items_center().gap_2().child(div().text_color(c("faint")).child("⌕")).child(div().flex_1().child(s)));
    let model_col = div().flex_1().min_w_0().flex().flex_col().child(head(1, "Model")).children(search).child(div().relative().child(list).children(bar));
    // the effort column: a bar a step, higher to the right, the chosen one lit and those below it filled; a step the model does not take drawn disabled
    let ss = steps(&levels(&rows, &cur.model));
    let on_now = lit(&ss, &cur.effort);
    let below = |i: usize| on_now.as_ref().and_then(|l| ss.iter().position(|s| &s.level == l)).map_or(false, |at| i < at);
    let mut meter = div().flex().items_end().gap(px(6.)).px_3().pt_2();
    for (i, s) in ss.iter().enumerate() {
        let (litd, fill, level, ok) = (on_now.as_deref() == Some(s.level.as_str()), below(i), s.level.clone(), s.on);
        let tone = if litd { c("accent") } else if fill { c("dim") } else { c("line") };
        meter = meter.child(div().id(SharedString::from(format!("pick-e-{i}"))).flex().flex_col().items_center().gap_1().when(ok, |d| d.cursor_pointer()).when(!ok, |d| d.opacity(0.35))
            .child(div().w(px(14.)).h(px(6. + 5. * i as f32)).bg(tone))
            .child(div().text_size(px(10.)).text_color(if litd { c("accent") } else { c("faint") }).child(word(&s.level, &ss).to_string()))
            .when(ok, |d| d.on_click(cx.listener(move |this, _, _, cx| this.picker_press(2, &level, cx)))));
    }
    let effort_col = div().w(px(250.)).flex_shrink_0().flex().flex_col().border_l_1().border_color(c("line")).child(head(2, "Effort")).child(meter)
        .child(div().px_3().pt_2().text_size(px(11.)).text_color(c("faint")).child(meter_note(&ss, &cur.provider, &own)));
    Some(div().id("picker").flex().items_start().border_1().border_color(c("line")).bg(c("panel")).text_size(px(12.)).child(agents).child(model_col).child(effort_col).into_any_element())
}

impl Backplane {
    /// The model chip or its key: open or shut the menu; opening clears the search and puts the keyboard in it.
    pub(crate) fn picker_toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.screens.picker_open { self.picker_close(cx); return; }
        self.picker_show(window, cx);
    }
    /// Open the menu (screens/combo.rs) with the keyboard in the model search.
    pub(crate) fn picker_show(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        self.screens.picker_open = false; self.screens.acct.menu = false; self.screens.pick.cur = None; self.want_focus = true; cx.notify();
    }
    fn picker_shown(&self) -> Option<Shown> { shown(&self.data["thread"], &self.screens.pick, &self.local.favs) }
    fn picker_store(&mut self, c: Cursor) { self.screens.pick.cur = Some((string(&self.data["thread"], "id"), c)); self.picker_reveal(); }
    /// Scroll the cursor's model into view.
    fn picker_reveal(&mut self) {
        let Some(s) = self.picker_shown() else { return };
        if let Some(ix) = child_of(&s.rows, &s.cur) { self.screens.pick.scroll.scroll_to_item(ix); }
        self.m("pick", serde_json::json!({"col": COLS[s.cur.col], "provider": s.cur.provider, "model": s.cur.model, "effort": s.cur.effort, "child": child_of(&s.rows, &s.cur), "y": -f32::from(self.screens.pick.scroll.offset().y),
            "models": s.rows.iter().find(|x| x.action == "model").map_or(0, |x| x.choices.len()), "hidden": s.hidden}));
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
    /// Enter: the cursor applies and the menu shuts; with no model on show it asks for nothing.
    pub(crate) fn picker_enter(&mut self, cx: &mut Context<Self>) {
        if let Some(s) = self.picker_shown() { if s.rows.iter().any(|x| x.action == "model" && !x.choices.is_empty()) { self.picker_apply(s.cur, &s.all, &s.own); } }
        self.picker_close(cx);
    }
    /// A row pressed (0 agent, 1 model, 2 effort): the cursor moves there and applies at once; the menu stays open (0.14's Mpk.click).
    fn picker_press(&mut self, col: usize, value: &str, cx: &mut Context<Self>) {
        if let Some(s) = self.picker_shown() {
            let mut c = s.cur.clone(); c.set(col, value, &s.own); c.col = col;
            let all = for_provider(&self.data["thread"]["picker"], &self.data["thread"]["pickerAgents"], &c.provider);
            self.picker_apply(c.clone(), &all, &s.own);
            self.picker_store(c);
        }
        cx.notify();
    }
    /// Apply a cursor: an agent that cannot be used here is only shown (its column says why), never switched to.
    fn picker_apply(&self, mut c: Cursor, all: &[Section], own: &Cursor) {
        if !agent_ok(all, &c.provider) { return }
        c.effort = effort_applied(&c, all);
        for cmd in plan(&c, own) { let _ = self.tx.try_send(cmd); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn fixture() -> (Value, Value) {
        let picker = serde_json::json!({"provider": "claude", "chip": "claude / opus / high",
            "providers": [{"label": "claude", "value": "@claude", "on": true, "note": "Claude Code", "ok": true}, {"label": "codex", "value": "@codex", "note": "Codex CLI", "ok": true}, {"label": "grok", "value": "@grok", "ok": false, "note": "not installed on this machine"}],
            "models": [{"label": "default", "value": "", "note": "Claude Code's own default"}, {"label": "opus", "value": "opus", "on": true, "note": "Opus 5.5 · claude-opus-5-5"}, {"label": "haiku", "value": "haiku", "efforts": []}, {"label": "sonnet", "value": "sonnet"}],
            "efforts": [{"label": "auto", "value": ""}, {"label": "low", "value": "low"}, {"label": "high", "value": "high", "on": true}]});
        let agents = serde_json::json!({"codex": {"ok": true, "models": [{"label": "default", "value": ""}, {"label": "gpt", "value": "gpt", "note": "GPT · fast", "efforts": ["", "low", "medium"]}], "efforts": [{"label": "auto", "value": ""}, {"label": "low", "value": "low"}, {"label": "xhigh", "value": "xhigh"}]},
            "grok": {"ok": false, "why": "not installed on this machine", "models": [], "efforts": []}});
        (picker, agents)
    }
    fn acts(v: Vec<Command>) -> Vec<(String, String)> { v.into_iter().map(|c| match c { Command::Action(a, v) => (a, v), c => panic!("{c:?}") }).collect() }
    fn pair(a: &str, v: &str) -> (String, String) { (a.to_string(), v.to_string()) }
    fn lv(xs: &[&str]) -> Vec<String> { xs.iter().map(|x| x.to_string()).collect() }
    #[test] fn sections_in_tab_order_with_the_marked_choice_notes_and_empty_ones_dropped() {
        let p: Value = serde_json::from_str(r#"{"label":"x","models":[{"label":"Opus","value":"opus","on":true,"note":"Opus 5.5"},{"label":"Haiku","value":"haiku","on":false}],"efforts":[{"label":"auto","value":"","on":true}],"providers":[]}"#).unwrap();
        let s = parse(&p);
        assert_eq!(s.iter().map(|x| x.action).collect::<Vec<_>>(), ["model", "effort"]);
        assert!(s[0].choices[0].on && !s[0].choices[1].on); assert_eq!(s[0].choices[0].note, "Opus 5.5"); assert!(s[0].choices[1].ok, "ok unless the hub says otherwise");
        assert!(parse(&Value::Null).is_empty());
        let (p, _) = fixture(); assert_eq!(parse(&p).iter().map(|x| x.action).collect::<Vec<_>>(), COLS); assert_eq!(parse(&p)[1].choices[2].efforts, Some(vec![]));
    }
    #[test] fn search_finds_every_word_in_the_id_or_label_starred_lead_and_the_other_columns_stay() {
        let (p, _) = fixture(); let s = parse(&p);
        let (a, n) = arrange(&s, "@claude", "", &["@claude/sonnet".to_string()], ""); assert_eq!(n, 0);
        assert_eq!(a[1].choices.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(), ["sonnet", "", "opus", "haiku"]);
        let (b, _) = arrange(&s, "@claude", "OPUS 5.5", &[], ""); assert_eq!(b[1].choices.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(), ["opus"], "words in the label count");
        assert_eq!(b[0].choices.len(), 3, "the agents are never filtered"); assert_eq!(b[2].choices.len(), 3);
        assert!(arrange(&s, "@claude", "zzz", &[], "").0[1].choices.is_empty());
    }
    #[test] fn the_model_column_shows_at_most_the_cap_and_keeps_the_cursors_model() {
        let ms: Vec<Value> = (0..120).map(|i| serde_json::json!({"label": format!("m{i}"), "value": format!("m{i}")})).collect();
        let s = parse(&serde_json::json!({"models": ms, "providers": [{"label": "pi", "value": "@pi", "on": true}]}));
        let (a, hidden) = arrange(&s, "@pi", "", &[], "m99"); assert_eq!(hidden, 70); assert_eq!(a[1].choices.len(), CAP + 1); assert_eq!(a[1].choices.last().unwrap().value, "m99");
        let (a, hidden) = arrange(&s, "@pi", "m11", &[], ""); assert_eq!((hidden, a[1].choices.len()), (0, 11), "m11, m110..m119");
    }
    #[test] fn the_cursor_starts_on_the_threads_own_in_the_model_column() {
        let (p, _) = fixture(); let c = Cursor::own(&parse(&p));
        assert_eq!(c, Cursor { col: 1, provider: "@claude".into(), model: "opus".into(), effort: "high".into() });
    }
    #[test] fn up_and_down_go_round_the_agent_and_model_columns_and_step_the_effort() {
        let (p, _) = fixture(); let s = parse(&p); let own = Cursor::own(&s);
        let c = key(Key::Down, &own, &s, &own); assert_eq!(c.model, "haiku");
        let c = key(Key::Down, &c, &s, &own); let c = key(Key::Down, &c, &s, &own); assert_eq!(c.model, "", "past the last it wraps to the first");
        let c = key(Key::Up, &c, &s, &own); assert_eq!(c.model, "sonnet", "and back up over the top");
        let mut e = own.clone(); e.col = 2; assert_eq!(key(Key::Down, &e, &s, &own).effort, "low", "down is less effort"); assert_eq!(key(Key::Up, &e, &s, &own).effort, "high", "the top stays");
    }
    #[test] fn tab_goes_round_the_three_columns() {
        let (p, _) = fixture(); let s = parse(&p); let own = Cursor::own(&s);
        let c = key(Key::Tab, &own, &s, &own); assert_eq!(c.col, 2, "model to effort");
        let c = key(Key::Tab, &c, &s, &own); assert_eq!(c.col, 0, "effort to agent");
        assert_eq!(key(Key::BackTab, &c, &s, &own).col, 2); assert_eq!(key(Key::BackTab, &own, &s, &own).col, 0, "shift+tab: model to agent");
        assert_eq!((own.model.as_str(), key(Key::Tab, &own, &s, &own).model.as_str()), ("opus", "opus"), "Tab moves no choice");
    }
    #[test] fn left_and_right_step_the_effort_the_model_takes_and_stop_at_its_ends() {
        let (p, _) = fixture(); let s = parse(&p); let own = Cursor::own(&s);
        assert_eq!(key(Key::Right, &own, &s, &own).effort, "high", "no wrapping at the top");
        let c = key(Key::Left, &own, &s, &own); assert_eq!((c.effort.as_str(), c.col), ("low", 1), "the column stays");
        let c = key(Key::Left, &c, &s, &own); assert_eq!(c.effort, ""); assert_eq!(key(Key::Left, &c, &s, &own).effort, "");
        let h = Cursor { model: "haiku".into(), ..own.clone() }; assert_eq!(key(Key::Left, &h, &s, &own).effort, "high", "a model that takes no effort leaves it");
    }
    #[test] fn the_meter_draws_the_common_steps_and_the_agents_own_and_lights_what_applies() {
        let ss = steps(&lv(&["", "low", "high"]));
        assert_eq!(ss.iter().map(|s| (s.level.as_str(), s.on)).collect::<Vec<_>>(), [("", true), ("low", true), ("medium", false), ("high", true), ("xhigh", false), ("max", false)]);
        assert_eq!(lit(&ss, "high").as_deref(), Some("high")); assert_eq!(lit(&ss, "max").as_deref(), Some(""), "a level the model does not take: its default");
        let omp = steps(&lv(&["", "off", "auto", "minimal", "low"])); assert_eq!(omp.iter().map(|s| s.level.as_str()).collect::<Vec<_>>(), ["", "off", "auto", "minimal", "low", "medium", "high", "xhigh", "max"]);
        let none = steps(&[]); assert!(none.iter().all(|s| !s.on)); assert_eq!(lit(&none, "high"), None); assert_eq!(none.len(), 6, "drawn, all disabled");
        assert_eq!(steps(&lv(&["", "turbo"])).last().unwrap().level, "turbo", "a level off the scale goes last");
        let (p, a) = fixture(); let rows = for_provider(&p, &a, "@codex");
        assert_eq!(levels(&rows, "gpt"), lv(&["", "low", "medium"]), "the model's own levels"); assert_eq!(levels(&rows, ""), lv(&["", "low", "xhigh"]), "else its agent's");
        let mut h = p.clone(); h["models"][1]["on"] = Value::Bool(false); h["models"][2]["on"] = Value::Bool(true); h["efforts"] = serde_json::json!([]);
        let a2 = serde_json::json!({"claude": {"efforts": [{"value": ""}, {"value": "low"}, {"value": "high"}]}});
        let own_rows = for_provider(&h, &a2, "@claude"); assert_eq!(levels(&own_rows, "haiku"), lv(&[]), "on haiku, which takes none"); assert_eq!(levels(&own_rows, "opus"), lv(&["", "low", "high"]), "opus takes the agent's");
        assert_eq!(Cursor::own(&parse(&h)).model, "haiku");
        assert_eq!((word("", &omp), word("auto", &omp), word("", &ss)), ("def", "auto", "auto"), "two autos are told apart");
        assert_eq!(meter_note(&none, "@grok", &Cursor::own(&parse(&p))), "grok takes no effort setting"); assert_eq!(step_word("medium"), "med"); assert_eq!(step_word(""), "auto");
    }
    #[test] fn another_agent_shows_its_own_models_and_starts_on_its_default() {
        let (p, a) = fixture(); let s = parse(&p); let own = Cursor::own(&s);
        let mut c = own.clone(); c.col = 0;
        let c = key(Key::Down, &c, &s, &own); assert_eq!((c.provider.as_str(), c.model.as_str(), c.effort.as_str()), ("@codex", "", "high"));
        let rows = for_provider(&p, &a, &c.provider);
        assert_eq!(rows[1].choices.iter().map(|x| (x.value.as_str(), x.note.as_str())).collect::<Vec<_>>(), [("", ""), ("gpt", "GPT · fast")]);
        assert!(rows[1].choices.iter().all(|x| !x.on) && rows[0].choices[0].on, "only what is in force is marked");
        assert_eq!(effort_applied(&c, &rows), "", "an effort its model does not take is its default");
        let g = for_provider(&p, &a, "@grok"); assert!(g[1].choices.is_empty(), "an agent that is not installed lists nothing"); assert_eq!(agent_why(&g, "@grok", &a), "not installed on this machine"); assert!(!agent_ok(&g, "@grok"));
        assert_eq!(for_provider(&p, &Value::Null, "@codex")[1].choices.len(), 1, "unknown lists: its default");
        let back = key(Key::Up, &c, &rows, &own); assert_eq!(back.model, "opus", "back on the thread's agent: the thread's model");
    }
    #[test] fn a_search_puts_the_cursor_on_the_first_model_it_finds() {
        let (p, _) = fixture(); let s = parse(&p); let own = Cursor::own(&s);
        let c = settle(&own, &arrange(&s, "@claude", "son", &[], "").0, &own); assert_eq!((c.col, c.model.as_str()), (1, "sonnet"));
        assert_eq!(settle(&own, &arrange(&s, "@claude", "zzz", &[], "").0, &own), own, "nothing matches: it stays");
        let c = settle(&own, &arrange(&s, "@claude", "o", &["@claude/sonnet".into()], "").0, &own); assert_eq!(c.model, "sonnet", "a star leads");
    }
    #[test] fn enter_asks_for_what_changed_and_another_agent_is_one_switch() {
        let (p, _) = fixture(); let own = Cursor::own(&parse(&p));
        assert!(plan(&own, &own).is_empty());
        let mut c = own.clone(); c.model = "sonnet".into(); assert_eq!(acts(plan(&c, &own)), [pair("model", "sonnet")]);
        c.effort = "".into(); assert_eq!(acts(plan(&c, &own)), [pair("model", "sonnet"), pair("effort", "")]);
        let d = Cursor { col: 0, provider: "@codex".into(), model: "gpt".into(), effort: "high".into() }; assert_eq!(acts(plan(&d, &own)), [pair("provider", "codex\u{1f}gpt")]);
        let d = Cursor { effort: "low".into(), ..d }; assert_eq!(acts(plan(&d, &own)), [pair("provider", "codex\u{1f}gpt\u{1f}low")]);
    }
    #[test] fn the_row_to_scroll_to_is_the_models_own() {
        let (p, _) = fixture(); let s = parse(&p); let own = Cursor::own(&s);
        assert_eq!(child_of(&s, &own), Some(1)); assert_eq!(child_of(&[], &own), None);
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
    #[test] fn the_chip_reads_the_hubs_words_and_its_agent() {
        let (p, _) = fixture(); assert_eq!(chip_text(&p), ("claude / opus / high".to_string(), "claude".to_string()));
        let old = serde_json::json!({"providers": [{"label": "codex", "value": "@codex", "on": true}]}); assert_eq!(chip_text(&old), ("codex / default".to_string(), "codex".to_string()), "a hub that sends no chip");
    }
    #[test] fn a_provider_value_names_its_mark() { assert_eq!(mark(bare("@claude")), mark("claude")); assert_eq!(bare("codex"), "codex"); }
    #[test] fn the_thumb_follows_the_scroll_and_hides_without_overflow() {
        assert_eq!(thumb(300., 300., 0., 0.), None);
        let (top, h) = thumb(300., 300., 300., 0.).unwrap(); assert_eq!((top, h), (0., 150.));
        let (top, h) = thumb(300., 300., 300., 300.).unwrap(); assert_eq!(top + h, 300.);
        assert!(thumb(300., 300., 1_000_000., 0.).unwrap().1 >= 24.);
    }
    #[test] fn vendor_marks_for_claude_and_opencode_only() { for p in ["claude", "opencode"] { assert!(crate::art::mark_path(p).map_or(false, |s| s.len() > 3), "{p}"); } for p in ["codex", "grok", "muse", "pi", "omp", "antigravity", "mystery"] { assert!(crate::art::mark_path(p).is_none(), "{p}"); } }
    #[test] fn paths_parse_relative_and_absolute() { use crate::art::{parse_d, Seg}; let s = parse_d("M22 24H2V0h20zM17 4.8H7v14.4h10z"); assert_eq!(s.len(), 10); assert!(matches!(s[3], Seg::L(x, y) if x == 22. && y == 0.)); assert!(matches!(s[8], Seg::L(x, y) if x == 17. && (y - 19.2).abs() < 1e-4)); }
    #[test] fn every_provider_has_a_mark() { for p in ["claude", "codex", "grok", "muse", "opencode", "pi", "omp", "antigravity", "mystery"] { assert!(!mark(p).1.is_empty()); } }
}
