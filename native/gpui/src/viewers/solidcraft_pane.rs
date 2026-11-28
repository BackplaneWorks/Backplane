//! Embedded SolidCraft CAD pane (replaces the desktop STEP/GLB Mech viewer): the hub runs SolidCraft
//! per project root (`solidcraft/embed.rs`) and publishes `editor.solidcraft.<digest(root)>`.
//! This pane creates a 24-bit X11 container window (`native_child.rs`), attaches SolidCraft to it via
//! `editor.solidcraft.attach`, adopts SolidCraft's winit/egui top-level by PID, keeps the container
//! alive across file switches (`editor.solidcraft.open` -> `--control` `doc.open`), and cuts holes
//! for any GPUI overlays drawn above the pane so there is zero Z-order bleed and zero flicker.
use super::{kicad_pane, native_child as nc};
use crate::{hub::Command, i18n::trs, screens::settings::Secret, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// All CAD/3D file extensions handled by the embedded SolidCraft pane.
pub const CAD_EXTS: &[&str] = &[
    ".step",
    ".stp",
    ".sc",
    ".solidcraft",
    ".iges",
    ".igs",
    ".stl",
    ".3mf",
    ".obj",
    ".glb",
];

/// Whether `path` has a CAD/mesh extension supported by SolidCraft (case-insensitive).
pub fn is_cad_file(path: &str) -> bool {
    let lower = path.trim().to_ascii_lowercase();
    let name = lower.rsplit(['/', '\\']).next().unwrap_or("");
    CAD_EXTS.iter().any(|ext| name.len() > ext.len() && name.ends_with(ext))
}

/// Normalize path separators and strip trailing slashes for consistent matching.
pub fn norm_path(s: &str) -> String {
    let mut n = s.trim().replace('\\', "/");
    while n.len() > 1 && n.ends_with('/') && !(n.len() == 3 && n.as_bytes()[1] == b':') {
        n.pop();
    }
    if n.len() >= 2 && n.as_bytes()[0].is_ascii_alphabetic() && n.as_bytes()[1] == b':' {
        n.make_ascii_lowercase();
    }
    n
}

fn is_abs_path(s: &str) -> bool {
    s.starts_with('/')
        || s.starts_with('\\')
        || (s.len() >= 3
            && s.as_bytes()[0].is_ascii_alphabetic()
            && s.as_bytes()[1] == b':'
            && matches!(s.as_bytes()[2], b'/' | b'\\'))
}

/// Resolve `file` relative to `root` unless `file` is already empty or absolute.
pub fn resolve_file_path(root: &str, file: &str) -> String {
    let f = file.trim();
    if f.is_empty() {
        return String::new();
    }
    if is_abs_path(f) {
        return norm_path(f);
    }
    let r = norm_path(root);
    if r.is_empty() {
        return norm_path(f);
    }
    let rel = f.trim_start_matches(['/', '\\']).replace('\\', "/");
    format!("{r}/{rel}")
}

/// Extract normalized project-relative path from an absolute path under `root`.
pub fn new_part_rel(root: &str, file: &str) -> String {
    let r = norm_path(root);
    let f = norm_path(file);
    f.strip_prefix(&r)
        .map(|s| s.trim_start_matches(['/', '\\']).to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or(f)
}

/// Extract `(root, resolved_file)` for the Mech/CAD viewer from the thread and project list.
/// Returns `None` only when no project root can be determined.
pub fn mech_target(data: &Value, thread: &Value) -> Option<(String, String)> {
    let viewer = &thread["viewer"];
    let mut key = viewer["key"].as_str().unwrap_or("");
    let mut mech = &viewer["mech"];
    if crate::screens::viewer::is_tiled(viewer) {
        if let Some(tile_id) = mech_tile(viewer) {
            let row = &viewer["tiles"][tile_id];
            if let Some(rk) = row["key"].as_str().filter(|s| !s.is_empty()) {
                key = rk;
            }
            if row["mech"].is_object() {
                mech = &row["mech"];
            }
        }
    }
    let key_root = crate::viewers::kicad_pane::root_of(key).unwrap_or_default();
    let tid = crate::screens::attach::raw_id(thread["id"].as_str().unwrap_or(""));
    let pid = crate::keys::project_of(data, tid);
    let proj_root = crate::screens::projects::find(data, &pid)
        .and_then(|p| p["root"].as_str())
        .map(str::to_string)
        .or_else(|| thread["project"]["root"].as_str().map(str::to_string))
        .unwrap_or_default();
    let root = if !key_root.is_empty() {
        norm_path(&key_root)
    } else {
        norm_path(&proj_root)
    };
    if root.is_empty() {
        return None;
    }
    let key_file = key
        .splitn(3, '|')
        .nth(2)
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.contains('|'))
        .unwrap_or("");
    let rel = mech["rel"].as_str().unwrap_or("").trim();
    let active_part = mech["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["on"] == true)
        .and_then(|p| p["value"].as_str())
        .unwrap_or("")
        .trim();
    let raw_file = if !key_file.is_empty() {
        key_file
    } else if !rel.is_empty() {
        rel
    } else {
        active_part
    };
    let file = resolve_file_path(&root, raw_file);
    Some((root, file))
}

/// Active `"mech"` tile id in docked/tiled layout, preferring maximized, then focused, then first.
pub fn mech_tile(viewer: &Value) -> Option<&str> {
    let tiles = viewer["tiles"].as_object()?;
    if let Some(id) = viewer["layout"]["maximized"].as_str() {
        return (tiles.get(id)?["kind"] == "mech").then_some(id);
    }
    if let Some(id) = viewer["layout"]["focus"].as_str() {
        if tiles.get(id).is_some_and(|t| t["kind"] == "mech") {
            return Some(id);
        }
    }
    tiles
        .iter()
        .find(|(_, t)| t["kind"] == "mech")
        .map(|(id, _)| id.as_str())
}

/// Pure status summary derived from hub `editor.solidcraft.<digest>` info and native child state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaneStatus {
    PoppedOut,
    Ready,
    NotInstalled,
    Installing { done: u64, total: u64 },
    Starting(String),
    Unresponsive(String),
    Failed(String),
    Stopped,
}

/// The header's Full screen / Pop out / Bring back: only with a SolidCraft window, in the pane or popped out.
pub fn window_tools(st: &PaneStatus) -> bool { matches!(st, PaneStatus::Ready | PaneStatus::PoppedOut) }

pub fn pane_status(info: &Value, has_child: bool, popped: bool) -> PaneStatus {
    if popped {
        return PaneStatus::PoppedOut;
    }
    match info["state"].as_str() {
        Some("installing") => PaneStatus::Installing {
            done: info["install_progress"]["done"].as_u64().unwrap_or(0),
            total: info["install_progress"]["total"].as_u64().unwrap_or(0),
        },
        Some("ready") => {
            if has_child {
                PaneStatus::Ready
            } else {
                PaneStatus::Starting(trs("Waiting for SolidCraft window…"))
            }
        }
        Some("starting") => {
            let reason = info["reason"].as_str().unwrap_or("").trim();
            let msg = if reason.is_empty() {
                trs("Starting SolidCraft…")
            } else {
                trs(reason)
            };
            PaneStatus::Starting(msg)
        }
        Some("unresponsive") => {
            let reason = info["reason"].as_str().unwrap_or("").trim();
            let msg = if reason.is_empty() {
                trs("SolidCraft is not responding.")
            } else {
                trs(reason)
            };
            PaneStatus::Unresponsive(msg)
        }
        Some("failed") => {
            let reason = info["reason"].as_str().unwrap_or("").trim();
            let msg = if reason.is_empty() {
                trs("SolidCraft could not start.")
            } else {
                /* i18n-ok: hub error detail appended */ format!("{} {reason}", trs("SolidCraft could not start:"))
            };
            PaneStatus::Failed(msg)
        }
        Some("stopped") | None => {
            if info["runtime"]["installed"] == false {
                PaneStatus::NotInstalled
            } else if info["runtime"]["installed"] == true
                && info["runtime"]["gui"].as_str().unwrap_or("").is_empty()
            {
                PaneStatus::Failed(trs("This computer has SolidCraft's command-line program, not its desktop window."))
            } else if info.is_null() {
                PaneStatus::Starting(trs("Starting SolidCraft…"))
            } else {
                PaneStatus::Stopped
            }
        }
        _ => PaneStatus::Starting(trs("Starting SolidCraft…")),
    }
}

/// True while the pane is popped out to `SolidCraftWindow` and still on the home toplevel
/// (before `rehome` moves it to the detached window), or when rendered on the home window.
pub fn popped_elsewhere(popped: bool, toplevel: u64, home: u64) -> bool {
    popped && (toplevel == 0 || toplevel == home)
}

/// Never map the X11 container until the pane is on screen, not popped out, has a valid physical rect,
/// and has already adopted a child window — prevents black-frame flash while SolidCraft launches.
pub fn container_visible(in_page: bool, popped: bool, has_rect: bool, has_child: bool) -> bool {
    in_page && !popped && has_rect && has_child
}

/// Intersection of two physical rectangles, or `None` when empty/disjoint.
pub fn rect_intersect(a: nc::R, b: nc::R) -> Option<nc::R> {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.w).min(b.x + b.w);
    let y1 = (a.y + a.h).min(b.y + b.h);
    (x0 < x1 && y0 < y1).then_some(nc::R {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    })
}

/// Two-phase placement plan around `Window::present()` so the native child never paints over stale host pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlacePlan {
    HideNow,
    Immediate,
    SyncResize { pre_clip: Vec<nc::R> },
}

/// Choose how to synchronize a container transition from `prev` to `next` (with host overlays `over`) around `present()`.
pub fn plan_placement(prev: Option<nc::R>, next: Option<nc::R>, over: &[nc::R]) -> PlacePlan {
    match (prev, next) {
        (_, None) => PlacePlan::HideNow,
        (None, Some(_)) => PlacePlan::Immediate,
        (Some(m), Some(r)) if m == r => PlacePlan::Immediate,
        (Some(m), Some(r)) => {
            let Some(inter) = rect_intersect(m, r) else {
                return PlacePlan::SyncResize { pre_clip: Vec::new() };
            };
            let dx = inter.x - m.x;
            let dy = inter.y - m.y;
            let pre_clip: Vec<nc::R> = nc::cut(inter, over)
                .into_iter()
                .map(|c| nc::R {
                    x: c.x + dx,
                    y: c.y + dy,
                    w: c.w,
                    h: c.h,
                })
                .collect();
            PlacePlan::SyncResize { pre_clip }
        }
    }
}

/// What the not-installed screen says it would install: the hub's offer (info `solidcraft.runtime`), its version and its size in the words and
/// the number Settings, CAD and setup use ("about 218 MB": MiB, rounded); with no size known, no number.
pub fn install_words(runtime: &Value) -> String {
    let offer = &runtime["offer"];
    match offer["size"].as_u64().filter(|b| *b > 0) {
        Some(b) => crate::i18n::tf("gpui.c.solidcraft.install.desc", &[("version", offer["version"].as_str().unwrap_or("").into()), ("mb", ((b + (1 << 19)) >> 20).to_string().into())]),
        None => crate::i18n::t("gpui.c.solidcraft.install.desc.nosize"),
    }
}
/// Result of deciding how to reclaim focus from the embedded SolidCraft child window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReclaimDecision {
    pub reclaim_native: bool,
    pub keys_changed: bool,
    pub focus_target: u64,
}

