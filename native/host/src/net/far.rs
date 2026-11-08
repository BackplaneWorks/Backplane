//! core/far, the pure part: threads that live on a linked machine. A thing there is named here by the link
//! and its id there ("<link>~<id>"), so nothing can be mistaken for a local one. The owning hub pushes each shared
//! change as its own log numbers it (`Item`), a mirror takes a batch only when it follows what it has and only the
//! items it lacks (`take`), so a batch that comes twice, late or out of order is harmless. Laws `far_*`.
use serde_json::{json, Map, Value};

// ----- ids -----
/// A thing on the machine a link reaches, named here.
pub fn id(link: &str, id: &str) -> String { format!("{link}~{id}") }
/// The same, keeping "" (no project, no parent) as it is.
pub fn opt(link: &str, i: &str) -> String { if i.is_empty() { String::new() } else { id(link, i) } }
/// Whether an id names a thing elsewhere (local ids never hold a "~").
pub fn is(i: &str) -> bool { i.contains('~') }
/// The link an id elsewhere lives behind.
pub fn link(i: &str) -> String { let p: Vec<&str> = i.split('~').collect(); if p.len() >= 2 { p[0].to_string() } else { String::new() } }
/// Its id on its own machine (an id here is itself).
pub fn local(i: &str) -> String { match i.split_once('~') { Some((_, rest)) => rest.to_string(), None => i.to_string() } }

/// M.Link.from: the thread a message came from ("" for the user's own): the second ':' field when the first is
/// "t" and a third follows.
pub fn link_from(msg: &str) -> String {
    let Some(rest) = msg.strip_prefix("t:") else { return String::new() };
    rest.split_once(':').map(|(f, _)| f.to_string()).unwrap_or_default()
}
/// M.Link.id
pub fn link_id(from: &str, n: &str) -> String { format!("t:{from}:{n}") }

/// The thread that sent a message from another thread, as a machine that mirrors it names it: one of that
/// machine's own ("<link>~<id>" there, the link being this one) is its own again, one of the owner's is named
/// through the link.
pub fn sender(l: &str, from: &str) -> String { if is(from) { if link(from) == l { local(from) } else { from.to_string() } } else { id(l, from) } }
/// A message id as the mirror names it ("t:<from>:<n>" with the sender renamed; other ids are kept).
pub fn sent(l: &str, msg: &str) -> String {
    let from = link_from(msg);
    if from.is_empty() { return msg.to_string() }
    link_id(&sender(l, &from), &msg.chars().skip(3 + from.chars().count()).collect::<String>())
}
/// Mail addresses retain their kind while the object id follows `sender`.
pub fn address(l: &str, from: &str) -> String {
    if let Some(r) = from.strip_prefix("thread:") { format!("thread:{}", sender(l, r)) }
    else if let Some(r) = from.strip_prefix("bot:") { format!("bot:{}", sender(l, r)) } else { from.to_string() }
}

// ----- changes, as a client here reads them -----
#[derive(Clone, Copy)]
enum K { Id, Opt, Sender, Sent, Addr, Text }
use K::*;
/// Fr.change's table: for each change that shows a thread, a bot and its space, the fields renamed into this hub's
/// ids. Settings, devices, links, webhooks, rooms (shared rooms travel on their own), ordering and usage are absent:
/// they stay home.
/// Every variant `change` renames; the test below checks each field against codec's wire names.
/// Requests to a thing elsewhere: only these may go to a linked machine (thread.delete is not one: law far_no_delete).
pub const METHODS: &[&str] = &["turn.start", "turn.interrupt", "ask.answer", "queue.move", "queue.remove", "queue.resume", "queue.steer", "queue.edit",
    "thread.settle", "thread.unsettle", "thread.back", "thread.rename", "thread.btw", "thread.modes", "thread.provider", "thread.effort", "bots.space", "bots.routine_run",
    "thread.create", "thread.fork", "thread.archive", "thread.unarchive", "thread.pin", "thread.unpin",
    "project.add", "project.new", "project.remove", "fs.list", "fs.mkdir", "fs.find", "thread.detach", "todos.edit", "kicad.open", "viewer.state", "boards.list"];
