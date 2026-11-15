//! Infinite context for bots (and any thread that asks for it): UniiChat's memory tree on the hub's log.
//!
//! A thread's conversation is its own rows, in log order: the person's and others' messages (user), its replies (you), its tool
//! calls (tool) and Backplane's notes (note). Message i is the i-th of them, so the numbering is the log's and never changes;
//! it is read from the model, not kept. Over the messages grows a binary tree of one-line summaries (tree.rs). A line the
//! compactor writes is a `RecallNoted` event (one per call, every line it wrote), and the view as a batch merge last left it a
//! `RecallView`: both fold into the bots world (world.rs `recall`), checkpointed with it, so a restart has the tree as it was
//! and the compactor goes on with what is not written.
//!
//! A thread with infinite context runs each new turn in a fresh provider session whose message starts with the view
//! (agents/recall.rs), and its agent has memory_zoom. The compactor is a cheap one-shot model (Settings `recall.provider`,
//! `recall.model`; Haiku by default) run through the agents' one-shot seam, which goes to a login that can take it in the
//! failover's order; a round it cannot run waits and is asked again later, so an outage only delays a line. Several lines go
//! in one call. The thread's whole history is the tree from the start: its old lines are written in the background, oldest
//! first (a recent long message first), and until then the view shows each message cut to a line.
pub mod tree;

use crate::{agents::{mcp::ToolSet, Agents}, api::Fut, hub::Hub, store::Store};
use serde_json::{json, Value};
use std::{cell::RefCell, collections::{HashMap, HashSet}, sync::{Arc, Mutex as StdMutex, Weak}, time::Duration};
use super::{rpc::{now_s, Bots}, world::World};
use tree::{Part, HIGH, NODE};

/// The nodes one compactor call writes at most, and the most bytes of input it reads.
const BATCH: usize = 16;
const BATCH_BYTES: usize = 32_000;
/// How much of a long message the compactor reads to compress it.
const SOURCE: usize = 8_000;
/// Calls that answered without a node's line before it is its source, cut (a refusal is its input's doing).
const TRIES: u32 = 3;
/// How much of a message one zoom gives.
const PAGE: usize = 16_000;
/// The messages at the end whose long leaves are written before the backlog.
const RECENT: usize = 64;
/// Who wrote a message, as the view says it.
const WHO: [&str; 4] = ["user", "you", "tool", "note"];

/// The compactor's brief: it writes the lines, nothing else.
pub const SYSTEM: &str = "You write the memory of an AI agent: one step of a binary tree of one-line summaries of its endless conversation in a thread. \
Each <task> below is either one message to compress into one line (kind=\"compress\"), or two adjacent lines of the conversation, oldest first, to merge into one line (kind=\"merge\"). \
Each line you write stands in for its input from now on, so keep what may matter later: names, numbers, paths, decisions, requests, results, promises and open questions; drop pleasantries and repetition. \
Messages are tagged with who wrote them: user (the person, or a report or another bot's message), you (the agent), tool (the agent's tool calls), note (the app's notes). \
Start a compressed message's line with its tag (\"user: \", \"you: \"); in a merged line name who said what. \
Each line is at most 512 bytes (about 70 words), plain text, no markdown, no line breaks. Do not answer the messages, do not do any work and do not use tools. \
Reply with exactly one line per task, each the task's id, a bar and the line: <id>|<line>. Nothing else.";

/// A thread's tree as the log holds it: each written line's place (`line`·64 + its place in its RecallNoted, 0 none) by level and
/// index, and the view a batch merge last kept.
#[derive(Default, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Kept { pub nodes: Vec<Vec<u64>>, pub parts: Vec<Part> }
impl Kept {
    /// RecallNoted at log line `line` (1-based): its lines, [[l, i, text], ...]
    pub fn noted(&mut self, line: usize, e: &Value) {
        for (k, n) in e["nodes"].as_array().into_iter().flatten().enumerate().take(64) {
            let (Some(l), Some(i)) = (n[0].as_u64(), n[1].as_u64()) else { continue };
            if l > 40 || i > (1 << 40) { continue }
            let (l, i) = (l as usize, i as usize);
            if self.nodes.len() <= l { self.nodes.resize(l + 1, vec![]) }
            if self.nodes[l].len() <= i { self.nodes[l].resize(i + 1, 0) }
            self.nodes[l][i] = ((line as u64) << 6) | k as u64;
        }
    }
    /// RecallView: the kept view, [[l, i], ...]
    pub fn viewed(&mut self, e: &Value) {
        self.parts = e["parts"].as_array().into_iter().flatten().filter_map(|p| Some((p[0].as_u64()? as u32, p[1].as_u64()?))).collect();
    }
    /// Where a written line is: (log line, 1-based; its place in that event)
    pub fn at(&self, (l, i): Part) -> Option<(usize, usize)> {
        let v = *self.nodes.get(l as usize)?.get(i as usize)?;
        if v == 0 { None } else { Some(((v >> 6) as usize, (v & 63) as usize)) }
    }
    pub fn written(&self) -> usize { self.nodes.iter().map(|l| l.iter().filter(|v| **v != 0).count()).sum() }
}

