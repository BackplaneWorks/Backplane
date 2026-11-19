//! The shared empty, loading and error state for every viewer page's body (Board, Schematic, 3D, Gerbers, Split, Mech, Review, Browser, Diff).
//! One pattern: centred in the pane body (never touching the header bar or pane geometry), one plain line saying what is happening or what
//! is missing, an optional second dim line with the next step, elapsed seconds from 1 s while loading (one-shot 1 s timer re-armed only by
//! a visible loading render and scoped to the viewer part, zero repaints at rest), and a Retry button only for retryable errors.
use crate::{i18n::{tf, trs}, theme, viewers::raster::Backdrop, Backplane};
use gpui::{prelude::*, *};
use std::{collections::{HashMap, HashSet}, time::{Duration, Instant}};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Empty, Loading, Error }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaneState {
    pub kind: Kind,
    pub line: String,
    pub next: String,
    pub retry: bool,
}

impl PaneState {
    pub fn empty(line: String, next: String) -> Self { Self { kind: Kind::Empty, line, next, retry: false } }
    pub fn loading(line: String) -> Self { Self { kind: Kind::Loading, line, next: String::new(), retry: false } }
    pub fn error(line: String, retry: bool) -> Self { Self { kind: Kind::Error, line, next: String::new(), retry } }
}

/// The loading line: base text through the catalog, elapsed seconds from 1 s, and the hub's estimate when 2 s or more.
pub fn loading_line(text: &str, secs: u64, about_ms: u64) -> String {
    let mut s = trs(text);
    if secs >= 1 { s.push(' '); s.push_str(&tf("gpui.c.secs", &[("secs", secs.into())])); }
    if about_ms >= 2000 { s.push(' '); s.push_str(&tf("gpui.c.about.secs", &[("secs", ((about_ms + 500) / 1000).into())])); }
    s
}

/// The loading text for a viewer page before its first frame or stage arrives.
pub fn loading_for(open: &str, gerbers: bool) -> String {
    match open {
        "board" if gerbers => trs("Loading the Gerbers…"),
        "board" => trs("Loading the board…"),
        "schematic" => trs("Loading the schematic…"),
        "gerbers" => trs("Loading the Gerbers…"),
        "3d" | "mech" => trs("Loading the model…"),
        "split" => trs("Loading the views…"),
        _ => trs("Opening…"),
    }
}

/// Whether a hub `none` reason names a missing project file (an empty state) rather than a failure.
pub fn is_empty_none(why: &str) -> bool {
    matches!(why, "No board in this project yet" | "No schematic in this project yet" | "No 3D model yet" | "No Gerber files in this project yet" | "No board to compare these Gerbers against.") // i18n-ok: matching the hub's canonical English none strings
        || why.starts_with("@bp.file-missing:")
}

/// Fallback for an older hub that sends no `retryable` flag on a plot/3D `none` frame: current hubs send `retryable` explicitly in the frame.
pub fn is_retryable_none(why: &str) -> bool {
    if why.is_empty() || is_empty_none(why) { return false }
    let low = why.to_ascii_lowercase();
    !["too large", "not found", "no such file", "does not exist", "unsupported", "permission denied"].iter().any(|p| low.contains(p)) // i18n-ok: fallback for older hubs without a retryable flag
}