pub fn may(m: &str) -> bool { METHODS.contains(&m) }
/// The params that name a thing (the ones renamed on the way)
const KEYS: &[&str] = &["thread", "id", "bot", "routine", "task", "parent", "child", "project", "ask"];
/// `Fr.params.link`: the link a request's params name, "" for nothing elsewhere. The first field that decides wins: a renamed key holding a
/// `<link>~<id>`, or `on` holding a machine's link that is not itself an id elsewhere.
pub fn params_link(p: &Value) -> String {
    let Some(o) = p.as_object() else { return String::new() };
    for (k, v) in o {
        let Some(v) = v.as_str() else { continue };
        if KEYS.contains(&k.as_str()) && is(v) { return link(v) }
        if k == "on" && !v.is_empty() && !is(v) { return v.to_string() }
    }
    String::new()
}
/// `Fr.params.local`: the params with that link's ids made local (the request as the owning hub takes it)
pub fn params_local(p: &Value, l: &str) -> Value {
    let Some(o) = p.as_object() else { return p.clone() };
    Value::Object(o.iter().map(|(k, v)| (k.clone(), match v.as_str() { Some(s) if KEYS.contains(&k.as_str()) && link(s) == l && is(s) => Value::String(local(s)), _ => v.clone() })).collect())
}
/// `Fr.reply.on`: the owning hub's reply on the receiving hub: the caller's request id back, and the thread a reply names made an id here
pub fn reply_on(mut r: Value, id: &Value, l: &str) -> Value {
    if let Some(o) = r.as_object_mut() {
        if o.contains_key("id") { o.insert("id".into(), id.clone()); }
        if let Some(t) = o.get("thread").and_then(Value::as_str).map(str::to_string) { o.insert("thread".into(), Value::String(if is(&t) { t } else { opt(l, &t) })); }
    }
    r
}
pub const VARIANTS: &[&str] = &["ProjectCreated", "ProjectRemoved", "ThreadRenamed", "ThreadSettled", "ThreadUnsettled", "ThreadWoke", "ThreadArchived", "ThreadPinned",
    "ThreadDeleted", "ModesSet", "ProviderSet", "AskClosed", "BotDeleted", "RoutineDeleted", "RoutineFired", "RoutinePlan", "RoutineSlot", "RoutineDone", "ThreadCreated",
    "MessagePosted", "MessageFrom", "TurnChanged", "PendingChanged", "TurnQueued", "QueueTaken", "QueueEdited", "QueueRemoved", "QueueMoved", "QueueTakenOne", "TodosChanged",
    "TodosSet", "WorkSet", "ActivityLogged", "ThreadForked", "TaskDelegated", "TaskStateSet", "TaskDelivered", "AskOpened", "BotSet", "RoutineSet", "Reminded", "MemorySaved",
    "MemoryForgot", "TaskSet", "TaskDeleted", "SpaceSet", "SpacePageSet", "ThreadParentSet", "ThreadLaunched"];
