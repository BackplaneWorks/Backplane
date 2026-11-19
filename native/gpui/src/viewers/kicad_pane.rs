//! The real KiCad editors in the viewer pane (docs/embedded-editors.md): the hub runs the project's GUI KiCad (kicad/embed.rs) and publishes
//! info `editor.kicad.<digest>`; this page makes a container window in the client's window (native_child.rs), tells the hub to attach KiCad to
//! it (`editor.kicad.attach`), and after every frame gpui draws puts the container over the pane with the rail's editor (Board: PCB,
//! Schematic: SCH, 3D: PCB_3D) mapped in it and the others unmapped, cutting holes where gpui drew something over the pane (the palette, a menu).
//! Stock KiCad on Linux makes its frames as top-levels: a tick adopts them into the container (the fork's `--embedded` makes them there).
//! On Windows, only the fork is embedded (`WS_CHILD` under the container HWND); stock mode explains that Backplane KiCad is needed.
use super::native_child as nc;
use crate::{hub::Command, screens::settings::Secret};
use gpui::{prelude::*, *};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

/// The client window's native handle (X11 XID or Win32 HWND; None elsewhere).
fn top_of(window: &Window) -> Option<u64> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle as R};
    HasWindowHandle::window_handle(window).ok().map(|h| h.as_raw()).and_then(|h| match h {
        R::Xcb(x) => Some(x.window.get() as u64),
        R::Xlib(x) => Some(x.window as u64),
        R::Win32(w) => Some(w.hwnd.get() as usize as u64),
        // macOS: the window's number (CGWindowID), what KiCad orders its frames above (native_child_mac.rs)
        #[cfg(target_os = "macos")]
        R::AppKit(a) => nc::window_number(a.ns_view.as_ptr()),
        _ => None,
    })
}

/// The editor tabs: (kind, label). Board, Schematic and 3D always; the symbol and footprint editors once KiCad has them (or the person asks).
pub const TABS: &[(&str, &str)] = &[("PCB", "Board"), ("SCH", "Schematic"), ("PCB_3D", "3D"), ("SYMBOL_EDITOR", "Symbols"), ("FOOTPRINT_EDITOR", "Footprints"), ("GERBVIEW", "Gerbers"), ("SIMULATOR", "Simulation")]; // i18n-ok: wire kinds; labels go through trs
/// The tab Ctrl+Tab (step 1) or Ctrl+Shift+Tab (step -1) goes to from `at`, round the tabs.
pub fn next_tab(at: &str, step: i32) -> &'static str {
    let i = TABS.iter().position(|(k, _)| *k == at).unwrap_or(0) as i32;
    TABS[(i + step).rem_euclid(TABS.len() as i32) as usize].0
}

/// Ctrl+Tab's next tab where KiCad's Simulator is only gone to once KiCad has it (`have`: the kinds of its frames): it is opened from the
/// rail, never by going round the tabs (a runtime without it would refuse; one with it would load the schematic for a key press).
pub fn next_open_tab(at: &str, step: i32, have: &[String]) -> &'static str {
    let mut to = next_tab(at, step);
    while to == SIM && !have.iter().any(|h| h == SIM) { to = next_tab(to, step); }
    to
}
/// KiCad's Simulator frame (bp.13.2, capability `sim-frame`): the rail's Simulation entry opens the pane on it.
pub const SIM: &str = "SIMULATOR"; // i18n-ok: wire frame kind
/// The frame the rail's Simulation entry opens, from the hub's viewer (`simFrame`: its KiCad embeds the Simulator and the project has a
/// schematic), where this window can hold KiCad; None: no entry.
pub fn rail_simulation(viewer: &Value, local: bool) -> Option<String> {
    viewer["simFrame"].as_str().filter(|f| local && !f.is_empty()).map(str::to_string)
}
/// The hub's frame kind for a rail page.
/// An editor.kicad.prewarm reply that builds nothing: refused (not ok), `prewarming: []`, or (a hub before 676b72333)
/// `prewarmed: []` with no `prewarming`. `prewarming: true` (an older hub's memory refusal) cannot be told apart: kept.
pub fn prewarm_builds_nothing(reply: &Value) -> bool {
    if reply["ok"] != true { return true }
    match reply.get("prewarming") {
        Some(v) => v.as_array().is_some_and(|a| a.is_empty()),
        None => reply["prewarmed"].as_array().is_some_and(|a| a.is_empty()),
    }
}
pub fn frame_for(open: &str) -> &'static str { match open { "schematic" => "SCH", "3d" => "PCB_3D", _ => "PCB" } } // i18n-ok: wire frame kind enum names
/// The editor an attach names for the page it was pressed on (#1347): a design page its own (Board, Schematic, 3D); a page with no editor
/// of its own (Files, Review, Mech, Browser, Split) none, and the hub opens a document the project has (the board, or the schematic of a
/// project without one: #1334's attach_frame). The pane then follows the frame KiCad shows (set_info's `active`).
pub fn page_frame(open: &str) -> Option<&'static str> { matches!(open, "board" | "schematic" | "3d").then(|| frame_for(open)) } // i18n-ok: rail page kinds

fn is_abs_path(s: &str) -> bool {
    s.starts_with('/') || s.starts_with('\\')
        || (s.len() >= 3 && s.as_bytes()[0].is_ascii_alphabetic() && s.as_bytes()[1] == b':' && matches!(s.as_bytes()[2], b'/' | b'\\'))
}

fn norm_path(s: &str) -> String {
    let mut n = s.replace('\\', "/");
    while n.len() > 1 && n.ends_with('/') && !(n.len() == 3 && n.as_bytes()[1] == b':') {
        n.pop();
    }
    if n.len() >= 2 && n.as_bytes()[0].is_ascii_alphabetic() && n.as_bytes()[1] == b':' {
        n.make_ascii_lowercase();
    }
    n
}

/// The project file the hub attaches to, from a viewer key `<kind>|<root>|<file>` (the file relative to the root, or absolute).
pub fn project_of(key: &str) -> Option<String> {
    let mut p = key.splitn(3, '|'); let (_, root, file) = (p.next()?, p.next()?, p.next()?);
    // Gerbers vs board ("gerbercmp\n<set>\n<board>": kicad/gerber) is its set's project
    let file = file.strip_prefix("gerbercmp\n").map_or(file, |r| r.split('\n').next().unwrap_or(r));
    let r = root.trim_end_matches(['/', '\\']);
    if file.is_empty() { return (!r.is_empty()).then(|| r.to_string()) }
    if is_abs_path(file) || r.is_empty() { return Some(file.to_string()) }
    let sep = if r.contains('\\') && !r.contains('/') { '\\' } else { '/' };
    Some(format!("{r}{sep}{file}"))
}

/// The project root directory from a viewer key `<kind>|<root>|<file>`, trimmed of any trailing `/` or `\`.
pub fn root_of(key: &str) -> Option<String> {
    let mut p = key.splitn(3, '|'); let (_, root, _) = (p.next()?, p.next()?, p.next()?);
    let r = root.trim_end_matches(['/', '\\']);
    (!r.is_empty()).then(|| r.to_string())
}

/// The `(project, root)` to attach KiCad to: the viewer key's own path when the hub's picture has arrived, else the sidebar project's `root`
/// (an optimistic rail press draws `Edit` before the hub's `viewer.key` round-trip completes).
pub fn edit_target(data: &Value, key: &str, pid: &str) -> Option<(String, String)> {
    if let Some(project) = project_of(key) {
        return Some((project, root_of(key).unwrap_or_default()));
    }
    let root = crate::screens::projects::find(data, pid)
        .and_then(|p| p["root"].as_str())
        .map(|r| r.trim_end_matches(['/', '\\']))
        .filter(|r| !r.is_empty())?;
    Some((root.to_string(), root.to_string()))
}

/// What a shown thread asks warm: the KiCad file the viewer shows (Edit's own target), else its folder (a viewer on a STEP or a page of
/// another kind leaves the folder's choice to the hub, as before).
pub fn warm_target(project: String, root: String) -> String {
    let kicad = [".kicad_pcb", ".kicad_sch", ".kicad_pro"].iter().any(|x| project.ends_with(x));
    if kicad || root.is_empty() { project } else { root }
}

/// Whether the pre-warm may start by itself on an idle pane (2 s after the first editor is placed, with no new placement): everywhere but
/// Windows, and there only on a runtime whose pre-warm never takes the keyboard from the shown editor (`prewarm-keeps-focus`).
pub fn idle_prewarm(windows: bool, keeps_focus: bool) -> bool { !windows || keeps_focus }

/// Path without any file extension on its final segment (dots in parent folder names are preserved).
pub fn path_stem(s: &str) -> &str {
    let s = s.trim_end_matches(['/', '\\']);
    let base_at = s.rfind(['/', '\\']).map_or(0, |i| i + 1);
    match s[base_at..].rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => &s[..base_at + stem.len()],
        _ => s,
    }
}

/// Whether a pane opened for `(pane_proj, pane_root, pane_pid)` belongs to the viewer showing `(viewer_key, cur_pid)`.
pub fn belongs_to(pane_proj: &str, pane_root: &str, pane_pid: &str, viewer_key: &str, cur_pid: &str) -> bool {
    if let Some(vr) = root_of(viewer_key) {
        if !pane_root.is_empty() { return norm_path(pane_root) == norm_path(&vr) }
    }
    if let Some(vp) = project_of(viewer_key) {
        let nr = norm_path(pane_root);
        let nvp = norm_path(&vp);
        if norm_path(path_stem(pane_proj)) == norm_path(path_stem(&vp))
            || (!nr.is_empty() && nvp.starts_with(&format!("{nr}/")))
        {
            return true;
        }
    }
    !pane_pid.is_empty() && pane_pid == cur_pid
}

/// The window to show for `want` among the container's frames, preferring the hub's own list (fork mode: kinds it knows) over titles.
/// In fork mode, wait until `info["frames"]` lists the window as shown so a half-opened frame parked in `holder` during `ShowFrame` is never mapped early.
pub fn pick(want: &str, info: &Value, frames: &[nc::Frame]) -> Option<u64> { pick_local(want, info, frames, &[]) }
/// `pick`, where `ever` are the fork's frames shown at least once (bp.9's local map: the client maps such a frame itself, no round trip).
pub fn pick_local(want: &str, info: &Value, frames: &[nc::Frame], ever: &[u64]) -> Option<u64> {
    if info["mode"] == "fork" && info["local_map"] == true {
        let x = info["frames"].as_array().into_iter().flatten().find(|f| f["kind"] == want).and_then(|f| f["xid"].as_u64());
        if let Some(x) = x.filter(|x| ever.contains(x) && frames.iter().any(|f| f.window == *x)) { return Some(x) }
    }
    if info["mode"] == "fork" {
        let hub = info["frames"].as_array().into_iter().flatten().find(|f| f["kind"] == want && f["shown"] != false).and_then(|f| f["xid"].as_u64());
        // macOS: KiCad's frames are another process's windows, never children of ours (nc::frames lists none): the hub's word is the list
        if cfg!(target_os = "macos") { return hub }
        return hub.filter(|x| frames.iter().any(|f| f.window == *x));
    }
    let hub = info["frames"].as_array().into_iter().flatten().find(|f| f["kind"] == want).and_then(|f| f["xid"].as_u64());
    hub.filter(|x| frames.iter().any(|f| f.window == *x)).or_else(|| frames.iter().find(|f| f.kind == want).map(|f| f.window))
}

/// Pure OS-aware status text when the editor is not on show, from the hub's state (`None`: the editor is there).
pub fn waiting_on(info: &Value, have_frame: bool, windows: bool) -> Option<String> {
    if windows && info["mode"] == "stock" {
        return Some(crate::i18n::trs("The embedded editor needs Backplane KiCad."));
    }
    // another live KiCad holds a document this one is about to open: the person answers before it opens (Open anyway / Cancel)
    if let Some(a) = info["lock_ask"].as_object() {
        let g = |k: &str| a.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        return Some(crate::i18n::tf("gpui.c.kicad.lock.ask", &[("file", g("file").into()), ("holder", g("holder").into())]));
    }
    match info["state"].as_str().unwrap_or("") {
        // the hub has already tried a missed start window once more (embed.rs start_retry); the pane's Try again under this starts it again
        "failed" => Some(format!("{} {}", crate::i18n::trs("KiCad could not start:"), info["reason"].as_str().unwrap_or(""))),
        // KiCad died after it was ready (a crash, a kill): not a failed start; what it means for the person's work, in plain words
        // KiCad kept an autosave (it offers it back, an agent's change in it too) wins; else an agent's change the hub knows was not on disk
        // (it was asked not to save it yet) is said lost
        "closed_unexpectedly" if info["autosaves"].as_array().map_or(true, |a| a.is_empty()) && info["agent_unsaved"].as_array().is_some_and(|a| !a.is_empty()) => Some(crate::i18n::t("gpui.c.kicad.closed.agent")),
        "closed_unexpectedly" => Some(crate::i18n::t(if info["autosaves"].as_array().is_some_and(|a| !a.is_empty()) { "gpui.c.kicad.closed.kept" } else { "gpui.c.kicad.closed.lost" })),
        "unresponsive" => Some(crate::i18n::trs("KiCad is not responding.")),
        "stopped" => Some(crate::i18n::trs("KiCad was closed.")),
        _ if have_frame => None,
        // KiCad has a dialog of its own open (a notice from loading a file, its own progress): said, not a silent "Starting KiCad…"
        _ if info["dialogs"].as_array().is_some_and(|d| !d.is_empty()) => {
            let title = info["dialogs"].as_array().and_then(|d| d.last()).and_then(Value::as_str).unwrap_or("");
            // KiCad's own question to the person (its offer to recover unsaved work after a crash): said as a question for them
            let key = if title.to_ascii_lowercase().contains("recover") { "gpui.c.kicad.asking" } else { "gpui.c.kicad.dialog" };
            Some(crate::i18n::tf(key, &[("title", title.to_string().into())]))
        }
        // KiCad's check or refill holds the switch (task-gate-reads): said, so the pane does not look dead; the hub sends it when it ends
        // macOS: KiCad's start waits on a system prompt that names Backplane (the hub's mac_prompt_wait): said, so the person looks for it
        // macOS: KiCad's own offer to recover unsaved work is a native alert before its API is up (no dialog event): the hub tells it from
        // the permission prompt by what the project's folder holds (embed.rs mac_wait_reason); said as KiCad's question, with the answers
        "starting" if info["reason"] == "kicad-recover-prompt" => Some(crate::i18n::t("gpui.c.kicad.recover.prompt")),
        "starting" if info["reason"] == "mac-permission-prompt" => Some(crate::i18n::trs("macOS is asking for permission for Backplane: answer the prompt to let KiCad start.")),
        _ if info["task_wait"].as_str().is_some_and(|w| !w.is_empty()) => Some(crate::i18n::tf("gpui.c.kicad.task.wait", &[("what", info["task_wait"].as_str().unwrap_or("").to_string().into())])),
        "starting" => Some(starting_words(info)),
        _ => Some(crate::i18n::trs("Starting KiCad…")),
    }
}

/// macOS: how long KiCad's frame may stay behind our window, counted from the first check that found it there (through every re-order
/// that did not hold), before the pane says so and offers Pop out; and how long it must then stay above to count as back.
pub const KICAD_RAISE_BOUND: std::time::Duration = std::time::Duration::from_secs(2);
pub const KICAD_RAISE_HELD: std::time::Duration = std::time::Duration::from_secs(1);

/// macOS: what the checks of the real window order have seen since the frame was last known above for good: how many found it behind
/// (each one re-orders), since when, and since when it has been above again.
#[derive(Debug, Default, PartialEq)] pub struct RaiseState { pub tries: u32, pub since: Option<std::time::Instant>, pub above_since: Option<std::time::Instant> }
/// One check of the real window order (pure, for the tests). `Above`: all is well. `Idle`: the frame is not on screen (hidden by KiCad
/// or by us), nothing to order. `Retry(n)`: on screen and not above ours, the n-th time: order again; past KICAD_RAISE_BOUND since the
/// first, `buried` (the pane says so, and the tries go on). `Holding`: above again after a retry, watched until it has stayed there for
/// KICAD_RAISE_HELD: an order that is undone a moment later (by the other side's own ordering) is the same failure, under the same
/// bound, not a new one each time. `Verified(n)`: it stayed, after n tries.
#[derive(Debug, PartialEq)] pub enum RaiseStep { Above, Idle, Retry(u32), Holding, Verified(u32) }
pub fn raise_step(above: bool, on_screen: bool, st: &mut RaiseState, buried: &mut bool, now: std::time::Instant) -> RaiseStep {
    if !above && !on_screen { *st = RaiseState::default(); *buried = false; return RaiseStep::Idle }
    if above {
        if st.tries == 0 { *buried = false; return RaiseStep::Above }
        let held = *st.above_since.get_or_insert(now);
        if now.duration_since(held) < KICAD_RAISE_HELD { return RaiseStep::Holding }
        let n = st.tries; *st = RaiseState::default(); *buried = false;
        return RaiseStep::Verified(n)
    }
    st.above_since = None;
    let since = *st.since.get_or_insert(now); st.tries += 1;
    if now.duration_since(since) >= KICAD_RAISE_BOUND { *buried = true; }
    RaiseStep::Retry(st.tries)
}
/// macOS: whose a normal on-screen window is, for the order rule: this pane's own window is `Ours`, the frame on show `Frame`, KiCad's
/// other windows (its dialogs) `Kicad`, our app's other windows `OurOther`, anyone else's `Other`.
#[derive(Debug, Clone, Copy, PartialEq)] pub enum Who { Ours, Frame, Kicad, OurOther, Other }
/// macOS: what may be done about the order, from the window server's list front to back (pure, for the tests). THE RULE: the client
/// orders one window only, its own, and only past its own and its KiCad's windows. It never orders, raises or lowers a window that is
/// not its own, and never moves its own window over or under another app's: with another app's window between ours and the frame (in
/// either direction) the only one who may act is KiCad, on its own frame (`AskKicad`: it puts the frame just above ours).
/// `Above`: the frame is above ours with nothing but KiCad's own windows between. `Idle`: the frame or our window is not on screen.
/// `OursBelow`: the frame is behind ours with only our own and KiCad's windows between: ours goes just below the frame.
#[derive(Debug, PartialEq)] pub enum OrderPlan { Above, Idle, OursBelow, AskKicad }
/// macOS: whether KiCad's frame is to be on show over the pane, from the state alone (never our window's activity or focus): an editor
/// wanted (`show`), our window seen (`host_visible`), and the pane's hitbox drawn this frame. A frame with no hitbox (our window inactive
/// while KiCad has the keyboard draws such frames, d563d8a5ab) keeps a frame on show while the page still holds the editor (`in_page`) or
/// KiCad has a dialog open (hiding its frame would strand the modal); the page no longer holding it (the viewer closed by X, rail, key
/// or the hub; another thread; Files, Mech, Browser, Gerbers; a Diff over it) hides it (#1199). Shown again only once the hitbox is
/// drawn: there is a rect to show it at. `covered`: a screen of Backplane's drawn over the whole pane (Settings, the palette, the
/// lightbox, Improvements: `covers_pane`): macOS cannot cut holes in KiCad's frame, which would stay above it, so it is hidden too.
pub fn mac_shown(show: bool, host_visible: bool, hitbox: bool, in_page: bool, covered: bool, dialog: bool, was_visible: bool) -> bool {
    show && host_visible && (!covered || (dialog && was_visible)) && (hitbox || (was_visible && (in_page || dialog)))
}
/// macOS: how long after the last push of a burst it is sent once more (`mac_watch`, KicadPane::mac_settle). Why: with bp.13.1 KiCad's
/// frame ended off the last rect pushed (e2e-macos 44255662838: 496x524 visible, then 496x500, the frame kept 496x524) or never shown
/// (44274808579 return 3, this send off). The hub sends each SetHostGeometry on KiCad's main API socket, one at a time, after KiCad
/// answered the one before; there KiCad applies it before it answers (fork embed, bp.13.2 349339a52d: only the events socket defers
/// and coalesces, and the hub never uses it). What did not apply was refused: AS_BUSY, "a task is running: DRC" (return 3), which the
/// client did not log or send again before HostSends; 44255662838's 500 was most likely the same, unlogged then. HostSends now sends a
/// refused push again until taken, and the fork's next batch lets SetHostGeometry through the task gate; this send stays until a run of
/// that fork and this client shows it is not needed (the KiCad lead). Repeating an applied state is harmless (fork embed: SetHostRect
/// re-places the frames where they are). BACKPLANE_KICAD_MAC_SETTLE_MS: another wait, 0 none (mac_pane_e2e's swap check runs with it off).
pub const MAC_SETTLE_MS: u64 = 300;
/// macOS: the host pushes (`editor.kicad.host`) in flight and the latest wanted, shared by the window's panes and the app (a pane
/// dropped at View still has its hide's answer to come). The hub answers one connection's requests in order, so the answer that leaves
/// none in flight is the latest push's: an earlier push's answer decides nothing. The latest refused (KiCad busy: "SetHostGeometry: a
/// task is running: DRC", e2e-macos 44274808579 return 3, the frame never shown; or an error) is sent again, as the latest push is then,
/// after a wait that grows (HOST_RETRY_MS, to HOST_RETRY_CAP_MS) until KiCad takes it or a newer push replaces it: a newer hide is never
/// undone by an older show sent again, and a stale timer finds nothing due.
/// Answers are told apart by order only: the client's rpc map forgets ids past 64 outstanding (hub.rs `rpcs`, `rpc_names`), and an
/// answer it forgot never comes back here, so the count stays up until HOST_LOST_MS takes it as lost and the latest is sent again; an
/// answer that comes after that (late, in order) counts down from 0 and decides as the latest's would: a refusal sends the latest again
/// (harmless), a take ends a retry the latest's own answer then still decides.
#[derive(Default)]
pub struct HostSends { inflight: u32, last: Option<Value>, retry_at: Option<std::time::Instant>, refused: u32, sent_at: Option<std::time::Instant>,
    /// the app's one watch (Backplane::host_retry_watch), woken by every push: it sleeps on this while nothing waits
    wake: Option<async_channel::Sender<()>> }
/// macOS: whether a host push is due: the pane's rect and visibility now differ from the last pushed, or what was pushed may not have
/// reached KiCad (`unsent`: the hub's connection was made again, kicad_rejoin), so the same rect is pushed again.
pub fn host_push_due(now: Option<(i32, i32, i32, i32, bool)>, sent: Option<(i32, i32, i32, i32, bool)>, unsent: bool) -> bool { now.is_some() && (now != sent || unsent) }
/// macOS: the longest a show is held for the pane's layout to settle (`show_settled`).
pub const MAC_SHOW_HOLD_MS: u64 = 100;
/// macOS: whether a host push may go now. A show (the frame hidden or not yet sent, now visible) at a rect other than the last one sent
/// waits until two frames in a row give the same rect, or MAC_SHOW_HOLD_MS: the page's first layout can be a transient one (e2e-macos
/// 44465483071 leave (b): 496x524, then 496x500 a frame later), and each push is a hub round trip of about 500 ms one after another, so
/// KiCad showed the frame at the transient rect and corrected it a push later: back in 1358 ms against 300. A show at the rect last
/// sent (back from a covering sheet) and every push while shown go at once. `held`: the rect waiting and since when.
/// `fresh`: the rect is this frame's layout (false: the watch's look between frames, which only lets a held show go at its time).
pub fn show_settled(now: (i32, i32, i32, i32, bool), sent: Option<(i32, i32, i32, i32, bool)>, held: &mut Option<((i32, i32, i32, i32), std::time::Instant)>, fresh: bool, t: std::time::Instant) -> bool {
    let r = (now.0, now.1, now.2, now.3);
    let showing = now.4 && !sent.is_some_and(|s| s.4);
    if !showing { *held = None; return true }
    if !fresh && held.is_none() { return true }
    if fresh && sent.is_some_and(|s| (s.0, s.1, s.2, s.3) == r) { *held = None; return true }
    match *held {
        Some((h, _)) if fresh && h == r => { *held = None; true }
        Some((_, since)) if t.duration_since(since) >= std::time::Duration::from_millis(MAC_SHOW_HOLD_MS) => { *held = None; true }
        Some((_, since)) => { *held = Some((r, since)); false }
        None => { *held = Some((r, t)); false }
    }
}
/// macOS: whether a yield_to's fallback (KiCad activated 400 ms after a hand-in that did not take) may still act: taken at generation
/// `mine`, the generation now `now` (native_child_mac.rs YIELD_GEN: a newer hand-in, Backplane taking the keyboard back, or KiCad's
/// frame leaving the screen each move it on, #1199).
pub fn yield_current(mine: u64, now: u64) -> bool { mine == now }
/// What the watch does next (`watch_step`): send this push again now, look again in a while, or sleep until a push wakes it.
#[derive(Debug, PartialEq)]
pub enum HostStep { Send(Value), Look(std::time::Duration), Idle }
pub const HOST_RETRY_MS: u64 = 250;
pub const HOST_RETRY_CAP_MS: u64 = 2000;
/// An answer this long late is taken as lost (the hub reconnected, the reply dropped): the latest push is due again.
pub const HOST_LOST_MS: u64 = 8000;
impl HostSends {
    /// A push sent now (its params, the project's): the latest wanted, any retry replaced by it.
    pub fn sent(&mut self, params: Value, now: std::time::Instant) {
        self.inflight += 1; self.last = Some(params); self.retry_at = None; self.sent_at = Some(now);
        if let Some(w) = &self.wake { let _ = w.try_send(()); } // the watch wakes for every push, the first one included (lane B)
    }
    /// The watch's waker (set once, when the app starts its watch).
    pub fn set_wake(&mut self, w: async_channel::Sender<()>) { self.wake = Some(w); }
    pub fn has_wake(&self) -> bool { self.wake.is_some() }
    /// The hub's answer to a push: when the latest was refused, when to send it again.
    pub fn reply(&mut self, ok: bool, now: std::time::Instant) -> Option<std::time::Instant> {
        self.inflight = self.inflight.saturating_sub(1);
        if self.inflight > 0 { return None }
        if ok { self.refused = 0; self.retry_at = None; return None }
        self.refused += 1;
        let ms = (HOST_RETRY_MS << (self.refused - 1).min(4)).min(HOST_RETRY_CAP_MS);
        let at = now + std::time::Duration::from_millis(ms); self.retry_at = Some(at); Some(at)
    }
    /// The push to send again now, if one is due (taken: the send that follows is a push of its own, `sent`).
    pub fn due(&mut self, now: std::time::Instant) -> Option<Value> {
        let lost = self.inflight > 0 && self.sent_at.is_some_and(|t| now.duration_since(t) >= std::time::Duration::from_millis(HOST_LOST_MS));
        if lost { self.inflight = 0; self.retry_at = Some(now); }
        // never with raise: whether KiCad's frame is ordered in is mac_order's to decide fresh, so a retry answered after a long DRC
        // cannot take the keyboard to KiCad by itself
        match self.retry_at { Some(t) if t <= now && self.inflight == 0 => { self.retry_at = None; self.last.clone().map(|mut p| { p["raise"] = json!(false); p }) } _ => None }
    }
    /// The watch's next step at `now`: a push due again (sent: counted in flight), a look in a while (at most HOST_RETRY_MS: a refusal
    /// that comes meanwhile is sent again in time), or nothing to wait for.
    pub fn watch_step(&mut self, now: std::time::Instant) -> HostStep {
        if let Some(p) = self.due(now) { self.sent(p.clone(), now); return HostStep::Send(p) }
        match self.next_look() { Some(at) => HostStep::Look(at.saturating_duration_since(now).min(std::time::Duration::from_millis(HOST_RETRY_MS))), None => HostStep::Idle }
    }
    /// When the next look is wanted (a retry waiting, or an answer outstanding that may be lost).
    pub fn next_look(&self) -> Option<std::time::Instant> {
        self.retry_at.or_else(|| (self.inflight > 0).then(|| self.sent_at.map(|t| t + std::time::Duration::from_millis(HOST_LOST_MS))).flatten())
    }
    /// A new hub (reconnected): nothing of the old one's is in flight.
    /// A new hub (reconnected): nothing of the old one's is in flight, and nothing it refused is sent again (the reattach's own
    /// placement is the latest from here).
    /// Answers the latest push it drops (kicad_rejoin sends a hide again when no pane will push).
    pub fn rejoined(&mut self) -> Option<Value> { self.inflight = 0; self.retry_at = None; self.refused = 0; self.sent_at = None; self.last.take().map(|mut p| { p["raise"] = json!(false); p }) }
}
/// The settle wait in force (MAC_SETTLE_MS, or BACKPLANE_KICAD_MAC_SETTLE_MS; 0: never sent again).
pub fn mac_settle_ms(env: Option<&str>) -> u64 { env.and_then(|v| v.trim().parse().ok()).unwrap_or(MAC_SETTLE_MS) }
/// Whether something drawn after the pane's own content covers all of the pane (`over`: what was drawn over it, each clipped to
/// the pane, as Window::hitbox_drawn_after gives it): a full-window screen (its backdrop), not a menu, a tooltip or a card.
pub fn covers_pane(pane: (f32, f32), over: &[(f32, f32)]) -> bool {
    pane.0 > 0. && pane.1 > 0. && over.iter().any(|o| o.0 >= pane.0 - 1. && o.1 >= pane.1 - 1.)
}