/// Decide whether and where to return native keyboard focus when Backplane takes input back.
pub fn decide_reclaim(
    keys: &mut bool,
    press_in: &mut bool,
    live_focus: bool,
    pane_toplevel: u64,
    target_toplevel: Option<u64>,
) -> ReclaimDecision {
    *press_in = false;
    let had_keys = std::mem::take(keys);
    let reclaim_native = had_keys || live_focus;
    let focus_target = if reclaim_native {
        target_toplevel.filter(|t| *t != 0).unwrap_or(pane_toplevel)
    } else {
        0
    };
    ReclaimDecision {
        reclaim_native,
        keys_changed: had_keys,
        focus_target,
    }
}

/// How the user clicked into the embedded SolidCraft pane during a tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickClickIn {
    None,
    GrabClick,
    AlreadyFocused,
}

/// Native focus action to perform at the end of a `SolidCraftPane::tick()` pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickFocusAction {
    None,
    FocusChild,
    ReclaimToHost,
}

/// Pure focus state transition computed by `decide_tick_focus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TickFocusDecision {
    pub next_keys: bool,
    pub next_press_in: bool,
    pub action: TickFocusAction,
}

/// Decide SolidCraft keyboard ownership on a tick without allowing unsolicited focus grabs.
#[allow(clippy::too_many_arguments)]
pub fn decide_tick_focus(
    in_page: bool,
    has_child: bool,
    keys: bool,
    press_in: bool,
    has_focus: bool,
    in_dialog: bool,
    button_over: Option<bool>,
    pointer_elsewhere: bool,
    click_in: TickClickIn,
    escaped_in_event: bool,
) -> TickFocusDecision {
    if !in_page {
        return TickFocusDecision {
            next_keys: false,
            next_press_in: false,
            action: if keys || has_focus {
                TickFocusAction::ReclaimToHost
            } else {
                TickFocusAction::None
            },
        };
    }
    if escaped_in_event {
        return TickFocusDecision {
            next_keys: false,
            next_press_in: false,
            action: TickFocusAction::ReclaimToHost,
        };
    }
    if click_in != TickClickIn::None && has_child {
        return TickFocusDecision {
            next_keys: true,
            next_press_in: matches!(button_over, Some(true)),
            action: if click_in == TickClickIn::GrabClick {
                TickFocusAction::FocusChild
            } else {
                TickFocusAction::None
            },
        };
    }
    if in_dialog {
        return TickFocusDecision {
            next_keys: true,
            next_press_in: false,
            action: TickFocusAction::None,
        };
    }
    if pointer_elsewhere {
        return TickFocusDecision {
            next_keys: keys && has_focus,
            next_press_in: false,
            action: if !keys && has_focus {
                TickFocusAction::ReclaimToHost
            } else {
                TickFocusAction::None
            },
        };
    }
    match button_over {
        Some(true) if has_child => TickFocusDecision {
            next_keys: true,
            next_press_in: true,
            action: if has_focus {
                TickFocusAction::None
            } else {
                TickFocusAction::FocusChild
            },
        },
        Some(false) if press_in => TickFocusDecision {
            next_keys: keys && has_focus,
            next_press_in: true,
            action: TickFocusAction::None,
        },
        Some(false) => TickFocusDecision {
            next_keys: false,
            next_press_in: false,
            action: if keys || has_focus {
                TickFocusAction::ReclaimToHost
            } else {
                TickFocusAction::None
            },
        },
        _ => {
            if !keys && has_focus {
                TickFocusDecision {
                    next_keys: false,
                    next_press_in: false,
                    action: TickFocusAction::ReclaimToHost,
                }
            } else {
                TickFocusDecision {
                    next_keys: keys && has_focus,
                    next_press_in: false,
                    action: TickFocusAction::None,
                }
            }
        }
    }
}

/// Whether `editor.solidcraft.open` should be sent for live file-follow without respawning.
pub fn needs_file_open(
    attached: bool,
    state: &str,
    wanted_file: &str,
    sent_file: &str,
    hub_file: &str,
) -> bool {
    if !attached || state != "ready" || wanted_file.is_empty() {
        return false;
    }
    let want = norm_path(wanted_file);
    want != norm_path(sent_file) && want != norm_path(hub_file)
}

fn top_of(window: &Window) -> Option<u64> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle as R};
    HasWindowHandle::window_handle(window)
        .ok()
        .map(|h| h.as_raw())
        .and_then(|h| match h {
            R::Xcb(x) => Some(x.window.get() as u64),
            R::Xlib(x) => Some(x.window as u64),
            R::Win32(w) => Some(w.hwnd.get() as usize as u64),
            #[cfg(target_os = "macos")]
            R::AppKit(a) => nc::window_number(a.ns_view.as_ptr()),
            _ => None,
        })
}

// Kept container pool (zero respawn & zero flicker across tab/thread switches) ------------------

struct Kept {
    toplevel: u64,
    root: String,
    container: u64,
    frames: Vec<nc::Frame>,
    rect: Option<nc::R>,
}

static KEPT: Mutex<Vec<Kept>> = Mutex::new(Vec::new());
const KEEP_CAP: usize = 3;

fn keep(k: Kept) -> Vec<Kept> {
    let mut g = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    let mut dropped = Vec::new();
    g.retain(|x| {
        if x.toplevel == k.toplevel && norm_path(&x.root) == norm_path(&k.root) {
            if x.container != k.container {
                nc::destroy(x.container);
            }
            false
        } else {
            true
        }
    });
    g.push(k);
    while g.len() > KEEP_CAP {
        dropped.push(g.remove(0));
    }
    dropped
}

fn take_kept(toplevel: u64, root: &str) -> Option<Kept> {
    let mut g = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    let idx = g
        .iter()
        .position(|x| x.toplevel == toplevel && norm_path(&x.root) == norm_path(root))?;
    Some(g.remove(idx))
}

/// Release all held child windows before the GPUI process exits.
pub fn release_all() {
    let kept: Vec<Kept> = std::mem::take(&mut *KEPT.lock().unwrap_or_else(|e| e.into_inner()));
    for k in kept {
        nc::release_holders(Some(k.container));
    }
    nc::release_holders(None);
}

#[derive(Clone, Debug)]
pub enum SolidCraftEvent {
    Popout,
    BringBack,
    Escape,
    /// Not installed: show the part in Backplane's own viewer instead; true also makes it the default (setting `viewer.mech`)
    ViewNative(bool),
}

/// Compute the next SolidCraft fullscreen flag given current flag, desired toggle/set, and
/// whether SolidCraft is actively embedded (visible, local, not popped out).
pub fn next_solidcraft_full(current: bool, want: Option<bool>, active: bool) -> bool {
    if !active {
        return false;
    }
    want.unwrap_or(!current)
}

/// Embedded SolidCraft GPUI pane.
pub struct SolidCraftPane {
    pub root: String,
    pub project_id: String,
    pub file: String,
    sent_file: String,
    hub: tokio::sync::mpsc::Sender<Command>,
    pub container: Option<u64>,
    pub toplevel: u64,
    pub home: u64,
    pub attached: bool,
    pub in_page: bool,
    pub popped: bool,
    /// The not-installed screen's "Always view parts in Backplane" tick
    pub native_always: bool,
    pub info: Value,
    pub frames: Vec<nc::Frame>,
    marker: Arc<Mutex<Option<HitboxId>>>,
    end_marker: Arc<Mutex<Option<HitboxId>>>,
    placed: Option<(Option<nc::R>, Option<u64>, Vec<nc::R>)>,
    last_place: Option<(u64, nc::R, Option<u64>)>,
    home_rect: Option<nc::R>,
    bg_applied: Option<(u64, Option<u64>, u32)>,
    last_root: (i32, i32),
    place_seq: u64,
    screen_key: Option<(u64, u64)>,
    focus: FocusHandle,
    pub keys: bool,
    press_in: bool,
    press: Vec<String>,
    pub light: bool,
    at_px: Option<Bounds<Pixels>>,
    ticking: bool,
    adopt_started: Option<Instant>,
    create_on_attach: bool,
    awaiting_created: bool,
}

impl EventEmitter<SolidCraftEvent> for SolidCraftPane {}

impl SolidCraftPane {
    pub fn new(
        cx: &mut Context<Self>,
        root: String,
        project_id: String,
        file: String,
        hub: tokio::sync::mpsc::Sender<Command>,
    ) -> Self {
        Self {
            root: norm_path(&root),
            project_id,
            file: resolve_file_path(&root, &file),
            sent_file: String::new(),
            hub,
            container: None,
            toplevel: 0,
            home: 0,
            attached: false,
            in_page: true,
            popped: false,
            native_always: false,
            info: Value::Null,
            frames: Vec::new(),
            marker: Arc::default(),
            end_marker: Arc::default(),
            placed: None,
            last_place: None,
            home_rect: None,
            bg_applied: None,
            last_root: (i32::MIN, 0),
            place_seq: 0,
            screen_key: None,
            focus: cx.focus_handle(),
            keys: false,
            press_in: false,
            press: Vec::new(),
            light: false,
            at_px: None,
            ticking: false,
            adopt_started: None,
            create_on_attach: false,
            awaiting_created: false,
        }
    }

    pub fn home(&self) -> u64 {
        self.home
    }

    pub fn matches(&self, info: &Value) -> bool {
        let ir = info["root"].as_str().unwrap_or("");
        let ip = info["project"].as_str().unwrap_or("");
        (!ir.is_empty() && norm_path(ir) == norm_path(&self.root))
            || (!ip.is_empty() && norm_path(ip) == norm_path(&self.root))
    }

    pub fn child_window(&self) -> Option<u64> {
        self.frames.first().map(|f| f.window)
    }

    pub fn status(&self) -> PaneStatus {
        pane_status(
            &self.info,
            self.child_window().is_some(),
            popped_elsewhere(self.popped, self.toplevel, self.home),
        )
    }

    /// Whether SolidCraft's window is there to be made full screen, popped out or brought back (the Mech header's window buttons):
    /// not on the not-installed, installing, starting or could-not-start screens.
    pub fn has_window(&self) -> bool { window_tools(&self.status()) }

