//! The client's keyboard shortcuts, ported from the Bend-era web client (src/web/workspace-keys.js on the tag `main-bend-final`).
//! One table (`TABLE`) feeds the key bindings, the palette's hints, the tooltips and the Settings list. `secondary` is Cmd on macOS and Ctrl elsewhere.
//! Bare keys (`/`, the rail's letters) are bound only outside every text field and viewer (`FREE`), so typing is never taken.
use crate::{hub::Command, local::Local, theme, Backplane};
use gpui::{actions, prelude::*, *};
use serde_json::Value;

actions!(keys, [NewThread, OpenDiff, OpenFiles, StopTurn, ModelPicker, PrevThread, NextThread, UberChat, SettleThread, FocusComposer, CycleLayers,
    Project1, Project2, Project3, Project4, Project5, Project6, Project7, Project8, Project9,
    Thread1, Thread2, Thread3, Thread4, Thread5, Thread6, Thread7, Thread8, Thread9,
    RailBoard, RailSchematic, Rail3d, RailSplit, RailMech, RailFiles, RailDiff, RailReview, RailBrowser]);

/// Where no text field, file list or viewer has the keyboard: a bare key may act.
pub const FREE: &str = "!Composer && !TextInput && !PickInput && !AskInput && !ImpInput && !Viewer && !BrowserBar && !FilesPage";
/// No text field has the keyboard (so a chord that edits text there is the field's: Option+digit types a character on a Mac, Ctrl+C copies).
const NOT_MAC_COMPOSER: &str = "!Composer && !TextInput && !PickInput && !AskInput && !ImpInput";
/// Not in any field or web page: Ctrl+H deletes a character in many text fields (a Mac's among them) and opens a web page's history, so there it stays theirs. It works from the plot, and with nothing focused.
const NOT_TEXT: &str = "!Composer && !TextInput && !PickInput && !AskInput && !ImpInput && !BrowserBar && !BrowserView && !CefBar && !CefPage";

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind { Legacy, Rebind, Fixed }
/// id, label, scope, default keystroke (GPUI syntax; display-only text for a group), kind.
pub struct Entry { pub id: &'static str, pub label: &'static str, pub scope: &'static str, pub key: &'static str, pub kind: Kind }
const fn e(id: &'static str, label: &'static str, scope: &'static str, key: &'static str, kind: Kind) -> Entry { Entry { id, label, scope, key, kind } }
use Kind::*;
pub const TABLE: &[Entry] = &[
    e("send", "Send the message", "Composer", "enter", Legacy),
    e("newline", "New line in the message", "Composer", "shift-enter", Fixed),
    e("send-alt", "Send (also)", "Composer", "ctrl-enter", Fixed),
    e("focus", "Put the keyboard in the message box", "Composer", "/", Fixed),
    e("models", "Agent, model and effort", "Composer", "ctrl-m", Rebind),
    e("close", "Close a popup, dialog or menu", "Dialogs", "escape", Legacy),
    e("palette", "Commands: jump to a thread or run a command", "Palette", "secondary-k", Rebind),
    e("palette-alt", "Commands (also)", "Palette", "secondary-shift-p", Fixed),
    e("sidebar", "Fold or unfold the sidebar", "Sidebar", "secondary-b", Legacy),
    e("search", "Search threads", "Sidebar", "secondary-shift-f", Legacy),
    e("projects", "Go to project 1…9", "Sidebar", "alt-1…9", Fixed),
    e("threads", "Go to thread 1…9 of the project", "Sidebar", "secondary-1…9", Fixed),
    e("prev", "Previous thread", "Sidebar", "secondary-shift-[", Rebind),
    e("next", "Next thread", "Sidebar", "secondary-shift-]", Rebind),
    e("uber", "Uberbot chat", "Sidebar", "secondary-shift-u", Rebind),
    e("new", "New thread", "Timeline", "secondary-n", Rebind),
    e("settle", "Settle or unsettle the thread", "Timeline", "secondary-shift-s", Rebind),
    e("stop", "Stop the running turn", "Timeline", "ctrl-.", Rebind),
    e("diff", "Diff", "Viewer", "secondary-d", Rebind),
    e("files", "Files", "Viewer", "secondary-p", Rebind),
    e("layers", "Layer focus: the other layers as they are, dimmed, hidden", "Viewer", "ctrl-h", Rebind),
    e("pad-undo", "Undo the last stroke or change", "Drawing pad", "secondary-z", Fixed),
    e("pad-delete", "Delete the selection, or clear the pad", "Drawing pad", "delete", Fixed),
    e("pad-delete-alt", "Delete (also)", "Drawing pad", "backspace", Fixed),
    e("rail", "Rail views: B board, S schematic, 3 3D, V split, M mech, F files, D diff, R review, W browser", "Viewer", "b s 3 v m f d r w", Fixed),
];
/// Keys the old client had that this one cannot do yet (no equivalent): listed, not faked.
pub const MISSING: &[(&str, &str)] = &[
    ("Ctrl+J, Backtick, Shift+Esc", "Floating terminal: the client has no terminal"),
    ("Ctrl+Shift+C / V", "Terminal copy and paste: no terminal"),
    ("Ctrl+Shift+H", "Performance overlay: the client has none"),
    ("Arrows, Tab in the model picker", "Picker cursor keys: the picker takes search and Enter only"),
];