fn table(variant: &str) -> Option<&'static [(&'static str, K)]> {
    Some(match variant {
        "ProjectCreated" | "ProjectRemoved" | "ThreadRenamed" | "ThreadSettled" | "ThreadUnsettled" | "ThreadWoke" | "ThreadArchived" | "ThreadPinned"
        | "ThreadDeleted" | "ModesSet" | "ProviderSet" | "AskClosed" | "BotDeleted" | "RoutineDeleted" | "RoutineFired" | "RoutinePlan" | "RoutineSlot" | "RoutineDone" => &[("id", Id)],
        "ThreadCreated" => &[("id", Id), ("project", Opt)],
        "MessagePosted" => &[("thread", Id), ("msg", Sent), ("text", Text)],
        "MessageFrom" => &[("thread", Id), ("msg", Sent), ("from", Addr)],
        "TurnChanged" | "PendingChanged" | "TurnQueued" | "QueueTaken" | "QueueEdited" | "QueueRemoved" | "QueueMoved" | "QueueTakenOne"
        | "TodosChanged" | "TodosSet" | "WorkSet" => &[("thread", Id)],
        "ActivityLogged" => &[("thread", Id), ("summary", Text)],
        "ThreadForked" => &[("id", Id), ("source", Id)],
        "TaskDelegated" => &[("task", Id), ("parent", Opt), ("child", Opt)],
        "TaskStateSet" | "TaskDelivered" => &[("task", Id)],
        "AskOpened" => &[("id", Id), ("thread", Id)],
        "BotSet" => &[("id", Id), ("thread", Id)],
        "RoutineSet" => &[("id", Id), ("bot", Id)],
        "Reminded" => &[("id", Id), ("bot", Id), ("thread", Id)],
        "MemorySaved" | "MemoryForgot" | "TaskSet" | "TaskDeleted" | "SpaceSet" | "SpacePageSet" => &[("bot", Id)],
        "ThreadParentSet" => &[("id", Id), ("parent", Opt)],
        "ThreadLaunched" => &[("id", Id), ("by", Sender)],
        _ => return None,
    })
}
/// The owning hub's change renamed into this hub's ids; None for what stays home. `origin` rewrites the thread and
/// project references inside a message's text (ORIGIN.Origin.show), given the text and the link.
pub fn change(l: &str, c: &Value, origin: &dyn Fn(&str, &str) -> String) -> Option<Value> {
    let t = table(c["$"].as_str()?)?;
    let mut o: Map<String, Value> = c.as_object()?.clone();
    for (f, k) in t {
        let Some(v) = o.get(*f).and_then(Value::as_str).map(str::to_string) else { continue };
        let r = match k { Id => id(l, &v), Opt => opt(l, &v), Sender => sender(l, &v), Sent => sent(l, &v), Addr => address(l, &v), Text => origin(&v, l) };
        o.insert((*f).to_string(), Value::String(r));
    }
    Some(Value::Object(o))
}

// ----- what the owning hub shares -----
/// What `shared` asks of the owner's state, after the change.
///
/// `ask_thread` MUST see a closed ask: `Fr.ask.shared` reads the asks after the change, where a closed ask
/// stays in the map with its flag off. A state that removes an ask on close (`commands::Fold`, the agents' world,
/// the lean state) makes this answer None, so `AskClosed` is never shared and the mirror's ask stays open forever.
/// Back it with the full state, or look the ask up before the fold. The same holds for `task_parent` of a finished task.
pub trait Owned {
    /// A thread of this hub's own (mirrors are client projections, never re-exported).
    fn thread(&self, id: &str) -> bool;
    /// The thread an ask is about, if the ask exists.
    fn ask_thread(&self, id: &str) -> Option<String>;
    /// The parent thread of a delegated task, if the task exists.
    fn task_parent(&self, id: &str) -> Option<String>;
}
fn thread_shared(o: &dyn Owned, th: &str) -> bool { o.thread(th) && !is(th) }
/// Whether a change (with the state after it) goes to the owner's other machines.
pub fn shared(o: &dyn Owned, c: &Value) -> bool {
    let f = |k: &str| c[k].as_str().unwrap_or("");
    let ts = |k: &str| thread_shared(o, f(k));
    match c["$"].as_str().unwrap_or("") {
        "ProjectCreated" | "ProjectRemoved" => !is(f("id")),
        "ThreadCreated" | "ThreadRenamed" | "ThreadSettled" | "ThreadUnsettled" | "ThreadWoke" | "ThreadArchived" | "ThreadPinned" | "ThreadForked"
        | "ThreadDeleted" | "ModesSet" | "ProviderSet" | "ThreadParentSet" | "ThreadLaunched" => ts("id"),
        "MessagePosted" | "MessageFrom" | "TurnChanged" | "PendingChanged" | "ActivityLogged" | "TurnQueued" | "QueueTaken" | "QueueEdited" | "QueueRemoved"
        | "QueueMoved" | "QueueTakenOne" | "TodosChanged" | "TodosSet" | "WorkSet" | "AskOpened" => ts("thread"),
        "TaskDelegated" => ts("parent"),
        "TaskStateSet" | "TaskDelivered" => o.task_parent(f("task")).map_or(false, |p| thread_shared(o, &p)) || (o.task_parent(f("task")).is_none() && thread_shared(o, "")),
        "AskClosed" => o.ask_thread(f("id")).map_or(false, |t| thread_shared(o, &t)) || (o.ask_thread(f("id")).is_none() && thread_shared(o, "")),
        "BotSet" | "BotDeleted" | "RoutineSet" | "RoutineDeleted" | "RoutineFired" | "RoutinePlan" | "RoutineSlot" | "RoutineDone" | "Reminded"
        | "MemorySaved" | "MemoryForgot" | "TaskSet" | "TaskDeleted" | "SpaceSet" | "SpacePageSet" => true,
        _ => false,
    }
}

