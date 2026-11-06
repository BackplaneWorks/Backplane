//! The model / effort / provider picker under the composer: the hub's `thread.picker` lists (models, efforts,
//! providers) each with the one in force marked; a press is the view action `model`, `effort` or `provider` with the
//! choice's value, as on the phones. Nothing is decided here.
use crate::{hub::Command, string, theme, Backplane};
use super::composer::Composer;
use gpui::{prelude::*, *};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Choice { pub label: String, pub value: String, pub on: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct Section { pub title: &'static str, pub action: &'static str, pub choices: Vec<Choice> }

pub fn parse(picker: &Value) -> Vec<Section> {
    let list = |key: &str| -> Vec<Choice> {
        picker[key].as_array().into_iter().flatten().map(|c| Choice { label: string(c, "label"), value: string(c, "value"), on: c["on"].as_bool().unwrap_or(false) }).collect()
    };
    [("Model", "model", "models"), ("Effort", "effort", "efforts"), ("Provider", "provider", "providers")].into_iter()
        .map(|(title, action, key)| Section { title, action, choices: list(key) }).filter(|s| !s.choices.is_empty()).collect()
}
pub fn press(s: &Section, c: &Choice) -> Command { Command::Action(s.action.to_string(), c.value.clone()) }

/// What the menu keeps between frames: the search box, the scroll of the list.
#[derive(Default)]
pub struct Pick { pub search: Option<Entity<Composer>>, pub q: String, pub scroll: ScrollHandle }

/// The provider in force (the marked choice of the provider section), "" when the hub lists none.
pub fn provider_of(sections: &[Section]) -> String {
    sections.iter().find(|s| s.action == "provider").and_then(|s| s.choices.iter().find(|c| c.on)).map(|c| c.value.clone()).unwrap_or_default()
}
pub fn fav_key(provider: &str, value: &str) -> String { format!("{provider}/{value}") }

/// The sections for a search text and the starred models: a choice stays when its label or value holds the text (any case); the models the person starred come first, in the hub's order otherwise.
pub fn arrange(sections: &[Section], q: &str, favs: &[String]) -> Vec<Section> {
    let provider = provider_of(sections); let q = q.trim().to_lowercase();
    sections.iter().filter_map(|s| {
        let mut ch: Vec<Choice> = s.choices.iter().filter(|c| q.is_empty() || c.label.to_lowercase().contains(&q) || c.value.to_lowercase().contains(&q)).cloned().collect();
        if s.action == "model" { ch.sort_by_key(|c| !favs.iter().any(|f| *f == fav_key(&provider, &c.value))); } // stable: the rest keep the hub's order
        (!ch.is_empty()).then(|| Section { choices: ch, ..s.clone() })
    }).collect()
}

/// A provider's mark: a square of its colour with its initial. (The hub sends no artwork; these stand in.)
pub fn mark(provider: &str) -> (u32, &'static str) {
    match provider { "claude" => (0xd97757, "A"), "codex" => (0x10a37f, "O"), "grok" => (0x6b7280, "X"), "muse" => (0x8b5cf6, "M"), "opencode" => (0x2563eb, "C"),
        "pi" => (0xdb2777, "π"), "omp" => (0xc026d3, "Ω"), "antigravity" | "agy" => (0x4285f4, "G"), _ => (0x64748b, "?") }
}
pub fn logo(provider: &str) -> Div {
    let (rgb_, letter) = mark(provider);
    div().w(px(16.)).h(px(16.)).flex_shrink_0().flex().items_center().justify_center().bg(rgb(rgb_)).text_color(rgb(0xffffff)).text_size(px(10.)).font_weight(FontWeight::BOLD).child(letter)
}

/// The thumb of a scroll bar: (top, height) in px inside a track of `track` px, for a view `view` px tall holding `max + view` px, scrolled `y` (>= 0).
pub fn thumb(track: f32, view: f32, max: f32, y: f32) -> Option<(f32, f32)> {
    if max <= 0.5 || track <= 0. { return None }
    let h = (track * view / (view + max)).clamp(24., track); Some(((track - h) * (y / max).clamp(0., 1.), h))
}

pub fn view(cx: &mut Context<Backplane>, light: bool, picker: &Value, open: bool, pick: &Pick, favs: &[String]) -> Option<AnyElement> {
    if !open { return None; }
    let c = |n: &str| theme::color(light, n);
    let all = parse(picker);
    if all.is_empty() { return None; }
    let provider = provider_of(&all);
    let sections = arrange(&all, &pick.q, favs);
    let cap = 320.;
    let mut col = div().id("picker-list").track_scroll(&pick.scroll).max_h(px(cap)).overflow_y_scroll().flex().flex_col().py_1().pr(px(10.));
    if sections.is_empty() { col = col.child(div().px_3().py_1().text_color(c("faint")).child("Nothing matches")); }
    for (si, s) in sections.iter().enumerate() {
        col = col.child(div().px_3().pt_1().text_size(px(11.)).text_color(c("faint")).child(s.title.to_uppercase()));
        for (ci, ch) in s.choices.iter().enumerate() {
            let (s2, c2) = (s.clone(), ch.clone());
            let fav = fav_key(&provider, &ch.value);
            let starred = s.action == "model" && favs.iter().any(|f| *f == fav);
            let mut row = div().flex().items_center().hover(|st| st.bg(c("raise")))
                .child(div().id(SharedString::from(format!("pick-{si}-{ci}"))).flex_1().min_w_0().flex().items_center().gap_2().px_3().py_1().cursor_pointer()
                    .child(div().w(px(14.)).text_color(c("accent")).child(if ch.on { "✓" } else { "" }))
                    .when(s.action == "provider", |d| d.child(logo(&ch.value)))
                    .child(ch.label.clone())
                    .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(press(&s2, &c2)); this.screens.picker_open = false; cx.notify(); })));
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
    /// The model chip: open or shut the menu; opening clears the search and puts the keyboard in it.
    pub(crate) fn picker_toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.screens.picker_open = !self.screens.picker_open;
        if self.screens.picker_open {
            if self.screens.pick.search.is_none() {
                let input = cx.new(|cx| Composer::with_context(cx, "PickInput"));
                input.update(cx, |i, _| i.set_placeholder("Search models"));
                cx.observe(&input, |this, input, cx| { this.screens.pick.q = input.read(cx).content.to_string(); cx.notify(); }).detach();
                // Enter in the box takes the first model that matches
                cx.subscribe(&input, |this, _, ev: &super::composer::ComposerEvent, cx| if matches!(ev, super::composer::ComposerEvent::Submit) { this.picker_first(cx); }).detach();
                self.screens.pick.search = Some(input);
            }
            let s = self.screens.pick.search.clone().unwrap();
            s.update(cx, |i, cx| i.set(String::new(), cx));
            self.screens.pick.q.clear();
            window.focus(&s.focus_handle(cx));
        }
        cx.notify();
    }
    fn picker_first(&mut self, cx: &mut Context<Self>) {
        let thread = self.data["thread"].clone();
        let sections = arrange(&parse(&thread["picker"]), &self.screens.pick.q, &self.local.favs);
        if let Some(s) = sections.first() { if let Some(c) = s.choices.first() { let _ = self.tx.try_send(press(s, c)); } }
        self.screens.picker_open = false; cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn sections_in_order_with_the_marked_choice_and_empty_ones_dropped() {
        let p: Value = serde_json::from_str(r#"{"label":"x","models":[{"label":"Opus","value":"opus","on":true},{"label":"Haiku","value":"haiku","on":false}],"efforts":[{"label":"auto","value":"","on":true}],"providers":[]}"#).unwrap();
        let s = parse(&p);
        assert_eq!(s.iter().map(|x| x.action).collect::<Vec<_>>(), ["model", "effort"]);
        assert!(s[0].choices[0].on && !s[0].choices[1].on);
        match press(&s[0], &s[0].choices[1]) { Command::Action(a, v) => assert_eq!((a.as_str(), v.as_str()), ("model", "haiku")), c => panic!("{c:?}") }
        assert!(parse(&Value::Null).is_empty());
    }
    #[test] fn search_filters_and_starred_models_lead() {
        let p: Value = serde_json::from_str(r#"{"models":[{"label":"Opus","value":"opus","on":true},{"label":"Haiku","value":"haiku"},{"label":"Sonnet","value":"sonnet"}],"providers":[{"label":"Claude","value":"claude","on":true},{"label":"Codex","value":"codex"}]}"#).unwrap();
        let s = parse(&p);
        assert_eq!(provider_of(&s), "claude");
        let a = arrange(&s, "", &["claude/sonnet".to_string()]);
        assert_eq!(a[0].choices.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(), ["sonnet", "opus", "haiku"]);
        let b = arrange(&s, "HAI", &[]); assert_eq!(b[0].choices.len(), 1); assert_eq!(b.len(), 1, "providers that do not match are dropped");
        assert!(arrange(&s, "zzz", &[]).is_empty());
        assert_eq!(arrange(&s, "co", &[]).iter().map(|x| x.action).collect::<Vec<_>>(), ["provider"], "Codex matches by name");
    }
    #[test] fn the_thumb_follows_the_scroll_and_hides_without_overflow() {
        assert_eq!(thumb(300., 300., 0., 0.), None);
        let (top, h) = thumb(300., 300., 300., 0.).unwrap(); assert_eq!((top, h), (0., 150.));
        let (top, h) = thumb(300., 300., 300., 300.).unwrap(); assert_eq!(top + h, 300.);
        assert!(thumb(300., 300., 1_000_000., 0.).unwrap().1 >= 24.);
    }
    #[test] fn every_provider_has_a_mark() { for p in ["claude", "codex", "grok", "muse", "opencode", "pi", "omp", "antigravity", "mystery"] { assert!(!mark(p).1.is_empty()); } }
}
