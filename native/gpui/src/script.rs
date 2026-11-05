//! Metrics contract and scripted input. See README.md.
use crate::Backplane;
use gpui::*;
use serde_json::{json, Value};
use std::{collections::VecDeque, path::Path, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

pub fn metric(started: Instant, event: &str, mut fields: Value) {
    fields["event"] = json!(event);
    fields["t_ms"] = json!((started.elapsed().as_secs_f64() * 1000.0 * 1000.0).round() / 1000.0);
    fields["epoch_ms"] = json!(SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64));
    eprintln!("GPUI_METRIC {}", fields);
}

#[derive(Clone, Debug)]
pub enum Step { Home(usize), Select(usize), Popup(String), Type(String), Send, Wait(u64), Record(Option<String>), Click(String), Scroll(f32), NetSleep(f64), Unsupported(String) }
/// What a click on a registered control does; the e2e harness's controls inventory and `click` step use these.
#[derive(Clone, Debug)]
pub enum Ctl { None, Select(String), Home(String), Fold(String), Send, Skill(String), Page(String), Node(String, String) }
#[derive(Clone, Debug)]
pub struct CtlEntry { pub id: String, pub label: String, pub ctl: Ctl }
#[derive(Clone, Debug)]
pub enum Goal { Frame, Changed(u64), Reconnected, Home, Thread(String), Popup, Typed(String), Sent(usize), Sleep }
pub struct Pending { pub label: String, pub started: Instant, pub goal: Goal, pub armed: bool, pub seq: u64 }

/// Steps: {"select":N} {"home":N} {"popup":"/"} {"type":"text"} {"send":true} {"wait":ms}; e2e hooks: {"record":"start","name":"x"} {"record":"stop"}
/// {"click":"<control id>"} {"scroll":px} {"net_sleep":secs}; {"paste_image"|"open_viewer"|"open_settings"|"files_download":..} answer ok:false, error:not_implemented.
pub fn load(path: &Path) -> VecDeque<Step> {
    let list: Vec<Value> = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    list.iter().filter_map(|v| {
        if let Some(n) = v["home"].as_u64() { Some(Step::Home(n as usize)) }
        else if let Some(n) = v["select"].as_u64() { Some(Step::Select(n as usize)) }
        else if let Some(t) = v["popup"].as_str() { Some(Step::Popup(t.into())) }
        else if let Some(t) = v["type"].as_str() { Some(Step::Type(t.into())) }
        else if v["send"].as_bool() == Some(true) { Some(Step::Send) }
        else if let Some(ms) = v["wait"].as_u64() { Some(Step::Wait(ms)) }
        else if let Some(r) = v["record"].as_str() { Some(Step::Record(if r == "start" { Some(v["name"].as_str().unwrap_or("rec").to_string()) } else { None })) }
        else if let Some(id) = v["click"].as_str() { Some(Step::Click(id.into())) }
        else if let Some(n) = v["scroll"].as_f64() { Some(Step::Scroll(n as f32)) }
        else if let Some(sec) = v["net_sleep"].as_f64() { Some(Step::NetSleep(sec)) }
        else if let Some(k) = ["paste_image", "open_viewer", "open_settings", "files_download"].iter().find(|k| v.get(**k).is_some()) { Some(Step::Unsupported((*k).to_string())) }
        else { None }
    }).collect()
}

impl Backplane {
    pub fn m(&self, event: &str, fields: Value) { metric(self.started, event, fields); }