    pub fn settled(&self) -> Option<&'static str> {
        match self.status() {
            PaneStatus::Ready | PaneStatus::PoppedOut => Some("ready"),
            PaneStatus::Failed(_) => Some("error"),
            PaneStatus::NotInstalled | PaneStatus::Stopped => Some("empty"),
            PaneStatus::Installing { .. } | PaneStatus::Starting(_) | PaneStatus::Unresponsive(_) => {
                None
            }
        }
    }

    fn rpc(&self, method: &str, params: Value) {
        let _ = self
            .hub
            .try_send(Command::Rpc(method.to_string(), Secret(params), "solidcraft"));
    }

    pub fn set_light(&mut self, light: bool, cx: &mut Context<Self>) {
        if self.light != light {
            self.light = light;
            self.sync_bg_pixel();
            cx.notify();
        }
    }

    fn pane_bg_rgb(&self) -> u32 {
        if self.light { 0xf5f4ef } else { 0x282b35 }
    }

    fn sync_bg_pixel(&mut self) {
        let Some(c) = self.container else { return };
        let child = self.child_window();
        let rgb = self.pane_bg_rgb();
        let key = (c, child, rgb);
        if self.bg_applied == Some(key) {
            return;
        }
        nc::set_bg_pixel(c, rgb);
        if let Some(w) = child {
            nc::set_bg_pixel(w, rgb);
        }
        self.bg_applied = Some(key);
    }

    pub fn reclaim_focus(&mut self, target_top: Option<u64>, hand_back: bool) -> bool {
        let live = self.child_window().is_some() && self.container.is_some_and(nc::has_focus);
        let d = decide_reclaim(
            &mut self.keys,
            &mut self.press_in,
            live,
            self.toplevel,
            target_top,
        );
        if d.reclaim_native {
            if self.toplevel != 0 {
                nc::grab_escape(self.toplevel, false);
            }
            if let Some(c) = self.container {
                nc::grab_click(c, true);
            }
            nc::note_pane_keys(false);
            if d.focus_target != 0 {
                if hand_back {
                    nc::hand_back(d.focus_target);
                } else {
                    nc::focus(d.focus_target);
                }
            }
        }
        d.keys_changed
    }

    pub fn lend_focus(&mut self) {
        self.reclaim_focus(None, false);
    }

    pub fn lend_focus_to(&mut self, target_top: Option<u64>, cx: &mut Context<Self>) {
        if self.reclaim_focus(target_top, false) {
            cx.notify();
        }
    }

    fn on_backplane_click(&mut self, target_top: Option<u64>, cx: &mut Context<Self>) {
        self.lend_focus_to(target_top, cx);
    }

    pub fn set_in_page(&mut self, in_page: bool, cx: &mut Context<Self>) {
        if self.in_page == in_page {
            return;
        }
        self.in_page = in_page;
        if !in_page {
            self.place_seq = self.place_seq.wrapping_add(1);
            if let Some((t, c)) = self.screen_key.take() {
                nc::note_pane_on_screen(t, c, false);
            }
            self.reclaim_focus(None, false);
            if let Some(c) = self.container {
                nc::place(c, None, None, None, &mut self.last_root);
                nc::grab_escape(self.toplevel, false);
                nc::grab_click(c, true);
            }
            self.placed = None;
        }
        cx.notify();
    }

    pub fn set_info(&mut self, info: Value, cx: &mut Context<Self>) {
        if info == self.info {
            return;
        }
        let prev_pid = self.info["pid"].as_u64().unwrap_or(0);
        let new_pid = info["pid"].as_u64().unwrap_or(0);
        if new_pid != prev_pid {
            self.frames.clear();
            self.placed = None;
            self.last_place = None;
            self.adopt_started = (new_pid > 0).then(Instant::now);
            #[cfg(target_os = "linux")]
            nc::told_reset();
        }
        self.info = info;
        if self.info["state"].as_str() == Some("ready") {
            if self.awaiting_created {
                if let Some(f) = self.info["file"].as_str().filter(|s| !s.is_empty()) {
                    let nf = norm_path(f);
                    if nf != norm_path(&self.file) {
                        self.file = nf.clone();
                        self.sent_file = nf;
                        self.awaiting_created = false;
                    }
                }
            }
            if std::mem::take(&mut self.create_on_attach) {
                self.create_new_part(cx);
            }
        }
        if matches!(
            self.info["state"].as_str(),
            Some("ready" | "starting")
        ) {
            self.sync_open_file();
        }
        cx.notify();
    }

    pub fn create_new_part(&mut self, cx: &mut Context<Self>) {
        match self.info["state"].as_str() {
            Some("ready") => {
                self.create_on_attach = false;
                self.awaiting_created = true;
                self.rpc(
                    "editor.solidcraft.open",
                    json!({"project": self.root, "create": true}),
                );
            }
            Some("starting") => {
                self.create_on_attach = true;
            }
            _ => {
                self.create_on_attach = true;
                if self.container.is_some() {
                    self.reattach();
                }
            }
        }
        cx.notify();
    }

    pub fn open_file(&mut self, file: &str, cx: &mut Context<Self>) {
        let resolved = resolve_file_path(&self.root, file);
        if resolved.is_empty() || norm_path(&resolved) == norm_path(&self.file) {
            return;
        }
        self.file = resolved;
        if matches!(self.info["state"].as_str(), Some("stopped" | "failed")) {
            self.reattach();
        } else {
            self.sync_open_file();
        }
        cx.notify();
    }

    fn sync_open_file(&mut self) {
        let state = self.info["state"].as_str().unwrap_or("");
        let hub_file = self.info["file"].as_str().unwrap_or("");
        if needs_file_open(self.attached, state, &self.file, &self.sent_file, hub_file) {
            self.sent_file = self.file.clone();
            self.rpc(
                "editor.solidcraft.open",
                json!({"project": self.root, "file": self.file}),
            );
        }
    }

    pub fn install(&mut self, cx: &mut Context<Self>) {
        self.rpc("editor.solidcraft.install", json!({"consent": true}));
        cx.notify();
    }

    pub fn close_session(&mut self, cx: &mut Context<Self>) {
        self.rpc("editor.solidcraft.close", json!({"project": self.root}));
        cx.notify();
    }

    pub fn reattach(&mut self) {
        let Some(c) = self.container else {
            return;
        };
        let display = std::env::var("DISPLAY").unwrap_or_default();
        let (w, h) = self
            .at_px
            .map(|b| (f32::from(b.size.width).round() as i32, f32::from(b.size.height).round() as i32))
            .unwrap_or((nc::INIT_CONTAINER_W, nc::INIT_CONTAINER_H));
        let create = std::mem::take(&mut self.create_on_attach);
        let file = if create {
            self.awaiting_created = true;
            String::new()
        } else {
            self.file.clone()
        };
        self.sent_file = file.clone();
        let mut params = json!({
            "project": self.root,
            "root": self.root,
            "file": file,
            "container": c,
            "toplevel": self.toplevel,
            "display": display,
            "width": w.max(100),
            "height": h.max(100),
            "theme": if self.light { "light" } else { "dark" },
        });
        if create {
            params["create"] = json!(true);
        }
        self.rpc("editor.solidcraft.attach", params);
    }

    pub fn rehome(&mut self, new_top: u64) {
        let Some(old) = self.container else {
            return;
        };
        if new_top == 0 || new_top == self.toplevel {
            return;
        }
        let Some(new_c) = nc::container_for(new_top, &format!("solidcraft:{}", self.root)) else {
            return;
        };
        if let Some((tk, k)) = self.screen_key.take() {
            nc::note_pane_on_screen(tk, k, false);
        }
        self.reclaim_focus(None, false);
        if new_c != old {
            nc::move_frames(old, new_c);
            nc::destroy(old);
        }
        self.toplevel = new_top;
        self.container = Some(new_c);
        self.placed = None;
        self.last_place = None;
        self.bg_applied = None;
        self.last_root = (i32::MIN, 0);
        self.place_seq = self.place_seq.wrapping_add(1);
        self.frames = nc::frames(new_c);
        nc::grab_click(new_c, true);
        self.sync_bg_pixel();
        if new_top == self.home {
            if let (Some(r), Some(w)) = (self.home_rect, self.child_window()) {
                self.report_screen(new_c, true);
                nc::place_shaped(new_c, Some(r), Some(w), None, None, &mut self.last_root);
                self.last_place = Some((new_c, r, Some(w)));
                self.placed = Some((Some(r), Some(w), Vec::new()));
            }
        }
        self.rpc(
            "editor.solidcraft.bring_back",
            json!({"project": self.root, "container": new_c, "toplevel": new_top}),
        );
    }

    pub fn detach(&mut self) {
        if let Some((t, c)) = self.screen_key.take() {
            nc::note_pane_on_screen(t, c, false);
        }
        self.reclaim_focus(None, false);
        let Some(c) = self.container.take() else {
            return;
        };
        nc::place(c, None, None, None, &mut self.last_root);
        nc::grab_escape(self.toplevel, false);
        nc::grab_click(c, true);
        let rect = self.last_place.map(|(_, r, _)| r);
        self.placed = None;
        self.last_place = None;
        self.bg_applied = None;
        self.place_seq = self.place_seq.wrapping_add(1);
        if self.info["state"] == "ready" && !self.frames.is_empty() && self.toplevel == self.home {
            for old in keep(Kept {
                toplevel: self.toplevel,
                root: self.root.clone(),
                container: c,
                frames: self.frames.clone(),
                rect,
            }) {
                self.rpc("editor.solidcraft.detach", json!({"project": old.root}));
                nc::destroy(old.container);
            }
            self.rpc(
                "editor.solidcraft.detach",
                json!({"project": self.root, "keep": true}),
            );
            return;
        }
        self.rpc("editor.solidcraft.detach", json!({"project": self.root}));
        nc::destroy(c);
    }

    fn report_screen(&mut self, container: u64, on: bool) {
        let next = on.then_some((self.toplevel, container));
        if self.screen_key == next {
            return;
        }
        if let Some((t, c)) = self.screen_key.take() {
            nc::note_pane_on_screen(t, c, false);
        }
        if let Some((t, c)) = next {
            nc::note_pane_on_screen(t, c, true);
            self.screen_key = Some((t, c));
        }
    }

    fn ensure_tick(&mut self, cx: &mut Context<Self>) {
        if self.ticking {
            return;
        }
        self.ticking = true;
        cx.spawn(async move |weak, cx| loop {
            let interval = weak
                .update(cx, |this, cx| {
                    this.tick(cx);
                    if this.child_window().is_none()
                        && matches!(
                            this.info["state"].as_str(),
                            Some("starting" | "ready") | None
                        )
                    {
                        Duration::from_millis(25)
                    } else {
                        Duration::from_millis(60)
                    }
                })
                .ok();
            let Some(ms) = interval else { break };
            cx.background_executor().timer(ms).await;
        })
        .detach();
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let Some(c) = self.container else {
            return;
        };
        if let Some(pid) = self.info["pid"].as_u64().filter(|p| *p > 0) {
            #[cfg(not(windows))]
            if !nc::adopt(pid as u32, c, self.toplevel).is_empty() {
                self.placed = None;
                cx.notify();
            }
            nc::dialogs(pid as u32, c, self.toplevel);
        }
        let f = nc::frames(c);
        if f != self.frames {
            self.frames = f;
            self.placed = None;
            cx.notify();
        }
        if let Some((Some(at), Some(show), _)) = &self.placed {
            nc::sync_root(c, *at, *show, &mut self.last_root);
        }
        let child = self.child_window();
        let has_child = child.is_some();
        let has_focus = has_child && nc::has_focus(c);
        if !self.in_page {
            nc::discard_events_for(c);
            if self.keys || has_focus {
                if self.reclaim_focus(None, false) {
                    cx.notify();
                }
            }
            return;
        }
        let pid = self.info["pid"].as_u64().unwrap_or(0) as u32;
        let in_dialog = has_child && pid != 0 && nc::focus_in_pid(pid) && !has_focus;
        if cfg!(windows) && self.in_page && !self.keys && !has_focus && !in_dialog && self.toplevel != 0 {
            nc::keep_focus(self.toplevel);
        }
        let mut escaped = false;
        let mut plain_escape = false;
        let mut saw_key = false;
        let mut click_in = TickClickIn::None;
        for e in nc::events_for(c, self.toplevel) {
            match e {
                nc::Ev::Escape => {
                    escaped = true;
                    plain_escape = true;
                }
                nc::Ev::Key(k) => {
                    escaped = true;
                    saw_key = true;
                    self.press.push(k);
                }
                nc::Ev::Click => {
                    if !escaped {
                        click_in = TickClickIn::GrabClick;
                    }
                }
                nc::Ev::ShieldClick | nc::Ev::Focus(_) => {
                    if !escaped && click_in == TickClickIn::None {
                        click_in = TickClickIn::AlreadyFocused;
                    }
                }
            }
        }
        let button_over = child.and_then(nc::button_over);
        let pointer_elsewhere =
            child.is_some_and(|w| button_over.is_some() && nc::pointer_on_other_window(w));
        let d = decide_tick_focus(
            self.in_page,
            has_child,
            self.keys,
            self.press_in,
            has_focus,
            in_dialog,
            button_over,
            pointer_elsewhere,
            click_in,
            escaped,
        );
        let prev_keys = self.keys;
        self.keys = d.next_keys;
        self.press_in = d.next_press_in;
        match d.action {
            TickFocusAction::FocusChild => {
                if let Some(w) = child {
                    nc::grab_click(c, false);
                    nc::focus(w);
                    nc::grab_escape(self.toplevel, true);
                    nc::note_pane_keys(true);
                }
            }
            TickFocusAction::ReclaimToHost => {
                nc::grab_escape(self.toplevel, false);
                nc::grab_click(c, true);
                nc::note_pane_keys(false);
                if self.toplevel != 0 {
                    if escaped {
                        nc::hand_back(self.toplevel);
                    } else {
                        nc::focus(self.toplevel);
                    }
                }
            }
            TickFocusAction::None => {
                if self.keys != prev_keys {
                    nc::grab_escape(self.toplevel, self.keys);
                    nc::grab_click(c, !self.keys);
                    nc::note_pane_keys(self.keys);
                }
            }
        }
        if plain_escape {
            cx.emit(SolidCraftEvent::Escape);
        }
        if self.keys != prev_keys || saw_key || plain_escape {
            cx.notify();
        }
    }

    fn decide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let top = top_of(window);
        if let (Some(_), Some(t)) = (self.container, top) {
            if t != self.toplevel {
                self.rehome(t);
                let weak = cx.entity().downgrade();
                window.on_frame_drawn(move |window, cx| {
                    weak.update(cx, |this, cx| this.after_frame(window, cx))
                        .is_ok()
                });
            }
            return;
        }
        if self.container.is_some() || self.attached {
            return;
        }
        let Some(top) = top else {
            self.info = json!({
                "state": "failed",
                "reason": trs("This window cannot embed SolidCraft (X11/XWayland or Win32 required).")
            });
            self.attached = true;
            return;
        };
        self.toplevel = top;
        self.home = top;
        if let Some(k) = take_kept(top, &self.root) {
            self.container = Some(k.container);
            self.frames = nc::frames(k.container);
            if self.frames.is_empty() {
                self.frames = k.frames;
            }
        } else {
            self.container = nc::container_for(top, &format!("solidcraft:{}", self.root));
        }
        let Some(c) = self.container else {
            self.info = json!({
                "state": "failed",
                "reason": trs("Could not create embedded SolidCraft container window.")
            });
            self.attached = true;
            return;
        };
        nc::grab_click(c, true);
        self.attached = true;
        self.ensure_tick(cx);
        self.reattach();
        let weak = cx.entity().downgrade();
        window.on_frame_drawn(move |window, cx| {
            weak.update(cx, |this, cx| this.after_frame(window, cx))
                .is_ok()
        });
    }

    fn after_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if top_of(window) != Some(self.toplevel) {
            return;
        }
        for k in std::mem::take(&mut self.press) {
            if let Ok(k) = Keystroke::parse(&k) {
                window.defer(cx, move |window, cx| {
                    window.dispatch_keystroke(k, cx);
                });
            }
        }
        let Some(c) = self.container else {
            return;
        };
        if popped_elsewhere(self.popped, self.toplevel, self.home) {
            return;
        }
        let seen = self
            .marker
            .lock()
            .unwrap()
            .and_then(|id| window.hitbox_drawn_after(id));
        self.at_px = seen.as_ref().map(|(b, _)| *b);
        let scale = window.scale_factor();
        let to_rect = |b: &Bounds<Pixels>| {
            nc::pane_rect_from_f32(
                f32::from(b.origin.x),
                f32::from(b.origin.y),
                f32::from(b.size.width),
                f32::from(b.size.height),
                scale,
            )
        };
        let at = seen.as_ref().and_then(|(b, _)| to_rect(b));
        let after = self
            .end_marker
            .lock()
            .unwrap()
            .and_then(|id| window.hitbox_drawn_after(id));
        let mut over: Vec<nc::R> = after
            .as_ref()
            .map(|(_, o)| {
                o.iter()
                    .filter(|b| f32::from(b.size.width) >= 12. && f32::from(b.size.height) >= 12.)
                    .filter_map(to_rect)
                    .collect()
            })
            .unwrap_or_default();
        if let Some(p) = after.as_ref().and_then(|(b, _)| to_rect(b)).or(at) {
            if over.iter().any(|o| *o != p) {
                over.retain(|o| *o != p);
            }
        }
        let child = self.child_window();
        let show_ok = container_visible(
            self.in_page,
            popped_elsewhere(self.popped, self.toplevel, self.home),
            at.is_some(),
            child.is_some(),
        );
        let occluded = if let (true, Some(p)) = (show_ok, at) {
            !over.is_empty() && nc::cut(p, &over).is_empty()
        } else {
            false
        };
        let effective_at = (show_ok && !occluded).then_some(at).flatten();
        let effective_show = effective_at.and(child);
        let key = (effective_at, effective_show, over.clone());
        if self.placed.as_ref() == Some(&key) {
            if let (Some(r), Some(w)) = (effective_at, effective_show) {
                nc::sync_root(c, r, w, &mut self.last_root);
            }
            return;
        }
        self.sync_bg_pixel();
        let prev_at = self.placed.as_ref().and_then(|(prev, _, _)| *prev);
        let plan = plan_placement(prev_at, effective_at, &over);
        self.place_seq = self.place_seq.wrapping_add(1);
        let seq = self.place_seq;
        let cut_rects = effective_at.filter(|_| !over.is_empty()).map(|p| nc::cut(p, &over));
        match plan {
            PlacePlan::HideNow => {
                if self.reclaim_focus(None, false) {
                    cx.notify();
                }
                self.report_screen(c, false);
                nc::place_shaped(c, None, None, None, None, &mut self.last_root);
                self.last_place = None;
                self.placed = Some(key);
            }
            PlacePlan::Immediate => {
                self.report_screen(c, effective_at.is_some() && effective_show.is_some());
                nc::place_shaped(
                    c,
                    effective_at,
                    effective_show,
                    None,
                    cut_rects.as_deref(),
                    &mut self.last_root,
                );
                self.last_place = effective_at.map(|r| (c, r, effective_show));
                if self.toplevel == self.home && effective_at.is_some() {
                    self.home_rect = effective_at;
                }
                self.placed = Some(key);
            }
            PlacePlan::SyncResize { pre_clip } => {
                nc::holes(c, Some(&pre_clip));
                let weak = cx.entity().downgrade();
                window.after_present(move |window, cx| {
                    let _ = weak.update(cx, |this, _| {
                        if this.place_seq != seq
                            || this.container != Some(c)
                            || top_of(window) != Some(this.toplevel)
                        {
                            return;
                        }
                        this.report_screen(c, effective_at.is_some() && effective_show.is_some());
                        nc::place_shaped(
                            c,
                            effective_at,
                            effective_show,
                            None,
                            cut_rects.as_deref(),
                            &mut this.last_root,
                        );
                        this.last_place = effective_at.map(|r| (c, r, effective_show));
                        if this.toplevel == this.home && effective_at.is_some() {
                            this.home_rect = effective_at;
                        }
                        this.placed = Some((effective_at, effective_show, over));
                    });
                });
            }
        }
    }

    pub fn report(&self) -> Value {
        let live = self.container.map_or_else(Vec::new, nc::frames);
        let frames_src = if !live.is_empty() { &live } else { &self.frames };
        let child = frames_src.first().map(|f| f.window);
        let g = |w: u64| {
            nc::report(w).map_or(Value::Null, |(r, on, lit)| {
                let mut v = json!({"x": r.x, "y": r.y, "w": r.w, "h": r.h, "shown": on, "painted": lit});
                if let Some((depth, (rx, ry), rects, in_tree)) =
                    nc::window_meta(w, self.container, self.toplevel)
                {
                    v["depth"] = depth.into();
                    v["root_x"] = rx.into();
                    v["root_y"] = ry.into();
                    v["shape_rects"] = rects.into();
                    v["in_tree"] = in_tree.into();
                }
                v
            })
        };
        let pane = self
            .placed
            .as_ref()
            .and_then(|(at, _, _)| *at)
            .or_else(|| {
                self.last_place
                    .filter(|(lc, _, _)| Some(*lc) == self.container)
                    .map(|(_, r, _)| r)
            })
            .map_or(Value::Null, |p| json!({"x": p.x, "y": p.y, "w": p.w, "h": p.h}));
        json!({
            "ok": true,
            "root": self.root,
            "file": self.file,
            "hub_file": self.info["file"],
            "state": self.info["state"],
            "reason": self.info["reason"],
            "pid": self.info["pid"],
            "gen": self.info["gen"],
            "parked": self.info["parked"],
            "control_port": self.info["control_port"],
            "version": self.info["version"],
            "runtime": self.info["runtime"],
            "in_page": self.in_page,
            "popped": self.popped,
            "keys": self.keys,
            "pane": pane,
            "container": self.container.map_or(Value::Null, |c| { let mut v = g(c); v["xid"] = c.into(); v }),
            "child": child.map_or(Value::Null, |w| { let mut v = g(w); v["xid"] = w.into(); v }),
            "toplevel": self.toplevel,
            "frames": frames_src.iter().map(|f| { let mut v = g(f.window); v["xid"] = f.window.into(); v["title"] = f.title.clone().into(); v }).collect::<Vec<_>>(),
        })
    }
}