pub fn order_plan(stack: &[Who]) -> OrderPlan {
    let (Some(u), Some(f)) = (stack.iter().position(|w| *w == Who::Ours), stack.iter().position(|w| *w == Who::Frame)) else { return OrderPlan::Idle };
    let between = &stack[u.min(f) + 1..u.max(f)];
    if between.iter().any(|w| *w == Who::Other) { return OrderPlan::AskKicad }
    if f < u { if between.iter().all(|w| *w == Who::Kicad) { OrderPlan::Above } else { OrderPlan::AskKicad } } else { OrderPlan::OursBelow }
}
/// macOS: whether a re-order of ours also asks KiCad to order its frame above our window (`raise` in editor.kicad.host). KiCad's own
/// order only counts while KiCad is the active app: an inactive app's window is not let in front of the active app's key window, and a
/// frame ordered "above" ours then lands behind it, after our own order had already put it right. So: while KiCad is active, both (as
/// before); otherwise ours alone, and KiCad's too on every fourth try in case ours is the one that does not take.
/// BACKPLANE_KICAD_MAC_RAISE=always|never fixes it either way (the gate's A/B).
pub fn raise_also_kicad(policy: &str, kicad_active: bool, tries: u32) -> bool {
    match policy { "always" => true, "never" => false, _ => kicad_active || (tries > 0 && tries % 4 == 0) }
}
/// macOS: the next check of the real order, in ms: quick while an order of ours is unproven or something just changed, once a second
/// while the pane says the editor is behind, otherwise a steady look (another process orders its windows without telling us).
pub fn raise_watch_ms(tries: u32, buried: bool, hot: bool) -> u64 { if buried { 1000 } else if tries > 0 || hot { 33 } else { 250 } }

/// From this many seconds a start the pane is waiting on says what KiCad is doing and for how long.
pub const STARTING_SAY: u64 = 10;

/// "Starting KiCad…", and past STARTING_SAY what KiCad is doing (the hub's `start_phase`) and for how long (the pane's
/// `starting_for_s`): the release check's fresh-home first Edit sat on "Starting KiCad…" for minutes with nothing said.
pub fn starting_words(info: &Value) -> String {
    let secs = info["starting_for_s"].as_u64().unwrap_or(0);
    if secs < STARTING_SAY { return crate::i18n::trs("Starting KiCad…") }
    let what = match info["start_phase"].as_str().unwrap_or("") {
        "handover" => crate::i18n::tf("gpui.c.kicad.phase.handover", &[]),
        "spawned" => crate::i18n::tf("gpui.c.kicad.phase.spawned", &[]),
        "socket" => crate::i18n::tf("gpui.c.kicad.phase.socket", &[]),
        "answering" => crate::i18n::tf("gpui.c.kicad.phase.answering", &[]),
        "first show" | "ready" => crate::i18n::tf("gpui.c.kicad.phase.show", &[]),
        _ => crate::i18n::tf("gpui.c.kicad.phase.none", &[]),
    };
    crate::i18n::tf("gpui.c.kicad.starting.slow", &[("what", what.into()), ("secs", secs.to_string().into())])
}

/// KiCad's own dialog up while the keyboard is elsewhere (an agent's or the API's Plot, DRC...: it never takes the person's keyboard,
/// lead's rule): the strip says so, its title; None while KiCad has the keyboard (the dialog has it) or no dialog is up.
pub fn asking(dialogs: &[String], kicad_has_keys: bool) -> Option<String> {
    if kicad_has_keys { return None }
    dialogs.last().filter(|t| !t.is_empty()).map(|t| crate::i18n::tf("gpui.c.kicad.asking", &[("title", t.clone().into())]))
}

/// KiCad's notice the person has not dismissed (the hub's `notices`, from `embed-notice`: a message box KiCad would have shown over a
/// frame not on screen yet, "Could not use OpenGL, falling back to software rendering"): the newest, as (id, the line the pane shows).
pub fn open_notice(info: &Value) -> Option<(u64, String)> {
    let n = info["notices"].as_array()?.iter().rev().find(|n| n["dismissed"] != true)?;
    let (title, detail) = (n["title"].as_str().unwrap_or(""), n["detail"].as_str().unwrap_or(""));
    let text = if detail.is_empty() || title.is_empty() { format!("{title}{detail}") } else { format!("{title} ({detail})") }; // i18n-ok: KiCad's own (translated) title and detail joined; the line is the catalog's gpui.c.kicad.notice
    Some((n["id"].as_u64()?, crate::i18n::tf("gpui.c.kicad.notice", &[("what", text.into())])))
}

/// Whether a warm first Edit's hold goes on once its frame is placed: until KiCad says the frame is drawn (`embed-frame-drawn`; the 3 s
/// cap of KicadPane::handoff ends it if that never comes), else for HANDOFF_PAINT.
pub fn handoff_holds(drawn_event: bool, drawn: bool, since_placed: std::time::Duration) -> bool {
    if drawn_event { !drawn } else { since_placed < HANDOFF_PAINT }
}

/// The pane offers Try again under its message: KiCad's start failed (the hub has already retried a missed start window once).
pub fn try_again(info: &Value) -> bool { info["state"] == "failed" && !lock_asked(info) }
/// The pane asks about another KiCad's lock (Open anyway / Cancel).
pub fn lock_asked(info: &Value) -> bool { info["lock_ask"].is_object() }
/// KiCad closed unexpectedly after it was ready: the pane's one button opens the editor again (a new KiCad).
pub fn open_again(info: &Value) -> bool { info["state"] == "closed_unexpectedly" }
/// What the pane says when KiCad died after it was ready, by what is on disk (the hub's `autosaves`): saved files are as last saved;
/// KiCad's autosave of unsaved changes (_autosave-* beside the files, kept by the hub) is offered back when it opens the file again.

/// The pane offers the KiCad app under its message where the editor cannot be in the pane: its start failed (no window to hold it, no
/// KiCad that starts), or this is stock KiCad on Windows. The app opens only on that press: the pane never falls back to it by itself.
pub fn app_instead(info: &Value, windows: bool) -> bool { try_again(info) || (windows && info["mode"] == "stock") }

/// The strip offers Backplane KiCad while the pane holds a stock KiCad (the hub's `mode`; on Windows the pane says it in its middle instead).
pub fn stock_hint(info: &Value, windows: bool) -> bool { !windows && info["mode"] == "stock" && matches!(info["state"].as_str(), Some("starting" | "ready")) }

/// What the pane says when the editor is not on show, from the hub's state (None: the editor is there).
pub fn waiting(info: &Value, have_frame: bool) -> Option<String> {
    waiting_on(info, have_frame, cfg!(windows))
}

/// Decide whether a render pass should switch the editor's page from `hub_open` / `pressed_open`,
/// without letting a stale `hub_open` revert a local `set_open` while the hub's view round-trip is still plotting.
pub fn hub_open_step<'a>(cur_open: &str, last_hub_open: &mut String, hub_open: &'a str, pressed_open: Option<&'a str>) -> Option<&'a str> {
    if let Some(p) = pressed_open {
        return (p != cur_open).then_some(p);
    }
    if hub_open != last_hub_open {
        *last_hub_open = hub_open.to_string();
        if matches!(hub_open, "board" | "schematic" | "3d") && hub_open != cur_open { // i18n-ok: rail page kinds
            return Some(hub_open);
        }
    }
    None
}

pub struct KicadPane {
    /// when this pane was made (the first Edit's), its warm hand-over's end (the frame placed + HANDOFF_PAINT), and whether it is over
    /// since when this pane has seen KiCad starting, and the last whole second past STARTING_SAY it redrew for (the pane says
    /// what KiCad is doing from then: never a silent "Starting KiCad…", the release check's fresh-home stall)
    starting_since: Option<std::time::Instant>, starting_said: u64,
    born: std::time::Instant, handoff_placed: Option<(std::time::Instant, u64)>, handoff_over: bool,
    /// macOS: the pane's screen rect (points) and our window's visibility last pushed to KiCad (`editor.kicad.host`)
    pub host_sent: Option<(i32, i32, i32, i32, bool)>,
    /// macOS: our window is in full screen, where KiCad's frame cannot be: the pane offers Pop out instead (a window of its own)
    mac_full: bool,
    /// The page holds the editor this frame (ui.rs sets it from the state as it draws: the viewer open, this thread's design on a Board,
    /// Schematic or 3D page with no Gerber set or Diff over it, or popped out into its own window). macOS: a frame with no hitbox hides
    /// KiCad's frame only when this is false (`mac_shown`; #1199: the viewer closed, KiCad's PCB Editor stayed on screen)
    pub in_page: bool,
    /// macOS: KiCad's frame is not above our window by the window server's own list though we ordered ours below it (the Mac mini gate,
    /// Nov 16: a 1900 pt window on a 1920 pt display, the pane blank with the frame behind ours): the tries, since when, and whether the
    /// bound passed, when the pane says so and offers Pop out rather than staying blank
    mac_raise: RaiseState, mac_buried: bool, mac_watching: bool,
    /// macOS: until when the watch looks every frame's time (a push, a re-order, an activation or a press of ours just happened), and
    /// what was last seen, to tell a change (our window key, the frame on show)
    mac_hot: Option<std::time::Instant>, mac_seen: (bool, Option<u64>), mac_pressed: bool, mac_looked: Option<std::time::Instant>,
    /// the .kicad_pro the hub resolved this pane's project to (its attach reply), "" until then
    pub bound: String,
    pub project: String, pub root: String, pub project_id: String, pub open: String, last_hub_open: String, hub: tokio::sync::mpsc::Sender<Command>,
    /// Gerbers asked for before the pane was attached and KiCad ready ("Open in GerbView" from Backplane's Gerbers page): opened then
    want_gerbers: Vec<String>,
    /// the button now down was pressed over the editor (it keeps the keyboard until the button is up)
    press_in: bool,
    /// The keys went to KiCad while this press was still down outside the editor (its own tab, Full size): that press gave
    /// them, so it is not also the "click elsewhere" that hands them back (Windows: the tab's press did both)
    press_gave_keys: bool,
    /// when the hub was last told the person is working in KiCad's canvas
    busy_sent: Option<std::time::Instant>,
    container: Option<u64>, toplevel: u64, attached: bool, pub info: Value, frames: Vec<nc::Frame>,
    marker: Arc<Mutex<Option<HitboxId>>>,
    /// macOS: a hitbox drawn after all the pane's own content (its last child), and whether what was drawn after it covered all of
    /// the pane in the last frame (`covers_pane`: Settings or another full-window screen over the page)
    end_marker: Arc<Mutex<Option<HitboxId>>>, mac_covered: bool,
    /// macOS: when the last push of a burst is sent again (`mac_watch`): a show and the rect corrected a frame later reached KiCad as
    /// two pushes, and its frame kept the first one's rect (e2e-macos 44255662838: 24 pt too tall over the strip, 13.6 s and on)
    mac_settle: Option<std::time::Instant>,
    /// the window's host pushes, shared with the app (HostSends; Backplane::screens.host_sends)
    pub host_sends: std::rc::Rc<std::cell::RefCell<HostSends>>,
    /// macOS: what was last pushed may not have reached KiCad (the hub's connection made again): pushed again though unchanged
    host_unsent: bool,
    /// macOS: a show waiting for the pane's layout to settle: its rect and since when (show_settled).
    mac_show_held: Option<((i32, i32, i32, i32), std::time::Instant)>,
    last_root: (i32, i32), placed: Option<(Option<nc::R>, Option<u64>, Vec<nc::R>)>,
    focus: FocusHandle, keys: bool, pub light: bool, _subs: Vec<Subscription>,
    /// the fork's holder already in our save-set; reserved chords pressed in KiCad, waiting to be pressed in gpui
    kept: u64, pub press: Vec<String>,
    /// when a reserved key pressed in KiCad (Ctrl+K, Ctrl+J...) last went to Backplane's own actions (`lent`)
    lent_at: Option<std::time::Instant>,
    /// stock KiCad: the editor last asked for with its shortcut, and when; auxiliary frame (Symbol/Footprint/3D) opened from inside KiCad
    poked: Option<(String, std::time::Instant)>, unmapped_since: Option<(u64, std::time::Instant)>, unshown_since: Option<(String, std::time::Instant)>, override_kind: Option<String>, pending_show: Option<String>,
    /// the theme told to the hub at least once; the main window (its first home), where Pop back in returns it
    theme_sent: bool, home: u64,
    /// files changed on disk that KiCad holds back over the person's unsaved edits (info `kicad.reload.<digest>`)
    pub conflicts: Vec<String>,
    /// KiCad's long jobs now (bp.12 ET_PROGRESS via info `kicad.progress.<digest>`): id, title, detail, fraction (-1 indeterminate), cancellable
    pub jobs: Vec<Value>,
    /// the fork's frames KiCad has shown at least once (bp.9: the client may map and unmap them itself, so a tab switch is local)
    ever_shown: Vec<u64>,
    /// the other editors were asked pre-warmed (once per KiCad process: its pid)
    prewarmed: Option<u64>,
    /// when the first editor was placed over the pane (the pre-warm waits 2 s after it)
    first_placed: Option<std::time::Instant>,
    /// this pane has had a KiCad frame on show (any platform; until KiCad is another process): a later info without the frame it wants is
    /// that frame closed in KiCad. Sticky: on macOS the frame's hiding reached the pane before its close, and the placement read at the
    /// close already said nothing on show, so a Cmd+W left the pane editing with no frame and Edit did nothing (lane M's Mac table)
    shown_once: bool,
    /// where the pane was last drawn (window coordinates): the comment window opens over it (screens/kcomment.rs)
    pub at_px: Option<Bounds<Pixels>>,
    /// KiCad's dialogs on screen now (their titles), as the client's X connection sees them
    dialogs_open: Vec<String>,
    /// the frame KiCad gave the keyboard to on a click, and when (a click-to-focus window manager may take it back at once)
    took: Option<(u64, std::time::Instant)>,
    /// The popped-out window closed by its X (its handle) and when: the keys are handed in once it is gone and Backplane's
    /// window is the foreground (tick), with a 1 s bound as the fallback
    pub await_close: Option<(u64, std::time::Instant)>,
    /// fork: a press over the editor, its keyboard given on the release (a focus change mid-press made KiCad lose the release: it took the
    /// click for a long press and opened Clarify Selection, which then held the keyboard)
    press_focus: Option<u64>,
    /// Edit, a tab, Full size: the person chose the editor, so it gets the keyboard the moment its frame is on show (KiCad's hotkeys work with
    /// no canvas click first; lane A: Ctrl+Shift+R after Edit reached nothing)
    pub want_keys: bool,
    /// Windows: the editor given the keyboard as its frame came on show, and when. The focus request posted then can reach KiCad
    /// before the client's ShowWindowAsync of the frame and container, and KiCad rejects a request for a frame not yet visible
    /// (opening-focus run Bp-repost: ignored at show, accepted once the pane was shown), so it is posted once more on the first
    /// frame both are visible, while the pane still has the keyboard
    refocus: Option<(u64, std::time::Instant)>,
    /// F5, Windows: after a hand-in to this frame, for 1.5 s: an invisible KiCad window (a pre-warmed editor, wx-shown and
    /// parked off-screen) that takes the foreground gets the hand-in posted to the shown frame once more (run Fmain5: the
    /// first tab click after the pre-warm left the keys in a hidden "untitled [Unsaved] — Schematic Editor")
    verify_handin: Option<(u64, std::time::Instant)>,
    /// F5 while the pane pre-warms the other editors (Windows): from the pre-warm request until the hub lists every frame it
    /// builds, plus 2 s, an invisible KiCad window that takes the foreground while KiCad has the keys gets the hand-in posted
    /// to the shown editor once per stray (run IB1: the self-started pre-warm took it 1,244 ms after the opening hand-in,
    /// caught only by verify_handin's 1.5 s window; a slower build would land outside it). Started at, done at.
    pub(crate) prewarm_guard: Option<(std::time::Instant, Option<std::time::Instant>, u64)>,
    /// One re-post per F5 window for keys drifting to Backplane's own window with no press there.
    unexplained_reposted: bool,
    /// Debug: the last foreground/focus seen by the tick, to log only changes.
    focus_seen: (u64, u64),
    /// the keyboard was in a KiCad dialog last tick: when it closes, the keyboard goes back to the editor (a window manager hands it to
    /// Backplane's window)
    was_dialog: bool,
}

/// A chord KiCad handed back that the window acts on at once (the comment window: screens/kcomment.rs).
pub struct PaneKey(pub String);
impl EventEmitter<PaneKey> for KicadPane {}
/// KiCad closed the editor frame the pane shows (Ctrl+W / Cmd+W, File > Close, its own quit of that frame; KiCad asked about unsaved
/// changes itself): its kind. The page leaves Edit for View, or moves to the page's own editor when it was one of the extra tabs.
pub struct PaneFrameClosed(pub String);
impl EventEmitter<PaneFrameClosed> for KicadPane {}
/// The kind of the frame KiCad closed under the pane: the one it showed (`wanted`), in the old info and in the new one no longer, the
/// same KiCad (pid) still ready. None while KiCad starts, restarts or is gone (those say so themselves).
/// KiCad is up and has no frame of `kind` (closed in KiCad, or never opened).
pub fn missing_frame(info: &Value, kind: &str) -> bool {
    info["state"] == "ready" && !info["frames"].as_array().into_iter().flatten().any(|f| f["kind"] == kind)
}
pub fn closed_frame(old: &Value, new: &Value, wanted: &str) -> Option<String> {
    if old["pid"] != new["pid"] || new["state"] != "ready" || old["state"] != "ready" { return None }
    let has = |i: &Value| i["frames"].as_array().into_iter().flatten().any(|f| f["kind"] == wanted);
    (has(old) && !has(new)).then(|| wanted.to_string())
}
/// What the page does when the frame it showed closed: View (the board's or schematic's own page), or that page's editor (an extra tab
/// closed: the symbol or footprint editor, GerbView, the 3D viewer) when it is still open.
pub fn after_close(closed: &str, page_kind: &str, info: &Value) -> Option<String> {
    // (the Simulator is the rail's own entry, as the page's editor is: closed, the pane is back on View)
    if closed == page_kind || closed == SIM { return None }
    info["frames"].as_array().into_iter().flatten().any(|f| f["kind"] == page_kind).then(|| page_kind.to_string())
}
/// The pane's warm hand-over ended (`handoff`): the page draws it alone, without the viewer's picture under it.
pub struct PaneShown;
impl EventEmitter<PaneShown> for KicadPane {}
/// A runtime without `embed-frame-drawn` (bp.12.x): how long the viewer's picture stays under a warm KiCad's frame once it is mapped.
/// Measured on the GPU display (RTX 5080, Collar main, bp.12.4, Nov 14): from the frame's first pixels to the pane settled took 52.6, 54.4
/// and 58.7 ms (KiCad paints its toolbars, then the canvas; what it has not painted shows what is under it): 150 ms is about 2.5x the most.
/// With the capability the hold ends on KiCad's own word (ET_FRAME_DRAWN, the session's `drawn`)
const HANDOFF_PAINT: std::time::Duration = std::time::Duration::from_millis(150);
/// A warm hand-over that never sees its frame placed (the hub slow, the frame gone) ends after this: the pane says what it waits on
const HANDOFF_MAX: std::time::Duration = std::time::Duration::from_secs(3);