/// The threads that run with infinite context: every live bot's own thread here unless `recall.bots` or its bot's `recall.bot.<id>`
/// is "off", and any thread whose `recall.thread.<id>` is "on" (a long-lived thread the person chose).
pub fn on_threads(w: &World) -> HashSet<String> {
    let all = w.setting("recall.bots", "on") != "off";
    let mut s: HashSet<String> = w.bots.values().filter(|b| all && !b.deleted && !b.thread.is_empty() && !b.id.contains('~') && w.setting(&format!("recall.bot.{}", b.id), "on") != "off").map(|b| b.thread.clone()).collect();
    s.extend(w.settings.iter().filter(|(k, v)| k.starts_with("recall.thread.") && v.as_str() == "on").map(|(k, _)| k["recall.thread.".len()..].to_string()));
    s
}
/// A bot's switch as its Settings show it.
pub fn bot_on(w: &World, bot: &str) -> bool { w.setting("recall.bots", "on") != "off" && w.setting(&format!("recall.bot.{bot}"), "on") != "off" }

/// A message of the conversation: its log line, its line's bytes (fits: at most NODE), who wrote it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leaf { pub seq: u32, pub bytes: u32, pub who: u8 }
/// A row of the log as a message of the conversation: (who, text, when); None for a row that is not one (an empty message, an activity that is no tool call).
pub fn leaf_of(line: &str) -> Option<(u8, String, u64)> {
    let v: Value = serde_json::from_str(&crate::store::clean(line)).ok()?;
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string(); let at = v["at"].as_u64().unwrap_or(0);
    let (who, text) = match v["$"].as_str()? {
        "MessagePosted" => (match v["role"].as_str()? { "user" => 0, "assistant" => 1, "system" => 3, _ => return None }, crate::model::voice_mark::split(&s("text")).0),
        "ActivityLogged" if v["tone"] == "tool" => (2, s("summary")),
        _ => return None,
    };
    if text.trim().is_empty() { None } else { Some((who, text, at)) }
}

/// What writes the lines: a one-shot model call (the system prompt, the tasks) answering the lines, or why it could not.
pub trait Writer: Send + Sync {
    fn write<'a>(&'a self, thread: &'a str, system: &'a str, input: &'a str) -> Fut<'a, Result<String, String>>;
}
/// The real compactor: the agents' one-shot on `recall.provider` / `recall.model` (Haiku on Claude by default). Claude goes to a login
/// that can take it in the failover's order (the default login unless it is held, then the named accounts, a limit skipping one for an hour).
pub struct AgentsWriter(pub Weak<Agents>);
impl Writer for AgentsWriter {
    fn write<'a>(&'a self, thread: &'a str, system: &'a str, input: &'a str) -> Fut<'a, Result<String, String>> {
        Box::pin(async move {
            let ag = self.0.upgrade().ok_or("the hub is stopping")?;
            let p = ag.hub.setting("recall.provider").await.unwrap_or_default(); let m = ag.hub.setting("recall.model").await.unwrap_or_default();
            let (provider, model) = crate::agents::textgen::pick(&p, if m.trim().is_empty() { "haiku" } else { &m });
            let r = if provider == "claude" { ag.shot_healthy(&model, system, input, &format!("recall: {thread}")).await } else { ag.shot(provider, &model, system, input, &Default::default()).await };
            r.ok_or_else(|| format!("{provider} {model} gave no answer"))
        })
    }
}

/// A thread's messages as last read: how many of its own rows were read, and the messages among them.
#[derive(Default)]
struct Msgs { scanned: usize, leaves: Vec<Leaf> }

