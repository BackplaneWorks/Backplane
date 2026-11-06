//! Mail (src/core/mail.bend, mailbody.bend, hub.bend `Mail.*`, mailowner.bend): the mailroom's pure part. A mail is a body (the text, kept in
//! `<home>/mail/<conversation>.jsonl`) and a header event (`MailPosted`, with `ConvSet` for a new conversation); the box folded from those events is
//! `mailbox::Box`. Slice 1 files mail (`file.rs`), slice 2 is the requests, slice 3 routing, triage and private threads (`route.rs`, `escalate.rs`, `run.rs`, `funnel.rs`, `tools.rs`); Google (slice 4) will use these types.
//! No message text or address is ever logged by this module.
pub mod file;
pub mod owner;
pub mod requests;
pub mod mailbox;
pub mod charter;
pub mod escalate;
pub mod funnel;
pub mod prompt;
pub mod prompt_text;
pub mod route;
pub mod run;
pub mod tools;
pub mod store;


use serde_json::{json, Value};

/// ML.Origin: who a mail is from, what kind of source, and its reference
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Origin { pub from: String, pub kind: String, pub r#ref: String }
impl Origin { pub fn new(from: &str, kind: &str, r: &str) -> Origin { Origin { from: from.into(), kind: kind.into(), r#ref: r.into() } } }
/// ML.Conv
#[derive(Clone, Debug, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Conv { pub id: String, pub subject: String, pub kind: String, pub parts: Vec<String>, pub project: String, pub tags: Vec<String>, pub made: u64, pub last: u64, pub n: u64 }
/// ML.Mail: a mail's header
#[derive(Clone, Debug, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Hdr { pub id: String, pub conv: String, pub from: String, pub to: String, pub kind: String, pub r#ref: String, pub want: String, pub head: String, pub size: u64, pub at: u64 }
/// MB.Body
#[derive(Clone, Debug, PartialEq)]
pub struct Body { pub conv: Conv, pub mail: Hdr, pub text: String }

// ---- addresses (Addr.*, Ref) ----
/// Addr.thread, Addr.bot
pub fn addr_thread(s: &str) -> &str { s.strip_prefix("thread:").unwrap_or(s) }
pub fn addr_bot(s: &str) -> &str { s.strip_prefix("bot:").unwrap_or(s) }
/// Addr.of
pub fn addr_of(s: &str) -> String {
    if s.is_empty() || s == "person" || s == "you" { "person".into() } else if s == "backplane" { "backplane".into() } else if s.contains(':') { s.into() } else { format!("thread:{s}") }
}
/// Addr.dm
pub fn addr_dm(s: &str) -> String { if s == "you" || s == "person" { "person".into() } else if s.contains(':') { s.into() } else { format!("bot:{s}") } }
/// Mail.pair: the two addresses in order
pub fn pair(a: &str, b: &str) -> String { if a > b { format!("pair:{b}|{a}") } else { format!("pair:{a}|{b}") } }
fn until(s: &str, end: char) -> &str { s.split(end).next().unwrap_or("") }
fn take(s: &str, n: usize) -> String { s.chars().take(n).collect() }
fn drop_(s: &str, n: usize) -> String { s.chars().skip(n).collect() }

/// Mail.kind_code
pub fn kind_code(k: &str) -> u32 {
    ["person", "thread", "bot", "room", "launch", "wake", "lost", "task", "routine", "hook", "deploy", "lock", "carry", "peer", "reminder", "system"].iter().position(|x| *x == k).map_or(99, |i| i as u32)
}
/// Mail.dm: the conversation of a direct room "dm:a:b"
fn dm(parts: &[&str], fallback: String) -> String { if let [a, b] = parts { pair(&addr_dm(a), &addr_dm(b)) } else { fallback } }
/// Mail.route: the conversation id a mail from this origin to `to` belongs to
pub fn route(o: &Origin, to: &str) -> String {
    let (from, r) = (o.from.as_str(), o.r#ref.as_str());
    match kind_code(&o.kind) {
        0 => String::new(),
        1 | 2 => pair(from, to),
        3 => if let Some(d) = r.strip_prefix("dm:") { dm(&d.split(':').collect::<Vec<_>>(), format!("room:{r}")) } else { format!("room:{r}") },
        4 | 7 => format!("launch:{}", addr_thread(from)),
        5 => if r.is_empty() { format!("launch:{}", addr_thread(from)) } else { format!("wake:{r}") },
        8 => format!("routine:{r}"),
        9 => format!("hook:{r}"),
        13 => format!("peer:{}:{r}", from.strip_prefix("peer:").unwrap_or(from)),
        _ => format!("system:{}", addr_bot(to)),
    }
}
/// Conv.source_kind
fn source_kind(k: u32) -> &'static str { match k { 0 | 1 | 2 => "pair", 3 => "room", 4 | 7 => "launch", 5 => "wake", 8 => "routine", 9 => "hook", 13 => "peer", _ => "system" } }
/// Conv.mail_kind: the conversation's kind from its id, else from the mail's kind
pub fn conv_mail_kind(id: &str, kind: &str) -> String {
    for p in ["pair", "room", "launch", "wake", "system", "routine", "hook", "peer"] { if id.starts_with(&format!("{p}:")) { return p.into() } }
    source_kind(kind_code(kind)).into()
}
/// ML.Flags
pub struct Flags { pub outcome: String, pub own: bool, pub direct: bool, pub mentioned: bool }
/// Mail.flags
pub fn flags(o: &Origin, text: &str) -> Flags {
    Flags { outcome: if take(text, 240).contains("completed") { "completed".into() } else { String::new() }, own: ["task", "wake", "deploy", "lock"].contains(&o.kind.as_str()),
        direct: ["thread", "bot", "person", "peer"].contains(&o.kind.as_str()), mentioned: false }
}
/// Mail.want_kind
fn want_kind(k: u32, f: &Flags) -> &'static str {
    let act = |b: bool| if b { "act" } else { "fyi" };
    match k { 4 => if f.outcome == "completed" { "fyi" } else { "act" }, 7 | 10 | 11 => act(f.own), 5 | 6 | 8 | 9 => "act", 3 => act(f.mentioned), 0 => act(f.direct), _ => act(f.own || f.direct || f.mentioned) }
}
/// Mail.want: a given "fyi" or "act" wins, else the kind decides
pub fn want(kind: &str, given: &str, f: &Flags) -> String { match given { "fyi" => "fyi".into(), "act" => "act".into(), _ => want_kind(kind_code(kind), f).into() } }
/// Mail.head: the first line, at most 160 characters, tabs and returns as spaces
pub fn head(text: &str) -> String { let mut o = String::new(); for c in text.chars().take(160) { match c { '\n' => break, '\t' | '\r' => o.push(' '), c => o.push(c) } } o }
/// Mail.bytes: UTF-8 bytes
pub fn bytes(text: &str) -> u64 { text.len() as u64 }

// ---- classify (Mail.classify) ----
fn classify_base(from: &str, system: bool) -> Origin {
    let kind_of = |from: &str, own: &str| if from.is_empty() { own.to_string() } else if from.starts_with("bot:") { "bot".into() } else { "thread".into() };
    if system { Origin { from: if from.is_empty() { "backplane".into() } else { addr_of(from) }, kind: kind_of(from, "system"), r#ref: String::new() } }
    else { Origin { from: if from.is_empty() { "person".into() } else { addr_of(from) }, kind: kind_of(from, "person"), r#ref: String::new() } }
}
fn envelope_from(from: &str, label: &str) -> String {
    let name = until(&drop_(label, 14), ' ').to_string();
    if !from.is_empty() { addr_of(from) } else if name == "you" || name.starts_with("person@") { "person".into() } else if name.contains('@') { format!("peer:{}", name.rsplit('@').next().unwrap_or("")) } else { format!("bot:{name}") }
}
fn direct_unlinked(name: &str) -> Origin {
    Origin { from: if name.starts_with("person@") { "person".into() } else if name.contains('@') { format!("peer:{name}") } else { format!("bot:{name}") },
        kind: if name.starts_with("person@") { "person".into() } else if name.contains('@') { "peer".into() } else { "bot".into() }, r#ref: String::new() }
}
/// Mail.classify: what a delivered text and its message id say of where it came from
pub fn classify(from: &str, msg: &str, system: bool, text: &str) -> Origin {
    let h = take(text, 160); let label = until(&h, ']').to_string(); let room = label.rsplit('#').next().unwrap_or("").to_string();
    let from_line = h.starts_with("[message from ");
    if from_line && !label.contains(" in #") { return if !from.is_empty() { classify_base(from, false) } else { direct_unlinked(&drop_(&label, 14)) } }
    if h.starts_with("Backplane restarted while you were working") { return Origin::new("backplane", "carry", "") }
    if from_line && label.contains(" in #") { return Origin::new(&envelope_from(from, &label), "room", &room) }
    if h.starts_with("[routine: ") { let n = drop_(&label, 10); return Origin::new(&format!("routine:{n}"), "routine", &n) }
    if h.starts_with("[webhook: ") { let n = drop_(&label, 10); return Origin::new(&format!("hook:{n}"), "hook", &n) }
    if msg.starts_with("t:backplane:l") { return Origin::new("backplane", "lost", "") }
    let wake_name = until(&drop_(&h, 5), ':').to_string();
    if msg.contains(":lr-") && h.starts_with("Wake ") && h.contains(':') && !wake_name.is_empty() && !wake_name.contains(' ') { return Origin::new(&addr_of(from), "wake", &wake_name) }
    if from == "deploy" { return Origin::new("backplane", "deploy", "") }
    if from.starts_with("lock:") || msg.starts_with("t:lock:") { return Origin::new("backplane", "lock", &if from.starts_with("lock:") { drop_(from, 5) } else { until(&drop_(msg, 7), ':').to_string() }) }
    if msg.contains(":td-") || h.starts_with("[Backplane] Delegated task") { return Origin::new(&addr_of(from), "task", "") }
    if msg.contains(":lr-") || h.starts_with("[Backplane] Launched thread") { return Origin::new(&addr_of(from), "launch", "") }
    classify_base(from, system)
}

// ---- JSON (mailbody.bend Body.encode / Json.emit) ----
fn esc(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() { match c { '\n' => o += "\\n", '\r' => o += "\\r", '\t' => o += "\\t", '"' => o += "\\\"", '\\' => o += "\\\\", c if (c as u32) < 32 => o += &format!("\\u00{:02x}", c as u32), c => o.push(c) } }
    o.push('"'); o
}
fn arr(xs: &[String]) -> String { format!("[{}]", xs.iter().map(|x| esc(x)).collect::<Vec<_>>().join(",")) }
/// Body.line: the compact JSON of a body and a newline (the exact text Bend writes; string escapes as Json.esc)
pub fn body_line(b: &Body) -> String {
    let (c, m) = (&b.conv, &b.mail);
    format!("{{\"v\":1,\"conv\":{{\"id\":{},\"subject\":{},\"kind\":{},\"parts\":{},\"project\":{},\"tags\":{},\"made\":{},\"last\":{},\"n\":{}}},\"mail\":{{\"id\":{},\"conv\":{},\"from\":{},\"to\":{},\"kind\":{},\"ref\":{},\"want\":{},\"head\":{},\"size\":{},\"at\":{}}},\"text\":{}}}\n",
        esc(&c.id), esc(&c.subject), esc(&c.kind), arr(&c.parts), esc(&c.project), arr(&c.tags), c.made, c.last, c.n,
        esc(&m.id), esc(&m.conv), esc(&m.from), esc(&m.to), esc(&m.kind), esc(&m.r#ref), esc(&m.want), esc(&m.head), m.size, m.at, esc(&b.text))
}
fn jstr(v: &Value, k: &str) -> Option<String> { v.get(k)?.as_str().map(str::to_string) }
fn jnat(v: &Value, k: &str) -> Option<u64> { v.get(k)?.as_u64() }
fn jlist(v: &Value, k: &str) -> Option<Vec<String>> { v.get(k)?.as_array()?.iter().map(|x| x.as_str().map(str::to_string)).collect() }
/// Body.decode (Body.parse): None for anything but a version 1 body with matching conversation ids
pub fn body_parse(line: &str) -> Option<Body> {
    let v: Value = serde_json::from_str(line.trim_end_matches('\n')).ok()?;
    if jnat(&v, "v")? != 1 { return None }
    let (cj, mj) = (v.get("conv")?, v.get("mail")?);
    let conv = Conv { id: jstr(cj, "id")?, subject: jstr(cj, "subject")?, kind: jstr(cj, "kind")?, parts: jlist(cj, "parts")?, project: jstr(cj, "project")?, tags: jlist(cj, "tags")?, made: jnat(cj, "made")?, last: jnat(cj, "last")?, n: jnat(cj, "n")? };
    let mail = Hdr { id: jstr(mj, "id")?, conv: jstr(mj, "conv")?, from: jstr(mj, "from")?, to: jstr(mj, "to")?, kind: jstr(mj, "kind")?, r#ref: jstr(mj, "ref")?, want: jstr(mj, "want")?, head: jstr(mj, "head")?, size: jnat(mj, "size")?, at: jnat(mj, "at")? };
    if conv.id.is_empty() || mail.id.is_empty() || !(mail.want == "fyi" || mail.want == "act") || conv.id != mail.conv { return None }
    Some(Body { conv, mail, text: jstr(&v, "text")? })
}
/// Body.metadata, Body.header: the events that tell the log a mail exists
pub fn conv_event(c: &Conv) -> Value { json!({"$":"ConvSet","id":c.id,"subject":c.subject,"kind":c.kind,"parts":c.parts.join("\n"),"project":c.project,"tags":c.tags.join("\n"),"at":c.made}) }
pub fn header_event(m: &Hdr) -> Value { json!({"$":"MailPosted","id":m.id,"conv":m.conv,"from":m.from,"to":m.to,"kind":m.kind,"ref":m.r#ref,"want":m.want,"head":m.head,"size":m.size,"at":m.at}) }
/// Ref.pct: a conversation id as a file name
pub fn pct(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() { if b.is_ascii_alphanumeric() || b == b'-' || b == b'.' || b == b'_' || b == b'~' { o.push(b as char) } else { o += &format!("%{b:02X}") } }
    o
}
