//! `live`: from the microphone (or stdin) to words as they are spoken, by
//! OpenAI's realtime session or the local engine.

use crate::args::Args;
use crate::audio::{level_of, Resampler};
use crate::cmd::opts;
use crate::devices::{list_devices, Devices};
use crate::local::{self, Word};
use crate::openai::{Hooks, Realtime};
use crate::out::{emit, end, error, fail, log};
use crate::recorder::{missing, open_source, Source};
use crate::session::{self, In, Sink};
use crate::sherpa;
use crate::stop::{self, Stop};
use crate::vad::{Action, Rechunker, Segmenter, BYTES_PER_MS};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};

const LIVE_LIMIT: Duration = Duration::from_secs(10 * 60);
const WAIT_MS: u64 = 8000;
pub const CHUNK: usize = 100 * BYTES_PER_MS; // 100 ms of audio
const LEVEL: Duration = Duration::from_millis(250);
const LABEL_WAIT: Duration = Duration::from_millis(150); // how long "ready" waits for the microphone's name
const NO_AUDIO: Duration = Duration::from_millis(3000);
const SILENT: Duration = Duration::from_millis(4000);
const LATE: Duration = Duration::from_millis(1000);

struct Events;

fn words_json(w: &[Word]) -> Value {
    Value::Array(w.iter().map(Word::to_json).collect())
}

impl Sink for Events {
    fn hyp(&mut self, item: &str, words: &[Word], end: i64) {
        emit(json!({"event": "hyp", "item": item, "words": words_json(words), "end": end}));
    }

    fn voice(&mut self, item: &str, emb: &[f32]) {
        emit(json!({"event": "voice", "item": item, "emb": emb.iter().map(|x| (x * 10000.0).round() / 10000.0).collect::<Vec<f32>>()}));
    }

    fn turn(&mut self, item: &str, p: f32, ms: i64) {
        emit(json!({"event": "turn", "item": item, "p": (p * 1000.0).round() / 1000.0, "ms": ms}));
    }

    fn done(&mut self, item: &str, words: &[Word], decode_ms: i64, final_ms: i64) {
        let text: Vec<&str> = words.iter().map(|w| w.0.as_str()).collect();
        emit(json!({"event": "done", "item": item, "text": text.join(" "), "words": words_json(words), "decode_ms": decode_ms, "final_ms": final_ms}));
    }
}

/// The local engine, loading while the microphone already records: audio
/// sent meanwhile waits in the inbox.
fn start_local(dir: PathBuf, model: String, server: String, stop: Stop) -> (std_mpsc::Sender<In>, std::thread::JoinHandle<()>) {
    let (tx, rx) = std_mpsc::channel();
    let reply_to = tx.clone();
    let t = std::thread::spawn(move || {
        let t0 = Instant::now();
        match sherpa::engine(&dir, &model, &server) {
            Ok((rec, vad, resident)) => {
                emit(json!({"event": "loaded", "ms": t0.elapsed().as_millis() as u64, "resident": resident}));
                session::run(rx, reply_to, vad, rec, Events, true);
            }
            Err(e) => {
                error(&format!("could not load the local model ({e})"));
                stop.stop();
            }
        }
    });
    (tx, t)
}

/// The level meter, the hints, and where each piece of audio goes.
struct Feed {
    local: Option<std_mpsc::Sender<In>>,
    cloud: mpsc::UnboundedSender<Action>,
    seg: Segmenter,
    re: Rechunker,
    conv: Option<Resampler>,
    bytes: usize,
    peak: f64,
    heard: bool,
    hints: Vec<&'static str>,
}

impl Feed {
    fn hint(&mut self, h: &'static str) {
        if !self.hints.contains(&h) {
            self.hints.push(h);
            emit(json!({"event": "hint", "hint": h}));
        }
    }

    fn cloud(&self, acts: Vec<Action>) {
        if self.local.is_none() {
            for a in acts {
                let _ = self.cloud.send(a);
            }
        }
    }