impl KicadPane {
    pub fn new(cx: &mut Context<Self>, project: String, root: String, project_id: String, open: String, hub_open: String, hub: tokio::sync::mpsc::Sender<Command>) -> Self {
        // the tick: adopt stock KiCad's new frames, read the container's frames, and give the editor the keyboard on a click over it
        // and at once when our X connection has something to read (a key KiCad hands back: the comment window opens on it with no 40 ms wait;
        // Linux: a thread waits on the connection's socket, native_child `conn_fd`)
        let (wake_tx, wake_rx) = async_channel::bounded::<()>(1);
        let (drained_tx, drained_rx) = std::sync::mpsc::channel::<()>();
        // the tick holds a sender: with no wake thread (Windows; Linux with no X socket or BACKPLANE_KICAD_NO_XWAKE) recv() then waits
        // instead of failing at once, which spun the tick on the UI thread and starved every timer (e2e drivers stalled on Windows)
        let hold_tx = wake_tx.clone();
        #[cfg(not(unix))]
        drop((wake_tx, drained_rx));
        #[cfg(unix)]
        if let Some(fd) = nc::conn_fd().filter(|_| std::env::var_os("BACKPLANE_KICAD_NO_XWAKE").is_none()) {
            std::thread::spawn(move || loop {
                let mut p = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
                let r = unsafe { libc::poll(&mut p, 1, 1000) };
                if wake_tx.is_closed() { break }
                // the socket is read here (nc::pump), so it is empty again after: only an event for the pane wakes the tick, and other X noise
                // never spins it (a wake per readable socket the tick did not read kept the main thread busy for seconds: every lane's stall)
                if r > 0 && nc::pump() { while drained_rx.try_recv().is_ok() {} let _ = wake_tx.try_send(()); let _ = drained_rx.recv_timeout(std::time::Duration::from_millis(40)); }
                else if r < 0 { std::thread::sleep(std::time::Duration::from_millis(40)); }
            });
        }
        // one 40 ms timer at a time, kept across wakes (a new timer per pass was a queued entry each in gpui's timer thread: a loop that never
        // waited grew it without bound), and a wake that comes again within 2 ms of the last pass waits for the timer: the tick can never spin
        cx.spawn(async move |weak, cx| { let _hold = hold_tx; let mut timer = Box::pin(cx.background_executor().timer(std::time::Duration::from_millis(40))); let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1); loop {
            let woke = if last.elapsed() < std::time::Duration::from_millis(2) { (&mut timer).await; false } else {
                match futures_util::future::select(&mut timer, Box::pin(wake_rx.recv())).await { futures_util::future::Either::Left(_) => false, futures_util::future::Either::Right(_) => true }
            };
            if !woke { timer = Box::pin(cx.background_executor().timer(std::time::Duration::from_millis(40))); }
            if weak.update(cx, |this, cx| this.tick(cx)).is_err() { break }
            last = std::time::Instant::now();
            let _ = drained_tx.send(());
        } }).detach();
        KicadPane { born: std::time::Instant::now(), handoff_placed: None, handoff_over: false, host_sent: None, mac_show_held: None, mac_full: false, in_page: true, mac_raise: RaiseState::default(), mac_buried: false, mac_watching: false, mac_hot: None, mac_seen: (false, None), mac_pressed: false, mac_looked: None, busy_sent: None, press_in: false, press_gave_keys: false, want_gerbers: Vec::new(), bound: String::new(), project, root, project_id, open, last_hub_open: hub_open, hub, container: None, toplevel: 0, attached: false, info: Value::Null, frames: Vec::new(), marker: Arc::default(), end_marker: Arc::default(), mac_covered: false, mac_settle: None, host_sends: Default::default(), host_unsent: false, last_root: (i32::MIN, 0), placed: None,
            focus: cx.focus_handle(), keys: false, light: false, _subs: Vec::new(), kept: 0, press: Vec::new(), lent_at: None, poked: None, unmapped_since: None, unshown_since: None, override_kind: None, pending_show: None, theme_sent: false, home: 0, conflicts: Vec::new(), jobs: Vec::new(), ever_shown: Vec::new(), prewarmed: None, first_placed: None, shown_once: false, at_px: None, dialogs_open: Vec::new(), took: None, await_close: None, press_focus: None, want_keys: true, refocus: None, verify_handin: None, prewarm_guard: None, unexplained_reposted: false, focus_seen: (0, 0), was_dialog: false, starting_since: None, starting_said: 0 }
    }
    /// Edit pressed on this pane: KiCad ready but without a frame of the kind the pane wants: it is asked for one (a fresh frame).
    pub fn edit_again(&mut self, cx: &mut Context<Self>) {
        let want = self.wanted_kind().to_string();
        if !missing_frame(&self.info, &want) { return }
        eprintln!("GPUI_LOG {}", json!({"kicad_edit_again": want}));
        self.placed = None; self.pending_show = Some(want.clone());
        self.rpc("editor.kicad.show", json!({"project": self.project, "frame": want, "focus": true}));
        cx.notify();
    }
    fn rpc(&self, name: &str, params: Value) { let _ = self.hub.try_send(Command::Rpc(name.to_string(), Secret(params), "kicad")); }
    /// The hub's `editor.kicad.<digest>` for this project changed.
    pub fn set_info(&mut self, info: Value, cx: &mut Context<Self>) {
        if info == self.info { return }
        // (only a frame this pane has shown: a new pane, Edit pressed again, starts from the hub's last word, which may still list the
        // frame closed before it and not yet the one KiCad opens anew)
        // (macOS: KiCad places its own frames, nothing is mapped locally and first_placed stays unset; a frame on show is the mark there)
        let closed = closed_frame(&self.info, &info, self.wanted_kind()).filter(|_| self.first_placed.is_some() || self.shown_once || self.placed.as_ref().is_some_and(|p| p.1.is_some())).map(|k| { let next = after_close(&k, frame_for(&self.open), &info); (k, next) });
        let was_tab = self.tab(); let mut cause = String::from("hub info"); // i18n-ok: a debug log cause (BACKPLANE_KICAD_DEBUG)
        if info["pid"] != self.info["pid"] { self.ever_shown.clear(); self.shown_once = false; }
        // shown once, or realized (bp.9: KiCad says the client may map it itself, e.g. a pre-warmed editor): the client's to map
        for f in info["frames"].as_array().into_iter().flatten().filter(|f| f["shown"] == true || f["realized"] == true) { if let Some(x) = f["xid"].as_u64() { if !self.ever_shown.contains(&x) { self.ever_shown.push(x); } } }
        // KiCad became ready (a launch, a restart): it is told Backplane's theme again
        if info["state"] == "ready" && (self.info["state"] != "ready" || info["pid"] != self.info["pid"]) && self.theme_sent {
            self.rpc("editor.kicad.theme", json!({"project": self.project, "mode": if self.light { "light" } else { "dark" }}));
        }
        if info["pid"] != self.info["pid"] {
            let had_pid = !self.info["pid"].is_null();
            self.poked = None; self.unmapped_since = None; self.kept = 0; self.placed = None; if had_pid || self.override_kind.as_deref() != Some(SIM) { self.override_kind = None; } cause = format!("hub info: KiCad pid {} -> {}", self.info["pid"], info["pid"]); // i18n-ok: a debug log cause (BACKPLANE_KICAD_DEBUG)
            if had_pid { self.pending_show = None; }
        }
        if matches!(info["state"].as_str(), Some("failed" | "stopped")) { self.pending_show = None; }
        // macOS: a KiCad that became ready, or another one (a restart), has not had this pane's rect: pushed again on the next frame
        if info["state"] != self.info["state"] || info["pid"] != self.info["pid"] { self.host_sent = None; }
        #[cfg(target_os = "macos")]
        nc::note_kicad_pid(info["pid"].as_u64().unwrap_or(0) as u32);
        if let Some(act) = info["active"].as_str().filter(|a| !a.is_empty()) {
            if self.pending_show.as_deref() == Some(act) {
                self.pending_show = None;
            } else if self.pending_show.is_none() && Some(act) != self.info["active"].as_str() {
                self.override_kind = (act != frame_for(&self.open)).then(|| act.to_string());
                cause = format!("hub info active {} -> {act} (KiCad's active frame)", self.info["active"]); // i18n-ok: a debug log cause (BACKPLANE_KICAD_DEBUG)
            }
        }
        if info["mode"] == "fork" && self.info["mode"] != "fork" { if let Some(c) = self.container { nc::grab_click(c, false); } }
        if info["frames"] != self.info["frames"] {
            if let Some(c) = self.container { self.frames = nc::frames(c); }
            self.placed = None; self.unmapped_since = None;
        }
        // macOS: the reserved chords pressed in KiCad come through the hub (its window cannot post to ours): queued as the events the
        // other platforms read from the window system; a session's earlier chords are never replayed
        #[cfg(target_os = "macos")]
        for k in info["keys"].as_array().into_iter().flatten().filter_map(|k| Some((k[0].as_u64()?, k[1].as_u64()?))) {
            if info["pid"] != self.info["pid"] || self.info["keys"].as_array().is_some_and(|old| old.iter().any(|o| o[0].as_u64() == Some(k.0))) { continue }
            if let Some(chord) = nc::reserved_chord_index(k.1 as usize) { nc::push_key(&nc::mac_chord(chord)); }
        }
        match (info["state"] == "starting", self.starting_since) {
            (true, None) => self.starting_since = Some(std::time::Instant::now()),
            (false, Some(_)) => { self.starting_since = None; self.starting_said = 0; }
            _ => {}
        }
        self.info = info;
        self.note_tab(&was_tab, &cause);
        // the frame on show closed in KiCad: never a wait for it ("Starting KiCad…" for ever, lane M's Cmd+W)
        if let Some((k, next)) = closed {
            eprintln!("GPUI_LOG {}", json!({"kicad_frame_closed": k, "to": next}));
            match next { Some(to) => { self.override_kind = None; self.show_kind_by(&to, "the frame shown closed in KiCad: the page's own editor", cx) } None => cx.emit(PaneFrameClosed(k)) } // i18n-ok: a debug log cause
        }
        cx.notify();
    }
    pub fn matches(&self, info: &Value) -> bool {
        let p = info["project"].as_str().unwrap_or("");
        // the hub's own name for this pane's project, from its attach reply: the .kicad_pro may sit below the folder the viewer named
        // (a repository whose board is in boards/<name>/), so the viewer's path alone did not match (Nov 11, SOT23_Card)
        if !self.bound.is_empty() && norm_path(p) == norm_path(&self.bound) { return true }
        let root = info["root"].as_str().unwrap_or("");
        (!p.is_empty() && norm_path(path_stem(p)) == norm_path(path_stem(&self.project)))
            || (!root.is_empty() && (norm_path(root) == norm_path(&self.project) || (!self.root.is_empty() && norm_path(root) == norm_path(&self.root))))
    }
    pub fn needs_sync_open(&self, hub_open: &str, pressed_open: Option<&str>) -> bool {
        pressed_open.map_or(hub_open != self.last_hub_open, |p| p != self.open)
    }
    pub fn sync_hub_open(&mut self, hub_open: &str, pressed_open: Option<&str>, cx: &mut Context<Self>) {
        if let Some(next) = hub_open_step(&self.open, &mut self.last_hub_open, hub_open, pressed_open).map(str::to_string) {
            self.set_open_by(&next, "hub open (sync_hub_open)", cx); // i18n-ok: a debug log cause (BACKPLANE_KICAD_DEBUG)
        }
    }
    /// The rail moved to another page: show its editor (the hub opens it when KiCad has not yet).
    pub fn set_open(&mut self, open: &str, cx: &mut Context<Self>) { self.set_open_by(open, "rail page (set_open)", cx) } // i18n-ok: a debug log cause (BACKPLANE_KICAD_DEBUG)
    pub fn set_open_by(&mut self, open: &str, cause: &str, cx: &mut Context<Self>) {
        let was_tab = self.tab(); self.set_open_inner(open, cx); self.note_tab(&was_tab, cause);
    }
    fn set_open_inner(&mut self, open: &str, cx: &mut Context<Self>) {
        if self.open == open && self.override_kind.is_none() { return }
        self.open = open.to_string();
        self.override_kind = None;
        self.unmapped_since = None;
        let kind = frame_for(open);
        // KiCad was closed (or failed): the rail's page starts it again, instead of a "stopped" refusal from show
        if self.attached && matches!(self.info["state"].as_str(), Some("stopped" | "failed")) { self.reattach(); }
        else if self.attached && self.locally_shown(kind) { self.placed = None; }
        else if self.attached {
            self.pending_show = (self.info["active"].as_str() != Some(kind)).then(|| kind.to_string());
            self.rpc("editor.kicad.show", json!({"project": self.project, "frame": kind, "focus": self.keys}));
        }
        cx.notify();
    }
    pub fn has_override(&self) -> bool { self.override_kind.is_some() }
    /// The editor an attach asks KiCad for: the rail page's, or the Simulator when the pane was opened on it (the rail's Simulation):
    /// KiCad then opens only that (and the schematic it needs, hidden), not the page's board first.
    fn attach_kind(&self) -> Option<&'static str> { if self.override_kind.as_deref() == Some(SIM) { Some(SIM) } else { page_frame(&self.open) } }
    /// The rail's Simulation: the pane on KiCad's Simulator (before the pane is attached: its first editor).
    pub fn open_simulation(&mut self, cx: &mut Context<Self>) {
        if self.container.is_none() { self.override_kind = Some(SIM.into()); cx.notify(); return }
        // (its attach is on its way with the page's editor: asked again, for the Simulator)
        if !self.attached { self.override_kind = Some(SIM.into()); self.placed = None; self.reattach(); cx.notify(); return }
        if self.tab() == SIM && !missing_frame(&self.info, SIM) { return }
        if self.tab() == SIM { self.edit_again(cx); return }
        self.show_kind_by(SIM, "rail Simulation", cx); // i18n-ok: a debug log cause (BACKPLANE_KICAD_DEBUG)
    }
    /// The passive grab that turns a click on the unfocused editor into the keyboard for it: stock KiCad only. The fork takes the keyboard on
    /// its own click (and says so with `_BACKPLANE_FOCUS`); a grab there froze the click until the pane's next tick replayed it, after the
    /// button was up, and KiCad dropped the keys until a second click (one click must do).
    fn grab_click(&self, c: u64, on: bool) {
        nc::grab_click(c, on && self.info["mode"] != "fork");
        // Windows, fork: the input shield over the pane while Backplane has the keys (F3)
        nc::shield(c, on && cfg!(windows) && self.info["mode"] == "fork");
    }
    /// KiCad's frames moved into a new container in the window `t` (and the hub told), before the window they were in goes away: a window's
    /// destruction takes its children with it, and KiCad's would be among them. Also how a pane goes back in from its own window.
    pub fn rehome(&mut self, t: u64) {
        let Some(old) = self.container else { return };
        if t == self.toplevel { return }
        let Some(new) = nc::container_for(t, &self.project) else { return };
        if new != old { nc::move_frames(old, new); nc::destroy(old); }
        self.toplevel = t; self.container = Some(new); self.placed = None; self.unmapped_since = None; self.last_root = (i32::MIN, 0);
        // the editor in another window now (popped out, or back in): its keys are handed in when it is placed there
        // (bp9 Nov 15: set at open_window, the old window's next frame used the request before the move, and the editor
        // in the new window never got the keys)
        self.want_keys = true;
        self.grab_click(new, !self.keys);
        self.reattach();
    }
    /// The hub's `kicad.reload.<digest>`: this project's waiting files, if it is this pane's.
    pub fn set_reload(&mut self, v: Value, cx: &mut Context<Self>) {
        if !self.matches(&v) { return }
        let c: Vec<String> = v["conflicts"].as_array().into_iter().flatten().filter_map(|f| f.as_str().map(String::from)).collect();
        if c != self.conflicts { self.conflicts = c; cx.notify(); }
    }
    /// The hub's `kicad.progress.<digest>`: this project's running jobs, if it is this pane's.
    pub fn set_progress(&mut self, v: Value, cx: &mut Context<Self>) {
        if !self.matches(&v) { return }
        let j = v["jobs"].as_array().cloned().unwrap_or_default();
        if j != self.jobs { self.jobs = j; cx.notify(); }
    }
    /// The × on a job's bar.
    /// The person dismissed KiCad's notice in the pane: hidden here (the hub keeps it for an agent that asks).
    pub fn dismiss_notice(&mut self, id: u64) { self.rpc("editor.kicad.notice.dismiss", json!({"project": self.project, "id": id})); }
    pub fn cancel_job(&mut self, id: u64) { self.rpc("editor.kicad.cancel", json!({"project": self.project, "id": id})); }
    /// The person's answer to a file changed on disk (Keep yours / Reload), for every waiting file.
    pub fn resolve(&mut self, choice: &str, cx: &mut Context<Self>) {
        for f in std::mem::take(&mut self.conflicts) { self.rpc("editor.kicad.resolve", json!({"project": self.project, "file": f, "choice": choice})); }
        cx.notify();
    }
    /// The main window's native handle (the pane's first home).
    pub fn home(&self) -> u64 { self.home }
    /// A tab pressed: that editor in the pane (KiCad opens it when it has not yet); the rail's page stays as it is.
    /// The embedded KiCad can show Gerbers in its GerbView (bp.13: `embed-gerbview`, from its GetCapabilities).
    pub fn can_gerbview(&self) -> bool { self.info["embed_caps"].as_array().is_some_and(|a| a.iter().any(|c| c == "embed-gerbview")) }
    /// Gerbers (a `.gbrjob` for all its layers, or Gerber and drill files) opened in the embedded GerbView and shown in the pane: the
    /// "Open in GerbView" entry point. Its tab then switches back to it like the editors'.
    pub fn open_gerbers(&mut self, files: &[String], cx: &mut Context<Self>) -> bool {
        if !self.attached || !self.can_gerbview() || files.is_empty() { return false }
        let was_tab = self.tab(); self.override_kind = Some("GERBVIEW".into()); self.note_tab(&was_tab, "open_gerbers"); self.placed = None; self.pending_show = Some("GERBVIEW".into()); // i18n-ok: wire frame kind
        self.rpc("editor.kicad.show", json!({"project": self.project, "frame": "GERBVIEW", "document": files.join("\n")})); // i18n-ok: wire frame kind
        cx.notify(); true
    }
    /// The person is working in KiCad's canvas (a button down over it: a pan, a drag, a click): the hub hears it at most every 500 ms,
    /// so its automatic checks wait for the gesture to end rather than take KiCad's UI thread mid-way.
    fn busy_in_kicad(&mut self) {
        if self.busy_sent.is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(500)) { return }
        self.busy_sent = Some(std::time::Instant::now());
        self.rpc("editor.kicad.touched", json!({"project": self.project}));
    }
    /// open_gerbers now if it can, else as soon as the pane is attached and KiCad ready (the tick); dropped if KiCad then has no GerbView.
    pub fn open_gerbers_soon(&mut self, files: &[String], cx: &mut Context<Self>) {
        if !self.open_gerbers(files, cx) { self.want_gerbers = files.to_vec(); cx.notify(); }
    }
    pub fn show_kind(&mut self, kind: &str, cx: &mut Context<Self>) { self.show_kind_by(kind, "show_kind", cx) }
    pub fn show_kind_by(&mut self, kind: &str, cause: &str, cx: &mut Context<Self>) {
        let was_tab = self.tab(); self.show_kind_inner(kind, cx); self.note_tab(&was_tab, cause);
    }
    fn show_kind_inner(&mut self, kind: &str, cx: &mut Context<Self>) {
        self.want_keys = true;
        if kind == frame_for(&self.open) { self.override_kind = None; } else { self.override_kind = Some(kind.to_string()); }
        self.placed = None;
        if self.attached && matches!(self.info["state"].as_str(), Some("stopped" | "failed")) { self.reattach(); }
        else if self.attached && self.locally_shown(kind) {}
        else if self.attached { self.pending_show = Some(kind.to_string()); self.rpc("editor.kicad.show", json!({"project": self.project, "frame": kind})); }
        cx.notify();
    }
    /// bp.10+ local map: `kind`'s frame is one KiCad has shown or pre-warmed, so the client maps it itself and asks KiCad nothing. A hub
    /// ShowFrame here hid the other frames in KiCad while the client was mapping them (a quick Board, 3D, Board left the board's Show a
    /// no-op until the hub's re-show ~0.9 s later).
    /// Not on Windows: there a frame shown only by the client (`ShowWindowAsync`) kept its canvas unpainted, showing the board's pixels
    /// under the Schematic Editor's toolbars (bp.10 and bp.12 alike), so the hub's ShowFrame still switches it.
    fn locally_shown(&self, kind: &str) -> bool {
        !cfg!(windows) && self.info["mode"] == "fork" && self.info["local_map"] == true
            && pick_local(kind, &self.info, &self.frames, &self.ever_shown).is_some_and(|x| self.ever_shown.contains(&x))
    }
    pub fn tab(&self) -> String { self.wanted_kind().to_string() }
    /// BACKPLANE_KICAD_DEBUG: one line the moment the pane's tab changes, with its cause (a click, a shortcut, the hub's active frame
    /// that KiCad asked for, the rail, a script). A periodic state log cannot place a switch within the ~40 ms of a race: on Nov 13 it
    /// took this line to see the hub following KiCad's show request 102 ms before a Ctrl+Y (fixed in 0decc2de6).
    fn note_tab(&self, was: &str, cause: &str) {
        let now = self.wanted_kind();
        if now == was || std::env::var_os("BACKPLANE_KICAD_DEBUG").is_none() { return }
        let epoch = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0);
        eprintln!("GPUI_LOG {}", json!({"kicad_tab": {"epoch_ms": epoch, "from": was, "to": now, "cause": cause, "open": self.open, "override": self.override_kind, "hub_active": self.info["active"], "pending_show": self.pending_show}}));
    }
    /// Backplane's theme: KiCad follows it (the fork switches live; stock KiCad keeps its own).
    pub fn set_light(&mut self, light: bool, cx: &mut Context<Self>) {
        if self.light == light && self.theme_sent { return }
        self.light = light; self.theme_sent = true;
        if self.attached { self.rpc("editor.kicad.theme", json!({"project": self.project, "mode": if light { "light" } else { "dark" }})); }
        cx.notify();
    }
    /// The hub's state with KiCad's dialogs as this window sees them on screen (a modal one stops KiCad's events, so the hub may not know yet).
    /// The first Edit on a KiCad already running (pre-warmed): until its frame is mapped here and has painted, the pane draws nothing of
    /// its own and the page keeps the viewer's picture under it (lane A, batch 8 undo runs; bp.12.4, batch 7 and 8 alike: the attach's round
    /// trip and KiCad's first paint showed the pane's navy for ~50 ms, one blank frame on every first Edit). A cold start says Starting KiCad…
    pub fn handoff(&self) -> bool {
        if self.handoff_over || self.info["state"] != "ready" { return false }
        // the hand-over e2e: BACKPLANE_GPUI_TEST_HANDOFF_MIN_MS holds it at least that long (a click and keys inside it, with KiCad mapped)
        if std::env::var("BACKPLANE_GPUI_TEST_HANDOFF_MIN_MS").ok().and_then(|v| v.parse::<u64>().ok()).is_some_and(|ms| self.born.elapsed() < std::time::Duration::from_millis(ms)) { return true }
        if self.born.elapsed() >= HANDOFF_MAX { return false }
        let Some((at, w)) = self.handoff_placed else { return true };
        let drawn_event = self.info["embed_caps"].as_array().is_some_and(|a| a.iter().any(|c| c == "embed-frame-drawn"));
        handoff_holds(drawn_event, self.info["drawn"].as_array().is_some_and(|d| d.iter().any(|x| x.as_u64() == Some(w))), at.elapsed())
    }
    fn end_handoff(&mut self, cx: &mut Context<Self>) {
        if self.handoff_over { return }
        self.handoff_over = true;
        if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() {
            let drawn = self.handoff_placed.is_some_and(|(_, w)| self.info["drawn"].as_array().is_some_and(|d| d.iter().any(|x| x.as_u64() == Some(w))));
            eprintln!("GPUI_LOG {}", json!({"kicad_handoff_end_ms": self.born.elapsed().as_millis() as u64, "placed_ms": self.handoff_placed.map(|(t, _)| t.duration_since(self.born).as_millis() as u64), "drawn_event": drawn}));
        }
        cx.emit(PaneShown); cx.notify();
    }
    pub fn seen_info(&self) -> Value {
        let mut i = self.info.clone();
        if let Some(t) = self.starting_since { i["starting_for_s"] = json!(t.elapsed().as_secs()); }
        if !self.dialogs_open.is_empty() && i["dialogs"].as_array().map_or(true, |d| d.is_empty()) { i["dialogs"] = json!(self.dialogs_open); }
        i
    }
    /// The editor the pane wants on show: the one it switched to (override), else its page's own; a page with no editor of its own
    /// (#1347: the attach named none) the one KiCad shows (the hub's `active`), the board only while that is not known
    fn wanted_kind(&self) -> &str {
        if let Some(k) = self.override_kind.as_deref() { return k }
        match page_frame(&self.open) { Some(k) => k, None => self.info["active"].as_str().filter(|a| !a.is_empty()).unwrap_or("PCB") }
    }
    /// The person took the editor: the hub then refuses an agent's save while anything is unsaved (kicad/embed.rs `Unsaved`).
    fn touched(&self) { self.rpc("editor.kicad.touched", json!({"project": self.project})); }
    /// What a comment is made on: the project and the editor on show (PCB or SCH; the 3D view comments on its board), when one is.
    pub fn comment_target(&self) -> Option<(String, &'static str)> {
        self.want()?; let p = if self.bound.is_empty() { self.project.clone() } else { self.bound.clone() };
        Some((p, if self.wanted_kind() == "SCH" { "SCH" } else { "PCB" })) // i18n-ok: wire frame kinds
    }
    /// Backplane's window over the editor has the keyboard (`on`): the editor takes no pointer input meanwhile, so the keys typed with the pointer
    /// resting over it come to Backplane (native_child `input_off`).
    pub fn hold_keys(&mut self, on: bool) {
        let Some(c) = self.container else { return };
        nc::input_off(c, on);
        if on && self.keys { nc::focus(self.toplevel); self.keys = false; nc::grab_escape(self.toplevel, false); self.grab_click(c, true); }
    }
    /// Whether the keyboard came from KiCad just now: a reserved key pressed there went to Backplane's own action (an overlay it opens
    /// gives the keyboard back to KiCad when it is dismissed: `Backplane::kicad_lend`, `kicad_return`).
    pub fn lent_recently(&self) -> bool { self.lent_at.is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(1500)) }
    /// The pointer's input given back to the editor (`hold_keys` took it), the keyboard left where it is.
    pub fn release_hold(&mut self) { if let Some(c) = self.container { nc::input_off(c, false); } }
    /// The keyboard back to the editor on show (a comment posted or cancelled), as a click on it gives it.
    pub fn give_keys(&mut self, cx: &mut Context<Self>) {
        let Some(c) = self.container else { return };
        nc::input_off(c, false);
        let Some(w) = self.want() else { return };
        self.grab_click(c, false);
        #[cfg(not(target_os = "macos"))]
        nc::focus(w);
        // macOS: consent, then ShowFrame{focus}, so KiCad's canvas has the keys (activating KiCad from here focused a toolbar field)
        #[cfg(target_os = "macos")]
        { nc::yield_to(w); self.rpc("editor.kicad.show", json!({"project": self.project, "frame": self.wanted_kind(), "focus": true})); }
        self.keys = true; nc::grab_escape(self.toplevel, true); cx.notify();
    }
    fn want(&self) -> Option<u64> { if self.info.is_null() || (cfg!(windows) && self.info["mode"] == "stock") { return None } pick_local(self.wanted_kind(), &self.info, &self.frames, &self.ever_shown) }
    fn tick(&mut self, cx: &mut Context<Self>) {
        // Windows: Backplane in front with nothing holding the keyboard (a jump away from the editor it was lent from) gets it
        if cfg!(windows) && !self.keys && self.toplevel != 0 { nc::keep_focus(self.toplevel); }
        if !self.handoff_over && !self.handoff() { self.end_handoff(cx); }
        // past STARTING_SAY the pane's words change each second (what KiCad is doing, for how long)
        if let Some(t) = self.starting_since {
            let s = t.elapsed().as_secs();
            if s >= STARTING_SAY && s != self.starting_said { self.starting_said = s; cx.notify(); }
        }
        if let Some((popped, at)) = self.await_close {
            let activated = nc::activated_after_close(self.toplevel, popped);
            if activated || at.elapsed() >= std::time::Duration::from_millis(1000) {
                self.await_close = None;
                self.want_keys = true; self.placed = None;
                if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() {
                    eprintln!("GPUI_LOG {}", json!({"kicad_close_handin": {"path": if activated { "activated" } else { "timeout" }, "after_ms": at.elapsed().as_millis() as u64}}));
                }
                cx.notify();
            }
        }
        nc::note_pane_keys(self.keys);
        // the shield never re-raises while the F5 guard is armed (pre-warm or a hand-in being verified)
        let f5_armed = self.prewarm_guard.is_some() || self.verify_handin.is_some();
        if let Some(c) = self.container { nc::shield_tick(c, if cfg!(windows) && self.info["mode"] == "fork" && !(f5_armed && self.keys) { self.want() } else { None }); }
        if let (true, Some(c)) = (std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() && cfg!(windows), self.container) {
            let d = nc::focus_debug(c);
            let now = (d["foreground"].as_u64().unwrap_or(0), d["focus"].as_u64().unwrap_or(0));
            if now != self.focus_seen { self.focus_seen = now; eprintln!("GPUI_LOG {}", json!({"kicad_focus_timeline": d})); }
        }
        if let Some((started, done, last_stray)) = self.prewarm_guard {
            let shown = self.wanted_kind();
            let built = ["SCH", "PCB"].iter().filter(|k| **k != shown).all(|k| self.info["frames"].as_array().is_some_and(|a| a.iter().any(|f| f["kind"].as_str() == Some(*k)))); // i18n-ok: wire frame kind enum names
            let done = done.or_else(|| built.then(std::time::Instant::now));
            // the cap does not run out while the pre-warm waits on KiCad's check (task_wait): the guard follows the pre-warm itself
            let held = self.info["task_wait"].as_str().is_some_and(|w| !w.is_empty());
            if done.is_some_and(|d| d.elapsed() > std::time::Duration::from_secs(2)) || (started.elapsed() > std::time::Duration::from_secs(60) && !held) {
                self.prewarm_guard = None;
            } else {
                let mut last = last_stray;
                if let (true, Some(w)) = (self.keys, self.want()) {
                    if let Some((stray, title)) = nc::invisible_kicad_foreground(w) {
                        if stray != last_stray {
                            last = stray;
                            eprintln!("GPUI_LOG {}", json!({"kicad_stray_foreground": {"frame": w, "stray": stray, "title": title, "during": "prewarm", "after_prewarm_request_ms": started.elapsed().as_millis() as u64}}));
                            nc::focus(w);
                        }
                    }
                }
                self.prewarm_guard = Some((started, done, last));
            }
        }
        if !self.want_gerbers.is_empty() && self.attached && self.info["state"] == "ready" {
            let f = std::mem::take(&mut self.want_gerbers);
            if !self.open_gerbers(&f, cx) { eprintln!("GPUI_LOG {}", json!({"gerbview": "not opened", "why": if self.can_gerbview() { "refused" } else { "no embed-gerbview" }})); }
        }
        // the pre-warm's 2 s are up: a frame drawn now sends it (after_frame), even when nothing else redraws the window
        // (not where the pre-warm waits for a placement: a redraw every tick would ask nothing and only cost frames)
        let idle_ok = idle_prewarm(cfg!(windows), self.info["embed_caps"].as_array().map(|a| a.iter().any(|c| c == "prewarm-keeps-focus")).unwrap_or(false));
        if let (Some(t), Some(pid), true) = (self.first_placed, self.info["pid"].as_u64(), idle_ok) { if self.prewarmed != Some(pid) && t.elapsed() >= std::time::Duration::from_secs(2) { cx.notify(); } }
        let Some(c) = self.container else { return };
        if let Some(pid) = self.info["pid"].as_u64().filter(|p| *p > 0) {
            if self.info["mode"] == "fork" {
                nc::dialogs(pid as u32, c, self.toplevel);
                // what KiCad asks (a load-time notice, a question) comes up over its pane, not in the middle of the screen; and the hub is told
                // which are open (a modal one stops KiCad's API: no events, no pings), so the pane names it and KiCad is not called hung
                let over = match (&self.placed, self.last_root.0 != i32::MIN) { (Some((Some(at), _, _)), true) => nc::R { x: self.last_root.0, y: self.last_root.1, w: at.w, h: at.h }, _ => nc::R { x: 0, y: 0, w: 0, h: 0 } };
                let open = nc::centre_dialogs(pid as u32, over);
                // a dialog KiCad brought up while the keyboard was KiCad's (its frame, or another of its dialogs: a dialog of a dialog) is given
                // the keyboard; one that comes up while the person types in Backplane is left as the window manager places it (X11 only)
                if let Some(&w) = nc::take_new_dialogs().last() {
                    if nc::has_focus(c) || nc::focus_in_pid(pid as u32) { nc::activate(w); }
                    else if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() { eprintln!("GPUI_LOG {}", json!({"kicad_dialog_not_activated": w, "why": "the keyboard is not KiCad's"})); }
                }
                if open != self.dialogs_open { self.dialogs_open = open; self.rpc("editor.kicad.dialogs", json!({"project": self.project, "open": self.dialogs_open})); cx.notify(); }
            } else if !cfg!(windows) && !nc::adopt(pid as u32, c, self.toplevel).is_empty() {
                self.placed = None; cx.notify();
            }
        }
        let f = nc::frames(c);
        if f != self.frames {
            let was_tab = self.tab();
            // If stock KiCad on Linux just opened Symbol Editor, Footprint Editor, or 3D Viewer from inside an editor, show it in the pane.
            if !cfg!(windows) && self.info["mode"] == "stock" {
                for nf in &f {
                    if !self.frames.iter().any(|of| of.window == nf.window)
                        && (matches!(nf.kind, "SYMBOL_EDITOR" | "FOOTPRINT_EDITOR") || (nf.kind == "PCB_3D" && self.open != "3d")) // i18n-ok: wire frame kind enum names
                    {
                        self.override_kind = Some(nf.kind.into());
                    }
                }
                let sync: Vec<Value> = f.iter().map(|nf| json!({
                    "kind": nf.kind,
                    "title": nf.title,
                    "modified": nf.title.trim_start().starts_with('*'),
                    "xid": nf.window,
                })).collect();
                self.rpc("editor.kicad.frames", json!({"project": self.project, "frames": sync}));
            }
            if let Some(ov) = &self.override_kind {
                let was_open = self.frames.iter().any(|x| x.kind == ov.as_str());
                if was_open && !f.iter().any(|x| x.kind == ov.as_str()) && !self.info["frames"].as_array().is_some_and(|a| a.iter().any(|x| x["kind"].as_str() == Some(ov.as_str()))) {
                    self.override_kind = None;
                }
            }
            self.frames = f; self.placed = None;
            self.note_tab(&was_tab, "X frames changed (stock new editor / overridden editor closed)"); // i18n-ok: a debug log cause (BACKPLANE_KICAD_DEBUG)
            cx.notify();
        }
        if let Some((Some(at), Some(show), _)) = &self.placed {
            nc::sync_root(c, *at, *show, &mut self.last_root);
        }
        let want = self.wanted_kind().to_string();
        // fork KiCad: if the placed frame stayed unmapped (e.g. cold-start wxGTK decor-extents deferral), ask the hub to remount it once cooldown passes.
        if self.info["mode"] == "fork" && self.info["state"] == "ready" && self.pending_show.is_none() {
            if let Some((Some(_), Some(w), _)) = &self.placed {
                if nc::report(*w).is_some_and(|(_, on, _)| !on) {
                    let since = match self.unmapped_since {
                        Some((uw, t)) if uw == *w => t,
                        _ => { let now = std::time::Instant::now(); self.unmapped_since = Some((*w, now)); now }
                    };
                    if since.elapsed() >= std::time::Duration::from_millis(800)
                        && self.poked.as_ref().map_or(true, |(k, t)| k != &want || t.elapsed() >= std::time::Duration::from_millis(1500))
                    {
                        self.poked = Some((want.clone(), std::time::Instant::now()));
                        self.unmapped_since = None;
                        self.placed = None;
                        self.rpc("editor.kicad.show", json!({"project": self.project, "frame": &want, "remount": true, "focus": self.keys}));
                    }
                } else {
                    self.unmapped_since = None;
                }
            } else {
                self.unmapped_since = None;
            }
        } else {
            self.unmapped_since = None;
        }
        // fork KiCad: the wanted editor exists but KiCad never said it was shown (bp.10 deferred a frame's first show; the hub's frames were read
        // before it mapped), so there was nothing to place and nothing to remount: the pane stayed on the last editor (lane K: Schematic made
        // and never shown, the board left in the pane). Asked again after 1.2 s, then every 2 s while it lasts.
        let exists = self.info["frames"].as_array().is_some_and(|a| a.iter().any(|f| f["kind"].as_str() == Some(want.as_str())));
        if self.info["mode"] == "fork" && self.info["state"] == "ready" && self.pending_show.is_none() && exists && self.want().is_none() {
            let since = match &self.unshown_since { Some((k, t)) if *k == want => *t, _ => { let now = std::time::Instant::now(); self.unshown_since = Some((want.clone(), now)); now } };
            if since.elapsed() >= std::time::Duration::from_millis(1200)
                && self.poked.as_ref().map_or(true, |(k, t)| k != &want || t.elapsed() >= std::time::Duration::from_millis(2000))
            {
                if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() { eprintln!("GPUI_LOG {}", json!({"kicad_unshown": want, "ms": since.elapsed().as_millis() as u64})); }
                self.poked = Some((want.clone(), std::time::Instant::now()));
                self.rpc("editor.kicad.show", json!({"project": self.project, "frame": &want, "remount": true, "focus": self.keys}));
            }
        } else {
            self.unshown_since = None;
        }
        // stock KiCad (Linux only): the wanted editor not open yet, asked for with its own shortcut (at most every 4 s; the fork opens it through ShowFrame)
        if !cfg!(windows) && self.info["mode"] == "stock" && pick(&want, &self.info, &self.frames).is_none() && self.poked.as_ref().map_or(true, |(k, t)| k != &want || t.elapsed().as_secs() >= 4) {
            if let Some((host, ks, m)) = nc::stock_opener(&want) {
                // the 3D viewer is the PCB editor's: open that first
                let (host, ks, m) = if host == "PCB" && !self.frames.iter().any(|f| f.kind == "PCB") { nc::stock_opener("PCB").unwrap() } else { (host, ks, m) }; // i18n-ok: wire frame kind enum names
                if let Some(w) = self.frames.iter().find(|f| f.kind == host).map(|f| f.window) { nc::poke(w, ks, m); self.poked = Some((want, std::time::Instant::now())); }
            }
        }
        // a click over the editor gives it the keyboard; a click anywhere else in the client's window gives it back
        // a KiCad dialog (its own top-level, outside the pane) has the keyboard: it stays there, Escape and all (lane D: Symbol Properties
        // could not be dismissed with Escape, the keyboard taken back to the frame and Backplane's window)
        let in_dialog = self.info["mode"] == "fork" && nc::focus_in_pid(self.info["pid"].as_u64().unwrap_or(0) as u32) && !nc::has_focus(c);
        // a press on another top-level over the pane (a KiCad dialog, another program) is not the editor's: nothing is taken or given
        let elsewhere = self.want().is_some_and(|w| nc::button_over(w).is_some() && nc::pointer_on_other_window(w));
        if elsewhere { self.press_focus = None; }
        if let (Some(w), false, false) = (self.want(), in_dialog, elsewhere) {
            // BACKPLANE_KICAD_FOCUS_ON_PRESS: the old way (focus mid-press), to check a KiCad that must cope with it
            let fork = self.info["mode"] == "fork" && std::env::var_os("BACKPLANE_KICAD_FOCUS_ON_PRESS").is_none();
            match (nc::button_over(w), nc::has_focus(c)) {
                (Some(true), false) if fork => { self.press_focus = Some(w); self.press_in = true; self.busy_in_kicad(); }
                (Some(true), false) => { nc::focus(w); self.press_in = true; self.busy_in_kicad(); }
                (Some(true), true) => { self.press_in = true; self.busy_in_kicad(); }
                // a press that began over the editor stays the editor's until it ends, however the layout moves under it: a drag in KiCad
                // that hits one of its DRC markers opens the Checks panel, the pane shrinks under the pointer, and the keyboard went to
                // Backplane mid-drag (lane B, Hubble: the person's next Escape closed the viewer and the editor with it)
                (Some(false), true) if !self.press_in && !self.press_gave_keys => nc::focus(self.toplevel),
                (None, false) => { self.press_in = false; self.press_gave_keys = false; if let Some(pw) = self.press_focus.take() { if pw == w { nc::focus(w); } } }
                (None, true) => { self.press_in = false; self.press_gave_keys = false; self.press_focus = None; }
                _ => {}
            }
        }
        let mut has = nc::has_focus(c) || in_dialog;
        // Windows: focus outside KiCad is not Backplane taking the keys unless it is really Backplane's (its own thread
        // owns the foreground). The desktop or another app having it for a moment (runs S1w, S2w: SysListView32, the
        // desktop) flipped the pane to "Backplane has the keys" while KiCad still had them, and raised the shield.
        if cfg!(windows) && self.keys && !has && !nc::keys_really_backplanes(c) { has = true; }
        // Windows: while the pane pre-warms or a hand-in is being verified (the F5 windows), KiCad's keys moving to Backplane's
        // own window with no press in it is a pre-warmed frame's activation ending (Windows passes it on), not the person
        // taking them (run S7w): the hand-in is posted once more instead of flipping the state and raising the shield.
        if cfg!(windows) && self.keys && !has && (self.prewarm_guard.is_some() || self.verify_handin.is_some())
            && nc::backplane_press_ms_ago() > 600 && !self.unexplained_reposted {
            if let Some(w) = self.want() {
                self.unexplained_reposted = true;
                eprintln!("GPUI_LOG {}", json!({"kicad_unexplained_focus_loss": {"frame": w, "why": nc::focus_debug(c)}}));
                nc::focus(w);
                has = true;
            }
        }
        // a KiCad dialog closed with the keyboard in it: to the dialog still open under it (a dialog of a dialog closed: back to its parent),
        // else to the editor's frame
        if std::mem::replace(&mut self.was_dialog, in_dialog) && !has {
            // (never when the person went to Backplane itself: the focus is then in our window, outside KiCad's container)
            let to_dialog = if nc::has_focus(self.toplevel) { None } else { nc::open_dialogs(self.info["pid"].as_u64().unwrap_or(0) as u32).last().copied() };
            if let Some(d) = to_dialog { nc::activate(d); self.was_dialog = true; has = true; }
            else if let Some(w) = self.want() { nc::focus(w); has = true; }
        }
        // KiCad took the keyboard on a click (`_BACKPLANE_FOCUS`) and a window manager that focuses on click (i3) gave it straight back to
        // Backplane's window: the click was KiCad's, so is the keyboard (once, within moments of it)
        if !has && self.keys {
            if let Some((w, at)) = self.took.take() { if at.elapsed() < std::time::Duration::from_millis(600) { nc::focus(w); has = true; } }
        }
        if has != self.keys && std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() {
            eprintln!("GPUI_LOG {}", json!({"kicad_keys_flip": {"to": has, "in_dialog": in_dialog, "has_focus": nc::has_focus(c), "why": nc::focus_debug(c)}}));
        }
        if has != self.keys { self.keys = has; nc::grab_escape(self.toplevel, has); self.grab_click(c, !has); if has { self.touched(); } cx.notify(); }
        if let Some(h) = self.info["holder"].as_u64().filter(|h| *h > 0 && *h != self.kept) { nc::keep(h); self.kept = h; }
        for e in nc::events() {
            // what reached Backplane from the editor (the e2e reads it: one click in KiCad, then a reserved chord, must arrive)
            eprintln!("GPUI_LOG {}", json!({"kicad_ev": match &e { nc::Ev::Escape => "escape".to_string(), nc::Ev::Click => "click".to_string(), nc::Ev::ShieldClick => "shield click".to_string(), nc::Ev::Key(k) => format!("key {k}"), nc::Ev::Focus(w) => format!("focus {w}") }}));
            match e {
                // the keyboard back to the client's window; a reserved chord is then pressed in gpui (after_frame, which has the window)
                nc::Ev::Escape => { self.refocus = None; nc::hand_back(self.toplevel); self.keys = false; nc::grab_escape(self.toplevel, false); self.grab_click(c, true); cx.notify(); }
                nc::Ev::Click => { if let Some(w) = self.want() { self.grab_click(c, false); nc::focus(w); self.keys = true; self.touched(); nc::grab_escape(self.toplevel, true); cx.notify(); } }
                // the shield did the hand-in itself and replays the press: the state only (no second focus call landing
                // after the replayed press, which closed a right click's menu before it opened)
                nc::Ev::ShieldClick => { if self.want().is_some() { self.grab_click(c, false); self.keys = true; self.touched(); nc::grab_escape(self.toplevel, true); cx.notify(); } }
                // Ctrl+Tab / Ctrl+Shift+Tab pressed in KiCad: the next editor tab, and KiCad keeps the keyboard
                nc::Ev::Key(k) if k == "ctrl-tab" || k == "ctrl-shift-tab" => { let have: Vec<String> = self.info["frames"].as_array().into_iter().flatten().filter_map(|f| f["kind"].as_str().map(String::from)).collect(); let to = next_open_tab(self.wanted_kind(), if k == "ctrl-tab" { 1 } else { -1 }, &have); self.show_kind_by(to, &format!("host shortcut {k} (KiCad key hook)"), cx); } // i18n-ok: a debug log cause (BACKPLANE_KICAD_DEBUG)
                nc::Ev::Key(k) => { eprintln!("GPUI_LOG {}", json!({"kicad_key": k, "at_ms": crate::screens::kcomment::epoch_ms() as u64})); self.refocus = None; nc::hand_back(self.toplevel); self.keys = false; nc::grab_escape(self.toplevel, false); self.grab_click(c, true);
                    // the comment chord (and bp.13's ';', KiCad's own comment key) opens the window now, from this event (no keystroke round trip through the next frame)
                    // the comment chord and the management panels' keys are acted on here, from the event: replayed as a keystroke after a
                    // comment window had the keyboard they reached no action (gpui's dispatch in this window found none), so they go by name
                    if k == "ctrl-;" || k == ";" || k.starts_with("alt-shift-") { cx.emit(PaneKey(k)); } else { self.press.push(k); } cx.notify(); }
                nc::Ev::Focus(_) => { if let Some(w) = self.want() { self.took = Some((w, std::time::Instant::now())); } if !self.keys { self.keys = true; self.touched(); nc::grab_escape(self.toplevel, true); self.grab_click(c, false); cx.notify(); } }
            }
        }
    }
    /// First render: the container in the client's window, and the hub asked to attach KiCad to it.
    fn decide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let top = top_of(window);
        // drawn in another window now (popped out into its own, or back): a new container there, KiCad's frames moved into it, the hub told
        if let (Some(_), Some(t)) = (self.container, top) {
            if t == self.toplevel { return }
            self.rehome(t);
            let weak = cx.entity().downgrade();
            window.on_frame_drawn(move |window, cx| weak.update(cx, |this, cx| this.after_frame(window, cx)).is_ok());
            return
        }
        if self.container.is_some() || self.attached { return }
        let Some(top) = top else { self.info = json!({"state": "failed", "reason": crate::i18n::trs("this window cannot hold another program's (only X11 and XWayland for now)")}); self.attached = true; return };
        self.toplevel = top; self.home = top;
        // the project's container kept from its last View (KiCad's frames still in it, attached): the editor is mapped in this very frame where
        // it was, with no round trip and no frame of the pane's background
        if let Some(k) = take_kept(top, &self.project) {
            self.container = Some(k.container); self.ever_shown = k.ever_shown; self.frames = nc::frames(k.container);
            if let (Some(r), Some(w)) = (k.rect, self.want()) { nc::place_local(k.container, Some(r), Some(w), &self.ever_shown, &mut self.last_root); self.placed = Some((Some(r), Some(w), Vec::new())); }
            self.handoff_over = true; // mapped in this very frame: nothing to hand over
        } else {
            self.container = nc::container_for(top, &self.project);
        }
        let Some(c) = self.container else {
            let why = if cfg!(windows) { crate::i18n::trs("could not create container window") } else { crate::i18n::trs("no X11 connection") };
            self.info = json!({"state": "failed", "reason": why});
            self.attached = true;
            return;
        };
        self.grab_click(c, true);
        self.attached = true;
        // macOS: a move or resize of our window, or its activation, redraws nothing by itself; KiCad's frame must follow (after_frame)
        #[cfg(target_os = "macos")]
        { cx.observe_window_bounds(window, |_, _, cx| cx.notify()).detach(); cx.observe_window_activation(window, |_, _, cx| cx.notify()).detach(); }
        // macOS: and the order of the two windows is watched between our frames (mac_watch): for as long as this pane lives
        #[cfg(target_os = "macos")]
        if !self.mac_watching {
            self.mac_watching = true;
            cx.spawn(async move |v, cx| loop {
                let Ok(ms) = v.update(cx, |this: &mut Self, cx| this.mac_watch(cx)) else { break };
                cx.background_executor().timer(std::time::Duration::from_millis(ms)).await;
            }).detach();
        }
        let display = std::env::var("DISPLAY").unwrap_or_default();
        let kind = self.attach_kind();
        // no frame from a page with no editor of its own: the hub opens a document the project has, and the pane follows it (#1347)
        self.pending_show = kind.map(str::to_string);
        let mut params = json!({"project": self.project, "container": c, "toplevel": top, "display": display, "wait": false, "theme": if self.light { "light" } else { "dark" }});
        if let Some(k) = kind { params["frame"] = json!(k); }
        self.rpc("editor.kicad.attach", params);
        let weak = cx.entity().downgrade();
        window.on_frame_drawn(move |window, cx| weak.update(cx, |this, cx| this.after_frame(window, cx)).is_ok());
    }
    /// Re-send `editor.kicad.attach` after a WebSocket reconnect so any info transition missed while disconnected is recovered.
    /// KiCad's start failed: the pane offers Try again.
    pub fn failed(&self) -> bool { try_again(&self.seen_info()) }
    pub fn reattach(&mut self) {
        let Some(c) = self.container else { return };
        let display = std::env::var("DISPLAY").unwrap_or_default();
        // what the pane shows now (KiCad's frame it followed), else its page's own editor, else none (#1347: the hub's choice)
        let kind = self.override_kind.clone().or_else(|| page_frame(&self.open).map(str::to_string));
        self.pending_show = kind.clone();
        let mut params = json!({"project": self.project, "container": c, "toplevel": self.toplevel, "display": display, "wait": false, "theme": if self.light { "light" } else { "dark" }});
        if let Some(k) = kind { params["frame"] = json!(k); }
        self.rpc("editor.kicad.attach", params);
    }
    /// After every frame: the container over the pane (the marker's bounds) with the wanted editor mapped, holes where gpui drew over it.
    fn after_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // a window this pane left (popped out, or back in) places nothing
        if top_of(window) != Some(self.toplevel) { return }
        if !self.press.is_empty() { self.lent_at = Some(std::time::Instant::now()); }
        for k in std::mem::take(&mut self.press) { if let Ok(k) = Keystroke::parse(&k) { eprintln!("GPUI_LOG {}", json!({"kicad_key_dispatch": k.unparse(), "at_ms": crate::screens::kcomment::epoch_ms() as u64})); window.defer(cx, move |window, cx| { window.dispatch_keystroke(k, cx); }); } }
        let Some(c) = self.container else { return };
        let seen = self.marker.lock().unwrap().and_then(|id| window.hitbox_drawn_after(id));
        self.at_px = seen.as_ref().map(|(b, _)| *b);
        let scale = window.scale_factor();
        let r = |b: &Bounds<Pixels>| nc::R { x: (f32::from(b.origin.x) * scale).round() as i32, y: (f32::from(b.origin.y) * scale).round() as i32, w: (f32::from(b.size.width) * scale).round() as i32, h: (f32::from(b.size.height) * scale).round() as i32 };
        let at = seen.as_ref().map(|(b, _)| r(b)).filter(|r| r.w > 0 && r.h > 0);
        let mut over: Vec<nc::R> = seen.as_ref().map(|(_, o)| o.iter().filter(|b| f32::from(b.size.width) >= 12. && f32::from(b.size.height) >= 12.).map(|b| r(b)).collect()).unwrap_or_default();
        // Centered modals (keysheet, improvements) draw a full-window dim backdrop plus a smaller card: cut out the card so the uncovered pane stays visible.
        if let Some(p) = at {
            if over.iter().any(|o| *o != p) { over.retain(|o| *o != p); }
        }
        // the pane asks about another KiCad's lock: no editor over the question (KiCad's frame, mapped in its container, covered the
        // pane's own buttons: a click on Open anyway went to KiCad, P3)
        let show = if lock_asked(&self.info) { None } else { self.want() };
        // (the pane's full-screen line comes and goes with our window's full screen: drawn again when it changes)
        #[cfg(target_os = "macos")]
        {
            let px = |b: &Bounds<Pixels>| (f32::from(b.size.width), f32::from(b.size.height));
            self.mac_covered = self.end_marker.lock().unwrap().and_then(|id| window.hitbox_drawn_after(id)).is_some_and(|(e, o)| covers_pane(px(&e), &o.iter().map(px).collect::<Vec<_>>()));
            if self.mac_host(at, at.is_some(), show) { cx.notify(); }
        }
        // bp.9 local map: the frames shown once are the client's to map and unmap, the switch with no round trip (KiCad follows the map state)
        let local = self.info["mode"] == "fork" && self.info["local_map"] == true;
        let managed: Option<Vec<u64>> = if local { None } else { (self.info["mode"] == "fork").then(|| {
            self.info["frames"].as_array().into_iter().flatten()
                .filter(|f| f["shown"] != false)
                .filter_map(|f| f["xid"].as_u64())
                .collect()
        }) };
        if let Some((w, since)) = self.refocus {
            if !self.keys || show != Some(w) || since.elapsed() > std::time::Duration::from_secs(3) { self.refocus = None; }
            else if nc::frame_visible(c, w) {
                self.refocus = None;
                if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() { eprintln!("GPUI_LOG {}", json!({"kicad_refocus_shown": {"frame": w, "after_ms": since.elapsed().as_millis() as u64}})); }
                nc::focus(w);
            } else { cx.notify(); }
        }
        if let Some((w, since)) = self.verify_handin {
            if !self.keys || show != Some(w) || since.elapsed() > std::time::Duration::from_millis(1500) { self.verify_handin = None; }
            else if let Some((stray, title)) = nc::invisible_kicad_foreground(w) {
                self.verify_handin = None;
                eprintln!("GPUI_LOG {}", json!({"kicad_stray_foreground": {"frame": w, "stray": stray, "title": title, "after_ms": since.elapsed().as_millis() as u64}}));
                nc::focus(w);
            } else { cx.notify(); }
        }
        // the first editor on show (bp.9+): the others pre-warmed at this pane's size, so their first switch is one paint with no relayout
        // (2 s after the first editor was placed: asked at once, KiCad's pre-warm sometimes hid the editor just shown, a rare race). Before the
        // unchanged-placement return: an idle pane is drawn again by tick's notify but placed the same, and the pre-warm waited for the next
        // re-place, the first tab switch (Nov 13: nothing asked 40 s after Edit, so that switch opened the schematic cold)
        // Windows: a runtime whose pre-warm takes the keyboard from the shown editor (no `prewarm-keeps-focus`) gave the hidden schematic the keys for
        // 350-476 ms on every open when it ran by itself (Carbon's idle_trace, 20/20 on c7fc906203): there it waits for the next placement, as
        // before 46343fe7e (a tab switch, a resize), where the F5 guard covers the hand-in
        let idle_ok = idle_prewarm(cfg!(windows), self.info["embed_caps"].as_array().map(|a| a.iter().any(|c| c == "prewarm-keeps-focus")).unwrap_or(false));
        let replace = self.placed.as_ref() != Some(&(at, show, over.clone()));
        if let (true, Some(p), Some(_), Some(pid), true) = (local, at, show, self.info["pid"].as_u64(), idle_ok || replace) {
            let since = *self.first_placed.get_or_insert_with(std::time::Instant::now);
            if self.prewarmed != Some(pid) && since.elapsed() >= std::time::Duration::from_secs(2) {
                self.prewarmed = Some(pid);
                let have: Vec<&str> = self.frames.iter().map(|f| f.kind).collect();
                self.rpc("editor.kicad.prewarm", json!({"project": self.project, "width": p.w, "height": p.h, "active": self.wanted_kind(), "have": have}));
                if cfg!(windows) { self.prewarm_guard = Some((std::time::Instant::now(), None, 0)); self.unexplained_reposted = false; }
            }
        }
        let prev_show = self.placed.as_ref().and_then(|(_, s, _)| *s);
        let key = (at, show, over.clone());
        if self.placed.as_ref() == Some(&key) {
            if let (Some(at), Some(show)) = (at, show) { nc::sync_root(c, at, show, &mut self.last_root); }
            return;
        }
        if local { nc::place_local(c, at, show, &self.ever_shown, &mut self.last_root); } else { nc::place(c, at, show, managed.as_deref(), &mut self.last_root); }
        if let (Some(w), Some(_), false, None) = (show, at, self.handoff_over, self.handoff_placed) { self.handoff_placed = Some((std::time::Instant::now(), w)); }
        // a KiCad dialog with the keyboard keeps it: the shown frame can change under an open dialog (a dialog of a dialog, Change
        // Footprints over Footprint Properties, re-placed the frame twice), and the hand-in took the keyboard from the dialog to the frame
        let kicad_dialog_has_keys = self.info["mode"] == "fork" && nc::focus_in_pid(self.info["pid"].as_u64().unwrap_or(0) as u32) && !nc::has_focus(c);
        if (self.keys || self.want_keys) && show != prev_show && !kicad_dialog_has_keys {
            if let Some(w) = show {
                if self.want_keys && nc::button_over(w) == Some(false) { self.press_gave_keys = true; }
                #[cfg(not(target_os = "macos"))]
                nc::focus(w);
                // macOS: consent, then ShowFrame{focus}: KiCad activates itself and puts the keyboard on its canvas (nc::yield_to)
                #[cfg(target_os = "macos")]
                { nc::yield_to(w); self.rpc("editor.kicad.show", json!({"project": self.project, "frame": self.wanted_kind(), "focus": true})); }
                if cfg!(windows) { self.verify_handin = Some((w, std::time::Instant::now())); self.unexplained_reposted = false; cx.notify(); }
                if cfg!(windows) && !nc::frame_visible(c, w) { self.refocus = Some((w, std::time::Instant::now())); cx.notify(); }
                if self.want_keys && !self.keys { self.keys = true; nc::grab_escape(self.toplevel, true); self.grab_click(c, false); self.touched(); }
                self.want_keys = false;
            }
        } else if self.want_keys && show.is_some() && show == prev_show {
            // already on show (Full size, the tab of the editor shown): the keyboard to it now
            let w = show.unwrap();
            if nc::button_over(w) == Some(false) { self.press_gave_keys = true; }
            #[cfg(not(target_os = "macos"))]
            nc::focus(w);
            #[cfg(target_os = "macos")]
            { nc::yield_to(w); self.rpc("editor.kicad.show", json!({"project": self.project, "frame": self.wanted_kind(), "focus": true})); }
            if cfg!(windows) { self.verify_handin = Some((w, std::time::Instant::now())); self.unexplained_reposted = false; cx.notify(); }
            if cfg!(windows) && !nc::frame_visible(c, w) { self.refocus = Some((w, std::time::Instant::now())); cx.notify(); }
            if !self.keys { self.keys = true; nc::grab_escape(self.toplevel, true); self.grab_click(c, false); self.touched(); }
            self.want_keys = false;
        }
        match at { Some(p) if !over.is_empty() => nc::holes(c, Some(&nc::cut(p, &over))), _ => nc::holes(c, None) }
        if key.1.is_some() { self.shown_once = true; }
        self.placed = Some(key);
    }
    /// macOS: KiCad's frame follows the pane by itself once it knows where the pane is on screen and whether our window can be seen
    /// (native_child_mac.rs); pushed when either changes. The order (KiCad's frame above our window) is `mac_order`'s.
    #[cfg(target_os = "macos")]
    fn mac_host(&mut self, at: Option<nc::R>, hitbox: bool, show: Option<u64>) -> bool {
        let full = nc::host_full_screen(self.toplevel);
        let redraw = full != self.mac_full;
        if redraw { self.mac_full = full; eprintln!("GPUI_LOG {}", json!({"kicad_mac_full_screen": full})); }
        let was = self.host_sent.is_some_and(|s| s.4);
        let dialog = self.seen_info()["dialogs"].as_array().is_some_and(|d| !d.is_empty());
        let visible = mac_shown(show.is_some(), nc::host_visible(self.toplevel), hitbox, self.in_page, self.mac_covered, dialog, was);
        // a frame in which the pane's hitbox was not found (our window inactive while KiCad has the keyboard draws such frames) keeps
        // the last rect, and the frame on show while the page holds the editor (mac_shown); an unseen window goes through host_visible
        // (a show held for the layout: its rect, not the last one sent)
        let rect = at.and_then(|r| nc::screen_rect(self.toplevel, r)).or_else(|| self.mac_show_held.map(|(r, _)| r)).or_else(|| self.host_sent.map(|(x, y, w, h, _)| (x, y, w, h)));
        let now = rect.map(|(x, y, w, h)| (x, y, w, h, visible));
        let due = host_push_due(now, self.host_sent, self.host_unsent);
        let settled = due && now.is_some_and(|n| show_settled(n, self.host_sent, &mut self.mac_show_held, at.is_some(), std::time::Instant::now()));
        if due && !settled {
            // a show at a rect not yet seen twice: held for the next frame's layout (show_settled); drawn again so that frame comes
            eprintln!("GPUI_LOG {}", json!({"kicad_host_push_held": now.map(|(x, y, w, h, _)| [x, y, w, h]), "at_ms": crate::screens::kcomment::epoch_ms() as u64}));
            return true;
        }
        if due {
            self.host_unsent = false;
            if let Some((x, y, w, h, v)) = now {
                eprintln!("GPUI_LOG {}", json!({"kicad_host_push": [x, y, w, h], "visible": v, "raise": false, "changed": true, "pane_px": at.map(|r| [r.x, r.y, r.w, r.h]), "frame": show, "in_page": self.in_page, "covered": self.mac_covered, "hitbox": hitbox, "dialog": dialog, "at_ms": crate::screens::kcomment::epoch_ms() as u64}));
                self.host_rpc(json!({"project": self.project, "x": x, "y": y, "width": w, "height": h, "visible": v, "raise": false}));
                let ms = mac_settle_ms(std::env::var("BACKPLANE_KICAD_MAC_SETTLE_MS").ok().as_deref());
                self.mac_settle = (ms > 0).then(|| std::time::Instant::now() + std::time::Duration::from_millis(ms));
                // the page let the editor go (#1199): the keyboard back to Backplane's window if KiCad had it, as detach does (a8bbdc74e);
                // the frame stays parked in KiCad, shown again at the next frame that draws the pane, and the keys go to it on a click
                // and whatever hid it (the hub closing the viewer too), a hand-in still waiting to fall back is void: KiCad is never
                // activated behind a frame that left the screen (nc::yield_to)
                if was && !v { nc::cancel_yield(); }
                if was && !v && (!self.in_page || self.mac_covered) && (self.keys || nc::has_focus(0)) {
                    nc::focus(self.toplevel); self.keys = false; self.want_keys = false; nc::grab_escape(self.toplevel, false);
                }
            }
            self.host_sent = now;
            // KiCad places (and may order) its frame when this arrives, some ms from now: watched
            self.mac_heat();
        }
        let said = self.mac_order(show, visible, "frame"); // i18n-ok: a log word
        redraw || said
    }
    /// A host push (`editor.kicad.host`): counted in flight and kept as the latest wanted (HostSends), then sent. A push the client's
    /// command queue refuses (try_send: full or closed) is still counted: it is never answered, and HOST_LOST_MS takes it as lost and
    /// sends the latest again.
    fn host_rpc(&self, params: Value) { self.host_sends.borrow_mut().sent(params.clone(), std::time::Instant::now()); self.rpc("editor.kicad.host", params); }
    /// macOS: the watch looks every frame's time for the next while (something that can change the order just happened).
    #[cfg(target_os = "macos")]
    fn mac_heat(&mut self) { self.mac_hot = Some(std::time::Instant::now() + std::time::Duration::from_millis(1500)); }
    /// macOS: KiCad's frame must be above our window, by the window server's own list (never by what we last asked for, and never
    /// only when gpui happens to draw: `mac_watch` calls this on a timer too). Found behind, our window is ordered just below it again
    /// (ours to do whoever is active), KiCad is asked to order its own when that can help (`raise_also_kicad`), and it is checked
    /// again a frame later; an order that does not hold within KICAD_RAISE_BOUND is said in the pane, with Pop out, and tried on
    /// once a second. Each try is logged with the stacking before and after it. True when what the pane says changed (draw again).
    /// (The Mac mini gate, Nov 16: a 1900 pt window, the pane blank with the frame behind ours: our own list had shown the frame
    /// above after two orders, nothing was drawn after that, and the frame was behind a moment later.)
    #[cfg(target_os = "macos")]
    fn mac_order(&mut self, show: Option<u64>, visible: bool, by: &'static str) -> bool {
        let was = self.mac_buried;
        // our window became key or stopped being it, or another frame is on show: orders follow from both sides
        let seen = (nc::host_key(self.toplevel), show);
        if seen != self.mac_seen { self.mac_seen = seen; self.mac_heat(); }
        let (true, Some(f)) = (visible, show) else { self.mac_raise = RaiseState::default(); self.mac_buried = false; return was };
        let plan = order_plan(&nc::stack(self.toplevel, f));
        match raise_step(plan == OrderPlan::Above, plan != OrderPlan::Idle, &mut self.mac_raise, &mut self.mac_buried, std::time::Instant::now()) {
            RaiseStep::Retry(n) => {
                let before = nc::stacking(self.toplevel, f);
                // THE RULE (order_plan): ours goes below the frame only past our own and KiCad's windows; with another app's window
                // between, nothing of ours moves, and KiCad is asked to bring its own frame to just above ours
                let sunk = plan == OrderPlan::AskKicad;
                let ordered = plan == OrderPlan::OursBelow && nc::order_below(self.toplevel, f);
                let kicad_active = nc::has_focus(0);
                let also = sunk || raise_also_kicad(&std::env::var("BACKPLANE_KICAD_MAC_RAISE").unwrap_or_default(), kicad_active, n);
                if let (true, Some((x, y, w, h, v))) = (also, self.host_sent) {
                    self.host_rpc(json!({"project": self.project, "x": x, "y": y, "width": w, "height": h, "visible": v, "raise": true}));
                }
                let said = self.mac_buried && !was;
                if n <= 8 || n % 30 == 0 || said {
                    eprintln!("GPUI_LOG {}", json!({"kicad_mac_order": {"try": n, "by": by, "ms": self.mac_raise.since.map_or(0, |t| t.elapsed().as_millis() as u64), "top": self.toplevel, "frame": f,
                        "ordered": ordered, "others_between": sunk, "key": seen.0, "kicad_active": kicad_active, "raise_kicad": also, "before": before, "after": nc::stacking(self.toplevel, f), "at_ms": crate::screens::kcomment::epoch_ms() as u64}}));
                }
                if said { eprintln!("GPUI_LOG {}", json!({"kicad_mac_buried": {"tries": n, "top": self.toplevel, "frame": f, "stack": nc::stacking(self.toplevel, f)}})); }
                self.mac_heat();
            }
            RaiseStep::Verified(n) => eprintln!("GPUI_LOG {}", json!({"kicad_mac_order_verified": {"tries": n, "by": by, "was_said": was, "stack": nc::stacking(self.toplevel, f), "at_ms": crate::screens::kcomment::epoch_ms() as u64}})),
            RaiseStep::Above | RaiseStep::Holding | RaiseStep::Idle => {}
        }
        was != self.mac_buried
    }
    /// macOS: the watch's own look at the order, between gpui's frames (KiCad orders its windows when it handles a push, takes or loses
    /// the keyboard or shows a frame: none of it draws a frame of ours). Answers when to look again, in ms.
    #[cfg(target_os = "macos")]
    fn mac_watch(&mut self, cx: &mut Context<Self>) -> u64 {
        // BACKPLANE_KICAD_MAC_WATCH=off (the e2e's A/B only): no look between our frames, as before Nov 16
        if std::env::var("BACKPLANE_KICAD_MAC_WATCH").is_ok_and(|v| v == "off") { return 1000 }
        // a press brings our window in front of the frame, and one on the title bar or an edge draws no frame of ours (the gate's blank
        // pane: a press on the resize corner of a window that could not grow): the button going down is seen here within a frame's time
        // the last push of a burst, once more when the burst is over: whatever order the pushes reached KiCad in, the last it applies is
        // the pane's rect and visibility now (mac_settle)
        if self.mac_settle.is_some_and(|t| t <= std::time::Instant::now()) {
            self.mac_settle = None;
            if let Some((x, y, w, h, v)) = self.host_sent {
                eprintln!("GPUI_LOG {}", json!({"kicad_host_push": [x, y, w, h], "visible": v, "raise": false, "changed": false, "settle": true, "at_ms": crate::screens::kcomment::epoch_ms() as u64}));
                self.host_rpc(json!({"project": self.project, "x": x, "y": y, "width": w, "height": h, "visible": v, "raise": false}));
            }
        }
        let pressed = nc::mouse_down();
        if pressed != self.mac_pressed { self.mac_pressed = pressed; self.mac_heat(); }
        let hot = self.mac_hot.is_some_and(|t| t > std::time::Instant::now());
        // the list itself is read at raise_watch_ms' pace; between its looks only the button is
        let due = raise_watch_ms(self.mac_raise.tries, self.mac_buried, hot);
        if self.mac_looked.is_some_and(|t| (t.elapsed().as_millis() as u64) + 5 < due) { return 33 }
        self.mac_looked = Some(std::time::Instant::now());
        // only a pane with its editor placed over it (not back in View, not before the first push)
        if let (Some(_), Some((.., sent_visible))) = (self.container, self.host_sent) {
            let show = if lock_asked(&self.info) { None } else { self.want() };
            let dialog = self.seen_info()["dialogs"].as_array().is_some_and(|d| !d.is_empty());
            let visible = mac_shown(show.is_some(), nc::host_visible(self.toplevel), self.at_px.is_some(), self.in_page, self.mac_covered, dialog, sent_visible);
            // our window hidden or back (minimised, another Space, Cmd+H) or the page letting the editor go, with no frame of ours drawn
            // (a hidden app draws none): pushed from here at the last rect, and drawn again for the next frame's own placement
            if visible != sent_visible || self.host_unsent { self.mac_host(None, self.at_px.is_some(), show); cx.notify(); }
            else if self.mac_order(show, visible, "watch") { cx.notify(); } // i18n-ok: a log word
        }
        33
    }
    /// Back to the viewer: the editor's windows leave the pane (KiCad stays warm in the hub).
    pub fn detach(&mut self) {
        // macOS: KiCad's frame leaves the screen with the pane (its windows are its own; nothing of ours to unmap)
        #[cfg(target_os = "macos")]
        nc::cancel_yield(); // the editor leaves the pane: a hand-in still waiting to fall back is void (nc::yield_to)
        #[cfg(target_os = "macos")]
        if let Some((x, y, w, h, true)) = self.host_sent.take() { self.host_rpc(json!({"project": self.project, "x": x, "y": y, "width": w, "height": h, "visible": false, "raise": false})); }
        let Some(c) = self.container.take() else { return };
        nc::place(c, None, None, None, &mut self.last_root); nc::grab_escape(self.toplevel, false);
        // the keyboard back to Backplane's window on leaving the pane (View, another thread)
        if self.keys { nc::focus(self.toplevel); self.keys = false; }
        // kept, unmapped, with KiCad's frames in it (still attached in the hub): the next Edit of this project in this window maps it at once
        // (a fork that maps locally; the oldest kept beyond KEEP is let go as before)
        // BACKPLANE_GPUI_KICAD_NO_KEEP=1 (e2e): the other way, the container destroyed (as for another window, or a start not ready)
        if self.info["mode"] == "fork" && self.info["local_map"] == true && self.info["state"] == "ready" && self.toplevel == self.home && std::env::var_os("BACKPLANE_GPUI_KICAD_NO_KEEP").is_none() {
            let rect = self.placed.as_ref().and_then(|(at, _, _)| *at);
            for old in keep(Kept { toplevel: self.toplevel, project: self.project.clone(), container: c, ever_shown: self.ever_shown.clone(), rect }) {
                self.rpc("editor.kicad.detach", json!({"project": old.project})); nc::destroy(old.container);
            }
            self.rpc("editor.kicad.detach", json!({"project": self.project, "keep": true}));
            return;
        }
        self.rpc("editor.kicad.detach", json!({"project": self.project})); nc::destroy(c);
    }
    /// The `{"kicad":{...}}` script step's answer: the container and every frame (geometry, mapped, painted), the hub's state, who has the keys.
    pub fn report(&self) -> Value {
        let live = self.container.map_or_else(Vec::new, nc::frames);
        let frames_src = if !live.is_empty() { &live } else { &self.frames };
        let shown = (!self.info.is_null() && !(cfg!(windows) && self.info["mode"] == "stock"))
            .then(|| pick_local(self.wanted_kind(), &self.info, frames_src, &self.ever_shown)).flatten();
        let top_depth = if self.toplevel != 0 { nc::window_meta(self.toplevel, None, self.toplevel).map(|(d, _, _, _)| d) } else { None };
        let g = |w: u64| nc::report(w).map_or(Value::Null, |(r, on, lit)| {
            let mut v = json!({"x": r.x, "y": r.y, "w": r.w, "h": r.h, "shown": on, "painted": lit});
            if let Some((depth, (rx, ry), rects, in_tree)) = nc::window_meta(w, self.container, self.toplevel) {
                v["depth"] = depth.into(); v["root_x"] = rx.into(); v["root_y"] = ry.into(); v["shape_rects"] = rects.into(); v["in_tree"] = in_tree.into();
            }
            v
        });
        let pane = self.placed.as_ref().and_then(|(at, _, _)| *at).map_or(Value::Null, |p| json!({"x": p.x, "y": p.y, "w": p.w, "h": p.h}));
        let shown_meta = shown.and_then(|w| nc::window_meta(w, self.container, self.toplevel));
        json!({"ok": true, "project": self.project, "root": self.root, "open": self.open, "want": self.wanted_kind(), "say": waiting(&self.seen_info(), shown.is_some()), "dialogs": self.seen_info()["dialogs"], "notice": open_notice(&self.info).map(|(_, t)| t), "jobs": self.jobs, "local_map": self.info["local_map"], "ever_shown": self.ever_shown.len(), "conflicts": self.conflicts,
            "state": self.info["state"], "reason": self.info["reason"], "mode": self.info["mode"], "pid": self.info["pid"],
            "holder": self.info["holder"], "sock": self.info["sock"], "pane": pane,
            "container": self.container.map_or(Value::Null, |c| { let mut v = g(c); v["xid"] = c.into(); v }),
            "toplevel": self.toplevel, "toplevel_depth": top_depth, "keys": self.keys, "handoff": self.handoff(),
            "frames": frames_src.iter().map(|f| { let mut v = g(f.window); v["xid"] = f.window.into(); v["kind"] = f.kind.into(); v["title"] = f.title.clone().into(); v }).collect::<Vec<_>>(),
            "shown": shown,
            "shown_root": shown_meta.map_or(Value::Null, |(_, (rx, ry), _, _)| json!({"x": rx, "y": ry})),
            "in_gpui_tree": shown_meta.map_or(false, |(_, _, _, ok)| ok)})
    }
}
impl Drop for KicadPane { fn drop(&mut self) { self.detach(); } }