pub fn find(id: &str) -> Option<&'static Entry> { TABLE.iter().find(|x| x.id == id) }
pub fn rebindable(id: &str) -> bool { find(id).map_or(false, |x| x.kind != Fixed) }
/// The keystrokes a shortcut can be set to, its default first (a pick, not free text: a bad keystroke would unbind the action).
pub fn choices(id: &str) -> Vec<String> {
    let Some(x) = find(id) else { return vec![] };
    let mut v = vec![x.key.to_string()];
    if x.kind == Legacy { v.extend(crate::local::KEYS.iter().map(|k| k.to_string())); }
    else if let Some(last) = x.key.rsplit('-').next().filter(|k| k.len() == 1 || x.key.ends_with("-[") || x.key.ends_with("-]")) { let l = if x.key.ends_with("-") { "-".into() } else { last.to_string() }; v.push(format!("ctrl-alt-{l}")); v.push(format!("alt-shift-{l}")); }
    v.dedup(); let mut seen = std::collections::HashSet::new(); v.retain(|k| seen.insert(k.clone())); v
}

/// "secondary-shift-n" as the person reads it: "Ctrl+Shift+N", or "⌘⇧N" on a Mac.
pub fn show(key: &str, mac: bool) -> String {
    let parts: Vec<&str> = if key.ends_with("--") { key[..key.len() - 2].split('-').chain(["-"]).collect() } else { key.split('-').collect() };
    let mut out: Vec<String> = vec![];
    for (i, p) in parts.iter().enumerate() {
        let last = i + 1 == parts.len();
        out.push(match (*p, mac) {
            ("secondary", true) => "⌘".into(), ("secondary" | "ctrl", false) => "Ctrl".into(), ("ctrl", true) => "⌃".into(),
            ("alt", true) => "⌥".into(), ("alt", false) => "Alt".into(), ("shift", true) => "⇧".into(), ("shift", false) => "Shift".into(),
            ("escape", _) => "Esc".into(), ("enter", _) => "Enter".into(), ("delete", _) => "Delete".into(), ("backspace", _) => "Backspace".into(),
            (k, _) if last && k.chars().count() == 1 => k.to_uppercase(), (k, _) => k.to_string(),
        });
    }
    if mac { out.concat() } else { out.join("+") }
}
pub fn mac() -> bool { cfg!(target_os = "macos") }
/// The key of a shortcut for the person to read, "" when it has none.
pub fn hint(l: &Local, id: &str) -> String { match find(id) { Some(x) if x.kind == Fixed => show(x.key, mac()), Some(_) => show(&l.key(id), mac()), None => String::new() } }
/// The shortcut a palette item or tool does, by the hub's action and value (`current`: the open thread's project, for New thread).
pub fn id_for(action: &str, value: &str, current: &str) -> &'static str {
    match (action, value) {
        ("new-thread", v) if !v.is_empty() && v == current => "new", ("diff", _) => "diff", ("interrupt", _) => "stop", ("settle" | "unsettle", _) => "settle",
        ("view-kind", "files") => "files", _ => "",
    }
}
/// A rail view's bare key.
pub fn rail_letter(kind: &str) -> &'static str { match kind { "board" => "b", "schematic" => "s", "3d" => "3", "split" => "v", "mech" => "m", "files" => "f", "review" => "r", "browser" => "w", _ => "" } }

