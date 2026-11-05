//! Screens the hub's screen model describes beyond generic nodes. Each one reads a plain JSON shape the
//! hub already sends the phones (src/mobile/screen.bend) and answers with a view action; no decision lives here.
pub mod asks;
pub mod attach;
pub mod bar;
pub mod blink;
pub mod bots;
pub mod drafts;
pub mod composer;
pub mod edit;
pub mod images;
pub mod improv_model;
pub mod improvements;
pub mod capture;
pub mod md;
pub mod picker;
pub mod place;
pub mod plot;
pub mod projects;
pub mod plotview;
pub mod select;
pub mod strip;
pub mod settings;
pub mod timeline;
pub mod tools;
pub mod viewer;
mod glue;

use gpui::{Context, Entity, FocusHandle};
use std::collections::HashMap;

/// State the screens keep between frames (input boxes, caches); the hub owns everything else.
pub struct Screens {
    pub ask_inputs: HashMap<String, Entity<composer::Composer>>,
    pub attacher: attach::Attacher,
    pub drafts: drafts::Writer,
    /// Timeline scroll offset per thread, so a thread left scrolled up keeps its place. A thread left at its end has none: it opens at its end.
    pub offsets: HashMap<String, f32>,
    /// A remembered place was just put back and the new thread's first picture has not landed: the scroll range is still the last thread's.
    pub restoring: bool,
    pub reg: timeline::Reg,
    pub menu_open: bool,
    pub projects: projects::Projects,
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
    pub picker_open: bool,
    pub fields: HashMap<String, settings::FieldSlot>,
    pub settings_focus: FocusHandle,
    pub focus_settings: bool,
    pub settings_was_open: bool,
    pub settings_ui: settings::SettingsUi,
    pub parsed: timeline::ParseCache,
    pub images: images::Images,
    pub img_poll: bool,
    /// A popup row was pressed: the hub's reply carries the new draft; take it once.
    pub await_draft: Option<(String, std::time::Instant)>,
    /// The newest draft not yet sent to the hub, and whether the 30 ms timer that sends it is running.
    pub pending_draft: Option<String>,
    pub draft_timer: bool,
    pub imp: improvements::Imp,
}

impl Screens {
    pub fn new<T: 'static>(_cx: &mut Context<T>, target: Option<attach::Target>, drafts_path: std::path::PathBuf, saved: HashMap<String, String>) -> Self {
        let imp = improvements::Imp::new(&drafts_path, target.clone());
        Self { ask_inputs: HashMap::new(), attacher: attach::Attacher::new(target), drafts: drafts::Writer::start(drafts_path, saved), offsets: HashMap::new(), restoring: false, reg: Default::default(), menu_open: false, projects: Default::default(), solid: None, browser: None, rail_pending: None, rail_at: None, focus_field: None, bots_open: String::new(), plot_click: None, plot: Default::default(), plotview: Default::default(), picker_open: false, fields: HashMap::new(), settings_focus: _cx.focus_handle(), focus_settings: false, settings_was_open: false, settings_ui: Default::default(), parsed: Default::default(), images: Default::default(), img_poll: false, await_draft: None, pending_draft: None, draft_timer: false, imp }
    }
}