/// At quit (the window's close, the app's quit): KiCad's holders out of every container of this window (nc::release_holders), before the
/// process ends and the X server destroys its windows with them.
pub fn release_all() { nc::release_holders(None); eprintln!("GPUI_LOG {}", json!({"kicad_release_at_quit": true})); }

impl Render for KicadPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.decide(window, cx);
        let light = self.light; let c = |n: &str| crate::theme::color(light, n);
        // a warm first Edit: nothing of the pane's own until KiCad's frame is up (the page draws the viewer's picture under it)
        let handoff = self.handoff();
        let say = if handoff { None } else { waiting(&self.seen_info(), self.want().is_some()) };
        // a notice during the start: said under "Starting KiCad…" (and in the strip, where it stays once the editor is up)
        let notice = say.as_ref().and_then(|_| open_notice(&self.info)).map(|(_, t)| t);
        let failed = self.failed(); let app = app_instead(&self.seen_info(), cfg!(windows)); let again = open_again(&self.seen_info()); let asked = lock_asked(&self.info);
        // macOS: in full screen KiCad's frame cannot be over the pane; it says so and offers the editor in a window of its own
        let say = if self.mac_full && self.info["state"] == "ready" { Some(crate::i18n::trs("KiCad cannot be shown here while Backplane is full screen on macOS. Pop it out into a window of its own, or leave full screen.")) } else { say };
        // macOS: KiCad's frame stays behind our window though we ordered it above (mac_host's bound): said, with Pop out, never a blank pane
        let buried = self.mac_buried && !self.mac_full && self.info["state"] == "ready";
        let say = if buried { Some(crate::i18n::t("gpui.c.kicad.mac.buried")) } else { say };
        let full = (self.mac_full || self.mac_buried) && self.info["state"] == "ready";
        let marker = self.marker.clone(); let focus = self.focus.clone();
        let keys = self.keys;
        // its rect as `kicad:pane` (an e2e judges KiCad's frame against the pane as it is drawn now: a panel, Full size, Full screen change it)
        crate::ui::with_rect(div().id("kicad-pane"), || "kicad:pane".to_string()).size_full().relative().when(!handoff, |d| d.bg(rgb(0x001023))).track_focus(&focus)
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.took = None;
                // macOS: a press on our window brings it in front of KiCad's frame: ordered back at once and watched (mac_order)
                #[cfg(target_os = "macos")]
                { this.mac_heat(); cx.notify(); }
                if this.keys {
                    nc::focus(this.toplevel);
                    this.keys = false;
                    nc::grab_escape(this.toplevel, false);
                    if let Some(c) = this.container { this.grab_click(c, true); }
                    cx.notify();
                }
            }))
            // the keyboard's outline: always 2 px, coloured only while KiCad has the keyboard, so the editor's container keeps one size
            // (a border that came and went resized KiCad by 4 px at each hand-over: a relayout and repaint each time)
            // in a warm hand-over the outline is the pane's navy where it would be clear (the picture under it otherwise showed in those 2 px,
            // then turned navy at the hold's end: a change after KiCad had finished painting, GPU display bp.12.4)
            .border_2().border_color(if keys { c("accent") } else if handoff { Rgba::from(rgb(0x001023)) } else { Rgba { a: 0., ..c("accent") } })
            .child(canvas(move |b, window, _| { *marker.lock().unwrap() = Some(window.insert_hitbox(b, HitboxBehavior::Normal).id); }, |_, _, _, _| {}).size_full())
            .when_some(say, |d, s| d.child(div().absolute().inset_0().flex().flex_col().gap_2().items_center().justify_center().text_color(c("faint")).child(s)
                .when_some(notice, |d, n| d.child(crate::ui::with_rect(div().id("kicad-notice-start"), || "kicad:notice-start".to_string()).max_w(px(520.)).text_size(crate::scale::tpx(12.)).text_color(c("dim")).child(n)))
                .when(again, |d| d.child(crate::ui::with_rect(div().id("kicad-open-again"), || "kicad:open-again".to_string()).px_3().py_1().rounded_md().cursor_pointer()
                    .bg(c("accent")).text_color(c("text")).child(crate::i18n::t("gpui.c.kicad.open.again"))
                    .on_click(cx.listener(|this, _, _, cx| { this.reattach(); cx.notify(); }))))
                .when(asked, |d| d.child(div().flex().flex_row().gap_2()
                    .child(crate::ui::with_rect(div().id("kicad-lock-open"), || "kicad:lock-open".to_string()).px_3().py_1().rounded_md().cursor_pointer()
                        .bg(c("raise")).hover(|x| x.bg(c("line"))).text_color(c("text")).child(crate::i18n::trs("Open anyway"))
                        .on_click(cx.listener(|this, _, _, cx| { this.rpc("editor.kicad.lock", json!({"project": this.project, "answer": "open"})); cx.notify(); })))
                    .child(crate::ui::with_rect(div().id("kicad-lock-cancel"), || "kicad:lock-cancel".to_string()).px_3().py_1().rounded_md().cursor_pointer()
                        .bg(c("raise")).hover(|x| x.bg(c("line"))).text_color(c("text")).child(crate::i18n::trs("Cancel"))
                        .on_click(cx.listener(|this, _, _, cx| { this.rpc("editor.kicad.lock", json!({"project": this.project, "answer": "cancel"})); cx.notify(); })))))
                .when(failed, |d| d.child(crate::ui::with_rect(div().id("kicad-retry"), || "kicad:retry".to_string()).px_3().py_1().rounded_md().cursor_pointer()
                    .bg(c("raise")).hover(|x| x.bg(c("line"))).text_color(c("text")).child(crate::i18n::trs("Try again"))
                    .on_click(cx.listener(|this, _, _, cx| { this.reattach(); cx.notify(); }))))
                .when(full, |d| d.child(crate::ui::with_rect(div().id("kicad-pop-full"), || "kicad:pop-full".to_string()).px_3().py_1().rounded_md().cursor_pointer()
                    .bg(c("raise")).hover(|x| x.bg(c("line"))).text_color(c("text")).child(crate::i18n::trs("Pop out"))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(PaneKey("kicad-popout".into()))))))
                // the editor cannot be here: the same design in the KiCad app instead, by the person's own press (never by itself)
                .when(app, |d| d.child(crate::ui::with_rect(div().id("kicad-app"), || "kicad:app".to_string()).px_3().py_1().rounded_md().cursor_pointer()
                    .bg(c("raise")).hover(|x| x.bg(c("line"))).text_color(c("text")).child(crate::i18n::trs("Open the KiCad app instead"))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(PaneKey("kicad-app".into()))))))))
            // macOS: the last of the pane's own hitboxes; what is drawn after it over all of the pane is a screen over the page (covers_pane).
            // Not on Linux and Windows: a hitbox the pane's size after the marker would be cut out of KiCad's frame there (holes)
            .when(cfg!(target_os = "macos"), |d| { let end = self.end_marker.clone(); d.child(canvas(move |b, window, _| { *end.lock().unwrap() = Some(window.insert_hitbox(b, HitboxBehavior::Normal).id); }, |_, _, _, _| {}).absolute().inset_0()) })
    }
}

