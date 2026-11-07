//! The performance overlay (Ctrl+Shift+H): the last few seconds of the window's work, ported from the Bend client's Hud (src/core/perf.bend on the tag `main-bend-final`).
//! Hidden, the state is `None`: `mark` builds no element, no input listener exists, and each hook is one relaxed load of `ON`. Nothing is measured before the key is pressed.
//! Up, it refreshes with the frames the window draws anyway and asks for none (no repaint loop), so an idle window keeps showing its last busy moment.
//!
//! GPUI has no frame timing of its own, so a frame's steps come from what can be hooked: `render` starts it (build), the first mark's prepaint ends layout, the marks split
//! prepaint and paint by region, and a `cx.defer` from `render` runs in the same effect flush as `window.present()`, after it, so it ends the frame. The GPU may still be working then.
use crate::{art, hub::Timing, theme};
use gpui::{prelude::*, *};
use std::{cell::RefCell, collections::{BTreeMap, VecDeque}, sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed}, time::{Duration, Instant}};

/// How far back the overlay looks.
pub const SPAN_MS: f64 = 5000.0;
/// The text is worked out at most this often, however fast frames come.
pub const REFRESH: Duration = Duration::from_millis(250);
/// Most values a row keeps (a bound on memory, past any display's rate over the span).
const CAP: usize = 2048;
/// An input with no frame within this long changed nothing on screen: it is not charged to a later frame.
const INPUT_MS: f64 = 1000.0;
/// The rows of the table: label, kind, name.
const TABLE: &[(&str, &str, &str)] = &[("frame", "span", "frame"), (" build", "span", "build"), (" layout", "span", "layout"), (" prepaint", "span", "prepaint"), (" paint", "span", "paint"), (" present", "span", "present"),
    ("input>px", "span", "input"), ("hub>px", "span", "hub"), ("hub rtt", "hub", "rtt"), (" decode", "hub", "decode"), (" merge", "hub", "merge"), (" apply", "hub", "apply")];

static ON: AtomicBool = AtomicBool::new(false);
static WIRE_MSGS: AtomicU64 = AtomicU64::new(0);
static WIRE_BYTES: AtomicU64 = AtomicU64::new(0);
thread_local! { static STATE: RefCell<Option<(Instant, Perf)>> = const { RefCell::new(None) }; }

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase { Prepaint, Paint }

/// What a row's values come to.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Stats { pub n: usize, pub sum: f64, pub mean: f64, pub p50: f64, pub p99: f64, pub max: f64 }
impl Stats {
    /// Percentiles are nearest-rank: each is a value that happened.
    pub fn of(mut v: Vec<f64>) -> Stats {
        if v.is_empty() { return Stats::default() }
        v.sort_by(f64::total_cmp);
        let (n, sum) = (v.len(), v.iter().sum::<f64>());
        let rank = |q: f64| v[((q * n as f64).ceil() as usize).clamp(1, n) - 1];
        Stats { n, sum, mean: sum / n as f64, p50: rank(0.5), p99: rank(0.99), max: v[n - 1] }
    }
}

/// Values with the time each happened (ms).
#[derive(Default)]
struct Series(VecDeque<(f64, f64)>);
impl Series {
    fn add(&mut self, t: f64, v: f64) { if self.0.len() == CAP { self.0.pop_front(); } self.0.push_back((t, v)); }
    fn trim(&mut self, cut: f64) { while self.0.front().is_some_and(|x| x.0 < cut) { self.0.pop_front(); } }
    fn stats(&self) -> Stats { Stats::of(self.0.iter().map(|x| x.1).collect()) }
}

/// The frame being drawn: when each step ended (ms), and the marks passed in order.
#[derive(Default)]
struct Frame { start: Option<f64>, built: Option<f64>, prepaints: Vec<(&'static str, f64)>, paints: Vec<(&'static str, f64)> }

/// The process, read at a refresh.
#[derive(Clone, Copy, Default, Debug)]
pub struct Sample { pub cpu_ms: f64, pub rss_mb: Option<f64>, pub peak_mb: f64, pub wire_msgs: u64, pub wire_bytes: u64 }