/// Every binding of the client: the editing keys, then these shortcuts with this device's choices.
pub fn bindings(l: &Local) -> Vec<KeyBinding> {
    let mut v = crate::input::bindings();
    v.extend(crate::screens::composer::bindings());
    v.extend(crate::screens::improvements::bindings());
    let k = |id: &str| l.key(id);
    v.extend([KeyBinding::new(&k("send"), crate::Send, Some("Composer")), KeyBinding::new("ctrl-enter", crate::Send, Some("Composer")),
        KeyBinding::new(&k("close"), crate::ClosePopup, None), KeyBinding::new("secondary-c", crate::CopySelection, Some(NOT_MAC_COMPOSER)),
        KeyBinding::new(&k("sidebar"), crate::ToggleSidebarKey, None), KeyBinding::new(&k("search"), crate::FocusSearchKey, None),
        KeyBinding::new(&k("palette"), crate::OpenPaletteKey, None), KeyBinding::new("secondary-shift-p", crate::OpenPaletteKey, None),
        KeyBinding::new(&k("new"), NewThread, None), KeyBinding::new(&k("diff"), OpenDiff, None), KeyBinding::new(&k("files"), OpenFiles, None),
        KeyBinding::new(&k("stop"), StopTurn, None), KeyBinding::new(&k("models"), ModelPicker, None),
        KeyBinding::new(&k("prev"), PrevThread, None), KeyBinding::new(&k("next"), NextThread, None),
        KeyBinding::new(&k("uber"), UberChat, None), KeyBinding::new(&k("settle"), SettleThread, None), KeyBinding::new(&k("layers"), CycleLayers, Some(NOT_TEXT)),
        KeyBinding::new("/", FocusComposer, Some(FREE))]);
    let alt = if mac() { Some(NOT_MAC_COMPOSER) } else { None };
    macro_rules! digits { ($($n:literal $p:ident $t:ident),*) => { v.extend([$(KeyBinding::new(concat!("alt-", $n), $p, alt), KeyBinding::new(concat!("secondary-", $n), $t, None)),*]); } }
    digits!("1" Project1 Thread1, "2" Project2 Thread2, "3" Project3 Thread3, "4" Project4 Thread4, "5" Project5 Thread5, "6" Project6 Thread6, "7" Project7 Thread7, "8" Project8 Thread8, "9" Project9 Thread9);
    v.extend([KeyBinding::new("b", RailBoard, Some(FREE)), KeyBinding::new("s", RailSchematic, Some(FREE)), KeyBinding::new("3", Rail3d, Some(FREE)), KeyBinding::new("v", RailSplit, Some(FREE)),
        KeyBinding::new("m", RailMech, Some(FREE)), KeyBinding::new("f", RailFiles, Some(FREE)), KeyBinding::new("d", RailDiff, Some(FREE)), KeyBinding::new("r", RailReview, Some(FREE)),
        KeyBinding::new("w", RailBrowser, Some(FREE))]);
    v.extend(pad_bindings(&v, &k("close")));
    v
}
/// The drawing pad's keys, and every other shortcut muted while it is open, as the old pad took all the keys: nothing behind it moves under the person.
/// A binding without a context is as deep as the innermost context, so the mute names the length box's context too, and comes last to win the tie.
fn pad_bindings(all: &[KeyBinding], close: &str) -> Vec<KeyBinding> {
    use crate::screens::padview::{Delete, Undo};
    let mut v = vec![KeyBinding::new("secondary-z", Undo, Some("DrawPad")), KeyBinding::new("delete", Delete, Some("DrawPad")), KeyBinding::new("backspace", Delete, Some("DrawPad"))];
    let close = Keystroke::parse(close).map(|k| k.unparse()).unwrap_or_default();
    let own = ["composer::", "text_input::", "backplane::Send", "backplane::CopySelection"]; // the length box's editing keys, and keys only a text field or the page has
    let shortcuts = all.iter().filter(|b| !own.iter().any(|o| b.action().name().starts_with(o)));
    for key in shortcuts.filter_map(|b| b.keystrokes().first().map(|k| k.unparse())).filter(|k| *k != close) {
        v.extend(["DrawPad", "PadLength"].map(|ctx| KeyBinding::new(&key, NoAction, Some(ctx))));
    }
    v
}
pub fn bind_all(cx: &mut App, l: &Local) { cx.clear_key_bindings(); cx.bind_keys(bindings(l)); }