impl Drop for SolidCraftPane {
    fn drop(&mut self) {
        if let Some((t, c)) = self.screen_key.take() {
            nc::note_pane_on_screen(t, c, false);
        }
        self.detach();
    }
}

impl Render for SolidCraftPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.decide(window, cx);
        let light = self.light;
        let c = |n: &str| crate::theme::color(light, n);
        let status = self.status();
        let marker = self.marker.clone();
        let end_marker = self.end_marker.clone();
        let pane = cx.entity().downgrade();
        let focus = self.focus.clone();
        let keys = self.keys;

        let overlay: Option<AnyElement> = match status {
            PaneStatus::Ready => None,
            PaneStatus::PoppedOut => Some(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .items_center()
                    .justify_center()
                    .bg(c("view"))
                    .text_color(c("dim"))
                    .child(trs("SolidCraft is open in its own window."))
                    .child(
                        crate::ui::with_rect(
                            div().id("solidcraft-bring-back"),
                            || "solidcraft:bring_back".to_string(),
                        )
                        .px_3()
                        .py_1()
                        .border_1()
                        .border_color(c("line"))
                        .bg(c("raise"))
                        .text_color(c("text"))
                        .cursor_pointer()
                        .hover(|s| s.bg(c("panel")))
                        .child(trs("Bring back"))
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(SolidCraftEvent::BringBack))),
                    )
                    .into_any_element(),
            ),
            PaneStatus::NotInstalled => Some(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .items_center()
                    .justify_center()
                    .px_6()
                    .bg(c("view"))
                    .text_color(c("dim"))
                    .child(
                        div()
                            .text_color(c("text"))
                            .child(trs("SolidCraft is not installed.")),
                    )
                    .child(
                        div()
                            .max_w(px(380.))
                            .text_center()
                            .text_size(crate::scale::tpx(12.))
                            .child(install_words(&self.info["runtime"])),
                    )
                    .child(
                        crate::ui::with_rect(
                            div().id("solidcraft-install"),
                            || "solidcraft:install".to_string(),
                        )
                        .px_3()
                        .py_1()
                        .border_1()
                        .border_color(c("accent"))
                        .bg(c("accent"))
                        .text_color(c("on-accent"))
                        .cursor_pointer()
                        .child(trs("Install SolidCraft"))
                        .on_click(cx.listener(|this, _, _, cx| this.install(cx))),
                    )
                    // or no install: the part in Backplane's own 3D viewer (view only), and that as the default from now on if ticked
                    .child(div().text_size(crate::scale::tpx(12.)).child(trs("or")))
                    .child(
                        crate::ui::with_rect(
                            div().id("solidcraft-view-native"),
                            || "solidcraft:view_native".to_string(),
                        )
                        .px_3()
                        .py_1()
                        .border_1()
                        .border_color(c("line"))
                        .bg(c("raise"))
                        .text_color(c("text"))
                        .cursor_pointer()
                        .hover(|s| s.bg(c("panel")))
                        .child(trs("View directly in Backplane instead"))
                        .on_click(cx.listener(|this, _, _, cx| cx.emit(SolidCraftEvent::ViewNative(this.native_always)))),
                    )
                    .child({
                        let on = self.native_always;
                        crate::ui::with_rect(
                            div().id("solidcraft-native-always"),
                            || "solidcraft:native_always".to_string(),
                        )
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(crate::scale::tpx(12.))
                        .cursor_pointer()
                        .child(
                            div()
                                .size(px(14.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .border_1()
                                .border_color(if on { c("accent") } else { c("line") })
                                .bg(if on { c("accent") } else { c("raise") })
                                .text_color(c("on-accent"))
                                .text_size(crate::scale::tpx(10.))
                                .child(if on { "✓" } else { "" }),
                        )
                        .child(trs("Always view parts in Backplane (change it in Settings › CAD)"))
                        .on_click(cx.listener(|this, _, _, cx| { this.native_always = !this.native_always; cx.notify(); }))
                    })
                    .into_any_element(),
            ),
            PaneStatus::Installing { done, total } => {
                let pct = if total > 0 {
                    (done as f32 / total as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let detail = if total > 0 {
                    format!(
                        "{:.1} / {:.1} MB",
                        done as f64 / (1024.0 * 1024.0),
                        total as f64 / (1024.0 * 1024.0)
                    )
                } else {
                    trs("Downloading…")
                };
                Some(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .items_center()
                        .justify_center()
                        .px_6()
                        .bg(c("view"))
                        .text_color(c("dim"))
                        .child(trs("Installing SolidCraft…"))
                        .child(
                            div()
                                .w(px(220.))
                                .h(px(4.))
                                .bg(c("line"))
                                .child(div().h_full().bg(c("accent")).w(px(220. * pct))),
                        )
                        .child(div().text_size(crate::scale::tpx(11.)).child(detail))
                        .into_any_element(),
                )
            }
            PaneStatus::Starting(msg) => Some(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .items_center()
                    .justify_center()
                    .px_6()
                    .bg(c("view"))
                    .text_color(c("dim"))
                    .child(msg)
                    .into_any_element(),
            ),
            PaneStatus::Stopped => Some(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .items_center()
                    .justify_center()
                    .px_6()
                    .bg(c("view"))
                    .text_color(c("dim"))
                    .child(trs("SolidCraft is closed."))
                    .child(
                        crate::ui::with_rect(
                            div().id("solidcraft-start"),
                            || "solidcraft:retry".to_string(),
                        )
                        .px_3()
                        .py_1()
                        .border_1()
                        .border_color(c("line"))
                        .bg(c("raise"))
                        .text_color(c("text"))
                        .cursor_pointer()
                        .hover(|s| s.bg(c("panel")))
                        .child(trs("Open SolidCraft"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.reattach();
                            cx.notify();
                        })),
                    )
                    .into_any_element(),
            ),
            PaneStatus::Failed(msg) | PaneStatus::Unresponsive(msg) => Some(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .items_center()
                    .justify_center()
                    .px_6()
                    .bg(c("view"))
                    .text_color(c("bad"))
                    .child(div().max_w(px(420.)).text_center().child(msg))
                    .child(
                        crate::ui::with_rect(
                            div().id("solidcraft-retry"),
                            || "solidcraft:retry".to_string(),
                        )
                        .px_3()
                        .py_1()
                        .border_1()
                        .border_color(c("line"))
                        .bg(c("raise"))
                        .text_color(c("text"))
                        .cursor_pointer()
                        .hover(|s| s.bg(c("panel")))
                        .child(trs("Try again"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.reattach();
                            cx.notify();
                        })),
                    )
                    // a SolidCraft that will not start: the part in Backplane's own viewer, as on the not-installed screen (no "Always" tick here)
                    .child(
                        crate::ui::with_rect(
                            div().id("solidcraft-failed-view-native"),
                            || "solidcraft:view_native".to_string(),
                        )
                        .px_3()
                        .py_1()
                        .border_1()
                        .border_color(c("line"))
                        .bg(c("raise"))
                        .text_color(c("text"))
                        .cursor_pointer()
                        .hover(|s| s.bg(c("panel")))
                        .child(trs("View directly in Backplane instead"))
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(SolidCraftEvent::ViewNative(false)))),
                    )
                    .into_any_element(),
            ),
        };

        crate::ui::with_rect(div().id("solidcraft-pane"), || "solidcraft:pane".to_string())
            .size_full()
            .relative()
            .bg(if light { c("view") } else { Rgba::from(rgb(0x282b35)) })
            .track_focus(&focus)
            .border_2()
            .border_color(if keys {
                c("accent")
            } else {
                Rgba { a: 0., ..c("accent") }
            })
            .child(
                canvas(
                    move |b, window, _| {
                        let hb = window.insert_hitbox(b, HitboxBehavior::Normal);
                        *marker.lock().unwrap() = Some(hb.id);
                        hb
                    },
                    move |_, hb, window, _| {
                        let pane = pane.clone();
                        let end_marker = end_marker.clone();
                        window.on_mouse_event(move |ev: &MouseDownEvent, phase, window, cx| {
                            if phase != DispatchPhase::Capture {
                                return;
                            }
                            let pane_hovered = hb.is_hovered(window);
                            let hit_after = if pane_hovered {
                                let after_id = end_marker.lock().unwrap().unwrap_or(hb.id);
                                window.hitbox_drawn_after(after_id).is_some_and(|(_, over)| {
                                    over.iter().any(|b| b.contains(&ev.position))
                                })
                            } else {
                                false
                            };
                            if kicad_pane::click_is_in_backplane(pane_hovered, hit_after) {
                                let target = top_of(window);
                                let _ = pane.update(cx, |this, cx| this.on_backplane_click(target, cx));
                            }
                        });
                    },
                )
                .size_full(),
            )
            .children(overlay)
            .child({
                let end = self.end_marker.clone();
                canvas(
                    move |b, window, _| {
                        *end.lock().unwrap() =
                            Some(window.insert_hitbox(b, HitboxBehavior::Normal).id);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0()
            })
    }
}