/// Counters only grow, so a rate over the window is the difference of two readings.
#[derive(Clone, Copy, Default)]
struct Reading { t: f64, cpu_ms: f64, wire_msgs: u64, wire_bytes: u64, events: u64, pictures: u64, deltas: u64 }

#[derive(Default)]
pub struct Perf {
    rows: BTreeMap<(&'static str, &'static str), Series>, frame: Frame, input_at: Option<f64>, hub_at: Option<f64>,
    events: u64, pictures: u64, deltas: u64, readings: VecDeque<Reading>, lines: Vec<SharedString>, next: f64,
}
impl Perf {
    fn add(&mut self, kind: &'static str, name: &'static str, t: f64, v: f64) { self.rows.entry((kind, name)).or_default().add(t, v); }
    fn stats(&self, kind: &str, name: &str) -> Stats { self.rows.iter().find(|((k, n), _)| *k == kind && *n == name).map_or(Stats::default(), |(_, s)| s.stats()) }
    fn frame_start(&mut self, t: f64) { self.frame = Frame { start: Some(t), ..Frame::default() }; }
    fn built(&mut self, t: f64) { self.frame.built = Some(t); }
    fn stamp(&mut self, phase: Phase, name: &'static str, t: f64) { match phase { Phase::Prepaint => self.frame.prepaints.push((name, t)), Phase::Paint => self.frame.paints.push((name, t)) } }
    /// A mark's region runs to the next mark, so the last mark only closes the one before it. A frame that never finished building (a measuring page) is dropped.
    fn frame_end(&mut self, t: f64) {
        let f = std::mem::take(&mut self.frame);
        let (Some(start), Some(built)) = (f.start, f.built) else { return };
        let first = f.prepaints.first().map_or(built, |m| m.1); let pre = f.prepaints.last().map_or(first, |m| m.1); let paint = f.paints.last().map_or(pre, |m| m.1);
        for (name, v) in [("frame", t - start), ("build", built - start), ("layout", first - built), ("prepaint", pre - first), ("paint", paint - pre), ("present", t - paint)] { self.add("span", name, t, v); }
        for (kind, marks) in [("prepaint", &f.prepaints), ("paint", &f.paints)] { for w in marks.windows(2) { self.add(kind, w[0].0, t, w[1].1 - w[0].1); } }
        if let Some(at) = self.input_at.take().filter(|at| t - at <= INPUT_MS) { self.add("span", "input", t, t - at); }
        if let Some(at) = self.hub_at.take() { self.add("span", "hub", t, t - at); }
    }
    /// The first input since the last frame is the one the next frame answers.
    fn input(&mut self, t: f64) { self.input_at.get_or_insert(t); }
    fn event(&mut self, name: &'static str, t: f64, ms: f64) { self.events += 1; self.add("event", name, t, ms); }
    fn picture(&mut self, t: f64, x: &Timing, apply_ms: f64, at: f64) {
        self.pictures += 1; self.deltas += x.delta as u64; self.hub_at.get_or_insert(at);
        for (name, v) in [("rtt", x.rtt_ms), ("decode", x.decode_ms), ("merge", x.process_ms), ("apply", apply_ms)] { self.add("hub", name, t, v); }
    }
    /// The heaviest rows of these kinds by total time, the overlay's own left out.
    fn slowest(&self, kinds: &[&str], k: usize) -> Vec<(String, Stats)> {
        let mut v: Vec<(String, Stats)> = self.rows.iter().filter(|((kind, name), _)| kinds.contains(kind) && *name != "overlay").map(|((kind, name), s)| (format!("{kind} {name}"), s.stats())).collect();
        v.sort_by(|a, b| b.1.sum.total_cmp(&a.1.sum)); v.truncate(k); v
    }
    /// Drops what left the window, takes a reading and works out the text.
    fn refresh(&mut self, t: f64, s: Sample, key: &str) {
        let cut = t - SPAN_MS;
        self.rows.values_mut().for_each(|r| r.trim(cut)); self.rows.retain(|_, r| !r.0.is_empty());
        self.readings.push_back(Reading { t, cpu_ms: s.cpu_ms, wire_msgs: s.wire_msgs, wire_bytes: s.wire_bytes, events: self.events, pictures: self.pictures, deltas: self.deltas });
        while self.readings.len() > 1 && self.readings[1].t <= cut { self.readings.pop_front(); }
        self.lines = self.text(t, s, key).into_iter().map(SharedString::from).collect();
        self.next = t + REFRESH.as_secs_f64() * 1000.0;
    }
    fn text(&self, t: f64, s: Sample, key: &str) -> Vec<String> {
        let mem = match s.rss_mb { Some(r) => format!("rss {r:.0} MB  peak {:.0} MB", s.peak_mb), None => format!("peak {:.0} MB", s.peak_mb) };
        let frames = self.stats("span", "frame");
        if frames.n == 0 { return vec![format!("measuring ({key} hides this)"), mem] }
        let span = t.clamp(1.0, SPAN_MS);
        let (a, b) = (self.readings.front().copied().unwrap_or_default(), self.readings.back().copied().unwrap_or_default());
        let secs = ((b.t - a.t) / 1000.0).max(0.001);
        let rate = |x: u64, y: u64| y.saturating_sub(x) as f64 / secs;
        let mut l = vec![format!("{:<10}{:>5}{:>7}{:>7}{:>7}{:>7}", format!("last {:.1} s", span / 1000.0), "n", "mean", "p50", "p99", "max")];
        for (label, kind, name) in TABLE {
            let x = self.stats(kind, name);
            l.push(if x.n == 0 { format!("{label:<10}{:>5}", 0) } else { format!("{label:<10}{:>5}{:>7}{:>7}{:>7}{:>7}", x.n, ms(x.mean), ms(x.p50), ms(x.p99), ms(x.max)) });
        }
        l.push(format!("fps {:.0}  cpu {:.0}%  {mem}", frames.n as f64 * 1000.0 / span, (b.cpu_ms - a.cpu_ms) / (b.t - a.t).max(1.0) * 100.0));
        l.push(format!("wire {:.0}/s {:.2} MB/s  events {:.0}/s", rate(a.wire_msgs, b.wire_msgs), rate(a.wire_bytes, b.wire_bytes) / 1048576.0, rate(a.events, b.events)));
        l.push(format!("pictures {:.0}/s  deltas {:.0}/s", rate(a.pictures, b.pictures), rate(a.deltas, b.deltas)));
        for (title, kinds, k) in [("slowest paints", &["prepaint", "paint"][..], 4), ("slowest events", &["event"][..], 3)] {
            l.push(format!("{title:<22}{:>9}{:>6}", "ms total", "n"));
            for (name, x) in self.slowest(kinds, k) { l.push(format!("{name:<22}{:>9}{:>6}", ms(x.sum), x.n)); }
        }
        let own = self.stats("prepaint", "overlay").sum + self.stats("paint", "overlay").sum + self.stats("span", "refresh").sum;
        l.push(format!("overlay {} ms/frame", ms(own / frames.n as f64)));
        l
    }
}

fn ms(v: f64) -> String { if v < 10.0 { format!("{v:.2}") } else if v < 100.0 { format!("{v:.1}") } else { format!("{v:.0}") } }
/// Resident memory from /proc/self/status, in MB.
fn rss_of(status: &str) -> Option<f64> { status.lines().find_map(|l| l.strip_prefix("VmRSS:"))?.split_whitespace().next()?.parse::<f64>().ok().map(|kb| kb / 1024.0) }
fn sample() -> Sample {
    let (cpu_ms, peak_mb) = crate::plat::usage();
    // no cheap current figure on a Mac: its line gives the peak alone
    let rss_mb = if cfg!(target_os = "linux") { std::fs::read_to_string("/proc/self/status").ok().and_then(|s| rss_of(&s)) } else { None };
    Sample { cpu_ms, rss_mb, peak_mb, wire_msgs: WIRE_MSGS.load(Relaxed), wire_bytes: WIRE_BYTES.load(Relaxed) }
}

/// Whether the overlay is up.
#[inline] pub fn on() -> bool { ON.load(Relaxed) }
fn with<R>(f: impl FnOnce(&mut Perf, f64) -> R) -> Option<R> { STATE.with(|s| s.borrow_mut().as_mut().map(|(t0, p)| { let t = t0.elapsed().as_secs_f64() * 1000.0; f(p, t) })) }
/// The key: up from empty, or down with everything measured dropped. Answers whether it is up now.
pub fn toggle() -> bool {
    let up = !on(); STATE.with(|s| *s.borrow_mut() = up.then(|| (Instant::now(), Perf::default()))); ON.store(up, Relaxed); up
}
/// A frame from the hub (the one hook off the window's thread).
#[inline] pub fn wire(bytes: usize) { if on() { WIRE_MSGS.fetch_add(1, Relaxed); WIRE_BYTES.fetch_add(bytes as u64, Relaxed); } }
pub fn frame_start() { with(|p, t| p.frame_start(t)); }
pub fn built() { with(|p, t| p.built(t)); }
pub fn frame_end() { with(|p, t| p.frame_end(t)); }
fn stamp(phase: Phase, name: &'static str) { with(|p, t| p.stamp(phase, name, t)); }
/// A hub event handled, which took from `t0` to now.
pub fn event(name: &'static str, t0: Instant) { with(|p, t| p.event(name, t, t0.elapsed().as_secs_f64() * 1000.0)); }
/// A picture applied: where its time went, and when the hub's frame arrived (hub>px counts from there, the wait for the window's thread included).
pub fn picture(x: &Timing, apply_ms: f64) { with(|p, t| p.picture(t, x, apply_ms, x.at.map_or(t, |i| t - i.elapsed().as_secs_f64() * 1000.0))); }
/// The overlay's text, for the script's `{"read":"perf"}`.
pub fn lines() -> Option<Vec<String>> { with(|p, _| p.lines.iter().map(|l| l.to_string()).collect()) }

/// A region starts here: nothing at all while the overlay is hidden.
pub fn mark(name: &'static str) -> Option<AnyElement> {
    on().then(|| canvas(move |_, _, _| stamp(Phase::Prepaint, name), move |_, _, _, _| stamp(Phase::Paint, name)).absolute().size_0().into_any_element())
}
/// The overlay: top right, over everything, with no hit regions (a press goes through to what is under it). Its last element closes the last region and listens for input
/// (only while painted, so hidden there is no listener), in the capture phase so the time starts before anything handles the event.
pub fn overlay(light: bool, key: &str) -> AnyElement {
    let t0 = Instant::now();
    let lines = with(|p, t| { if t >= p.next { p.refresh(t, sample(), key); } p.lines.clone() }).unwrap_or_default();
    with(|p, t| p.add("span", "refresh", t, t0.elapsed().as_secs_f64() * 1000.0));
    let c = |n: &str| theme::color(light, n);
    let end = canvas(|_, _, _| stamp(Phase::Prepaint, "end"), |_, _, window, _| {
        stamp(Phase::Paint, "end");
        window.on_mouse_event(|_: &MouseDownEvent, phase, _, _| if phase == DispatchPhase::Capture { with(|p, t| p.input(t)); });
        window.on_mouse_event(|_: &ScrollWheelEvent, phase, _, _| if phase == DispatchPhase::Capture { with(|p, t| p.input(t)); });
        window.on_key_event(|_: &KeyDownEvent, phase, _, _| if phase == DispatchPhase::Capture { with(|p, t| p.input(t)); });
    }).absolute().size_0();
    deferred(div().absolute().top(px(60.)).right(px(84.)).px_2().py_1().bg(c("raise")).border_1().border_color(c("line")).text_color(c("text")).text_size(px(11.)).line_height(px(15.)).font(art::mono_font()).whitespace_nowrap()
        .children(mark("overlay")).children(lines.into_iter().map(|l| div().child(l))).child(end)).with_priority(1000).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-9, "{a} is not {b}"); }
    fn s() -> Sample { Sample { cpu_ms: 0.0, rss_mb: Some(100.0), peak_mb: 120.0, wire_msgs: 0, wire_bytes: 0 } }
    /// A frame starting at `t`, with marks: built 1 ms in, a, b, then the end of the frame at +10.
    fn frame(p: &mut Perf, t: f64) {
        p.frame_start(t); p.built(t + 1.0);
        for (n, at) in [("a", 2.0), ("b", 3.0), ("end", 4.0)] { p.stamp(Phase::Prepaint, n, t + at); }
        for (n, at) in [("a", 5.0), ("b", 7.0), ("end", 8.0)] { p.stamp(Phase::Paint, n, t + at); }
        p.frame_end(t + 10.0);
    }

    #[test] fn percentiles_are_values_that_happened() {
        let x = Stats::of((1..=100).rev().map(f64::from).collect()); assert_eq!((x.n, x.p50, x.p99, x.max), (100, 50.0, 99.0, 100.0)); close(x.mean, 50.5); close(x.sum, 5050.0);
        assert_eq!(Stats::of(vec![]), Stats::default()); let one = Stats::of(vec![7.0]); assert_eq!((one.p50, one.p99, one.max), (7.0, 7.0, 7.0));
    }
    #[test] fn a_frame_splits_into_its_steps_and_regions() {
        let mut p = Perf::default(); frame(&mut p, 100.0);
        for (name, v) in [("frame", 10.0), ("build", 1.0), ("layout", 1.0), ("prepaint", 2.0), ("paint", 4.0), ("present", 2.0)] { close(p.stats("span", name).sum, v); }
        close(p.stats("prepaint", "a").sum, 1.0); close(p.stats("paint", "a").sum, 2.0); close(p.stats("paint", "b").sum, 1.0);
        assert_eq!(p.stats("paint", "end").n, 0, "the last mark only closes the one before it");
    }
    #[test] fn a_frame_that_never_finished_building_is_dropped() {
        let mut p = Perf::default(); p.frame_start(1.0); p.frame_end(2.0); assert_eq!(p.stats("span", "frame").n, 0);
        p.frame_end(3.0); assert_eq!(p.stats("span", "frame").n, 0, "a defer with no frame open");
    }
    #[test] fn input_and_hub_are_charged_to_the_next_frame_once() {
        let mut p = Perf::default(); p.input(95.0); p.input(99.0); p.picture(98.0, &Timing { rtt_ms: 4.0, ..Timing::default() }, 0.5, 96.0);
        frame(&mut p, 100.0); close(p.stats("span", "input").sum, 15.0); close(p.stats("span", "hub").sum, 14.0); close(p.stats("hub", "rtt").sum, 4.0); close(p.stats("hub", "apply").sum, 0.5);
        frame(&mut p, 200.0); assert_eq!((p.stats("span", "input").n, p.stats("span", "hub").n), (1, 1), "the next frame answered nothing");
    }
    #[test] fn an_input_that_changed_nothing_is_not_charged_to_a_later_frame() {
        let mut p = Perf::default(); p.input(0.0); frame(&mut p, 5000.0); assert_eq!(p.stats("span", "input").n, 0); assert_eq!(p.input_at, None);
    }
    #[test] fn the_window_forgets_what_is_older_than_the_span() {
        let mut p = Perf::default(); frame(&mut p, 0.0); frame(&mut p, 4000.0); p.refresh(4100.0, s(), "K"); assert_eq!(p.stats("span", "frame").n, 2);
        p.refresh(5100.0, s(), "K"); assert_eq!(p.stats("span", "frame").n, 1, "the first frame left the window");
        p.refresh(9100.0, s(), "K"); assert!(p.rows.is_empty(), "rows that emptied go");
    }
    #[test] fn a_row_is_bounded() { let mut r = Series::default(); for i in 0..CAP + 10 { r.add(i as f64, 1.0); } assert_eq!(r.0.len(), CAP); assert_eq!(r.0[0].0, 10.0); }
    #[test] fn rates_are_the_difference_of_two_readings() {
        let mut p = Perf::default(); p.refresh(1000.0, Sample { cpu_ms: 100.0, ..s() }, "K");
        for i in 0..60 { frame(&mut p, 1000.0 + i as f64 * 16.0); }
        for _ in 0..30 { p.event("picture", 2000.0, 1.0); p.picture(2000.0, &Timing { delta: true, ..Timing::default() }, 0.0, 2000.0); }
        for _ in 0..10 { p.picture(2000.0, &Timing::default(), 0.0, 2000.0); }
        p.refresh(3000.0, Sample { cpu_ms: 600.0, wire_msgs: 80, wire_bytes: 2 << 20, ..s() }, "K");
        let text = p.lines.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n");
        assert!(text.contains("fps 20  cpu 25%  rss 100 MB  peak 120 MB"), "{text}"); assert!(text.contains("wire 40/s 1.00 MB/s  events 15/s"), "{text}"); assert!(text.contains("pictures 20/s  deltas 15/s"), "{text}");
    }
    #[test] fn the_slowest_are_ranked_by_total_time_and_leave_the_overlay_out() {
        let mut p = Perf::default(); for _ in 0..3 { p.add("paint", "main", 0.0, 2.0); } p.add("prepaint", "viewer", 0.0, 9.0); p.add("paint", "overlay", 0.0, 99.0); p.add("event", "picture", 0.0, 50.0);
        let v = p.slowest(&["prepaint", "paint"], 4); assert_eq!(v.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["prepaint viewer", "paint main"]);
        assert_eq!(p.slowest(&["paint"], 1).len(), 1);
    }
    #[test] fn before_any_frame_it_says_so_and_names_the_key() {
        let mut p = Perf::default(); p.refresh(0.0, s(), "Ctrl+Shift+H"); assert_eq!(p.lines, vec![SharedString::from("measuring (Ctrl+Shift+H hides this)"), SharedString::from("rss 100 MB  peak 120 MB")]);
        p.refresh(1.0, Sample { rss_mb: None, ..s() }, "K"); assert_eq!(p.lines[1], "peak 120 MB", "a Mac has only the peak");
    }
    #[test] fn the_table_keeps_its_columns_when_a_row_is_empty() {
        let mut p = Perf::default(); frame(&mut p, 0.0); p.refresh(1000.0, s(), "K");
        assert!(p.lines[0].starts_with("last 1.0 s    n   mean")); assert!(p.lines[1].starts_with("frame         1   10.0"), "{}", p.lines[1]); assert!(p.lines.iter().any(|l| l.as_ref() == format!("{:<10}{:>5}", "hub rtt", 0)));
    }
    #[test] fn resident_memory_is_read_from_status() { assert_eq!(rss_of("Name:\tx\nVmPeak:\t9 kB\nVmRSS:\t  2048 kB\n"), Some(2.0)); assert_eq!(rss_of("Name:\tx\n"), None); }
    #[test] fn hidden_it_measures_nothing_and_up_it_starts_empty() {
        assert!(!on()); assert!(mark("x").is_none(), "no element"); let n = WIRE_MSGS.load(Relaxed);
        wire(10); frame_start(); built(); frame_end(); event("picture", Instant::now()); assert_eq!(WIRE_MSGS.load(Relaxed), n); assert_eq!(lines(), None);
        assert!(toggle() && on()); wire(10); assert_eq!(WIRE_MSGS.load(Relaxed), n + 1); frame_start(); built(); frame_end(); assert_eq!(with(|p, _| p.stats("span", "frame").n), Some(1));
        assert!(!toggle()); assert_eq!(with(|_, _| ()), None, "what it measured is dropped"); assert!(toggle()); assert_eq!(with(|p, _| p.rows.len()), Some(0)); toggle();
    }
}
