//! Screens the hub's screen model describes beyond generic nodes. Each one reads a plain JSON shape the
//! hub already sends the phones and answers with a view action; no decision lives here.
pub mod thread_context;
pub mod asks;
pub mod review;
pub mod popover;
pub mod sections;
pub mod find;
pub mod kicad_update;
pub mod handoff;
pub mod thought;
pub mod halt;
pub mod unanswered;
pub mod provider_notice;
pub mod google;
pub mod migrate;
pub mod voice;
pub mod voice_mic;
pub mod attach;
pub mod desk;
pub mod devbar;
pub mod hubbar;
pub mod head;
pub mod hubpick;
pub mod ctxmenu;
pub mod badge;
pub mod machines;
pub mod bar;
pub mod hints;
pub mod blink;
pub mod bots;
pub mod swarm;
pub mod drafts;
pub mod first;
pub mod onboard;
pub mod composer;
pub mod edit;
pub mod histbar;
pub mod filesui;
pub mod history;
pub mod images;
pub mod pair;
pub mod improv_model;
pub mod improvements;
pub mod capture;
pub mod md;
pub mod copyable;
pub mod linkroute;
pub mod order;
pub mod picker;
pub mod dial;
pub mod pad;
pub mod usage;
pub mod padview;
pub mod palette;
pub mod panels;
pub mod panestate;
pub mod subview;
pub mod place;
pub mod plot;
pub mod projects;
pub mod queue;
pub mod unsent;
pub mod plotview;
pub mod follow;
pub mod sheets;
pub mod select;
pub mod siderow;
pub mod sidebox;
pub mod star;
pub mod tmnotice;
pub mod dlbar;
pub mod strip;
pub mod term;
pub mod settings;
pub mod cards;
pub mod catpick;
pub mod accounts;
pub mod combo;
pub mod timeline;
pub mod xprobe;
pub mod tools;
pub mod pane_header;
pub mod tile_menu;
pub mod viewer;
pub mod tiles;
pub(crate) mod tile_pages;
pub mod keysheet;
pub mod mgmt;
pub mod mgmt_bom;
pub mod mgmt_fab;
pub mod mgmt_plan;
pub mod mgmt_plan_canvas;
pub mod kcomment;
pub mod mgmt_checks;
pub mod mgmt_life;
pub mod planmode;
mod glue;

use gpui::{Context, Entity, FocusHandle};
use std::collections::HashMap;