// ----- batches -----
/// An item is a change as its own hub numbers it (its place in that log) and sends it (its wire JSON, ids its own).
#[derive(Clone, Debug, PartialEq)]
pub struct Item { pub n: u64, pub c: Value }
impl Item {
    pub fn json(&self) -> Value { json!({"n": self.n, "c": self.c}) }
    pub fn of(o: &Value) -> Item { Item { n: o["n"].as_u64().unwrap_or(0), c: o.get("c").cloned().unwrap_or(Value::Null) } }
}
/// What a batch leaves: the new head, the items to take, and whether a gap came first (then nothing is taken
/// and the rest is asked for).
#[derive(Debug, PartialEq)]
pub struct Got { pub head: u64, pub items: Vec<Item>, pub gap: bool }
/// A batch that follows `since` and ends at `to`, for a mirror at `head`.
pub fn take(head: u64, since: u64, to: u64, xs: &[Item]) -> Got {
    if since > head { return Got { head, items: vec![], gap: true } }
    Got { head: head.max(to), items: xs.iter().filter(|i| i.n > head).cloned().collect(), gap: false }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn same(_: &str, l: &str) -> String { format!("<{l}>") }
    fn id_(l: &str, t: &str) -> String { id(l, t) }
    struct St { threads: Vec<&'static str>, asks: Vec<(&'static str, &'static str)>, tasks: Vec<(&'static str, &'static str)> }
    impl Owned for St {
        fn thread(&self, id: &str) -> bool { self.threads.contains(&id) }
        fn ask_thread(&self, id: &str) -> Option<String> { self.asks.iter().find(|(a, _)| *a == id).map(|(_, t)| t.to_string()) }
        fn task_parent(&self, id: &str) -> Option<String> { self.tasks.iter().find(|(a, _)| *a == id).map(|(_, p)| p.to_string()) }
    }
    fn st() -> St { St { threads: vec!["t1", "p9~tm"], asks: vec![("a1", "t1"), ("a2", "p9~tm")], tasks: vec![("k1", "t1")] } }

    #[test] fn far_id_parts() { assert_eq!(format!("{} {}", link(&id("p1", "t1")), local(&id("p1", "t1"))), "p1 t1"); }
    #[test] fn far_id_here() { assert_eq!(format!("{}|{}", link("t1790715007-34164"), local("t1790715007-34164")), "|t1790715007-34164"); }
    #[test] fn far_ids_with_more_tildes() {
        assert_eq!((link("a~b~c"), local("a~b~c")), ("a".to_string(), "b~c".to_string()));
        assert_eq!(opt("p1", ""), ""); assert_eq!(opt("p1", "x"), "p1~x"); assert!(is("a~b") && !is("ab"));
    }
    #[test] fn far_sent_names_sender() {
        assert_eq!(format!("{} {} {}", sent("p1", &link_id("p1~ta", "m5")), sent("p1", &link_id("tb", "m6")), sent("p1", "m1790715007-3")), "t:ta:m5 t:p1~tb:m6 m1790715007-3");
        assert_eq!(link_from("t:ta:m5"), "ta"); assert_eq!(link_from("t:ta"), ""); assert_eq!(link_from("m1"), "");
        assert_eq!(address("p1", "thread:p1~x"), "thread:x"); assert_eq!(address("p1", "bot:b"), "bot:p1~b"); assert_eq!(address("p1", "person"), "person");
    }
    #[test] fn far_change_names_link() {
        let c = change("L", &json!({"$":"MessagePosted","thread":"t9","msg":"m1","role":"user","text":"hi","at":5}), &same).unwrap();
        assert_eq!(c, json!({"$":"MessagePosted","thread":"L~t9","msg":"m1","role":"user","text":"<L>","at":5}));
        let c = change("L", &json!({"$":"ThreadCreated","id":"t9","project":"","title":"x","env":"","branch":"","worktree":"","provider":"","model":"","at":1}), &same).unwrap();
        assert_eq!((c["id"].as_str().unwrap(), c["project"].as_str().unwrap()), ("L~t9", ""), "no project stays none");
    }
    #[test] fn far_change_bot_named() {
        let c = change("L", &json!({"$":"BotSet","id":"b1","name":"Bub","thread":"t1","look":3,"persona":"p","at":9}), &same).unwrap();
        assert_eq!((c["id"].as_str().unwrap(), c["thread"].as_str().unwrap(), c["name"].as_str().unwrap()), ("L~b1", "L~t1", "Bub"));
    }
    #[test] fn far_settings_stay_home() {
        for v in ["SettingSet", "PeerSet", "HookSet", "DeviceRegistered", "RoomSet", "OrderPlaced", "TurnUsage", "Nope"] {
            let c = json!({"$":v,"key":"k","value":"v","id":"t1","at":1});
            assert!(!shared(&st(), &c) && change("p1", &c, &same).is_none(), "{v}");
        }
    }
    #[test] fn far_peers_stay_home() { let c = json!({"$":"PeerSet","id":"i","name":"n","url":"u","revoked":false,"at":1}); assert!(!shared(&st(), &c) && change("p1", &c, &same).is_none()); }
    #[test] fn far_only_threads_here_are_shared() {
        let s = st();
        let m = |t: &str| json!({"$":"MessagePosted","thread":t,"msg":"m","role":"user","text":"x","at":1});
        assert!(shared(&s, &m("t1")), "a thread here");
        assert!(!shared(&s, &m("p9~tm")), "a mirror is never re-exported");
        assert!(!shared(&s, &m("zz")), "no such thread");
        assert!(shared(&s, &json!({"$":"AskClosed","id":"a1","answer":"y","at":1})) && !shared(&s, &json!({"$":"AskClosed","id":"a2","answer":"y","at":1})) && !shared(&s, &json!({"$":"AskClosed","id":"gone","answer":"y","at":1})));
        assert!(shared(&s, &json!({"$":"TaskStateSet","task":"k1","state":"done","summary":"","at":1})) && !shared(&s, &json!({"$":"TaskDelivered","task":"nope","at":1})));
        assert!(shared(&s, &json!({"$":"BotSet","id":"b","name":"n","thread":"","look":1,"persona":"","at":1})), "bots are shared");
        assert!(shared(&s, &json!({"$":"ProjectCreated","id":"proj","title":"t","root":"/","at":1})) && !shared(&s, &json!({"$":"ProjectCreated","id":"p9~proj","title":"t","root":"/","at":1})));
        assert!(shared(&s, &json!({"$":"TaskDelegated","task":"k","parent":"t1","child":"","depth":1,"provider":"","model":"","at":1})));
    }
    fn it(n: u64) -> Item { Item { n, c: json!(n) } }
    #[test] fn far_gap_takes_nothing() { assert_eq!(take(3, 5, 9, &[it(6)]), Got { head: 3, items: vec![], gap: true }); }
    #[test] fn far_old_takes_nothing() { assert_eq!(take(9, 3, 6, &[it(4), it(6)]), Got { head: 9, items: vec![], gap: false }); }
    #[test] fn far_overlap_takes_new() { assert_eq!(take(5, 3, 8, &[it(4), it(6), it(8)]), Got { head: 8, items: vec![it(6), it(8)], gap: false }); }
    #[test] fn far_batch_again() { assert_eq!(take(8, 5, 8, &[it(6), it(8)]), Got { head: 8, items: vec![], gap: false }); }
    #[test] fn far_change_fork_names_its_source() {
        let c = change("L", &json!({"$":"ThreadForked","id":"t2","source":"t1","at":1}), &same).unwrap();
        assert_eq!((c["id"].as_str().unwrap(), c["source"].as_str().unwrap()), ("L~t2", "L~t1"), "a mirror's fork points at the mirrored source");
    }
    #[test] fn far_settingset_has_no_table() { assert!(table("SettingSet").is_none()); }
    #[test] fn far_item_roundtrip() { let i = it(7); assert_eq!(Item::of(&i.json()), i); let _ = id_("a", "b"); }
}