/// The engine: reads conversations, gives views and zooms, and runs the compactor.
pub struct Recall {
    hub: Arc<Hub>, bots: Weak<Bots>, writer: Arc<dyn Writer>,
    msgs: tokio::sync::Mutex<HashMap<String, Msgs>>,
    /// nodes a call is writing, the calls a node went unanswered, a thread held after a call failed (until, wait)
    busy: StdMutex<HashSet<(String, Part)>>, tries: StdMutex<HashMap<(String, Part), u32>>, hold: StdMutex<HashMap<String, (u64, u64)>>,
    kick: tokio::sync::Notify,
}

/// Reads a tree's lines and messages from the log for one view, zoom or batch (no lock held, nothing awaited).
struct Reader<'a> { store: &'a Store, kept: &'a Kept, leaves: &'a [Leaf], events: RefCell<HashMap<usize, Value>>, shown: RefCell<HashMap<Part, String>> }
impl<'a> Reader<'a> {
    fn new(store: &'a Store, kept: &'a Kept, leaves: &'a [Leaf]) -> Self { Reader { store, kept, leaves, events: Default::default(), shown: Default::default() } }
    fn fits(&self, i: u64) -> bool { self.leaves.get(i as usize).map_or(false, |l| l.bytes as usize <= NODE) }
    /// a node has its line: written, or a leaf that fits
    fn built(&self, p: Part) -> bool { self.kept.at(p).is_some() || (p.0 == 0 && self.fits(p.1)) }
    /// message i: (who, text, when)
    fn message(&self, i: u64) -> Option<(u8, String, u64)> { let l = self.leaves.get(i as usize)?; leaf_of(&self.store.get(l.seq as usize - 1).ok()?) }
    fn said(&self, i: u64) -> Option<String> { self.message(i).map(|(w, t, _)| tree::said(WHO[w as usize], &t)) }
    /// a node's line: written, or a leaf's own when it fits
    fn line(&self, p: Part) -> Option<String> {
        if let Some((line, k)) = self.kept.at(p) {
            let mut ev = self.events.borrow_mut();
            if !ev.contains_key(&line) { let v = self.store.get(line - 1).ok().and_then(|r| serde_json::from_str::<Value>(&r).ok()).unwrap_or(Value::Null); ev.insert(line, v); }
            if let Some(t) = ev[&line]["nodes"][k][2].as_str() { return Some(t.to_string()) }
        }
        if p.0 == 0 && self.fits(p.1) { return self.said(p.1) }
        None
    }
    /// what the view shows for a node: its line, a long leaf cut, or WAIT
    fn shown(&self, p: Part) -> String {
        if let Some(s) = self.shown.borrow().get(&p) { return s.clone() }
        let s = self.line(p).or_else(|| if p.0 == 0 { self.said(p.1).map(|s| tree::clip(&s)) } else { None }).unwrap_or_else(|| tree::WAIT.to_string());
        self.shown.borrow_mut().insert(p, s.clone()); s
    }
    fn size(&self, p: Part) -> usize { tree::line(p, &self.shown(p)).len() + 1 }
    /// what the compactor reads for a node: a message (most of it), or the two lines it merges
    fn source(&self, p: Part) -> Option<String> {
        if p.0 == 0 { let (w, t, _) = self.message(p.1)?; return Some(tree::flat(&format!("{}: {}", WHO[w as usize], tree::cut(t.trim(), SOURCE)))) }
        let hs = tree::halves(p); let (a, b) = (self.line(hs[0])?, self.line(hs[1])?);
        Some(format!("{}\n{}", tree::line(hs[0], &a), tree::line(hs[1], &b)))
    }
}

/// The compactor's tasks, one per node.
pub fn tasks(batch: &[(Part, String)]) -> String {
    batch.iter().map(|((l, i), src)| { let kind = if *l == 0 { "compress" } else { "merge" }; format!("<task id=\"{l}/{i}\" kind=\"{kind}\" covers=\"{}\">\n{src}\n</task>\n", tree::name((*l, *i))) }).collect()
}
/// The compactor's answer as lines by node: `l/i|line`, nothing else counts.
pub fn answers(text: &str) -> HashMap<Part, String> {
    let mut out = HashMap::new();
    for raw in text.lines() {
        let l = raw.trim().trim_start_matches(['-', '*', '`', ' ']);
        let Some((id, line)) = l.split_once('|') else { continue };
        let Some((a, b)) = id.trim().trim_start_matches("id=").trim_matches('"').split_once('/') else { continue };
        let (Ok(a), Ok(b)) = (a.trim().parse::<u32>(), b.trim().parse::<u64>()) else { continue };
        let line = tree::flat(line.trim().trim_matches('`')).trim().to_string();
        if !line.is_empty() { out.insert((a, b), clean(&line)); }
    }
    out
}
/// A line within NODE bytes: cut at a word, "..." after it.
pub fn clean(line: &str) -> String {
    if line.len() <= NODE { return line.to_string() }
    let c = tree::cut(line, NODE - 3); let c = match c.rfind(' ') { Some(k) if k > NODE / 2 => &c[..k], _ => c };
    format!("{}...", c.trim_end())
}

impl Recall {
    pub fn new(bots: &Arc<Bots>, writer: Arc<dyn Writer>) -> Arc<Recall> {
        let r = Arc::new(Recall { hub: bots.hub.clone(), bots: Arc::downgrade(bots), writer, msgs: Default::default(), busy: Default::default(), tries: Default::default(), hold: Default::default(), kick: Default::default() });
        let _ = bots.recall.set(r.clone()); r
    }
    /// Whether the thread runs with infinite context, as last folded.
    pub fn on(&self, thread: &str) -> bool { self.bots.upgrade().map_or(false, |b| b.recall_on.read().unwrap().contains(thread)) }