/// State the screens keep between frames (input boxes, caches); the hub owns everything else.
pub struct Screens {
    pub tile_menu: tile_menu::State,
    /// the regions drawn by their own cached views (part.rs), and the controls they drew
    pub parts: crate::part::Parts,
    pub voice: voice::Voice,
    /// streamer mode's state in this window (streamer.rs)
    pub streamer: crate::streamer::Ui,
    pub acct: accounts::Accounts,
    pub setup: onboard::Setup,
    pub setup_focus: FocusHandle,
    /// the keyboard shortcuts sheet is open (keysheet.rs)
    pub keysheet: bool,
    /// the halt cards (halt.rs) whose Details are open, by entry id: what the window shows, nothing the hub keeps
    pub halt_open: std::collections::HashSet<String>,
    /// the comment window over the KiCad pane (Ctrl+;), while it is open
    pub kcomment: Option<kcomment::KComment>,
    pub ask_inputs: HashMap<String, Entity<composer::Composer>>,
    pub plan_expanded: std::collections::HashSet<String>,
    pub plan_revising: Option<String>,
    pub attacher: attach::Attacher,
    /// the dev build strip (devbar.rs), none when no build is being made
    pub hubbar: hubbar::State,
    /// the window's hub when it is another computer's (`hubs::elsewhere`): (short name, address); the window marks it the whole time
    pub elsewhere: Option<(String, String)>,
    /// the home hub's offer being carried out, and the connection on the home hub (hubpick.rs)
    pub offers: crate::screens::hubpick::Offers,
    /// the GitHub star suggestion while the hub says it is out and undecided (star.rs)
    pub star: Option<star::Offer>,
    /// the one-time telemetry notice while the hub offers it (tmnotice.rs), and the token already told `shown`
    pub tm_notice: Option<tmnotice::Notice>, pub tm_shown: String,
    /// the one-time "Backplane no longer pushes automatically" notice while the hub offers it (#1171, the same strip)
    pub gp_notice: Option<tmnotice::Notice>,
    pub devbar: Option<devbar::Bar>, /// when Restart now was first clicked (the second click confirms)
    pub devbar_confirm: Option<std::time::Instant>,
    /// The suggestion popup's highlighted row, and the rows it was for (a new set starts at the first).
    /// the right-click menu on a sidebar row, when open (ctxmenu.rs)
    pub ctx: Option<ctxmenu::Menu>,
    pub scroll_logged: Option<std::time::Instant>,
    pub popup_ix: usize, pub popup_key: String, pub popup_scroll: gpui::ScrollHandle,
    pub drafts: drafts::Writer,
    /// messages sent while the window had no hub (unsent.rs): shown in the queue strip, sent in order once it is back
    pub unsent: unsent::Unsent,
    /// The place a thread was left at, the row on top and the px scrolled into it, so a thread left scrolled up keeps it. One left at its end has none.
    pub offsets: HashMap<String, place::RowPlace>,
    /// A place not in the rows held yet, only the newest page being there after a relaunch: history is read until it comes.
    pub seeking: Option<place::RowPlace>,
    /// The text of the reply last seen streaming: its message, when it lands, takes its place without the arrival fade (ui.rs).
    pub streamed: String,
    /// the person's message whose Restore to before this was pressed: its confirm shows under it (one at a time; "" for none)
    pub restore_ask: String,
    /// When the history page on its way was asked for: another waits for its answer, or for 10 s if that never comes.
    pub paging: Option<std::time::Instant>,
    pub reg: timeline::Reg,
    pub projects: projects::Projects,
    pub files: filesui::FilesUi,
    /// Downloads from the hub, drawn as a strip along the bottom (dlbar.rs).
    pub downloads: dlbar::Downloads,
    /// The 3D page (fed the hub's solid frames) and the Browser page (polls the thread's frame while it is shown); made when first needed.
    pub solids: HashMap<String, Entity<crate::viewers::solid::SolidPage>>,
    pub solid_slot: Option<String>,
    pub solid_order: std::collections::VecDeque<String>,
    tile_pages: HashMap<usize, (String, Entity<crate::viewers::solid::SolidPage>)>,
    pub retired_solids: Vec<Entity<crate::viewers::solid::SolidPage>>,
    pub browser: Option<(String, Entity<crate::viewers::browser::BrowserPage>)>,
    /// The embedded KiCad editor (viewers/kicad_pane.rs) while the person edits, and every `editor.kicad.<digest>` the hub published (JSON).
    pub kicad: Option<Entity<crate::viewers::kicad_pane::KicadPane>>,
    /// where the person was when a reserved key in KiCad opened an overlay (the keyboard goes back to KiCad when it is dismissed there)
    pub kicad_loan: Option<crate::viewers::kicad_pane::Loan>,
    /// the Backplane KiCad update offered at Edit, and its install (kicad_update.rs)
    pub kicad_update: kicad_update::Update,
    pub kicad_infos: HashMap<String, serde_json::Value>,
    /// Projects whose KiCad (or the editor frame on show) the person closed: an automatic attach leaves them alone until the person's own
    /// Edit (viewers/kicad_pane.rs `kicad_open`)
    pub kicad_closed: std::collections::HashSet<String>,
    /// (project, KiCad pid) whose adrift pane was attached again (`Backplane::kicad_adrift`): once per KiCad, never a loop
    pub kicad_adrift_tried: std::collections::HashSet<(String, u64)>,
    /// macOS: the window's host pushes in flight and the latest wanted (viewers/kicad_pane.rs HostSends), shared with its panes
    pub host_sends: std::rc::Rc<std::cell::RefCell<crate::viewers::kicad_pane::HostSends>>,
    /// The management panels beside the design (mgmt.rs): the hub's `mgmt.*` infos, the shown project, each panel's own state.
    pub mgmt: mgmt::Mgmt,
    /// Cross-probing from the chat and the viewers (screens/xprobe.rs): the thread's design names, a zoom waiting for the hub's probe.
    pub xprobe: xprobe::State,
    /// the editor drawn in a window of its own (popped out), and the pane's full-size mode (the sidebar and the thread give it their room)
    pub kicad_window: Option<gpui::AnyWindowHandle>,
    pub kicad_full: bool,
    /// the embedded editor takes the whole window: only its own strip (the editor tabs, Exit full screen) is left of Backplane
    pub kicad_screen: bool,
    /// KiCad comments already posted (project#seq), so a repeated info frame posts nothing twice
    pub kicad_comments_seen: std::collections::HashSet<String>,
    /// The kind asked for by a rail press, shown at once until the hub's picture answers; and when a plot was asked for (click to first paint).
    pub(crate) rail_labels: crate::ui::RailLabels,
    pub rail_scroll: gpui::ScrollHandle,
    pub rail_last_lit: Option<String>,
    pub rail_pending: Option<String>,
    pub rail_at: Option<std::time::Instant>,
    /// Last `GPUI_METRIC {"event":"rail",...}` payload emitted under `BACKPLANE_GPUI_RECTS=1` (change-only).
    pub rail_metric: Option<serde_json::Value>,
    /// What the bots/rooms screens want focused when they open (a field name), once; and the page last seen open.
    pub focus_field: Option<String>,
    pub bots_open: String,
    /// a rail press that opens a drawing: when, and the plot on show then (the metric waits for another one)
    pub plot_click: Option<(std::time::Instant, String)>,
    pub plot: plot::Store,
    pub plots: PlotSlots,
    pub dock: tiles::Dock,
    /// The canvas a `kicad.info` request is answering for.
    pub pick_slot: Option<usize>,
    /// The last viewer state sent to the hub (`viewer.state`), so only a change is reported.
    pub last_state: String,
    /// Whether this window's page holds the embedded KiCad now and the frame it shows ("PCB", "SCH", ...), for viewer.state (Follow agent in KiCad)
    pub kicad_shown: (bool, String),
    pub picker_open: bool,
    pub pick: picker::Pick,
    pub panels: panels::Panels,
    /// The thread header's measured groups (head.rs): one row or two.
    pub head: head::Head,
    pub palette: palette::Palette,
    pub find: find::Find,
    /// The floating terminal (term.rs): its screens, and whether it is on show.
    pub term: term::Term,
    /// The drawing pad, while it is open.
    pub pad: Option<padview::PadView>,
    /// The picture shown large over the window (a click on an image in the thread), with its name.
    pub lightbox: Option<images::Lightbox>,
    /// Reference words the hub has offered in the popup (chips in the draft).
    pub known_refs: Vec<String>,
    /// Esc shut the suggestion popup on this draft: it stays shut until the text changes.
    pub popup_off: Option<String>,
    pub popup_shut: bool,
    /// the Context chip's panel (screens/usage.rs), and the chip's bounds as last drawn (the panel floats from it)
    pub ctx_open: bool,
    pub thread_context: thread_context::Inspector,
    pub ctx_chip: std::rc::Rc<std::cell::Cell<gpui::Bounds<gpui::Pixels>>>,
    pub box_at: std::rc::Rc<std::cell::Cell<gpui::Bounds<gpui::Pixels>>>,
    pub fields: HashMap<String, settings::FieldSlot>,
    pub settings_focus: FocusHandle,
    pub focus_settings: bool,
    pub settings_was_open: bool,
    pub settings_ui: settings::SettingsUi,
    pub parsed: timeline::ParseCache,
    /// Rows that came in after the thread was shown, by id: when each was first drawn (for the arrival fade); `shown_at` is when the thread was shown.
    pub born: HashMap<String, std::time::Instant>,
    /// The history pages read into the rows held (`history::page_mark`), since the rows last started over: none is asked for twice.
    pub pages_read: std::collections::HashSet<String>,
    pub shown_at: std::time::Instant,
    pub images: images::Images,
    /// the picture a right click on an image in a message is about, for the menu's Copy image
    pub ctx_img: Option<std::sync::Arc<gpui::Image>>,
    /// the picture a right click is about, for the menu's Save image as…
    pub ctx_save: Option<images::Save>,
    pub img_poll: bool,
    /// The project page's scroll (a README's `#` links move it).
    pub home_scroll: gpui::ScrollHandle,
    /// a project's page: its list, the project it is for, and the README block each anchor is in
    pub home_list: gpui::ListState, pub home_list_key: String, pub home_doc_w: f32, pub home_anchors: std::collections::HashMap<String, usize>,
    /// A popup row was pressed: the hub's reply carries the new draft; take it once.
    pub await_draft: Option<(String, std::time::Instant)>,
    /// The newest draft not yet sent to the hub, and whether the 30 ms timer that sends it is running.
    pub pending_draft: Option<String>,
    pub draft_timer: bool,
    pub imp: improvements::Imp,
    /// What a held modifier shows this frame (keys::hint_mode), and the threads and projects its digits go to.
    pub held: crate::keys::Held,
    pub hold_threads: Vec<String>,
    pub hold_projects: Vec<String>,
    /// The window's own keyboard: where Esc puts it from the message box (a bare key then presses a rail button), on an element of no size that is never clicked.
    pub root_focus: FocusHandle,
    /// the sidebar's keyboard (`Sidebar`: Left and Right fold a project row), taken by a press on a project row, and the project row under the pointer
    pub side_focus: FocusHandle, pub side_hover: Option<String>,
    /// an Open in KiCad (embedded) from a file link: the file's name and when, until the viewer shows it (ctxmenu.rs `edit_due`)
    pub edit_after: Option<(String, std::time::Instant)>,
    /// The sidebar the window folded to its strip for want of room (viewer::fit) was unfolded anyway; its fold button clears it.
    pub side_force: bool,
    /// Whether the sidebar was drawn open on the last frame (fold and unfold act on what is shown).
    pub side_shown: bool,
    /// Where the composer's "runs on desktop" line was drawn last frame: its machine picker opens from it (ctxmenu `anchored`).
    pub runs_at: std::rc::Rc<std::cell::Cell<gpui::Bounds<gpui::Pixels>>>,
}