    fn feed(&mut self, b0: Vec<u8>) {
        // audio from a client (--source stdin --rate R) at another rate is
        // converted to the engine's
        let b = match &mut self.conv {
            Some(c) => c.push(&b0),
            None => b0,
        };
        if b.is_empty() {
            return;
        }
        self.bytes += b.len();
        for c in self.re.push(&b) {
            let acts = self.seg.push(&c);
            self.cloud(acts);
            self.peak = self.peak.max(self.seg.last_rms);
            if self.seg.last_loud && !self.heard {
                self.heard = true;
                if !self.hints.is_empty() {
                    self.hint("ok");
                }
            }
        }
        if let Some(l) = &self.local {
            let _ = l.send(In::Pcm(b));
        }
    }

    fn meter(&mut self, since: Instant) {
        emit(json!({"event": "level", "v": level_of(self.peak)}));
        self.peak = 0.0;
        let el = since.elapsed();
        if el >= NO_AUDIO && self.bytes == 0 {
            self.hint("no-audio");
        }
        if el >= SILENT && self.bytes > 0 && !self.heard {
            self.hint("silent");
        }
    }
}

fn say_ready(src: &str, a: &Args, devs: &Option<Devices>) {
    let device = if !a.device.is_empty() { a.device.clone() } else { devs.as_ref().map_or(String::new(), |d| d.default.clone()) };
    let label = devs.as_ref().and_then(|d| d.devices.iter().find(|x| x.name == device)).map_or(String::new(), |d| d.label.clone());
    emit(json!({"event": "ready", "recorder": src, "device": device, "label": label}));
}

/// Reads SRC until a stop or its end.
async fn pump(mut src: Source, a: &Args, feed: &mut Feed, stop: &Stop, mut lrx: oneshot::Receiver<Option<Devices>>) {
    // the device's name is a nicety: "ready" goes now with what is known,
    // and again with the name if the listing comes in a little later
    let mut devs = None;
    let mut listed = false;
    tokio::select! {
        r = &mut lrx => {
            devs = r.ok().flatten();
            listed = true;
        }
        _ = tokio::time::sleep(LABEL_WAIT) => {}
    }
    say_ready(&src.name, a, &devs);
    let t0 = Instant::now();
    let mut meter = tokio::time::interval_at(tokio::time::Instant::now() + LEVEL, LEVEL);
    meter.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut metering = true;
    let mut sr = stop.subscribe();
    let mut killed = false;
    let mut late = Box::pin(tokio::time::sleep(Duration::from_secs(86400)));
    loop {
        tokio::select! {
            biased;
            _ = meter.tick(), if metering => {
                if stop.stopped() {
                    metering = false;
                } else {
                    feed.meter(t0);
                }
            }
            r = &mut lrx, if !listed => {
                devs = r.ok().flatten();
                listed = true;
                let named = devs.as_ref().is_some_and(|d| !d.default.is_empty() || !d.devices.is_empty());
                if !stop.stopped() && feed.hints.is_empty() && named {
                    say_ready(&src.name, a, &devs);
                }
            }
            r = src.next() => match r {
                Some(b) => feed.feed(b),
                None => {
                    if !stop.stopped() && !src.is_stdin() {
                        let why = src.why().await;
                        error(&format!("the recorder stopped ({why})"));
                    }
                    break;
                }
            },
            _ = stop::stopped(&mut sr), if !killed => {
                // let the recorder hand over what it has, briefly
                killed = true;
                late.as_mut().reset(tokio::time::Instant::now() + LATE);
                src.kill();
                if src.is_stdin() {
                    while let Some(b) = src.ready() {
                        feed.feed(b);
                    }
                    break;
                }
            }
            _ = &mut late, if killed => break,
        }
    }
    if !killed {
        src.kill();
    }
}