#[cfg(test)]
mod tests {
    /// #1199: on macOS KiCad's frame leaves the screen when the page lets the editor go (the viewer closed, another thread or page), not
    /// on a frame that only missed the pane's hitbox (our window inactive while KiCad has the keyboard, d563d8a5ab), and never while
    /// KiCad has a dialog open; it comes back only once the pane is drawn again.
    #[test] fn macos_hides_kicads_frame_when_the_page_lets_the_editor_go() {
        use super::mac_shown;
        // (show, host_visible, hitbox, in_page, covered, dialog, was_visible)
        assert!(mac_shown(true, true, true, true, false, false, false), "the pane drawn: on show");
        assert!(!mac_shown(true, true, false, false, false, false, true), "the viewer closed: the pane not drawn, the page without it: hidden");
        assert!(mac_shown(true, true, false, true, false, false, true), "a frame with no hitbox, the page still holding the editor: kept on show");
        assert!(mac_shown(true, true, false, false, false, true, true), "a KiCad dialog open: its frame kept, never a stranded modal");
        assert!(!mac_shown(true, true, false, true, false, false, false), "hidden, the page back but no hitbox yet: shown only once it is drawn");
        assert!(mac_shown(true, true, true, true, false, false, false), "the page back and drawn: on show again");
        assert!(!mac_shown(true, false, true, true, false, false, true), "our window unseen (minimised, Cmd+H): hidden");
        assert!(!mac_shown(false, true, true, true, false, false, true), "no editor wanted: hidden");
        // Settings (or another full-window screen) over the page: the pane drawn under it, but KiCad's frame would be above it
        assert!(!mac_shown(true, true, true, true, true, false, true), "Settings over the page: hidden");
        assert!(mac_shown(true, true, true, true, true, true, true), "Settings over a KiCad dialog: the dialog kept");
        assert!(mac_shown(true, true, true, true, false, false, false), "Settings closed: on show again");
        assert_eq!((super::mac_settle_ms(None), super::mac_settle_ms(Some("0")), super::mac_settle_ms(Some("x"))), (super::MAC_SETTLE_MS, 0, super::MAC_SETTLE_MS));
        use super::covers_pane;
        assert!(covers_pane((800., 600.), &[(800., 600.)]), "a backdrop over the whole pane");
        assert!(!covers_pane((800., 600.), &[(200., 30.), (300., 600.)]), "a tooltip, a menu or a card over a part of it");
        assert!(!covers_pane((800., 600.), &[]) && !covers_pane((0., 0.), &[(0., 0.)]));
    }
    /// #1199: a host push KiCad refuses (a DRC running past 300 ms) is sent again, the latest one, until taken; a newer push (a hide)
    /// replaces a waiting retry; an earlier push's answer decides nothing; an answer never coming is taken as lost.
    #[test] fn a_refused_host_push_is_sent_again_until_taken_and_a_newer_one_wins() {
        use super::{HostSends, HOST_RETRY_MS, HOST_RETRY_CAP_MS, HOST_LOST_MS};
        use serde_json::json;
        let ms = std::time::Duration::from_millis; let t0 = std::time::Instant::now();
        let show = json!({"x": 430, "width": 496, "height": 500, "visible": true, "raise": false}); let hide = json!({"x": 430, "width": 496, "height": 500, "visible": false, "raise": false});
        // a DRC well past 300 ms: refused, refused, refused, then taken; each retry the latest push, the wait growing to its cap
        let mut s = HostSends::default();
        s.sent(show.clone(), t0);
        let at = s.reply(false, t0 + ms(10)).expect("refused: a retry"); assert_eq!(at, t0 + ms(10 + HOST_RETRY_MS));
        assert_eq!(s.due(t0 + ms(100)), None, "not before its time");
        let mut now = at; let mut waits = vec![];
        for _ in 0..6 { assert_eq!(s.due(now), Some(show.clone()), "the latest push, again"); s.sent(show.clone(), now); let next = s.reply(false, now + ms(5)).unwrap(); waits.push((next - (now + ms(5))).as_millis() as u64); now = next; }
        assert_eq!(waits, vec![2 * HOST_RETRY_MS, 4 * HOST_RETRY_MS, HOST_RETRY_CAP_MS, HOST_RETRY_CAP_MS, HOST_RETRY_CAP_MS, HOST_RETRY_CAP_MS], "bounded, never a tight loop");
        assert_eq!(s.due(now), Some(show.clone())); s.sent(show.clone(), now); assert_eq!(s.reply(true, now + ms(5)), None, "taken: nothing more");
        assert_eq!(s.due(now + ms(60_000)), None);
        // a newer hide replaces a waiting retry of a show: the show is never sent again
        let mut s = HostSends::default();
        s.sent(show.clone(), t0); s.reply(false, t0).unwrap();
        s.sent(hide.clone(), t0 + ms(50));
        assert_eq!(s.due(t0 + ms(400)), None, "the hide in flight: no retry of the show");
        assert_eq!(s.reply(true, t0 + ms(60)), None);
        assert_eq!(s.due(t0 + ms(5_000)), None, "the hide taken: the closed viewer stays closed");
        // the same with the hide refused too: what is sent again is the hide
        let mut s = HostSends::default();
        s.sent(show.clone(), t0); s.reply(false, t0).unwrap(); s.sent(hide.clone(), t0 + ms(50)); let at = s.reply(false, t0 + ms(60)).unwrap();
        assert_eq!(s.due(at), Some(hide.clone()), "the latest wanted: the hide");
        // two in flight: the earlier one's refusal decides nothing, the latest's answer does
        let mut s = HostSends::default();
        s.sent(show.clone(), t0); s.sent(hide.clone(), t0 + ms(5));
        assert_eq!(s.reply(false, t0 + ms(10)), None, "an earlier push refused, the latest still to answer");
        assert_eq!(s.reply(true, t0 + ms(20)), None); assert_eq!(s.due(t0 + ms(5_000)), None, "the latest taken");
        // an answer that never comes (the hub reconnected): the latest push is due again once it is that late
        let mut s = HostSends::default();
        s.sent(show.clone(), t0); assert_eq!(s.next_look(), Some(t0 + ms(HOST_LOST_MS)));
        assert_eq!(s.due(t0 + ms(HOST_LOST_MS - 1)), None); assert_eq!(s.due(t0 + ms(HOST_LOST_MS)), Some(show.clone()));
    }
    /// #1199 (lane B): the watch's own path, from the first push with no answer at all to the push sent again; every push wakes it;
    /// a retry never raises; a new hub drops what the old one refused.
    #[test] fn the_watch_sends_an_unanswered_first_push_again_without_a_raise() {
        use super::{HostSends, HostStep, HOST_RETRY_MS, HOST_LOST_MS};
        use serde_json::json;
        let ms = std::time::Duration::from_millis; let t0 = std::time::Instant::now();
        let mut s = HostSends::default();
        let (tx, rx) = async_channel::bounded::<()>(1); s.set_wake(tx);
        assert_eq!(s.watch_step(t0), HostStep::Idle, "nothing sent: asleep");
        // the first push (a show with a raise), never answered (KiCad stopped)
        let show = json!({"x": 430, "width": 496, "height": 500, "visible": true, "raise": true});
        s.sent(show.clone(), t0);
        assert!(rx.try_recv().is_ok(), "the first push wakes the watch");
        assert_eq!(s.watch_step(t0 + ms(10)), HostStep::Look(ms(HOST_RETRY_MS)), "an answer outstanding: looked at again, bounded");
        let mut now = t0 + ms(10); let mut looks = 0;
        let sent = loop { match s.watch_step(now) { HostStep::Look(d) => { assert!(d <= ms(HOST_RETRY_MS) && d > ms(0)); now += d; looks += 1; } HostStep::Send(p) => break p, HostStep::Idle => panic!("asleep with an answer outstanding") } };
        assert!(now >= t0 + ms(HOST_LOST_MS) && looks <= (HOST_LOST_MS / HOST_RETRY_MS) as usize + 1, "sent again once the answer is that late ({looks} looks)");
        assert_eq!(sent["raise"], json!(false), "a retry never raises"); assert_eq!(sent["visible"], json!(true)); assert_eq!(sent["width"], json!(496));
        assert!(rx.try_recv().is_ok(), "the retry is a push of its own");
        // a refused retry: sent again, without a raise, until taken; then asleep
        let at = s.reply(false, now + ms(5)).unwrap();
        assert!(matches!(s.watch_step(now + ms(5)), HostStep::Look(_)));
        let HostStep::Send(again) = s.watch_step(at) else { panic!("due") }; assert_eq!(again["raise"], json!(false));
        s.reply(true, at + ms(5)); assert_eq!(s.watch_step(at + ms(6)), HostStep::Idle, "taken: asleep again");
        // a new hub: what the old one refused is not sent again
        s.sent(show.clone(), at + ms(10)); s.reply(false, at + ms(20)).unwrap(); assert_eq!(s.rejoined().map(|p| p["visible"].clone()), Some(json!(true)));
        assert_eq!(s.watch_step(at + ms(10_000)), HostStep::Idle, "rejoined: nothing of the old hub's is due");
        // the same KiCad after a reconnect, the pane's rect unchanged: pushed again (lane B), and not again once pushed
        use super::host_push_due;
        let r = Some((430, 153, 496, 500, true));
        assert!(!host_push_due(r, r, false), "unchanged: nothing");
        assert!(host_push_due(r, r, true), "unchanged, but the connection was made again: pushed");
        assert!(host_push_due(Some((430, 153, 496, 524, true)), r, false) && !host_push_due(None, r, true), "a new rect; no rect at all");
    }
    /// #1199 (e2e-macos 44314267887 leave a): a hand-in's fallback acts only while nothing came after it.
    #[test] fn a_hand_in_fallback_is_void_once_backplane_takes_the_keyboard_or_the_frame_leaves() {
        use super::yield_current;
        let mut g = 0u64; let mut next = || { g += 1; g };
        let click = next(); // the canvas click: yield_to takes generation 1
        assert!(yield_current(click, click), "nothing since: the fallback activates KiCad, as before");
        let x = next(); // the viewer's Close: focus of our window (cancel_yield)
        assert!(!yield_current(click, x), "Backplane took the keyboard: the fallback is skipped");
        let hide = next(); assert!(!yield_current(click, hide), "the frame left the screen: skipped");
        let again = next(); assert!(yield_current(again, again) && !yield_current(click, again), "a newer hand-in voids the older one only");
    }
    /// e2e-macos 44465483071 leave (b): the page's first layout 496x524, a frame later 496x500; one push, at 500 (#1199).
    #[test] fn a_show_waits_for_its_layout_to_settle() {
        use super::{show_settled, MAC_SHOW_HOLD_MS};
        let t0 = std::time::Instant::now(); let ms = |n| t0 + std::time::Duration::from_millis(n);
        let old = Some((430, 153, 496, 500, false));
        let mut held = None;
        assert!(!show_settled((430, 153, 496, 524, true), old, &mut held, true, ms(0)), "a show at a new rect waits a frame");
        assert!(show_settled((430, 153, 496, 500, true), old, &mut held, true, ms(22)) && held.is_none(), "settled at the rect last sent: sent");
        let mut held = None;
        assert!(!show_settled((430, 153, 496, 524, true), None, &mut held, true, ms(0)));
        assert!(!show_settled((430, 153, 496, 500, true), None, &mut held, true, ms(22)), "the layout changed: waits again");
        assert!(!show_settled((430, 153, 496, 500, true), None, &mut held, false, ms(40)), "the watch's look is not a second layout");
        assert!(show_settled((430, 153, 496, 500, true), None, &mut held, true, ms(44)) && held.is_none(), "the same rect twice: sent");
        let mut held = None;
        assert!(show_settled((430, 153, 496, 500, true), old, &mut held, true, ms(0)), "back at the rect last sent (a covering sheet closed): at once");
        assert!(show_settled((1, 2, 3, 4, true), Some((430, 153, 496, 500, true)), &mut held, true, ms(0)), "while shown every rect goes at once");
        assert!(show_settled((1, 2, 3, 4, false), old, &mut held, true, ms(0)), "a hide goes at once");
        let mut held = None;
        for (i, h) in [524, 510, 505, 502].iter().enumerate() { assert!(!show_settled((0, 0, 496, *h, true), None, &mut held, true, ms(i as u64 * 20))); }
        assert!(show_settled((0, 0, 496, 501, true), None, &mut held, true, ms(MAC_SHOW_HOLD_MS)), "a layout that keeps moving goes at the hold's end");
        let mut held = None;
        assert!(!show_settled((0, 0, 496, 524, true), None, &mut held, true, ms(0)));
        assert!(show_settled((0, 0, 496, 524, false), None, &mut held, true, ms(10)) && held.is_none(), "hidden again before it went: the held show is dropped");
    }
    #[test] fn a_frame_left_behind_our_window_is_tried_again_then_said_with_pop_out() {
        use super::{raise_step, RaiseStep, RaiseState, KICAD_RAISE_BOUND, KICAD_RAISE_HELD};
        let ms = std::time::Duration::from_millis;
        let t0 = std::time::Instant::now(); let (mut st, mut buried) = (RaiseState::default(), false);
        // above and never behind: nothing to do
        assert_eq!(raise_step(true, true, &mut st, &mut buried, t0), RaiseStep::Above);
        // behind: a try each check, not yet said
        assert_eq!(raise_step(false, true, &mut st, &mut buried, t0), RaiseStep::Retry(1));
        assert_eq!(raise_step(false, true, &mut st, &mut buried, t0 + ms(33)), RaiseStep::Retry(2));
        assert!(!buried);
        // still behind past the bound: said (the pane offers Pop out), and still tried
        assert_eq!(raise_step(false, true, &mut st, &mut buried, t0 + KICAD_RAISE_BOUND), RaiseStep::Retry(3));
        assert!(buried);
        // above at last: watched until it has stayed there, then the tries reset and the pane shows the editor again
        let t1 = t0 + KICAD_RAISE_BOUND * 2;
        assert_eq!(raise_step(true, true, &mut st, &mut buried, t1), RaiseStep::Holding);
        assert_eq!(raise_step(true, true, &mut st, &mut buried, t1 + KICAD_RAISE_HELD), RaiseStep::Verified(3));
        assert!(!buried && st == RaiseState::default());
        // a frame not on screen (hidden by KiCad or by us): nothing to order, nothing said
        assert_eq!(raise_step(false, false, &mut st, &mut buried, t0), RaiseStep::Idle);
        assert!(!buried && st.tries == 0);
        // the words: the editor behind ours, and Pop out, never a blank pane
        let w = crate::i18n::t("gpui.c.kicad.mac.buried");
        assert!(w.contains("behind") && w.contains("Pop it out"), "{w}");
    }
    /// The gate's failure: our order takes (the next check reads "above"), then the other side's own order undoes it a moment later,
    /// again and again. One failure under one bound, not a fresh first try each time.
    #[test] fn an_order_undone_a_moment_later_is_the_same_failure_under_the_same_bound() {
        use super::{raise_step, RaiseStep, RaiseState, KICAD_RAISE_BOUND};
        let ms = std::time::Duration::from_millis;
        let t0 = std::time::Instant::now(); let (mut st, mut buried) = (RaiseState::default(), false);
        let mut t = t0; let mut n = 0;
        while t < t0 + KICAD_RAISE_BOUND {
            n += 1;
            assert_eq!(raise_step(false, true, &mut st, &mut buried, t), RaiseStep::Retry(n));
            assert_eq!(raise_step(true, true, &mut st, &mut buried, t + ms(33)), RaiseStep::Holding);
            assert!(!buried);
            t += ms(66);
        }
        assert_eq!(raise_step(false, true, &mut st, &mut buried, t), RaiseStep::Retry(n + 1));
        assert!(buried, "flapping past the bound is said");
    }
    /// THE RULE: the client orders its own window only, and never over or under another app's.
    #[test] fn the_client_orders_only_its_own_window_and_never_past_another_apps() {
        use super::{order_plan, OrderPlan::*, Who::*};
        // the frame above ours, KiCad's dialogs over it or between: well
        assert_eq!(order_plan(&[Frame, Ours]), Above);
        assert_eq!(order_plan(&[Kicad, Frame, Kicad, Ours, Other]), Above);
        // behind ours (a press brought ours up), nothing or only our own and KiCad's windows between: ours goes below the frame
        assert_eq!(order_plan(&[Ours, Frame]), OursBelow);
        assert_eq!(order_plan(&[Other, Ours, OurOther, Kicad, Frame, Other]), OursBelow);
        // another app's window between ours and the frame: ours would sink under it. Never: KiCad's to bring its frame up
        assert_eq!(order_plan(&[Ours, Other, Frame]), AskKicad);
        assert_eq!(order_plan(&[Ours, Kicad, Other, OurOther, Frame]), AskKicad);
        // the frame above another app's window that is above ours: ours would jump over that window. Never: KiCad's to bring it down
        assert_eq!(order_plan(&[Frame, Other, Ours]), AskKicad);
        // one of our own other windows between the frame and ours: not ours to reorder under it by moving the main window; KiCad's
        assert_eq!(order_plan(&[Frame, OurOther, Ours]), AskKicad);
        // the frame or our window not on screen: nothing
        assert_eq!(order_plan(&[Ours, Other]), Idle);
        assert_eq!(order_plan(&[Other, Frame]), Idle);
        assert_eq!(order_plan(&[]), Idle);
        // every arrangement of five windows: ours is moved only when no other app's window lies between it and the frame, so the
        // move (to just below the frame) crosses none
        let all = [Ours, Frame, Kicad, OurOther, Other];
        for a in 0..5 { for b in 0..5 { for c in 0..5 { for d in 0..5 { for e in 0..5 {
            let st = [all[a], all[b], all[c], all[d], all[e]];
            if st.iter().filter(|w| **w == Ours).count() != 1 || st.iter().filter(|w| **w == Frame).count() != 1 { continue }
            let (u, f) = (st.iter().position(|w| *w == Ours).unwrap(), st.iter().position(|w| *w == Frame).unwrap());
            if order_plan(&st) == OursBelow { assert!(u < f && !st[u + 1..f].contains(&Other), "{st:?}"); }
        } } } } }
    }
    /// The only window the macOS module ever orders is our own: one call site, on our own NSWindow, below the frame.
    #[test] fn the_macos_module_orders_no_window_but_ours() {
        let src = include_str!("native_child_mac.rs");
        let code: Vec<&str> = src.lines().filter(|l| !l.trim_start().starts_with("//")).collect();
        let orders: Vec<&&str> = code.iter().filter(|l| l.contains("orderWindow:") || l.contains("orderFront") || l.contains("orderBack") || l.contains("orderOut") || l.contains("setLevel")).collect();
        assert_eq!(orders.len(), 1, "{orders:?}");
        assert!(orders[0].contains("ns_window(top)") && orders[0].contains("NSWindowBelow"), "{orders:?}");
        // and it reaches a window only through our own app's table (windowWithWindowNumber: answers nil for anyone else's)
        assert!(src.contains(r#"fn ns_window(n: u64) -> *mut c_void { unsafe { send_isize_arg(app(), c"windowWithWindowNumber:", n as isize) } }"#));
    }
    #[test] fn kicad_is_asked_to_order_its_frame_only_when_that_can_help() {
        use super::{raise_also_kicad, raise_watch_ms};
        // KiCad active: both sides order (as before). Backplane active: ours alone (KiCad's would land its frame behind our key window),
        // and KiCad's too on every fourth try in case ours is the one that does not take
        assert!(raise_also_kicad("", true, 1));
        assert!(!raise_also_kicad("", false, 1) && !raise_also_kicad("", false, 3) && raise_also_kicad("", false, 4));
        assert!(raise_also_kicad("always", false, 1) && !raise_also_kicad("never", true, 4));
        // the watch: quick while unproven or just changed, slow once said, steady otherwise
        assert_eq!((raise_watch_ms(1, false, false), raise_watch_ms(0, false, true), raise_watch_ms(0, false, false), raise_watch_ms(9, true, true)), (33, 33, 250, 1000));
    }
    #[test] fn a_start_waiting_on_a_macos_prompt_says_so() {
        let i = |st: &str, reason: &str| json!({"state": st, "reason": reason, "mode": "fork", "frames": [], "dialogs": []});
        let say = super::waiting_on(&i("starting", "mac-permission-prompt"), false, false).unwrap();
        assert!(say.contains("macOS is asking for permission for Backplane"), "{say}");
        // KiCad's recovery alert on macOS (no dialog event): KiCad's question with what Restore and Discard do, never the permission words
        let rec = super::waiting_on(&i("starting", "kicad-recover-prompt"), false, false).unwrap();
        assert!(rec.contains("KiCad is asking") && rec.contains("Recover Unsaved Changes") && rec.contains("Restore") && rec.contains("Discard") && !rec.contains("permission"), "{rec}");
        assert_eq!(super::waiting_on(&i("starting", ""), false, false).as_deref(), Some("Starting KiCad…"));
        // another live KiCad's lock: asked before KiCad opens the document, whatever the state, over the editor too
        let ask = super::waiting_on(&json!({"state": "starting", "lock_ask": {"file": "a.kicad_pro", "holder": "kicad (pid 55) of h at desktop"}}), false, false).unwrap();
        assert!(ask.contains("a.kicad_pro") && ask.contains("pid 55") && ask.contains("anyway"), "{ask}");
        assert!(super::waiting_on(&json!({"state": "ready", "lock_ask": {"file": "a.kicad_sch", "holder": "x"}}), true, false).is_some());
        assert!(!super::try_again(&json!({"state": "failed", "lock_ask": {"file": "a"}})) && super::lock_asked(&json!({"lock_ask": {"file": "a"}})));
        // past 10 s a start says what KiCad is doing and for how long, never a silent "Starting KiCad…" (the fresh-home stall)
        let slow = |phase: &str, secs: u64| super::waiting_on(&json!({"state": "starting", "start_phase": phase, "starting_for_s": secs}), false, false).unwrap();
        assert_eq!(slow("socket", 9), "Starting KiCad…");
        assert_eq!(slow("socket", 12), "Starting KiCad: KiCad is up, waiting for it to answer (12 s)");
        assert!(slow("spawned", 10).contains("loading") && slow("answering", 40).contains("opening the project") && slow("first show", 11).contains("drawing"));
        assert!(slow("", 30).contains("waiting for KiCad to be started"));
        assert!(super::waiting_on(&json!({"state": "starting", "start_phase": "socket", "starting_for_s": 30, "dialogs": ["Configure Global Footprint Library Table"]}), false, false).unwrap().contains("Configure Global"), "a dialog KiCad shows is said over the phase");
        assert!(super::waiting_on(&i("failed", "KiCad waited 10 min on a macOS permission prompt that was not answered (answer it, then Try again)"), false, false).unwrap().contains("KiCad waited 10 min"));
    }
    #[test] fn a_warm_first_edit_holds_the_picture_until_kicad_has_drawn() {
        use std::time::Duration;
        assert!(super::handoff_holds(true, false, Duration::from_secs(2)), "with the event: held until it comes (the 3 s cap aside)");
        assert!(!super::handoff_holds(true, true, Duration::ZERO), "drawn: at once, no timer");
        assert!(super::handoff_holds(false, false, Duration::from_millis(100)) && !super::handoff_holds(false, false, Duration::from_millis(150)), "bp.12.x: the measured wait");
    }
    #[test] fn kicads_notice_is_one_line_until_dismissed() {
        use serde_json::json;
        let i = json!({"notices": [{"id": 1, "title": "Old", "detail": "", "dismissed": true}, {"id": 2, "title": "Could not use OpenGL, falling back to software rendering", "detail": "GLX missing", "dismissed": false}]});
        let (id, t) = super::open_notice(&i).unwrap();
        assert_eq!(id, 2); assert!(t.contains("Could not use OpenGL, falling back to software rendering (GLX missing)"), "{t}");
        assert!(super::open_notice(&json!({"notices": [{"id": 1, "title": "x", "dismissed": true}]})).is_none() && super::open_notice(&json!({})).is_none());
    }
    #[test] fn a_prewarm_reply_that_builds_nothing_ends_the_guard() {
        use serde_json::json;
        assert!(super::prewarm_builds_nothing(&json!({"ok": true, "prewarming": []})), "refused for memory");
        assert!(super::prewarm_builds_nothing(&json!({"ok": true, "prewarming": [], "prewarmed": []})), "no route / unsupported");
        assert!(super::prewarm_builds_nothing(&json!({"ok": true, "prewarmed": []})), "a hub before 676b72333");
        assert!(super::prewarm_builds_nothing(&json!({"ok": false, "error": "x"})), "an error");
        assert!(!super::prewarm_builds_nothing(&json!({"ok": true, "prewarming": ["SCH"]})));
        assert!(!super::prewarm_builds_nothing(&json!({"ok": true, "prewarming": true})), "an older hub's memory refusal: indistinguishable, kept");
    }
    #[test] fn idle_prewarm_waits_on_windows_without_prewarm_keeps_focus() {
        assert!(super::idle_prewarm(false, false), "Linux: the pre-warm starts by itself");
        assert!(!super::idle_prewarm(true, false), "Windows, a pre-warm that activates its frame: the next placement");
        assert!(super::idle_prewarm(true, true), "Windows with prewarm-keeps-focus");
    }
    #[test] fn an_early_edit_press_waits_for_the_viewer_then_edits() {
        use std::time::Duration;
        let p = |t, s| Some((t, Duration::from_secs(s)));
        assert_eq!(super::pending_edit_now(None, "t1", true), None, "nothing kept");
        assert_eq!(super::pending_edit_now(p("t1", 6), "t1", false), None, "still no file: keep waiting");
        assert_eq!(super::pending_edit_now(p("t1", 8), "t1", true), Some(true), "the file came: Edit now");
        assert_eq!(super::pending_edit_now(p("t1", 8), "t2", true), Some(false), "another thread: dropped");
        assert_eq!(super::pending_edit_now(p("t1", 61), "t1", true), Some(false), "a minute later: dropped");
    }
    #[test] fn warm_target_is_the_shown_board() {
        assert_eq!(warm_target("/p/Day_11/Day_11.kicad_pcb".into(), "/p".into()), "/p/Day_11/Day_11.kicad_pcb");
        assert_eq!(warm_target("/p/mech/case.step".into(), "/p".into()), "/p");
        assert_eq!(warm_target("/p".into(), "/p".into()), "/p");
        let key = "board|/p|Day_11/Day_11.kicad_pcb";
        let (proj, root) = edit_target(&serde_json::Value::Null, key, "").unwrap();
        assert_eq!(warm_target(proj, root), "/p/Day_11/Day_11.kicad_pcb");
    }
    use super::{belongs_to, edit_target, frame_for, hub_open_step, nc, path_stem, pick, project_of, root_of, waiting, waiting_on, warm_target};
    use serde_json::{json, Value};
    #[test] fn an_overlay_from_a_key_in_kicad_gives_the_keyboard_back_only_where_it_was_opened() {
        use super::{loan_back, Loan};
        let at = Loan { thread: "t1".into(), view: "board|0|/p|".into(), page: None };
        assert!(loan_back(&at, &at.clone(), true), "Escape, a click away, a choice that left the editor on show: back to KiCad");
        assert!(!loan_back(&at, &Loan { thread: "t2".into(), ..at.clone() }, true), "another thread: the keyboard stays with Backplane");
        assert!(!loan_back(&at, &Loan { view: "schematic|0|/p|".into(), ..at.clone() }, true), "another view");
        assert!(!loan_back(&at, &Loan { page: Some("p1".into()), ..at.clone() }, true), "a project's page over it");
        assert!(!loan_back(&at, &at.clone(), false), "the editor not on show");
    }
    #[test] fn a_frame_kicad_closed_under_the_pane_is_noticed_and_the_page_goes_back_to_view_or_its_own_editor() {
        use super::{after_close, closed_frame};
        let i = |pid: u64, st: &str, kinds: &[&str]| json!({"pid": pid, "state": st, "frames": kinds.iter().map(|k| json!({"kind": k})).collect::<Vec<_>>()});
        assert_eq!(closed_frame(&i(7, "ready", &["PCB", "SCH"]), &i(7, "ready", &["SCH"]), "PCB").as_deref(), Some("PCB"), "Ctrl+W on the board");
        assert_eq!(closed_frame(&i(7, "ready", &["PCB", "SCH"]), &i(7, "ready", &["PCB"]), "PCB"), None, "another frame closed");
        use super::missing_frame; assert!(missing_frame(&i(7, "ready", &["SCH"]), "PCB") && !missing_frame(&i(7, "ready", &["PCB"]), "PCB") && !missing_frame(&i(7, "starting", &[]), "PCB"), "Edit asks again only for a frame KiCad no longer has");
        assert_eq!(closed_frame(&i(7, "ready", &["PCB"]), &i(8, "ready", &[]), "PCB"), None, "a new KiCad: its own wait");
        assert_eq!(closed_frame(&i(7, "ready", &["PCB"]), &i(7, "stopped", &[]), "PCB"), None, "KiCad gone: said as such");
        assert_eq!(closed_frame(&i(7, "starting", &[]), &i(7, "ready", &[]), "PCB"), None, "not open yet");
        assert_eq!(after_close("PCB", "PCB", &i(7, "ready", &["SCH"])), None, "the page's own editor closed: View");
        assert_eq!(after_close("FOOTPRINT_EDITOR", "PCB", &i(7, "ready", &["PCB"])).as_deref(), Some("PCB"), "an extra tab closed: the page's editor");
        assert_eq!(after_close("PCB_3D", "PCB", &i(7, "ready", &["SCH"])), None, "and View when that one is gone too");
    }
    #[test] fn kicads_dialog_up_while_the_person_types_in_backplane_is_said_in_the_strip() {
        let d = vec!["Plot".to_string()];
        assert!(super::asking(&d, false).is_some_and(|t| t.contains("Plot")), "a dialog, the keyboard Backplane's: said");
        assert!(super::asking(&d, true).is_none(), "the keyboard KiCad's (the dialog has it): nothing to say");
        assert!(super::asking(&[], false).is_none(), "no dialog");
    }
    #[test] fn ctrl_tab_goes_round_the_editor_tabs() {
        assert_eq!(super::next_tab("PCB", 1), "SCH"); assert_eq!(super::next_tab("PCB", -1), "SIMULATOR"); assert_eq!(super::next_tab("FOOTPRINT_EDITOR", 1), "GERBVIEW"); assert_eq!(super::next_tab("GERBVIEW", 1), "SIMULATOR"); assert_eq!(super::next_tab("SIMULATOR", 1), "PCB");
        // the Simulator is gone round to only once KiCad has it
        assert_eq!(super::next_open_tab("PCB", -1, &[]), "GERBVIEW"); assert_eq!(super::next_open_tab("GERBVIEW", 1, &["PCB".into()]), "PCB");
        assert_eq!(super::next_open_tab("GERBVIEW", 1, &["SIMULATOR".into()]), "SIMULATOR"); assert_eq!(super::next_open_tab("PCB", -1, &["SIMULATOR".into()]), "SIMULATOR");
        // the rail's entry: the hub's word, in a window that can hold KiCad
        let v = serde_json::json!({"simFrame": "SIMULATOR"});
        assert_eq!(super::rail_simulation(&v, true).as_deref(), Some("SIMULATOR")); assert_eq!(super::rail_simulation(&v, false), None); assert_eq!(super::rail_simulation(&serde_json::json!({"open": "schematic"}), true), None);
        // closed in KiCad: back to View, whatever else is open
        assert_eq!(super::after_close("SIMULATOR", "SCH", &serde_json::json!({"frames": [{"kind": "SCH"}]})), None); assert_eq!(super::after_close("GERBVIEW", "PCB", &serde_json::json!({"frames": [{"kind": "PCB"}]})).as_deref(), Some("PCB")); assert_eq!(super::next_tab("?", 1), "SCH");
    }
    #[test] fn an_attach_names_a_frame_only_from_a_page_with_an_editor_of_its_own() {
        use super::page_frame;
        // #1347: the design pages name theirs; every other page none, and the hub opens a document the project has
        assert_eq!((page_frame("board"), page_frame("schematic"), page_frame("3d")), (Some("PCB"), Some("SCH"), Some("PCB_3D")));
        for p in ["files", "review", "mech", "browser", "split", "diff", ""] { assert_eq!(page_frame(p), None, "{p}"); }
    }
    #[test] fn rail_pages_name_editors() { assert_eq!(frame_for("board"), "PCB"); assert_eq!(frame_for("schematic"), "SCH"); assert_eq!(frame_for("3d"), "PCB_3D"); }
    #[test] fn the_project_comes_from_the_viewer_key() {
        assert_eq!(project_of("board|/p/proj|boards/a/a.kicad_pcb").as_deref(), Some("/p/proj/boards/a/a.kicad_pcb"));
        assert_eq!(project_of("schematic|/p/proj|/abs/x.kicad_sch").as_deref(), Some("/abs/x.kicad_sch"));
        assert_eq!(project_of("0|/p/|").as_deref(), Some("/p"), "the rail's default board: the project's root");
        assert_eq!(root_of("1|/p/proj/|sub/power.kicad_sch").as_deref(), Some("/p/proj"));
        assert_eq!(project_of(r"board|C:\bp\proj|amp.kicad_pcb").as_deref(), Some(r"C:\bp\proj\amp.kicad_pcb"));
        assert_eq!(project_of(r"schematic|C:\bp\proj|C:\bp\proj\amp.kicad_sch").as_deref(), Some(r"C:\bp\proj\amp.kicad_sch"));
        assert_eq!(root_of(r"0|C:\bp\proj\|").as_deref(), Some(r"C:\bp\proj"));
        assert_eq!(project_of("0||"), None); assert_eq!(project_of("nothing"), None);
        let data = json!({"projects": [{"id": "http://127.0.0.1:4100|p1", "root": r"C:\bp\proj\"}]});
        assert_eq!(edit_target(&data, "", "p1"), Some((r"C:\bp\proj".into(), r"C:\bp\proj".into())), "early rail press before viewer.key arrives falls back to project root");
        assert_eq!(edit_target(&data, r"0|C:\bp\proj|amp.kicad_pcb", "p1"), Some((r"C:\bp\proj\amp.kicad_pcb".into(), r"C:\bp\proj".into())));
        assert_eq!(edit_target(&data, "", "missing"), None);
    }
    #[test] fn dots_in_parent_folders_and_windows_separators_match_cleanly() {
        assert_eq!(path_stem("/tmp/bp-kembed-xr.lVLR/editors1/proj"), "/tmp/bp-kembed-xr.lVLR/editors1/proj");
        assert_eq!(path_stem("/tmp/bp-kembed-xr.lVLR/editors1/proj/amp.kicad_pro"), "/tmp/bp-kembed-xr.lVLR/editors1/proj/amp");
        assert_eq!(path_stem(r"C:\bp\run.1\proj\amp.kicad_pro"), r"C:\bp\run.1\proj\amp");
        assert!(!belongs_to("/tmp/bp-kembed-xr.lVLR/editors1/proj", "", "p1", "0|/tmp/bp-kembed-xr.lVLR/other|", "p2"));
        assert!(belongs_to(r"C:\bp\proj\amp.kicad_pcb", r"C:\bp\proj", "p1", "1|c:/bp/proj|c:/bp/proj/amp.kicad_sch", "p1"));
    }
    #[test] fn switching_rail_pages_in_same_project_keeps_the_editor_pane() {
        assert!(belongs_to("/p/amp/amp.kicad_pcb", "/p/amp", "p1", "1|/p/amp|/p/amp/amp.kicad_sch", "p1"), "board -> schematic");
        assert!(belongs_to("/p/amp/amp.kicad_pcb", "/p/amp", "p1", "1|/p/amp|/p/amp/power.kicad_sch", "p1"), "board -> subsheet");
        assert!(belongs_to("/p/amp/amp.kicad_pcb", "/p/amp", "p1", "", "p1"), "3d page before STEP/GLB exists");
        assert!(!belongs_to("/p/amp/amp.kicad_pcb", "/p/amp", "p1", "0|/p/other|/p/other/b.kicad_pcb", "p2"), "another project");
    }
    #[test] fn the_hubs_frames_win_over_titles_when_they_are_in_the_container() {
        let frames = vec![nc::Frame { window: 10, title: "a — PCB Editor".into(), kind: "PCB" }, nc::Frame { window: 11, title: "x".into(), kind: "MANAGER" }];
        assert_eq!(pick("PCB", &Value::Null, &frames), Some(10));
        assert_eq!(pick("PCB", &json!({"frames": [{"kind": "PCB", "xid": 11}]}), &frames), Some(11));
        assert_eq!(pick("PCB", &json!({"frames": [{"kind": "PCB", "xid": 99}]}), &frames), Some(10), "not (yet) in the container: by title");
        assert_eq!(pick("PCB", &json!({"mode": "fork", "frames": []}), &frames), None, "fork mode waits until ShowFrame registers xid");
        assert_eq!(pick("PCB", &json!({"mode": "fork", "frames": [{"kind": "PCB", "xid": 10}]}), &frames), Some(10));
        assert_eq!(pick("SCH", &Value::Null, &frames), None);
    }
    #[test] fn the_pane_says_what_it_waits_for() {
        assert!(waiting(&json!({"state": "ready", "task_wait": "DRC"}), false).unwrap().contains("DRC"), "a switch waiting on KiCad's check says so");
        assert!(waiting(&json!({"state": "ready", "task_wait": "DRC"}), true).is_none(), "an editor on show is not covered");
        assert!(waiting(&json!({"state": "starting"}), false).is_some());
        assert!(waiting(&json!({"state": "ready"}), true).is_none());
        assert!(waiting(&json!({"state": "failed", "reason": "no kicad"}), true).unwrap().contains("no kicad"));
        assert!(super::try_again(&json!({"state": "failed", "reason": "no kicad"})), "a failed start offers Try again");
        let closed = json!({"state": "closed_unexpectedly", "reason": "KiCad closed unexpectedly (signal 9, SIGKILL)", "frames": []});
        assert!(super::open_again(&closed) && !super::try_again(&closed) && !super::app_instead(&closed, false), "a death offers Open the editor again, alone");
        let say = super::waiting_on(&closed, false, false).unwrap();
        assert!(say.starts_with("KiCad closed unexpectedly.") && say.contains("were not kept") && !say.contains("could not start") && !say.contains("Fontconfig"), "{say}");
        let kept = json!({"state": "closed_unexpectedly", "reason": "x", "frames": [], "autosaves": ["_autosave-a.kicad_pcb"]});
        assert!(super::waiting_on(&kept, false, false).unwrap().contains("kept a copy"), "an autosave is offered back");
        let agent = json!({"state": "closed_unexpectedly", "reason": "x", "frames": [], "autosaves": [], "agent_unsaved": ["a.kicad_pcb"]});
        assert!(super::waiting_on(&agent, false, false).unwrap().contains("agent's change"), "an agent's unsaved change is said lost");
        let asking = json!({"state": "starting", "frames": [], "dialogs": ["Recover Unsaved Changes"]});
        assert_eq!(super::waiting_on(&asking, false, false).as_deref(), Some("KiCad is asking: Recover Unsaved Changes"));
        // a stock KiCad in the pane: the strip offers Backplane KiCad (Linux; Windows says it in the pane's middle), a fork's never
        assert!(super::stock_hint(&json!({"mode": "stock", "state": "ready"}), false) && super::stock_hint(&json!({"mode": "stock", "state": "starting"}), false));
        assert!(!super::stock_hint(&json!({"mode": "fork", "state": "ready"}), false) && !super::stock_hint(&json!({"mode": "stock", "state": "ready"}), true) && !super::stock_hint(&json!({"mode": "stock", "state": "stopped"}), false) && !super::stock_hint(&Value::Null, false));
        // where the editor cannot be in the pane, the KiCad app is offered (pressed, never opened by itself); not while it starts or runs
        assert!(super::app_instead(&json!({"state": "failed", "reason": "this window cannot hold another program's"}), false));
        assert!(super::app_instead(&json!({"mode": "stock", "state": "ready"}), true) && !super::app_instead(&json!({"mode": "stock", "state": "ready"}), false));
        assert!(!super::app_instead(&json!({"mode": "fork", "state": "starting"}), false) && !super::app_instead(&json!({"mode": "fork", "state": "ready"}), true));
        assert!(!super::try_again(&json!({"state": "starting"})) && !super::try_again(&json!({"state": "stopped"})));
        assert_eq!(
            waiting_on(&json!({"mode": "stock", "state": "ready"}), false, true).as_deref(),
            Some("The embedded editor needs Backplane KiCad."),
            "stock mode on Windows says Backplane KiCad is needed"
        );
        assert!(waiting_on(&json!({"mode": "stock", "state": "ready"}), true, false).is_none(), "stock mode on Linux embeds");
        let asking = waiting(&json!({"state": "ready", "dialogs": ["Load Schematic", "Information"]}), false).unwrap();
        assert!(asking.contains("Information") && !asking.contains("Starting"), "{asking}");
        assert!(waiting(&json!({"state": "ready", "dialogs": ["Information"]}), true).is_none(), "the editor on show says nothing over it");
    }
    #[test] fn stale_hub_open_never_reverts_a_rail_switch_while_svg_plots() {
        let mut last = "board".to_string();
        // Person clicked rail:Schematic (`cur_open` became "schematic"); hub's `viewer.open` is still "board" even after PRESSED expires.
        assert_eq!(hub_open_step("schematic", &mut last, "board", Some("schematic")), None);
        assert_eq!(hub_open_step("schematic", &mut last, "board", None), None, "stale board must not revert schematic");
        assert_eq!(last, "board");
        // Hub finishes plotting schematic and updates `viewer.open` -> "schematic": `last_hub_open` catches up with no duplicate switch.
        assert_eq!(hub_open_step("schematic", &mut last, "schematic", None), None);
        assert_eq!(last, "schematic");
        // Agent or remote client later changes `viewer.open` -> "3d": editor switches.
        assert_eq!(hub_open_step("schematic", &mut last, "3d", None), Some("3d"));
        assert_eq!(last, "3d");
    }
}