/// Cameras and input state belong to the selected thread and its mounted slots.
#[derive(Default)]
pub struct PlotSlots {
    pub owner: String,
    slots: HashMap<usize, plotview::PlotView>,
    empty: plotview::PlotView,
    pub measure: bool,
    tiles: HashMap<String, TileSlot>,
    next_tile: usize,
    previous: Option<(String, HashMap<usize, plotview::PlotView>, HashMap<String, TileSlot>)>,
}

struct TileSlot {
    slot: usize,
    source: serde_json::Value,
    keys: Vec<String>,
    pending: Vec<String>,
}

/// The same source can have different display keys as focus moves. Compare
/// the presenter's source descriptor, independent of focus and split geometry.
fn tile_source(tile: &serde_json::Value) -> serde_json::Value {
    serde_json::json!([tile["kind"], tile["source"], tile["version"], tile["overlay"],
        tile["error"], tile["identity"]["missing"]])
}

impl PlotSlots {
    pub fn sync(&mut self, owner: &str, open: bool, split: bool) {
        if self.owner != owner && !owner.is_empty() {
            let returning = self.previous.take().filter(|(id, ..)| id == owner);
            let (slots, tiles) = returning.map(|(_, slots, tiles)| (slots, tiles)).unwrap_or_default();
            let old = std::mem::replace(&mut self.slots, slots);
            let old_tiles = std::mem::replace(&mut self.tiles, tiles);
            for tile in self.tiles.values_mut() { tile.keys.clear(); tile.pending.clear(); }
            self.previous = Some((std::mem::replace(&mut self.owner, owner.into()), old, old_tiles));
        }
        if open {
            self.slot(0);
            if split { self.slot(1); self.slot(2); }
        }
    }