    /// Called from render: arms first_frame, usable, and the pending step's result.
    pub fn frame_marks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let started = self.started;
        let this = cx.entity().downgrade();
        if !self.first_frame {
            self.first_frame = true;
            window.on_next_frame(move |_, _| metric(started, "first_frame", json!({})));
            match window.gpu_specs() {
                Some(g) => metric(started, "gpu", json!({"device": g.device_name, "driver": g.driver_name, "driver_info": g.driver_info, "software": g.is_software_emulated})),
                None => metric(started, "gpu", json!({"device": null})),
            }
        }
        // G1: the first frame presented after the injected event (its local effect: pressed row, typed text, selection)
        if let Some((label, t0)) = self.ack_pending.take() {
            window.on_next_frame(move |_, _| metric(started, "ack", json!({"step": label, "ack_ms": (t0.elapsed().as_secs_f64() * 1e6).round() / 1e3})));
        }
        // G2: a frame timestamp for every frame drawn while a record is open (frames are kept coming while it is)
        if let Some((_, frames)) = &mut self.recording { frames.push((started.elapsed().as_secs_f64() * 1e6).round() / 1e3); window.request_animation_frame(); }
        if crate::art::rects_on() {
            // G3: the stable regions' rectangles (probes painted last frame), on change
            let snap = crate::art::layout_snapshot();
            if snap != self.last_layout { self.last_layout = snap.clone(); metric(started, "layout", json!({"rects": serde_json::from_str::<Value>(&snap).unwrap_or(Value::Null)})); }
            // G4: the controls on screen, on change
            let ctl: Vec<Value> = self.controls.iter().map(|c| json!({"id": c.id, "label": c.label, "enabled": !matches!(c.ctl, Ctl::None) , "visible": true, "wired": !matches!(c.ctl, Ctl::None)})).collect();
            let text = serde_json::to_string(&ctl).unwrap_or_default();
            if text != self.ctl_last { self.ctl_last = text; metric(started, "controls", json!({"controls": ctl})); }
        }
        let draws = self.data["projects"].as_array().map_or(false, |p| !p.is_empty());
        if !self.usable && draws {
            self.usable = true;
            let entries = self.data["thread"]["entries"].as_array().map_or(0, |e| e.len());
            let threads = self.thread_ids().len();
            window.on_next_frame(move |_, _| metric(started, "usable", json!({"threads": threads, "entries": entries})));
            let this = this.clone();
            window.on_next_frame(move |_, cx| { let _ = this.update(cx, |t, cx| t.pump(cx)); });
        }
        let Some(p) = &self.pending else { return };
        if p.armed { return; }
        let done = match &p.goal {
            Goal::Thread(id) => self.data["thread"]["id"].as_str().map_or(false, |t| t.rsplit_once('|').map_or(t, |(_, i)| i) == id) && self.data["thread"]["loading"] != true,
            Goal::Frame => true,
            Goal::Changed(epoch) => self.ui_epoch != *epoch,
            Goal::Reconnected => self.reconnected,
            Goal::Home => self.data["projectHome"].is_object(),
            Goal::Popup => self.popup && (self.data["thread"]["skills"].as_array().map_or(false, |s| !s.is_empty()) || self.input.read(cx).content.starts_with('/')),
            Goal::Typed(text) => self.input.read(cx).content.as_ref() == text,
            Goal::Sent(before) => self.user_entries() > *before,
            Goal::Sleep => false,
        };
        if !done { return; }
        let (label, t0, seq) = (p.label.clone(), p.started, p.seq);
        self.pending.as_mut().unwrap().armed = true;
        if let Some((id, action)) = self.click_info.take() { self.m("click", json!({"id": id, "changed": true, "action": action})); }
        // where the result came from: the hub, the cache of a thread seen before, or rows drawn locally while the hub answers
        let source = if self.popup_local() { "local" } else { self.thread_source };
        if source != "hub" { self.confirm = Some((format!("{}:hub", self.pending.as_ref().map_or(String::new(), |p| p.label.clone())), self.pending.as_ref().map_or(Instant::now(), |p| p.started))); }
        // where the time went: hub round trip, decode, delta/merge, applying the body, building the tree, waiting for the frame
        let phases = match (&self.timing, self.applied_at) {
            (Some(t), Some(a)) => Some((t.clone(), self.apply_ms, self.render_us as f64 / 1000.0, a)),
            _ => None,
        };
        window.on_next_frame(move |_, cx| {
            let mut fields = json!({"step": label, "ok": true, "latency_ms": (t0.elapsed().as_secs_f64() * 1e6).round() / 1e3, "source": source});
            if let Some((t, apply_ms, render_ms, applied)) = phases {
                let r = |v: f64| (v * 1e3).round() / 1e3;
                fields["hub_rtt_ms"] = json!(r(t.rtt_ms)); fields["decode_ms"] = json!(r(t.decode_ms)); fields["process_ms"] = json!(r(t.process_ms));
                fields["apply_ms"] = json!(r(apply_ms)); fields["render_build_ms"] = json!(r(render_ms));
                fields["applied_to_frame_ms"] = json!(r(applied.elapsed().as_secs_f64() * 1000.0)); fields["bytes"] = json!(t.bytes); fields["delta"] = json!(t.delta);
            }
            metric(started, "input", fields);
            let _ = this.update(cx, |t, cx| { if t.pending.as_ref().map_or(false, |p| p.seq == seq) { t.pending = None; t.pump(cx); } });
        });
        cx.notify();
    }

    fn begin(&mut self, label: String, goal: Goal, cx: &mut Context<Self>) {
        self.seq += 1;
        let seq = self.seq;
        self.timing = None; self.applied_at = None; self.ack_pending = Some((label.clone(), Instant::now()));
        self.pending = Some(Pending { label, started: Instant::now(), goal, armed: false, seq });
        // A step whose result never shows is reported failed after 15 s so the script still ends.
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(15)).await;
            let _ = this.update(cx, |t, cx| {
                if let Some(p) = &t.pending { if p.seq == seq && !p.armed {
                    let (l, t0) = (p.label.clone(), p.started);
                    t.m("input", json!({"step": l, "ok": false, "latency_ms": t0.elapsed().as_secs_f64() * 1e3}));
                    t.pending = None; t.pump(cx);
                } }
            });
        }).detach();
    }

    /// Run the next script step when none is pending.
    pub fn pump(&mut self, cx: &mut Context<Self>) {
        if self.pending.is_some() { return; }
        let Some(step) = self.steps.pop_front() else {
            if self.script_total > 0 && !self.script_done {
                self.script_done = true;
                self.m("script_done", json!({"steps": self.script_total}));
                cx.spawn(async move |_, cx| { cx.background_executor().timer(Duration::from_millis(300)).await; let _ = cx.update(|cx| cx.quit()); }).detach();
            }
            return;
        };
        match step {
            Step::Select(i) => {
                let ids = self.thread_ids();
                match ids.get(i).cloned() {
                    Some(id) => {
                        let raw = id.rsplit_once('|').map_or(id.as_str(), |(_, r)| r).to_string();
                        self.begin(format!("select:{i}"), Goal::Thread(raw), cx);
                        self.select_id(id, cx);
                    }
                    None => { self.m("input", json!({"step": format!("select:{i}"), "ok": false, "error": "no such thread"})); self.pump(cx); }
                }
            }
            Step::Home(i) => {
                let pid = self.data["projects"][i]["id"].as_str().unwrap_or("").to_string();
                let raw = pid.rsplit_once('|').map_or(pid.as_str(), |(_, r)| r).to_string();
                self.begin(format!("home:{i}"), Goal::Home, cx);
                let _ = self.tx.try_send(crate::hub::Command::Action("select".into(), format!("project-home-{raw}")));
            }
            Step::Popup(text) => { self.begin(format!("popup:{text}"), Goal::Popup, cx); self.set_text(text, cx); }
            Step::Type(text) => {
                let full = format!("{}{}", self.input.read(cx).content, text);
                self.begin(format!("type:{}", text.chars().count()), Goal::Typed(full.clone()), cx);
                self.set_text(full, cx);
            }
            Step::Send => {
                let before = self.user_entries();
                self.begin("send".into(), Goal::Sent(before), cx);
                self.do_send(cx);
            }
            Step::Record(Some(name)) => { self.m("record", json!({"state": "start", "name": name})); self.recording = Some((name, Vec::new())); cx.notify(); self.pump(cx); }
            Step::Record(None) => {
                if let Some((name, frames)) = self.recording.take() { self.m("frames", json!({"name": name, "n": frames.len(), "frames_ms": frames})); }
                self.pump(cx);
            }
            Step::Scroll(n) => {
                self.begin(format!("scroll:{n}"), Goal::Frame, cx);
                let o = self.scroll.offset(); self.scroll.set_offset(point(o.x, o.y - px(n))); self.ui_epoch += 1; cx.notify();
            }
            Step::NetSleep(secs) => {
                self.reconnected = false; self.begin(format!("net_sleep:{secs}"), Goal::Reconnected, cx);
                let _ = self.tx.try_send(crate::hub::Command::Sleep(secs));
            }
            Step::Unsupported(k) => { self.m("input", json!({"step": k, "ok": false, "error": "not_implemented"})); self.pump(cx); }
            Step::Click(id) => {
                let epoch = self.ui_epoch;
                match self.controls.iter().find(|c| c.id == id).cloned() {
                    None => { self.m("click", json!({"id": id, "found": false, "changed": false, "action": Value::Null})); self.pump(cx); }
                    Some(entry) => {
                        self.begin(format!("click:{id}"), Goal::Changed(epoch), cx);
                        let action = self.do_click(&entry.ctl, cx);
                        self.click_info = Some((id.clone(), action.clone()));
                        let seq = self.seq;
                        // a click that changes nothing is reported after 1.5 s (a dead control)
                        cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(Duration::from_millis(1500)).await;
                            let _ = this.update(cx, |t, cx| {
                                if t.pending.as_ref().map_or(false, |p| p.seq == seq && !p.armed) {
                                    t.m("click", json!({"id": id, "changed": false, "action": if action == "none" { Value::Null } else { json!(action) }}));
                                    t.click_info = None; t.pending = None; t.pump(cx);
                                }
                            });
                        }).detach();
                    }
                }
            }
            Step::Wait(ms) => {
                self.seq += 1;
                let seq = self.seq;
                self.pending = Some(Pending { label: "wait".into(), started: Instant::now(), goal: Goal::Sleep, armed: true, seq });
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_millis(ms)).await;
                    let _ = this.update(cx, |t, cx| { if t.pending.as_ref().map_or(false, |p| p.seq == seq) { t.pending = None; t.pump(cx); } });
                }).detach();
            }
        }
    }

    /// What a click on a registered control does; returns what it sent ("none" for a control with no handler).
    pub fn do_click(&mut self, ctl: &Ctl, cx: &mut Context<Self>) -> String {
        use crate::hub::Command;
        match ctl {
            Ctl::None => "none".into(),
            Ctl::Select(id) => { self.select_id(id.clone(), cx); "view.open thread".into() }
            Ctl::Home(pid) => { let _ = self.tx.try_send(Command::Action("select".into(), format!("project-home-{pid}"))); "view.action select project-home".into() }
            Ctl::Fold(v) => { let _ = self.tx.try_send(Command::Action("fold".into(), v.clone())); "view.action fold".into() }
            Ctl::Send => { self.do_send(cx); self.ui_epoch += 1; "send".into() }
            Ctl::Skill(w) => { let _ = self.tx.try_send(Command::Action("skill".into(), w.clone())); self.set_text(format!("{w} "), cx); self.popup = false; self.ui_epoch += 1; "view.action skill".into() }
            Ctl::Page(v) => { let _ = self.tx.try_send(Command::Page(v.clone())); "view.action native-sidebar-page".into() }
            Ctl::Node(a, p) => { let _ = self.tx.try_send(Command::Action(a.clone(), p.clone())); format!("view.action {a}") }
        }
    }
    /// The popup is showing the client's own slash rows because the hub has sent none yet.
    pub fn popup_local(&self) -> bool { self.popup && self.data["thread"]["skills"].as_array().map_or(true, |s| s.is_empty()) && self.pending.as_ref().map_or(false, |p| matches!(p.goal, Goal::Popup)) }
    fn set_text(&mut self, text: String, cx: &mut Context<Self>) {
        self.input.update(cx, |i, cx| i.set(text, cx));
    }
}
