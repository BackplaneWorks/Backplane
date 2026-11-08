//! The client's keyboard shortcuts, ported from the Bend-era web client (src/web/workspace-keys.js on the tag `main-bend-final`).
//! One table (`TABLE`) feeds the key bindings, the palette's hints, the tooltips and the Settings list. `secondary` is Cmd on macOS and Ctrl elsewhere.
//! Bare keys (`/`, the rail's letters) are bound only outside every text field and viewer (`FREE`), so typing is never taken.
use crate::{hub::Command, local::Local, theme, Backplane};
use gpui::{actions, prelude::*, *};
use serde_json::Value;

actions!(keys, [NewThread, OpenDiff, OpenFiles, StopTurn, ModelPicker, PrevThread, NextThread, UberChat, SettleThread, FocusComposer, CycleLayers, PerfOverlay,
    AccountMenu, Account1, Account2, Account3, Account4, Account5, Account6, Account7, Account8, Account9,
    PickUp, PickDown, PickLeft, PickRight, PickTab, PickBackTab, PickEnter,
    Project1, Project2, Project3, Project4, Project5, Project6, Project7, Project8, Project9,
    Thread1, Thread2, Thread3, Thread4, Thread5, Thread6, Thread7, Thread8, Thread9,
    RailBoard, RailSchematic, Rail3d, RailSplit, RailMech, RailFiles, RailDiff, RailReview, RailBrowser, ToggleTerminal, DismissTerminal]);