/// Placeholder rendered in the main window's Mech pane/tile while SolidCraft is popped out
/// into `SolidCraftWindow`, avoiding double-mounting the same `SolidCraftPane` entity.
pub fn popped_placeholder(cx: &mut Context<Backplane>, light: bool) -> AnyElement {
    let c = |n: &str| crate::theme::color(light, n);
    div()
        .size_full()
        .flex()
        .flex_col()
        .gap_2()
        .items_center()
        .justify_center()
        .bg(c("view"))
        .text_color(c("dim"))
        .child(trs("SolidCraft is open in its own window."))
        .child(
            crate::ui::with_rect(
                div().id("solidcraft-bring-back"),
                || "solidcraft:bring_back".to_string(),
            )
            .px_3()
            .py_1()
            .border_1()
            .border_color(c("line"))
            .bg(c("raise"))
            .text_color(c("text"))
            .cursor_pointer()
            .hover(|s| s.bg(c("panel")))
            .child(trs("Bring back"))
            .on_click(cx.listener(|this, _, _, cx| {
                let w = this.screens.solidcraft_window;
                this.solidcraft_bring_back(cx);
                if let Some(w) = w {
                    let _ = w.update(cx, |_, win, _| win.remove_window());
                }
            })),
        )
        .into_any_element()
}

/// Detached window hosting SolidCraft when popped out of the main Backplane pane.
pub struct SolidCraftWindow {
    pub pane: Entity<SolidCraftPane>,
    pub app: WeakEntity<Backplane>,
    pub light: bool,
}

impl Render for SolidCraftWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let light = self.pane.read(cx).light;
        self.light = light;
        let c = |n: &str| crate::theme::color(light, n);
        let file_label = {
            let p = self.pane.read(cx);
            let f = std::path::Path::new(&p.file)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if f.is_empty() {
                std::path::Path::new(&p.root)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("SolidCraft") // i18n-ok: product name
                    .to_string()
            } else {
                f.to_string()
            }
        };
        let _ = window;
        let bar = div()
            .id("solidcraft-win-bar")
            .h(crate::scale::tpx(30.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .bg(c("panel"))
            .border_b_1()
            .border_color(c("rule"))
            .child(
                div()
                    .text_size(crate::scale::tpx(12.))
                    .text_color(c("text"))
                    .child(/* i18n-ok: product name + filename */ format!("SolidCraft · {file_label}")),
            )
            .child(
                crate::ui::with_rect(
                    div().id("solidcraft-win-bring-back"),
                    || "solidcraft:bring_back".to_string(),
                )
                .px_2()
                .py(px(3.))
                .text_size(crate::scale::tpx(12.))
                .border_1()
                .border_color(c("line"))
                .bg(c("raise"))
                .text_color(c("text"))
                .cursor_pointer()
                .hover(|s| s.bg(c("panel")))
                .child(trs("Bring back"))
                .on_click(cx.listener(|this, _, window, cx| {
                    if let Some(app) = this.app.upgrade() {
                        app.update(cx, |app, cx| app.solidcraft_bring_back(cx));
                    }
                    window.remove_window();
                })),
            );
        let font = crate::keys::tip_font();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(c("bg"))
            .text_color(c("text"))
            .text_size(crate::scale::tpx(13.))
            .line_height(crate::scale::tpx(20.))
            .when(!font.is_empty(), |d| d.font_family(font))
            .child(bar)
            .child(div().flex_1().min_h_0().child(self.pane.clone()))
    }
}

pub(crate) fn window_title(root: &str) -> String {
    let name = std::path::Path::new(root)
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| /* i18n-ok: product name */ "SolidCraft".into());
    let title = crate::streamer::project_shown(&name);
    /* i18n-ok: detached window title */ crate::streamer::shown(&format!("SolidCraft · {title} — Backplane"))
}

