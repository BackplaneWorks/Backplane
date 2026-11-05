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
pub enum Step { Home(usize), Select(usize), Popup(String), Type(String), Send, Wait(u64) }
#[derive(Clone, Debug)]
pub enum Goal { Home, Thread(String), Popup, Typed(String), Sent(usize), Sleep }
pub struct Pending { pub label: String, pub started: Instant, pub goal: Goal, pub armed: bool, pub seq: u64 }

/// Steps: {"select":N} {"popup":"/"} {"type":"text"} {"send":true} {"wait":ms}.
pub fn load(path: &Path) -> VecDeque<Step> {
    let list: Vec<Value> = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    list.iter().filter_map(|v| {
        if let Some(n) = v["home"].as_u64() { Some(Step::Home(n as usize)) }
        else if let Some(n) = v["select"].as_u64() { Some(Step::Select(n as usize)) }
        else if let Some(t) = v["popup"].as_str() { Some(Step::Popup(t.into())) }
        else if let Some(t) = v["type"].as_str() { Some(Step::Type(t.into())) }
        else if v["send"].as_bool() == Some(true) { Some(Step::Send) }
        else if let Some(ms) = v["wait"].as_u64() { Some(Step::Wait(ms)) }
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
            Goal::Thread(id) => self.data["thread"]["id"].as_str().map_or(false, |t| t.rsplit_once('|').map_or(t, |(_, i)| i) == id),
            Goal::Home => self.data["projectHome"].is_object(),
            Goal::Popup => self.popup && self.data["thread"]["skills"].as_array().map_or(false, |s| !s.is_empty()),
            Goal::Typed(text) => self.input.read(cx).content.as_ref() == text,
            Goal::Sent(before) => self.user_entries() > *before,
            Goal::Sleep => false,
        };
        if !done { return; }
        let (label, t0, seq) = (p.label.clone(), p.started, p.seq);
        self.pending.as_mut().unwrap().armed = true;
        window.on_next_frame(move |_, cx| {
            metric(started, "input", json!({"step": label, "ok": true, "latency_ms": (t0.elapsed().as_secs_f64() * 1e6).round() / 1e3}));
            let _ = this.update(cx, |t, cx| { if t.pending.as_ref().map_or(false, |p| p.seq == seq) { t.pending = None; t.pump(cx); } });
        });
        cx.notify();
    }

    fn begin(&mut self, label: String, goal: Goal, cx: &mut Context<Self>) {
        self.seq += 1;
        let seq = self.seq;
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

    fn set_text(&mut self, text: String, cx: &mut Context<Self>) {
        self.input.update(cx, |i, cx| i.set(text, cx));
    }
}