/// Whether this window can hold the thread's KiCad: the hub is on this machine (the editor runs beside it) and the thread is not another machine's.
pub fn local(hub_addr: Option<&str>, thread: &Value) -> bool {
    if thread["mark"]["name"].as_str().map_or(false, |m| !m.is_empty()) { return false }
    match hub_addr.map(|a| a.rsplit_once(':').map_or(a, |(h, _)| h)) { Some(h) => h.is_empty() || matches!(h, "127.0.0.1" | "localhost" | "[::1]"), None => true }
}

/// A pane's container after View, kept for its next Edit (detach, decide).
struct Kept { toplevel: u64, project: String, container: u64, ever_shown: Vec<u64>, rect: Option<nc::R> }
static KEPT: std::sync::Mutex<Vec<Kept>> = std::sync::Mutex::new(Vec::new());
const KEEP: usize = 3;
/// Keep `k` (replacing one of the same window and project); answers those let go to stay within KEEP.
fn keep(k: Kept) -> Vec<Kept> {
    let mut m = KEPT.lock().unwrap();
    let mut out: Vec<Kept> = Vec::new();
    if let Some(i) = m.iter().position(|x| x.toplevel == k.toplevel && x.project == k.project) { out.push(m.remove(i)); }
    m.push(k);
    while m.len() > KEEP { out.push(m.remove(0)); }
    out.retain(|o| !m.iter().any(|x| x.container == o.container));
    out
}
fn take_kept(toplevel: u64, project: &str) -> Option<Kept> {
    let mut m = KEPT.lock().unwrap();
    m.iter().position(|x| x.toplevel == toplevel && x.project == project).map(|i| m.remove(i))
}

