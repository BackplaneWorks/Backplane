//! Screens the hub's screen model describes beyond generic nodes. Each one reads a plain JSON shape the
//! hub already sends the phones and answers with a view action; no decision lives here.
pub mod asks;
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
pub mod order;
pub mod picker;
pub mod pad;
pub mod padview;
pub mod palette;
pub mod panels;
pub mod subview;
pub mod place;
pub mod plot;
pub mod projects;
pub mod queue;
pub mod plotview;
pub mod select;
pub mod siderow;
pub mod sidebox;
pub mod star;
pub mod strip;
pub mod term;
pub mod settings;
pub mod accounts;
pub mod combo;
pub mod timeline;
pub mod tools;
pub mod viewer;
mod glue;

use gpui::{Context, Entity, FocusHandle};
use std::collections::HashMap;

/// State the screens keep between frames (input boxes, caches); the hub owns everything else.
pub struct Screens {
    /// the regions drawn by their own cached views (part.rs), and the controls they drew
    pub parts: crate::part::Parts,
    pub voice: voice::Voice,
    pub acct: accounts::Accounts,
    pub setup: onboard::Setup,
    pub setup_focus: FocusHandle,
    pub ask_inputs: HashMap<String, Entity<composer::Composer>>,
    pub attacher: attach::Attacher,
    /// the dev build strip (devbar.rs), none when no build is being made
    pub hubbar: hubbar::State,
    /// the window's hub when it is another computer's (`hubs::elsewhere`): (short name, address); the window marks it the whole time
    pub elsewhere: Option<(String, String)>,
    /// the GitHub star suggestion while the hub says it is out and undecided (star.rs)
    pub star: Option<star::Offer>,
    pub devbar: Option<devbar::Bar>, /// when Restart now was first clicked (the second click confirms)
    pub devbar_confirm: Option<std::time::Instant>,
    /// The suggestion popup's highlighted row, and the rows it was for (a new set starts at the first).
    /// the right-click menu on a sidebar row, when open (ctxmenu.rs)
    pub ctx: Option<ctxmenu::Menu>,
    pub scroll_logged: Option<std::time::Instant>,
    pub popup_ix: usize, pub popup_key: String, pub popup_scroll: gpui::ScrollHandle,
    pub drafts: drafts::Writer,
    /// The place a thread was left at, the row on top and the px scrolled into it, so a thread left scrolled up keeps it. One left at its end has none.
    pub offsets: HashMap<String, place::RowPlace>,
    /// A place not in the rows held yet, only the newest page being there after a relaunch: history is read until it comes.
    pub seeking: Option<place::RowPlace>,
    /// The text of the reply last seen streaming: its message, when it lands, takes its place without the arrival fade (ui.rs).
    pub streamed: String,
    /// When the history page on its way was asked for: another waits for its answer, or for 10 s if that never comes.
    pub paging: Option<std::time::Instant>,
    pub reg: timeline::Reg,
    pub projects: projects::Projects,
    pub files: filesui::FilesUi,
    /// The 3D page (fed the hub's solid frames) and the Browser page (polls the thread's frame while it is shown); made when first needed.
    pub solid: Option<Entity<crate::viewers::solid::SolidPage>>,
    pub browser: Option<(String, Entity<crate::viewers::browser::BrowserPage>)>,
    /// The kind asked for by a rail press, shown at once until the hub's picture answers; and when a plot was asked for (click to first paint).
    pub rail_pending: Option<String>,
    pub rail_at: Option<std::time::Instant>,
    /// What the bots/rooms screens want focused when they open (a field name), once; and the page last seen open.
    pub focus_field: Option<String>,
    pub bots_open: String,
    pub plot_click: Option<std::time::Instant>,
    pub plot: plot::Store,
    pub plotview: plotview::PlotView,
    /// The cameras of a Split's schematic and board panes (`pv(1)`, `pv(2)`).
    pub split_pv: [plotview::PlotView; 2],
    /// The canvas a `kicad.info` request is answering for.
    pub pick_slot: Option<usize>,
    /// The last viewer state sent to the hub (`viewer.state`), so only a change is reported.
    pub last_state: String,
    pub picker_open: bool,
    pub pick: picker::Pick,
    pub panels: panels::Panels,
    /// The thread header's measured groups (head.rs): one row or two.
    pub head: head::Head,
    pub palette: palette::Palette,
    /// The floating terminal (term.rs): its screens, and whether it is on show.
    pub term: term::Term,
    /// The drawing pad, while it is open.
    pub pad: Option<padview::PadView>,
    /// The picture shown large over the window (a click on an image in the thread), with its name.
    pub lightbox: Option<(std::sync::Arc<gpui::Image>, String)>,
    /// Reference words the hub has offered in the popup (chips in the draft).
    pub known_refs: Vec<String>,
    /// Esc shut the suggestion popup on this draft: it stays shut until the text changes.
    pub popup_off: Option<String>,
    pub popup_shut: bool,
    /// the Context chip's panel
    pub ctx_open: bool,
    pub fields: HashMap<String, settings::FieldSlot>,
    pub settings_focus: FocusHandle,
    pub focus_settings: bool,
    pub settings_was_open: bool,
    pub settings_ui: settings::SettingsUi,
    pub parsed: timeline::ParseCache,
    /// Rows that came in after the thread was shown, by id: when each was first drawn (for the arrival fade); `shown_at` is when the thread was shown.
    pub born: HashMap<String, std::time::Instant>,
    pub shown_at: std::time::Instant,
    pub images: images::Images,
    pub img_poll: bool,
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
    /// The sidebar the window folded to its strip for want of room (viewer::fit) was unfolded anyway; its fold button clears it.
    pub side_force: bool,
    /// Whether the sidebar was drawn open on the last frame (fold and unfold act on what is shown).
    pub side_shown: bool,
}