/// Classify a plot frame's `none` reason (Board, Schematic, Gerbers, or a Split sub-pane) into a PaneState.
/// `retryable` comes from the hub frame's `retryable` flag; `None` falls back to `is_retryable_none` for older hubs.
pub fn classify_none(why: &str, retryable: Option<bool>, open: &str, gerbers: bool) -> PaneState {
    if why == "No board in this project yet" {
        return PaneState::empty(trs("No board in this project yet"), trs("Ask an agent to lay one out."));
    }
    if why == "No schematic in this project yet" {
        return PaneState::empty(trs("No schematic in this project yet"), trs("Ask an agent to draw one."));
    }
    if why == "No Gerber files in this project yet" || (gerbers && why.is_empty()) {
        return PaneState::empty(trs("No Gerber files in this project yet"), trs("Ask an agent to export fabrication files into mfg/."));
    }
    if why == "No board to compare these Gerbers against." {
        return PaneState::empty(trs("No board to compare these Gerbers against."), trs("Add a board to this project to compare them."));
    }
    if why == "@bp.file-unread" {
        return PaneState::error(trs("This version of the file could not be read."), retryable.unwrap_or(true));
    }
    if let Some(n) = why.strip_prefix("@bp.file-missing:") {
        return PaneState::empty(tf("gpui.b.hist.file.missing", &[("name", n.into())]), String::new());
    }
    if why.is_empty() {
        return match open {
            "schematic" => PaneState::empty(trs("No schematic in this project yet"), trs("Ask an agent to draw one.")),
            _ => PaneState::empty(trs("No board in this project yet"), trs("Ask an agent to lay one out.")),
        };
    }
    PaneState::error(trs(why), retryable.unwrap_or_else(|| is_retryable_none(why)))
}

/// Classify a 3D/Mech page's `note` (when no image is shown) into a PaneState.
/// `retryable` comes from the hub frame's `retryable` flag; `None` falls back to `is_retryable_none` for older hubs.
pub fn classify_solid_note(note: &str, failed: bool, retryable: Option<bool>) -> PaneState {
    if note == "No board in this project yet" {
        return PaneState::empty(trs("No board in this project yet"), trs("Ask an agent to lay one out."));
    }
    if note == "No 3D model yet" || note.is_empty() {
        return PaneState::empty(trs("No 3D model yet"), trs("Ask an agent to lay out a board."));
    }
    if is_empty_none(note) {
        return PaneState::empty(trs(note), String::new());
    }
    PaneState::error(crate::viewers::solid::note_words(note), failed && retryable.unwrap_or_else(|| is_retryable_none(note)))
}

/// A Mech page with no parts (the hub's `mech.say` and `mech.note`).
pub fn mech_empty(say: &str, note: &str) -> PaneState {
    let line = if say.is_empty() { trs("No mechanical parts in this project yet") } else { trs(say) };
    let next = if note.is_empty() && say.is_empty() { trs("Ask an agent to model one, or add a STEP file to mech/.") } else if note.is_empty() { String::new() } else { trs(note) };
    PaneState::empty(line, next)
}

// ---- pure one-shot elapsed-time clock keyed per (thread, source) ----

#[derive(Default)]
pub struct LoadingClock {
    thread: String,
    starts: HashMap<String, Instant>,
    seen: HashSet<String>,
    armed: bool,
}

impl LoadingClock {
    /// Start a viewer render frame for `thread`. Switching threads drops prior thread's start times.
    pub fn begin_frame(&mut self, thread: &str) {
        if self.thread != thread {
            self.thread.clear();
            self.thread.push_str(thread);
            self.starts.clear();
        }
        self.seen.clear();
    }
    /// Record that `source` is visibly loading at `now`; returns elapsed whole seconds for `(thread, source)`.
    pub fn track(&mut self, source: &str, now: Instant) -> u64 {
        let key = format!("{}\x1f{source}", self.thread);
        self.seen.insert(key.clone());
        let start = *self.starts.entry(key).or_insert(now);
        now.saturating_duration_since(start).as_secs()
    }
    /// Finish a viewer render frame: drops any source that settled (no longer loading this frame).
    /// Returns true if at least one loading pane is visible and a one-shot 1 s tick should be armed.
    pub fn end_frame(&mut self) -> bool {
        self.starts.retain(|k, _| self.seen.contains(k));
        !self.seen.is_empty()
    }
    /// Tear down all loading state (pane closed or hidden).
    pub fn clear(&mut self) {
        self.starts.clear();
        self.seen.clear();
    }
    /// Whether any loading pane was visible on the last frame.
    pub fn is_loading(&self) -> bool { !self.seen.is_empty() }
    /// Claim the one-shot timer slot if loading and not already armed.
    pub fn try_arm(&mut self) -> bool {
        if self.armed || !self.is_loading() { return false }
        self.armed = true;
        true
    }
    /// Release the one-shot timer slot when it fires; returns true only if a loading pane is still active.
    pub fn fire(&mut self) -> bool {
        self.armed = false;
        self.is_loading()
    }
    /// Record a visibly loading pane for `source` in this window's current thread; returns elapsed seconds.
    pub fn track_now(&mut self, source: &str) -> u64 { self.track(source, Instant::now()) }
    /// Called at the end of `viewer::page` on this window's own `LoadingClock`: drops settled sources and arms a one-shot 1 s timer
    /// scoped to `Backplane::viewer_part` on THIS entity ONLY if at least one loading pane rendered this frame.
    pub fn end_frame_and_arm(&mut self, cx: &mut Context<Backplane>) {
        if self.end_frame() && self.try_arm() {
            cx.spawn(async move |weak, cx| {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let _ = weak.update(cx, |app, cx| {
                    let now = Instant::now();
                    app.screens.plot.retrying.retain(|_, since| !retry_expired(*since, now));
                    if app.screens.plotview.loading.fire() {
                        app.screens.parts.tick(Backplane::viewer_part, Duration::ZERO, cx);
                    }
                });
            }).detach();
        }
    }
}