impl Backplane {
    /// Handle hub `editor.solidcraft.<digest>`, `solidcraft.runtime`, and `solidcraft.install` info broadcasts.
    pub(crate) fn solidcraft_info(&mut self, digest: String, text: &str, cx: &mut Context<Self>) {
        if text.is_empty() {
            self.screens.solidcraft_infos.remove(&digest);
            return;
        }
        let Ok(v) = serde_json::from_str::<Value>(text) else {
            return;
        };
        self.screens.solidcraft_infos.insert(digest.clone(), v.clone());
        if digest == "solidcraft.install" {
            if let Some(p) = self.screens.solidcraft.clone() {
                match v["state"].as_str().unwrap_or("") {
                    "running" => {
                        p.update(cx, |p, cx| {
                            let mut info = p.info.clone();
                            if !info.is_object() {
                                info = json!({"root": p.root});
                            }
                            info["state"] = json!("installing");
                            info["install_progress"] = json!({
                                "done": v["done"].as_u64().unwrap_or(0),
                                "total": v["total"].as_u64().unwrap_or(0),
                            });
                            if let Some(note) = v["note"].as_str() {
                                info["reason"] = json!(note);
                            }
                            p.set_info(info, cx);
                        });
                    }
                    "done" => {
                        p.update(cx, |p, cx| {
                            p.reattach();
                            cx.notify();
                        });
                    }
                    "failed" => {
                        p.update(cx, |p, cx| {
                            let mut info = p.info.clone();
                            if !info.is_object() {
                                info = json!({"root": p.root});
                            }
                            info["state"] = json!("failed");
                            let msg = v["note"].as_str().map(trs).unwrap_or_else(|| trs("Install failed."));
                            info["reason"] = json!(msg);
                            p.set_info(info, cx);
                        });
                    }
                    _ => {}
                }
            }
            cx.notify();
            return;
        }
        if digest == "solidcraft.runtime" {
            if v["installed"] == true {
                if let Some(p) = self.screens.solidcraft.clone() {
                    if matches!(p.read(cx).status(), PaneStatus::NotInstalled) {
                        p.update(cx, |p, cx| {
                            p.reattach();
                            cx.notify();
                        });
                    }
                }
            }
            cx.notify();
            return;
        }
        if let Some(p) = self.screens.solidcraft.clone() {
            if p.read(cx).matches(&v) {
                let sync_part = p.read(cx).awaiting_created
                    && v["state"].as_str() == Some("ready")
                    && v["file"]
                        .as_str()
                        .is_some_and(|f| !f.is_empty() && norm_path(f) != norm_path(&p.read(cx).file));
                let created_rel = sync_part
                    .then(|| new_part_rel(&p.read(cx).root, v["file"].as_str().unwrap_or("")));
                p.update(cx, |p, cx| p.set_info(v, cx));
                if let Some(rel) = created_rel.filter(|r| !r.is_empty()) {
                    let _ = self.tx.try_send(Command::Action("mech-part".into(), rel));
                }
            }
        }
        cx.notify();
    }

    /// Handle direct `editor.solidcraft.*` RPC reply.
    pub(crate) fn solidcraft_reply(&mut self, method: String, reply: Value, cx: &mut Context<Self>) {
        if reply["ok"] != true {
            let msg = crate::hub::rpc_error(&reply);
            if let Some(p) = self.screens.solidcraft.clone() {
                if method == "editor.solidcraft.attach" || method == "editor.solidcraft.install" {
                    let info = json!({"state": "failed", "reason": msg, "root": p.read(cx).root});
                    p.update(cx, |p, cx| p.set_info(info, cx));
                } else if method == "editor.solidcraft.open" {
                    p.update(cx, |p, _| p.awaiting_created = false);
                    self.note = msg;
                }
            }
            cx.notify();
            return;
        }
        let info = if reply["result"].is_object() {
            reply["result"].clone()
        } else {
            reply
        };
        if info["state"].is_string() {
            if let Some(p) = self.screens.solidcraft.clone() {
                if p.read(cx).matches(&info) {
                    let created_file = (p.read(cx).awaiting_created
                        && matches!(method.as_str(), "editor.solidcraft.open" | "editor.solidcraft.attach"))
                    .then(|| info["file"].as_str().unwrap_or("").to_string())
                    .filter(|f| !f.is_empty());
                    let rel = created_file
                        .as_deref()
                        .map(|f| new_part_rel(&p.read(cx).root, f));
                    p.update(cx, |p, cx| {
                        if let Some(ref f) = created_file {
                            p.file = norm_path(f);
                            p.sent_file = p.file.clone();
                            p.awaiting_created = false;
                        }
                        p.set_info(info, cx);
                    });
                    if let Some(rel) = rel.filter(|r| !r.is_empty()) {
                        let _ = self.tx.try_send(Command::Action("mech-part".into(), rel));
                    }
                }
            }
        }
        cx.notify();
    }

    /// Ensure the SolidCraft pane is attached and tracking the current Mech/CAD target.
    pub(crate) fn solidcraft_sync(&mut self, cx: &mut Context<Self>) {
        let thread = &self.data["thread"];
        let viewer = &thread["viewer"];
        let is_mech = crate::screens::viewer::is_open(thread)
            && !thread["diff"].is_object()
            && !crate::screens::viewer::mech_native(thread)
            && (viewer["open"] == "mech"
                || (crate::screens::viewer::is_tiled(viewer) && mech_tile(viewer).is_some()));
        if !is_mech {
            return;
        }
        let local = kicad_pane::local(
            self.screens.attacher.target.as_ref().map(|t| t.addr.as_str()),
            thread,
        );
        if !local {
            return;
        }
        let Some((root, file)) = mech_target(&self.data, thread) else {
            return;
        };
        let tid = crate::screens::attach::raw_id(thread["id"].as_str().unwrap_or(""));
        let pid = crate::keys::project_of(&self.data, tid);
        self.ensure_solidcraft_pane(root, pid, file, cx);
    }

    fn ensure_solidcraft_pane(
        &mut self,
        root: String,
        project_id: String,
        file: String,
        cx: &mut Context<Self>,
    ) -> Entity<SolidCraftPane> {
        let create = std::mem::take(&mut self.screens.solidcraft_create_next);
        if let Some(existing) = self.screens.solidcraft.clone() {
            let same_root = norm_path(&existing.read(cx).root) == norm_path(&root);
            if same_root {
                if create {
                    existing.update(cx, |p, cx| p.create_new_part(cx));
                } else if !file.is_empty() {
                    existing.update(cx, |p, cx| p.open_file(&file, cx));
                }
                return existing;
            }
            existing.update(cx, |p, _| p.detach());
            self.screens.solidcraft = None;
        }
        let hub = self.tx.clone();
        let infos = self.screens.solidcraft_infos.clone();
        let light = self.light();
        let page = cx.new(move |cx| {
            let mut p = SolidCraftPane::new(cx, root, project_id, file, hub);
            p.light = light;
            p.create_on_attach = create;
            if let Some(v) = infos.values().find(|v| p.matches(v)).cloned() {
                p.info = v;
            } else if let Some(rt) = infos.get("solidcraft.runtime").cloned() {
                if rt["installed"] == false {
                    p.info = json!({"state": "stopped", "root": p.root, "runtime": rt});
                }
            }
            p
        });
        cx.subscribe(&page, |this, _, ev: &SolidCraftEvent, cx| match ev {
            SolidCraftEvent::Popout => this.solidcraft_popout(cx),
            SolidCraftEvent::BringBack => {
                let h = this.screens.solidcraft_window;
                this.solidcraft_bring_back(cx);
                if let Some(h) = h {
                    let _ = h.update(cx, |_, w, _| w.remove_window());
                }
            }
            SolidCraftEvent::Escape => {
                if this.screens.solidcraft_full {
                    this.solidcraft_set_full(false, cx);
                }
            }
            SolidCraftEvent::ViewNative(always) => {
                // the default from now on: the hub's setting (Settings › CAD › Mech tab); this window switches at once either way
                if *always { let _ = this.tx.try_send(Command::Action("setting".into(), "viewer.mech=backplane".into())); }
                crate::screens::viewer::set_mech_native(this, true);
                cx.notify();
            }
        })
        .detach();
        self.screens.solidcraft = Some(page.clone());
        page
    }

    /// Return the active `SolidCraftPane` if it belongs to `thread` and local hub.
    pub(crate) fn solidcraft_for(
        &self,
        thread: &Value,
        cx: &App,
    ) -> Option<Entity<SolidCraftPane>> {
        if !kicad_pane::local(
            self.screens.attacher.target.as_ref().map(|t| t.addr.as_str()),
            thread,
        ) {
            return None;
        }
        let pane = self.screens.solidcraft.as_ref()?;
        let (root, _) = mech_target(&self.data, thread)?;
        (norm_path(&pane.read(cx).root) == norm_path(&root)).then(|| pane.clone())
    }

    /// Live file-follow when an agent calls `viewer_show` with a CAD file path.
    pub(crate) fn solidcraft_follow(&mut self, thread: &str, path: &str, cx: &mut Context<Self>) {
        if path.is_empty() || !is_cad_file(path) {
            return;
        }
        if !(self.selected == thread
            || self.selected.ends_with(&format!("|{thread}"))
            || thread.is_empty())
        {
            return;
        }
        if let Some(pane) = self.screens.solidcraft.clone() {
            pane.update(cx, |p, cx| p.open_file(path, cx));
        }
    }

    pub(crate) fn solidcraft_rejoin(&mut self, cx: &mut Context<Self>) {
        if let Some(pane) = self.screens.solidcraft.clone() {
            pane.update(cx, |p, _| p.reattach());
            cx.notify();
        }
    }

    pub(crate) fn solidcraft_lend(&mut self, cx: &mut Context<Self>) {
        if let Some(pane) = self.screens.solidcraft.clone() {
            pane.update(cx, |p, cx| p.lend_focus_to(None, cx));
        }
    }

    pub(crate) fn solidcraft_reclaim_to(&mut self, window: &Window, cx: &mut Context<Self>) {
        if let Some(pane) = self.screens.solidcraft.clone() {
            let target = top_of(window);
            pane.update(cx, |p, cx| p.lend_focus_to(target, cx));
        }
    }

    pub(crate) fn solidcraft_reclaim_if_popped(&mut self, window: &Window, cx: &mut Context<Self>) {
        if let Some(pane) = self.screens.solidcraft.clone() {
            if pane.read(cx).popped {
                let target = top_of(window);
                pane.update(cx, |p, cx| p.lend_focus_to(target, cx));
            }
        }
    }

    pub(crate) fn solidcraft_new_part(&mut self, cx: &mut Context<Self>) {
        let on_mech = self.data["thread"]["viewer"]["open"] == "mech"
            || (crate::screens::viewer::is_tiled(&self.data["thread"]["viewer"])
                && mech_tile(&self.data["thread"]["viewer"]).is_some());
        if on_mech {
            if let Some(pane) = self.solidcraft_for(&self.data["thread"], cx) {
                pane.update(cx, |p, cx| p.create_new_part(cx));
                return;
            }
        }
        self.screens.solidcraft_create_next = true;
        self.rail_press("view-kind", "mech", cx);
    }