    /// The thread's messages, the new ones read from the log (off the runtime's workers: a long history is read once).
    pub async fn leaves(&self, th: &str) -> Vec<Leaf> {
        let mut g = self.msgs.lock().await;
        let m = g.entry(th.to_string()).or_default();
        let ents = { let model = self.hub.model().await; if model.count(th) <= m.scanned { return m.leaves.clone() } model.own_ents(th) };
        let (store, from, upto) = (self.hub.store.clone(), m.scanned, ents.len());
        let new = tokio::task::spawn_blocking(move || ents[from..].iter().filter_map(|e| { let (w, t, _) = leaf_of(&store.get(e.seq as usize - 1).ok()?)?; Some(Leaf { seq: e.seq, bytes: tree::said(WHO[w as usize], &t).len() as u32, who: w }) }).collect::<Vec<_>>()).await.unwrap_or_default();
        m.scanned = upto; // a row filed meanwhile is read next time
        m.leaves.extend(new); m.leaves.clone()
    }
    async fn kept(&self, th: &str) -> Kept { match self.bots.upgrade() { Some(b) => b.world().await.recall.get(th).cloned().unwrap_or_default(), None => Kept::default() } }

    /// The block a fresh turn starts with: the doc and the view of the conversation up to the turn's own messages (the person's and
    /// the notes at its end, which the turn's message carries whole). A batch merge it makes is kept (`RecallView`). "" for no conversation.
    pub async fn view(&self, th: &str) -> String {
        let leaves = self.leaves(th).await;
        let mut to = leaves.len(); while to > 0 && matches!(leaves[to - 1].who, 0 | 3) { to -= 1 }
        if to == 0 { return String::new() }
        let kept = self.kept(th).await;
        let (parts, changed, lines) = {
            let r = Reader::new(&self.hub.store, &kept, &leaves);
            let (parts, changed) = tree::fit(&kept.parts, to as u64, &|p| r.size(p), &|p| r.built(p), HIGH);
            let lines: Vec<String> = parts.iter().map(|p| tree::line(*p, &r.shown(*p))).collect();
            (parts, changed, lines)
        };
        if changed { if let Err(e) = self.hub.commit_many(&[json!({"$":"RecallView","thread":th,"parts":parts,"at":now_s()})]).await { eprintln!("recall: {th}: the view was not kept: {e:#}") } }
        tree::block(&lines, HIGH)
    }