    pub fn sync_tiles(&mut self, viewer: &serde_json::Value) {
        let tiles = viewer["tiles"].as_object().filter(|_| viewer::is_tiled(viewer));
        self.tiles.retain(|id, tile| {
            let keep = tiles.and_then(|t| t.get(id)).is_some_and(|t| tile_source(t) == tile.source);
            if !keep { self.slots.remove(&tile.slot); }
            keep
        });
        for (id, row) in tiles.into_iter().flatten() {
            if !self.tiles.contains_key(id) {
                self.next_tile = self.next_tile.max(2) + 1;
                let slot = self.next_tile;
                self.slot(slot);
                self.tiles.insert(id.clone(), TileSlot { slot, source: tile_source(row), keys: Vec::new(), pending: Vec::new() });
            }
        }
    }

    pub fn tile_slot(&self, id: &str) -> Option<usize> { Some(self.tiles.get(id)?.slot) }
    pub fn tile_for_slot(&self, slot: usize) -> Option<&str> {
        self.tiles.iter().find_map(|(id, t)| (t.slot == slot).then_some(id.as_str()))
    }
    pub fn tile_keys(&self, id: &str) -> &[String] {
        self.tiles.get(id).map_or(&[], |t| t.keys.as_slice())
    }
    pub fn tile_frame_count(&self, id: &str) -> u64 {
        self.tile_slot(id).map_or(0, |s| self[s].presented)
    }
    pub fn watches(&self, key: &str) -> bool {
        self.tiles.values().any(|t| t.keys.iter().chain(&t.pending).any(|k| k == key))
    }

