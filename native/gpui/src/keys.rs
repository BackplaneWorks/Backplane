//! The client's keyboard shortcuts, ported from the Bend-era web client (src/web/workspace-keys.js on the tag `main-bend-final`).
//! One table (`TABLE`) feeds the key bindings, the palette's hints, the tooltips and the Settings list. `secondary` is Cmd on macOS and Ctrl elsewhere.
//! Bare keys (`/`, the rail's letters) are bound only outside every text field and viewer (`FREE`), so typing is never taken.
use crate::{hub::Command, local::Local, theme, Backplane};
use crate::i18n::{tr, trs};
use gpui::{actions, prelude::*, *};
use serde_json::Value;

actions!(keys, [NewThread, OpenDiff, OpenFiles, StopTurn, ModelPicker, PrevThread, NextThread, UberChat, SettleThread, FocusComposer, CycleLayers, PerfOverlay,
    AccountMenu, Account1, Account2, Account3, Account4, Account5, Account6, Account7, Account8, Account9,
    PickUp, PickDown, PickLeft, PickRight, PickTab, PickBackTab, PickEnter,
    Project1, Project2, Project3, Project4, Project5, Project6, Project7, Project8, Project9,
    Thread1, Thread2, Thread3, Thread4, Thread5, Thread6, Thread7, Thread8, Thread9,
    RailBoard, RailSchematic, Rail3d, RailSplit, RailMech, RailFiles, RailDiff, RailReview, RailBrowser, ToggleTerminal, DismissTerminal]);

