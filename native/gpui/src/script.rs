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
pub enum Step { Cef(Value), Files(String), ScrollState(String), Home(usize), Select(usize), Tap(f32, f32, usize), Sheets(bool), Compare(Option<usize>), Hist(Option<i64>), HistMode(u8), HistPlay, ViewShow(String, String), Popup(String), Type(String), Send, Attach(String), Action(String, String), Field(String, String), Wait(u64), Record(Option<String>), Click(String), Imp(String), Read(String), View(String), Layers(bool, Option<f32>, f32), Expect(String), Pan(f32, f32), Zoom(f32, f32, f32), Scroll(f32), NetSleep(f64), Mark(String), Fit, Unsupported(String) }
/// What a click on a registered control does; the e2e harness's controls inventory and `click` step use these.
#[derive(Clone, Debug)]
pub enum Ctl { None, OpenPalette, Rail(String), PickFiles, ToggleSidebar, ToggleContext, Select(String), Home(String), Fold(String), Send, Skill(String), Page(String), Node(String, String), More, RenameSave(String), Imp(String), FocusSearch }
#[derive(Clone, Debug)]
pub struct CtlEntry { pub id: String, pub label: String, pub ctl: Ctl }
#[derive(Clone, Debug)]
pub enum Goal { Frame, Changed(u64), Reconnected, Home, Thread(String), Popup, Typed(String), Sent(usize), Sleep }
pub struct Pending { pub label: String, pub started: Instant, pub goal: Goal, pub armed: bool, pub seq: u64 }