    fn bind_watch(&mut self, watch: &serde_json::Value) -> bool {
        if watch["thread"].as_str() != Some(self.owner.as_str()) || self.tiles.is_empty() { return false; }
        for (id, tile) in &mut self.tiles {
            if tile_source(&watch["picture"][id]) != tile.source { continue; }
            let keys: Vec<_> = watch["tiles"][id].as_array().into_iter().flatten()
                .filter_map(|key| key.as_str().filter(|k| !k.is_empty()).map(str::to_owned)).collect();
            if !tile.keys.is_empty() && tile.keys.first() != keys.first() {
                self.slots.insert(tile.slot, plotview::PlotView { measure: self.measure, ..Default::default() });
            }
            if watch["pending"] == true {
                tile.keys.retain(|k| keys.contains(k));
                tile.pending = keys;
            } else {
                tile.pending.clear();
                tile.keys = keys;
            }
        }
        true
    }

    pub fn mounted(&mut self) -> impl Iterator<Item = &mut plotview::PlotView> {
        self.slots.values_mut()
    }

    fn slot(&mut self, id: usize) -> &mut plotview::PlotView {
        self.slots.entry(id).or_insert_with(|| plotview::PlotView { measure: self.measure, ..Default::default() })
    }

    pub fn views(&mut self) -> (&mut plotview::PlotView, Option<[&mut plotview::PlotView; 2]>) {
        self.slot(0);
        let [main, sch, pcb] = self.slots.get_disjoint_mut([&0, &1, &2]);
        (main.unwrap(), sch.zip(pcb).map(|(s,b)| [s,b]))
    }
}