/// Where no text field, file list or viewer has the keyboard: a bare key may act.
/* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ pub const FREE: &str = "!AccountMenu && !Composer && !TextInput && !PickInput && !FindInput && !AskInput && !ImpInput && !Viewer && !BrowserBar && !FilesPage";
/// Where a rail letter acts: as `FREE`, and from a board or schematic plot too. A plot takes no typing, and a click on it is where the keyboard
/// goes when a person looks at a board; with the letters dead there they never acted (the message box has the keyboard everywhere else).
/// The 3D and Mech views keep their own letters (F fits, 1-6 turn), and a web page, the terminal and the Files page their typing.
/* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ pub const RAIL: &str = "!AccountMenu && !Composer && !TextInput && !PickInput && !FindInput && !AskInput && !ImpInput && (!Viewer || Plot) && !BrowserBar && !FilesPage";
/// Where "/" puts the keyboard in the message box: as `FREE`, and from a viewer too (a web page's own "/" is kept: `focus_composer`).
/* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ const SLASH: &str = "!AccountMenu && !Composer && !TextInput && !PickInput && !FindInput && !AskInput && !ImpInput && !BrowserBar && !FilesPage";
/// No text field has the keyboard (so a chord that edits text there is the field's: Option+digit types a character on a Mac, Ctrl+C copies).
/* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ const NOT_MAC_COMPOSER: &str = "!Composer && !TextInput && !PickInput && !FindInput && !AskInput && !ImpInput";
/// Where Esc is the field's or the page's own: the viewer does not shut under it.
/* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ const ESC_OWNERS: [&str; 10] = ["TextInput", "PickInput", "FindInput", "AskInput", "ImpInput", "Terminal", "BrowserBar", "BrowserView", "CefBar", "CefPage"];
/// Where a bare backtick shows the terminal: the message box, or nowhere that types (a shell, another field and a viewer keep their backticks).
/* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ const TERM_BARE: &str = "Composer || (!TextInput && !PickInput && !FindInput && !AskInput && !ImpInput && !Viewer && !BrowserBar && !FilesPage && !Terminal)";
/// Where Ctrl+H is the viewer's: everywhere but the small fields, the terminal and web pages, whose own it stays (it deletes a character in many fields and in a shell, and opens a web page's history).
/// The composer is included, as in the old client: the board beside it answers without a click on the canvas. With no plot on show the key goes on to the composer (`key_layers`).
/// Ctrl+L to the message box: everywhere but the shell (its Ctrl+L clears the screen) and the Browser page (its address bar)
/* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ const NOT_SHELL: &str = "!Terminal && !BrowserBar && !BrowserView && !CefBar && !CefPage";
/* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ const NOT_FIELD: &str = "!TextInput && !PickInput && !FindInput && !AskInput && !ImpInput && !Terminal && !BrowserBar && !BrowserView && !CefBar && !CefPage";
/// The open model menu has its keys wherever the keyboard is: the window says `Picker` while it is open, and these bind at the depth of each text field, after the field's own keys, so they come first.
/* i18n-ok: key contexts */ const PICK_SCOPES: [&str; 6] = ["Picker", "Picker > Composer", "Picker > PickInput", "Picker > TextInput", "Picker > AskInput", "Picker > ImpInput"];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind { Legacy, Rebind, Fixed }
/// id, label, scope, default keystroke (GPUI syntax; display-only text for a group), kind.
pub struct Entry { pub id: &'static str, pub label: &'static str, pub scope: &'static str, pub key: &'static str, pub kind: Kind }
const fn e(id: &'static str, label: &'static str, scope: &'static str, key: &'static str, kind: Kind) -> Entry { Entry { id, label, scope, key, kind } }
use Kind::*;
pub const TABLE: &[Entry] = &[
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("send", "Send the message", "Composer", "enter", Legacy),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("newline", "New line in the message", "Composer", "shift-enter", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("send-alt", "Send (also)", "Composer", "ctrl-enter", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("focus", "Put the keyboard in the message box", "Composer", "/", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("focus-key", "Put the keyboard in the message box, from anywhere (a field, a viewer, a dialog)", "Composer", "secondary-l", Rebind),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("models", "Agent, model and effort", "Composer", "ctrl-m", Rebind),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("account", "Account of this thread, with what each has left", "Composer", "secondary-shift-a", Rebind),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("account-pick", "Pick account 1…9 in the account menu", "Composer", "1…9", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("menu-up", "Previous row: wraps round, and scrolls into view", "Model menu and popups", "up", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("menu-down", "Next row: wraps round, and scrolls into view", "Model menu and popups", "down", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("menu-left", "Less effort, or the caret while the search holds text", "Model menu", "left", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("menu-right", "More effort, or the caret while the search holds text", "Model menu", "right", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("menu-tab", "Next column: provider, model, effort. In a popup, take the row", "Model menu and popups", "tab", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("menu-back", "Previous column", "Model menu", "shift-tab", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("menu-enter", "Take the highlighted row", "Model menu and popups", "enter", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("close", "Close a popup, dialog or menu", "Dialogs", "escape", Legacy),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("palette", "Commands: jump to a thread or run a command", "Palette", "secondary-k", Rebind),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("palette-alt", "Commands (also)", "Palette", "secondary-shift-p", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("sidebar", "Fold or unfold the sidebar", "Sidebar", "secondary-b", Legacy),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("search", "Find what was said in any thread", "Sidebar", "secondary-shift-f", Legacy),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("find", "Find in this thread: Enter next, Shift+Enter previous, Esc closes", "Timeline", "secondary-f", Rebind),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("projects", "Go to project 1…9", "Sidebar", "alt-1…9", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("threads", "Go to thread 1…9 of the project", "Sidebar", "secondary-1…9", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("prev", "Previous thread", "Sidebar", "secondary-shift-[", Rebind),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("next", "Next thread", "Sidebar", "secondary-shift-]", Rebind),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("uber", "Uberbot chat", "Sidebar", "secondary-shift-u", Rebind),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("new", "New thread", "Timeline", "secondary-n", Rebind),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("settle", "Settle or unsettle the thread", "Timeline", "secondary-shift-s", Rebind),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("stop", "Stop the running turn", "Timeline", "ctrl-.", Rebind),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("diff", "Diff", "Viewer", "secondary-d", Rebind),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("files", "Files", "Viewer", "secondary-p", Rebind),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("layers", "Layer focus: the other layers as they are, dimmed, hidden", "Viewer", "ctrl-h", Rebind),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("pad-undo", "Undo the last stroke or change", "Drawing pad", "secondary-z", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("pad-delete", "Delete the selection, or clear the pad", "Drawing pad", "delete", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("pad-delete-alt", "Delete (also)", "Drawing pad", "backspace", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("term", "Show or hide the floating terminal", "Terminal", "secondary-j", Rebind),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("term-alt", "Show the floating terminal (also)", "Terminal", "`", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("term-super", "Show or hide the floating terminal (also, not on a Mac)", "Terminal", "super-`", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("term-leave", "Leave the terminal: put it away, or take the keyboard back to it", "Terminal", "shift-escape", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("term-copy", "Copy the terminal's selection", "Terminal", "secondary-shift-c", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("term-paste", "Paste into the terminal", "Terminal", "secondary-shift-v", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("rail", "Rail views: B board, S schematic, 3 3D, V split, M mech, F files, D diff, R review, W browser", "Viewer", "b s 3 v m f d r w", Fixed),
    /* i18n-ok: key contexts, and the table's words are drawn by Settings through trs */ e("perf", "Performance overlay: the last few seconds of the window's work", "Window", "ctrl-shift-h", Rebind),
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

/// A chord's Shift+symbol as the keyboard reports it (US layout): "secondary-shift-]" is "secondary-}"; None for a chord without one.
pub fn shifted(key: &str) -> Option<String> {
    const US: [(char, char); 21] = [('[', '{'), (']', '}'), ('\\', '|'), (';', ':'), ('\'', '"'), (',', '<'), ('.', '>'), ('/', '?'), ('`', '~'), ('=', '+'),
        ('1', '!'), ('2', '@'), ('3', '#'), ('4', '$'), ('5', '%'), ('6', '^'), ('7', '&'), ('8', '*'), ('9', '('), ('0', ')'), ('-', '_')];
    let (mods, last) = match key.strip_suffix("--") { Some(m) => (m, "-"), None => key.rsplit_once('-')? };
    let mut c = last.chars(); let (Some(ch), None) = (c.next(), c.next()) else { return None };
    let up = US.iter().find(|(a, _)| *a == ch)?.1;
    let rest: Vec<&str> = mods.split('-').filter(|m| *m != "shift").collect();
    (rest.len() < mods.split('-').count()).then(|| if rest.is_empty() { up.to_string() } else { format!("{}-{up}", rest.join("-")) })
}
/// "secondary-shift-n" as the person reads it: "Ctrl+Shift+N", or "⌘⇧N" on a Mac.
pub fn show(key: &str, mac: bool) -> String {
    let parts: Vec<&str> = if key.ends_with("--") { key[..key.len() - 2].split('-').chain(["-"]).collect() } else { key.split('-').collect() };
    let mut out: Vec<String> = vec![];
    for (i, p) in parts.iter().enumerate() {
        let last = i + 1 == parts.len();
        out.push(match (*p, mac) {
            /* i18n-ok: key names stay as on the keyboard */ ("secondary", true) => "⌘".into(), ("secondary" | "ctrl", false) => "Ctrl".into(), ("ctrl", true) => "⌃".into(),
            /* i18n-ok: key names stay as on the keyboard */ ("super", true) => "⌘".into(), ("super", false) => "Super".into(), ("alt", true) => "⌥".into(), ("alt", false) => "Alt".into(), ("shift", true) => "⇧".into(), ("shift", false) => "Shift".into(),
            /* i18n-ok: key names stay as on the keyboard */ ("escape", _) => "Esc".into(), ("enter", _) => "Enter".into(), ("delete", _) => "Delete".into(), ("backspace", _) => "Backspace".into(), ("tab", _) => "Tab".into(),
            /* i18n-ok: key names stay as on the keyboard */ ("up", _) => "Up".into(), ("down", _) => "Down".into(), ("left", _) => "Left".into(), ("right", _) => "Right".into(),
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
pub fn rail_letter(kind: &str) -> &'static str { match kind { "board" => "b", "schematic" => "s", "3d" => "3", "split" => "v", "mech" => "m", "files" => "f", "diff" => "d", "review" => "r", "browser" => "w", _ => "" } }

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
        KeyBinding::new("/", FocusComposer, Some(SLASH)), KeyBinding::new(&k("focus-key"), FocusComposer, Some(NOT_SHELL)), KeyBinding::new(&k("find"), crate::FindInThreadKey, Some(NOT_SHELL)),
        KeyBinding::new(&k("term"), ToggleTerminal, None), KeyBinding::new("`", ToggleTerminal, Some(TERM_BARE)), KeyBinding::new("shift-escape", DismissTerminal, None)]);
    // Super+` (off a Mac: Cmd+` there goes from window to window) toggles the terminal too, from the shell as well, as in the old client
    if !mac() { v.push(KeyBinding::new("super-`", ToggleTerminal, None)); }
    // Shift with a symbol arrives as the shifted symbol and no Shift (Ctrl+Shift+] is "ctrl-}" from Linux): the chords that hold one are bound that way too
    for (key, a) in [(k("prev"), Box::new(PrevThread) as Box<dyn Action>), (k("next"), Box::new(NextThread)), (k("stop"), Box::new(StopTurn))] {
        if let Some(alias) = shifted(&key) { v.push(KeyBinding::load(&alias, a, None, false, None, &DummyKeyboardMapper).expect("a valid keystroke")); }
    }
    v.extend([KeyBinding::new("1", Account1, Some("AccountMenu")), KeyBinding::new("2", Account2, Some("AccountMenu")), KeyBinding::new("3", Account3, Some("AccountMenu")), KeyBinding::new("4", Account4, Some("AccountMenu")), KeyBinding::new("5", Account5, Some("AccountMenu")),
        KeyBinding::new("6", Account6, Some("AccountMenu")), KeyBinding::new("7", Account7, Some("AccountMenu")), KeyBinding::new("8", Account8, Some("AccountMenu")), KeyBinding::new("9", Account9, Some("AccountMenu"))]);
    let alt = if mac() { Some(NOT_MAC_COMPOSER) } else { None };
    macro_rules! digits { ($($n:literal $p:ident $t:ident),*) => { v.extend([$(KeyBinding::new(concat!("alt-", $n), $p, alt), KeyBinding::new(concat!("secondary-", $n), $t, None)),*]); } }
    digits!("1" Project1 Thread1, "2" Project2 Thread2, "3" Project3 Thread3, "4" Project4 Thread4, "5" Project5 Thread5, "6" Project6 Thread6, "7" Project7 Thread7, "8" Project8 Thread8, "9" Project9 Thread9);
    v.extend([KeyBinding::new("b", RailBoard, Some(RAIL)), KeyBinding::new("s", RailSchematic, Some(RAIL)), KeyBinding::new("3", Rail3d, Some(RAIL)), KeyBinding::new("v", RailSplit, Some(RAIL)),
        KeyBinding::new("m", RailMech, Some(RAIL)), KeyBinding::new("f", RailFiles, Some(RAIL)), KeyBinding::new("d", RailDiff, Some(RAIL)), KeyBinding::new("r", RailReview, Some(RAIL)),
        KeyBinding::new("w", RailBrowser, Some(RAIL))]);
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
    let own: Vec<String> = [toggle, "shift-escape", "secondary-shift-c", "secondary-shift-v", "super-`"].iter().chain(if mac() { &["cmd-c", "cmd-v"][..] } else { &[][..] })
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
        /* i18n-ok: key contexts */ v.extend(["DrawPad", "PadLength"].map(|ctx| KeyBinding::new(&key, NoAction, Some(ctx))));
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
/// The threads Ctrl+1…9 go to, in order: the open project page's, else the project holding `sel`'s, else the first project's (0.14's Side.here).
pub fn here_threads(data: &Value, sel: &str, page: Option<&str>) -> Vec<String> {
    let pid = new_target(data, sel, page);
    let rows = sidebar(data);
    rows.iter().find(|(p, _)| raw(p) == pid).or(rows.first()).map_or(vec![], |r| r.1.iter().take(9).cloned().collect())
}
/// The `n`th (1-based) of `here_threads`.
pub fn nth_thread(data: &Value, sel: &str, page: Option<&str>, n: usize) -> Option<String> { here_threads(data, sel, page).get(n.checked_sub(1)?).cloned() }
/// The projects Alt+1…9 go to, in order (raw ids).
pub fn digit_projects(data: &Value) -> Vec<String> { sidebar(data).into_iter().take(9).map(|x| raw(&x.0).to_string()).collect() }
/// The project Ctrl+N starts a thread in, and whose threads Ctrl+1…9 count: the project page on show (a thread selected shuts it), else the open thread's, else the first.
pub fn new_target(data: &Value, sel: &str, page: Option<&str>) -> String {
    if let Some(p) = page.filter(|p| sidebar(data).iter().any(|(id, _)| raw(id) == *p)) { return p.to_string() }
    let pid = project_of(data, sel); if !pid.is_empty() { return pid }
    nth_project(data, 1).unwrap_or_default()
}
pub fn nth_project(data: &Value, n: usize) -> Option<String> { digit_projects(data).get(n.checked_sub(1)?).cloned() }

/// What a held modifier shows (0.14's workspace-hints.js): the secondary key (Ctrl, Cmd on a Mac) alone shows the keys on the window's face (the thread digits,
/// the rail's letters, the palette's and the model chip's keys, the legend at the sidebar's foot); Alt alone the project digits. Any other mix shows nothing.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Held { #[default] None, Threads, Projects }
pub fn held(m: &Modifiers, mac: bool) -> Held {
    let secondary = if mac { m.platform && !m.control } else { m.control && !m.platform };
    if secondary && !m.alt && !m.shift && !m.function { Held::Threads }
    else if m.alt && !m.control && !m.platform && !m.shift && !m.function { Held::Projects }
    else { Held::None }
}
/// A hint's keycaps: "Ctrl+Shift+[" is Ctrl, Shift and [; a Mac's "⌘⇧[" is one cap.
pub fn caps(hint: &str) -> Vec<String> {
    if hint.is_empty() { return vec![] }
    if mac() || hint.len() == 1 { return vec![hint.to_string()] }
    let (body, plus) = match hint.strip_suffix("++") { Some(b) => (b, true), None => (hint, false) };
    let mut v: Vec<String> = body.split('+').map(str::to_string).collect(); if plus { v.push("+".into()) } v
}
/// Two hints' keycaps side by side, their shared modifiers once (Ctrl Shift [ ], as the old legend).
pub fn caps_pair(a: &str, b: &str) -> Vec<String> {
    let (x, y) = (caps(a), caps(b));
    if x.len() == y.len() && x.len() > 1 && x[..x.len() - 1] == y[..y.len() - 1] { let mut v = x.clone(); v.push(y[y.len() - 1].clone()); v } else { x.into_iter().chain(y).collect() }
}
/// The legend at the sidebar's foot while the secondary key is held: (keycaps, words); the last row opens Commands, which lists every key.
pub fn legend(l: &Local) -> Vec<(Vec<String>, String)> {
    vec![(caps_pair(&hint(l, "prev"), &hint(l, "next")), trs("prev / next thread")), (caps(&hint(l, "new")), trs("new thread here")), (caps(&hint(l, "threads")), trs("jump to a thread here")),
        (caps(&hint(l, "focus")), trs("write a message")), (caps(&hint(l, "palette")), trs("all shortcuts"))]
}
fn has_tool(data: &Value, action: &str) -> bool { data["thread"]["tools"].as_array().into_iter().flatten().any(|t| t["action"] == action) }

struct Tip(SharedString, bool);
impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let c = |n: &str| theme::color(self.1, n);
        div().px_2().py_1().bg(c("panel")).border_1().border_color(c("line")).text_size(px(11.)).text_color(c("text")).child(self.0.clone())
    }
}
/// A tooltip builder for `.tooltip(...)`: `text` through the catalog (English written in the code; a name or title the catalog does not know comes
/// back as it is), then its key when it has one.
pub fn tip(text: &str, key: &str, light: bool) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let t: SharedString = tip_text(&tr(text), key).into();
    move |_, cx| cx.new(|_| Tip(t.clone(), light)).into()
}
/// `tip` for a text already in the language in force (made with `tf`), so it is not looked up a second time.
pub fn tip_raw(text: &str, key: &str, light: bool) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let t: SharedString = tip_text(text, key).into();
    move |_, cx| cx.new(|_| Tip(t.clone(), light)).into()
}
/// A tooltip's words: the text, then its key when it has one.
/* i18n-ok: the text and its key, no words */ pub fn tip_text(text: &str, key: &str) -> String { if key.is_empty() { text.to_string() } else { format!("{text} · {key}") } }

impl Backplane {
    /// A rail button pressed, or its key: the viewer's kind (or Diff) is asked for; the kind already lit shuts the viewer.
    pub(crate) fn rail_press(&mut self, a: &str, v: &str, cx: &mut Context<Self>) {
        let cmd = if a == "view-kind" { self.rail_view(v) } else { Command::Action(a.into(), v.into()) };
        let _ = self.tx.try_send(cmd); cx.notify();
    }
    /// The command a rail press on viewer kind `v` sends (screens::viewer::rail_command), with the rail lit at once for what it asks: the kind, or none.
    pub(crate) fn rail_view(&mut self, v: &str) -> Command {
        let thread = &self.data["thread"];
        let cmd = crate::screens::viewer::rail_command(&thread["viewer"], thread["diff"].is_object(), self.screens.rail_pending.as_deref(), v);
        let opens = matches!(&cmd, Command::Action(_, k) if !k.is_empty());
        self.screens.rail_pending = Some(if opens { v.to_string() } else { String::new() }); self.screens.rail_at = Some(std::time::Instant::now());
        self.screens.plot_click = (opens && matches!(v, "board" | "schematic")).then(std::time::Instant::now);
        cmd
    }
    /// A bare key does nothing while something covers the page.
    fn covered(&self) -> bool { self.screens.palette.open || self.screens.lightbox.is_some() || self.screens.picker_open || self.screens.acct.menu || crate::screens::settings::is_open(self) || self.data["tour"].is_object() || crate::screens::migrate::is_up(&self.data) }
    /// The hints on show now: what is held, but none while the window is not the one in use, something covers it, the pad is open or the keyboard is in the shell
    /// (every key is the shell's there); on a Mac Alt shows nothing from the message box, where Option+digit types a character.
    pub(crate) fn hint_mode(&self, window: &Window, cx: &App) -> Held {
        let h = held(&window.modifiers(), mac());
        if h == Held::None || !window.is_window_active() || self.covered() || self.screens.pad.is_some() || self.screens.term.focus.as_ref().map_or(false, |f| f.contains_focused(window, cx)) { return Held::None }
        if h == Held::Projects && mac() && self.input.focus_handle(cx).contains_focused(window, cx) { return Held::None }
        h
    }
    /// Esc once nothing over the page took it (0.14's WS.esc.next, after the terminal): the plot's tapped piece goes, else the viewer shuts as its lit rail button
    /// does and the keyboard goes back to the message box; with nothing to put away the keyboard leaves the message box, so a bare key may press a rail button
    /// ("/" or Esc again brings it back), and from anywhere else it goes back to it. Never from a field, a shell or a web page, whose Esc it is. True when it acted.
    pub(crate) fn escape_rest(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        // the token panel (the Context chip's) shuts first, as 0.14's did
        if self.screens.ctx_open { self.screens.ctx_open = false; return true }
        if window.context_stack().iter().any(|k| ESC_OWNERS.iter().any(|o| k.contains(o))) { return false }
        let mut put = false;
        for slot in 0..3 { let pv = self.screens.pv(slot); if pv.picked.is_some() || pv.card.is_some() || !pv.lit.is_empty() { pv.picked = None; pv.card = None; pv.lit.clear(); put = true; } }
        if put { return true }
        // a web page with the keyboard keeps its Esc (the old client gave the bot's page every key)
        let in_viewer = window.context_stack().iter().any(|k| k.contains("Viewer"));
        if in_viewer && crate::screens::viewer::active_kind(&self.data["thread"]["viewer"], None) == "browser" { return false }
        if crate::screens::viewer::is_open(&self.data["thread"]) {
            self.screens.rail_pending = Some(String::new()); self.screens.rail_at = Some(std::time::Instant::now());
            let _ = self.tx.try_send(crate::screens::viewer::close()); self.want_focus = true; return true
        }
        // (not a blur: with nothing focused the window hears no keys at all; the window's own handle has only its shortcuts)
        if self.input.focus_handle(cx).is_focused(window) { window.focus(&self.screens.root_focus); return true }
        self.want_focus = true; true
    }
    /// A modifier went down or up: draw again only when what the hints show changes.
    pub(crate) fn hints_maybe(&mut self, window: &Window, cx: &mut Context<Self>) { if self.hint_mode(window, cx) != self.screens.held { cx.notify(); } }
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
    /* i18n-ok: key contexts */ let el = el.key_context(if picker { "Backplane Picker" } else { "Backplane" }).on_modifiers_changed(cx.listener(|this, _: &ModifiersChangedEvent, window, cx| this.hints_maybe(window, cx)));
    macro_rules! on { ($el:expr, $a:ty, $f:expr) => { $el.on_action(cx.listener(|this, _: &$a, window, cx| { let _ = &window; ($f)(this, window, cx) })) } }
    let el = on!(el, NewThread, |this: &mut Backplane, _, cx: &mut Context<Backplane>| {
        let pid = new_target(&this.data, &this.selected, this.project_page.as_deref());
        if !pid.is_empty() { this.arm_new(); let _ = this.tx.try_send(Command::Action("new-thread".into(), pid)); cx.notify(); }
    });
    let el = on!(el, OpenDiff, |this: &mut Backplane, _, cx: &mut Context<Backplane>| this.key_rail("diff", "", cx));
    let el = on!(el, OpenFiles, |this: &mut Backplane, _, cx: &mut Context<Backplane>| this.key_rail("view-kind", "files", cx));
    let el = on!(el, StopTurn, |this: &mut Backplane, _, cx: &mut Context<Backplane>| if has_tool(&this.data, "interrupt") { let _ = this.tx.try_send(Command::Action("interrupt".into(), String::new())); cx.notify(); });
    let el = on!(el, SettleThread, |this: &mut Backplane, _, cx: &mut Context<Backplane>| for a in ["settle", "unsettle"] { if has_tool(&this.data, a) { let _ = this.tx.try_send(Command::Action(a.into(), String::new())); cx.notify(); break } });
    let el = on!(el, ModelPicker, |this: &mut Backplane, w: &mut Window, cx: &mut Context<Backplane>| if this.data["thread"].is_object() { this.combo_model(w, cx) });
    let el = on!(el, AccountMenu, |this: &mut Backplane, w: &mut Window, cx: &mut Context<Backplane>| if this.data["thread"].is_object() { this.combo_account(w, cx) });
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
    let el = on!(el, FocusComposer, |this: &mut Backplane, w: &mut Window, cx: &mut Context<Backplane>| {
        // the Browser page (and its address bar, whose context the viewer's overrides) types its own "/"
        if w.context_stack().iter().any(|k| k.contains("Viewer")) && crate::screens::viewer::active_kind(&this.data["thread"]["viewer"], None) == "browser" { cx.propagate(); return }
        this.want_focus = true; cx.notify();
    });
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
        for c in ["Composer", "TextInput", "PickInput", "AskInput", "ImpInput", "Viewer", "BrowserBar", "FilesPage"] { for k in ["/", "b", "s", "3", "v", "m", "f", "d", "r", "w"] { if k == "/" && c == "Viewer" { continue } assert_eq!(d(k, &[c]), "", "{k} in {c}"); } }
        assert_eq!(d("/", &["Viewer"]), "keys::FocusComposer", "a key back to the message box from the board, schematic or 3D");
        // from a board or schematic plot the rail's letters act (a click on the board is where the keyboard is while a person looks at it); the 3D view keeps F and its digits
        assert_eq!(d("s", &["Viewer Plot"]), "keys::RailSchematic"); assert_eq!(d("3", &["Viewer Plot"]), "keys::Rail3d"); assert_eq!(d("b", &["Viewer Plot"]), "keys::RailBoard"); assert_eq!(d("f", &["Viewer Plot"]), "keys::RailFiles");
        assert_eq!(d("/", &["Viewer Plot"]), "keys::FocusComposer"); assert_eq!(d("s", &["Viewer Plot", "TextInput"]), "", "a field over the plot keeps its typing");
        assert_eq!(d("/", &["Viewer", "FilesPage"]), "", "the Files page's filter"); assert_eq!(d("/", &["Viewer", "Terminal"]), "", "the shell's");
        assert_eq!(d("ctrl-shift-s", &[]), "keys::SettleThread");
    }
    #[test] fn ctrl_l_puts_the_keyboard_in_the_message_box_from_anywhere_but_the_shell_and_the_browser() {
        for c in [&[][..], &["TextInput"][..], &["Viewer"][..], &["Viewer", "FilesPage"][..], &["AskInput"][..], &["Composer"][..]] { assert_eq!(d("ctrl-l", c), "keys::FocusComposer", "{c:?}"); }
        for c in ["Terminal", "BrowserBar", "BrowserView", "CefPage"] { assert_eq!(d("ctrl-l", &["Viewer", c]), "", "{c} keeps its own Ctrl+L"); }
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
    #[test] fn shift_with_a_symbol_also_binds_as_the_keyboard_reports_it() {
        assert_eq!(shifted("secondary-shift-]").as_deref(), Some("secondary-}")); assert_eq!(shifted("secondary-shift-[").as_deref(), Some("secondary-{"));
        assert_eq!(shifted("alt-shift-."), Some("alt->".into())); assert_eq!(shifted("ctrl-."), None); assert_eq!(shifted("secondary-shift-f"), None); assert_eq!(shifted("shift-escape"), None);
        // Ctrl+Shift+] from a Linux keyboard is "ctrl-}" (gpui keeps Shift only on letters): both spellings go to the thread
        assert_eq!(d("ctrl-}", &[]), "keys::NextThread"); assert_eq!(d("ctrl-{", &["Composer"]), "keys::PrevThread"); assert_eq!(d("ctrl-shift-]", &[]), "keys::NextThread");
        assert_eq!(d("ctrl-}", &["Terminal"]), "", "the shell's"); assert_eq!(d("ctrl-{", &["DrawPad"]), "", "nothing behind the pad");
        let mut l = Local::default(); assert!(l.set("shortcut", "next\u{1f}alt-shift-]")); assert_eq!(does(&l, "alt-}", &[]), "keys::NextThread"); assert_eq!(does(&l, "ctrl-}", &[]), "", "the old one goes");
    }
    #[test] fn held_ctrl_shows_the_thread_keys_and_alt_the_projects() {
        let m = |control, alt, shift, platform| Modifiers { control, alt, shift, platform, function: false };
        assert_eq!(held(&m(true, false, false, false), false), Held::Threads); assert_eq!(held(&m(false, true, false, false), false), Held::Projects);
        assert_eq!(held(&m(true, false, true, false), false), Held::None, "Ctrl+Shift: a chord on its way, no hints"); assert_eq!(held(&m(true, true, false, false), false), Held::None);
        assert_eq!(held(&m(false, false, false, true), false), Held::None, "Super is not the secondary key off a Mac"); assert_eq!(held(&Modifiers::default(), false), Held::None);
        assert_eq!(held(&m(false, false, false, true), true), Held::Threads, "Cmd on a Mac"); assert_eq!(held(&m(true, false, false, false), true), Held::None, "Ctrl is not Cmd");
    }
    #[test] fn the_digits_on_show_are_where_the_keys_go() {
        let data = json!({"projects": [{"id": "h|p1", "threads": [{"id": "h|a"}, {"id": "h|b"}]}, {"id": "h|p2", "threads": (0..12).map(|i| json!({"id": format!("h|c{i}")})).collect::<Vec<_>>()}]});
        assert_eq!(here_threads(&data, "b", None), vec!["h|a", "h|b"]); assert_eq!(here_threads(&data, "c3", None).len(), 9, "nine at most");
        for n in 1..=9 { assert_eq!(nth_thread(&data, "c3", None, n), here_threads(&data, "c3", None).get(n - 1).cloned()) }
        assert_eq!(here_threads(&data, "a", Some("p2"))[0], "h|c0", "the project page on show counts first"); assert_eq!(here_threads(&data, "", Some("gone"))[0], "h|a", "a page that is gone does not");
        assert_eq!(here_threads(&data, "", None)[0], "h|a"); assert_eq!(digit_projects(&data), vec!["p1", "p2"]); assert_eq!(nth_project(&data, 2).as_deref(), Some("p2"));
        assert_eq!(new_target(&data, "a", Some("p2")), "p2"); assert_eq!(new_target(&data, "c1", None), "p2"); assert_eq!(new_target(&Value::Null, "", None), "");
    }
    #[test] fn the_legend_reads_the_keys_this_device_uses() {
        if mac() { return }
        let l = Local::default();
        let g = crate::i18n::with_lang("en", || legend(&l));
        assert_eq!(g[0].0, vec!["Ctrl", "Shift", "[", "]"]); assert_eq!(g[1].0, vec!["Ctrl", "N"]); assert_eq!(g[2].0, vec!["Ctrl", "1…9"]); assert_eq!(g[3].0, vec!["/"]); assert_eq!(g[4], (vec!["Ctrl".to_string(), "K".to_string()], "all shortcuts".to_string()));
        let mut l = Local::default(); assert!(l.set("shortcut", "new\u{1f}ctrl-alt-n")); assert_eq!(legend(&l)[1].0, vec!["Ctrl", "Alt", "N"], "a rebound key reads as rebound");
        assert_eq!(caps("Ctrl++"), vec!["Ctrl", "+"]); assert_eq!(caps_pair("Ctrl+D", "Alt+E"), vec!["Ctrl", "D", "Alt", "E"]); assert_eq!(caps(""), Vec::<String>::new());
    }
    /// The legend's words and every shortcut's label and scope (drawn by Settings through `trs`) are in Japanese; the keys stay as they are.
    #[test] fn the_shortcuts_read_in_japanese() {
        crate::i18n::with_lang("ja", || {
            let g = legend(&Local::default());
            assert!(g.iter().all(|(_, w)| !w.chars().any(|c| c.is_ascii_alphabetic())), "{g:?}");
            for x in TABLE { for w in [x.label, x.scope] { assert_ne!(trs(w), w, "{} has no Japanese", w); } }
            assert_eq!(tip_text(&tr("New thread"), "Ctrl+N"), "新しいスレッド · Ctrl+N");
        });
    }
    #[test] fn super_backtick_toggles_the_terminal_off_a_mac_even_from_the_shell() {
        if mac() { return }
        for ctx in [&[][..], &["Composer"][..], &["Terminal"][..], &["Viewer"][..]] { assert_eq!(d("super-`", ctx), "keys::ToggleTerminal", "{ctx:?}"); }
        assert_eq!(d("super-`", &["DrawPad"]), ""); assert_eq!(hint(&Local::default(), "term-super"), "Super+`");
        assert_eq!(rail_letter("diff"), "d", "Diff's bare key, as the rail's keycap and tooltip say");
    }
    #[test] fn hints_follow_the_hubs_actions() {
        assert_eq!(id_for("diff", "", ""), "diff"); assert_eq!(id_for("interrupt", "", ""), "stop"); assert_eq!(id_for("new-thread", "p1", "p1"), "new"); assert_eq!(id_for("new-thread", "p2", "p1"), "");
        assert_eq!(hint(&Local::default(), "stop"), show("ctrl-.", mac())); assert_eq!(rail_letter("3d"), "3");
    }
}