    pub(crate) fn solidcraft_set_full(&mut self, full: bool, cx: &mut Context<Self>) {
        let active = self.screens.solidcraft.is_some() && self.screens.solidcraft_window.is_none();
        let next = next_solidcraft_full(self.screens.solidcraft_full, Some(full), active);
        if self.screens.solidcraft_full == next {
            return;
        }
        self.screens.solidcraft_full = next;
        // Keep `placed` intact so `after_frame()` computes `PlacePlan::SyncResize { pre_clip }`
        // and updates the X11 container inside `window.after_present(...)` without flicker.
        if let Some(p) = self.screens.solidcraft.clone() {
            p.update(cx, |p, _| p.lend_focus());
        }
        self.ui_epoch += 1;
        cx.notify();
    }

    pub(crate) fn solidcraft_toggle_full(&mut self, cx: &mut Context<Self>) {
        let next = !self.screens.solidcraft_full;
        self.solidcraft_set_full(next, cx);
    }

    pub(crate) fn solidcraft_popout(&mut self, cx: &mut Context<Self>) {
        let Some(pane) = self.screens.solidcraft.clone() else {
            return;
        };
        if self.screens.solidcraft_window.is_some() {
            return;
        }
        let (app, light) = (cx.entity().downgrade(), self.light());
        let title = window_title(&pane.read(cx).root);
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1280.), px(860.)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some(title.into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let pane_win = pane.clone();
        match cx.open_window(opts, move |window, cx| {
            let app2 = app.clone();
            window.on_window_should_close(cx, move |_, cx| {
                if let Some(a) = app2.upgrade() {
                    a.update(cx, |a, cx| a.solidcraft_bring_back(cx));
                }
                true
            });
            cx.new(|cx| {
                cx.observe(&pane_win, |_, _, cx| cx.notify()).detach();
                SolidCraftWindow { pane: pane_win.clone(), app, light }
            })
        }) {
            Ok(h) => {
                pane.update(cx, |p, cx| {
                    p.popped = true;
                    cx.notify();
                });
                self.screens.solidcraft_full = false;
                self.screens.solidcraft_window = Some(h.into());
            }
            Err(e) => {
                self.note = format!("{} {e}", trs("SolidCraft window could not open:"));
            }
        }
        cx.notify();
    }