/// Whether this hub's KiCad runtime has GerbView in process (a session said `embed-gerbview`).
pub static GERBVIEW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// The project last asked warm (kicad_warm), so each project shown in a row is asked once.
static WARM_LAST: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
/// The early-Edit e2e: BACKPLANE_GPUI_TEST_KEY_HOLD_MS=N keeps the viewer's file key out of the pictures for the client's first N ms (a
/// slow first preview, as on bp-win after an update), and with it the project, so Edit has nothing to open until then. Off unless set.
pub fn test_hold_key(data: &mut Value) {
    if !test_holding() { return }
    let v = &mut data["thread"]["viewer"];
    if v.is_object() { v["key"] = json!(""); if let Some(ps) = v["panes"].as_array_mut() { for p in ps { p["key"] = json!(""); } } }
}
/// Within BACKPLANE_GPUI_TEST_KEY_HOLD_MS of the client's first picture: Edit finds nothing to open (the project's folder too).
fn test_holding() -> bool {
    static T0: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let Some(ms) = std::env::var("BACKPLANE_GPUI_TEST_KEY_HOLD_MS").ok().and_then(|v| v.parse::<u64>().ok()) else { return false };
    T0.get_or_init(std::time::Instant::now).elapsed() < std::time::Duration::from_millis(ms)
}
/// The editor the next Edit opens the pane on, when not the page's own (the rail's Simulation: SIM); taken by kicad_edit when it acts.
static EDIT_TAB: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
/// An Edit pressed before the viewer knew its file: the thread, and when (kept 60 s).
static EDIT_PENDING: std::sync::Mutex<Option<(String, std::time::Instant)>> = std::sync::Mutex::new(None);
/// BACKPLANE_KICAD_DEBUG: each change of what the Edit button says ("edit", "opening", "view"), as drawn (a screenshot's timing cannot
/// prove the "Opening…" of an early press: Carbon, bp-win key-hold runs, Nov 14).
pub fn log_edit_label(kind: &str) {
    static LAST: std::sync::Mutex<&'static str> = std::sync::Mutex::new("");
    if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_none() { return }
    let k: &'static str = match kind { "opening" => "opening", "view" => "view", _ => "edit" };
    let mut l = LAST.lock().unwrap();
    if *l != k { *l = k; eprintln!("GPUI_LOG {}", json!({"kicad_edit_label": k})); }
}
/// Whether an Edit press waits on this thread's first picture (the button says Opening…).
pub fn edit_pending(thread: &str) -> bool { EDIT_PENDING.lock().unwrap().as_ref().is_some_and(|(t, at)| t == thread && at.elapsed() < std::time::Duration::from_secs(60)) }
/// What to do with a kept press on a picture: Edit now (the viewer has a target), keep waiting, or drop it (another thread, or stale).
pub fn pending_edit_now(pending: Option<(&str, std::time::Duration)>, thread: &str, has_target: bool) -> Option<bool> {
    let (t, age) = pending?;
    if t != thread || age > std::time::Duration::from_secs(60) { return Some(false) }
    has_target.then_some(true)
}

impl crate::Backplane {
    /// The hub's `editor.kicad.<digest>` (JSON text; "" when the editor went away): kept, and given to the pane when it is this project's.
    pub(crate) fn kicad_info(&mut self, digest: String, text: &str, cx: &mut Context<Self>) {
        let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
        // the runtime has GerbView in process (any of its sessions says embed-gerbview): Backplane's Gerbers page offers "Open in GerbView"
        if v["embed_caps"].as_array().is_some_and(|a| a.iter().any(|c| c == "embed-gerbview")) { GERBVIEW.store(true, std::sync::atomic::Ordering::Relaxed); }
        if v.is_null() { self.screens.kicad_infos.remove(&digest); } else { self.screens.kicad_infos.insert(digest.clone(), v.clone()); }
        let (has_pane, matched, pane_proj) = match self.screens.kicad.clone() {
            Some(p) => {
                let (m, prj) = { let pr = p.read(cx); (pr.matches(&v), pr.project.clone()) };
                if m { p.update(cx, |p, cx| p.set_info(v.clone(), cx)); }
                (true, m, prj)
            }
            None => (false, false, String::new()),
        };
        eprintln!("GPUI_LOG {}", json!({"kicad_info": digest, "gone": text.is_empty(), "state": v["state"], "mode": v["mode"], "pid": v["pid"], "active": v["active"], "frames": v["frames"].as_array().map_or(0, Vec::len), "dialogs": v["dialogs"], "project": v["project"], "root": v["root"], "has_pane": has_pane, "matched": matched, "pane_project": pane_proj}));
    }
    /// Direct reply from an `editor.kicad.*` RPC: surface synchronous attach refusals on the pane and seed initial info if `KicadEditor` has not arrived yet.
    pub(crate) fn kicad_reply(&mut self, method: String, reply: Value, cx: &mut Context<Self>) {
        // the hub builds no pre-warmed frame (refused for memory, no GUI route, a runtime without it, or an error): nothing will
        // take the foreground, so the pane's pre-warm guard stops instead of watching to its 60 s cap (lane B, 676b72333)
        if method == "editor.kicad.prewarm" && prewarm_builds_nothing(&reply) {
            if let Some(p) = self.screens.kicad.clone() { p.update(cx, |p, _| p.prewarm_guard = None); }
        }
        if reply["ok"] != true {
            let why = crate::hub::rpc_error(&reply);
            // macOS: KiCad refused a host push (a task running: "SetHostGeometry: a task is running: DRC", e2e-macos 44274808579, the
            // frame then never shown): the latest push is sent again until KiCad takes it or a newer one replaces it (HostSends)
            if method == "editor.kicad.host" {
                let at = self.screens.host_sends.borrow_mut().reply(false, std::time::Instant::now());
                eprintln!("GPUI_LOG {}", json!({"kicad_host_refused": why, "id": reply["id"], "retry_ms": at.map(|t| t.saturating_duration_since(std::time::Instant::now()).as_millis() as u64), "at_ms": crate::screens::kcomment::epoch_ms() as u64}));
                self.host_retry_watch(cx);
            }
            if method == "editor.kicad.attach" {
                if let Some(p) = self.screens.kicad.clone() {
                    p.update(cx, |p, cx| p.set_info(json!({"state": "failed", "reason": why}), cx));
                }
            }
            return;
        }
        if method == "editor.kicad.host" { self.screens.host_sends.borrow_mut().reply(true, std::time::Instant::now()); eprintln!("GPUI_LOG {}", json!({"kicad_host_reply": reply["applied"], "why": reply["why"], "error": reply["error"], "id": reply["id"], "at_ms": crate::screens::kcomment::epoch_ms() as u64})); }
        if method == "editor.kicad.attach" && reply["state"].is_string() {
            let mut v = reply;
            if let Some(obj) = v.as_object_mut() { obj.remove("t"); obj.remove("id"); obj.remove("ok"); }
            if let Some(p) = self.screens.kicad.clone() {
                // the reply to this pane's own attach: it binds the pane to the project the hub resolved
                if let Some(pro) = v["project"].as_str().filter(|s| !s.is_empty()) { let pro = pro.to_string(); p.update(cx, |p, _| p.bound = pro); }
                let apply = {
                    let pr = p.read(cx);
                    pr.matches(&v) && (pr.info.is_null() || v["state"] == "ready")
                };
                if apply { p.update(cx, |p, cx| p.set_info(v, cx)); }
                // info that came before the reply (kept by digest) applies now
                let late: Vec<Value> = self.screens.kicad_infos.values().cloned().collect();
                for i in late { if p.read(cx).matches(&i) { p.update(cx, |p, cx| p.set_info(i, cx)); } }
            }
        }
    }
    /// A thread is shown: its project's KiCad kept warm in the hub (`editor.kicad.warm`; the hub decides whether, embed.rs `warm_decision`), so
    /// Edit draws the editor at once instead of starting KiCad. Asked once per project shown in a row; only for a hub on this machine. Linux
    /// for now (a fork started with no window to live in parks its frames; Windows not yet checked).
    pub(crate) fn kicad_warm(&mut self) {
        let thread = &self.data["thread"];
        if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() { eprintln!("GPUI_LOG {}", json!({"kicad_pic": thread["id"], "viewer_open": thread["viewer"]["open"], "panel": self.data["panel"], "editing": self.screens.kicad.is_some()})); }
        if !cfg!(target_os = "linux") { return }
        if !local(self.screens.attacher.target.as_ref().map(|t| t.addr.as_str()), thread) { return }
        let tid = crate::screens::attach::raw_id(thread["id"].as_str().unwrap_or(""));
        if tid.is_empty() { return }
        let pid = crate::keys::project_of(&self.data, tid);
        // the board Edit will open (the viewer's own file once the hub's picture says it), not only the folder: in a folder of several
        // boards the folder resolves to its first one, and the board the person edits started cold (Nov 13, Framework_Expansion_Cards:
        // Day_01 warmed, Day_11 opened in 10.3 s)
        let key = crate::screens::viewer::shown_key(&thread["viewer"]);
        let Some((project, root)) = edit_target(&self.data, &key, &pid) else { return };
        let project = warm_target(project, root);
        { let mut last = WARM_LAST.lock().unwrap(); if *last == project { return } *last = project.clone(); }
        let display = std::env::var("DISPLAY").unwrap_or_default();
        let _ = self.tx.try_send(Command::Rpc("editor.kicad.warm".into(), Secret(json!({"project": project, "display": display, "theme": if self.light() { "light" } else { "dark" }})), "kicad"));
    }
    pub(crate) fn kicad_rejoin(&mut self, cx: &mut Context<Self>) {
        // a new hub (a restart ends its KiCads): the shown project is asked warm again
        WARM_LAST.lock().unwrap().clear(); self.kicad_warm();
        let dropped = self.screens.host_sends.borrow_mut().rejoined();
        // the same hub and KiCad, connected again: a push lost with the connection is not resent by anything else (an unchanged rect
        // pushes nothing, and set_info clears host_sent only for another KiCad): the pane pushes its state again, and with no pane a
        // hide that was the latest wanted is sent again (a hide is never stale: a new KiCad starts with its frames hidden)
        if let Some(p) = self.screens.kicad.clone() { p.update(cx, |p, cx| { p.host_unsent = true; p.reattach(); cx.notify(); }); }
        else if let Some(hide) = dropped.filter(|d| d["visible"] == false) {
            self.screens.host_sends.borrow_mut().sent(hide.clone(), std::time::Instant::now());
            let _ = self.tx.try_send(Command::Rpc("editor.kicad.host".into(), Secret(hide), "kicad"));
        }
    }
    /// Edit: the thread's design in the real KiCad, in the pane (the page stays on the rail's Board, Schematic or 3D).
    /// "Open in GerbView" on a Gerber set: the Board page with Edit on (the pane attaches where it is drawn), then the set in KiCad's
    /// GerbView (lane B's open_gerbers, once the pane is attached and KiCad ready). `files`: the set's job, else its files (the hub's).
    pub(crate) fn open_in_gerbview(&mut self, files: Vec<String>, cx: &mut Context<Self>) {
        if files.is_empty() { return }
        eprintln!("GPUI_LOG {}", json!({"gerbview": "open", "files": files.len()}));
        self.rail_press("view-kind", "board", cx); // i18n-ok: rail page kind
        if !self.kicad_edit(cx) { return }
        if let Some(p) = self.screens.kicad.clone() { p.update(cx, |p, cx| p.open_gerbers_soon(&files, cx)); }
    }
    /// The rail's Simulation: KiCad's Simulator in the pane. The page under it is a design page (the Schematic, when another kind of page
    /// is on show), Edit goes on, and the pane opens on the Simulator (a pane already there switches to it).
    pub(crate) fn open_simulation(&mut self, cx: &mut Context<Self>) {
        let thread = &self.data["thread"];
        let local = local(self.screens.attacher.target.as_ref().map(|t| t.addr.as_str()), thread);
        if rail_simulation(&thread["viewer"], local).is_none() { return }
        eprintln!("GPUI_LOG {}", json!({"simulation": "open"}));
        let v = &thread["viewer"];
        let design = matches!(v["open"].as_str(), Some("board" | "schematic" | "3d")) && v["gerbers"] != true && !thread["diff"].is_object(); // i18n-ok: rail page kinds
        if !design { self.rail_press("view-kind", "schematic", cx); } // i18n-ok: rail page kind
        *EDIT_TAB.lock().unwrap() = Some(SIM.into());
        self.kicad_edit(cx);
    }
    /// Whether the pane is on KiCad's Simulator (the rail lights Simulation, not the page under it).
    pub(crate) fn simulation_on(&self, cx: &App) -> bool { self.screens.kicad.as_ref().is_some_and(|p| p.read(cx).tab() == SIM) }
    /// After a picture: an Edit pressed before the viewer had a file, acted on now that it has one (or dropped: another thread, 60 s).
    pub(crate) fn edit_pending_due(&mut self, cx: &mut Context<Self>) {
        let pending = EDIT_PENDING.lock().unwrap().clone();
        let thread = &self.data["thread"];
        let tid = crate::screens::attach::raw_id(thread["id"].as_str().unwrap_or("")).to_string();
        let key = crate::screens::viewer::shown_key(&thread["viewer"]);
        let has = edit_target(&self.data, &key, &crate::keys::project_of(&self.data, &tid)).is_some() && !test_holding();
        match pending_edit_now(pending.as_ref().map(|(t, at)| (t.as_str(), at.elapsed())), &tid, has) {
            Some(true) => { *EDIT_PENDING.lock().unwrap() = None; self.kicad_edit(cx); }
            Some(false) => { *EDIT_PENDING.lock().unwrap() = None; *EDIT_TAB.lock().unwrap() = None; }
            None => {}
        }
    }
    pub(crate) fn kicad_edit(&mut self, cx: &mut Context<Self>) -> bool {
        let thread = &self.data["thread"];
        let viewer = &thread["viewer"];
        let key = crate::screens::viewer::shown_key(viewer);
        let tid = crate::screens::attach::raw_id(thread["id"].as_str().unwrap_or(""));
        let pid = crate::keys::project_of(&self.data, tid);
        let Some((project, root)) = edit_target(&self.data, &key, &pid).filter(|_| !test_holding()) else {
            // the viewer has no file yet (its first picture still on the way: bp-win after an update, 8 s to the key, 15 s to the paint):
            // the press is kept and acted on when the picture says what to edit (edit_pending_due); the button says Opening… meanwhile
            if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() { eprintln!("GPUI_LOG {}", json!({"kicad_edit": "pending", "key": key, "project": pid})); }
            *EDIT_PENDING.lock().unwrap() = Some((tid.to_string(), std::time::Instant::now()));
            self.ui_epoch += 1; cx.notify();
            return false
        };
        *EDIT_PENDING.lock().unwrap() = None;
        // the rail's Simulation asked for this Edit: the pane opens on the Simulator
        let sim = EDIT_TAB.lock().unwrap().take().is_some_and(|k| k == SIM);
        let open_now = viewer["open"].as_str().unwrap_or("");
        let open = crate::screens::viewer::pressed(open_now)
            .filter(|k| matches!(k.as_str(), "board" | "schematic" | "3d")) // i18n-ok: rail page kinds
            .or_else(|| (!open_now.is_empty()).then(|| open_now.to_string()))
            .unwrap_or_else(|| "board".to_string()); // i18n-ok: rail page kinds
        // never KiCad on a design page whose document the project does not have (#1347; the hub's `has`): KiCad would ask to create it
        if !sim && crate::screens::viewer::page_document(viewer, &open) == Some(false) {
            if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() { eprintln!("GPUI_LOG {}", json!({"kicad_edit": "refused", "page": open, "why": "no document"})); }
            return false
        }
        if let Some(p) = self.screens.kicad.clone() {
            let pr = p.read(cx);
            // Edit on this design's own pane: if KiCad no longer has the editor it wants (its frame closed and the pane missed the close), it
            // is asked for it again: never an Edit that does nothing
            if belongs_to(&pr.project, &pr.root, &pr.project_id, &key, &pid) { p.update(cx, |p, cx| if sim { p.open_simulation(cx) } else { p.edit_again(cx) }); return true }
        }
        let (hub, infos, hub_open, sends) = (self.tx.clone(), self.screens.kicad_infos.clone(), open_now.to_string(), self.screens.host_sends.clone());
        self.host_retry_watch(cx);
        let page = cx.new(move |cx| { let mut p = KicadPane::new(cx, project, root, pid, open, hub_open, hub); p.host_sends = sends; if let Some(v) = infos.values().find(|v| p.matches(v)).cloned() { p.info = v; } if sim { p.override_kind = Some(SIM.into()); } p });
        // the warm hand-over's end: the page drawn again without the viewer's picture under the pane
        cx.subscribe(&page, |_, _, _: &PaneShown, cx| cx.notify()).detach();
        // the editor's frame closed in KiCad (Ctrl+W, File > Close): back to View, Edit there again (a fresh frame on the next Edit)
        cx.subscribe(&page, |this, _, _: &PaneFrameClosed, cx| this.kicad_view(cx)).detach();
        cx.subscribe(&page, |this, _, e: &PaneKey, cx| {
            if e.0 == "ctrl-;" || e.0 == ";" { this.kcomment_open(None, cx); return }
            // the pane's "Open the KiCad app instead" (the editor could not be here): back to the viewer, and the hub starts KiCad on what it shows
            // the pane's Pop out (macOS full screen: KiCad cannot be over the pane there)
            if e.0 == "kicad-popout" { this.kicad_popout(cx); return }
            if e.0 == "kicad-app" { let open = crate::screens::viewer::kicad_open(&this.data["thread"]["viewer"]); this.kicad_view(cx); let _ = this.tx.try_send(open); return }
            // a management panel's key handed over by KiCad: the panel whose binding it is (the person's, from Settings)
            if let Some(id) = ["bom", "checks", "life", "fab"].into_iter().find(|id| this.local.key(&format!("mgmt-{id}")) == e.0) { crate::screens::mgmt::toggle(this, id, cx); }
            else { this.screens.kicad.as_ref().map(|p| p.update(cx, |p, cx| { p.press.push(e.0.clone()); cx.notify(); })); }
        }).detach();
        self.screens.kicad = Some(page); cx.notify(); true
    }
    /// The latest host push sent again when it is due (HostSends: refused, or its answer lost), from a timer of the app's own, so a
    /// pane dropped at View still has its hide sent again; one timer at a time, none while nothing waits.
    pub(crate) fn host_retry_watch(&mut self, cx: &mut Context<Self>) {
        if self.screens.host_sends.borrow().has_wake() { return } // one watch for the window, for as long as it lives
        let (tx, rx) = async_channel::bounded::<()>(1);
        self.screens.host_sends.borrow_mut().set_wake(tx);
        cx.spawn(async move |this, cx| loop {
            let Ok(step) = this.update(cx, |this, _| this.screens.host_sends.borrow_mut().watch_step(std::time::Instant::now())) else { break };
            match step {
                HostStep::Send(params) => {
                    eprintln!("GPUI_LOG {}", json!({"kicad_host_push": [params["x"], params["y"], params["width"], params["height"]], "visible": params["visible"], "raise": params["raise"], "retry": true, "at_ms": crate::screens::kcomment::epoch_ms() as u64}));
                    let Ok(()) = this.update(cx, |this, _| { let _ = this.tx.try_send(Command::Rpc("editor.kicad.host".into(), Secret(params), "kicad")); }) else { break };
                }
                HostStep::Look(d) => cx.background_executor().timer(d).await,
                // nothing waits: asleep until the next push wakes it (no timer runs meanwhile)
                HostStep::Idle => if rx.recv().await.is_err() { break },
            }
        }).detach();
    }
    /// View: back to Backplane's own viewer (KiCad stays open in the hub, warm for the next Edit).
    pub(crate) fn kicad_view(&mut self, cx: &mut Context<Self>) { if let Some(p) = self.screens.kicad.take() { p.update(cx, |p, _| p.detach()); } cx.notify(); }
    /// The editor for the thread's design when the person is editing it (None: the viewer draws the page).
    pub(crate) fn kicad_for(&self, thread: &Value, cx: &App) -> Option<Entity<KicadPane>> {
        let p = self.screens.kicad.as_ref()?;
        let key = crate::screens::viewer::shown_key(&thread["viewer"]);
        let tid = crate::screens::attach::raw_id(thread["id"].as_str().unwrap_or(""));
        let pid = crate::keys::project_of(&self.data, tid);
        let pr = p.read(cx);
        belongs_to(&pr.project, &pr.root, &pr.project_id, &key, &pid).then(|| p.clone())
    }
}

