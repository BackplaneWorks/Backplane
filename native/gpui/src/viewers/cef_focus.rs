//! Pure focus-handoff state machine and click-tracking logic for embedded windowed CEF (`cef.rs`, `cef_page.rs`, `cef_native.rs`).
//! Compiled unconditionally so unit tests run with or without `--features cef` and across all targets.
#![cfg_attr(not(feature = "cef"), allow(dead_code))]

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// How long a recorded mouse press in a browser view can explain a subsequent CEF focus callback.
pub const PRESS_SPAN: Duration = Duration::from_millis(400);

static FOCUS_SEQ: AtomicU64 = AtomicU64::new(1);

/// Returns a strictly increasing sequence number for ordering CEF callbacks against host focus changes.
pub fn next_focus_seq() -> u64 {
    FOCUS_SEQ.fetch_add(1, Ordering::SeqCst)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusCallback {
    SetFocus,
    GotFocus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusSourceKind {
    System,
    Navigation,
}

/// Outcome of a Win32 `WM_MOUSEACTIVATE` dispatch reported by `WH_CALLWNDPROCRET`.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseActivateOutcome {
    /// `MA_ACTIVATE` / `MA_NOACTIVATE`: button-down will follow via `WH_MOUSE`.
    DeliverDown,
    /// `MA_ACTIVATEANDEAT`: activates the window and eats the button-down.
    ActivateAndEat,
    /// `MA_NOACTIVATEANDEAT`: neither activates nor delivers the button-down.
    NoActivateAndEat,
}

/// Result of querying a platform press watcher for a recent click in a watched native browser window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PressProbe {
    /// Watched window received a qualifying press (carries the press-time sequence number).
    Pressed(u64),
    /// Window is watched (or temporarily gated after blur until button release), with no eligible press.
    NotPressed,
    /// No press watcher covers this target; callers may fall back to querying live pointer state.
    Unwatched,
}

#[derive(Clone, Copy, Debug)]
struct PressRecord {
    target: u64,
    msg: u32,
    at: Instant,
    seq: u64,
    used_set: bool,
    used_got: bool,
    confirmed: bool,
}

#[derive(Clone, Copy, Debug)]
struct ActivateFrame {
    target: u64,
    msg: u32,
    seq: u64,
    down_paired: bool,
}

#[derive(Clone, Copy, Debug)]
struct PendingActivateDown {
    target: u64,
    msg: u32,
    seq: u64,
}

/// Tracks mouse presses delivered to embedded browser windows so paired
/// `on_set_focus` and `on_got_focus` callbacks for the same click both recognize it once,
/// carrying the press-time sequence number, decoupling Win32 `WM_MOUSEACTIVATE` + button-down pairing
/// from `PRESS_SPAN` via a LIFO activation stack and per-`(target, msg)` `WH_CALLWNDPROCRET` records,
/// and gating unwatched targets across blur until the held mouse button releases.
#[derive(Debug, Default)]
pub struct PressTracker {
    presses: Vec<PressRecord>,
    activate_stack: Vec<ActivateFrame>,
    pending_activate_downs: Vec<PendingActivateDown>,
    unwatched_blocked: Vec<u64>,
    unwatched_block_gen: u64,
}

impl PressTracker {
    pub const fn new() -> Self {
        Self {
            presses: Vec::new(),
            activate_stack: Vec::new(),
            pending_activate_downs: Vec::new(),
            unwatched_blocked: Vec::new(),
            unwatched_block_gen: 0,
        }
    }

    fn upsert_press(&mut self, target: u64, msg: u32, at: Instant, seq: u64, confirmed: bool) {
        self.presses.retain(|rec| {
            !(rec.target == target && rec.msg == msg)
                && (!rec.confirmed || at.saturating_duration_since(rec.at) <= PRESS_SPAN)
                && (self.activate_stack.iter().any(|f| f.target == rec.target && f.msg == rec.msg)
                    || self.pending_activate_downs.iter().any(|p| p.target == rec.target && p.msg == rec.msg))
        });
        self.presses.push(PressRecord {
            target,
            msg,
            at,
            seq,
            used_set: false,
            used_got: false,
            confirmed,
        });
    }

    fn bump_press(&mut self, target: u64, msg: u32) {
        if let Some(pos) = self.presses.iter().position(|rec| rec.target == target && rec.msg == msg) {
            let rec = self.presses.remove(pos);
            self.presses.push(rec);
        }
    }

    /// Records a mouse press on `target` at `at`, returning its press-time sequence number (`0` if `target == 0`).
    #[cfg_attr(windows, allow(dead_code))]
    pub fn record(&mut self, target: u64, at: Instant) -> u64 {
        if target == 0 {
            return 0;
        }
        let seq = next_focus_seq();
        self.record_with_seq(target, at, seq);
        seq
    }

    /// Deterministic variant of [`Self::record`] for tests.
    #[cfg_attr(windows, allow(dead_code))]
    pub fn record_with_seq(&mut self, target: u64, at: Instant, seq: u64) {
        if target == 0 {
            return;
        }
        self.upsert_press(target, 0, at, seq, true);
    }

    /// Records entry into a Win32 `WM_MOUSEACTIVATE` dispatch for `target` and button-down message `msg`.
    /// Reentrant dispatches push another LIFO frame onto `activate_stack` (reusing an active `(target, msg)` sequence
    /// without resetting existing press state) and keep nested clicks unconfirmed until `complete_activate` returns.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn record_activate(&mut self, target: u64, msg: u32, at: Instant) -> u64 {
        if target == 0 {
            return 0;
        }
        let seq = self
            .activate_stack
            .iter()
            .rev()
            .find(|f| f.target == target && f.msg == msg)
            .map_or_else(next_focus_seq, |f| f.seq);
        self.record_activate_with_seq(target, msg, at, seq)
    }

    /// Deterministic variant of [`Self::record_activate`] for tests.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn record_activate_with_seq(&mut self, target: u64, msg: u32, at: Instant, seq: u64) -> u64 {
        if target == 0 {
            return 0;
        }
        if let Some(existing) = self.activate_stack.iter().rev().find(|f| f.target == target && f.msg == msg) {
            let existing_seq = existing.seq;
            let down_paired = self
                .activate_stack
                .iter()
                .any(|f| f.target == target && f.msg == msg && f.down_paired);
            self.activate_stack.push(ActivateFrame {
                target,
                msg,
                seq: existing_seq,
                down_paired,
            });
            return existing_seq;
        }
        self.pending_activate_downs.retain(|p| !(p.target == target && p.msg == msg));
        self.activate_stack.push(ActivateFrame {
            target,
            msg,
            seq,
            down_paired: false,
        });
        self.upsert_press(target, msg, at, seq, false);
        seq
    }

    /// Handles return from a Win32 `WM_MOUSEACTIVATE` dispatch (`WH_CALLWNDPROCRET`).
    /// Pops only the top matching `(target, msg)` LIFO frame from `activate_stack`.
    /// When the last frame for `(target, msg)` unwinds: `NoActivateAndEat` discards an unpaired press,
    /// nested `ActivateAndEat` discards an unpaired press while an outer frame on `target` stays active,
    /// and surviving outcomes confirm `(target, msg)` at confirmation time (upgrading unconfirmed presses
    /// above any mid-dispatch unprompted `on_got_focus` cutoff) and queue `pending_activate_downs` on
    /// `DeliverDown` unless already paired.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn complete_activate(&mut self, target: u64, msg: u32, outcome: MouseActivateOutcome, at: Instant) -> bool {
        self.complete_activate_inner(target, msg, outcome, at, next_focus_seq)
    }

    /// Deterministic variant of [`Self::complete_activate`] for tests.
    #[cfg(test)]
    pub fn complete_activate_with_seq(
        &mut self,
        target: u64,
        msg: u32,
        outcome: MouseActivateOutcome,
        at: Instant,
        confirm_seq: u64,
    ) -> bool {
        self.complete_activate_inner(target, msg, outcome, at, || confirm_seq)
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn complete_activate_inner(
        &mut self,
        target: u64,
        msg: u32,
        outcome: MouseActivateOutcome,
        at: Instant,
        confirm_seq: impl FnOnce() -> u64,
    ) -> bool {
        let Some(idx) = self.activate_stack.iter().rposition(|f| f.target == target && f.msg == msg) else {
            return false;
        };
        let done = self.activate_stack.remove(idx);
        if self.activate_stack.iter().any(|f| f.target == done.target && f.msg == done.msg) {
            return false;
        }
        match outcome {
            MouseActivateOutcome::NoActivateAndEat if !done.down_paired => {
                self.presses.retain(|rec| !(rec.target == done.target && rec.msg == done.msg));
                return false;
            }
            MouseActivateOutcome::ActivateAndEat
                if !done.down_paired && self.activate_stack.iter().any(|f| f.target == done.target) =>
            {
                self.presses.retain(|rec| !(rec.target == done.target && rec.msg == done.msg));
                return false;
            }
            _ => {}
        }
        let mut confirmed_seq = None;
        let mut confirm_seq = Some(confirm_seq);
        for rec in &mut self.presses {
            if rec.target == done.target && rec.msg == done.msg {
                if !rec.confirmed {
                    let seq = *confirmed_seq.get_or_insert_with(|| (confirm_seq.take().unwrap())());
                    rec.confirmed = true;
                    rec.seq = seq;
                    rec.at = at;
                } else if confirmed_seq.is_none() {
                    confirmed_seq = Some(rec.seq);
                }
            }
        }
        if outcome == MouseActivateOutcome::DeliverDown && !done.down_paired {
            self.pending_activate_downs.retain(|p| !(p.target == done.target && p.msg == done.msg));
            self.pending_activate_downs.push(PendingActivateDown {
                target: done.target,
                msg: done.msg,
                seq: confirmed_seq.unwrap_or(done.seq),
            });
        }
        confirmed_seq.is_some()
    }

    /// Records a Win32 `WH_MOUSE` button-down `msg` on `target`.
    /// If it pairs with a matching `(target, msg)` activation record, consumes only that pairing
    /// regardless of elapsed time and returns the activation sequence WITHOUT recreating or resetting its press record.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn record_button_down(&mut self, target: u64, msg: u32, at: Instant) -> u64 {
        self.record_button_down_inner(target, msg, at, next_focus_seq)
    }

    /// Deterministic variant of [`Self::record_button_down`] for tests.
    #[cfg(test)]
    pub fn record_button_down_with_seq(&mut self, target: u64, msg: u32, at: Instant, seq: u64) -> u64 {
        self.record_button_down_inner(target, msg, at, || seq)
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn record_button_down_inner(
        &mut self,
        target: u64,
        msg: u32,
        at: Instant,
        alloc_seq: impl FnOnce() -> u64,
    ) -> u64 {
        if target == 0 {
            return 0;
        }
        if let Some(pos) = self.pending_activate_downs.iter().position(|p| p.target == target && p.msg == msg) {
            let seq = self.pending_activate_downs.swap_remove(pos).seq;
            self.bump_press(target, msg);
            return seq;
        }
        if let Some(top_seq) = self
            .activate_stack
            .iter()
            .rev()
            .find(|f| f.target == target && f.msg == msg && !f.down_paired)
            .map(|f| f.seq)
        {
            let mut paired_seq = top_seq;
            if let Some(pos) = self.presses.iter().position(|rec| rec.target == target && rec.msg == msg) {
                let mut rec = self.presses.remove(pos);
                if !rec.confirmed {
                    paired_seq = alloc_seq();
                    rec.confirmed = true;
                    rec.seq = paired_seq;
                    rec.at = at;
                } else {
                    paired_seq = rec.seq;
                }
                self.presses.push(rec);
            }
            for f in &mut self.activate_stack {
                if f.target == target && f.msg == msg {
                    f.down_paired = true;
                    f.seq = paired_seq;
                }
            }
            return paired_seq;
        }
        let seq = alloc_seq();
        self.upsert_press(target, msg, at, seq, true);
        seq
    }

    #[cfg(test)]
    pub fn has_pending_activate(&self) -> bool {
        !self.activate_stack.is_empty() || !self.pending_activate_downs.is_empty()
    }

    #[cfg(test)]
    pub fn has_pending_activate_for(&self, target: u64, msg: u32) -> bool {
        self.activate_stack.iter().any(|f| f.target == target && f.msg == msg)
            || self.pending_activate_downs.iter().any(|p| p.target == target && p.msg == msg)
    }

    /// Blocks `target` from using the unwatched held-button fallback until the mouse button is released.
    /// Returns `Some(generation)` only when transitioning from empty to non-empty so callers spawn one watcher.
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub fn block_unwatched_until_release(&mut self, target: u64) -> Option<u64> {
        if target == 0 {
            return None;
        }
        let was_empty = self.unwatched_blocked.is_empty();
        if !self.unwatched_blocked.contains(&target) {
            self.unwatched_blocked.push(target);
        }
        self.unwatched_block_gen = self.unwatched_block_gen.wrapping_add(1);
        was_empty.then_some(self.unwatched_block_gen)
    }

    /// Returns whether `target` is currently blocked from using the unwatched held-button fallback.
    pub fn is_unwatched_blocked(&self, target: u64) -> bool {
        self.unwatched_blocked.contains(&target)
    }

    /// Current generation of the unwatched held-button block set.
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub fn unwatched_block_gen(&self) -> u64 {
        self.unwatched_block_gen
    }

    /// Clears the unwatched held-button block set if no newer block arrived since `block_gen` was observed.
    /// Returns `true` when the block set is now empty (either already empty or cleared on matching generation).
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub fn clear_unwatched_release_block_if(&mut self, block_gen: u64) -> bool {
        if self.unwatched_blocked.is_empty() {
            return true;
        }
        if self.unwatched_block_gen == block_gen {
            self.unwatched_blocked.clear();
            true
        } else {
            false
        }
    }

    /// Returns the newest confirmed press-time sequence number if `target` has a fresh (`<= PRESS_SPAN`) press recorded strictly after `after_seq`.
    /// Unconfirmed in-flight Win32 `WM_MOUSEACTIVATE` records are excluded until `complete_activate` or `WH_MOUSE` button-down confirms them.
    pub fn fresh_press_seq_after(&self, target: u64, after_seq: u64, now: Instant) -> Option<u64> {
        if target == 0 {
            return None;
        }
        self.presses
            .iter()
            .filter(|rec| {
                rec.confirmed
                    && rec.target == target
                    && rec.seq > after_seq
                    && now.saturating_duration_since(rec.at) <= PRESS_SPAN
            })
            .map(|rec| rec.seq)
            .max()
    }

    /// Consumes a confirmed recorded press on `target` for `cb` if it is fresh (`<= PRESS_SPAN`) and unused for `cb`.
    /// Prefers an in-flight `activate_stack` frame's `(target, msg)` click over nested or older buttons on `target`,
    /// returning `None` while that active frame remains unconfirmed so pending activations cannot leak or fall back.
    /// `SetFocus` leaves `GotFocus` available for the same click; `GotFocus` spends both.
    /// Never deletes activation pairings, so a delayed paired button-down still deduplicates cleanly.
    pub fn consume(&mut self, target: u64, cb: FocusCallback, now: Instant) -> Option<u64> {
        if target == 0 {
            return None;
        }
        self.presses.retain(|rec| {
            rec.target != target || !rec.confirmed || now.saturating_duration_since(rec.at) <= PRESS_SPAN
        });
        let eligible = |rec: &PressRecord| -> bool {
            rec.confirmed
                && rec.target == target
                && match cb {
                    FocusCallback::SetFocus => !rec.used_set,
                    FocusCallback::GotFocus => !rec.used_got,
                }
        };
        let pos = if let Some(frame) = self.activate_stack.iter().rev().find(|f| f.target == target) {
            self.presses
                .iter()
                .rposition(|rec| rec.msg == frame.msg && eligible(rec))?
        } else {
            self.presses.iter().rposition(eligible)?
        };
        let rec = &mut self.presses[pos];
        match cb {
            FocusCallback::SetFocus => {
                rec.used_set = true;
                Some(rec.seq)
            }
            FocusCallback::GotFocus => {
                rec.used_got = true;
                rec.used_set = true;
                Some(rec.seq)
            }
        }
    }

    /// Clears any recorded press belonging to `target` (e.g. when returning keyboard focus or deactivating).
    /// Leaves activation pairings intact so an arriving paired `WM_*BUTTONDOWN` from this cancelled click is still absorbed.
    pub fn clear_target(&mut self, target: u64) {
        self.presses.retain(|rec| rec.target != target);
    }

    /// Clears a confirmed recorded press belonging to `target` only if its sequence is `<= max_seq`,
    /// preserving genuinely newer post-cutoff presses and unconfirmed in-flight Win32 `WM_MOUSEACTIVATE`
    /// records whose confirmation sequence has not yet been assigned.
    pub fn clear_target_up_to(&mut self, target: u64, max_seq: u64) {
        self.presses
            .retain(|rec| !(rec.target == target && rec.confirmed && rec.seq <= max_seq));
    }

    /// Drops any press, pending activation pairings, and unwatched release block belonging to `target` (used on destroy/unwatch).
    #[allow(dead_code)]
    pub fn drop_target(&mut self, target: u64) {
        self.presses.retain(|rec| rec.target != target);
        self.activate_stack.retain(|f| f.target != target);
        self.pending_activate_downs.retain(|p| p.target != target);
        self.unwatched_blocked.retain(|t| *t != target);
    }

    /// Clears any recorded press regardless of target (e.g. on macOS when a click hits outside watched CEF views).
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn clear_all(&mut self) {
        self.presses.clear();
    }
}