/// Steps: {"select":N} {"home":N} {"popup":"/"} {"type":"text"} {"send":true} {"wait":ms}; e2e hooks: {"record":"start","name":"x"} {"record":"stop"}
/// {"click":"<control id>"} {"scroll":px} {"net_sleep":secs}; {"open_viewer":"files"} {"files":"select:N|open:N|key:down|filter:text|new:name|rename:N:name|delete:N|confirm_delete|menu:N|upload:PATH|download:N|zip|copy:N|up|back|cancel"} {"files_download":N|"name"}; {"paste_image"|"open_settings"} answer ok:false, error:not_implemented.
pub fn load(path: &Path) -> VecDeque<Step> {
    let list: Vec<Value> = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    list.iter().filter_map(|v| {
        if let Some(n) = v["home"].as_u64() { Some(Step::Home(n as usize)) }
        else if let Some(n) = v["select"].as_u64() { Some(Step::Select(n as usize)) }
        else if let Some(a) = v["viewshow"].as_array() { Some(Step::ViewShow(a.first().and_then(|x| x.as_str()).unwrap_or("").into(), a.get(1).and_then(|x| x.as_str()).unwrap_or("").into())) }
        else if v.get("hist").is_some() { Some(Step::Hist(v["hist"].as_i64())) }
        else if let Some(m) = v["histmode"].as_u64() { Some(Step::HistMode(m as u8)) }
        else if v["histplay"].as_bool() == Some(true) { Some(Step::HistPlay) }
        else if v.get("compare").is_some() { Some(Step::Compare(v["compare"].as_u64().map(|n| n as usize))) }
        else if let Some(on) = v["sheets"].as_bool() { Some(Step::Sheets(on)) }
        else if let Some(a) = v["tap"].as_array() { Some(Step::Tap(a.first().and_then(Value::as_f64).unwrap_or(0.5) as f32, a.get(1).and_then(Value::as_f64).unwrap_or(0.5) as f32, a.get(2).and_then(Value::as_u64).unwrap_or(0) as usize)) }
        else if let Some(t) = v["popup"].as_str() { Some(Step::Popup(t.into())) }
        else if let Some(t) = v["type"].as_str() { Some(Step::Type(t.into())) }
        else if v["send"].as_bool() == Some(true) { Some(Step::Send) }
        else if let Some(p) = v["attach"].as_str() { Some(Step::Attach(p.into())) }
        else if let Some(a) = v["field"].as_array() { Some(Step::Field(a.first().and_then(|x| x.as_str()).unwrap_or("").into(), a.get(1).and_then(|x| x.as_str()).unwrap_or("").into())) }
        else if let Some(a) = v["action"].as_array() { Some(Step::Action(a.first().and_then(|x| x.as_str()).unwrap_or("").into(), a.get(1).and_then(|x| x.as_str()).unwrap_or("").into())) }
        else if let Some(ms) = v["wait"].as_u64() { Some(Step::Wait(ms)) }
        else if let Some(r) = v["record"].as_str() { Some(Step::Record(if r == "start" { Some(v["name"].as_str().unwrap_or("rec").to_string()) } else { None })) }
        else if let Some(id) = v["click"].as_str() { Some(Step::Click(id.into())) }
        else if let Some(op) = v["imp"].as_str() { Some(Step::Imp(op.into())) }
        else if let Some(what) = v["read"].as_str() { Some(Step::Read(what.into())) }
        else if let Some(on) = v["layers"].as_bool() { Some(Step::Layers(on, v["max_h"].as_f64().map(|x| x as f32), v["scroll"].as_f64().unwrap_or(0.0) as f32)) }
        else if let Some(name) = v["mark"].as_str() { Some(Step::Mark(name.into())) }
        else if v["fit"].as_bool() == Some(true) { Some(Step::Fit) }
        else if let Some(what) = v["expect"].as_str() { Some(Step::Expect(what.into())) }
        else if v["cef"].is_object() { Some(Step::Cef(v["cef"].clone())) }
        else if let Some(kind) = v["view"].as_str() { Some(Step::View(kind.into())) }
        else if let Some(a) = v["pan"].as_array() { Some(Step::Pan(a.first().and_then(Value::as_f64).unwrap_or(0.0) as f32, a.get(1).and_then(Value::as_f64).unwrap_or(0.0) as f32)) }
        else if let Some(a) = v["zoom"].as_array() { Some(Step::Zoom(a.first().and_then(Value::as_f64).unwrap_or(0.0) as f32, a.get(1).and_then(Value::as_f64).unwrap_or(0.5) as f32, a.get(2).and_then(Value::as_f64).unwrap_or(0.5) as f32)) }
        else if let Some(n) = v["scroll_state"].as_str() { Some(Step::ScrollState(n.to_string())) }
        else if let Some(n) = v["scroll"].as_f64() { Some(Step::Scroll(n as f32)) }
        else if let Some(sec) = v["net_sleep"].as_f64() { Some(Step::NetSleep(sec)) }
        else if let Some(k) = v["open_viewer"].as_str() { Some(Step::View(k.into())) }
        else if let Some(f) = v["files"].as_str() { Some(Step::Files(f.into())) }
        else if let Some(n) = v["files_download"].as_str().map(String::from).or_else(|| v["files_download"].as_u64().map(|n| n.to_string())) { Some(Step::Files(format!("download:{n}"))) }
        else if let Some(k) = ["paste_image", "open_settings"].iter().find(|k| v.get(**k).is_some()) { Some(Step::Unsupported((*k).to_string())) }
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
            let rects = crate::art::ctl_rects();
            let ctl: Vec<Value> = self.controls.iter().map(|c| json!({"id": c.id, "label": c.label, "rect": rects.get(&c.id).map(|r| json!({"x": r[0], "y": r[1], "w": r[2], "h": r[3]})), "enabled": !matches!(c.ctl, Ctl::None), "visible": true, "wired": !matches!(c.ctl, Ctl::None), "disabled_look": matches!(c.ctl, Ctl::None)})).collect();
            let text = serde_json::to_string(&ctl).unwrap_or_default();
            if text != self.ctl_last {
                self.ctl_last = text; metric(started, "controls", json!({"controls": ctl}));
            }
        }
        if crate::art::rects_on() {
            // links in messages as painted last frame (the check harness aims at these; a code block's Copy is not one)
            let links: Vec<Value> = self.link_and_copy_rects().into_iter().filter(|(id, _, _)| id.starts_with("link:")).map(|(id, href, b)| json!({"id": id, "href": href, "rect": {"x": f32::from(b.origin.x), "y": f32::from(b.origin.y), "w": f32::from(b.size.width), "h": f32::from(b.size.height)}})).collect();
            let text = serde_json::to_string(&links).unwrap_or_default();
            if !self.links_sent || text != self.links_last { self.links_sent = true; self.links_last = text; metric(started, "links", json!({"links": links, "supported": true})); }
        }
        let draws = self.data["projects"].is_array(); // (a fresh home has none, and is as usable as one that has)
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
            Goal::Home => self.data["projectHome"].is_object() || self.project_page.is_some(),
            Goal::Popup => self.data["thread"]["skills"].as_array().map_or(false, |s| !s.is_empty()), // the hub's rows, nothing local
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

    fn begin(&mut self, label: String, goal: Goal, cx: &mut Context<Self>) { self.begin_for(label, goal, 15, cx) }
    /// `fail_after`: seconds before a step whose result never shows is reported failed (a net_sleep is its sleep + 15).
    fn begin_for(&mut self, label: String, goal: Goal, fail_after: u64, cx: &mut Context<Self>) {
        self.seq += 1;
        let seq = self.seq;
        self.timing = None; self.applied_at = None; self.ack_pending = Some((label.clone(), Instant::now()));
        self.pending = Some(Pending { label, started: Instant::now(), goal, armed: false, seq });
        // A step whose result never shows is reported failed after 15 s so the script still ends.
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(fail_after)).await;
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
                // BACKPLANE_GPUI_HOLD keeps the window up after the script (the harness screenshots it, then ends the process)
                if std::env::var_os("BACKPLANE_GPUI_HOLD").is_none() { cx.spawn(async move |_, cx| { cx.background_executor().timer(Duration::from_millis(300)).await; let _ = cx.update(|cx| cx.quit()); }).detach(); }
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
                self.open_project(raw.clone(), cx);
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
                if let Some((name, frames)) = self.recording.take() {
                    self.m("frames", json!({"name": name, "n": frames.len(), "frames_ms": frames}));
                    // the plot canvas's own CPU per paint while the record was open (culling, path building, tessellation, submission), not the rasterizer's time
                    let mut ms = std::mem::take(&mut self.screens.plotview.paint_ms);
                    if !ms.is_empty() {
                        ms.sort_by(|a, b| a.partial_cmp(b).unwrap()); let p = |q: f32| ms[((ms.len() - 1) as f32 * q).round() as usize];
                        let sp = std::mem::take(&mut self.screens.plotview.split);
                        let part = |k: usize| { let mut v: Vec<f32> = sp.iter().map(|x| x[k]).collect(); v.sort_by(|a, b| a.partial_cmp(b).unwrap()); if v.is_empty() { (0.0, 0.0) } else { (v[(v.len() - 1) / 2], v[((v.len() - 1) as f32 * 0.95).round() as usize]) } };
                        let (lp, tess, sub) = (part(0), part(1), part(2));
                        self.m("plot_paint", json!({"name": name, "n": ms.len(), "p50_ms": p(0.5), "p95_ms": p(0.95), "max_ms": p(1.0), "loop_cull_build_p50_p95_ms": [lp.0, lp.1], "tessellate_p50_p95_ms": [tess.0, tess.1], "paint_path_p50_p95_ms": [sub.0, sub.1]}));
                    }
                }
                self.pump(cx);
            }
            // where the timeline stands: e2e reads this, a picture cannot say whether a scroll held
            Step::ScrollState(name) => {
                let max = f32::from(self.timeline.max_offset_for_scrollbar().height);
                let y = -(-f32::from(self.timeline.scroll_px_offset_for_scrollbar().y)).min(max);
                let rows = self.data["thread"]["entries"].as_array().map_or(&[][..], |e| e.as_slice());
                let ids: std::collections::HashSet<&str> = rows.iter().filter_map(|e| e["id"].as_str()).collect();
                let top = self.timeline.logical_scroll_top();
                self.m("scroll", json!({"name": name, "y": y, "max": max, "entries": rows.len(), "dups": rows.len() - ids.len(), "first": rows.first().map_or("", |e| e["id"].as_str().unwrap_or("")),
                    "last": rows.last().map_or("", |e| e["id"].as_str().unwrap_or("")), "top": rows.get(top.item_ix).map_or("(end)", |e| e["id"].as_str().unwrap_or("")), "px": f32::from(top.offset_in_item), "more": self.data["thread"]["historyOlder"].as_str().is_some_and(|c| !c.is_empty())}));
                self.pump(cx);
            }
            Step::Scroll(n) => {
                self.begin(format!("scroll:{n}"), Goal::Frame, cx);
                // at the end the list's own top is past its last row, a screen below where the wheel starts from
                let at_end = self.timeline.logical_scroll_top().item_ix >= self.timeline.item_count();
                self.timeline.scroll_by(px(n) - if at_end { self.timeline.viewport_bounds().size.height } else { px(0.) }); self.ui_epoch += 1; cx.notify();
            }
            Step::NetSleep(secs) => {
                self.reconnected = false; self.begin_for(format!("net_sleep:{secs}"), Goal::Reconnected, secs.max(0.0) as u64 + 15, cx);
                let _ = self.tx.try_send(crate::hub::Command::Sleep(secs));
            }
            Step::Unsupported(k) => { self.m("input", json!({"step": k, "ok": false, "error": "not_implemented"})); self.pump(cx); }
            Step::View(kind) => { // open a viewer page like the rail does (the idempotent `view` action)
                let _ = self.tx.try_send(crate::screens::viewer::view_kind(&kind)); self.m("view", json!({"kind": kind})); cx.notify(); self.pump(cx);
            }
            Step::Files(spec) => { let r = crate::screens::filesui::script(self, &spec, cx); self.m("files", json!({"step": spec, "ok": r.is_ok(), "error": r.err()})); self.pump(cx); }
            Step::Cef(cmd) => { // {"cef":{"report":true|"goto":url|"click":[x,y]|"type":text|"handback":true}}: the embedded browser page, for the e2e (feature cef; else not implemented)
                #[cfg(feature = "cef")]
                { let page = self.screens.browser.as_ref().and_then(|(_, p)| p.read(cx).cef.clone());
                  match page { Some(p) => { let r = p.update(cx, |c, cx| c.script(&cmd, cx)); self.m("cef", r); } None => { let v = self.data["thread"]["viewer"].clone(); self.m("cef", json!({"ok": false, "error": "no embedded page", "have_page": self.screens.browser.is_some(), "viewer_open": v["open"], "viewer_keys": v.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>())})) } } }
                #[cfg(not(feature = "cef"))]
                { let _ = cmd; self.m("cef", json!({"ok": false, "error": "built without the cef feature"})); }
                self.pump(cx);
            }
            Step::Mark(name) => { self.m("mark", json!({"name": name})); self.pump(cx); }
            Step::Fit => { self.screens.plotview.view = None; self.ui_epoch += 1; self.begin("fit".into(), Goal::Frame, cx); cx.notify(); }
            Step::Expect(what) => { // a run that cannot measure what it claims stops itself: "canvas" needs a plot canvas with real height
                let h = self.screens.plotview.bounds.map_or(0.0, |b| f32::from(b.size.height));
                if what == "canvas" && h < 200.0 { self.m("not_usable", json!({"why": "plot canvas has no height", "canvas_h": h})); eprintln!("expect canvas: height {h} px: stopping"); std::process::exit(4); }
                self.m("expect", json!({"what": what, "canvas_h": h})); self.pump(cx);
            }
            Step::Layers(on, cap, scroll) => { let pv = &mut self.screens.plotview; pv.layers_cap = cap; pv.layers_scroll.set_offset(point(px(0.), px(-scroll)));
                self.screens.plotview.layers_open = on; self.ui_epoch += 1; self.begin("layers".into(), Goal::Frame, cx); cx.notify(); }
            Step::ViewShow(kind, path) => { let t = self.selected.clone(); self.view_show(&t, &kind, &path, cx); self.m("viewshow", json!({"kind": kind, "path": path})); self.ui_epoch += 1; self.begin("viewshow".into(), Goal::Frame, cx); cx.notify(); }
            Step::Compare(i) => { // the Compare menu's row i (null: put the comparison away); the versions are asked for first
                match i { None => crate::screens::histbar::compare(self, None),
                    Some(i) => { if self.screens.plotview.hist.refs.is_none() { crate::screens::histbar::ask_refs(self); } let r = self.screens.plotview.hist.refs.clone().and_then(|r| r.rows.get(i).cloned());
                        if let Some((_, name, commit)) = r { crate::screens::histbar::compare(self, Some((&commit, &name))); } else { self.m("compare", json!({"error": "no such version yet"})); } } }
                self.ui_epoch += 1; self.begin("compare".into(), Goal::Frame, cx); cx.notify(); }
            Step::Hist(i) => { // the history bar's step i (0-based; null or negative: live); the steps are asked for first
                if self.screens.plotview.hist.steps.is_none() { crate::screens::histbar::ask_steps(self); self.m("hist", json!({"asked": true})); }
                else { crate::screens::histbar::select(self, i.filter(|n| *n >= 0).map(|n| n as usize)); self.m("hist", json!({"at": i})); }
                self.ui_epoch += 1; self.begin("hist".into(), Goal::Frame, cx); cx.notify(); }
            Step::HistMode(m) => { crate::screens::histbar::mode(self, m); self.ui_epoch += 1; self.begin("histmode".into(), Goal::Frame, cx); cx.notify(); }
            Step::HistPlay => { crate::screens::histbar::play(self, cx); self.ui_epoch += 1; self.begin("histplay".into(), Goal::Frame, cx); cx.notify(); }
            Step::Sheets(on) => { self.screens.plotview.sheets_open = on; self.ui_epoch += 1; self.begin("sheets".into(), Goal::Frame, cx); cx.notify(); }
            Step::Tap(fx, fy, slot) => { // a tap at (fx, fy) as fractions of canvas `slot` (0 the Board/Schematic page, 1 a Split's schematic, 2 its board): inspect what lies there
                let (view, bounds) = { let pv = self.screens.pv(slot); (pv.view, pv.bounds) };
                let panes = self.data["thread"]["viewer"]["panes"].as_array().cloned().unwrap_or_default();
                let (key, offs) = if slot == 0 { (self.screens.plot.frame.as_ref().map(|f| f.key.clone()).unwrap_or_default(), self.data["thread"]["viewer"]["off"].as_u64().unwrap_or(0) as u32) }
                    else { let p = panes.get(slot - 1).cloned().unwrap_or_default(); (p["key"].as_str().unwrap_or("").to_string(), p["off"].as_u64().unwrap_or(0) as u32) };
                if let Some(b) = bounds { let at = (f32::from(b.origin.x) + fx * f32::from(b.size.width), f32::from(b.origin.y) + fy * f32::from(b.size.height)); crate::screens::plotview::tap_at(self, slot, &key, offs, at, view, Some(b)); }
                let got = self.screens.pv(slot).picked.is_some(); self.m("tap", json!({"fx": fx, "fy": fy, "slot": slot, "picked": got})); self.ui_epoch += 1; self.begin("tap".into(), Goal::Frame, cx); cx.notify();
            }
            Step::Pan(dx, dy) => { // a drag of the plot canvas by (dx, dy) window px
                let pv = &mut self.screens.plotview; if let Some(v) = pv.view { pv.view = Some(v.pan(dx, dy)); }
                self.ui_epoch += 1; self.begin("pan".into(), Goal::Frame, cx); cx.notify();
            }
            Step::Zoom(steps, fx, fy) => { // wheel notches at (fx, fy) as fractions of the canvas
                let pv = &mut self.screens.plotview;
                if let (Some(v), Some(b)) = (pv.view, pv.bounds) {
                    let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
                    pv.view = Some(v.zoom((1.0 + steps * 16.0 * 0.0015).clamp(0.5, 2.0), fx * w, fy * h, w, h, pv.fitted));
                }
                self.ui_epoch += 1; self.begin("zoom".into(), Goal::Frame, cx); cx.notify();
            }
            Step::Read(what) => {
                // G6: the composer's text, or the thread selection (GPUI has no text selection yet: "" with supported:false)
                let (text, supported) = match what.as_str() { "composer" => (self.input.read(cx).content.to_string(), true),
                    // the settings overlay's structure (labels, button actions, field names; never a field's value), to design its layout from the real payload
                    "settings" => (crate::screens::settings::structure(&self.data["settings"]), true),
                    _ => (String::new(), false) };
                self.m("read", json!({"what": what, "text": text, "supported": supported}));
                self.pump(cx);
            }
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
            Step::Field(name, text) => {
                // the e2e hook for typing into a named form field (the rename box): its text set as typing would
                let input = match name.as_str() { "pq" => self.screens.projects.search.clone(), "picker" => self.screens.projects.folder.clone(), _ => self.screens.fields.get(&name).map(|s| s.input.clone()) };
                if let Some(input) = input { input.update(cx, |i, cx| i.set(text.clone(), cx)); }
                self.seq += 1; let seq = self.seq;
                self.pending = Some(Pending { label: "field".into(), started: Instant::now(), goal: Goal::Sleep, armed: true, seq });
                cx.spawn(async move |this, cx| { cx.background_executor().timer(Duration::from_millis(100)).await; let _ = this.update(cx, |t, cx| { if t.pending.as_ref().map_or(false, |p| p.seq == seq) { t.pending = None; t.pump(cx); } }); }).detach();
            }
            Step::Action(a, v) => {
                // the e2e hook for a view action a menu or a row would send (e.g. a delete asked on a thread that is not selected)
                let v = if a.starts_with("row-") { crate::screens::tools::qualify(&self.data, &v) } else { v.clone() }; // a row's own menu sends the id as the sidebar carries it
                let _ = self.tx.try_send(crate::hub::Command::Action(a.clone(), v));
                self.seq += 1; let seq = self.seq;
                self.pending = Some(Pending { label: "action".into(), started: Instant::now(), goal: Goal::Sleep, armed: true, seq });
                cx.spawn(async move |this, cx| { cx.background_executor().timer(Duration::from_millis(100)).await; let _ = this.update(cx, |t, cx| { if t.pending.as_ref().map_or(false, |p| p.seq == seq) { t.pending = None; t.pump(cx); } }); }).detach();
            }
            Step::Imp(op) => {
                // Improvements e2e hook (screens::improvements::script_op): the same code as the panel, then a short wait for its effect
                crate::screens::improvements::script_op(self, &op, cx);
                self.seq += 1; let seq = self.seq;
                self.pending = Some(Pending { label: "imp".into(), started: Instant::now(), goal: Goal::Sleep, armed: true, seq });
                cx.spawn(async move |this, cx| { cx.background_executor().timer(Duration::from_millis(250)).await; let _ = this.update(cx, |t, cx| { if t.pending.as_ref().map_or(false, |p| p.seq == seq) { t.pending = None; t.pump(cx); } }); }).detach();
            }
            Step::Attach(path) => {
                // the e2e hook for a dropped or picked file: the same path as a drop on the window (screens::glue::attach_event)
                self.attach_event(&crate::screens::composer::ComposerEvent::Files(vec![std::path::PathBuf::from(path)]), cx);
                self.seq += 1; let seq = self.seq;
                self.pending = Some(Pending { label: "attach".into(), started: Instant::now(), goal: Goal::Sleep, armed: true, seq });
                cx.spawn(async move |this, cx| { cx.background_executor().timer(Duration::from_millis(100)).await; let _ = this.update(cx, |t, cx| { if t.pending.as_ref().map_or(false, |p| p.seq == seq) { t.pending = None; t.pump(cx); } }); }).detach();
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
            Ctl::Rail(kind) => { self.screens.rail_pending = Some(kind.clone()); self.screens.rail_at = Some(Instant::now()); self.screens.plot_click = matches!(kind.as_str(), "board" | "schematic").then(Instant::now); let _ = self.tx.try_send(crate::screens::viewer::view_kind(kind)); self.ui_epoch += 1; "view.action view".into() }
            Ctl::PickFiles => { self.pick_files(cx); self.ui_epoch += 1; "attach picker".into() }
            Ctl::ToggleSidebar => { self.sidebar_toggle(cx); "toggle sidebar".into() }
            Ctl::ToggleContext => { self.screens.ctx_open = !self.screens.ctx_open; self.ui_epoch += 1; cx.notify(); "context panel".into() }
            Ctl::Select(id) => { self.select_id(id.clone(), cx); "view.open thread".into() }
            Ctl::Home(pid) => { self.open_project(pid.clone(), cx); "project page".into() }
            Ctl::Fold(v) => { self.fold_click(v.clone()); "view.action fold".into() }
            Ctl::Send => { self.do_send(cx); self.ui_epoch += 1; "send".into() }
            Ctl::Skill(w) => { let _ = self.tx.try_send(Command::Action("skill".into(), w.clone())); self.await_completion(cx); self.ui_epoch += 1; "view.action skill".into() }
            Ctl::Page(v) => { let _ = self.tx.try_send(Command::Page(v.clone())); "view.action native-sidebar-page".into() }
            Ctl::OpenPalette => { self.palette_open(cx); self.ui_epoch += 1; "open palette".into() }
            Ctl::FocusSearch => { self.focus_search(cx); self.ui_epoch += 1; "focus search".into() }
            Ctl::More => { self.screens.menu_open = !self.screens.menu_open; self.ui_epoch += 1; cx.notify(); "thread menu".into() }
            Ctl::RenameSave(id) => { let text = self.screens.fields.get("title").map(|s| s.input.read(cx).content.to_string()).unwrap_or_default(); if let Some(c) = crate::screens::tools::rename_save(id, &text) { let _ = self.tx.try_send(c); } "view.action rename-save".into() }
            Ctl::Imp(id) => { crate::screens::improvements::act(self, id, cx); self.ui_epoch += 1; format!("improvements {id}") }
            Ctl::Node(a, p) => { if a == "flag" && p == "settings" { self.open_settings(cx); return format!("view.action {a}"); } if a == "proj-add" || a == "proj-new" || a == "new-thread" { self.arm_new(); } let _ = self.tx.try_send(Command::Action(a.clone(), p.clone())); format!("view.action {a}") }
        }
    }
    /// The popup is always the hub's rows now (no client-side rows), so a popup step is never local.
    pub fn popup_local(&self) -> bool { false }
    fn set_text(&mut self, text: String, cx: &mut Context<Self>) {
        self.input.update(cx, |i, cx| i.set(text, cx));
    }
}
