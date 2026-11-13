//! The window before there is a hub: choose its home, or connect to a hub on another machine. When a hub answers, this view gives the window to `Backplane`.
//! `auto`: a home is known already (a later launch, or `BACKPLANE_HOME`), so it only says it is starting the hub.
use super::composer::Composer;
use crate::{hub::Config, i18n::{tf, trs}, launch, theme, Backplane};
use gpui::{prelude::*, *};
use std::{path::PathBuf, time::Instant};

pub struct First {
    config: Config, started: Instant, light: bool,
    default_home: String, old: Option<PathBuf>,
    path: Entity<Composer>, addr: Entity<Composer>, code: Entity<Composer>,
    busy: String, error: String, other: bool,
}

/// Words a person reads before a hub exists; every_word below checks them with the same rules as setup.
/* i18n-ok: drawn through trs */ const TITLE: &str = "Where should Backplane keep your data?";
/* i18n-ok: drawn through trs */ const INTRO: &str = "Backplane keeps your threads, settings and keys in one folder on this computer. A background program, the hub, runs your agents from that folder. It keeps running when you close this window, so agents finish their work. The folder below is a good choice.";
/* i18n-ok: drawn through trs */ const USE: &str = "Use this folder";
/* i18n-ok: drawn through trs */ const PICK: &str = "Choose a folder";
/* i18n-ok: drawn through trs */ const OTHER: &str = "Connect to a hub on another computer";
/* i18n-ok: drawn through trs */ const OTHER_NOTE: &str = "A hub is the Backplane program on another computer. On that computer, open Settings, Devices, Add a device. Paste its pairing link here, or type its address and short code. Each code works once, for ten minutes.";
/* i18n-ok: drawn through trs */ const CONNECT: &str = "Connect";
/* i18n-ok: drawn through trs */ const BACK: &str = "Back";
/* i18n-ok: drawn through trs */ const HAS_DATA: &str = "This folder already has a hub's data. Backplane opens it as it is.";
/* i18n-ok: drawn through trs */ const OLD_NOTE: &str = "Backplane 0.14 data is in that folder. This version only reads it. Once the hub starts, setup offers to copy your projects, threads and bots into the folder you choose here.";
const GUIDE: &str = "https://github.com/BackplaneWorks/Backplane/blob/main/docs/onboarding.md";

impl First {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, config: Config, started: Instant, auto: Option<PathBuf>) -> Self {
        let input = |hint: &str, cx: &mut Context<Self>| { let hint = hint.to_string(); cx.new(move |cx| { let mut c = Composer::with_context(cx, "AskInput"); c.set_placeholder(&hint); c }) };
        let default_home = launch::default_here().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        let path = input("Folder", cx); let addr = input("Pairing link or address", cx); let code = input("Short code", cx);
        path.update(cx, |p, cx| p.set(default_home.clone(), cx));
        cx.observe(&path, |_, _, cx| cx.notify()).detach();
        let light = config.theme.as_deref() == Some("light");
        let old = launch::user_home().and_then(|u| launch::old_home(&u));
        let mut me = Self { config, started, light, default_home, old, path, addr, code, busy: String::new(), error: String::new(), other: false };
        if let Some(home) = auto { me.start(home, window, cx); }
        me
    }
    fn hand_over(config: Config, started: Instant, url: String, token: PathBuf, window: &mut Window, cx: &mut App) {
        let mut c = config; c.hub = url.replacen("http://", "ws://", 1); c.token_file = token; c.pending = false;
        window.replace_root(cx, |window, cx| Backplane::new(window, cx, c, started));
    }
    fn start(&mut self, home: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.busy = tf("gpui.first.starting", &[("home", home.display().to_string().into())]); self.error.clear();
        let (config, started) = (self.config.clone(), self.started);
        cx.spawn_in(window, async move |this, cx| {
            let h = home.clone();
            let got = cx.background_executor().spawn(async move { launch::ensure_hub(&h) }).await;
            match got {
                Ok((url, token)) => { let saved = launch::save(&launch::Saved { home: home.to_string_lossy().into_owned(), ..Default::default() }); eprintln!("GPUI_LOG {}", serde_json::json!({"launch_saved": saved.as_ref().err().map(|e| format!("{e:#}")).unwrap_or_default(), "path": launch::saved_path()})); let _ = cx.update(|window, cx| Self::hand_over(config, started, url, token, window, cx)); }
                Err(e) => { let _ = this.update(cx, |t, cx| { t.busy.clear(); t.error = format!("{e:#}"); cx.notify(); }); }
            }
        }).detach();
        cx.notify();
    }
    fn press_use(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.path.read(cx).content.to_string();
        let why = launch::home_problem(&text, self.old.as_deref());
        if !why.is_empty() { self.error = trs(why); cx.notify(); return }
        self.start(PathBuf::from(text.trim()), window, cx);
    }
    fn press_pick(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: None });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await { if let Some(p) = paths.first() { let t = p.to_string_lossy().into_owned(); let _ = this.update(cx, |me, cx| { me.path.update(cx, |i, cx| i.set(t, cx)); cx.notify(); }); } }
        }).detach();
    }
    fn press_connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (text, typed) = (self.addr.read(cx).content.to_string(), self.code.read(cx).content.to_string());
        let Some((addr, secret)) = launch::parse_hub(&text) else { self.error = trs("Give the other computer's address, for example desktop.example.ts.net:3800."); cx.notify(); return };
        let code = secret.unwrap_or(typed);
        if code.trim().is_empty() { self.error = trs("Type the short code from the other computer."); cx.notify(); return }
        self.busy = tf("gpui.first.pairing", &[("addr", addr.as_str().into())]); self.error.clear();
        // a paired credential: when the hub later refuses it (revoked in its Devices), the window says so and offers pairing again (hub.rs `refused`)
        let (config, started) = (Config { paired: true, ..self.config.clone() }, self.started);
        cx.spawn_in(window, async move |this, cx| {
            let got = cx.background_executor().spawn(async move { launch::pair(&addr, &code) }).await;
            match got {
                Ok((url, token)) => { let _ = launch::save(&launch::Saved { url: url.clone(), token_file: token.to_string_lossy().into_owned(), ..Default::default() }); let _ = cx.update(|window, cx| Self::hand_over(config, started, url, token, window, cx)); }
                Err(e) => { let _ = this.update(cx, |t, cx| { t.busy.clear(); t.error = format!("{e:#}"); cx.notify(); }); }
            }
        }).detach();
        cx.notify();
    }
}