    pub(crate) fn solidcraft_bring_back(&mut self, cx: &mut Context<Self>) {
        if let Some(pane) = self.screens.solidcraft.clone() {
            pane.update(cx, |p, cx| {
                p.popped = false;
                let h = p.home();
                if h != 0 {
                    p.rehome(h);
                }
                cx.notify();
            });
        }
        self.screens.solidcraft_window = None;
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn recognizes_all_nine_cad_extensions_case_insensitively() {
        for ext in CAD_EXTS {
            assert!(is_cad_file(&format!("mech/part{ext}")), "{ext}");
            assert!(is_cad_file(&format!("MECH/PART{}", ext.to_ascii_uppercase())), "{ext}");
            assert!(!is_cad_file(ext), "bare {ext} without stem should be rejected");
            assert!(!is_cad_file(&format!("/dir/{ext}")), "dotfile /dir/{ext} without stem should be rejected");
        }
        assert!(!is_cad_file("board.kicad_pcb"));
        assert!(!is_cad_file("notes.md"));
        assert!(!is_cad_file(""));
    }

    #[test]
    fn kept_container_preserves_last_rect_across_keep_and_take() {
        let r = nc::R { x: 120, y: 80, w: 640, h: 480 };
        let top = 999_001;
        let root = "/proj/kept-rect-test";
        let dropped = keep(Kept {
            toplevel: top,
            root: root.into(),
            container: 42,
            frames: Vec::new(),
            rect: Some(r),
        });
        assert!(dropped.is_empty());
        let taken = take_kept(top, root).expect("kept entry should be found");
        assert_eq!(taken.container, 42);
        assert_eq!(taken.rect, Some(r));
        assert!(take_kept(top, root).is_none());
    }

    #[test]
    fn popped_pane_hides_on_home_window_and_shows_in_detached_window() {
        // Still on home window (or before attach assigns a toplevel): treated as popped elsewhere.
        let home_popped = popped_elsewhere(true, 100, 100);
        assert!(home_popped);
        assert_eq!(pane_status(&json!({"state": "ready"}), true, home_popped), PaneStatus::PoppedOut);
        assert!(!container_visible(true, home_popped, true, true));

        assert!(popped_elsewhere(true, 0, 100));

        // Once rehomed onto the detached SolidCraftWindow (toplevel != home): renders & maps normally.
        let win_popped = popped_elsewhere(true, 200, 100);
        assert!(!win_popped);
        assert_eq!(pane_status(&json!({"state": "ready"}), true, win_popped), PaneStatus::Ready);
        assert!(container_visible(true, win_popped, true, true));
    }

    #[test]
    fn starting_then_file_switch_sends_single_open_when_ready() {
        let a = "/proj/a.step";
        let b = "/proj/b.step";
        // While starting, clicking B updates wanted_file=B, sent_file stays A, hub_file stays A -> no open sent yet.
        assert!(!needs_file_open(true, "starting", b, a, a));
        // Once hub transitions to ready with hub_file=A, single doc.open B should fire.
        assert!(needs_file_open(true, "ready", b, a, a));
        // After sent_file is recorded as B, no duplicate open fires even before hub_file updates.
        assert!(!needs_file_open(true, "ready", b, b, a));
    }

    #[test]
    fn resolves_relative_and_absolute_paths_cleanly() {
        assert_eq!(resolve_file_path("/proj/repo", "mech/case.step"), "/proj/repo/mech/case.step");
        assert_eq!(resolve_file_path("/proj/repo/", "/abs/part.sc"), "/abs/part.sc");
        assert_eq!(resolve_file_path("/proj/repo", ""), "");
        assert_eq!(resolve_file_path("", "mech/case.stl"), "mech/case.stl");
    }

    #[test]
    fn new_part_rel_strips_project_root_cleanly() {
        assert_eq!(new_part_rel("/proj", "/proj/mech/part-1.sc"), "mech/part-1.sc");
        assert_eq!(new_part_rel("/proj/", "/proj/mech/part-2.sc"), "mech/part-2.sc");
        assert_eq!(new_part_rel("/other", "/abs/part.sc"), "/abs/part.sc");
    }

    #[test]
    fn container_stays_unmapped_until_on_screen_with_rect_and_child() {
        assert!(!container_visible(false, false, true, true), "hidden when off-page");
        assert!(!container_visible(true, true, true, true), "hidden in main pane while popped out");
        assert!(!container_visible(true, false, false, true), "hidden without valid bounds");
        assert!(!container_visible(true, false, true, false), "hidden before SolidCraft window is adopted");
        assert!(container_visible(true, false, true, true));
    }

    #[test]
    fn window_buttons_only_with_a_solidcraft_window() {
        assert!(window_tools(&PaneStatus::Ready));
        assert!(window_tools(&PaneStatus::PoppedOut));
        for st in [PaneStatus::NotInstalled, PaneStatus::Stopped, PaneStatus::Failed("x".into()), PaneStatus::Starting("x".into()), PaneStatus::Unresponsive("x".into()), PaneStatus::Installing { done: 1, total: 2 }] {
            assert!(!window_tools(&st), "{st:?}");
        }
    }

    #[test]
    fn the_install_words_say_the_size_as_settings_does() {
        let rt = json!({"installed": false, "offer": {"available": true, "version": "0.50.0", "size": 228331854u64}});
        assert_eq!(crate::i18n::with_lang("en", || install_words(&rt)), "Install SolidCraft 0.50.0 (about 218 MB) to view and edit STEP, SolidCraft, STL, and mesh files.");
        assert_eq!(crate::i18n::with_lang("en", || install_words(&json!({"installed": false}))), "Install SolidCraft to view and edit STEP, SolidCraft, STL, and mesh files.");
    }
    #[test]
    fn pane_status_maps_runtime_and_lifecycle_states() {
        assert_eq!(pane_status(&json!({"state": "ready"}), true, true), PaneStatus::PoppedOut);
        assert_eq!(pane_status(&json!({"state": "ready"}), true, false), PaneStatus::Ready);
        assert!(matches!(pane_status(&json!({"state": "ready"}), false, false), PaneStatus::Starting(_)));
        assert_eq!(
            pane_status(&json!({"state": "stopped", "runtime": {"installed": false}}), false, false),
            PaneStatus::NotInstalled
        );
        assert_eq!(
            pane_status(&json!({"state": "stopped", "runtime": {"installed": true, "gui": ""}}), false, false),
            PaneStatus::Failed(trs("This computer has SolidCraft's command-line program, not its desktop window."))
        );
        assert_eq!(
            pane_status(&json!({"state": "installing", "install_progress": {"done": 100, "total": 200}}), false, false),
            PaneStatus::Installing { done: 100, total: 200 }
        );
        assert!(matches!(pane_status(&json!({"state": "failed", "reason": "boom"}), false, false), PaneStatus::Failed(_)));
    }

    #[test]
    fn live_file_follow_only_sends_open_for_new_paths_when_ready() {
        assert!(!needs_file_open(false, "ready", "/p/a.step", "", ""));
        assert!(!needs_file_open(true, "starting", "/p/a.step", "", ""));
        assert!(!needs_file_open(true, "ready", "", "", ""));
        assert!(!needs_file_open(true, "ready", "/p/a.step", "/p/a.step", ""));
        assert!(!needs_file_open(true, "ready", "/p/a.step", "", "/p/a.step"));
        assert!(needs_file_open(true, "ready", "/p/b.sc", "/p/a.step", "/p/a.step"));
    }

    #[test]
    fn mech_target_extracts_root_and_file_from_standalone_or_tiled_viewer() {
        let data = json!({
            "projects": [{"id": "p1", "root": "/proj", "threads": [{"id": "t1"}]}]
        });
        let thread = json!({
            "id": "p1|t1",
            "viewer": {
                "open": "mech",
                "key": "2|/proj|mech/case.step",
                "mech": {"rel": "mech/case.step"}
            }
        });
        assert_eq!(
            mech_target(&data, &thread),
            Some(("/proj".into(), "/proj/mech/case.step".into()))
        );

        let empty_mech = json!({
            "id": "p1|t1",
            "viewer": {"open": "mech", "key": ""}
        });
        assert_eq!(
            mech_target(&data, &empty_mech),
            Some(("/proj".into(), "".into()))
        );
    }

    #[test]
    fn rect_intersect_handles_overlap_and_disjoint_rects() {
        let a = nc::R { x: 100, y: 50, w: 300, h: 200 };
        let b = nc::R { x: 250, y: 100, w: 200, h: 250 };
        assert_eq!(rect_intersect(a, b), Some(nc::R { x: 250, y: 100, w: 150, h: 150 }));
        let c = nc::R { x: 400, y: 50, w: 100, h: 200 };
        assert_eq!(rect_intersect(a, c), None);
    }

    #[test]
    fn plan_placement_covers_hide_immediate_show_and_sync_resize() {
        let r1 = nc::R { x: 700, y: 70, w: 660, h: 800 };
        let r2_small_shrink = nc::R { x: 710, y: 70, w: 650, h: 800 };
        let r2_small_grow = nc::R { x: 690, y: 70, w: 670, h: 800 };
        let disjoint = nc::R { x: 50, y: 70, w: 200, h: 200 };

        assert_eq!(plan_placement(Some(r1), None, &[]), PlacePlan::HideNow);
        assert_eq!(plan_placement(None, None, &[]), PlacePlan::HideNow);
        assert_eq!(plan_placement(None, Some(r1), &[]), PlacePlan::Immediate);
        assert_eq!(plan_placement(Some(r1), Some(r1), &[]), PlacePlan::Immediate);
        assert_eq!(
            plan_placement(Some(r1), Some(disjoint), &[]),
            PlacePlan::SyncResize { pre_clip: Vec::new() }
        );

        assert_eq!(
            plan_placement(Some(r1), Some(r2_small_shrink), &[]),
            PlacePlan::SyncResize {
                pre_clip: vec![nc::R { x: 10, y: 0, w: 650, h: 800 }],
            }
        );

        assert_eq!(
            plan_placement(Some(r1), Some(r2_small_grow), &[]),
            PlacePlan::SyncResize {
                pre_clip: vec![nc::R { x: 0, y: 0, w: 660, h: 800 }],
            }
        );

        let m_collapse = nc::R { x: 653, y: 70, w: 477, h: 799 };
        let r_collapse = nc::R { x: 528, y: 70, w: 582, h: 799 };
        assert_eq!(
            plan_placement(Some(m_collapse), Some(r_collapse), &[]),
            PlacePlan::SyncResize {
                pre_clip: vec![nc::R { x: 0, y: 0, w: 457, h: 799 }],
            }
        );

        assert_eq!(
            plan_placement(Some(r_collapse), Some(m_collapse), &[]),
            PlacePlan::SyncResize {
                pre_clip: vec![nc::R { x: 125, y: 0, w: 457, h: 799 }],
            }
        );

        assert_eq!(
            plan_placement(Some(r1), Some(r2_small_shrink), &[r1]),
            PlacePlan::SyncResize { pre_clip: Vec::new() }
        );

        let overlay = vec![nc::R { x: 710, y: 70, w: 650, h: 100 }];
        assert_eq!(
            plan_placement(Some(r1), Some(r2_small_shrink), &overlay),
            PlacePlan::SyncResize {
                pre_clip: vec![nc::R { x: 10, y: 100, w: 650, h: 700 }],
            }
        );
    }

    #[test]
    fn reclaim_focus_transitions_handle_cached_keys_live_focus_race_and_popped_target() {
        let mut keys = true;
        let mut press_in = true;
        assert_eq!(
            decide_reclaim(&mut keys, &mut press_in, false, 100, None),
            ReclaimDecision {
                reclaim_native: true,
                keys_changed: true,
                focus_target: 100,
            }
        );
        assert!(!keys);
        assert!(!press_in);

        // Tick-lag race: keys is still false, but native focus is already in SolidCraft.
        let mut keys = false;
        let mut press_in = true;
        assert_eq!(
            decide_reclaim(&mut keys, &mut press_in, true, 100, None),
            ReclaimDecision {
                reclaim_native: true,
                keys_changed: false,
                focus_target: 100,
            }
        );
        assert!(!keys);
        assert!(!press_in);

        // Neither cached nor live focus: no native reclaim.
        let mut keys = false;
        let mut press_in = false;
        assert_eq!(
            decide_reclaim(&mut keys, &mut press_in, false, 100, Some(200)),
            ReclaimDecision {
                reclaim_native: false,
                keys_changed: false,
                focus_target: 0,
            }
        );

        // Popped-out cross-window click: return focus to the clicked Backplane window, not SolidCraftWindow.
        let mut keys = true;
        let mut press_in = false;
        assert_eq!(
            decide_reclaim(&mut keys, &mut press_in, true, 200, Some(100)),
            ReclaimDecision {
                reclaim_native: true,
                keys_changed: true,
                focus_target: 100,
            }
        );

        // Zero target_toplevel falls back to pane_toplevel.
        let mut keys = true;
        let mut press_in = false;
        assert_eq!(
            decide_reclaim(&mut keys, &mut press_in, true, 200, Some(0)),
            ReclaimDecision {
                reclaim_native: true,
                keys_changed: true,
                focus_target: 200,
            }
        );
    }

    #[test]
    fn tick_focus_blocks_unsolicited_focus_grab_on_redraw_or_startup() {
        assert_eq!(
            decide_tick_focus(
                true,
                true,
                false,
                false,
                true,
                false,
                None,
                false,
                TickClickIn::None,
                false,
            ),
            TickFocusDecision {
                next_keys: false,
                next_press_in: false,
                action: TickFocusAction::ReclaimToHost,
            }
        );
        assert_eq!(
            decide_tick_focus(
                true,
                true,
                false,
                false,
                true,
                false,
                Some(true),
                true,
                TickClickIn::None,
                false,
            ),
            TickFocusDecision {
                next_keys: false,
                next_press_in: false,
                action: TickFocusAction::ReclaimToHost,
            }
        );
    }

    #[test]
    fn tick_focus_hands_in_on_click_and_reclaims_on_backplane_click_or_escape() {
        assert_eq!(
            decide_tick_focus(
                true,
                true,
                false,
                false,
                false,
                false,
                None,
                false,
                TickClickIn::GrabClick,
                false,
            ),
            TickFocusDecision {
                next_keys: true,
                next_press_in: false,
                action: TickFocusAction::FocusChild,
            }
        );
        assert_eq!(
            decide_tick_focus(
                true,
                true,
                false,
                false,
                true,
                false,
                Some(true),
                false,
                TickClickIn::AlreadyFocused,
                false,
            ),
            TickFocusDecision {
                next_keys: true,
                next_press_in: true,
                action: TickFocusAction::None,
            }
        );
        assert_eq!(
            decide_tick_focus(
                true,
                true,
                true,
                false,
                true,
                false,
                Some(false),
                false,
                TickClickIn::None,
                false,
            ),
            TickFocusDecision {
                next_keys: false,
                next_press_in: false,
                action: TickFocusAction::ReclaimToHost,
            }
        );
        assert_eq!(
            decide_tick_focus(
                true,
                true,
                true,
                true,
                true,
                false,
                Some(true),
                false,
                TickClickIn::GrabClick,
                true,
            ),
            TickFocusDecision {
                next_keys: false,
                next_press_in: false,
                action: TickFocusAction::ReclaimToHost,
            }
        );
        // External focus loss (e.g. another app focused) drops keys without yanking focus back.
        assert_eq!(
            decide_tick_focus(
                true,
                true,
                true,
                false,
                false,
                false,
                None,
                false,
                TickClickIn::None,
                false,
            ),
            TickFocusDecision {
                next_keys: false,
                next_press_in: false,
                action: TickFocusAction::None,
            }
        );
    }

    #[test]
    fn tick_focus_preserves_orbit_drag_and_solidcraft_dialogs() {
        // Press starts inside SolidCraft.
        let d1 = decide_tick_focus(
            true,
            true,
            false,
            false,
            false,
            false,
            Some(true),
            false,
            TickClickIn::None,
            false,
        );
        assert_eq!(
            d1,
            TickFocusDecision {
                next_keys: true,
                next_press_in: true,
                action: TickFocusAction::FocusChild,
            }
        );
        // Pointer drags outside the pane while mouse button stays held: preserve keys & drag.
        let d2 = decide_tick_focus(
            true,
            true,
            d1.next_keys,
            d1.next_press_in,
            true,
            false,
            Some(false),
            false,
            TickClickIn::None,
            false,
        );
        assert_eq!(
            d2,
            TickFocusDecision {
                next_keys: true,
                next_press_in: true,
                action: TickFocusAction::None,
            }
        );
        // Button released -> press_in clears.
        let d3 = decide_tick_focus(
            true,
            true,
            d2.next_keys,
            d2.next_press_in,
            true,
            false,
            None,
            false,
            TickClickIn::None,
            false,
        );
        assert_eq!(
            d3,
            TickFocusDecision {
                next_keys: true,
                next_press_in: false,
                action: TickFocusAction::None,
            }
        );
        // Fresh click outside pane reclaims focus to Backplane.
        assert_eq!(
            decide_tick_focus(
                true,
                true,
                d3.next_keys,
                d3.next_press_in,
                true,
                false,
                Some(false),
                false,
                TickClickIn::None,
                false,
            ),
            TickFocusDecision {
                next_keys: false,
                next_press_in: false,
                action: TickFocusAction::ReclaimToHost,
            }
        );
        // Floating SolidCraft dialog keeps keys.
        assert_eq!(
            decide_tick_focus(
                true,
                true,
                true,
                false,
                false,
                true,
                None,
                false,
                TickClickIn::None,
                false,
            ),
            TickFocusDecision {
                next_keys: true,
                next_press_in: false,
                action: TickFocusAction::None,
            }
        );
        // Off-page reclaims if keys or live focus remained, otherwise no-op.
        assert_eq!(
            decide_tick_focus(
                false,
                true,
                true,
                false,
                false,
                false,
                None,
                false,
                TickClickIn::None,
                false,
            ),
            TickFocusDecision {
                next_keys: false,
                next_press_in: false,
                action: TickFocusAction::ReclaimToHost,
            }
        );
        assert_eq!(
            decide_tick_focus(
                false,
                true,
                false,
                false,
                false,
                false,
                None,
                false,
                TickClickIn::None,
                false,
            ),
            TickFocusDecision {
                next_keys: false,
                next_press_in: false,
                action: TickFocusAction::None,
            }
        );
        // Click then Escape/Key in same tick: escape wins, focus returns to Backplane.
        assert_eq!(
            decide_tick_focus(
                true,
                true,
                false,
                false,
                true,
                false,
                Some(true),
                false,
                TickClickIn::GrabClick,
                true,
            ),
            TickFocusDecision {
                next_keys: false,
                next_press_in: false,
                action: TickFocusAction::ReclaimToHost,
            }
        );
        // Floating dialog keeps keys even if keys was false, without stealing or clearing.
        assert_eq!(
            decide_tick_focus(
                true,
                true,
                false,
                false,
                false,
                true,
                None,
                false,
                TickClickIn::None,
                false,
            ),
            TickFocusDecision {
                next_keys: true,
                next_press_in: false,
                action: TickFocusAction::None,
            }
        );
        // Mouse down over placeholder when no child exists yet never claims keys.
        assert_eq!(
            decide_tick_focus(
                true,
                false,
                false,
                false,
                false,
                false,
                Some(true),
                false,
                TickClickIn::None,
                false,
            ),
            TickFocusDecision {
                next_keys: false,
                next_press_in: false,
                action: TickFocusAction::None,
            }
        );
        // Pointer over another top-level while SolidCraft holds keys preserves keys until Backplane click.
        assert_eq!(
            decide_tick_focus(
                true,
                true,
                true,
                false,
                true,
                false,
                Some(true),
                true,
                TickClickIn::None,
                false,
            ),
            TickFocusDecision {
                next_keys: true,
                next_press_in: false,
                action: TickFocusAction::None,
            }
        );
    }

    #[test]
    fn solidcraft_fullscreen_toggles_when_active_and_resets_when_inactive() {
        assert!(next_solidcraft_full(false, None, true));
        assert!(!next_solidcraft_full(true, None, true));
        assert!(next_solidcraft_full(false, Some(true), true));
        assert!(!next_solidcraft_full(true, Some(false), true));
        assert!(!next_solidcraft_full(true, None, false));
        assert!(!next_solidcraft_full(false, Some(true), false));
    }
}