/// Where no text field, file list or viewer has the keyboard: a bare key may act.
pub const FREE: &str = "!AccountMenu && !Composer && !TextInput && !PickInput && !AskInput && !ImpInput && !Viewer && !BrowserBar && !FilesPage";
/// No text field has the keyboard (so a chord that edits text there is the field's: Option+digit types a character on a Mac, Ctrl+C copies).
const NOT_MAC_COMPOSER: &str = "!Composer && !TextInput && !PickInput && !AskInput && !ImpInput";
/// Where a bare backtick shows the terminal: the message box, or nowhere that types (a shell, another field and a viewer keep their backticks).
const TERM_BARE: &str = "Composer || (!TextInput && !PickInput && !AskInput && !ImpInput && !Viewer && !BrowserBar && !FilesPage && !Terminal)";
/// Where Ctrl+H is the viewer's: everywhere but the small fields, the terminal and web pages, whose own it stays (it deletes a character in many fields and in a shell, and opens a web page's history).
/// The composer is included, as in the old client: the board beside it answers without a click on the canvas. With no plot on show the key goes on to the composer (`key_layers`).
const NOT_FIELD: &str = "!TextInput && !PickInput && !AskInput && !ImpInput && !Terminal && !BrowserBar && !BrowserView && !CefBar && !CefPage";
/// The open model menu has its keys wherever the keyboard is: the window says `Picker` while it is open, and these bind at the depth of each text field, after the field's own keys, so they come first.
const PICK_SCOPES: [&str; 6] = ["Picker", "Picker > Composer", "Picker > PickInput", "Picker > TextInput", "Picker > AskInput", "Picker > ImpInput"];

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
    e("account", "Account of this thread, with what each has left", "Composer", "secondary-shift-a", Rebind),
    e("account-pick", "Pick account 1…9 in the account menu", "Composer", "1…9", Fixed),
    e("menu-up", "Previous row: wraps round, and scrolls into view", "Model menu and popups", "up", Fixed),
    e("menu-down", "Next row: wraps round, and scrolls into view", "Model menu and popups", "down", Fixed),
    e("menu-left", "Less effort, or the caret while the search holds text", "Model menu", "left", Fixed),
    e("menu-right", "More effort, or the caret while the search holds text", "Model menu", "right", Fixed),
    e("menu-tab", "Next column: provider, model, effort. In a popup, take the row", "Model menu and popups", "tab", Fixed),
    e("menu-back", "Previous column", "Model menu", "shift-tab", Fixed),
    e("menu-enter", "Take the highlighted row", "Model menu and popups", "enter", Fixed),
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
    e("term", "Show or hide the floating terminal", "Terminal", "secondary-j", Rebind),
    e("term-alt", "Show the floating terminal (also)", "Terminal", "`", Fixed),
    e("term-leave", "Leave the terminal: put it away, or take the keyboard back to it", "Terminal", "shift-escape", Fixed),
    e("term-copy", "Copy the terminal's selection", "Terminal", "secondary-shift-c", Fixed),
    e("term-paste", "Paste into the terminal", "Terminal", "secondary-shift-v", Fixed),
    e("rail", "Rail views: B board, S schematic, 3 3D, V split, M mech, F files, D diff, R review, W browser", "Viewer", "b s 3 v m f d r w", Fixed),
    e("perf", "Performance overlay: the last few seconds of the window's work", "Window", "ctrl-shift-h", Rebind),
];
/// Keys the old client had that this one cannot do yet (no equivalent): listed, not faked.
pub const MISSING: &[(&str, &str)] = &[
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
            ("escape", _) => "Esc".into(), ("enter", _) => "Enter".into(), ("delete", _) => "Delete".into(), ("backspace", _) => "Backspace".into(), ("tab", _) => "Tab".into(),
            ("up", _) => "Up".into(), ("down", _) => "Down".into(), ("left", _) => "Left".into(), ("right", _) => "Right".into(),
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
        KeyBinding::new(&k("stop"), StopTurn, None), KeyBinding::new(&k("models"), ModelPicker, None), KeyBinding::new(&k("account"), AccountMenu, None),
        KeyBinding::new(&k("prev"), PrevThread, None), KeyBinding::new(&k("next"), NextThread, None),
        KeyBinding::new(&k("uber"), UberChat, None), KeyBinding::new(&k("settle"), SettleThread, None), KeyBinding::new(&k("layers"), CycleLayers, Some(NOT_FIELD)), KeyBinding::new(&k("perf"), PerfOverlay, None),
        KeyBinding::new("/", FocusComposer, Some(FREE)),
        KeyBinding::new(&k("term"), ToggleTerminal, None), KeyBinding::new("`", ToggleTerminal, Some(TERM_BARE)), KeyBinding::new("shift-escape", DismissTerminal, None)]);
    v.extend([KeyBinding::new("1", Account1, Some("AccountMenu")), KeyBinding::new("2", Account2, Some("AccountMenu")), KeyBinding::new("3", Account3, Some("AccountMenu")), KeyBinding::new("4", Account4, Some("AccountMenu")), KeyBinding::new("5", Account5, Some("AccountMenu")),
        KeyBinding::new("6", Account6, Some("AccountMenu")), KeyBinding::new("7", Account7, Some("AccountMenu")), KeyBinding::new("8", Account8, Some("AccountMenu")), KeyBinding::new("9", Account9, Some("AccountMenu"))]);
    let alt = if mac() { Some(NOT_MAC_COMPOSER) } else { None };
    macro_rules! digits { ($($n:literal $p:ident $t:ident),*) => { v.extend([$(KeyBinding::new(concat!("alt-", $n), $p, alt), KeyBinding::new(concat!("secondary-", $n), $t, None)),*]); } }
    digits!("1" Project1 Thread1, "2" Project2 Thread2, "3" Project3 Thread3, "4" Project4 Thread4, "5" Project5 Thread5, "6" Project6 Thread6, "7" Project7 Thread7, "8" Project8 Thread8, "9" Project9 Thread9);
    v.extend([KeyBinding::new("b", RailBoard, Some(FREE)), KeyBinding::new("s", RailSchematic, Some(FREE)), KeyBinding::new("3", Rail3d, Some(FREE)), KeyBinding::new("v", RailSplit, Some(FREE)),
        KeyBinding::new("m", RailMech, Some(FREE)), KeyBinding::new("f", RailFiles, Some(FREE)), KeyBinding::new("d", RailDiff, Some(FREE)), KeyBinding::new("r", RailReview, Some(FREE)),
        KeyBinding::new("w", RailBrowser, Some(FREE))]);
    let key = |id: &str| find(id).map_or("", |x| x.key);
    for c in PICK_SCOPES.map(Some) {
        v.extend([KeyBinding::new(key("menu-up"), PickUp, c), KeyBinding::new(key("menu-down"), PickDown, c), KeyBinding::new(key("menu-left"), PickLeft, c), KeyBinding::new(key("menu-right"), PickRight, c),
            KeyBinding::new(key("menu-tab"), PickTab, c), KeyBinding::new(key("menu-back"), PickBackTab, c), KeyBinding::new(key("menu-enter"), PickEnter, c)]);
    }
    v.extend(pad_bindings(&v, &k("close")));
    let term = term_bindings(&v, &k("term"));
    v.extend(term);
    v
}
/// The floating terminal's own keys, and every other shortcut muted in it: Ctrl+C, Ctrl+K, Esc, a backtick and the rest are the shell's.
/// Its toggle and Shift+Esc stay (they are bound everywhere). Last, so the mutes win the tie with the bindings that have no context.
fn term_bindings(all: &[KeyBinding], toggle: &str) -> Vec<KeyBinding> {
    use crate::screens::term::{Copy, Paste};
    let mut v = vec![KeyBinding::new("secondary-shift-c", Copy, Some("Terminal")), KeyBinding::new("secondary-shift-v", Paste, Some("Terminal"))];
    if mac() { v.extend([KeyBinding::new("cmd-c", Copy, Some("Terminal")), KeyBinding::new("cmd-v", Paste, Some("Terminal"))]); }
    let own: Vec<String> = [toggle, "shift-escape", "secondary-shift-c", "secondary-shift-v"].iter().chain(if mac() { &["cmd-c", "cmd-v"][..] } else { &[][..] })
        .filter_map(|k| Keystroke::parse(k).ok()).map(|k| k.unparse()).collect();
    let mut seen = std::collections::HashSet::new();
    for key in all.iter().filter_map(|b| b.keystrokes().first().map(|k| k.unparse())) {
        if !own.contains(&key) && seen.insert(key.clone()) { v.push(KeyBinding::new(&key, NoAction, Some("Terminal"))); }
    }
    v
}
/// The drawing pad's keys, and every other shortcut muted while it is open, as the old pad took all the keys: nothing behind it moves under the person.
/// A binding without a context is as deep as the innermost context, so the mute names the length box's context too, and comes last to win the tie.
fn pad_bindings(all: &[KeyBinding], close: &str) -> Vec<KeyBinding> {
    use crate::screens::padview::{Delete, Undo};
    let mut v = vec![KeyBinding::new("secondary-z", Undo, Some("DrawPad")), KeyBinding::new("delete", Delete, Some("DrawPad")), KeyBinding::new("backspace", Delete, Some("DrawPad"))];
    let close = Keystroke::parse(close).map(|k| k.unparse()).unwrap_or_default();
    let own = ["composer::", "text_input::", "backplane::Send", "backplane::CopySelection", "keys::Pick"]; // the length box's editing keys, keys only a text field or the page has, and the picker's (bound only in its scopes)
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
    fn covered(&self) -> bool { self.screens.palette.open || self.screens.lightbox.is_some() || self.screens.picker_open || self.screens.acct.menu || crate::screens::settings::is_open(self) || self.data["tour"].is_object() || crate::screens::migrate::is_up(&self.data) }
    fn key_rail(&mut self, a: &str, v: &str, cx: &mut Context<Self>) { if !self.covered() && self.data["thread"].is_object() { self.rail_press(a, v, cx); } }
    /// Ctrl+H: the plot on show cycles its focus; on Split, the pane with the keyboard (the board when neither has it, as from the composer). The hub is told which layers that plot has.
    /// With no plot to act on the key is not spent: it goes on to whatever has the keyboard.
    fn key_layers(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.covered() || !crate::viewers::layers::cycles(&self.data["thread"]) { cx.propagate(); return }
        let split = self.data["thread"]["viewer"]["open"] == "split";
        let schematic = split && self.screens.pv(1).focus.as_ref().map_or(false, |f| f.is_focused(window));
        let present = self.screens.pv(if schematic { 1 } else if split { 2 } else { 0 }).present;
        let _ = self.tx.try_send(Command::Action("view-layer-cycle".into(), crate::viewers::layers::cycle_value(schematic, present))); cx.notify();
    }
    fn key_go(&mut self, id: Option<String>, cx: &mut Context<Self>) { if let Some(id) = id { self.select_id(id, cx); self.want_focus = true; } }
    /// The overlay up or down. Going up it asks for one more frame, so a quiet window shows the numbers of the frame that opened it instead of "measuring"; that is the only frame it ever asks for.
    fn perf_toggle(&mut self, cx: &mut Context<Self>) {
        if crate::perf::toggle() { cx.spawn(async move |this, cx| { cx.background_executor().timer(crate::perf::REFRESH).await; let _ = this.update(cx, |_, cx| cx.notify()); }).detach(); }
        cx.notify();
    }
}