/// Coordinates X11 watcher barrier timeouts so pre-barrier `ButtonPress` events arriving after a
/// 100 ms caller timeout are suppressed until the watcher drains the matching barrier `ClientMessage`,
/// while handling watcher-before-caller early completion without leaking state.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Debug, Default)]
pub struct X11BarrierFences {
    active: Vec<(u64, Vec<u64>)>,
    completed_early: Vec<(u64, u64)>,
    watcher_dead: bool,
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
impl X11BarrierFences {
    pub const fn new() -> Self {
        Self {
            active: Vec::new(),
            completed_early: Vec::new(),
            watcher_dead: false,
        }
    }

    pub fn is_fenced(&self, target: u64) -> bool {
        target != 0 && self.active.iter().any(|(_, targets)| targets.contains(&target))
    }

    /// Finalizes a timed-out barrier on the caller (`clear_presses`) under the press mutex.
    /// Returns `Some(cutoff)` if the watcher already cleared `watched` and allocated `cutoff`
    /// before timing out the rendezvous, or installs an active fence until `finish_timeout_watcher`.
    pub fn finish_timeout_caller(&mut self, barrier_id: u64, watched: &[u64]) -> Option<u64> {
        if let Some(pos) = self.completed_early.iter().position(|(id, _)| *id == barrier_id) {
            return Some(self.completed_early.swap_remove(pos).1);
        }
        if !self.watcher_dead {
            let live: Vec<u64> = watched.iter().copied().filter(|&t| t != 0).collect();
            self.active.push((barrier_id, live));
        }
        None
    }

    /// Finalizes a timed-out barrier on the watcher (`complete_barrier`) when its `ClientMessage` arrives.
    /// Retires any active fence installed by the caller, or runs `on_early` to clear targets and allocate a
    /// cutoff before returning to wait for post-barrier events.
    pub fn finish_timeout_watcher(&mut self, barrier_id: u64, on_early: impl FnOnce() -> u64) {
        if let Some(pos) = self.active.iter().position(|(id, _)| *id == barrier_id) {
            self.active.swap_remove(pos);
        } else if !self.watcher_dead {
            let cutoff = on_early();
            self.completed_early.push((barrier_id, cutoff));
        }
    }

    /// Unfences `target` on container destroy while keeping the `barrier_id` slot intact so the
    /// watcher's matching `ClientMessage` still retires the barrier cleanly without leaking state.
    pub fn drop_target(&mut self, target: u64) {
        for (_, targets) in &mut self.active {
            targets.retain(|&t| t != target);
        }
    }