/// Client-side cooldown (1 s) matching the presenter's retry guard so a throttled press cannot be sent.
pub const RETRY_COOLDOWN: Duration = Duration::from_secs(1);
/// Bounded wait (15 s) for a Retry: if no frame for that source arrives, return to the retained error with Retry enabled and stop ticking.
pub const RETRY_TIMEOUT: Duration = Duration::from_secs(15);

/// Whether the Retry control is enabled given the last press time `last` and current time `now`.
pub fn retry_ready(last: Option<Instant>, now: Instant) -> bool {
    last.is_none_or(|t| now.saturating_duration_since(t) >= RETRY_COOLDOWN)
}
/// Whether an in-flight Retry started at `since` has exceeded the 15 s bounded wait.
pub fn retry_expired(since: Instant, now: Instant) -> bool {
    now.saturating_duration_since(since) >= RETRY_TIMEOUT
}

// ---- shared GPUI elements (centred inside the pane body only) ----

/// A standard Retry button for `panestate::view` (on `theme` colours); disabled during the 1 s cooldown.
pub fn retry_btn(cx: &mut Context<Backplane>, light: bool, id: String, enabled: bool, on_retry: impl Fn(&mut Backplane, &mut Context<Backplane>) + 'static) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let base = div().id(SharedString::from(id)).px_3().py_1().text_size(crate::scale::tpx(12.)).border_1().border_color(c("line")).child(trs("Retry"));
    if !enabled {
        return base.text_color(c("faint")).bg(c("panel")).into_any_element();
    }
    base.text_color(c("text")).bg(c("raise")).cursor_pointer().hover(|s| s.bg(c("panel")))
        .on_click(cx.listener(move |this, _, _, cx| { on_retry(this, cx); cx.notify(); })).into_any_element()
}

/// Centred pane-body state on the viewer's `view` background (Board, Schematic, Gerbers, Mech empty, Review, Browser, Diff, Opening).
pub fn view(light: bool, st: &PaneState, retry: Option<AnyElement>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let primary: Hsla = match st.kind { Kind::Error => c("bad").into(), Kind::Empty => c("text").into(), Kind::Loading => c("dim").into() };
    body_box(primary, c("dim").into(), st, retry).bg(c("view")).into_any_element()
}