/// The sidebar's threads, project by project: (project id, thread ids).
pub fn sidebar(data: &Value) -> Vec<(String, Vec<String>)> {
    data["projects"].as_array().into_iter().flatten().map(|p| (crate::string(p, "id"), p["threads"].as_array().into_iter().flatten().map(|t| crate::string(t, "id")).collect())).collect()
}
fn raw(id: &str) -> &str { id.rsplit_once('|').map_or(id, |(_, r)| r) }
/// The raw id of the project holding thread `sel` (active or settled), "" for none.
pub fn project_of(data: &Value, sel: &str) -> String {
    for p in data["projects"].as_array().into_iter().flatten() {
        for t in p["threads"].as_array().into_iter().flatten().chain(p["settled"].as_array().into_iter().flatten()) { if raw(&crate::string(t, "id")) == sel { return raw(&crate::string(p, "id")).to_string(); } }
    }
    String::new()
}
/// The thread next to `sel` in the sidebar's order (the first, or the last going back, when `sel` is not in it).
pub fn neighbour(data: &Value, sel: &str, forward: bool) -> Option<String> {
    let all: Vec<String> = sidebar(data).into_iter().flat_map(|x| x.1).collect();
    if all.is_empty() { return None }
    let at = all.iter().position(|t| raw(t) == sel);
    let i = match (at, forward) { (Some(i), true) => i + 1, (Some(i), false) => i.checked_sub(1)?, (None, true) => 0, (None, false) => all.len() - 1 };
    all.get(i).cloned()
}
/// The `n`th (1-based) thread of the project holding `sel`, else of the project page open, else of the first project.
pub fn nth_thread(data: &Value, sel: &str, page: Option<&str>, n: usize) -> Option<String> {
    let mut pid = project_of(data, sel); if pid.is_empty() { pid = page.unwrap_or("").to_string(); }
    let rows = sidebar(data); let row = rows.iter().find(|(p, _)| raw(p) == pid).or(rows.first())?;
    row.1.get(n.checked_sub(1)?).cloned()
}
pub fn nth_project(data: &Value, n: usize) -> Option<String> { sidebar(data).get(n.checked_sub(1)?).map(|x| raw(&x.0).to_string()) }
fn has_tool(data: &Value, action: &str) -> bool { data["thread"]["tools"].as_array().into_iter().flatten().any(|t| t["action"] == action) }

struct Tip(SharedString, bool);
impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let c = |n: &str| theme::color(self.1, n);
        div().px_2().py_1().bg(c("panel")).border_1().border_color(c("line")).text_size(px(11.)).text_color(c("text")).child(self.0.clone())
    }
}
/// A tooltip builder for `.tooltip(...)`: `text`, then its key when it has one.
pub fn tip(text: &str, key: &str, light: bool) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let t: SharedString = if key.is_empty() { text.to_string() } else { format!("{text} · {key}") }.into();
    move |_, cx| cx.new(|_| Tip(t.clone(), light)).into()
}

impl Backplane {
    /// A rail button pressed, or its key: the viewer's kind (or Diff) is asked for.
    pub(crate) fn rail_press(&mut self, a: &str, v: &str, cx: &mut Context<Self>) {
        let cmd = if a == "view-kind" { self.screens.rail_pending = Some(v.to_string()); self.screens.rail_at = Some(std::time::Instant::now()); self.screens.plot_click = matches!(v, "board" | "schematic").then(std::time::Instant::now); crate::screens::viewer::view_kind(v) } else { Command::Action(a.into(), v.into()) };
        let _ = self.tx.try_send(cmd); cx.notify();
    }
    /// A bare key does nothing while something covers the page.
    fn covered(&self) -> bool { self.screens.palette.open || self.screens.lightbox.is_some() || self.screens.picker_open || self.screens.acct.menu || crate::screens::settings::is_open(self) }
    fn key_rail(&mut self, a: &str, v: &str, cx: &mut Context<Self>) { if !self.covered() && self.data["thread"].is_object() { self.rail_press(a, v, cx); } }
    /// Ctrl+H: the plot on show cycles its focus; on Split, the pane with the keyboard (the board when neither has it).
    fn key_layers(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.covered() || !crate::viewers::layers::cycles(&self.data["thread"]) { return }
        let schematic = self.screens.pv(1).focus.as_ref().map_or(false, |f| f.is_focused(window));
        let _ = self.tx.try_send(Command::Action("view-layer-cycle".into(), if schematic { "schematic" } else { "" }.into())); cx.notify();
    }
    fn key_go(&mut self, id: Option<String>, cx: &mut Context<Self>) { if let Some(id) = id { self.select_id(id, cx); self.want_focus = true; } }
}