    /// Clears active fences after the watcher thread exits so future waits never fence, while
    /// keeping already-completed early cutoffs available for their callers to collect and remove.
    pub fn mark_watcher_dead(&mut self) {
        self.watcher_dead = true;
        self.active.clear();
    }
}

/// Decides whether a CEF focus callback was triggered by a person's click in the page and returns the click sequence.
/// Consults `probe` first and falls back to `button_down_in_page` only when `probe` returns `PressProbe::Unwatched`
/// (`SetFocus` with a non-system source never triggers or consumes a press).
pub fn callback_click_seq(
    windowed: bool,
    cb: FocusCallback,
    source: FocusSourceKind,
    fallback_seq: u64,
    probe: impl FnOnce() -> PressProbe,
    button_down_in_page: impl FnOnce() -> bool,
) -> Option<u64> {
    if !windowed || (cb == FocusCallback::SetFocus && source != FocusSourceKind::System) {
        return None;
    }
    match probe() {
        PressProbe::Pressed(seq) => Some(seq),
        PressProbe::NotPressed => None,
        PressProbe::Unwatched => button_down_in_page().then_some(fallback_seq),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TabFocusEvents {
    pub click_seq: u64,
    pub key_seq: u64,
    pub got_focus_seq: u64,
    pub took_focus_seq: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FocusIntent {
    /// Click or typing inside a specific tab grants GPUI focus to that tab.
    FocusPage { tab: u64, seq: u64 },
    /// Tabbing past a specific tab's last/first focusable element blurs back to the host only if that tab is still active.
    TabOut { tab: u64, seq: u64 },
    /// User/scripted "Hand back" is intentionally page-wide (matches `CefPage.taken = false` / `{"op": "handback"}`),
    /// always returning keyboard control from the entire browser viewer (including any newly opened popup,
    /// `origin`, and any stray `unprompted_got` tabs) back to the host unless superseded by a genuine post-handback click.
    HandBack { origin: Option<u64>, seq: u64 },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GpuiFocusChange {
    #[default]
    None,
    FocusPage,
    BlurPage,
}

/// One tab targeted by a focus yank/release together with that tab's own cutoff sequence,
/// so another tab's stray callback can never inflate this tab's press-clear cutoff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TabYank {
    pub tab: u64,
    pub cutoff_seq: u64,
}

/// Plan of GPUI and native focus adjustments emitted for a single `CefPage::render`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RenderFocusPlan {
    pub gpui: GpuiFocusChange,
    /// Non-active tabs whose CEF/native focus must be cleared without changing GPUI focus or focusing the host.
    pub release_cef_only: Vec<TabYank>,
    /// Tabs whose keyboard focus should be yanked back to the host window.
    pub yank_to_host: Vec<TabYank>,
    /// Active tab whose native/CEF focus should be restored after releasing background tabs.
    pub restore_active: Option<u64>,
    /// Cutoff sequence for GPUI's host blur (`BlurPage`), set only from `TabOut`/`HandBack`/`click_cutoff_seq`
    /// and never inflated by another tab's stray `got_focus` (`0` unless `gpui == GpuiFocusChange::BlurPage`).
    pub blur_cutoff_seq: u64,
}

impl RenderFocusPlan {
    /// Returns the cutoff associated with blurring or yanking `active_tab` during this render, if any,
    /// allowing post-yank reconciliation to reclaim focus when a click arrives after precheck.
    pub fn active_blur_or_yank_cutoff(&self, active_tab: Option<u64>) -> Option<u64> {
        let yank = active_tab.and_then(|tab| self.yank_to_host.iter().find(|y| y.tab == tab).map(|y| y.cutoff_seq));
        let blur = (self.gpui == GpuiFocusChange::BlurPage).then_some(self.blur_cutoff_seq);
        match (blur, yank) {
            (Some(b), Some(y)) => Some(b.max(y)),
            (Some(b), None) => Some(b),
            (None, Some(y)) => Some(y),
            (None, None) => None,
        }
    }

    /// Returns the active-tab click recovery cutoff whenever this render blurred GPUI or yanked any tab to host
    /// (including background-only yanks, without inflating the active tab's cutoff to a background tab's sequence).
    pub fn recovery_cutoff_for_active(&self, active_tab: Option<u64>, floor_cutoff: u64) -> Option<u64> {
        if let Some(cutoff) = self.active_blur_or_yank_cutoff(active_tab) {
            Some(cutoff.max(floor_cutoff))
        } else if !self.yank_to_host.is_empty() && active_tab.is_some() {
            Some(floor_cutoff)
        } else {
            None
        }
    }
}

fn push_tab_yank(list: &mut Vec<TabYank>, tab: u64, cutoff_seq: u64) {
    if let Some(existing) = list.iter_mut().find(|y| y.tab == tab) {
        existing.cutoff_seq = existing.cutoff_seq.max(cutoff_seq);
    } else {
        list.push(TabYank { tab, cutoff_seq });
    }
}

/// Coordinates tab-bound, sequence-ordered focus handoff between GPUI and embedded CEF windows across `pump()` and `render()` ticks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FocusHandoff {
    intent: Option<FocusIntent>,
    unprompted_got: Vec<(u64, u64)>,
    click_cutoff_seq: u64,
    deactivate_cutoff_seq: u64,
    handed_back: bool,
}

impl FocusHandoff {
    pub const fn new() -> Self {
        Self {
            intent: None,
            unprompted_got: Vec::new(),
            click_cutoff_seq: 0,
            deactivate_cutoff_seq: 0,
            handed_back: false,
        }
    }

    pub const fn recovery_floor_seq(&self) -> u64 {
        if self.click_cutoff_seq >= self.deactivate_cutoff_seq {
            self.click_cutoff_seq
        } else {
            self.deactivate_cutoff_seq
        }
    }

    const fn intent_seq(&self) -> u64 {
        match self.intent {
            None => 0,
            Some(FocusIntent::FocusPage { seq, .. })
            | Some(FocusIntent::TabOut { seq, .. })
            | Some(FocusIntent::HandBack { seq, .. }) => seq,
        }
    }

    fn record_unprompted(&mut self, tab: u64, seq: u64) {
        if let Some((_, existing)) = self.unprompted_got.iter_mut().find(|(t, _)| *t == tab) {
            *existing = (*existing).max(seq);
        } else {
            self.unprompted_got.push((tab, seq));
        }
    }

    fn clear_unprompted(&mut self, tab: u64) {
        self.unprompted_got.retain(|(t, _)| *t != tab);
    }

    /// Applies one tab's drained focus events during `CefPage::pump` or pre-render reconciliation.
    /// Returns `true` when qualifying user interaction in the active tab requires taking the page over.
    /// While `handed_back` is active, only a genuine post-cutoff click (`click_seq > click_cutoff_seq`)
    /// or explicit GPUI host focus (`on_host_focus`) restores `key_seq` takeover or replaces `HandBack`.
    pub fn on_pump_tab(&mut self, tab: u64, ev: TabFocusEvents, is_active: bool) -> bool {
        let click = if ev.click_seq > self.click_cutoff_seq { ev.click_seq } else { 0 };
        let key = if !self.handed_back && ev.key_seq > self.click_cutoff_seq { ev.key_seq } else { 0 };
        let person = click.max(key);
        let got = if ev.got_focus_seq > self.deactivate_cutoff_seq { ev.got_focus_seq } else { 0 };
        let took = if ev.took_focus_seq > self.click_cutoff_seq { ev.took_focus_seq } else { 0 };

        if !is_active {
            if got > 0 {
                self.record_unprompted(tab, got);
            }
            return false;
        }

        if took > person && took > 0 {
            if matches!(self.intent, Some(FocusIntent::HandBack { .. })) {
                self.record_unprompted(tab, took);
            } else if took >= self.intent_seq() {
                self.intent = Some(FocusIntent::TabOut { tab, seq: took });
            }
            if got > took {
                self.record_unprompted(tab, got);
            }
            return person > 0;
        }
        if person > 0 {
            if person >= self.intent_seq() {
                if click > 0 {
                    self.handed_back = false;
                }
                self.intent = Some(FocusIntent::FocusPage { tab, seq: person });
                self.clear_unprompted(tab);
            } else if got > self.intent_seq() {
                self.record_unprompted(tab, got);
            }
            return true;
        }
        if got > 0
            && !matches!(self.intent, Some(FocusIntent::FocusPage { tab: ptab, .. }) if ptab == tab)
        {
            self.record_unprompted(tab, got);
        }
        false
    }

    /// Returns the cutoff sequence if `render()` should check for an already-recorded post-cutoff click on
    /// `active_tab` (`!page_focused`, or `HandBack`/`TabOut` pending on `active_tab`), so a native click that
    /// arrives while GPUI focus is elsewhere still recovers even when CEF emits no focus callback.
    pub fn pending_press_cutoff_for_active(&self, active_tab: Option<u64>, page_focused: bool) -> Option<u64> {
        let active = active_tab?;
        match self.intent {
            Some(FocusIntent::HandBack { seq, .. }) => Some(self.click_cutoff_seq.max(seq)),
            Some(FocusIntent::TabOut { tab, seq }) if tab == active => Some(self.click_cutoff_seq.max(seq)),
            Some(FocusIntent::FocusPage { tab, .. }) if tab == active => None,
            _ if page_focused => None,
            _ => {
                if let Some((_, seq)) = self
                    .unprompted_got
                    .iter()
                    .find(|(t, seq)| *t == active && *seq > self.deactivate_cutoff_seq)
                {
                    Some(self.click_cutoff_seq.max(*seq))
                } else {
                    Some(self.recovery_floor_seq())
                }
            }
        }
    }

    /// Promotes a fresh, unconsumed post-cutoff native press on `tab` to `FocusPage` during render
    /// reconciliation so `render()` does not erase a click that occurred just before `pump()` ran.
    pub fn on_pending_click(&mut self, tab: u64, press_seq: u64) -> bool {
        if press_seq <= self.click_cutoff_seq || press_seq <= self.deactivate_cutoff_seq {
            return false;
        }
        match self.intent {
            Some(FocusIntent::HandBack { seq, .. }) if press_seq < seq => return false,
            Some(FocusIntent::TabOut { tab: itab, seq }) if itab == tab && press_seq < seq => return false,
            Some(FocusIntent::FocusPage { tab: itab, seq }) if itab == tab && press_seq < seq => return false,
            Some(FocusIntent::TabOut { tab: itab, seq }) if itab != tab => {
                self.record_unprompted(itab, seq);
            }
            _ => {}
        }
        self.handed_back = false;
        self.intent = Some(FocusIntent::FocusPage { tab, seq: press_seq });
        self.clear_unprompted(tab);
        true
    }

    /// Recovers focus after a post-cutoff native press on `tab` arrived after precheck
    /// (during `yank_to_host` or between `window.blur()` in `render()` and `on_blur` at `finish_frame`),
    /// lifting `handed_back` and queueing `FocusPage` so subsequent `got_focus` events do not trigger stray yanks.
    pub fn recover_click_after_yank(&mut self, tab: u64, press_seq: u64) -> bool {
        if press_seq <= self.click_cutoff_seq
            || press_seq <= self.deactivate_cutoff_seq
            || press_seq < self.intent_seq()
        {
            return false;
        }
        self.handed_back = false;
        self.intent = Some(FocusIntent::FocusPage { tab, seq: press_seq });
        self.clear_unprompted(tab);
        true
    }

    /// Notes that GPUI itself gave focus to the browser pane (`cx.on_focus`), lifting `handed_back`
    /// so subsequent keyboard interaction inside the page behaves normally.
    pub fn on_host_focus(&mut self) {
        self.handed_back = false;
    }

    /// Requests GPUI to blur the browser view (e.g. on "Hand back") on the next render, invalidating older clicks
    /// and blocking queued key takeovers while preserving stray `unprompted_got` tabs so all native containers yield.
    pub fn request_blur(&mut self, origin: Option<u64>, seq: u64) {
        self.click_cutoff_seq = self.click_cutoff_seq.max(seq);
        self.handed_back = true;
        self.intent = Some(FocusIntent::HandBack { origin, seq });
    }

    /// Invalidates older clicks when GPUI focus leaves the browser view while the host window stays active,
    /// while preserving already-recorded stray `unprompted_got` tabs so background native focus is still yanked.
    pub fn on_host_blur(&mut self, seq: u64) {
        self.click_cutoff_seq = self.click_cutoff_seq.max(seq);
        if matches!(self.intent, Some(FocusIntent::FocusPage { seq: pseq, .. }) if pseq <= seq) {
            self.intent = None;
        }
    }

    /// Invalidates pre-deactivation clicks and unprompted yanks when the host window loses OS activation.
    /// Pending explicit `HandBack` blur remains intact so GPUI internal focus still drops cleanly.
    pub fn on_window_deactivate(&mut self, seq: u64) {
        self.click_cutoff_seq = self.click_cutoff_seq.max(seq);
        self.deactivate_cutoff_seq = self.deactivate_cutoff_seq.max(seq);
        self.unprompted_got.clear();
        if !matches!(self.intent, Some(FocusIntent::HandBack { .. })) {
            self.intent = None;
        }
    }

    /// Resolves and consumes the pending focus plan during `CefPage::render`.
    pub fn take_render_plan(&mut self, active_tab: Option<u64>, page_focused: bool, window_active: bool) -> RenderFocusPlan {
        let intent = self.intent.take();
        let mut unprompted = std::mem::take(&mut self.unprompted_got);
        let mut plan = RenderFocusPlan::default();
        let mut final_page_focused = page_focused;
        let mut explicit_yanks: Vec<TabYank> = Vec::new();
        let mut blur_cutoff = 0u64;

        match intent {
            None => {}
            Some(FocusIntent::FocusPage { tab, .. }) => {
                if window_active && active_tab == Some(tab) && !page_focused {
                    plan.gpui = GpuiFocusChange::FocusPage;
                    final_page_focused = true;
                }
            }
            Some(FocusIntent::TabOut { tab, seq }) => {
                let tab_cutoff = self.click_cutoff_seq.max(seq);
                if active_tab == Some(tab) || active_tab.is_none() {
                    if page_focused {
                        plan.gpui = GpuiFocusChange::BlurPage;
                        blur_cutoff = tab_cutoff;
                        final_page_focused = false;
                    }
                    if window_active {
                        push_tab_yank(&mut explicit_yanks, tab, tab_cutoff);
                    }
                } else if window_active {
                    if page_focused {
                        push_tab_yank(&mut plan.release_cef_only, tab, tab_cutoff);
                        plan.restore_active = active_tab;
                    } else {
                        push_tab_yank(&mut explicit_yanks, tab, tab_cutoff);
                    }
                }
            }
            Some(FocusIntent::HandBack { origin, seq }) => {
                let hb_cutoff = self.click_cutoff_seq.max(seq);
                if page_focused {
                    plan.gpui = GpuiFocusChange::BlurPage;
                    blur_cutoff = hb_cutoff;
                    final_page_focused = false;
                }
                if window_active {
                    if let Some(tab) = origin {
                        push_tab_yank(&mut explicit_yanks, tab, hb_cutoff);
                    }
                    if let Some(tab) = active_tab {
                        push_tab_yank(&mut explicit_yanks, tab, hb_cutoff);
                    }
                }
            }
        }

        if !window_active {
            if plan.gpui == GpuiFocusChange::BlurPage {
                plan.blur_cutoff_seq = blur_cutoff;
            }
            return plan;
        }

        unprompted.sort_by_key(|(tab, _)| *tab);
        let mut stray: Vec<(u64, u64)> = Vec::with_capacity(unprompted.len());
        for (tab, seq) in unprompted {
            if seq > self.deactivate_cutoff_seq {
                if let Some((_, max_seq)) = stray.iter_mut().find(|(t, _)| *t == tab) {
                    *max_seq = (*max_seq).max(seq);
                } else {
                    stray.push((tab, seq));
                }
            }
        }

        if final_page_focused {
            let mut background_stole = false;
            for (tab, seq) in stray {
                if active_tab != Some(tab) {
                    push_tab_yank(&mut plan.release_cef_only, tab, self.click_cutoff_seq.max(seq));
                    background_stole = true;
                }
            }
            if background_stole {
                if let Some(active) = active_tab {
                    plan.restore_active = Some(active);
                } else {
                    plan.yank_to_host.append(&mut plan.release_cef_only);
                }
            }
        } else {
            plan.yank_to_host = explicit_yanks;
            for (tab, seq) in stray {
                push_tab_yank(&mut plan.yank_to_host, tab, self.click_cutoff_seq.max(seq));
            }
        }

        if plan.gpui == GpuiFocusChange::BlurPage {
            plan.blur_cutoff_seq = blur_cutoff;
        }

        plan
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn got_focus_alone_after_press_release_hands_focus_to_page_without_yank() {
        let mut tracker = PressTracker::new();
        let t0 = Instant::now();
        let press_seq = next_focus_seq();
        tracker.record_with_seq(42, t0, press_seq);

        let fallback_seq = next_focus_seq();
        let click_seq = callback_click_seq(
            true,
            FocusCallback::GotFocus,
            FocusSourceKind::System,
            fallback_seq,
            || {
                tracker
                    .consume(42, FocusCallback::GotFocus, t0 + Duration::from_millis(12))
                    .map_or(PressProbe::NotPressed, PressProbe::Pressed)
            },
            || false,
        );
        assert_eq!(click_seq, Some(press_seq), "released quick click delivering only on_got_focus still counts as a user click with press-time seq");

        let mut handoff = FocusHandoff::new();
        let got_seq = next_focus_seq();
        let took_over = handoff.on_pump_tab(
            1,
            TabFocusEvents {
                click_seq: press_seq,
                key_seq: 0,
                got_focus_seq: got_seq,
                took_focus_seq: 0,
            },
            true,
        );
        assert!(took_over);
        assert_eq!(
            handoff.take_render_plan(Some(1), false, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::FocusPage,
                ..Default::default()
            }
        );
    }

    #[test]
    fn fallback_consults_button_down_for_got_focus_and_system_set_focus_only() {
        let mut called = false;
        assert_eq!(
            callback_click_seq(
                true,
                FocusCallback::GotFocus,
                FocusSourceKind::System,
                77,
                || PressProbe::Unwatched,
                || {
                    called = true;
                    true
                },
            ),
            Some(77)
        );
        assert!(called, "GotFocus falls back to button_down when watcher is unavailable");

        assert_eq!(
            callback_click_seq(
                true,
                FocusCallback::SetFocus,
                FocusSourceKind::System,
                88,
                || PressProbe::Unwatched,
                || true,
            ),
            Some(88)
        );

        assert_eq!(
            callback_click_seq(
                true,
                FocusCallback::SetFocus,
                FocusSourceKind::Navigation,
                99,
                || unreachable!("Navigation must not consume press"),
                || unreachable!("Navigation must not poll button_down"),
            ),
            None
        );
        assert_eq!(
            callback_click_seq(
                false,
                FocusCallback::GotFocus,
                FocusSourceKind::System,
                99,
                || unreachable!("non-windowed must not consume press"),
                || unreachable!("non-windowed must not poll button_down"),
            ),
            None
        );
        assert_eq!(
            callback_click_seq(
                true,
                FocusCallback::GotFocus,
                FocusSourceKind::System,
                99,
                || PressProbe::NotPressed,
                || unreachable!("watched target returning NotPressed must not fall back to button_down"),
            ),
            None
        );
    }

    #[test]
    fn press_tracker_pairs_set_and_got_focus_expires_and_isolates_targets() {
        let mut tracker = PressTracker::new();
        let t0 = Instant::now();

        assert_eq!(tracker.record(0, t0), 0);
        assert_eq!(tracker.consume(0, FocusCallback::SetFocus, t0), None);

        let s1 = tracker.record(10, t0);
        assert_ne!(s1, 0);
        assert_eq!(tracker.consume(20, FocusCallback::SetFocus, t0 + Duration::from_millis(5)), None, "wrong target never consumes");
        assert_eq!(tracker.consume(10, FocusCallback::SetFocus, t0 + Duration::from_millis(5)), Some(s1));
        assert_eq!(tracker.consume(10, FocusCallback::SetFocus, t0 + Duration::from_millis(6)), None, "SetFocus consumed once");
        assert_eq!(tracker.consume(10, FocusCallback::GotFocus, t0 + Duration::from_millis(10)), Some(s1), "paired GotFocus still succeeds once");
        assert_eq!(tracker.consume(10, FocusCallback::GotFocus, t0 + Duration::from_millis(12)), None, "third callback rejected");

        let s2 = tracker.record(10, t0);
        assert_eq!(tracker.consume(10, FocusCallback::GotFocus, t0 + Duration::from_millis(5)), Some(s2));
        assert_eq!(tracker.consume(10, FocusCallback::SetFocus, t0 + Duration::from_millis(6)), None, "GotFocus spends both");

        tracker.record(10, t0);
        assert_eq!(tracker.consume(10, FocusCallback::GotFocus, t0 + PRESS_SPAN + Duration::from_millis(1)), None, "expires after PRESS_SPAN");

        tracker.record(10, t0);
        tracker.clear_target(10);
        assert_eq!(tracker.consume(10, FocusCallback::GotFocus, t0 + Duration::from_millis(10)), None, "unused press cleared when focus leaves");

        tracker.record(10, t0);
        tracker.clear_all();
        assert_eq!(tracker.consume(10, FocusCallback::GotFocus, t0 + Duration::from_millis(5)), None, "clear_all drops recorded press");
    }

    #[test]
    fn press_tracker_deduplicates_win32_mouseactivate_bubble_and_paired_button_down() {
        const WM_LBUTTONDOWN: u32 = 0x0201;
        const WM_XBUTTONDOWN: u32 = 0x020B;
        let mut tracker = PressTracker::new();
        let t0 = Instant::now();

        // Nested bubbling WM_MOUSEACTIVATE (child -> parent) pushes another frame; returning eaten
        // on the child leaves the parent's activation alive until paired WH_MOUSE down arrives.
        let s1 = tracker.record_activate_with_seq(10, WM_XBUTTONDOWN, t0, 100);
        assert_eq!(s1, 100);
        assert_eq!(tracker.record_activate_with_seq(10, WM_XBUTTONDOWN, t0 + Duration::from_millis(1), 101), 100);
        assert!(tracker.has_pending_activate());
        assert_eq!(tracker.fresh_press_seq_after(10, 0, t0 + Duration::from_millis(1)), None, "in-flight activation is unconfirmed");
        // Inner child returning ActivateAndEat while parent still active leaves parent frame pending;
        // parent returning DeliverDown confirms it and waits for paired button-down.
        assert!(!tracker.complete_activate_with_seq(10, WM_XBUTTONDOWN, MouseActivateOutcome::ActivateAndEat, t0 + Duration::from_millis(1), 100));
        assert!(tracker.has_pending_activate(), "inner eaten must not drop outer bubbling activation");
        assert!(tracker.complete_activate_with_seq(10, WM_XBUTTONDOWN, MouseActivateOutcome::DeliverDown, t0 + Duration::from_millis(2), 100));
        assert_eq!(tracker.fresh_press_seq_after(10, 0, t0 + Duration::from_millis(2)), Some(100));
        assert!(tracker.has_pending_activate(), "non-eaten activation waits for paired button-down");
        assert_eq!(tracker.consume(10, FocusCallback::SetFocus, t0 + Duration::from_millis(2)), Some(100));
        assert_eq!(tracker.consume(10, FocusCallback::GotFocus, t0 + Duration::from_millis(3)), Some(100));
        assert_eq!(tracker.record_button_down_with_seq(10, WM_XBUTTONDOWN, t0 + Duration::from_millis(4), 102), 100);
        assert!(!tracker.has_pending_activate());
        assert_eq!(tracker.consume(10, FocusCallback::GotFocus, t0 + Duration::from_millis(5)), None, "paired button-down must not re-enable spent activation click");
        assert_eq!(tracker.consume(10, FocusCallback::SetFocus, t0 + Duration::from_millis(5)), None);

        // Outermost ActivateAndEat clears pairing immediately while keeping confirmed press so a subsequent click records fresh.
        assert_eq!(tracker.record_activate_with_seq(10, WM_XBUTTONDOWN, t0 + Duration::from_millis(15), 150), 150);
        assert!(tracker.complete_activate_with_seq(10, WM_XBUTTONDOWN, MouseActivateOutcome::ActivateAndEat, t0 + Duration::from_millis(15), 150));
        assert!(!tracker.has_pending_activate(), "outermost eaten activation expects no WH_MOUSE down");
        assert_eq!(tracker.consume(10, FocusCallback::GotFocus, t0 + Duration::from_millis(16)), Some(150));

        let s2 = tracker.record_button_down_with_seq(10, WM_XBUTTONDOWN, t0 + Duration::from_millis(20), 200);
        assert_eq!(s2, 200);
        assert_eq!(tracker.consume(10, FocusCallback::GotFocus, t0 + Duration::from_millis(22)), Some(200));

        // Activation cancelled via clear_target absorbs a stalled (> PRESS_SPAN) paired button-down without re-arming.
        assert_eq!(tracker.record_activate_with_seq(10, WM_LBUTTONDOWN, t0 + Duration::from_millis(40), 300), 300);
        assert!(tracker.complete_activate_with_seq(10, WM_LBUTTONDOWN, MouseActivateOutcome::DeliverDown, t0 + Duration::from_millis(40), 300));
        tracker.clear_target(10);
        assert!(tracker.has_pending_activate());
        let stalled = t0 + Duration::from_millis(40) + PRESS_SPAN + Duration::from_millis(250);
        assert_eq!(tracker.record_button_down_with_seq(10, WM_LBUTTONDOWN, stalled, 301), 300);
        assert!(!tracker.has_pending_activate());
        assert_eq!(tracker.consume(10, FocusCallback::GotFocus, stalled + Duration::from_millis(1)), None, "stalled cancelled activation click stays spent when button-down arrives");

        // Expired consume (> PRESS_SPAN) does NOT erase pending_activate_downs, so a delayed paired button-down still deduplicates.
        let t1 = stalled + Duration::from_millis(50);
        assert_eq!(tracker.record_activate_with_seq(10, WM_LBUTTONDOWN, t1, 400), 400);
        assert!(tracker.complete_activate_with_seq(10, WM_LBUTTONDOWN, MouseActivateOutcome::DeliverDown, t1, 400));
        let after_span = t1 + PRESS_SPAN + Duration::from_millis(10);
        assert_eq!(tracker.consume(10, FocusCallback::GotFocus, after_span), None, "expired activate record rejected");
        assert!(tracker.has_pending_activate(), "expired consume must keep pending pairing until paired button-down arrives");
        assert_eq!(tracker.record_button_down_with_seq(10, WM_LBUTTONDOWN, after_span + Duration::from_millis(5), 401), 400);
        assert_eq!(tracker.consume(10, FocusCallback::GotFocus, after_span + Duration::from_millis(6)), None, "delayed paired down after expiry must not open fresh press");
    }

    #[test]
    fn press_tracker_preserves_outer_win32_activate_across_reentrant_nested_dispatch() {
        const WM_LBUTTONDOWN: u32 = 0x0201;
        const WM_RBUTTONDOWN: u32 = 0x0204;
        let mut tracker = PressTracker::new();
        let t0 = Instant::now();

        // Outer WM_MOUSEACTIVATE on (10, WM_LBUTTONDOWN) stays unconfirmed for both callbacks and cross-thread
        // recovery while in-flight, and preserves its click across nested reentrant activations on other keys.
        assert_eq!(tracker.record_activate_with_seq(10, WM_LBUTTONDOWN, t0, 100), 100);
        assert_eq!(tracker.fresh_press_seq_after(10, 50, t0), None, "in-flight activation cannot wake render recovery before return");
        assert_eq!(tracker.consume(10, FocusCallback::SetFocus, t0), None, "in-flight activation cannot publish before outcome");
        assert_eq!(tracker.record_activate_with_seq(20, WM_RBUTTONDOWN, t0 + Duration::from_millis(1), 200), 200);
        assert_eq!(tracker.fresh_press_seq_after(20, 50, t0 + Duration::from_millis(1)), None);
        assert!(tracker.complete_activate_with_seq(20, WM_RBUTTONDOWN, MouseActivateOutcome::ActivateAndEat, t0 + Duration::from_millis(1), 200));
        assert!(!tracker.has_pending_activate_for(20, WM_RBUTTONDOWN));
        assert!(tracker.has_pending_activate_for(10, WM_LBUTTONDOWN));
        assert_eq!(tracker.fresh_press_seq_after(10, 50, t0 + Duration::from_millis(2)), None);
        assert_eq!(tracker.consume(10, FocusCallback::SetFocus, t0 + Duration::from_millis(2)), None);
        assert_eq!(tracker.consume(10, FocusCallback::GotFocus, t0 + Duration::from_millis(2)), None);

        assert_eq!(tracker.record_activate_with_seq(10, WM_RBUTTONDOWN, t0 + Duration::from_millis(2), 201), 201);
        assert!(tracker.complete_activate_with_seq(10, WM_RBUTTONDOWN, MouseActivateOutcome::DeliverDown, t0 + Duration::from_millis(2), 201));
        // Outer (10, WM_LBUTTONDOWN) stays unconfirmed while still on activate_stack, then confirms on DeliverDown.
        assert_eq!(tracker.consume(10, FocusCallback::SetFocus, t0 + Duration::from_millis(2)), None);
        assert!(tracker.complete_activate_with_seq(10, WM_LBUTTONDOWN, MouseActivateOutcome::DeliverDown, t0 + Duration::from_millis(2), 100));
        assert!(tracker.has_pending_activate_for(10, WM_LBUTTONDOWN));
        assert!(tracker.has_pending_activate_for(10, WM_RBUTTONDOWN));

        // Unrelated unpaired button-down on (30, WM_LBUTTONDOWN) leaves pending pairings intact,
        // and each pending activation pairs independently by `(target, msg)` returning its own seq.
        assert_eq!(tracker.record_button_down_with_seq(30, WM_LBUTTONDOWN, t0 + Duration::from_millis(3), 260), 260);
        assert_eq!(tracker.record_button_down_with_seq(10, WM_LBUTTONDOWN, t0 + Duration::from_millis(4), 261), 100);
        assert!(!tracker.has_pending_activate_for(10, WM_LBUTTONDOWN));
        assert!(tracker.has_pending_activate_for(10, WM_RBUTTONDOWN));
        assert_eq!(tracker.fresh_press_seq_after(10, 50, t0 + Duration::from_millis(4)), Some(201));
        assert_eq!(tracker.consume(10, FocusCallback::SetFocus, t0 + Duration::from_millis(4)), Some(100));
        assert_eq!(tracker.consume(10, FocusCallback::GotFocus, t0 + Duration::from_millis(4)), Some(100));
        assert_eq!(tracker.record_button_down_with_seq(10, WM_RBUTTONDOWN, t0 + Duration::from_millis(5), 262), 201);
        assert!(!tracker.has_pending_activate());

        // drop_target removes only that target's frames and pending pairings.
        assert_eq!(tracker.record_activate_with_seq(40, WM_LBUTTONDOWN, t0 + Duration::from_millis(10), 300), 300);
        assert_eq!(tracker.record_activate_with_seq(50, WM_LBUTTONDOWN, t0 + Duration::from_millis(11), 301), 301);
        assert!(tracker.complete_activate_with_seq(50, WM_LBUTTONDOWN, MouseActivateOutcome::DeliverDown, t0 + Duration::from_millis(11), 301));
        tracker.drop_target(40);
        assert!(!tracker.has_pending_activate_for(40, WM_LBUTTONDOWN));
        assert!(tracker.has_pending_activate_for(50, WM_LBUTTONDOWN));
        assert_eq!(tracker.record_button_down_with_seq(50, WM_LBUTTONDOWN, t0 + Duration::from_millis(12), 302), 301);

        // When paired button-down arrives AFTER nested non-eaten dispatch and BEFORE CEF focus callbacks,
        // consume() still succeeds once with the outer activation seq.
        let t1 = t0 + Duration::from_millis(20);
        assert_eq!(tracker.record_activate_with_seq(60, WM_LBUTTONDOWN, t1, 400), 400);
        assert_eq!(tracker.record_activate_with_seq(70, WM_RBUTTONDOWN, t1 + Duration::from_millis(1), 401), 401);
        assert!(tracker.complete_activate_with_seq(70, WM_RBUTTONDOWN, MouseActivateOutcome::DeliverDown, t1 + Duration::from_millis(1), 401));
        assert!(tracker.complete_activate_with_seq(60, WM_LBUTTONDOWN, MouseActivateOutcome::DeliverDown, t1 + Duration::from_millis(1), 400));
        assert_eq!(tracker.record_button_down_with_seq(60, WM_LBUTTONDOWN, t1 + Duration::from_millis(2), 402), 400);
        assert_eq!(tracker.consume(60, FocusCallback::GotFocus, t1 + Duration::from_millis(3)), Some(400));
        assert_eq!(tracker.consume(60, FocusCallback::GotFocus, t1 + Duration::from_millis(4)), None);

        // When CEF callbacks already spent the outer click across nested dispatch, a later paired button-down
        // returns the outer seq without re-arming consume().
        let t2 = t0 + Duration::from_millis(30);
        assert_eq!(tracker.record_activate_with_seq(80, WM_LBUTTONDOWN, t2, 500), 500);
        assert_eq!(tracker.record_activate_with_seq(90, WM_RBUTTONDOWN, t2 + Duration::from_millis(1), 501), 501);
        assert!(tracker.complete_activate_with_seq(90, WM_RBUTTONDOWN, MouseActivateOutcome::ActivateAndEat, t2 + Duration::from_millis(1), 501));
        assert!(tracker.complete_activate_with_seq(80, WM_LBUTTONDOWN, MouseActivateOutcome::DeliverDown, t2 + Duration::from_millis(1), 500));
        assert_eq!(tracker.consume(80, FocusCallback::SetFocus, t2 + Duration::from_millis(2)), Some(500));
        assert_eq!(tracker.consume(80, FocusCallback::GotFocus, t2 + Duration::from_millis(2)), Some(500));
        assert_eq!(tracker.record_button_down_with_seq(80, WM_LBUTTONDOWN, t2 + Duration::from_millis(3), 502), 500);
        assert_eq!(tracker.consume(80, FocusCallback::GotFocus, t2 + Duration::from_millis(4)), None);

        // Same-target nested two-button activation preserves outer click record and keeps both unconfirmed while active:
        // 1) Neither (100, R) nor outer (100, L) can consume while in-flight; outer (100, L) stays unconfirmed until
        //    outer DeliverDown, then its paired button-down bumps 600 to the front of consumption.
        let t3 = t0 + Duration::from_millis(40);
        assert_eq!(tracker.record_activate_with_seq(100, WM_LBUTTONDOWN, t3, 600), 600);
        assert_eq!(tracker.record_activate_with_seq(100, WM_RBUTTONDOWN, t3 + Duration::from_millis(1), 601), 601);
        assert_eq!(tracker.consume(100, FocusCallback::SetFocus, t3 + Duration::from_millis(1)), None);
        assert_eq!(tracker.fresh_press_seq_after(100, 0, t3 + Duration::from_millis(1)), None);
        assert!(tracker.complete_activate_with_seq(100, WM_RBUTTONDOWN, MouseActivateOutcome::DeliverDown, t3 + Duration::from_millis(1), 601));
        assert_eq!(tracker.consume(100, FocusCallback::SetFocus, t3 + Duration::from_millis(2)), None);
        assert_eq!(tracker.fresh_press_seq_after(100, 601, t3 + Duration::from_millis(2)), None);
        assert!(tracker.complete_activate_with_seq(100, WM_LBUTTONDOWN, MouseActivateOutcome::DeliverDown, t3 + Duration::from_millis(2), 600));
        assert_eq!(tracker.record_button_down_with_seq(100, WM_LBUTTONDOWN, t3 + Duration::from_millis(3), 602), 600);
        assert_eq!(tracker.consume(100, FocusCallback::SetFocus, t3 + Duration::from_millis(3)), Some(600));
        assert_eq!(tracker.consume(100, FocusCallback::GotFocus, t3 + Duration::from_millis(3)), Some(600));
        assert_eq!(tracker.consume(100, FocusCallback::GotFocus, t3 + Duration::from_millis(4)), Some(601));
        assert_eq!(tracker.consume(100, FocusCallback::GotFocus, t3 + Duration::from_millis(4)), None);
        assert_eq!(tracker.record_button_down_with_seq(100, WM_RBUTTONDOWN, t3 + Duration::from_millis(5), 603), 601);
        assert_eq!(tracker.consume(100, FocusCallback::GotFocus, t3 + Duration::from_millis(6)), None);

        // 2) Same-target nested eaten activation (110, WM_RBUTTONDOWN) drops its unused press record without
        //    ever exposing an in-flight recovery press, leaving outer (110, WM_LBUTTONDOWN) clean on stack.
        let t4 = t0 + Duration::from_millis(50);
        assert_eq!(tracker.record_activate_with_seq(110, WM_LBUTTONDOWN, t4, 700), 700);
        assert_eq!(tracker.record_activate_with_seq(110, WM_RBUTTONDOWN, t4 + Duration::from_millis(1), 701), 701);
        assert_eq!(tracker.fresh_press_seq_after(110, 0, t4 + Duration::from_millis(1)), None);
        assert!(!tracker.complete_activate_with_seq(110, WM_RBUTTONDOWN, MouseActivateOutcome::ActivateAndEat, t4 + Duration::from_millis(1), 701));
        assert_eq!(tracker.fresh_press_seq_after(110, 0, t4 + Duration::from_millis(2)), None);
        assert!(tracker.complete_activate_with_seq(110, WM_LBUTTONDOWN, MouseActivateOutcome::DeliverDown, t4 + Duration::from_millis(2), 700));
        assert_eq!(tracker.consume(110, FocusCallback::SetFocus, t4 + Duration::from_millis(2)), Some(700));
        assert_eq!(tracker.consume(110, FocusCallback::GotFocus, t4 + Duration::from_millis(2)), Some(700));
        assert_eq!(tracker.record_button_down_with_seq(110, WM_LBUTTONDOWN, t4 + Duration::from_millis(3), 702), 700);
        assert_eq!(tracker.consume(110, FocusCallback::GotFocus, t4 + Duration::from_millis(4)), None);

        // 3) Same-target nested non-eaten where outer paired button-down (120, WM_LBUTTONDOWN) precedes CEF focus callbacks
        //    bumps outer's press record to the back so consume() picks outer 800 first.
        let t5 = t0 + Duration::from_millis(60);
        assert_eq!(tracker.record_activate_with_seq(120, WM_LBUTTONDOWN, t5, 800), 800);
        assert_eq!(tracker.record_activate_with_seq(120, WM_RBUTTONDOWN, t5 + Duration::from_millis(1), 801), 801);
        assert!(tracker.complete_activate_with_seq(120, WM_RBUTTONDOWN, MouseActivateOutcome::DeliverDown, t5 + Duration::from_millis(1), 801));
        assert!(tracker.complete_activate_with_seq(120, WM_LBUTTONDOWN, MouseActivateOutcome::DeliverDown, t5 + Duration::from_millis(1), 800));
        assert_eq!(tracker.record_button_down_with_seq(120, WM_LBUTTONDOWN, t5 + Duration::from_millis(2), 802), 800);
        assert_eq!(tracker.consume(120, FocusCallback::GotFocus, t5 + Duration::from_millis(3)), Some(800));

        // 4) NoActivateAndEat never exposes a press before or after complete_activate when no button-down arrived.
        let t6 = t0 + Duration::from_millis(70);
        assert_eq!(tracker.record_activate_with_seq(125, WM_LBUTTONDOWN, t6, 850), 850);
        assert_eq!(tracker.fresh_press_seq_after(125, 0, t6), None);
        assert_eq!(tracker.consume(125, FocusCallback::GotFocus, t6), None);
        assert!(!tracker.complete_activate_with_seq(125, WM_LBUTTONDOWN, MouseActivateOutcome::NoActivateAndEat, t6, 850));
        assert_eq!(tracker.fresh_press_seq_after(125, 0, t6 + Duration::from_millis(1)), None);
        assert_eq!(tracker.consume(125, FocusCallback::GotFocus, t6 + Duration::from_millis(1)), None);

        // 5) Reentrant `left -> right -> left`: mid-dispatch `record_button_down` confirms and upgrades (130, L)
        //    to confirmation-time 903 before `right` unwinds, and outer `left` returning `NoActivateAndEat`
        //    preserves 903 without overwriting it with 906.
        let t7 = t0 + Duration::from_millis(80);
        assert_eq!(tracker.record_activate_with_seq(130, WM_LBUTTONDOWN, t7, 900), 900);
        assert_eq!(tracker.record_activate_with_seq(130, WM_RBUTTONDOWN, t7 + Duration::from_millis(1), 901), 901);
        assert_eq!(tracker.record_activate_with_seq(130, WM_LBUTTONDOWN, t7 + Duration::from_millis(2), 902), 900);
        assert_eq!(tracker.fresh_press_seq_after(130, 0, t7 + Duration::from_millis(2)), None);
        assert_eq!(tracker.consume(130, FocusCallback::SetFocus, t7 + Duration::from_millis(2)), None);
        assert_eq!(tracker.record_button_down_with_seq(130, WM_LBUTTONDOWN, t7 + Duration::from_millis(2), 903), 903);
        assert_eq!(tracker.fresh_press_seq_after(130, 0, t7 + Duration::from_millis(2)), Some(903));
        assert_eq!(tracker.consume(130, FocusCallback::SetFocus, t7 + Duration::from_millis(2)), Some(903));
        assert!(!tracker.complete_activate_with_seq(130, WM_LBUTTONDOWN, MouseActivateOutcome::DeliverDown, t7 + Duration::from_millis(2), 904));
        assert_eq!(tracker.consume(130, FocusCallback::SetFocus, t7 + Duration::from_millis(3)), None);
        assert!(!tracker.complete_activate_with_seq(130, WM_RBUTTONDOWN, MouseActivateOutcome::ActivateAndEat, t7 + Duration::from_millis(3), 905));
        assert!(tracker.complete_activate_with_seq(130, WM_LBUTTONDOWN, MouseActivateOutcome::NoActivateAndEat, t7 + Duration::from_millis(4), 906));
        assert!(!tracker.has_pending_activate_for(130, WM_LBUTTONDOWN));
        assert_eq!(tracker.fresh_press_seq_after(130, 0, t7 + Duration::from_millis(4)), Some(903));
        assert_eq!(tracker.consume(130, FocusCallback::GotFocus, t7 + Duration::from_millis(4)), Some(903));
        assert_eq!(tracker.consume(130, FocusCallback::GotFocus, t7 + Duration::from_millis(5)), None);
    }

    #[test]
    fn press_tracker_unwatched_release_gate_blocks_until_matching_generation_clears() {
        let mut tracker = PressTracker::new();
        assert_eq!(tracker.block_unwatched_until_release(0), None);
        assert!(!tracker.is_unwatched_blocked(7));

        let g1 = tracker.block_unwatched_until_release(7);
        assert_eq!(g1, Some(1));
        assert!(tracker.is_unwatched_blocked(7));

        // Second block while already non-empty bumps generation and asks caller not to spawn another poller.
        assert_eq!(tracker.block_unwatched_until_release(8), None);
        let g2 = tracker.unwatched_block_gen();
        assert_eq!(g2, 2);
        assert!(tracker.is_unwatched_blocked(7));
        assert!(tracker.is_unwatched_blocked(8));

        // Stale generation cannot clear the newer block.
        assert!(!tracker.clear_unwatched_release_block_if(g1.unwrap()));
        assert!(tracker.is_unwatched_blocked(7));
        assert!(tracker.is_unwatched_blocked(8));

        // drop_target removes only the closed target.
        tracker.drop_target(7);
        assert!(!tracker.is_unwatched_blocked(7));
        assert!(tracker.is_unwatched_blocked(8));

        // Matching generation clears remaining entries.
        assert!(tracker.clear_unwatched_release_block_if(g2));
        assert!(!tracker.is_unwatched_blocked(8));
    }

    #[test]
    fn render_reconciles_unconsumed_post_cutoff_press_and_clear_target_up_to_preserves_newer_click() {
        let mut tracker = PressTracker::new();
        let t0 = Instant::now();

        // Older press at seq 50 cleared by cutoff 50.
        tracker.record_with_seq(10, t0, 50);
        assert_eq!(tracker.fresh_press_seq_after(10, 50, t0 + Duration::from_millis(1)), None);
        tracker.clear_target_up_to(10, 50);
        assert_eq!(tracker.consume(10, FocusCallback::GotFocus, t0 + Duration::from_millis(2)), None);

        // Newer press at seq 75 survives yank_to_host cutoff 60, even if CEF already consumed GotFocus
        // before writing back st.click_seq.
        tracker.record_with_seq(10, t0 + Duration::from_millis(5), 75);
        assert_eq!(tracker.consume(10, FocusCallback::GotFocus, t0 + Duration::from_millis(6)), Some(75));
        assert_eq!(tracker.fresh_press_seq_after(10, 60, t0 + Duration::from_millis(7)), Some(75));
        tracker.clear_target_up_to(10, 60);
        assert_eq!(tracker.fresh_press_seq_after(10, 60, t0 + Duration::from_millis(8)), Some(75));

        // Render reconciliation promotes the post-cutoff native press into FocusPage, cancelling HandBack.
        let mut handoff = FocusHandoff::new();
        handoff.request_blur(Some(1), 60);
        assert_eq!(handoff.pending_press_cutoff_for_active(Some(1), false), Some(60));
        assert!(handoff.on_pending_click(1, 75));
        assert_eq!(handoff.pending_press_cutoff_for_active(Some(1), false), None);
        assert_eq!(
            handoff.take_render_plan(Some(1), false, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::FocusPage,
                ..Default::default()
            }
        );

        // Similarly, active-tab unprompted got_focus is overridden by a newer native press before render yanks it.
        let mut stray = FocusHandoff::new();
        let got_seq = next_focus_seq();
        let press_seq = next_focus_seq();
        stray.on_pump_tab(1, TabFocusEvents { got_focus_seq: got_seq, ..Default::default() }, true);
        assert_eq!(stray.pending_press_cutoff_for_active(Some(1), false), Some(got_seq));
        assert!(stray.on_pending_click(1, press_seq));
        assert_eq!(
            stray.take_render_plan(Some(1), false, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::FocusPage,
                ..Default::default()
            }
        );
    }

    #[test]
    fn queued_key_cannot_cancel_hand_back_before_or_after_render_until_click_or_host_focus() {
        let mut handoff = FocusHandoff::new();
        let hb_seq = next_focus_seq();
        handoff.request_blur(Some(1), hb_seq);

        // Queued key repeat arriving AFTER HandBack cutoff but BEFORE render must NOT cancel HandBack or take_over.
        let queued_key_before_render = next_focus_seq();
        assert!(!handoff.on_pump_tab(
            1,
            TabFocusEvents {
                click_seq: 0,
                key_seq: queued_key_before_render,
                got_focus_seq: 0,
                took_focus_seq: 0,
            },
            true,
        ));
        let plan = handoff.take_render_plan(Some(1), true, true);
        assert_eq!(
            plan,
            RenderFocusPlan {
                gpui: GpuiFocusChange::BlurPage,
                release_cef_only: vec![],
                yank_to_host: vec![TabYank { tab: 1, cutoff_seq: hb_seq }],
                restore_active: None,
                blur_cutoff_seq: hb_seq,
            }
        );

        // Even after render consumed HandBack, another queued key repeat must NOT trigger FocusPage.
        let queued_key_after_render = next_focus_seq();
        assert!(!handoff.on_pump_tab(
            1,
            TabFocusEvents {
                click_seq: 0,
                key_seq: queued_key_after_render,
                got_focus_seq: 0,
                took_focus_seq: 0,
            },
            true,
        ));
        assert_eq!(handoff.take_render_plan(Some(1), false, true), RenderFocusPlan::default());

        // A genuine user click DOES cancel handed_back and restores FocusPage.
        let click_seq = next_focus_seq();
        assert!(handoff.on_pump_tab(
            1,
            TabFocusEvents {
                click_seq,
                key_seq: 0,
                got_focus_seq: click_seq,
                took_focus_seq: 0,
            },
            true,
        ));
        assert_eq!(
            handoff.take_render_plan(Some(1), false, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::FocusPage,
                ..Default::default()
            }
        );

        // After another HandBack, explicit GPUI host focus (e.g. Tab-in) lifts handed_back so typing works again.
        let hb2 = next_focus_seq();
        handoff.request_blur(Some(1), hb2);
        let _ = handoff.take_render_plan(Some(1), true, true);
        handoff.on_host_focus();
        let next_key = next_focus_seq();
        assert!(handoff.on_pump_tab(
            1,
            TabFocusEvents {
                click_seq: 0,
                key_seq: next_key,
                got_focus_seq: 0,
                took_focus_seq: 0,
            },
            true,
        ));
    }

    #[test]
    fn split_pumps_keep_focus_page_across_subsequent_unclicked_got_focus() {
        let mut handoff = FocusHandoff::new();
        let s1 = next_focus_seq();
        assert!(handoff.on_pump_tab(
            1,
            TabFocusEvents {
                click_seq: s1,
                key_seq: 0,
                got_focus_seq: 0,
                took_focus_seq: 0,
            },
            true,
        ));
        let s2 = next_focus_seq();
        assert!(!handoff.on_pump_tab(
            1,
            TabFocusEvents {
                click_seq: 0,
                key_seq: 0,
                got_focus_seq: s2,
                took_focus_seq: 0,
            },
            true,
        ));
        assert_eq!(
            handoff.take_render_plan(Some(1), false, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::FocusPage,
                ..Default::default()
            }
        );
        assert_eq!(handoff.take_render_plan(Some(1), true, true), RenderFocusPlan::default());
    }

    #[test]
    fn unprompted_got_focus_yanks_originating_tab_only_when_eligible_and_window_active() {
        let mut handoff = FocusHandoff::new();
        let s1 = next_focus_seq();
        assert!(!handoff.on_pump_tab(
            1,
            TabFocusEvents {
                click_seq: 0,
                key_seq: 0,
                got_focus_seq: s1,
                took_focus_seq: 0,
            },
            true,
        ));
        assert_eq!(
            handoff.take_render_plan(Some(1), false, true),
            RenderFocusPlan {
                yank_to_host: vec![TabYank { tab: 1, cutoff_seq: s1 }],
                ..Default::default()
            }
        );

        let s2 = next_focus_seq();
        handoff.on_pump_tab(1, TabFocusEvents { got_focus_seq: s2, ..Default::default() }, true);
        assert_eq!(handoff.take_render_plan(Some(1), true, true), RenderFocusPlan::default(), "already page focused");

        let s3 = next_focus_seq();
        handoff.on_pump_tab(1, TabFocusEvents { got_focus_seq: s3, ..Default::default() }, true);
        assert_eq!(handoff.take_render_plan(Some(1), false, false), RenderFocusPlan::default(), "inactive window never yanks");

        // Inactive window never takes focus on delayed click either.
        let s4 = next_focus_seq();
        assert!(handoff.on_pump_tab(1, TabFocusEvents { click_seq: s4, ..Default::default() }, true));
        assert_eq!(handoff.take_render_plan(Some(1), false, false), RenderFocusPlan::default(), "inactive window never steals focus");

        // User click before render overrides a stale YankTab for the active tab.
        let s5 = next_focus_seq();
        let s6 = next_focus_seq();
        handoff.on_pump_tab(1, TabFocusEvents { got_focus_seq: s5, ..Default::default() }, true);
        assert!(handoff.on_pump_tab(1, TabFocusEvents { click_seq: s6, ..Default::default() }, true));
        assert_eq!(
            handoff.take_render_plan(Some(1), false, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::FocusPage,
                ..Default::default()
            }
        );
    }

    #[test]
    fn background_tab_never_steals_focus_from_active_tab_and_order_is_independent() {
        // 1. Background tab 1 unclicked got_focus while tab 2 is focused in GPUI
        //    must release tab 1 inside CEF and restore tab 2 without focusing the host.
        let mut handoff = FocusHandoff::new();
        let s1 = next_focus_seq();
        assert!(!handoff.on_pump_tab(1, TabFocusEvents { got_focus_seq: s1, ..Default::default() }, false));
        assert_eq!(
            handoff.take_render_plan(Some(2), true, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::None,
                release_cef_only: vec![TabYank { tab: 1, cutoff_seq: s1 }],
                yank_to_host: vec![],
                restore_active: Some(2),
                blur_cutoff_seq: 0,
            }
        );

        // 2. Background tab with a HIGHER got_focus_seq than active tab's click click_seq
        //    must produce the exact same FocusPage + background release regardless of tab order.
        let s_click = next_focus_seq();
        let s_bg_later = next_focus_seq();

        let mut forward = FocusHandoff::new();
        assert!(!forward.on_pump_tab(1, TabFocusEvents { click_seq: s_bg_later, key_seq: s_bg_later, got_focus_seq: s_bg_later, took_focus_seq: s_bg_later }, false));
        assert!(forward.on_pump_tab(2, TabFocusEvents { click_seq: s_click, ..Default::default() }, true));
        let plan_forward = forward.take_render_plan(Some(2), false, true);

        let mut reverse = FocusHandoff::new();
        assert!(reverse.on_pump_tab(2, TabFocusEvents { click_seq: s_click, ..Default::default() }, true));
        assert!(!reverse.on_pump_tab(1, TabFocusEvents { click_seq: s_bg_later, key_seq: s_bg_later, got_focus_seq: s_bg_later, took_focus_seq: s_bg_later }, false));
        let plan_reverse = reverse.take_render_plan(Some(2), false, true);

        let expected = RenderFocusPlan {
            gpui: GpuiFocusChange::FocusPage,
            release_cef_only: vec![TabYank { tab: 1, cutoff_seq: s_bg_later }],
            yank_to_host: vec![],
            restore_active: Some(2),
            blur_cutoff_seq: 0,
        };
        assert_eq!(plan_forward, expected);
        assert_eq!(plan_reverse, expected);

        // 3. Delayed click on tab 1 must not focus tab 2 if active switched before render.
        let mut switched = FocusHandoff::new();
        assert!(switched.on_pump_tab(1, TabFocusEvents { click_seq: next_focus_seq(), ..Default::default() }, true));
        assert_eq!(switched.take_render_plan(Some(2), false, true), RenderFocusPlan::default());
    }

    #[test]
    fn tab_out_crossing_tab_switch_does_not_blur_new_tab_and_blur_preserves_stray_yanks() {
        // 1. Tab 1 emits TabOut, then a popup/tab switch makes Tab 2 active before render
        //    while GPUI is focused: must NOT blur GPUI away from Tab 2, only release Tab 1 & restore Tab 2.
        let mut handoff = FocusHandoff::new();
        let k1 = next_focus_seq();
        let took1 = next_focus_seq();
        assert!(handoff.on_pump_tab(1, TabFocusEvents { key_seq: k1, took_focus_seq: took1, ..Default::default() }, true));
        assert_eq!(
            handoff.take_render_plan(Some(2), true, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::None,
                release_cef_only: vec![TabYank { tab: 1, cutoff_seq: took1 }],
                yank_to_host: vec![],
                restore_active: Some(2),
                blur_cutoff_seq: 0,
            }
        );

        // 2. Tab 1 emits TabOut while Tab 2 also emits unprompted got_focus on the same pump;
        //    GPUI blurs back to host AND both Tab 1 and Tab 2 are yanked with their own cutoffs.
        let mut combined = FocusHandoff::new();
        let k2 = next_focus_seq();
        let took2 = next_focus_seq();
        let bg2 = next_focus_seq();
        assert!(combined.on_pump_tab(1, TabFocusEvents { key_seq: k2, took_focus_seq: took2, ..Default::default() }, true));
        assert!(!combined.on_pump_tab(2, TabFocusEvents { got_focus_seq: bg2, ..Default::default() }, false));
        assert_eq!(
            combined.take_render_plan(Some(1), true, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::BlurPage,
                release_cef_only: vec![],
                yank_to_host: vec![
                    TabYank { tab: 1, cutoff_seq: took2 },
                    TabYank { tab: 2, cutoff_seq: bg2 },
                ],
                restore_active: None,
                blur_cutoff_seq: took2,
            }
        );

        // 3. HandBack requested while Tab 1 was active, then Tab 2 becomes active before render
        //    with Tab 3 unprompted got_focus: page-wide HandBack blurs GPUI at hb_seq and yanks each tab with its own cutoff.
        let mut hb = FocusHandoff::new();
        let hb_seq = next_focus_seq();
        hb.request_blur(Some(1), hb_seq);
        let stray3 = next_focus_seq();
        assert!(!hb.on_pump_tab(3, TabFocusEvents { got_focus_seq: stray3, ..Default::default() }, false));
        assert_eq!(
            hb.take_render_plan(Some(2), true, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::BlurPage,
                release_cef_only: vec![],
                yank_to_host: vec![
                    TabYank { tab: 1, cutoff_seq: hb_seq },
                    TabYank { tab: 2, cutoff_seq: hb_seq },
                    TabYank { tab: 3, cutoff_seq: stray3 },
                ],
                restore_active: None,
                blur_cutoff_seq: hb_seq,
            }
        );
    }

    #[test]
    fn hand_back_is_not_replaced_by_newer_tab_out_when_popup_switches_active_tab() {
        let mut handoff = FocusHandoff::new();
        let hb_seq = next_focus_seq();
        let took_after = next_focus_seq();
        handoff.request_blur(Some(1), hb_seq);
        assert!(!handoff.on_pump_tab(
            1,
            TabFocusEvents {
                click_seq: 0,
                key_seq: 0,
                got_focus_seq: 0,
                took_focus_seq: took_after,
            },
            true,
        ));
        assert_eq!(
            handoff.take_render_plan(Some(2), true, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::BlurPage,
                release_cef_only: vec![],
                yank_to_host: vec![
                    TabYank { tab: 1, cutoff_seq: took_after },
                    TabYank { tab: 2, cutoff_seq: hb_seq },
                ],
                restore_active: None,
                blur_cutoff_seq: hb_seq,
            }
        );
    }

    #[test]
    fn hand_back_and_host_blur_preserve_stray_yanks_including_pre_cutoff_got_focus() {
        // Background tab 2 got_focus pumped BEFORE request_blur stays in yank_to_host.
        let mut before_hb = FocusHandoff::new();
        let stray_before = next_focus_seq();
        before_hb.on_pump_tab(2, TabFocusEvents { got_focus_seq: stray_before, ..Default::default() }, false);
        let hb_seq1 = next_focus_seq();
        before_hb.request_blur(Some(1), hb_seq1);
        assert_eq!(
            before_hb.take_render_plan(Some(1), true, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::BlurPage,
                release_cef_only: vec![],
                yank_to_host: vec![
                    TabYank { tab: 1, cutoff_seq: hb_seq1 },
                    TabYank { tab: 2, cutoff_seq: hb_seq1 },
                ],
                restore_active: None,
                blur_cutoff_seq: hb_seq1,
            }
        );

        // Background tab 2 got_focus with seq < handback_seq pumped AFTER request_blur still yanks.
        let mut delayed_hb = FocusHandoff::new();
        let stray_pre_hb = next_focus_seq();
        let hb_seq = next_focus_seq();
        delayed_hb.request_blur(Some(1), hb_seq);
        assert!(!delayed_hb.on_pump_tab(2, TabFocusEvents { got_focus_seq: stray_pre_hb, ..Default::default() }, false));
        assert_eq!(
            delayed_hb.take_render_plan(Some(1), false, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::None,
                release_cef_only: vec![],
                yank_to_host: vec![
                    TabYank { tab: 1, cutoff_seq: hb_seq },
                    TabYank { tab: 2, cutoff_seq: hb_seq },
                ],
                restore_active: None,
                blur_cutoff_seq: 0,
            }
        );

        // Background tab 2 pumped before on_host_blur stays in yank_to_host.
        let mut host_blur = FocusHandoff::new();
        let stray_before_blur = next_focus_seq();
        host_blur.on_pump_tab(2, TabFocusEvents { got_focus_seq: stray_before_blur, ..Default::default() }, false);
        let blur_seq = next_focus_seq();
        host_blur.on_host_blur(blur_seq);
        assert_eq!(
            host_blur.take_render_plan(Some(1), false, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::None,
                release_cef_only: vec![],
                yank_to_host: vec![TabYank { tab: 2, cutoff_seq: blur_seq }],
                restore_active: None,
                blur_cutoff_seq: 0,
            }
        );
    }

    #[test]
    fn hand_back_deactivation_and_threaded_cutoff_race() {
        let mut handoff = FocusHandoff::new();

        // 1. HandBack while window is inactive must blur GPUI handle only (zero native focus calls).
        let hb_inactive_seq = next_focus_seq();
        handoff.request_blur(Some(1), hb_inactive_seq);
        assert_eq!(
            handoff.take_render_plan(Some(1), true, false),
            RenderFocusPlan {
                gpui: GpuiFocusChange::BlurPage,
                release_cef_only: vec![],
                yank_to_host: vec![],
                restore_active: None,
                blur_cutoff_seq: hb_inactive_seq,
            }
        );

        // 2. Threaded cutoff race: press happens at press_seq, then HandBack cutoff happens at handback_seq > press_seq,
        //    then delayed CEF callback publishes click_seq = press_seq alongside a later got_focus_seq = got_after.
        //    Because click_seq <= click_cutoff_seq, it must NOT take focus back; instead unprompted got_focus yanks!
        let press_seq = next_focus_seq();
        let handback_seq = next_focus_seq();
        let got_after = next_focus_seq();
        handoff.request_blur(Some(1), handback_seq);
        assert!(!handoff.on_pump_tab(
            1,
            TabFocusEvents {
                click_seq: press_seq,
                key_seq: 0,
                got_focus_seq: got_after,
                took_focus_seq: 0,
            },
            true,
        ));
        assert_eq!(
            handoff.take_render_plan(Some(1), false, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::None,
                release_cef_only: vec![],
                yank_to_host: vec![TabYank { tab: 1, cutoff_seq: got_after }],
                restore_active: None,
                blur_cutoff_seq: 0,
            }
        );

        // 3. Window deactivation invalidates pre-deactivation clicks and stray yanks, preserves pending HandBack,
        //    and accepts post-deactivation clicks upon reactivation.
        let mut deact = FocusHandoff::new();
        let pre_click = next_focus_seq();
        let pre_stray = next_focus_seq();
        deact.on_pump_tab(1, TabFocusEvents { click_seq: pre_click, ..Default::default() }, true);
        deact.on_pump_tab(2, TabFocusEvents { got_focus_seq: pre_stray, ..Default::default() }, false);
        deact.on_window_deactivate(next_focus_seq());
        assert_eq!(deact.take_render_plan(Some(1), false, true), RenderFocusPlan::default());

        let post_click = next_focus_seq();
        assert!(deact.on_pump_tab(1, TabFocusEvents { click_seq: post_click, got_focus_seq: post_click, ..Default::default() }, true));
        assert_eq!(
            deact.take_render_plan(Some(1), false, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::FocusPage,
                ..Default::default()
            }
        );

        // Pre-blur click discarded by on_host_blur, and newer post-took click in same batch still wins.
        let mut blur_case = FocusHandoff::new();
        blur_case.on_pump_tab(1, TabFocusEvents { click_seq: next_focus_seq(), ..Default::default() }, true);
        blur_case.on_host_blur(next_focus_seq());
        assert_eq!(blur_case.take_render_plan(Some(1), false, true), RenderFocusPlan::default());
        let old_took = next_focus_seq();
        let newer_click = next_focus_seq();
        assert!(blur_case.on_pump_tab(1, TabFocusEvents { click_seq: newer_click, took_focus_seq: old_took, ..Default::default() }, true));
        assert_eq!(
            blur_case.take_render_plan(Some(1), false, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::FocusPage,
                ..Default::default()
            }
        );
    }

    #[test]
    fn background_stray_got_focus_never_inflates_active_tab_or_blur_cutoff() {
        let mut tracker = PressTracker::new();
        let mut handoff = FocusHandoff::new();
        let t0 = Instant::now();

        let hb_seq = next_focus_seq();
        let press_seq = next_focus_seq();
        let bg_stale_press = next_focus_seq();
        let bg_stray_got = next_focus_seq();

        handoff.request_blur(Some(1), hb_seq);
        assert!(!handoff.on_pump_tab(2, TabFocusEvents { got_focus_seq: bg_stray_got, ..Default::default() }, false));

        let plan = handoff.take_render_plan(Some(1), true, true);
        assert_eq!(
            plan,
            RenderFocusPlan {
                gpui: GpuiFocusChange::BlurPage,
                release_cef_only: vec![],
                yank_to_host: vec![
                    TabYank { tab: 1, cutoff_seq: hb_seq },
                    TabYank { tab: 2, cutoff_seq: bg_stray_got },
                ],
                restore_active: None,
                blur_cutoff_seq: hb_seq,
            }
        );
        assert_eq!(plan.active_blur_or_yank_cutoff(Some(1)), Some(hb_seq));

        // Genuine active-tab press at `press_seq` (`hb_seq < press_seq < bg_stray_got`) survives
        // active tab's per-tab yank cutoff, while background tab's stale press is cleared by `bg_stray_got`.
        tracker.record_with_seq(10, t0, press_seq);
        for y in &plan.yank_to_host {
            let native_hwnd = if y.tab == 1 { 10 } else { 20 };
            tracker.clear_target_up_to(native_hwnd, y.cutoff_seq);
        }
        assert_eq!(tracker.fresh_press_seq_after(10, hb_seq, t0 + Duration::from_millis(5)), Some(press_seq));
        assert!(handoff.recover_click_after_yank(1, press_seq));

        // Background stale press <= bg_stray_got is cleared under tab 2's cutoff.
        tracker.record_with_seq(20, t0, bg_stale_press);
        tracker.clear_target_up_to(20, bg_stray_got);
        assert_eq!(tracker.fresh_press_seq_after(20, 0, t0 + Duration::from_millis(5)), None);
    }

    #[test]
    fn post_yank_recovery_reclaims_focus_when_click_lands_after_precheck() {
        let mut tracker = PressTracker::new();
        let t0 = Instant::now();

        // Path A: click lands after precheck, right before/during yank_to_host when page was not GPUI-focused.
        let mut handoff = FocusHandoff::new();
        let hb_seq = next_focus_seq();
        let post_precheck_press = next_focus_seq();
        let post_precheck_got = next_focus_seq();

        handoff.request_blur(Some(1), hb_seq);
        assert_eq!(handoff.pending_press_cutoff_for_active(Some(1), false), Some(hb_seq));
        let plan = handoff.take_render_plan(Some(1), false, true);
        let cutoff = plan.recovery_cutoff_for_active(Some(1), handoff.recovery_floor_seq()).unwrap();
        assert_eq!(cutoff, hb_seq);

        tracker.record_with_seq(10, t0, post_precheck_press);
        for y in &plan.yank_to_host {
            if y.tab == 1 {
                tracker.clear_target_up_to(10, y.cutoff_seq);
            }
        }
        let recovered_seq = tracker.fresh_press_seq_after(10, cutoff, t0 + Duration::from_millis(5)).unwrap();
        assert_eq!(recovered_seq, post_precheck_press);
        assert!(handoff.recover_click_after_yank(1, recovered_seq));

        // Subsequent got_focus from give_keyboard(true) must NOT trigger unprompted yank,
        // and unsettled page focus receives FocusPage on the next render.
        assert!(!handoff.on_pump_tab(1, TabFocusEvents { got_focus_seq: post_precheck_got, ..Default::default() }, true));
        assert_eq!(
            handoff.take_render_plan(Some(1), false, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::FocusPage,
                ..Default::default()
            }
        );

        // Path B: queued BlurPage in on_blur rejects stale press <= cutoff, accepts newer click,
        // lifts handed_back for subsequent keystrokes, and stays gated by newer deactivation.
        let mut blur_handoff = FocusHandoff::new();
        let stale_press = next_focus_seq();
        let hb2 = next_focus_seq();
        let late_press = next_focus_seq();
        let key_after = next_focus_seq();

        blur_handoff.request_blur(Some(1), hb2);
        let blur_plan = blur_handoff.take_render_plan(Some(1), true, true);
        let blur_cutoff = blur_plan.blur_cutoff_seq;
        assert_eq!(blur_cutoff, hb2);

        tracker.record_with_seq(10, t0, stale_press);
        assert_eq!(tracker.fresh_press_seq_after(10, blur_cutoff, t0 + Duration::from_millis(5)), None);
        assert!(!blur_handoff.recover_click_after_yank(1, stale_press));

        tracker.record_with_seq(10, t0, late_press);
        assert_eq!(tracker.fresh_press_seq_after(10, blur_cutoff, t0 + Duration::from_millis(5)), Some(late_press));
        assert!(blur_handoff.recover_click_after_yank(1, late_press));
        assert!(blur_handoff.on_pump_tab(1, TabFocusEvents { key_seq: key_after, ..Default::default() }, true));

        let deact = next_focus_seq();
        blur_handoff.on_window_deactivate(deact);
        assert!(!blur_handoff.recover_click_after_yank(1, late_press));
    }

    #[test]
    fn background_only_yank_recovers_active_tab_click_without_inflating_cutoff() {
        let mut tracker = PressTracker::new();
        let t0 = Instant::now();

        // 1. Pre-check with background-only stray got_focus promotes active tab click whose press_seq
        //    is strictly above click_cutoff_seq even when below background tab's higher got_focus_seq.
        let mut pre = FocusHandoff::new();
        let stale_press = next_focus_seq();
        let cutoff = next_focus_seq();
        let active_press = next_focus_seq();
        let bg_got = next_focus_seq();

        pre.on_host_blur(cutoff);
        pre.on_pump_tab(2, TabFocusEvents { got_focus_seq: bg_got, ..Default::default() }, false);
        assert_eq!(pre.pending_press_cutoff_for_active(Some(1), true), None);
        assert_eq!(pre.pending_press_cutoff_for_active(Some(1), false), Some(cutoff));

        tracker.record_with_seq(10, t0, stale_press);
        assert_eq!(tracker.fresh_press_seq_after(10, cutoff, t0 + Duration::from_millis(1)), None);
        assert!(!pre.on_pending_click(1, stale_press));

        tracker.record_with_seq(10, t0 + Duration::from_millis(2), active_press);
        let pre_seq = tracker.fresh_press_seq_after(10, cutoff, t0 + Duration::from_millis(3)).unwrap();
        assert_eq!(pre_seq, active_press);
        assert!(pre.on_pending_click(1, pre_seq));
        assert_eq!(
            pre.take_render_plan(Some(1), false, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::FocusPage,
                release_cef_only: vec![TabYank { tab: 2, cutoff_seq: bg_got }],
                yank_to_host: vec![],
                restore_active: Some(1),
                blur_cutoff_seq: 0,
            }
        );

        // 2. Post-yank recovery when click lands after pre-check and a background-only yank executes:
        //    recovery_cutoff_for_active returns the host floor cutoff (never bg_got), rejecting stale presses
        //    <= floor and reclaiming active tab 1 clicks in `(floor, bg_got)`.
        let mut post = FocusHandoff::new();
        let stale2 = next_focus_seq();
        let cutoff2 = next_focus_seq();
        let late_active_press = next_focus_seq();
        let bg_got2 = next_focus_seq();

        post.on_host_blur(cutoff2);
        post.on_pump_tab(2, TabFocusEvents { got_focus_seq: bg_got2, ..Default::default() }, false);
        let floor = post.recovery_floor_seq();
        assert_eq!(floor, cutoff2);
        let plan = post.take_render_plan(Some(1), false, true);
        assert_eq!(
            plan,
            RenderFocusPlan {
                gpui: GpuiFocusChange::None,
                release_cef_only: vec![],
                yank_to_host: vec![TabYank { tab: 2, cutoff_seq: bg_got2 }],
                restore_active: None,
                blur_cutoff_seq: 0,
            }
        );
        let recovery_cutoff = plan.recovery_cutoff_for_active(Some(1), floor).unwrap();
        assert_eq!(recovery_cutoff, cutoff2);

        tracker.record_with_seq(10, t0, stale2);
        assert_eq!(tracker.fresh_press_seq_after(10, recovery_cutoff, t0 + Duration::from_millis(4)), None);
        assert!(!post.recover_click_after_yank(1, stale2));

        tracker.record_with_seq(10, t0 + Duration::from_millis(5), late_active_press);
        let got_late = tracker.fresh_press_seq_after(10, recovery_cutoff, t0 + Duration::from_millis(6)).unwrap();
        assert_eq!(got_late, late_active_press);
        assert!(post.recover_click_after_yank(1, got_late));
        assert_eq!(
            post.take_render_plan(Some(1), false, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::FocusPage,
                ..Default::default()
            }
        );

        // 3. Even after a background-only yank has already been consumed (`intent == None`, `unprompted_got` empty),
        //    a subsequent active-tab click while `!page_focused` (even producing NO CEF focus callback)
        //    is picked up by pending_press_cutoff_for_active(Some(1), false) on the next woken render.
        let mut after_bg = FocusHandoff::new();
        let floor3 = next_focus_seq();
        let bg_got3 = next_focus_seq();
        let silent_click = next_focus_seq();

        after_bg.on_host_blur(floor3);
        after_bg.on_pump_tab(2, TabFocusEvents { got_focus_seq: bg_got3, ..Default::default() }, false);
        let _consumed_bg = after_bg.take_render_plan(Some(1), false, true);

        assert_eq!(after_bg.pending_press_cutoff_for_active(Some(1), false), Some(floor3));
        tracker.record_with_seq(10, t0 + Duration::from_millis(8), silent_click);
        let silent_seq = tracker.fresh_press_seq_after(10, floor3, t0 + Duration::from_millis(9)).unwrap();
        assert_eq!(silent_seq, silent_click);
        assert!(after_bg.on_pending_click(1, silent_seq));
        assert_eq!(
            after_bg.take_render_plan(Some(1), false, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::FocusPage,
                ..Default::default()
            }
        );
    }

    #[test]
    fn win32_mid_activate_got_focus_before_return_recovers_on_confirm_and_rejects_eaten() {
        const WM_LBUTTONDOWN: u32 = 0x0201;
        let t0 = Instant::now();

        // A) Mid-dispatch on_got_focus(101) arrives before WM_MOUSEACTIVATE returns, so GPUI yanks
        //    with cutoff 101 (`clear_target_up_to(10, 101)` leaves unconfirmed 100 intact); confirming on
        //    DeliverDown assigns 102 (> 101) so render recovery lifts the yank and button-down pairs 102.
        let mut tracker = PressTracker::new();
        let mut handoff = FocusHandoff::new();
        assert_eq!(tracker.record_activate_with_seq(10, WM_LBUTTONDOWN, t0, 100), 100);
        assert_eq!(tracker.consume(10, FocusCallback::GotFocus, t0), None);
        handoff.on_pump_tab(1, TabFocusEvents { got_focus_seq: 101, ..Default::default() }, false);
        let plan_a = handoff.take_render_plan(Some(1), false, true);
        assert_eq!(plan_a.yank_to_host, vec![TabYank { tab: 1, cutoff_seq: 101 }]);
        tracker.clear_target_up_to(10, 101);
        assert_eq!(tracker.fresh_press_seq_after(10, 101, t0 + Duration::from_millis(1)), None);
        assert!(tracker.complete_activate_with_seq(
            10,
            WM_LBUTTONDOWN,
            MouseActivateOutcome::DeliverDown,
            t0 + Duration::from_millis(1),
            102,
        ));
        let rec_a = tracker.fresh_press_seq_after(10, 101, t0 + Duration::from_millis(1));
        assert_eq!(rec_a, Some(102));
        assert!(handoff.recover_click_after_yank(1, 102));
        assert_eq!(
            handoff.take_render_plan(Some(1), false, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::FocusPage,
                ..Default::default()
            }
        );
        assert_eq!(tracker.record_button_down_with_seq(10, WM_LBUTTONDOWN, t0 + Duration::from_millis(2), 103), 102);
        assert_eq!(tracker.consume(10, FocusCallback::GotFocus, t0 + Duration::from_millis(2)), Some(102));

        // B) Mid-dispatch on_got_focus(201) -> DeliverDown confirms 202 before render -> zero yank.
        let mut h_b = FocusHandoff::new();
        assert_eq!(tracker.record_activate_with_seq(20, WM_LBUTTONDOWN, t0, 200), 200);
        assert_eq!(tracker.consume(20, FocusCallback::GotFocus, t0), None);
        h_b.on_pump_tab(2, TabFocusEvents { got_focus_seq: 201, ..Default::default() }, false);
        assert!(tracker.complete_activate_with_seq(
            20,
            WM_LBUTTONDOWN,
            MouseActivateOutcome::DeliverDown,
            t0 + Duration::from_millis(1),
            202,
        ));
        let cutoff_b = h_b.pending_press_cutoff_for_active(Some(2), false).unwrap();
        assert_eq!(cutoff_b, 201);
        let seq_b = tracker.fresh_press_seq_after(20, cutoff_b, t0 + Duration::from_millis(1));
        assert_eq!(seq_b, Some(202));
        assert!(h_b.on_pending_click(2, seq_b.unwrap()));
        assert_eq!(
            h_b.take_render_plan(Some(2), false, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::FocusPage,
                ..Default::default()
            }
        );

        // C) Mid-dispatch on_got_focus(301) -> mid-dispatch record_button_down(302) confirms 302 -> recovers;
        //    later NoActivateAndEat(305) preserves 302.
        let mut h_c = FocusHandoff::new();
        assert_eq!(tracker.record_activate_with_seq(30, WM_LBUTTONDOWN, t0, 300), 300);
        h_c.on_pump_tab(3, TabFocusEvents { got_focus_seq: 301, ..Default::default() }, false);
        assert_eq!(tracker.record_button_down_with_seq(30, WM_LBUTTONDOWN, t0 + Duration::from_millis(1), 302), 302);
        let cutoff_c = h_c.pending_press_cutoff_for_active(Some(3), false).unwrap();
        assert_eq!(cutoff_c, 301);
        assert_eq!(tracker.fresh_press_seq_after(30, cutoff_c, t0 + Duration::from_millis(1)), Some(302));
        assert!(h_c.on_pending_click(3, 302));
        assert_eq!(
            h_c.take_render_plan(Some(3), false, true),
            RenderFocusPlan {
                gpui: GpuiFocusChange::FocusPage,
                ..Default::default()
            }
        );
        assert!(tracker.complete_activate_with_seq(
            30,
            WM_LBUTTONDOWN,
            MouseActivateOutcome::NoActivateAndEat,
            t0 + Duration::from_millis(2),
            305,
        ));
        assert_eq!(tracker.fresh_press_seq_after(30, 301, t0 + Duration::from_millis(2)), Some(302));
        assert_eq!(tracker.consume(30, FocusCallback::GotFocus, t0 + Duration::from_millis(2)), Some(302));

        // D) Mid-dispatch on_got_focus(401) -> NoActivateAndEat(402) drops unconfirmed click -> yank proceeds.
        let mut h_d = FocusHandoff::new();
        assert_eq!(tracker.record_activate_with_seq(40, WM_LBUTTONDOWN, t0, 400), 400);
        h_d.on_pump_tab(4, TabFocusEvents { got_focus_seq: 401, ..Default::default() }, false);
        assert!(!tracker.complete_activate_with_seq(
            40,
            WM_LBUTTONDOWN,
            MouseActivateOutcome::NoActivateAndEat,
            t0 + Duration::from_millis(1),
            402,
        ));
        let cutoff_d = h_d.pending_press_cutoff_for_active(Some(4), false).unwrap();
        assert_eq!(cutoff_d, 401);
        assert_eq!(tracker.fresh_press_seq_after(40, cutoff_d, t0 + Duration::from_millis(1)), None);
        let plan_d = h_d.take_render_plan(Some(4), false, true);
        assert_eq!(plan_d.yank_to_host, vec![TabYank { tab: 4, cutoff_seq: 401 }]);
        let rec_cutoff_d = plan_d
            .recovery_cutoff_for_active(Some(4), h_d.recovery_floor_seq())
            .unwrap();
        assert_eq!(tracker.fresh_press_seq_after(40, rec_cutoff_d, t0 + Duration::from_millis(1)), None);

        // E) Explicit clear_target(50) while in-flight -> DeliverDown returns false without resurrecting,
        //    while paired WH_MOUSE down still returns original seq 500 to avoid re-arming.
        assert_eq!(tracker.record_activate_with_seq(50, WM_LBUTTONDOWN, t0, 500), 500);
        tracker.clear_target(50);
        assert!(!tracker.complete_activate_with_seq(
            50,
            WM_LBUTTONDOWN,
            MouseActivateOutcome::DeliverDown,
            t0 + Duration::from_millis(1),
            505,
        ));
        assert_eq!(tracker.fresh_press_seq_after(50, 0, t0 + Duration::from_millis(1)), None);
        assert_eq!(tracker.consume(50, FocusCallback::GotFocus, t0 + Duration::from_millis(1)), None);
        assert_eq!(tracker.record_button_down_with_seq(50, WM_LBUTTONDOWN, t0 + Duration::from_millis(2), 506), 500);
        assert_eq!(tracker.fresh_press_seq_after(50, 0, t0 + Duration::from_millis(2)), None);
        assert_eq!(tracker.consume(50, FocusCallback::GotFocus, t0 + Duration::from_millis(2)), None);

        // F) Long reentrant WM_MOUSEACTIVATE (> PRESS_SPAN) is never age-pruned by upsert_press or consume
        //    while unconfirmed; confirmation resets its clock at confirmation time.
        let confirm_at = t0 + PRESS_SPAN + Duration::from_millis(250);
        assert_eq!(tracker.record_activate_with_seq(60, WM_LBUTTONDOWN, t0, 600), 600);
        assert_eq!(tracker.record_button_down_with_seq(61, WM_LBUTTONDOWN, confirm_at, 601), 601);
        assert_eq!(tracker.consume(60, FocusCallback::GotFocus, confirm_at + Duration::from_millis(1)), None);
        tracker.clear_target_up_to(60, 602);
        assert!(tracker.complete_activate_with_seq(
            60,
            WM_LBUTTONDOWN,
            MouseActivateOutcome::DeliverDown,
            confirm_at + Duration::from_millis(2),
            603,
        ));
        assert_eq!(tracker.fresh_press_seq_after(60, 602, confirm_at + Duration::from_millis(3)), Some(603));
        assert_eq!(tracker.consume(60, FocusCallback::GotFocus, confirm_at + Duration::from_millis(3)), Some(603));
        assert_eq!(tracker.record_button_down_with_seq(60, WM_LBUTTONDOWN, confirm_at + Duration::from_millis(4), 604), 603);
        let expired = confirm_at + Duration::from_millis(2) + PRESS_SPAN + Duration::from_millis(1);
        assert_eq!(tracker.fresh_press_seq_after(60, 602, expired), None);
        assert_eq!(tracker.consume(60, FocusCallback::GotFocus, expired), None);

        // Also confirm an unconsumed long activation survives past confirm_at and then expires strictly after confirm_at + PRESS_SPAN.
        assert_eq!(tracker.record_activate_with_seq(62, WM_LBUTTONDOWN, t0, 610), 610);
        assert!(tracker.complete_activate_with_seq(
            62,
            WM_LBUTTONDOWN,
            MouseActivateOutcome::ActivateAndEat,
            confirm_at + Duration::from_millis(2),
            611,
        ));
        assert_eq!(tracker.consume(62, FocusCallback::GotFocus, expired), None);
    }

    #[test]
    fn x11_barrier_fences_suppress_pre_barrier_presses_across_timeout_and_rendezvous() {
        let mut fences = X11BarrierFences::new();

        // 1) Normal timeout: caller installs active fence for watched target 10; target 20 stays unfenced;
        //    watcher retires barrier 1 with no leak and without calling on_early.
        assert_eq!(fences.finish_timeout_caller(1, &[10]), None);
        assert!(fences.is_fenced(10));
        assert!(!fences.is_fenced(20));
        assert!(!fences.is_fenced(0));
        let mut early_ran = false;
        fences.finish_timeout_watcher(1, || {
            early_ran = true;
            999
        });
        assert!(!early_ran);
        assert!(!fences.is_fenced(10));
        assert!(fences.active.is_empty());
        assert!(fences.completed_early.is_empty());

        // 2) End-to-end watcher-wins-timeout race: watcher clears pre-barrier press 49, allocates cutoff 50,
        //    and records post-barrier press 51 before caller runs; caller consumes Some(50) and preserves 51.
        let mut tracker = PressTracker::new();
        let t0 = Instant::now();
        let t1 = t0 + Duration::from_millis(5);
        tracker.record_with_seq(10, t0, 49);
        fences.finish_timeout_watcher(2, || {
            tracker.clear_target(10);
            50
        });
        assert_eq!(tracker.fresh_press_seq_after(10, 0, t0), None);
        tracker.record_with_seq(10, t1, 51);
        let caller_cutoff = fences.finish_timeout_caller(2, &[10]);
        assert_eq!(caller_cutoff, Some(50));
        assert_eq!(tracker.fresh_press_seq_after(10, 50, t1), Some(51));
        assert_eq!(tracker.consume(10, FocusCallback::GotFocus, t1), Some(51));
        assert!(!fences.is_fenced(10));
        assert!(fences.active.is_empty());
        assert!(fences.completed_early.is_empty());

        // 3) Concurrent out-of-order timed-out barriers keep target 10 fenced until both retire.
        assert_eq!(fences.finish_timeout_caller(4, &[10]), None);
        assert_eq!(fences.finish_timeout_caller(5, &[10]), None);
        assert!(fences.is_fenced(10));
        fences.finish_timeout_watcher(5, || panic!("active barrier should not invoke on_early"));
        assert!(fences.is_fenced(10));
        fences.finish_timeout_watcher(4, || panic!("active barrier should not invoke on_early"));
        assert!(!fences.is_fenced(10));
        assert!(fences.active.is_empty());
        assert!(fences.completed_early.is_empty());

        // 4) drop_target unfences destroyed target while preserving barrier id slot so watcher retires cleanly.
        assert_eq!(fences.finish_timeout_caller(6, &[10, 11]), None);
        fences.drop_target(10);
        assert!(!fences.is_fenced(10));
        assert!(fences.is_fenced(11));
        fences.finish_timeout_watcher(6, || panic!("dropped target barrier slot should still retire active fence"));
        assert!(!fences.is_fenced(11));
        assert!(fences.active.is_empty());
        assert!(fences.completed_early.is_empty());

        // 5) mark_watcher_dead clears active fences while keeping completed_early cutoffs for live callers:
        //    even if the watcher exits between recording post-barrier press 89 and caller collecting cutoff 88,
        //    finish_timeout_caller(8) still returns Some(88) so post-barrier click 89 survives.
        assert_eq!(fences.finish_timeout_caller(7, &[10]), None);
        let t2 = t1 + Duration::from_millis(5);
        tracker.record_with_seq(10, t2, 87);
        fences.finish_timeout_watcher(8, || {
            tracker.clear_target(10);
            88
        });
        let t3 = t2 + Duration::from_millis(5);
        tracker.record_with_seq(10, t3, 89);
        fences.mark_watcher_dead();
        assert!(fences.watcher_dead);
        assert!(!fences.is_fenced(10));
        assert!(fences.active.is_empty());
        assert_eq!(fences.finish_timeout_caller(8, &[10]), Some(88));
        assert_eq!(tracker.fresh_press_seq_after(10, 88, t3), Some(89));
        assert_eq!(tracker.consume(10, FocusCallback::GotFocus, t3), Some(89));
        assert!(fences.completed_early.is_empty());
        assert_eq!(fences.finish_timeout_caller(9, &[10]), None);
        fences.finish_timeout_watcher(10, || panic!("dead watcher must not invoke on_early"));
        assert!(!fences.is_fenced(10));
        assert!(fences.active.is_empty());
        assert!(fences.completed_early.is_empty());
    }
}