impl Render for First {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let light = self.light; let c = |n: &str| theme::color(light, n);
        let button = |id: &'static str, label: &str, primary: bool| { let d = div().id(id).h(crate::scale::tpx(30.)).px_4().flex().items_center().text_size(crate::scale::tpx(13.)).cursor_pointer(); report_rect((if primary { d.bg(c("accent")).text_color(c("on-accent")) } else { d.bg(c("raise")).hover(|s| s.bg(c("line"))) }).child(trs(label)), id) };
        let field = |e: &Entity<Composer>| div().h(crate::scale::tpx(32.)).px_3().flex().items_center().bg(c("raise")).child(e.clone());
        let mut col = div().flex().flex_col().gap_3().w(px(620.));
        if !self.busy.is_empty() { col = col.child(div().text_size(crate::scale::tpx(14.)).child(self.busy.clone())); }
        else if self.other {
            col = col.child(div().text_size(crate::scale::tpx(20.)).font_weight(FontWeight::BOLD).child(trs(OTHER))).child(div().text_size(crate::scale::tpx(13.)).text_color(c("dim")).child(trs(OTHER_NOTE)))
                .child(field(&self.addr)).child(field(&self.code))
                .child(div().flex().gap_2().child(button("first-connect", CONNECT, true).on_click(cx.listener(|t, _, w, cx| t.press_connect(w, cx)))).child(button("first-back", BACK, false).on_click(cx.listener(|t, _, _, cx| { t.other = false; t.error.clear(); cx.notify(); }))));
        } else {
            let typed = self.path.read(cx).content.to_string();
            col = col.child(div().text_size(crate::scale::tpx(20.)).font_weight(FontWeight::BOLD).child(trs(TITLE))).child(div().text_size(crate::scale::tpx(13.)).text_color(c("dim")).child(trs(INTRO))).child(field(&self.path));
            if launch::has_data(std::path::Path::new(typed.trim())) { col = col.child(div().text_size(crate::scale::tpx(12.)).text_color(c("dim")).child(trs(HAS_DATA))); }
            /* i18n-ok: drawn through trs */ if let Some(o) = &self.old { col = col.child(div().flex().flex_col().gap_1().p_3().border_1().border_color(c("line")).child(div().text_size(crate::scale::tpx(12.)).child(o.to_string_lossy().into_owned())).child(div().text_size(crate::scale::tpx(12.)).text_color(c("dim")).child(trs(OLD_NOTE))).child(div().flex().child(button("first-guide", "Install guide", false).on_click(cx.listener(|_, _, _, cx| cx.open_url(GUIDE)))))); }
            col = col.child(div().flex().gap_2().child(button("first-use", USE, true).on_click(cx.listener(|t, _, w, cx| t.press_use(w, cx)))).child(button("first-pick", PICK, false).on_click(cx.listener(|t, _, _, cx| t.press_pick(cx)))).child(button("first-other", OTHER, false).on_click(cx.listener(|t, _, _, cx| { t.other = true; t.error.clear(); cx.notify(); }))));
            let _ = &self.default_home;
        }
        if !self.error.is_empty() { col = col.child(div().text_size(crate::scale::tpx(12.)).text_color(c("bad")).child(self.error.clone())); }
        div().size_full().flex().items_center().justify_center().bg(c("bg")).text_color(c("text")).child(col)
    }
}

/// With BACKPLANE_GPUI_RECTS=1, a button logs its rectangle as a GPUI_RECT line (test/onboarding/pos.py reads it). This window has no script runner of its own.
fn report_rect(e: Stateful<Div>, id: &'static str) -> Stateful<Div> {
    if !crate::art::rects_on() { return e }
    e.relative().child(canvas(|_, _, _| (), move |b: Bounds<Pixels>, _, w: &mut Window, _| crate::art::report(id, b, w.scale_factor())).absolute().top_0().left_0().size_full())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn every_word_follows_unslop() {
        for t in [TITLE, INTRO, USE, PICK, OTHER, OTHER_NOTE, CONNECT, BACK, HAS_DATA, OLD_NOTE] {
            assert!(!t.contains(['\u{2014}', '\u{2013}', '!', '(']), "{t}");
            for s in t.split(". ") { assert!(s.split_whitespace().count() <= 40, "{s}"); }
        }
    }
}