impl std::ops::Index<usize> for PlotSlots {
    type Output = plotview::PlotView;
    fn index(&self, slot: usize) -> &Self::Output { self.slots.get(&slot).unwrap_or(&self.empty) }
}
impl std::ops::IndexMut<usize> for PlotSlots {
    fn index_mut(&mut self, slot: usize) -> &mut Self::Output { self.slot(slot) }
}

#[cfg(test)]
mod slot_tests {
    use super::*;
    #[test]
    fn dock_watch_binds_only_the_current_thread_and_tile_source() {
        use serde_json::json;
        let mut slots = PlotSlots::default();
        let mut picture = json!({"left":{"kind":"board","source":"a"},
            "right":{"kind":"3d","source":"a"}});
        slots.sync("thread", true, false);
        slots.sync_tiles(&json!({"tiled":true,"tiles":picture}));
        let slot = slots.tile_slot("right").unwrap();
        assert_ne!(slots.tile_slot("left"), Some(slot));
        slots[slot].hands = 2;
        let mut watch = json!({"thread":"thread","picture":picture,"pending":true,
            "tiles":{"left":["plot"],"right":["model","plot"]}});
        slots.bind_watch(&watch);
        assert!(slots.watches("model"));
        assert!(slots.tile_keys("right").is_empty());
        watch["pending"] = json!(false);
        slots.bind_watch(&watch);
        assert_eq!(slots.tile_keys("right"), ["model", "plot"]);
        picture["right"]["key"] = json!("focused-key");
        slots.sync_tiles(&json!({"tiled":true,"tiles":picture}));
        assert_eq!(slots[slot].hands, 2);
        picture["right"]["source"] = json!("b");
        slots.sync_tiles(&json!({"tiled":true,"tiles":picture}));
        slots.bind_watch(&watch);
        assert_ne!(slots.tile_slot("right"), Some(slot));
        assert!(slots.tile_keys("right").is_empty());
        assert!(!slots.watches("model"));
        assert!(slots.watches("plot"));
        slots.sync("other", true, false);
        slots.sync_tiles(&json!({"tiled":true,"tiles":picture}));
        assert!(!slots.bind_watch(&watch));
        assert!(slots.tile_keys("left").is_empty());
        slots.sync_tiles(&json!({"tiled":false,"tiles":picture}));
        assert!(slots.tile_slot("left").is_none());
    }

    #[test] fn plot_state_lasts_for_one_open_viewer_and_measure_lasts_for_the_window() {
        let mut slots = PlotSlots { measure: true, ..Default::default() };
        slots.sync("thread", true, true);
        slots[1].hands = 3;
        slots[2].hands = 4;
        slots.sync("thread", true, false);
        assert_eq!((slots[1].hands, slots[2].hands), (3, 4));
        slots.sync("other", true, true);
        assert_eq!(slots[2].hands, 0);
        assert!(slots[0].measure && slots[1].measure);
        slots.sync("other", false, false);
        assert!(slots.slots.contains_key(&0));
        slots.sync("thread", true, true);
        assert_eq!((slots[1].hands, slots[2].hands), (3, 4));
    }
}