    /// memory_zoom: message `id` whole when n = 1 (a page from `at`), else the two lines line id+n was made from; a name that is
    /// no line lists the tree's roots.
    pub async fn zoom(&self, th: &str, id: u64, n: u64, at: usize) -> String {
        let leaves = self.leaves(th).await; let count = leaves.len() as u64;
        let kept = self.kept(th).await;
        let zone = match self.bots.upgrade() { Some(b) => { let w = b.world().await; let z = super::clock::Zones { settings: &w.settings }; z.rule(&z.person()) } None => None }.unwrap_or_else(super::clock::TzRule::utc0);
        let r = Reader::new(&self.hub.store, &kept, &leaves);
        if n == 1 && id < count {
            let Some((w, text, t)) = r.message(id) else { return format!("Message {id} could not be read.") };
            let page: String = text.chars().skip(at).take(PAGE).collect(); let more = text.chars().count() > at + PAGE;
            return format!("{id}+1|{} ({}): {page}{}", WHO[w as usize], zone.iso(t), if more { format!("\n(more: memory_zoom with at {})", at + PAGE) } else { String::new() });
        }
        match tree::part(count, id, n) {
            Some(p) => tree::halves(p).into_iter().map(|h| tree::line(h, &r.shown(h))).collect::<Vec<_>>().join("\n"),
            None => format!("No line {id}+{n}: n is a power of 2, id a multiple of n, and the conversation has {count} messages. The tree's roots: {}.", tree::roots(count).into_iter().map(tree::name).collect::<Vec<_>>().join(", ")),
        }
    }

    /// What the bot's Settings show: (messages, lines the compactor wrote, nodes it has still to write).
    pub async fn stats(&self, th: &str) -> (usize, usize, usize) {
        let leaves = self.leaves(th).await; let kept = self.kept(th).await;
        let r = Reader::new(&self.hub.store, &kept, &leaves);
        let left = tree::todo(leaves.len() as u64, &|p| r.built(p)).len();
        (leaves.len(), kept.written(), left)
    }

    /// The compactor's loop: a round every 20 s, or when kicked. Nothing runs on a copy of a log or in safe mode.
    pub fn start(self: &Arc<Self>) {
        if crate::copy::on() || crate::guard::safe() { return }
        let me = Arc::downgrade(self);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(30)).await; // the hub's start first
            loop {
                let Some(r) = me.upgrade() else { return };
                r.round().await;
                tokio::select! { _ = r.kick.notified() => {}, _ = tokio::time::sleep(Duration::from_secs(20)) => {} }
            }
        });
    }
    pub fn kick(&self) { self.kick.notify_one() }

    /// One round: batches of every thread with infinite context, `recall.jobs` calls at once (2 by default), until nothing is left
    /// to write or every thread with work is held. Answers how many lines were written.
    pub async fn round(self: &Arc<Self>) -> usize {
        let jobs = self.hub.setting("recall.jobs").await.and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(2).clamp(1, 8);
        let mut ths: Vec<String> = match self.bots.upgrade() { Some(b) => { let _ = b.world().await; b.recall_on.read().unwrap().iter().cloned().collect() } None => return 0 };
        ths.sort();
        let (mut set, mut written) = (tokio::task::JoinSet::new(), 0usize);
        loop {
            loop {
                let mut more = false;
                for th in &ths {
                    if set.len() >= jobs { break }
                    if self.hold.lock().unwrap().get(th).map_or(false, |(until, _)| *until > now_s()) { continue }
                    let Some(batch) = self.batch(th).await else { continue };
                    { let mut b = self.busy.lock().unwrap(); for (p, _) in &batch { b.insert((th.clone(), *p)); } }
                    let (me, th2) = (self.clone(), th.clone()); set.spawn(async move { me.write(&th2, batch).await }); more = true;
                }
                if !more || set.len() >= jobs { break }
            }
            match set.join_next().await { Some(Ok(n)) => written += n, Some(Err(_)) => {} None => break }
        }
        written
    }

    /// The next nodes of a thread to write, at most BATCH and BATCH_BYTES of input (one at least): its recent long messages first,
    /// then the rest in the order the messages came. None when it has nothing to write that no call is writing.
    async fn batch(&self, th: &str) -> Option<Vec<(Part, String)>> {
        let leaves = self.leaves(th).await; let kept = self.kept(th).await;
        let busy: HashSet<Part> = self.busy.lock().unwrap().iter().filter(|(t, _)| t == th).map(|(_, p)| *p).collect();
        let r = Reader::new(&self.hub.store, &kept, &leaves);
        let todo: Vec<Part> = tree::todo(leaves.len() as u64, &|p| r.built(p)).into_iter().filter(|p| !busy.contains(p)).collect();
        let recent = |p: &Part| p.0 == 0 && p.1 as usize + RECENT >= leaves.len();
        let (mut out, mut bytes) = (vec![], 0usize);
        for p in todo.iter().filter(|p| recent(p)).chain(todo.iter().filter(|p| !recent(p))) {
            let Some(src) = r.source(*p) else { continue };
            if !out.is_empty() && (out.len() >= BATCH || bytes + src.len() > BATCH_BYTES) { break }
            bytes += src.len(); out.push((*p, src));
        }
        if out.is_empty() { None } else { Some(out) }
    }

    /// One compactor call for a batch: its lines kept in one RecallNoted. A node it did not answer waits for the next round; after
    /// TRIES unanswered calls its line is its source, cut. A call that fails holds the thread (a minute, doubling to half an hour).
    async fn write(&self, th: &str, batch: Vec<(Part, String)>) -> usize {
        let res = self.writer.write(th, SYSTEM, &tasks(&batch)).await;
        let mut nodes = vec![];
        match res {
            Err(e) => {
                let mut h = self.hold.lock().unwrap(); let wait = h.get(th).map_or(60, |(_, w)| (w * 2).min(1800));
                h.insert(th.to_string(), (now_s() + wait, wait)); eprintln!("recall: {th}: the compactor gave nothing ({e}); asked again in {wait} s");
            }
            Ok(ans) => {
                self.hold.lock().unwrap().remove(th);
                let got = answers(&ans);
                for (p, src) in &batch {
                    if let Some(l) = got.get(p) { nodes.push(json!([p.0, p.1, l])); self.tries.lock().unwrap().remove(&(th.to_string(), *p)); continue }
                    let mut t = self.tries.lock().unwrap(); let n = t.entry((th.to_string(), *p)).or_insert(0); *n += 1;
                    if *n >= TRIES { nodes.push(json!([p.0, p.1, clean(&tree::flat(src))])); t.remove(&(th.to_string(), *p)); }
                }
            }
        }
        let n = nodes.len();
        if n > 0 { if let Err(e) = self.hub.commit_many(&[json!({"$":"RecallNoted","thread":th,"nodes":nodes,"at":now_s()})]).await { eprintln!("recall: {th}: lines not kept: {e:#}") } }
        if let Some(b) = self.bots.upgrade() { let _ = b.world().await; } // the lines folded before the nodes are let go, so the next batch sees them written
        { let mut b = self.busy.lock().unwrap(); for (p, _) in &batch { b.remove(&(th.to_string(), *p)); } }
        n
    }
}