/// Centred pane-body state on the 3D viewer's backdrop gradient (3D and Mech model loading/empty/error).
pub fn view_on_backdrop(back: Backdrop, st: &PaneState, retry: Option<AnyElement>) -> AnyElement {
    let hex = |c: [u8; 3]| rgb(((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32);
    let light_bg = (back.top[0] as u32 + back.top[1] as u32 + back.top[2] as u32) > 384;
    let primary: Hsla = match st.kind {
        Kind::Error => theme::color(light_bg, "bad").into(),
        Kind::Empty => if light_bg { rgb(0x1f2430).into() } else { rgb(0xd8dde6).into() },
        Kind::Loading => if light_bg { rgb(0x4b5563).into() } else { rgb(0xadb5c4).into() },
    };
    let secondary: Hsla = if light_bg { rgb(0x5b6472).into() } else { rgb(0x8a93a3).into() };
    body_box(primary, secondary, st, retry)
        .bg(gpui::linear_gradient(180., gpui::linear_color_stop(hex(back.top), 0.), gpui::linear_color_stop(hex(back.bottom), 1.)))
        .into_any_element()
}

fn body_box(primary: Hsla, secondary: Hsla, st: &PaneState, retry: Option<AnyElement>) -> Div {
    div().size_full().flex_1().min_h_0().flex().flex_col().items_center().justify_center().gap_2().px_6()
        .child(div().max_w(px(360.)).text_center().text_size(crate::scale::tpx(13.)).text_color(primary).child(st.line.clone()))
        .when(!st.next.is_empty(), |d| d.child(div().max_w(px(360.)).text_center().text_size(crate::scale::tpx(12.)).text_color(secondary).child(st.next.clone())))
        .when(st.retry, |d| d.children(retry))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test] fn loading_line_formats_elapsed_and_estimate_and_translates() {
        assert_eq!(loading_line("Loading the board…", 0, 0), "Loading the board…");
        assert_eq!(loading_line("Loading the board…", 1, 0), "Loading the board… 1 s");
        assert_eq!(loading_line("Loading the board…", 4, 11_400), "Loading the board… 4 s (about 11 s)");
        assert_eq!(loading_line("Loading the board…", 2, 1_500), "Loading the board… 2 s", "under 2 s estimate is omitted");
        crate::i18n::with_lang("ja", || {
            assert_eq!(loading_line("Loading the board…", 3, 0), "基板を読み込み中… 3秒");
        });
    }

    #[test] fn classify_none_separates_empty_states_from_retryable_and_permanent_errors() {
        let b = classify_none("No board in this project yet", Some(false), "board", false);
        assert_eq!((b.kind, b.retry), (Kind::Empty, false));
        assert!(b.next.contains("lay one out"), "{b:?}");

        let s = classify_none("No schematic in this project yet", Some(false), "schematic", false);
        assert_eq!((s.kind, s.retry), (Kind::Empty, false));
        assert!(s.next.contains("draw one"), "{s:?}");

        let g = classify_none("No Gerber files in this project yet", Some(false), "board", true);
        assert_eq!((g.kind, g.retry), (Kind::Empty, false));
        assert!(g.next.contains("mfg/"), "{g:?}");

        let nb = classify_none("No board to compare these Gerbers against.", Some(false), "board", true);
        assert_eq!((nb.kind, nb.retry), (Kind::Empty, false));
        assert!(nb.next.contains("Add a board"), "{nb:?}");

        let u = classify_none("@bp.file-unread", Some(true), "board", false);
        assert_eq!((u.kind, u.retry), (Kind::Error, true));

        let m = classify_none("@bp.file-missing:a.kicad_pcb", Some(false), "board", false);
        assert_eq!((m.kind, m.retry), (Kind::Empty, false));

        // Explicit hub retryable flag overrides the fallback heuristic
        let explicit_no = classify_none("Transient-looking message", Some(false), "board", false);
        assert_eq!((explicit_no.kind, explicit_no.retry), (Kind::Error, false));
        let explicit_yes = classify_none("File not found", Some(true), "board", false);
        assert_eq!((explicit_yes.kind, explicit_yes.retry), (Kind::Error, true));

        // Older hub sending no flag falls back to is_retryable_none
        for perm in ["This plot is 40 MB, too large to show here.", "File not found", "Unsupported format", "Permission denied"] {
            let p = classify_none(perm, None, "board", false);
            assert_eq!((p.kind, p.retry), (Kind::Error, false), "permanent error gets no Retry: {perm}");
        }

        let err = classify_none("Invalid geometry reference. Retry to reload this view.", None, "board", false);
        assert_eq!((err.kind, err.retry), (Kind::Error, true));
    }

    #[test] fn classify_solid_note_gives_empty_without_retry_and_error_with_retry() {
        let e = classify_solid_note("No board in this project yet", false, Some(false));
        assert_eq!((e.kind, e.retry), (Kind::Empty, false));
        assert!(e.next.contains("lay one out"));

        let n = classify_solid_note("No 3D model yet", false, Some(false));
        assert_eq!((n.kind, n.retry), (Kind::Empty, false));
        assert!(n.next.contains("board"));

        let bad = classify_solid_note("Incomplete model", true, Some(true));
        assert_eq!((bad.kind, bad.retry), (Kind::Error, true));

        let perm = classify_solid_note("Transient error", true, Some(false));
        assert_eq!((perm.kind, perm.retry), (Kind::Error, false));

        let fallback = classify_solid_note("File not found", true, None);
        assert_eq!((fallback.kind, fallback.retry), (Kind::Error, false));
    }

    #[test] fn mech_empty_has_plain_line_and_next_step() {
        let m = mech_empty("No mechanical parts in this project yet", "Ask an agent to model one, or add a STEP file to mech/.");
        assert_eq!((m.kind, m.retry), (Kind::Empty, false));
        assert!(!m.line.is_empty() && m.next.contains("STEP file"));
        let d = mech_empty("", "");
        assert_eq!((d.kind, d.retry), (Kind::Empty, false));
        assert!(!d.line.is_empty() && d.next.contains("STEP file"));
    }

    #[test] fn loading_clock_tears_down_on_close_thread_switch_and_settle() {
        let mut c = LoadingClock::default();
        let t0 = Instant::now();
        // Visible loading frame arms a single one-shot tick
        c.begin_frame("t1");
        assert_eq!(c.track("plot:0:board:0|/p|", t0), 0);
        assert!(c.end_frame());
        assert!(c.try_arm());
        assert!(!c.try_arm(), "already armed: one-shot only");

        // Second render 2 s later reports 2 s elapsed
        c.begin_frame("t1");
        assert_eq!(c.track("plot:0:board:0|/p|", t0 + Duration::from_secs(2)), 2);
        assert!(c.end_frame());
        assert!(c.fire(), "still loading when timer fires");

        // Teardown on settle: frame renders without tracking any loading pane
        c.begin_frame("t1");
        assert!(!c.end_frame(), "settled: no tick armed");
        assert!(!c.is_loading());
        assert!(!c.try_arm());
        // Re-loading the same source starts fresh at 0 s
        c.begin_frame("t1");
        assert_eq!(c.track("plot:0:board:0|/p|", t0 + Duration::from_secs(10)), 0);
        assert!(c.end_frame());
        assert!(c.try_arm());

        // Teardown on close: pending timer fire returns false and does not notify
        c.clear();
        assert!(!c.is_loading());
        assert!(!c.fire(), "closed mid-load: timer fire must be a no-op");

        // Teardown on thread switch: starts from thread 1 do not bleed into thread 2
        c.begin_frame("t1");
        assert_eq!(c.track("plot:0:board:0|/p|", t0), 0);
        assert!(c.end_frame());
        assert!(c.try_arm());
        c.begin_frame("t2");
        assert_eq!(c.track("plot:0:board:0|/p|", t0 + Duration::from_secs(5)), 0, "new thread starts at 0 s");
        assert!(c.end_frame());
        // Switching to a thread whose pane is shut clears and cancels the timer
        c.clear();
        assert!(!c.fire());
    }

    #[test] fn retry_cooldown_and_timeout_bound_trying_again() {
        let t0 = Instant::now();
        assert!(retry_ready(None, t0));
        assert!(!retry_ready(Some(t0), t0));
        assert!(!retry_ready(Some(t0), t0 + Duration::from_millis(999)));
        assert!(retry_ready(Some(t0), t0 + Duration::from_secs(1)));

        assert!(!retry_expired(t0, t0));
        assert!(!retry_expired(t0, t0 + Duration::from_secs(14)));
        assert!(retry_expired(t0, t0 + Duration::from_secs(15)));
    }
}