impl Screens {
    /// The camera state of canvas `slot`: 0 the Board/Schematic page, 1 and 2 the Split's schematic and board.
    pub fn tile_frame(&self, id: &str) -> Option<&plot::Frame> {
        self.plots.tile_keys(id).iter().find_map(|key| self.plot.frame_for(key))
    }
    pub fn tile_presented(&self, id: &str, cx: &gpui::App) -> u64 {
        self.tile_solid(id).map_or_else(|| self.plots.tile_frame_count(id), |page| page.read(cx).painted_count())
    }
    pub fn tile_solid(&self, id: &str) -> Option<&Entity<crate::viewers::solid::SolidPage>> {
        let (key, page) = self.tile_pages.get(&self.plots.tile_slot(id)?)?;
        self.plots.tile_keys(id).contains(key).then_some(page)
    }
    pub fn tile_solids(&self) -> impl Iterator<Item = &Entity<crate::viewers::solid::SolidPage>> {
        self.tile_pages.values().map(|(_, page)| page)
    }
    pub fn active_solid(&self) -> Option<&Entity<crate::viewers::solid::SolidPage>> {
        self.solid_slot.as_ref().and_then(|key| self.solids.get(key))
    }
    pub fn pv(&mut self, slot: usize) -> &mut plotview::PlotView { &mut self.plots[slot] }
    pub fn new<T: 'static>(_cx: &mut Context<T>, target: Option<attach::Target>, drafts_path: std::path::PathBuf, saved: HashMap<String, String>) -> Self {
        let imp = improvements::Imp::new(&drafts_path, target.clone());
        let unsent = unsent::Unsent::open(drafts_path.with_file_name("gpui-unsent.json"));
        Self { tile_menu: Default::default(), parts: Default::default(), side_force: false, side_shown: true, runs_at: Default::default(), hubbar: Default::default(), elsewhere: None, offers: Default::default(), star: None, tm_notice: None, tm_shown: String::new(), gp_notice: None, term: term::Term::new(_cx.focus_handle()), pad: None, voice: Default::default(), streamer: Default::default(), setup: Default::default(), setup_focus: _cx.focus_handle(), keysheet: false, halt_open: Default::default(), kcomment: None, acct: Default::default(), ctx_open: false, thread_context: Default::default(), ctx_chip: Default::default(), box_at: Default::default(), pick_slot: None, last_state: String::new(), kicad_shown: (false, String::new()), ask_inputs: HashMap::new(), attacher: attach::Attacher::new(target), drafts: drafts::Writer::start(drafts_path, saved), unsent, offsets: HashMap::new(), seeking: None, streamed: String::new(), restore_ask: String::new(), paging: None, reg: Default::default(), projects: Default::default(), files: Default::default(), downloads: Default::default(), solids: Default::default(), solid_slot: None, solid_order: Default::default(), tile_pages: Default::default(), retired_solids: Vec::new(), browser: None, kicad: None, kicad_loan: None, kicad_update: kicad_update::Update::from_env(), kicad_infos: HashMap::new(), kicad_closed: Default::default(), kicad_adrift_tried: Default::default(), host_sends: Default::default(), mgmt: Default::default(), kicad_window: None, kicad_full: false, kicad_screen: false, kicad_comments_seen: Default::default(), xprobe: Default::default(), rail_scroll: Default::default(), rail_last_lit: None, rail_pending: None, rail_at: None, rail_metric: None, focus_field: None, bots_open: String::new(), plot_click: None, plot: Default::default(), dock: Default::default(), plots: Default::default(), picker_open: false, pick: Default::default(), panels: Default::default(), head: Default::default(), palette: Default::default(), find: Default::default(), lightbox: None, known_refs: vec![], popup_off: None, popup_shut: false, fields: HashMap::new(), settings_focus: _cx.focus_handle(), focus_settings: false, settings_was_open: false, settings_ui: Default::default(), parsed: Default::default(), born: HashMap::new(), pages_read: Default::default(), shown_at: std::time::Instant::now(), images: Default::default(), ctx_img: None, ctx_save: None, img_poll: false, home_scroll: Default::default(), home_list: gpui::ListState::new(0, gpui::ListAlignment::Top, gpui::px(1200.)), home_list_key: String::new(), home_doc_w: 600., home_anchors: Default::default(), await_draft: None, pending_draft: None, draft_timer: false, imp, devbar: None, devbar_confirm: None, ctx: None, scroll_logged: None, popup_ix: 0, popup_key: String::new(), popup_scroll: Default::default(), held: Default::default(), hold_threads: vec![], hold_projects: vec![], root_focus: _cx.focus_handle(), side_focus: _cx.focus_handle(), side_hover: None, edit_after: None, rail_labels: Default::default(), plan_expanded: Default::default(), plan_revising: None }
    }
}