/// Registers every action on the root element.
pub fn attach(el: Div, cx: &mut Context<Backplane>) -> Div {
    let el = el.key_context("Backplane");
    macro_rules! on { ($el:expr, $a:ty, $f:expr) => { $el.on_action(cx.listener(|this, _: &$a, window, cx| { let _ = &window; ($f)(this, window, cx) })) } }
    let el = on!(el, NewThread, |this: &mut Backplane, _, cx: &mut Context<Backplane>| {
        let mut pid = project_of(&this.data, &this.selected); if pid.is_empty() { pid = this.project_page.clone().or_else(|| nth_project(&this.data, 1)).unwrap_or_default(); }
        if !pid.is_empty() { this.arm_new(); let _ = this.tx.try_send(Command::Action("new-thread".into(), pid)); cx.notify(); }
    });
    let el = on!(el, OpenDiff, |this: &mut Backplane, _, cx: &mut Context<Backplane>| this.key_rail("diff", "", cx));
    let el = on!(el, OpenFiles, |this: &mut Backplane, _, cx: &mut Context<Backplane>| this.key_rail("view-kind", "files", cx));
    let el = on!(el, StopTurn, |this: &mut Backplane, _, cx: &mut Context<Backplane>| if has_tool(&this.data, "interrupt") { let _ = this.tx.try_send(Command::Action("interrupt".into(), String::new())); cx.notify(); });
    let el = on!(el, SettleThread, |this: &mut Backplane, _, cx: &mut Context<Backplane>| for a in ["settle", "unsettle"] { if has_tool(&this.data, a) { let _ = this.tx.try_send(Command::Action(a.into(), String::new())); cx.notify(); break } });
    let el = on!(el, ModelPicker, |this: &mut Backplane, w: &mut Window, cx: &mut Context<Backplane>| if this.data["thread"].is_object() { this.picker_toggle(w, cx) });
    let el = on!(el, PrevThread, |this: &mut Backplane, _, cx: &mut Context<Backplane>| { let n = neighbour(&this.data, &this.selected, false); this.key_go(n, cx) });
    let el = on!(el, NextThread, |this: &mut Backplane, _, cx: &mut Context<Backplane>| { let n = neighbour(&this.data, &this.selected, true); this.key_go(n, cx) });
    let el = on!(el, UberChat, |this: &mut Backplane, _, cx: &mut Context<Backplane>| {
        let id = this.data["bots"].as_array().into_iter().flatten().find(|b| b["uber"].as_bool().unwrap_or(false)).map(|b| crate::string(b, "id"));
        if let Some(id) = id { let _ = this.tx.try_send(Command::Action("bot".into(), id)); cx.notify(); }
    });
    let el = on!(el, FocusComposer, |this: &mut Backplane, _, cx: &mut Context<Backplane>| { this.want_focus = true; cx.notify(); });
    let el = on!(el, CycleLayers, |this: &mut Backplane, w: &mut Window, cx: &mut Context<Backplane>| this.key_layers(w, cx));
    macro_rules! rail { ($el:expr, $($a:ty => ($k:literal, $v:literal)),*) => {{ let mut e = $el; $( e = on!(e, $a, |this: &mut Backplane, _, cx: &mut Context<Backplane>| this.key_rail($k, $v, cx)); )* e }} }
    let el = rail!(el, RailBoard => ("view-kind", "board"), RailSchematic => ("view-kind", "schematic"), Rail3d => ("view-kind", "3d"), RailSplit => ("view-kind", "split"),
        RailMech => ("view-kind", "mech"), RailFiles => ("view-kind", "files"), RailDiff => ("diff", ""), RailReview => ("view-kind", "review"), RailBrowser => ("view-kind", "browser"));
    macro_rules! digits { ($el:expr, $($n:literal $p:ty, $t:ty),*) => {{ let mut e = $el; $(
        e = on!(e, $p, |this: &mut Backplane, _, cx: &mut Context<Backplane>| if let Some(p) = nth_project(&this.data, $n) { this.open_project(p, cx) });
        e = on!(e, $t, |this: &mut Backplane, _, cx: &mut Context<Backplane>| { let n = nth_thread(&this.data, &this.selected, this.project_page.as_deref(), $n); this.key_go(n, cx) });
    )* e }} }
    digits!(el, 1 Project1, Thread1, 2 Project2, Thread2, 3 Project3, Thread3, 4 Project4, Thread4, 5 Project5, Thread5, 6 Project6, Thread6, 7 Project7, Thread7, 8 Project8, Thread8, 9 Project9, Thread9)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    use serde_json::json;

    /// The action a keystroke does with the given contexts open (outermost first): its name, "" for none.
    fn does(l: &Local, key: &str, ctx: &[&str]) -> String {
        let map = Keymap::new(bindings(l));
        let stack: Vec<KeyContext> = ["Backplane"].iter().chain(ctx.iter()).map(|c| KeyContext::parse(c).unwrap()).collect();
        let ks = [Keystroke::parse(key).unwrap()];
        let (b, _) = map.bindings_for_input(&ks, &stack);
        b.first().map(|b| b.action().name().to_string()).unwrap_or_default()
    }
    fn d(key: &str, ctx: &[&str]) -> String { does(&Local::default(), key, ctx) }

    #[test] fn global_scope_keys_work_everywhere_including_the_composer() {
        for ctx in [&[][..], &["Composer"][..], &["TextInput"][..]] {
            assert_eq!(d("ctrl-k", ctx), "backplane::OpenPaletteKey"); assert_eq!(d("ctrl-shift-p", ctx), "backplane::OpenPaletteKey");
            assert_eq!(d("ctrl-n", ctx), "keys::NewThread"); assert_eq!(d("ctrl-b", ctx), "backplane::ToggleSidebarKey");
            assert_eq!(d("ctrl-.", ctx), "keys::StopTurn"); assert_eq!(d("ctrl-m", ctx), "keys::ModelPicker");
        }
    }
    #[test] fn composer_scope_keeps_its_own_keys_and_types_a_slash() {
        assert_eq!(d("enter", &["Composer"]), "backplane::Send"); assert_eq!(d("ctrl-enter", &["Composer"]), "backplane::Send");
        assert_eq!(d("shift-enter", &["Composer"]), "composer::Newline");
        assert_eq!(d("/", &["Composer"]), "", "a slash is typed"); assert_eq!(d("b", &["Composer"]), "");
        // no shortcut shadows an editing key of the composer
        for k in ["ctrl-a", "ctrl-c", "ctrl-x", "ctrl-v", "ctrl-z", "ctrl-y", "ctrl-shift-z", "ctrl-left", "ctrl-right", "ctrl-backspace", "ctrl-home", "ctrl-end", "home", "end", "tab", "backspace", "delete"] {
            let a = d(k, &["Composer"]); assert!(a.starts_with("composer::"), "{k} must stay the composer's, got {a}");
        }
    }
    #[test] fn timeline_scope_bare_keys_act_only_outside_fields_and_viewers() {
        assert_eq!(d("/", &[]), "keys::FocusComposer"); assert_eq!(d("b", &[]), "keys::RailBoard"); assert_eq!(d("3", &[]), "keys::Rail3d"); assert_eq!(d("d", &[]), "keys::RailDiff");
        for c in ["Composer", "TextInput", "PickInput", "AskInput", "ImpInput", "Viewer", "BrowserBar", "FilesPage"] { for k in ["/", "b", "s", "3", "v", "m", "f", "d", "r", "w"] { assert_eq!(d(k, &[c]), "", "{k} in {c}"); } }
        assert_eq!(d("ctrl-shift-s", &[]), "keys::SettleThread");
    }
    #[test] fn the_pad_has_its_keys_and_they_never_reach_the_composer() {
        assert_eq!(d("ctrl-z", &["DrawPad"]), "pad::Undo"); assert_eq!(d("delete", &["DrawPad"]), "pad::Delete"); assert_eq!(d("backspace", &["DrawPad"]), "pad::Delete");
        assert_eq!(d("ctrl-z", &["Composer"]), "composer::Undo", "the message box keeps its own undo");
        assert_eq!(d("ctrl-z", &["DrawPad", "PadLength"]), "composer::Undo", "in the length box it undoes the typing");
        assert_eq!(d("delete", &["DrawPad", "PadLength"]), "composer::Delete"); assert_eq!(d("enter", &["DrawPad", "PadLength"]), "composer::Go");
        assert_eq!(d("escape", &["DrawPad"]), "backplane::ClosePopup"); assert_eq!(d("escape", &["DrawPad", "PadLength"]), "backplane::ClosePopup");
    }
    #[test] fn nothing_behind_the_pad_moves_while_it_is_open() {
        for ctx in [&["DrawPad"][..], &["DrawPad", "PadLength"][..]] {
            for k in ["ctrl-k", "ctrl-shift-p", "ctrl-n", "ctrl-b", "ctrl-.", "ctrl-m", "ctrl-d", "ctrl-p", "ctrl-shift-f", "ctrl-shift-s", "ctrl-shift-u", "alt-1", "ctrl-1", "ctrl-alt-i", "/", "b", "s", "3", "w"] { assert_eq!(d(k, ctx), "", "{k} in {ctx:?}"); }
        }
    }
    #[test] fn a_rebound_close_key_still_shuts_the_pad_and_rebound_shortcuts_stay_muted() {
        let mut l = Local::default(); assert!(l.set("shortcut", "new\u{1f}ctrl-alt-n"));
        assert_eq!(does(&l, "ctrl-alt-n", &["DrawPad"]), "");
        assert_eq!(does(&l, "ctrl-n", &[]), ""); assert_eq!(does(&l, "ctrl-alt-n", &[]), "keys::NewThread", "only inside the pad");
    }
    #[test] fn the_pads_keys_are_in_the_table_and_no_longer_missing() {
        assert_eq!((find("pad-undo").unwrap().kind, find("pad-delete").unwrap().kind), (Fixed, Fixed));
        assert_eq!(hint(&Local::default(), "pad-undo"), "Ctrl+Z"); assert_eq!(hint(&Local::default(), "pad-delete"), "Delete"); assert_eq!(hint(&Local::default(), "pad-delete-alt"), "Backspace");
        assert!(MISSING.iter().all(|(k, why)| !k.contains("drawing pad") && !why.contains("Drawing pad")));
    }
    #[test] fn viewer_scope_keeps_its_keys_but_the_chords_work() {
        assert_eq!(d("ctrl-d", &["Viewer"]), "keys::OpenDiff"); assert_eq!(d("ctrl-p", &["Viewer"]), "keys::OpenFiles"); assert_eq!(d("m", &["Viewer"]), "");
    }
    #[test] fn ctrl_h_cycles_layers_from_the_plot_and_stays_out_of_every_text_field() {
        assert_eq!(d("ctrl-h", &["Viewer"]), "keys::CycleLayers"); assert_eq!(d("ctrl-h", &[]), "keys::CycleLayers", "nothing focused");
        assert_eq!(d("ctrl-h", &["FilesPage"]), "keys::CycleLayers");
        for c in ["Composer", "TextInput", "PickInput", "AskInput", "ImpInput", "BrowserBar", "BrowserView", "CefBar", "CefPage"] {
            assert_eq!(d("ctrl-h", &[c]), "", "{c} keeps Ctrl+H"); assert_eq!(d("ctrl-h", &["Viewer", c]), "", "{c} inside a viewer");
        }
        assert_eq!(d("ctrl-shift-h", &["Viewer"]), "", "the performance overlay's chord stays free");
        assert!(MISSING.iter().all(|m| m.0 != "Ctrl+H"), "listed as done, not missing");
    }
    #[test] fn layer_focus_is_in_the_table_and_rebinds() {
        let x = find("layers").unwrap(); assert_eq!((x.key, x.kind), ("ctrl-h", Rebind)); assert_eq!(hint(&Local::default(), "layers"), "Ctrl+H");
        let mut l = Local::default();
        assert!(l.set("shortcut", "layers\u{1f}ctrl-alt-h")); assert_eq!(does(&l, "ctrl-alt-h", &["Viewer"]), "keys::CycleLayers"); assert_eq!(does(&l, "ctrl-h", &["Viewer"]), ""); assert_eq!(does(&l, "ctrl-alt-h", &["Composer"]), "");
    }
    #[test] fn sidebar_scope_numbers_and_neighbours() {
        assert_eq!(d("alt-1", &[]), "keys::Project1"); assert_eq!(d("alt-9", &[]), "keys::Project9"); assert_eq!(d("ctrl-1", &["Composer"]), "keys::Thread1"); assert_eq!(d("ctrl-9", &[]), "keys::Thread9");
        assert_eq!(d("ctrl-shift-[", &[]), "keys::PrevThread"); assert_eq!(d("ctrl-shift-]", &[]), "keys::NextThread");
        assert_eq!(d("ctrl-shift-f", &[]), "backplane::FocusSearchKey"); assert_eq!(d("ctrl-shift-u", &[]), "keys::UberChat");
        let data = json!({"projects": [{"id": "h|p1", "threads": [{"id": "h|a"}, {"id": "h|b"}]}, {"id": "h|p2", "threads": [{"id": "h|c"}]}]});
        assert_eq!(neighbour(&data, "a", true).as_deref(), Some("h|b")); assert_eq!(neighbour(&data, "b", true).as_deref(), Some("h|c"));
        assert_eq!(neighbour(&data, "a", false), None); assert_eq!(neighbour(&data, "c", false).as_deref(), Some("h|b"));
        assert_eq!(nth_project(&data, 2).as_deref(), Some("p2")); assert_eq!(nth_project(&data, 3), None);
        assert_eq!(nth_thread(&data, "b", None, 1).as_deref(), Some("h|a"), "the thread's own project"); assert_eq!(nth_thread(&data, "b", None, 3), None);
    }
    #[test] fn palette_and_dialog_scopes() {
        assert_eq!(d("escape", &["PickInput"]), "backplane::ClosePopup"); assert_eq!(d("escape", &["AskInput"]), "backplane::ClosePopup");
        assert_eq!(d("enter", &["PickInput"]), "composer::Go"); assert_eq!(d("ctrl-k", &["PickInput"]), "backplane::OpenPaletteKey", "the same chord shuts it");
    }
    #[test] fn a_rebound_shortcut_moves_and_reset_restores_it() {
        let mut l = Local::default();
        assert!(l.set("shortcut", "new\u{1f}ctrl-alt-n")); assert_eq!(does(&l, "ctrl-alt-n", &[]), "keys::NewThread"); assert_eq!(does(&l, "ctrl-n", &[]), "");
        assert!(l.set("shortcut", "new")); assert!(!l.set("shortcut", "new\u{1f}enter"), "a bare key is no choice for it");
        assert!(!l.set("shortcut", "projects")); l.set("shortcuts-reset", ""); assert_eq!(does(&l, "ctrl-n", &[]), "keys::NewThread");
    }
    #[test] fn keys_read_per_platform() {
        assert_eq!(show("secondary-shift-n", false), "Ctrl+Shift+N"); assert_eq!(show("secondary-shift-n", true), "⌘⇧N");
        assert_eq!(show("ctrl-.", false), "Ctrl+."); assert_eq!(show("escape", false), "Esc"); assert_eq!(show("alt-1", true), "⌥1"); assert_eq!(show("secondary-shift-[", false), "Ctrl+Shift+[");
    }
    #[test] fn every_rebindable_default_is_a_valid_keystroke_and_the_first_choice() {
        for x in TABLE { if x.kind != Fixed { assert!(Keystroke::parse(x.key).is_ok(), "{}", x.id); assert_eq!(choices(x.id)[0], x.key); for k in choices(x.id) { assert!(Keystroke::parse(&k).is_ok(), "{k}"); } } }
        let mut seen = std::collections::HashMap::new();
        for x in TABLE.iter().filter(|x| x.kind != Fixed) { assert!(seen.insert(x.key, x.id).is_none(), "{} clashes", x.key); }
    }
    #[test] fn hints_follow_the_hubs_actions() {
        assert_eq!(id_for("diff", "", ""), "diff"); assert_eq!(id_for("interrupt", "", ""), "stop"); assert_eq!(id_for("new-thread", "p1", "p1"), "new"); assert_eq!(id_for("new-thread", "p2", "p1"), "");
        assert_eq!(hint(&Local::default(), "stop"), "Ctrl+."); assert_eq!(rail_letter("3d"), "3");
    }
}