impl Screens {
    /// The camera state of canvas `slot`: 0 the Board/Schematic page, 1 and 2 the Split's schematic and board.
    pub fn pv(&mut self, slot: usize) -> &mut plotview::PlotView { match slot { 0 => &mut self.plotview, n => &mut self.split_pv[(n - 1).min(1)] } }
    pub fn new<T: 'static>(_cx: &mut Context<T>, target: Option<attach::Target>, drafts_path: std::path::PathBuf, saved: HashMap<String, String>) -> Self {
        let imp = improvements::Imp::new(&drafts_path, target.clone());
        Self { parts: Default::default(), side_force: false, side_shown: true, hubbar: Default::default(), elsewhere: None, star: None, term: term::Term::new(_cx.focus_handle()), pad: None, voice: Default::default(), setup: Default::default(), setup_focus: _cx.focus_handle(), acct: Default::default(), ctx_open: false, split_pv: Default::default(), pick_slot: None, last_state: String::new(), ask_inputs: HashMap::new(), attacher: attach::Attacher::new(target), drafts: drafts::Writer::start(drafts_path, saved), offsets: HashMap::new(), seeking: None, streamed: String::new(), paging: None, reg: Default::default(), projects: Default::default(), files: Default::default(), solid: None, browser: None, rail_pending: None, rail_at: None, focus_field: None, bots_open: String::new(), plot_click: None, plot: Default::default(), plotview: Default::default(), picker_open: false, pick: Default::default(), panels: Default::default(), head: Default::default(), palette: Default::default(), lightbox: None, known_refs: vec![], popup_off: None, popup_shut: false, fields: HashMap::new(), settings_focus: _cx.focus_handle(), focus_settings: false, settings_was_open: false, settings_ui: Default::default(), parsed: Default::default(), born: HashMap::new(), shown_at: std::time::Instant::now(), images: Default::default(), img_poll: false, await_draft: None, pending_draft: None, draft_timer: false, imp, devbar: None, devbar_confirm: None, ctx: None, scroll_logged: None, popup_ix: 0, popup_key: String::new(), popup_scroll: Default::default(), held: Default::default(), hold_threads: vec![], hold_projects: vec![], root_focus: _cx.focus_handle() }
    }
}