/// The editor's own row: a tab per editor (one click; Ctrl+Tab and Ctrl+Shift+Tab go round them), Full size, and Pop out / Pop back in.
/// `on` (the window it is drawn in): the main window's pane, or the editor's own window.
pub fn strip<V: 'static>(cx: &mut Context<V>, light: bool, pane: &Entity<KicadPane>, checks: &crate::screens::mgmt_checks::State, popped: bool, full: bool, screen: bool,
    act: impl Fn(&mut V, &str, &mut Window, &mut Context<V>) + Clone + 'static) -> AnyElement {
    let c = |n: &str| crate::theme::color(light, n);
    let at = pane.read(cx).tab();
    // the kinds KiCad says it has (the fork names them; an X title may come later than the frame), and those the X tree shows
    let have: Vec<String> = { let p = pane.read(cx); p.frames.iter().map(|f| f.kind.to_string())
        .chain(p.info["frames"].as_array().into_iter().flatten().filter_map(|f| f["kind"].as_str().map(String::from))).collect() };
    // a narrow pane (under 520 px: the narrowest is 380) packs the tabs and the buttons tighter, so all of them fit beside each other: at 380 px
    // Pop out was cut at the pane's edge
    let narrow = pane.read(cx).at_px.is_some_and(|b| f32::from(b.size.width) < 520.);
    let mut row = div().id("kicad-strip").flex().items_center().gap_1().px_2().h(crate::scale::tpx(30.)).flex_shrink_0().bg(c("panel")).border_b_1().border_color(c("rule"));
    for (kind, label) in TABS {
        // the symbol and footprint editors: a tab once KiCad has them, or while shown
        if matches!(*kind, "SYMBOL_EDITOR" | "FOOTPRINT_EDITOR" | "GERBVIEW" | "SIMULATOR") && !have.iter().any(|h| h == kind) && at != *kind { continue }
        let on = at == *kind; let k = kind.to_string(); let p = pane.clone();
        row = row.child(crate::ui::with_rect(div().id(SharedString::from(format!("kicad-tab-{kind}"))), { let k = kind.to_string(); move || format!("kicad-tab:{k}") }).when(narrow, |d| d.px_1()).when(!narrow, |d| d.px_2()).py(px(3.)).text_size(crate::scale::tpx(12.)).cursor_pointer()
            .text_color(c(if on { "text" } else { "dim" })).when(on, |d| d.border_b_2().border_color(c("accent"))).hover(|s| s.bg(c("raise")))
            .tooltip(crate::keys::tip(&crate::i18n::trs(label), "Ctrl+Tab", light))
            .child(crate::i18n::trs(label))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| { let k = k.clone(); p.update(cx, |p, cx| p.show_kind_by(&k, "click on the pane's tab strip", cx)); }));
    }
    // what stands between the tabs and the buttons never pushes the buttons (a job's bar coming up pushed Pop out past the pane's edge, and the
    // buttons jumped 66 px while it did). It is one line high and wraps what does not fit onto a second line that is clipped, so each thing is
    // shown whole or not at all (never a half-cut chip), in this order: a file conflict (the person's two answers), KiCad's long job, the checks
    // chip, the stock-KiCad note, and KiCad's notice, which takes what is left (80 px at least) and ends in an ellipsis
    let mut mid = div().id("kicad-strip-mid").flex().flex_wrap().items_center().content_start().gap_1().flex_1().min_w_0().h(crate::scale::tpx(30.)).pt(px(4.)).overflow_hidden();
    // a first item of no width: a wrapping row never moves its first item to the next line, so without it the chip, when alone, was clipped
    // instead of wrapped away whole
    mid = mid.child(div().w(px(0.)).h(px(18.)));
    // a file changed on disk over the person's unsaved edits: said here too (KiCad asks in its own infobar), with the same two answers
    if !pane.read(cx).conflicts.is_empty() {
        let names: Vec<String> = pane.read(cx).conflicts.iter().map(|f| std::path::Path::new(f).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).collect();
        let (pk, pr) = (pane.clone(), pane.clone());
        mid = mid.child(div().id("kicad-reload").ml_2().flex().items_center().gap_1().text_size(crate::scale::tpx(12.)).text_color(c("bad"))
            .child(crate::i18n::tf("gpui.c.kicad.changed", &[("files", names.join(", ").into())]))
            .child(div().id("kicad-keep").px_2().border_1().border_color(c("line")).bg(c("raise")).cursor_pointer().text_color(c("text")).child(crate::i18n::trs("Keep yours"))
                .on_click(move |_, _, cx| pk.update(cx, |p, cx| p.resolve("keep", cx))))
            .child(div().id("kicad-reload-btn").px_2().border_1().border_color(c("line")).bg(c("raise")).cursor_pointer().text_color(c("text")).child(crate::i18n::trs("Reload (lose your edits)"))
                .on_click(move |_, _, cx| pr.update(cx, |p, cx| p.resolve("reload", cx)))));
    }
    // KiCad's long job (loading, zone fill, DRC, plot): a slim bar here instead of KiCad's own progress window (bp.12)
    let jobs = if pane.read(cx).info["state"] == "ready" { pane.read(cx).jobs.clone() } else { Vec::new() };
    if let Some(j) = jobs.first() {
        let (id, f) = (j["id"].as_u64().unwrap_or(0), j["fraction"].as_f64().unwrap_or(-1.0));
        let title = j["title"].as_str().unwrap_or("").to_string(); let detail = j["detail"].as_str().unwrap_or("").to_string();
        let text = if detail.is_empty() { crate::i18n::trs(&title) } else { format!("{} · {detail}", crate::i18n::trs(&title)) };
        let w = 120.0f32; let p = pane.clone();
        mid = mid.child(crate::ui::with_rect(div().id("kicad-progress"), || "kicad:progress".to_string()).ml_2().flex().items_center().gap_1().text_size(crate::scale::tpx(11.)).text_color(c("dim"))
            .child(div().max_w(px(220.)).overflow_hidden().whitespace_nowrap().text_ellipsis().child(text))
            .child(div().w(px(w)).h(px(4.)).bg(c("line")).child(div().h_full().bg(c("accent")).w(px(if f >= 0.0 { w * f.min(1.0) as f32 } else { w * 0.3 }))))
            .when(jobs.len() > 1, |d| d.child(format!("+{}", jobs.len() - 1)))
            .when(j["cancellable"] == true, |d| d.child(div().id("kicad-progress-cancel").px_1().cursor_pointer().child("×").tooltip(crate::keys::tip(&crate::i18n::trs("Cancel"), "", light))
                .on_click(move |_, _, cx| p.update(cx, |p, _| p.cancel_job(id))))));
    }
    // the design's checks ("DRC 12 · ERC 3", coloured only when not clean): a press opens the Checks panel
    // a KiCad marker clicked with the Checks panel shut: the chip says that finding for a few seconds, and its press opens the panel on it
    if let Some((text, tone)) = crate::screens::mgmt_checks::chip_for(checks, &pane.read(cx).project) {
        let a = act.clone();
        let flash = crate::screens::mgmt_checks::chip_flash(checks);
        let chip = match &flash {
            Some((t, ..)) => crate::screens::mgmt_checks::chip_el(if t.is_empty() { &text } else { t }, "accent", light).border_color(c("accent")).bg(c("raise"))
                .max_w(px(260.)).overflow_hidden().whitespace_nowrap().text_ellipsis(),
            None => crate::screens::mgmt_checks::chip_el(&text, tone, light),
        };
        mid = mid.child(chip.on_click(cx.listener(move |v, _, w, cx| a(v, "checks", w, cx))));
        if let Some((_, left, true)) = flash {
            cx.spawn(async move |v, cx| { cx.background_executor().timer(left).await; let _ = v.update(cx, |_, cx| cx.notify()); }).detach();
        }
    }
    // stock KiCad in the pane (the system's own, taken in window by window): said, with the way to Backplane KiCad (Settings, CAD asks first)
    if !popped && stock_hint(&pane.read(cx).info, cfg!(windows)) {
        let a = act.clone(); let tip = crate::i18n::trs("This is stock KiCad: it can open in windows of its own, and agents cannot edit in it. Backplane KiCad opens in here.");
        mid = mid.child(crate::ui::with_rect(div().id("kicad-stock"), || "kicad:stock".to_string()).ml_2().px_2().py(px(3.)).flex_shrink_0().text_size(crate::scale::tpx(11.)).text_color(c("warn")).cursor_pointer().hover(|s| s.bg(c("raise")))
            .tooltip(crate::keys::tip(&tip, "", light)).child(crate::i18n::trs("Install Backplane KiCad"))
            .on_click(cx.listener(move |v, _, w, cx| a(v, "cad", w, cx))));
    }
    // KiCad asking something (its dialog up, on top) while the person types in Backplane: one line, so it is seen without taking their
    // keyboard; a click on the dialog answers it (no press here: GNOME gives another program's window the keyboard only on the person's
    // own click, never on ours)
    if let Some(text) = { let p = pane.read(cx); asking(&p.dialogs_open, p.keys) } {
        mid = mid.child(crate::ui::with_rect(div().id("kicad-asking"), || "kicad:asking".to_string()).ml_2().px_1().flex_shrink().min_w(px(80.)).max_w(px(300.)).overflow_hidden().whitespace_nowrap().text_ellipsis()
            .text_size(crate::scale::tpx(11.)).text_color(c("warn")).child(text.clone()).tooltip(crate::keys::tip(&text, "", light)));
    }
    // KiCad's notice (embed-notice): one line, readable after the editor is up, dismissed with its ×; never a modal
    if let Some((id, text)) = open_notice(&pane.read(cx).info) {
        let p = pane.clone();
        mid = mid.child(crate::ui::with_rect(div().id("kicad-notice"), || "kicad:notice".to_string()).ml_2().flex().items_center().gap_1().flex_grow().flex_basis(px(80.)).min_w(px(80.)).text_size(crate::scale::tpx(11.)).text_color(c("dim"))
            .child(div().flex_1().min_w_0().max_w(px(360.)).overflow_hidden().whitespace_nowrap().text_ellipsis().child(text.clone()))
            .tooltip(crate::keys::tip(&text, "", light))
            .child(crate::ui::with_rect(div().id("kicad-notice-x"), || "kicad:notice-dismiss".to_string()).px_1().cursor_pointer().child("×").tooltip(crate::keys::tip(&crate::i18n::trs("Dismiss"), "", light))
                .on_click(move |_, _, cx| p.update(cx, |p, cx| { p.dismiss_notice(id); cx.notify(); }))));
    }
    row = row.child(mid);
    let mk = |id: &'static str, label: String, tip: String, a: &'static str, cx: &mut Context<V>| {
        let act = act.clone();
        // its rect under `kicad:<action>` (kicad:full, kicad:screen, kicad:popout, kicad:popin): a driver clicks the button itself, never a place
        // measured once (real_use's fixed offset for Full size landed on Full screen once it was added beside it)
        crate::ui::with_rect(div().id(id), move || format!("kicad:{a}")).when(narrow, |d| d.px_1()).when(!narrow, |d| d.px_2()).py(px(3.)).text_size(crate::scale::tpx(12.)).border_1().border_color(c("line")).bg(c("raise")).cursor_pointer().hover(|s| s.bg(c("panel")))
            .tooltip(crate::keys::tip(&tip, "", light)).child(label)
            .on_click(cx.listener(move |this, _, window, cx| act(this, a, window, cx)))
    };
    if !popped && !screen { row = row.child(mk("kicad-full", crate::i18n::trs(if full { "Exit full size" } else { "Full size" }), crate::i18n::trs("The editor takes the sidebar's and the thread's room"), "full", cx)); }
    // full screen: the editor is the whole window, with only this strip above it
    if !popped { row = row.child(mk("kicad-screen", crate::i18n::trs(if screen { "Exit full screen" } else { "Full screen" }), crate::i18n::trs("The editor takes the whole window; only this row stays"), "screen", cx)); }
    if !screen { row = row.child(mk("kicad-pop", crate::i18n::trs(if popped { "Pop back in" } else { "Pop out" }), crate::i18n::trs(if popped { "Back into Backplane's pane, as it is" } else { "The editor in a window of its own, as it is" }), if popped { "popin" } else { "popout" }, cx)); }
    row.into_any_element()
}

/// The editor in a window of its own (Pop out): the strip and the same pane (the same KiCad, nothing restarted). Closing it pops it back in.
pub struct KicadWindow { checks: crate::screens::mgmt_checks::State, pub pane: Entity<KicadPane>, pub app: WeakEntity<crate::Backplane>, pub light: bool }
impl Render for KicadWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // the pane's own idea of the theme (Backplane's render sets it; Backplane is never read from here, it may be the one updating)
        let light = self.pane.read(cx).light; self.light = light;
        let _ = window;
        let strip = strip(cx, light, &self.pane, &self.checks, true, false, false, |this: &mut KicadWindow, a, window, cx| {
            if a == "popin" { if let Some(app) = this.app.upgrade() { app.update(cx, |app, cx| app.kicad_popin(cx)); } window.remove_window(); }
        });
        div().size_full().flex().flex_col().bg(crate::theme::color(light, "bg")).child(strip).child(div().flex_1().min_h_0().child(self.pane.clone()))
    }
}

impl crate::Backplane {
    /// Pop out: the editor into a window of its own; the pane says where it went.
    pub(crate) fn kicad_popout(&mut self, cx: &mut Context<Self>) {
        let Some(pane) = self.screens.kicad.clone() else { return };
        if self.screens.kicad_window.is_some() { return }
        let (app, light) = (cx.entity().downgrade(), self.light());
        let checks = self.screens.mgmt.checks.clone();
        let title = std::path::Path::new(&pane.read(cx).project).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let opts = WindowOptions { window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1400.), px(900.)), cx))),
            titlebar: Some(TitlebarOptions { title: Some(crate::streamer::shown(&format!("{title} — KiCad · Backplane")).into()), ..Default::default() }), ..Default::default() };
        match cx.open_window(opts, move |window, cx| {
            let app2 = app.clone();
            window.on_window_should_close(cx, move |window, cx| { let popped = top_of(window).unwrap_or(0); if let Some(a) = app2.upgrade() { a.update(cx, |a, cx| a.kicad_popin_closing(popped, cx)); } true });
            cx.new(|_| KicadWindow { pane, app, light, checks })
        }) {
            // the popped-out window is the person's now: the editor in it gets the keys as on Edit (the window itself is shown
            // active by open_window)
            Ok(h) => { self.screens.kicad_window = Some(h.into()); self.screens.kicad_full = false; self.screens.kicad_screen = false; }
            Err(e) => { self.note = format!("{} {e}", crate::i18n::trs("The editor's window could not open:")); }
        }
        cx.notify();
    }
    /// Pop back in: the pane draws the editor again (its window, if still open, is closed by its own button or its title bar).
    pub(crate) fn kicad_popin(&mut self, cx: &mut Context<Self>) {
        // KiCad back into the main window first (the editor's window is about to go, and its children with it); the
        // move asks for the keys to be handed to the editor once it is placed there (rehome)
        if let Some(p) = self.screens.kicad.clone() { p.update(cx, |p, _| { let h = p.home(); if h != 0 { p.rehome(h); } }); }
        self.screens.kicad_window = None; cx.notify();
    }
    /// The popped window closed by its own X: KiCad is moved back now, before the window and its children go, but the
    /// keys only once Windows has activated Backplane's window after the close, which would otherwise take them back
    /// (bp9 Nov 15: handed in inside should_close, F2 then did nothing)
    pub(crate) fn kicad_popin_closing(&mut self, popped: u64, cx: &mut Context<Self>) {
        let Some(p) = self.screens.kicad.clone() else { self.screens.kicad_window = None; cx.notify(); return };
        // KiCad back now (its frames are children of the window about to go); the keys once that window is gone and
        // Backplane's is the foreground (the pane's tick), not before: the activation would take them back
        p.update(cx, |p, _| { let h = p.home(); if h != 0 { p.rehome(h); } p.want_keys = false; p.await_close = Some((popped, std::time::Instant::now())); });
        self.screens.kicad_window = None; cx.notify();
    }
    /// The strip's buttons in the main window.
    pub(crate) fn kicad_strip_act(&mut self, a: &str, cx: &mut Context<Self>) {
        match a { "full" => { self.screens.kicad_full = !self.screens.kicad_full; if let Some(p) = self.screens.kicad.clone() { p.update(cx, |p, _| { p.want_keys = true; p.placed = None; }); } cx.notify(); } "popout" => self.kicad_popout(cx), "checks" => { self.screens.kicad_screen = false; crate::screens::mgmt_checks::open_panel(self, cx) }
            // the strip's Install Backplane KiCad (a stock KiCad in the pane): Settings, CAD, where the install says what it downloads and asks
            "screen" => { self.screens.kicad_screen = !self.screens.kicad_screen; if let Some(p) = self.screens.kicad.clone() { p.update(cx, |p, _| { p.want_keys = true; p.placed = None; }); } cx.notify(); }
            "cad" => { self.screens.kicad_screen = false; crate::screens::settings::go_page(self, "cad"); cx.notify(); }
            _ => {} }
    }
}

impl crate::Backplane {
    /// A comment from KiCad's own dialog: posted once (by its seq) into the open thread, when the window shows this project's editor.
    pub(crate) fn kicad_comment(&mut self, text: &str, cx: &mut Context<Self>) {
        let Ok(v) = serde_json::from_str::<Value>(text) else { return };
        let Some(p) = self.screens.kicad.clone() else { return };
        if !p.read(cx).matches(&v) { return }
        let key = format!("{}#{}", v["project"].as_str().unwrap_or(""), v["seq"]);
        if !self.screens.kicad_comments_seen.insert(key) { return }
        let th = self.data["thread"]["id"].as_str().unwrap_or("").rsplit('|').next().unwrap_or("").to_string();
        if th.is_empty() { self.note = crate::i18n::trs("A comment from KiCad needs an open thread."); cx.notify(); return }
        // the hub words it (kicad/comment.rs `text`), as it does for the comment window and a phone's viewer
        let mut p = v.clone(); p["thread"] = json!(th);
        let _ = self.tx.try_send(Command::Rpc("kicad.comment.post".into(), Secret(p), "kicad"));
        cx.notify();
    }
}

/// Where the person was when a reserved key pressed in KiCad opened one of Backplane's overlays (the palette, the terminal): the thread,
/// what its viewer showed, the project page if one was over it. The keyboard goes back to KiCad when the overlay is dismissed with this
/// unchanged (Escape, a click away, an action that leaves the editor on show), and stays with Backplane when the choice went elsewhere.
#[derive(Clone, Debug, PartialEq)]
pub struct Loan { pub thread: String, pub view: String, pub page: Option<String> }
/// Whether the keyboard goes back to KiCad: the same place as when it was lent, with the editor on show.
pub fn loan_back(lent: &Loan, now: &Loan, editor_shown: bool) -> bool { editor_shown && lent == now }

impl crate::Backplane {
    fn kicad_place(&self) -> Loan {
        let v = &self.data["thread"]["viewer"];
        Loan { thread: self.selected.clone(), view: format!("{}|{}", v["open"].as_str().unwrap_or(""), crate::screens::viewer::shown_key(v)), page: self.project_page.clone() }
    }
    /// An overlay opens: when a reserved key in KiCad opened it, where the person was is kept (`kicad_return` gives the keyboard back).
    pub fn kicad_lend(&mut self, cx: &mut Context<Self>) {
        let lent = self.screens.kicad.as_ref().is_some_and(|p| p.read(cx).lent_recently());
        self.screens.kicad_loan = lent.then(|| self.kicad_place());
        // while the overlay is up the editor takes no input: X hands a key to the window under the pointer when it is inside the focus
        // window, so with the pointer resting over KiCad every key typed into the palette (and its Escape) went to KiCad (lend e2e)
        if lent { if let Some(p) = self.screens.kicad.clone() { p.update(cx, |p, _| p.hold_keys(true)); } }
    }
    /// An overlay's choice wants the keyboard itself: the loan ends, the editor takes the pointer again, Backplane keeps the keyboard.
    pub fn kicad_release(&mut self, cx: &mut Context<Self>) {
        if self.screens.kicad_loan.take().is_none() { return }
        if let Some(p) = self.screens.kicad.clone() { p.update(cx, |p, _| p.release_hold()); }
    }
    /// An overlay was dismissed: the keyboard back to KiCad when nothing moved (the platform's own hand-over: `give_keys`).
    pub fn kicad_return(&mut self, cx: &mut Context<Self>) {
        let Some(lent) = self.screens.kicad_loan.take() else { return };
        let now = self.kicad_place();
        let shown = crate::screens::mgmt::shows(self.data["thread"]["viewer"]["open"].as_str().unwrap_or("")) && self.project_page.is_none();
        let back = loan_back(&lent, &now, shown);
        eprintln!("GPUI_LOG {}", json!({"kicad_loan_back": back, "moved": lent != now}));
        if let Some(p) = self.screens.kicad.clone() { p.update(cx, |p, cx| if back { p.give_keys(cx) } else { p.release_hold() }); }
    }
}
