//! Screens the hub's screen model describes beyond generic nodes. Each one reads a plain JSON shape the
//! hub already sends the phones (src/mobile/screen.bend) and answers with a view action; no decision lives here.
pub mod asks;
pub mod attach;
pub mod drafts;
pub mod composer;
pub mod edit;
pub mod images;
pub mod md;
pub mod select;
pub mod timeline;
mod glue;

use gpui::{Context, Entity};
use std::collections::HashMap;

/// State the screens keep between frames (input boxes, caches); the hub owns everything else.
pub struct Screens {
    pub ask_inputs: HashMap<String, Entity<composer::Composer>>,
    pub attacher: attach::Attacher,
    pub drafts: drafts::Writer,
    /// Timeline scroll offset per thread, so a thread left scrolled up keeps its place.
    pub offsets: HashMap<String, f32>,
    pub reg: timeline::Reg,
    pub parsed: timeline::ParseCache,
    pub images: images::Images,
    pub img_poll: bool,
    /// A popup row was pressed: the hub's reply carries the new draft; take it once.
    pub await_draft: Option<(String, std::time::Instant)>,
    /// The newest draft not yet sent to the hub, and whether the 30 ms timer that sends it is running.
    pub pending_draft: Option<String>,
    pub draft_timer: bool,
}

impl Screens {
    pub fn new<T: 'static>(_cx: &mut Context<T>, target: Option<attach::Target>, drafts_path: std::path::PathBuf, saved: HashMap<String, String>) -> Self {
        Self { ask_inputs: HashMap::new(), attacher: attach::Attacher::new(target), drafts: drafts::Writer::start(drafts_path, saved), offsets: HashMap::new(), reg: Default::default(), parsed: Default::default(), images: Default::default(), img_poll: false, await_draft: None, pending_draft: None, draft_timer: false }
    }
}