pub async fn live(a: Args) -> ! {
    let local = a.engine == "local";
    let rate: u32 = if local { local::RATE } else { crate::vad::RATE };
    let dir = PathBuf::from(&a.local_dir);
    if local {
        if let Some(why) = local::ready(&dir, &a.model) {
            fail(&why);
        }
    }
    let o = if local { None } else { Some(opts(&a)) };
    if let Some(why) = missing(&a.source) {
        fail(&why);
    }

    let stop = Stop::new(true);
    stop::on_signals(stop.clone());
    if !a.always {
        stop::after(stop.clone(), LIVE_LIMIT);
    }
    crate::resident::SPEAKER.store(a.speaker, std::sync::atomic::Ordering::Relaxed);
    let from_stdin = a.source == "stdin";
    if !from_stdin {
        stop::watch_stdin(stop.clone());
    }

    let (ltx, lrx) = oneshot::channel();
    tokio::spawn(async move {
        let _ = ltx.send(if from_stdin { None } else { Some(list_devices().await) });
    });

    // the recorder and the OpenAI session open at once: audio heard before
    // the session is up waits in the channel and goes the moment it opens, so
    // recording starts (and "ready" goes out) without waiting on the network
    let finishing = Arc::new(AtomicBool::new(false));
    let (acts, mut acts_rx) = mpsc::unbounded_channel::<Action>();
    let connecting = o.map(|o| {
        let stop = stop.clone();
        let finishing = finishing.clone();
        tokio::spawn(async move {
            let on_close = stop.clone();
            let hooks = Hooks {
                delta: Some(Arc::new(|item, text| emit(json!({"event": "delta", "item": item, "text": text})))),
                done: Some(Arc::new(|item, text| emit(json!({"event": "done", "item": item, "text": text})))),
                error: Some(Arc::new(|m| error(m))),
                closed: Some(Arc::new(move || {
                    if !finishing.load(Ordering::SeqCst) {
                        error("the connection to OpenAI closed");
                        on_close.stop();
                    }
                })),
            };
            match Realtime::connect(&o, hooks).await {
                Ok(rt) => {
                    while let Some(a) = acts_rx.recv().await {
                        rt.apply(&a);
                    }
                    Some(rt)
                }
                Err(e) => {
                    error(&e);
                    stop.stop();
                    None
                }
            }
        })
    });
    let engine = local.then(|| start_local(dir.clone(), a.model.clone(), a.server.clone(), stop.clone()));

    let conv = (from_stdin && a.rate > 0.0 && a.rate != rate as f64).then(|| Resampler::new(a.rate, rate as f64));
    let mut feed = Feed {
        local: engine.as_ref().map(|e| e.0.clone()),
        cloud: acts,
        seg: Segmenter::default(),
        re: Rechunker::new(CHUNK),
        conv,
        bytes: 0,
        peak: 0.0,
        heard: false,
        hints: vec![],
    };

    if !stop.stopped() {
        let (source, device) = (a.source.clone(), a.device.clone());
        let mut opening = tokio::spawn(async move { open_source(&source, &device, rate).await });
        let mut sr = stop.subscribe();
        tokio::select! {
            r = &mut opening => match r {
                Ok(Ok(src)) => pump(src, &a, &mut feed, &stop, lrx).await,
                Ok(Err(m)) => fail(&m),
                Err(e) => fail(&e.to_string()),
            },
            _ = stop::stopped(&mut sr) => {
                tokio::spawn(async move {
                    match opening.await {
                        Ok(Ok(s)) => s.kill(),
                        Ok(Err(m)) => fail(&m),
                        Err(_) => {}
                    }
                });
            }
        }
    }
    stop.stop();
    finishing.store(true, Ordering::SeqCst);

    if let Some((tx, thread)) = engine {
        let _ = tx.send(In::Finish);
        drop(feed);
        drop(tx);
        let _ = tokio::task::spawn_blocking(move || thread.join()).await;
        end();
    }
    for c in feed.re.rest() {
        let acts = feed.seg.push(&c);
        feed.cloud(acts);
    }
    let last = feed.seg.flush();
    feed.cloud(last);
    drop(feed);
    let Some(Ok(Some(rt))) = (match connecting {
        Some(c) => Some(c.await),
        None => None,
    }) else {
        end()
    };
    if !rt.wait_all(WAIT_MS).await {
        log("gave up waiting for transcripts");
    }
    rt.close().await;
    end()
}