/// The agents' seam (agents/recall.rs).
pub struct Seam(pub Arc<Recall>);
impl crate::agents::recall::Recall for Seam {
    fn on(&self, thread: &str) -> bool { self.0.on(thread) }
    fn view<'a>(&'a self, thread: &'a str) -> Fut<'a, String> { Box::pin(async move { let v = self.0.view(thread).await; self.0.kick(); v }) }
}

/// memory_zoom on the agents' MCP server: listed in a thread with infinite context only.
pub struct Tools(pub Arc<Recall>);
pub fn zoom_def() -> Value {
    json!({"name":"memory_zoom","description":"Open a line of your memory view (the <chat> your turn started with). memory_zoom(id, n) gives the two lines line id+n was made from; memory_zoom(id, 1) gives message id whole, with who wrote it and when (a long one a page at a time: pass `at` as the answer says). A name that is no line, such as memory_zoom(0, 0), lists the tree's roots.",
        "inputSchema":{"type":"object","properties":{"id":{"type":"integer","description":"the first message the line covers (the number before +)"},"n":{"type":"integer","description":"how many messages it covers (the number after +)"},"at":{"type":"integer","description":"where a long message's page starts, in characters"}},"required":["id","n"]}})
}
impl ToolSet for Tools {
    fn list(&self, thread: &str) -> Vec<Value> { if self.0.on(thread) { vec![zoom_def()] } else { vec![] } }
    fn owns(&self, name: &str) -> bool { name == "memory_zoom" }
    fn call<'a>(&'a self, thread: &'a str, _name: &'a str, a: &'a Value) -> Fut<'a, (bool, Value)> {
        Box::pin(async move {
            if !self.0.on(thread) { return (false, json!({"error":"this thread has no memory tree (its infinite context is off)"})) }
            let num = |k: &str| match &a[k] { Value::Number(n) => n.as_u64(), Value::String(s) => s.trim().parse().ok(), _ => None };
            let (Some(id), Some(n)) = (num("id"), num("n")) else { return (false, json!({"error":"memory_zoom needs id and n, the two numbers of a line's name id+n"})) };
            (true, Value::String(self.0.zoom(thread, id, n, num("at").unwrap_or(0) as usize).await))
        })
    }
}

#[cfg(test)]
mod tests;