/// Registers every action on the root element.
pub fn attach(el: Div, picker: bool, cx: &mut Context<Backplane>) -> Div {
    let el = el.key_context(if picker { "Backplane Picker" } else { "Backplane" });
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
    let el = on!(el, AccountMenu, |this: &mut Backplane, w: &mut Window, cx: &mut Context<Backplane>| if this.data["thread"].is_object() { crate::screens::accounts::toggle(this, w, cx) });
    macro_rules! picks { ($el:expr, $($n:literal $a:ty),*) => {{ let mut e = $el; $( e = on!(e, $a, |this: &mut Backplane, _, cx: &mut Context<Backplane>| crate::screens::accounts::press_nth(this, $n, cx)); )* e }} }
    let el = picks!(el, 1 Account1, 2 Account2, 3 Account3, 4 Account4, 5 Account5, 6 Account6, 7 Account7, 8 Account8, 9 Account9);
    use crate::screens::picker::Key;
    let el = on!(el, PickUp, |this: &mut Backplane, w: &mut Window, cx: &mut Context<Backplane>| this.picker_key(Key::Up, w, cx));
    let el = on!(el, PickDown, |this: &mut Backplane, w: &mut Window, cx: &mut Context<Backplane>| this.picker_key(Key::Down, w, cx));
    let el = on!(el, PickTab, |this: &mut Backplane, w: &mut Window, cx: &mut Context<Backplane>| this.picker_key(Key::Tab, w, cx));
    let el = on!(el, PickBackTab, |this: &mut Backplane, w: &mut Window, cx: &mut Context<Backplane>| this.picker_key(Key::BackTab, w, cx));
    let el = on!(el, PickLeft, |this: &mut Backplane, w: &mut Window, cx: &mut Context<Backplane>| this.picker_side(Key::Left, w, cx));
    let el = on!(el, PickRight, |this: &mut Backplane, w: &mut Window, cx: &mut Context<Backplane>| this.picker_side(Key::Right, w, cx));
    let el = on!(el, PickEnter, |this: &mut Backplane, _, cx: &mut Context<Backplane>| this.picker_enter(cx));
    let el = on!(el, PrevThread, |this: &mut Backplane, _, cx: &mut Context<Backplane>| { let n = neighbour(&this.data, &this.selected, false); this.key_go(n, cx) });
    let el = on!(el, NextThread, |this: &mut Backplane, _, cx: &mut Context<Backplane>| { let n = neighbour(&this.data, &this.selected, true); this.key_go(n, cx) });
    let el = on!(el, UberChat, |this: &mut Backplane, _, cx: &mut Context<Backplane>| {
        let id = this.data["bots"].as_array().into_iter().flatten().find(|b| b["uber"].as_bool().unwrap_or(false)).map(|b| crate::string(b, "id"));
        if let Some(id) = id { let _ = this.tx.try_send(Command::Action("bot".into(), id)); cx.notify(); }
    });
    let el = on!(el, FocusComposer, |this: &mut Backplane, _, cx: &mut Context<Backplane>| { this.want_focus = true; cx.notify(); });
    let el = on!(el, ToggleTerminal, |this: &mut Backplane, w: &mut Window, cx: &mut Context<Backplane>| if this.screens.pad.is_none() { this.term_toggle(w, cx) });
    let el = on!(el, DismissTerminal, |this: &mut Backplane, w: &mut Window, cx: &mut Context<Backplane>| this.term_dismiss(w, cx));
    let el = on!(el, CycleLayers, |this: &mut Backplane, w: &mut Window, cx: &mut Context<Backplane>| this.key_layers(w, cx));
    let el = on!(el, PerfOverlay, |this: &mut Backplane, _, cx: &mut Context<Backplane>| this.perf_toggle(cx));
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
    /// The chords below are written as on Linux (Ctrl); on a Mac the secondary ones are Cmd. The keys the table spells `ctrl-` stay Ctrl, as do chords with Alt (a person's rebinding).
    fn pk(key: &str) -> String {
        if !mac() || !key.starts_with("ctrl-") || key.contains("alt") || key == "ctrl-enter" || TABLE.iter().any(|x| x.key == key) { key.to_string() } else { key.replacen("ctrl-", "cmd-", 1) }
    }
    fn does(l: &Local, key: &str, ctx: &[&str]) -> String {
        let key = &pk(key);
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
    #[test] fn the_account_chord_works_everywhere_and_shadows_nothing() {
        for ctx in [&[][..], &["Composer"][..], &["TextInput"][..], &["Viewer"][..], &["FilesPage"][..]] { assert_eq!(d("ctrl-shift-a", ctx), "keys::AccountMenu", "{ctx:?}") }
        assert_eq!(hint(&Local::default(), "account"), if mac() { "⌘⇧A" } else { "Ctrl+Shift+A" }); assert_eq!(find("account").unwrap().kind, Rebind);
        let mut seen = std::collections::HashMap::new();
        for x in TABLE.iter().filter(|x| x.kind != Fixed || x.id == "palette-alt") { if let Some(o) = seen.insert(x.key, x.id) { panic!("{} and {o} share {}", x.id, x.key) } }
        for taken in ["alt-1", "alt-9"] { assert!(d(taken, &[]).starts_with("keys::Project"), "Alt+digit stays the project's") }
        let mut l = Local::default(); assert!(l.set("shortcut", "account\u{1f}ctrl-alt-a")); assert_eq!(does(&l, "ctrl-alt-a", &["Composer"]), "keys::AccountMenu"); assert_eq!(does(&l, "ctrl-shift-a", &["Composer"]), "", "rebinding moves it");
    }
    #[test] fn in_the_open_account_menu_a_digit_picks_and_nowhere_else() {
        for (k, a) in [("1", "keys::Account1"), ("3", "keys::Account3"), ("9", "keys::Account9")] { assert_eq!(d(k, &["AccountMenu"]), a) }
        assert_eq!(d("3", &[]), "keys::Rail3d", "with the menu shut 3 is the rail's"); assert_eq!(d("1", &["Composer"]), "", "and in the composer it is typed");
        for k in ["b", "m", "/", "s"] { assert_eq!(d(k, &["AccountMenu"]), "", "{k}: a bare key does nothing behind the menu") }
        assert_eq!(d("escape", &["AccountMenu"]), "backplane::ClosePopup"); assert_eq!(d("ctrl-shift-a", &["AccountMenu"]), "keys::AccountMenu", "the chord shuts it");
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
        assert_eq!(hint(&Local::default(), "pad-undo"), show("secondary-z", mac())); assert_eq!(hint(&Local::default(), "pad-delete"), "Delete"); assert_eq!(hint(&Local::default(), "pad-delete-alt"), "Backspace");
        assert!(MISSING.iter().all(|(k, why)| !k.contains("drawing pad") && !why.contains("Drawing pad")));
    }
    #[test] fn ctrl_j_toggles_the_terminal_from_everywhere_including_inside_it() {
        for ctx in [&[][..], &["Composer"][..], &["TextInput"][..], &["PickInput"][..], &["Viewer"][..], &["Terminal"][..]] { assert_eq!(d("ctrl-j", ctx), "keys::ToggleTerminal", "{ctx:?}"); }
        assert_eq!(d("ctrl-j", &["DrawPad"]), "", "nothing behind the pad moves");
    }
    #[test] fn a_bare_backtick_shows_the_terminal_but_stays_typed_in_fields_and_the_shell() {
        assert_eq!(d("`", &[]), "keys::ToggleTerminal"); assert_eq!(d("`", &["Composer"]), "keys::ToggleTerminal", "from the message box, as the old client");
        for c in ["Terminal", "TextInput", "PickInput", "AskInput", "ImpInput", "Viewer", "BrowserBar", "FilesPage", "DrawPad"] { assert_eq!(d("`", &[c]), "", "{c} keeps its backtick"); }
        assert_eq!(d("`", &["DrawPad", "PadLength"]), "");
    }
    #[test] fn shift_escape_leaves_the_terminal_and_plain_escape_is_the_shells() {
        for ctx in [&[][..], &["Composer"][..], &["Terminal"][..]] { assert_eq!(d("shift-escape", ctx), "keys::DismissTerminal", "{ctx:?}"); }
        assert_eq!(d("escape", &["Terminal"]), "", "Esc goes to the program (vim, less)"); assert_eq!(d("escape", &[]), "backplane::ClosePopup");
    }
    #[test] fn the_terminal_copies_and_pastes_with_shift_and_keeps_ctrl_c_for_the_shell() {
        assert_eq!(d("ctrl-shift-c", &["Terminal"]), "term::Copy"); assert_eq!(d("ctrl-shift-v", &["Terminal"]), "term::Paste");
        assert_eq!(d("ctrl-shift-c", &[]), "", "outside it the chord is not the terminal's"); assert_eq!(d("ctrl-shift-v", &["Composer"]), "");
        if mac() { assert_eq!(d("cmd-c", &["Terminal"]), "term::Copy"); assert_eq!(d("cmd-v", &["Terminal"]), "term::Paste"); }
    }
    #[test] fn every_other_shortcut_is_the_shells_inside_the_terminal() {
        for k in ["ctrl-c", "ctrl-k", "ctrl-n", "ctrl-b", "ctrl-d", "ctrl-p", "ctrl-.", "ctrl-m", "ctrl-h", "ctrl-shift-p", "ctrl-shift-f", "ctrl-shift-s", "ctrl-shift-u", "ctrl-shift-[", "alt-1", "ctrl-1", "ctrl-z", "ctrl-a", "enter", "tab", "backspace", "delete", "/", "b", "s", "3", "w", "`"] {
            if mac() && k == "ctrl-c" { continue } // Cmd+C copies in the terminal on a Mac (the secondary chord is its own)
            assert_eq!(d(k, &["Terminal"]), "", "{k} must reach the shell");
        }
    }
    #[test] fn a_rebound_terminal_key_moves_and_the_old_one_reaches_the_shell() {
        let mut l = Local::default(); assert!(l.set("shortcut", "term\u{1f}ctrl-alt-j"));
        assert_eq!(does(&l, "ctrl-alt-j", &[]), "keys::ToggleTerminal"); assert_eq!(does(&l, "ctrl-alt-j", &["Terminal"]), "keys::ToggleTerminal");
        assert_eq!(does(&l, "ctrl-j", &[]), ""); assert_eq!(does(&l, "ctrl-j", &["Terminal"]), "", "Ctrl+J is a line feed for the shell again");
    }
    #[test] fn the_terminals_keys_are_in_the_table_and_no_longer_missing() {
        assert_eq!(find("term").map(|x| (x.key, x.kind)), Some(("secondary-j", Rebind))); assert_eq!(hint(&Local::default(), "term"), if mac() { "⌘J" } else { "Ctrl+J" });
        assert_eq!(hint(&Local::default(), "term-leave"), if mac() { "⇧Esc" } else { "Shift+Esc" }); assert_eq!(hint(&Local::default(), "term-alt"), "`");
        assert!(["term-alt", "term-leave", "term-copy", "term-paste"].iter().all(|id| find(id).unwrap().kind == Fixed));
        assert!(MISSING.iter().all(|(k, why)| !k.contains("Ctrl+J") && !why.contains("terminal")), "listed as done, not missing");
    }
    #[test] fn viewer_scope_keeps_its_keys_but_the_chords_work() {
        assert_eq!(d("ctrl-d", &["Viewer"]), "keys::OpenDiff"); assert_eq!(d("ctrl-p", &["Viewer"]), "keys::OpenFiles"); assert_eq!(d("m", &["Viewer"]), "");
    }
    #[test] fn ctrl_h_cycles_layers_from_the_plot_and_the_composer_and_stays_out_of_the_small_fields() {
        assert_eq!(d("ctrl-h", &["Viewer"]), "keys::CycleLayers"); assert_eq!(d("ctrl-h", &[]), "keys::CycleLayers", "nothing focused");
        assert_eq!(d("ctrl-h", &["FilesPage"]), "keys::CycleLayers");
        assert_eq!(d("ctrl-h", &["Composer"]), "keys::CycleLayers", "the old client's: no click on the canvas first");
        for c in ["TextInput", "PickInput", "AskInput", "ImpInput", "Terminal", "BrowserBar", "BrowserView", "CefBar", "CefPage"] {
            assert_eq!(d("ctrl-h", &[c]), "", "{c} keeps Ctrl+H"); assert_eq!(d("ctrl-h", &["Viewer", c]), "", "{c} inside a viewer");
        }
        assert_eq!(d("ctrl-shift-h", &["Viewer"]), "keys::PerfOverlay", "Ctrl+Shift+H is the performance overlay, not layers");
        assert!(MISSING.iter().all(|m| m.0 != "Ctrl+H"), "listed as done, not missing");
    }
    #[test] fn layer_focus_is_in_the_table_and_rebinds() {
        let x = find("layers").unwrap(); assert_eq!((x.key, x.kind), ("ctrl-h", Rebind)); assert_eq!(hint(&Local::default(), "layers"), show("ctrl-h", mac()));
        let mut l = Local::default();
        assert!(l.set("shortcut", "layers\u{1f}ctrl-alt-h")); assert_eq!(does(&l, "ctrl-alt-h", &["Viewer"]), "keys::CycleLayers"); assert_eq!(does(&l, "ctrl-h", &["Viewer"]), ""); assert_eq!(does(&l, "ctrl-alt-h", &["Composer"]), "keys::CycleLayers"); assert_eq!(does(&l, "ctrl-alt-h", &["AskInput"]), "");
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
    #[test] fn the_performance_overlay_key_works_everywhere_and_can_be_moved() {
        for c in ["Composer", "TextInput", "PickInput", "AskInput", "ImpInput", "Viewer", "BrowserBar", "FilesPage"] { assert_eq!(d("ctrl-shift-h", &[c]), "keys::PerfOverlay", "in {c}"); }
        assert_eq!(d("ctrl-shift-h", &[]), "keys::PerfOverlay"); assert_eq!(hint(&Local::default(), "perf"), if mac() { "⌃⇧H" } else { "Ctrl+Shift+H" });
        assert!(MISSING.iter().all(|(k, why)| *k != "Ctrl+Shift+H" && !why.contains("Performance")), "no longer a key the client cannot do");
        let mut l = Local::default(); assert!(l.set("shortcut", "perf\u{1f}ctrl-alt-h")); assert_eq!(does(&l, "ctrl-alt-h", &[]), "keys::PerfOverlay"); assert_eq!(does(&l, "ctrl-shift-h", &[]), "");
    }
    #[test] fn palette_and_dialog_scopes() {
        assert_eq!(d("escape", &["PickInput"]), "backplane::ClosePopup"); assert_eq!(d("escape", &["AskInput"]), "backplane::ClosePopup");
        assert_eq!(d("enter", &["PickInput"]), "composer::Go"); assert_eq!(d("ctrl-k", &["PickInput"]), "backplane::OpenPaletteKey", "the same chord shuts it");
    }
    #[test] fn the_open_model_menu_has_its_keys_wherever_the_keyboard_is() {
        for ctx in [&["Picker"][..], &["Picker", "PickInput"][..], &["Picker", "Composer"][..], &["Picker", "TextInput"][..], &["Picker", "AskInput"][..], &["Picker", "ImpInput"][..]] {
            for (k, a) in [("up", "PickUp"), ("down", "PickDown"), ("left", "PickLeft"), ("right", "PickRight"), ("tab", "PickTab"), ("shift-tab", "PickBackTab"), ("enter", "PickEnter")] { assert_eq!(d(k, ctx), format!("keys::{a}"), "{k} in {ctx:?}"); }
            assert_eq!(d("escape", ctx), "backplane::ClosePopup"); assert_eq!(d("ctrl-m", ctx), "keys::ModelPicker", "the same chord shuts it");
        }
        // shut (or with something over it, when the window does not say Picker) the same keys are the fields' own
        assert_eq!(d("up", &["Composer"]), "composer::Up"); assert_eq!(d("tab", &["Composer"]), "composer::Tab"); assert_eq!(d("enter", &["Composer"]), "backplane::Send");
        assert_eq!(d("up", &["PickInput"]), "composer::Up", "the palette's search"); assert_eq!(d("enter", &["PickInput"]), "composer::Go"); assert_eq!(d("up", &[]), "");
        assert_eq!(d("shift-left", &["Picker", "PickInput"]), "composer::SelLeft", "selecting in the search stays");
    }
    #[test] fn the_menu_keys_are_listed_and_none_is_missing() {
        for id in ["menu-up", "menu-down", "menu-left", "menu-right", "menu-tab", "menu-back", "menu-enter"] { let x = find(id).unwrap(); assert_eq!(x.kind, Fixed); assert!(Keystroke::parse(x.key).is_ok(), "{id}"); }
        assert_eq!(hint(&Local::default(), "menu-up"), "Up"); assert_eq!(hint(&Local::default(), "menu-back"), show("shift-tab", mac())); assert_eq!(show("shift-tab", true), "⇧Tab");
        assert!(MISSING.iter().all(|(k, _)| !k.contains("picker")));
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
        assert_eq!(hint(&Local::default(), "stop"), show("ctrl-.", mac())); assert_eq!(rail_letter("3d"), "3");
    }
}
