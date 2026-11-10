//! The GPUI recent and plain-thread root. Borrows the state policies' lean fold; never replays or clones a log.
//! Approved differences for this slice: unread shown keys `bubHomes`, `online`, `version`, `error`,
//! `note`, `sel`, `making`, `lang`, `empty`, `older`, `settled`, `view`, `active`, `legacy`, `rowMenu`,
//! `removing`, `search`, `folders`, `tour`, `starNudge`, `find`, `island`, `islands`, `uber`, `hub`,
//! `hubs`, `usageMachines`, `found`, `tabs` are omitted. They belong to other clients/screens.
//! Overlay is the same reduced root (without the aggregate's `machine` field on projects),
//! with its hub key/name. Unsupported targets/actions return None, so Routed supplies a retryable error.
use super::native_sidebar::{text, NativeSidebar};
use super::picker;
use crate::{hub::Hub, sidebar, state::State};
use serde_json::{json, Value};
use std::{collections::{HashMap, HashSet}, sync::{Arc, OnceLock, Weak}};

#[derive(Clone, Default)]
pub struct Recent { hub: Arc<OnceLock<Weak<Hub>>> }
impl Recent {
    pub fn new() -> Self { Self::default() }
    pub fn bind(&self, hub: &Arc<Hub>) { super::settings::set_version(&hub.cfg.version); let _ = self.hub.set(Arc::downgrade(hub)); }
}
impl super::Screen for Recent {
    fn name(&self) -> &'static str { "recent/thread (native sidebar)" }
    fn view(&self, params: &Value) -> Option<Value> {
        if params["target"]["kind"]=="bot" && params["nativeSidebar"]==json!(1) && !params["botPage"].is_object() && !text(&params["target"],"id").contains('~') { return Some(json!({"stale":true})) } // an unknown bot: STALE_VIEW
        if params["target"]["kind"]=="room" && params["nativeSidebar"]==json!(1) && !params["roomPage"].is_object() { return Some(json!({"stale":true})) } // an unknown room: STALE_VIEW
        if !mine(params) { return refused("target.or-native-sidebar") }
        let Some(_hub)=self.hub.get().and_then(Weak::upgrade) else { return refused("screen.hub-not-bound") };
        let Some(policies)=crate::statehost::policies() else { return refused("state.not-started") };
        let setup=text(params,"action").starts_with("ob-") || policies.with_state(|st|super::onboard::due(st,&params["session"])).unwrap_or(false);
        // Settings → Devices: a press acts on the registry before the page reads it, so the list it draws is the one after the press (a new link listed,
        // a revoked device gone); opening the page reads it too (the page in the session is still the one before the press)
        let devpage=super::settings::page(&params["session"])=="devices" || (text(params,"action")=="settings-page" && text(params,"value")=="devices");
        let acted; let params=if devpage && text(params,"action").starts_with("device-") { match super::settings::device_act(text(params,"action"),text(params,"value")) { Some(patch)=>{ let mut c=params.clone(); crate::session::patch::merge(&mut c["session"],&patch); c["action"]=json!(""); c["value"]=json!(""); acted=c; &acted } None=>params } } else { params };
        let withinfo; let params=if super::settings::is_open(&params["session"]) || shows_thread(params) || setup { let mut c=params.clone(); c["hubInfo"]=_hub.info_now(); c["hubInfo"]["client"]=params["client"].clone(); if devpage { if let Some(r)=crate::net::start::routes() { c["hubInfo"]["devices"]=r.devices(); } } if setup || super::settings::page(&params["session"])=="machines" { if let Some(p)=crate::net::start::peering() { c["hubInfo"]["machines"]=p.page_data(); } } withinfo=c; &withinfo } else { params };
        // a 0.14 import of this hub: its card goes over any page (present/onboard.rs import_overlay)
        let withmig; let params=match crate::import014::last_published(&_hub.cfg.home) { Some(v) => { let mut c=params.clone(); c["importV014"]=v; withmig=c; &withmig } None => params };
        let m=_hub.mirrors();
        match policies.with_state(|st|m.shown(|far| { let map=unified_of(st,far,&|id|m.origin(id)); let marks: HashMap<String,Value>=far.iter().map(|(n,_)|(n.clone(),m.mark(n))).collect(); let here=m.here(); recent_unified(st,far,&Unified{ far:&map, marks:&marks, here:&here },params) })) { Some(out)=>out,None=>refused("state.booting") }
    }
}
fn mine(p: &Value) -> bool {
    let id=text(&p["target"],"id");
    p["nativeSidebar"]==json!(1) && match text(&p["target"],"kind") { "recent"=>id.is_empty(), "thread"=>!id.is_empty(), "bot"=>p["botPage"].is_object(), "room"=>p["roomPage"].is_object(), _=>false }
}
fn setting<'a>(st: &'a State,k: &str) -> &'a str { st.settings.get(k).map(String::as_str).unwrap_or("") }
pub use crate::sidebar::fuzzy_hit;
fn flag(s: &Value,k: &str) -> bool { s["open"][k].as_bool().unwrap_or(false) }
fn refused<T>(reason: &'static str) -> Option<T> { super::refusal::report(reason); None }

/// A target that shows a thread (a thread, or a bot, whose chat is one) needs the hub's models info for its picker.
pub fn shows_thread(p: &Value) -> bool { matches!(text(&p["target"],"kind"),"thread"|"bot") }
fn bots(st: &State, hub: &str, now: u64) -> Vec<Value> {
    let mut bots: Vec<_>=st.bots_nav.iter().filter(|(id,_)|!id.contains('~')).collect();
    let uber=setting(st,"uber.bot").to_string();
    bots.sort_by(|(ia,a),(ib,b)| (**ib==uber).cmp(&(**ia==uber)).then(a.created.cmp(&b.created)).then(ia.cmp(ib))); // the uberbot leads
    bots.iter().map(|(id,b)| {
        let name=if bots.iter().filter(|(_,other)|other.name.eq_ignore_ascii_case(&b.name)).count()>1 { format!("{} ({id})",b.name) }else{b.name.clone()};
        let t=st.threads.get(&b.thread);
        let (mood,note)=if st.asks.values().any(|(th,open)| th==&b.thread && *open) || t.map_or(false,|t|t.pending>0) { ("wait","needs you") }
            else if t.map_or(false,|t|t.turn==crate::state::Turn::Running) || st.queue.get(&b.thread).map_or(false,|q|!q.is_empty()) {
                if now<=b.talked.saturating_add(60) { ("talk","talking") }else{("work","working")}
            } else if t.map_or(false,|t|t.turn==crate::state::Turn::Failed) { ("error","failed") }
            else if now>t.map_or(0,|t|t.activity).saturating_add(900) { ("sleep","asleep") }else{("idle","ready")};
        json!({"id":format!("{hub}|{id}"),"name":name,"title":name,"act":if b.thread.is_empty(){"remote"}else{"bot"},
            "look":b.look,"mood":mood,"note":note,"uber":!uber.is_empty()&&**id==uber,"sel":false,"remote":false,"peer":"","cat":format!("{}:{mood}",b.look),"key":id,"link":"",
            "nav":if b.thread.is_empty(){format!("@remote:{id}")}else{format!("{hub}|{}",b.thread)}})
    }).collect()
}
fn rooms(st: &State, hub: &str) -> Vec<Value> {
    let mut rooms: Vec<_>=st.rooms_nav.iter().filter(|(id,_)|!id.starts_with("dm:")).collect(); rooms.sort_by_key(|(id,_)|*id);
    rooms.iter().map(|(id,r)| {
        let name=if rooms.iter().filter(|(_,other)|other.name.eq_ignore_ascii_case(&r.name)).count()>1 { format!("{} · {id}",r.name) }else{r.name.clone()};
        json!({"id":format!("{hub}|{id}"),"name":name,"members":r.members,"sel":false,"nav":format!("@room:{id}")})
    }).collect()
}

/// Pure root, shared by the oracle and the hub's bound Screen. Session/local are per view, never kept here.
/// Enter in the search (Find.go): the first project shown, its top thread. The query is dropped; a project with no thread goes nowhere (a new thread is the + beside it).
fn find_go(st: &State,p: &Value) -> Option<Value> {
    let q=text(&p["session"]["drafts"],"@f.pq").trim().to_string();
    let open: HashSet<String>=HashSet::new();
    let opts=sidebar::Opts { hub:text(p,"hub"),now:p["now"].as_u64().unwrap_or(0)/1000,selected:"",open:&open,older_open:true,query:&q };
    let plan=sidebar::plan(st,&opts);
    let named=|id:&str| q.is_empty() || st.projects.get(id).map_or(false,|pr|fuzzy_hit(&pr.title,&q));
    // the first project the search shows: its top thread, its top match when only its threads answered; failing those, a chat of the Conversation history
    let first=plan.ids().into_iter().find(|id| named(id) || plan.has_hit(id)).map(str::to_string);
    let top=first.and_then(|f| st.threads.iter().filter(|(_,t)|t.project==f && !t.archived && !t.deleted && (named(&f) || sidebar::word_hit(&t.title,&q))).max_by_key(|(id,t)|(t.pinned,t.activity,id.as_str().to_string())).map(|x|x.0.clone()))
        .or_else(|| plan.legacy(true).0.first().and_then(|r|r["id"].as_str()).map(|i|i.rsplit('|').next().unwrap_or("").to_string()))?;
    let mut c=p.clone(); c["action"]=json!("select"); c["value"]=json!(format!("{}|{top}",text(p,"hub")));
    c["session"]["drafts"]["@f.pq"]=json!("");
    Some(c)
}
pub fn recent(st: &State,p: &Value) -> Option<Value> { recent_far(st,&[],p) }
/// `recent` with the linked machines' display-only states (`crate::mirror`): `(machine, state)`, their projects after this hub's own, each labelled by machine.
/// Nothing of theirs is read for a decision: every action and the open thread are this hub's (`st`).
pub fn recent_far(st: &State,far: &[(String,&State)],p: &Value) -> Option<Value> { let marks=far.iter().map(|(n,_)|(n.clone(),crate::mirror::mark_of(n,false))).collect(); recent_unified(st,far,&Unified{ far:&HashMap::new(), marks:&marks, here:"" },p) }
/// One repository on two machines is one project: `far` maps a linked machine's project (`<link>~<id>`) to this hub's project of the
/// same origin (`crate::mirror::unify`); `marks` is each shown machine's marker by name (`crate::mirror::mark_of`), on every row of its threads.
/// `here` is this machine's name, for the machine switcher ("" when it is not known yet).
pub struct Unified<'a> { pub far: &'a HashMap<String,String>, pub marks: &'a HashMap<String,Value>, pub here: &'a str }
impl Unified<'_> { fn mark(&self,machine: &str) -> Value { self.marks.get(machine).cloned().unwrap_or_else(||crate::mirror::mark_of(machine,false)) } }
/// The unified map for the sidebar: the linked projects whose machine said their origin, against this hub's projects (oldest first).
pub fn unified_of(st: &State,far: &[(String,&State)],origin: &dyn Fn(&str)->String) -> HashMap<String,String> {
    let fo: Vec<(String,String)>=far.iter().flat_map(|(_,f)|f.projects.iter().filter(|(_,p)|!p.removed).map(|(id,_)|(id.clone(),origin(id)))).filter(|(_,o)|!o.is_empty()).collect();
    if fo.is_empty() { return HashMap::new() }
    let mut lp: Vec<(&String,&crate::state::Project)>=st.projects.iter().filter(|(id,p)|!p.removed && id.as_str()!=sidebar::SCRATCH && !p.root.is_empty()).collect();
    lp.sort_by(|a,b|a.1.created.cmp(&b.1.created).then(a.0.cmp(b.0)));
    let lo: Vec<(String,String)>=lp.into_iter().map(|(id,p)|(id.clone(),crate::mirror::origin_of_root(&p.root))).collect();
    crate::mirror::unify(&lo,&fo)
}
/// The machine switcher over the sidebar, when machines are linked: every machine, this one, then each linked one (an away one says so).
/// `value` is what `side-machine` takes; `on` the one in force.
pub fn machine_switch(far: &[(String,&State)],un: &Unified,pick: &str) -> Value {
    let here=if un.here.is_empty() { "This machine".to_string() } else { crate::mirror::short_name(un.here) };
    let mut v=vec![json!({"value":"","label":"All","on":pick.is_empty(),"away":false,"hint":"Every machine"}),
                   json!({"value":"@here","label":here,"on":pick=="@here","away":false,"hint":"This machine"})];
    for (n,_) in far { let m=un.mark(n); let away=m["away"].as_bool().unwrap_or(false);
        v.push(json!({"value":n,"label":m["short"],"on":pick==n,"away":away,"hint":if away { format!("{n} · away") } else { n.clone() }})); }
    json!(v)
}
/// A project of this machine that is also on linked machines: where its New thread can start, this machine first (`value` is the
/// project id `new-thread` takes there). An away machine's is offered too, and says so: the request waits for that machine's answer.
pub fn new_on<'a>(local: &str,linked: impl Iterator<Item=(&'a str,&'a str)>,un: &Unified) -> Value {
    let here=if un.here.is_empty() { "this machine".to_string() } else { crate::mirror::short_name(un.here) };
    let mut v=vec![json!({"value":local,"label":format!("New thread on {here}"),"away":false})];
    let mut seen=HashSet::new();
    for (m,fid) in linked { if !seen.insert(m) { continue } let mk=un.mark(m); let away=mk["away"].as_bool().unwrap_or(false);
        v.push(json!({"value":fid,"label":format!("New thread on {}{}",text(&mk,"short"),if away {" (away)"} else {""}),"away":away})); }
    json!(v)
}
/// The marker in the header of a thread drawn from `tst`: that machine's (full name), or none when it is this hub's state.
fn header_mark(far: &[(String,&State)],tst: &State,un: &Unified) -> Option<Value> { far.iter().find(|(_,f)|std::ptr::eq(*f,tst)).map(|(m,_)|un.mark(m)) }
/// `recent_far` with projects shared across machines shown once (`Unified`).
pub fn recent_unified(st: &State,far: &[(String,&State)],un: &Unified,p: &Value) -> Option<Value> {
    if text(p,"action")=="proj-go" { match find_go(st,p) { Some(c)=>return recent_unified(st,far,un,&c), None=>{ let mut c=p.clone(); c["action"]=json!("proj-find-q"); c["value"]=json!(""); return recent_unified(st,far,un,&c) } } }
    // a link clicked in a message: what it opens is decided here (present/linkopen.rs); a thread on a linked machine moves the target itself
    if text(p,"action")=="link-open" {
        let c=super::linkopen::request(st,far,un.here,p);
        if text(&c,"action")=="link-far" {
            if !mine(p) { return refused("target.or-native-sidebar") }
            let mut session=c["session"].clone(); let id=text(&c,"value").to_string();
            session["sel"]=json!(id);
            return Some(json!({"remoteReady":false,"nextTarget":{"kind":"thread","id":id},"session":session,"cmds":[],"local":p.get("local").cloned().unwrap_or(json!({"local.native.sidebar.pages":"","local.native.sidebar.context":"","local.native.sidebar.key":"","local.native.sidebar.budget":""}))}));
        }
        return recent_unified(st,far,un,&c);
    }
    if !mine(p) { return refused("target.or-native-sidebar") }
    let mut session=p.get("session").filter(|s|s.is_object()).cloned().unwrap_or(json!({}));
    // Ui.blank + Target.bind(recent). The actor reshapes this adopted session after presentation.
    for (k,v) in [("sel",json!("")),("adding",json!(false)),("panel",json!(false)),("rid",json!(0)),
        ("wantNew",json!(false)),("error",json!("")),("note",json!("")),("info",json!({})),("open",json!({})),("drafts",json!({})),("outbox",json!({}))] {
        session.as_object_mut()?.entry(k.to_string()).or_insert(v);
    }
    let room_page=&p["roomPage"];
    let bot_page=&p["botPage"]; // a bot's target shows its chat thread, under its tabs (session seam: bots::bot_page)
    let thread=if p["target"]["kind"]=="thread" { text(&p["target"],"id") }else if bot_page.is_object() { text(bot_page,"chat") }else{""};
    session["sel"]=json!(thread);
    if !session["drafts"].is_object() { session["drafts"]=json!({}); }
    for k in ["@boot","@bot","@bot.want","@btab","@del","@desk","@draft.bound","@mech.at","@presentation","@remote","@room","@solid.at","@telemetry.dismiss","@thread.want"] {
        session["drafts"].as_object_mut()?.entry(k.to_string()).or_insert(json!(""));
    }
    session["drafts"].as_object_mut()?.entry("@sync".to_string()).or_insert(json!("1"));
    let action=text(p,"action"); let value=text(p,"value"); let hub=text(p,"hub");
    // The hub's answers this request carries (Diff, Review) are applied before anything is answered; the commands they make go out first.
    let read_cmds=match p["reads"].as_array().filter(|r|!r.is_empty()) { Some(reads)=>crate::session::pages::apply_reads(&crate::session::dispatch::StateFacts(st),hub,&mut session,reads), None=>vec![] };
    let mut cmds=read_cmds.clone();
    // The shared pure dispatcher owns composer/commands/select; this root owns private sidebar paging.
    let mut adopted=p.clone(); adopted["session"]=session.clone();
    let dispatched=crate::session::dispatch::dispatch(&crate::session::dispatch::StateFacts(st),&adopted);
    if let Some(out)=dispatched {
        session=out.session; cmds=[read_cmds.clone(),out.cmds].concat();
        if let Some(target)=out.next_target {
            return Some(json!({"remoteReady":false,"nextTarget":target,"session":session,"cmds":cmds,
                "local":p.get("local").cloned().unwrap_or(json!({"local.native.sidebar.pages":"","local.native.sidebar.context":"","local.native.sidebar.key":"","local.native.sidebar.budget":""}))}));
        }
    } else if let Some(o)=super::onboard::act(st,hub,&session,p.get("hubInfo").unwrap_or(&Value::Null),&super::onboard::Client::of(p),&super::onboard::home_dir(),action,value) {
        let ended=super::onboard::ended(action,&o.patch);
        crate::session::patch::merge(&mut session,&o.patch); cmds=[read_cmds.clone(),o.cmds].concat();
        // setup ended with no thread open: the person lands in their newest thread (the step said they could write in it after setup),
        // not on a page whose message box sends nowhere
        if ended && text(&session,"sel").is_empty() { if let Some(id)=super::onboard::land(st) {
            let mut a=p.clone(); a["session"]=session.clone(); a["action"]=json!("select"); a["value"]=json!(format!("{hub}|{id}"));
            if let Some(out)=crate::session::dispatch::dispatch(&crate::session::dispatch::StateFacts(st),&a) {
                session=out.session; cmds.extend(out.cmds);
                if let Some(target)=out.next_target {
                    return Some(json!({"remoteReady":false,"nextTarget":target,"session":session,"cmds":cmds,
                        "local":p.get("local").cloned().unwrap_or(json!({"local.native.sidebar.pages":"","local.native.sidebar.context":"","local.native.sidebar.key":"","local.native.sidebar.budget":""}))}));
                }
            }
        } }
    } else if let Some(o)=super::botforms::act(st,hub,&session,action,value) {
        crate::session::patch::merge(&mut session,&o.patch); cmds=[read_cmds.clone(),o.cmds].concat();
    } else if let Some(o)=super::settings::act(hub,&session,action,value) {
        crate::session::patch::merge(&mut session,&o.patch); cmds=[read_cmds.clone(),o.cmds].concat();
    } else {
    // the viewers' actions read the open thread's state: a linked machine's mirror for its thread (its project folder is over there)
    let sel_id=text(&session,"sel").to_string();
    let sel_st: &State=far.iter().map(|(_,f)|*f).find(|f|sel_id.contains('~') && f.threads.contains_key(sel_id.as_str())).unwrap_or(st);
    match action {
        "" | "native-sidebar-page" => {},
        // the sidebar's search box: the query is kept in the session (`@f.pq`); "proj-find" (the phones' toggle) changes nothing here
        "proj-find-q" => { session["drafts"]["@f.pq"]=json!(value); },
        "proj-find" => {},
        // what a clicked link could not open, said once (present/linkopen.rs)
        "link-note" => cmds.push(json!({"type":"say","tone":"note","text":value})),
        // the machine switcher over the sidebar: "" every machine, "@here" this one, else a linked machine by name (not `machine-*`: those are the Machines page's)
        "side-machine" => { session["drafts"]["@machine"]=json!(value); },
        "flag" | "side-older" | "side-settled" => {
            let raw=value.strip_prefix(&format!("{hub}|" )).unwrap_or(value);
            let key=match action { "side-older"=>"old-projects", "side-settled"=>"active-settled", _=>raw };
            let known=key=="old-projects" || key=="active-settled" || key=="conversation-history" || { let k=key.strip_prefix("arch:").or_else(||key.strip_prefix("project-closed:")).unwrap_or(key); st.projects.contains_key(k) || far.iter().any(|(_,f)|f.projects.contains_key(k)) };
            if !known { return refused("action.sidebar-flag") }
            if !session["open"].is_object() { session["open"]=json!({}); }
            let was=flag(&session,key); session["open"][key]=json!(!was);
        },
        // the viewer panel opened on a drawing (or closed): the watch, the project card's look, the boards (present/viewer.rs); other pages are not ported
        "view" => {
            // a thread of another machine is drawn from that machine's mirror (its project folder is a folder over there); the watch it asks for goes there too
            let vsel=text(&session,"sel").to_string();
            let vst: &State=far.iter().map(|(_,f)|*f).find(|f|vsel.contains('~') && f.threads.contains_key(vsel.as_str())).unwrap_or(st);
            let Some(o)=super::viewer::view_action(vst,hub,&session,value) else { return refused("action.view-unported") };
            crate::session::patch::merge(&mut session,&o.patch);
            cmds=[read_cmds.clone(),o.before].concat();
            crate::session::dispatch::fab_tail(&crate::session::dispatch::StateFacts(st),hub,&mut session,&mut cmds);
            if o.boards_look {
                let rid=session["rid"].as_u64().unwrap_or(0);
                cmds.push(json!({"type":"send","hub":hub,"text":format!("{{\"id\":{rid},\"m\":\"boards.list\",\"p\":{{\"thread\":{}}}}}",serde_json::to_string(thread).unwrap_or_default())}));
                session["rid"]=json!(rid+1);
            }
        },
        "bot" => {
            let id=value.strip_prefix(&format!("{hub}|" )).unwrap_or(value);
            if !st.bots_nav.contains_key(id) { return refused("action.bot-unknown") }
            if let Some(o)=session.as_object_mut() { o.remove("route"); }
            return Some(json!({"remoteReady":false,"nextTarget":{"kind":"bot","id":id},"session":session,"cmds":cmds,"local":p.get("local").cloned().unwrap_or(json!({"local.native.sidebar.pages":"","local.native.sidebar.context":"","local.native.sidebar.key":"","local.native.sidebar.budget":""}))}));
        },
        // a room's page: its row opens it, the post box (`bfield` rpost) is sent as a post by the person (`bots.post`)
        "room" => {
            let id=value.strip_prefix(&format!("{hub}|" )).unwrap_or(value);
            if !st.rooms_nav.contains_key(id) || id.starts_with("dm:") { return refused("action.room-unknown") }
            if let Some(o)=session.as_object_mut() { o.remove("route"); }
            return Some(json!({"remoteReady":false,"nextTarget":{"kind":"room","id":id},"session":session,"cmds":cmds,"local":p.get("local").cloned().unwrap_or(json!({"local.native.sidebar.pages":"","local.native.sidebar.context":"","local.native.sidebar.key":"","local.native.sidebar.budget":""}))}));
        },
        "room-post" if room_page.is_object() => {
            let body=text(&session["drafts"],"@f.rpost").trim().to_string();
            if !body.is_empty() {
                let rid=session["rid"].as_u64().unwrap_or(0);
                cmds.push(json!({"type":"send","hub":hub,"text":json!({"id":rid,"m":"bots.post","p":{"room":text(&p["target"],"id"),"text":body}}).to_string()}));
                session["rid"]=json!(rid+1); session["drafts"]["@f.rpost"]=json!("");
            }
        },
        // the to-do list's editing (Todos.edit.rpc): check, text, move, delete, add; the hub's `todos.edit` applies it and the list comes back from the log
        "todo-check"|"todo-text"|"todo-move"|"todo-del"|"todo-add" if !thread.is_empty() => {
            let rows=super::panels::todos(st,thread);
            let row=|id:&str| rows["rows"].as_array().and_then(|r|r.iter().find(|x|text(x,"id")==id)).cloned();
            let (id,arg)=value.split_once('\u{1f}').unwrap_or((value,""));
            let p=match action {
                "todo-add"=>{ let t=text(&session["drafts"],"@f.todonew").trim().to_string(); if t.is_empty() { json!(null) } else { session["drafts"]["@f.todonew"]=json!(""); json!({"thread":thread,"op":"add","id":"","text":t}) } },
                "todo-text"=>match row(id) { Some(_)=>{ let t=text(&session["drafts"],&format!("@f.todo.{id}")).trim().to_string(); if t.is_empty() { json!(null) } else { json!({"thread":thread,"op":"set","id":id,"text":t,"status":""}) } }, None=>json!(null) },
                "todo-check"=>match row(id) { Some(r)=>json!({"thread":thread,"op":"set","id":id,"text":text(&r,"text"),"status":if text(&r,"status")=="completed" {"pending"} else {"completed"}}), None=>json!(null) },
                "todo-move"=>match row(id) { Some(_)=>json!({"thread":thread,"op":"move","id":id,"to":arg.parse::<u64>().unwrap_or(0)}), None=>json!(null) },
                _=>match row(id) { Some(_)=>json!({"thread":thread,"op":"delete","id":id}), None=>json!(null) },
            };
            if !p.is_null() {
                let rid=session["rid"].as_u64().unwrap_or(0);
                cmds.push(json!({"type":"send","hub":hub,"text":json!({"id":rid,"m":"todos.edit","p":p}).to_string()}));
                session["rid"]=json!(rid+1);
            }
        },
        // Open in KiCad (the viewer bar's button): the hub starts KiCad on the thread's design (kicad/open.rs) and says how it went in the thread
        "kicad-open" if !thread.is_empty() => {
            let rid=session["rid"].as_u64().unwrap_or(0);
            cmds.push(json!({"type":"send","hub":hub,"text":json!({"id":rid,"m":"kicad.open","p":{"thread":thread,"key":value}}).to_string()}));
            session["rid"]=json!(rid+1);
        },
        "routine-edit"|"routine-save" if bot_page.is_object() => {
            let Some(o)=super::botforms::routine_act(bot_page,text(&p["target"],"id"),hub,&session,action,value) else { return refused("action.routine-unknown") };
            crate::session::patch::merge(&mut session,&o.patch); cmds=[read_cmds.clone(),o.cmds].concat();
        },
        "bot-tab" if bot_page.is_object() => { session["route"]=json!({"kind":"bot-tab","bot":text(&p["target"],"id"),"tab":text(bot_page,"tab")}); if let Some(o)=session.as_object_mut() { o.remove("memoryFrom"); } },
        "bot-memory" if bot_page.is_object() => { session["route"]=json!({"kind":"bot-tab","bot":text(&p["target"],"id"),"tab":"memory"}); session["memoryFrom"]=json!(bot_page["memoryFrom"].as_u64().unwrap_or(0).to_string()); },
        "space-act" if bot_page.is_object() => {
            let bot=text(&p["target"],"id"); let rid=session["rid"].as_u64().unwrap_or(0);
            let (a,v)=value.split_once('\u{1f}').unwrap_or((value,""));
            cmds.push(json!({"type":"send","hub":hub,"text":json!({"id":rid,"m":"bots.space","p":{"bot":bot,"action":a,"value":v}}).to_string()}));
            session["rid"]=json!(rid+1);
        },
        "hook-revoke" if bot_page.is_object() => {
            let rid=session["rid"].as_u64().unwrap_or(0);
            if !bot_page["hooks"].as_array().map_or(false,|hs|hs.iter().any(|h|text(h,"id")==value)) { return refused("action.hook-unknown") }
            cmds.push(json!({"type":"send","hub":hub,"text":json!({"id":rid,"m":"bots.hook_revoke","p":{"hook":value}}).to_string()}));
            session["rid"]=json!(rid+1);
        },
        "hook-refresh" if bot_page.is_object() => {},
        "task-run"|"task-del" if bot_page.is_object() => {
            let bot=text(&p["target"],"id"); let rid=session["rid"].as_u64().unwrap_or(0);
            let (slug,params)=value.split_once('\u{1f}').unwrap_or((value,"{}"));
            if !bot_page["tasks"].as_array().map_or(false,|ts|ts.iter().any(|t|text(t,"slug")==slug)) { return refused("action.task-unknown") }
            let q=if action=="task-run" { json!({"bot":bot,"op":"run","slug":slug,"params":serde_json::from_str::<Value>(params).unwrap_or(json!({}))}) } else { json!({"bot":bot,"op":"delete","slug":slug}) };
            cmds.push(json!({"type":"send","hub":hub,"text":json!({"id":rid,"m":"bots.task","p":q}).to_string()}));
            session["rid"]=json!(rid+1);
        },
        "mem-forget"|"routine-toggle"|"routine-run"|"routine-delete" if bot_page.is_object() => {
            let bot=text(&p["target"],"id"); let rid=session["rid"].as_u64().unwrap_or(0);
            let row=bot_page["routines"].as_array().and_then(|rs|rs.iter().find(|r|text(r,"id")==value));
            let (m,q)=match (action,row) {
                ("mem-forget",_)=>("bots.forget",json!({"bot":bot,"key":value})),
                ("routine-toggle",Some(r))=>("bots.routine",json!({"bot":bot,"routine":value,"name":text(r,"name"),"prompt":text(r,"prompt"),"on":r["on"]!=json!(true),"keep":true})),
                ("routine-run",_)=>("bots.routine_run",json!({"routine":value})),
                ("routine-delete",_)=>("bots.routine_delete",json!({"routine":value})),
                _=>return refused("action.routine-unknown"),
            };
            cmds.push(json!({"type":"send","hub":hub,"text":json!({"id":rid,"m":m,"p":q}).to_string()}));
            session["rid"]=json!(rid+1);
        },
        // Mech's part chips and Refresh (present/viewer.rs `mech_action`)
        "mech-part" | "mech-refresh" => {
            let Some(o)=super::viewer::mech_action(st,hub,&session,value,action=="mech-refresh") else { return refused("action.mech-unported") };
            crate::session::patch::merge(&mut session,&o.patch);
            cmds=o.before;
        },
        // the Files tab: a folder, up, back from a preview, a file opened (present/viewer.rs `files_action`)
        "fm" | "fm-cd" | "fm-up" | "fm-open" => {
            let Some(o)=super::viewer::files_action(sel_st,hub,&session,action,value) else { return refused("action.files-unported") };
            crate::session::patch::merge(&mut session,&o.patch);
            cmds=o.before;
        },
        // another board of a project with several (present/viewer.rs `board_action`)
        "view-board" => {
            let Some(o)=super::viewer::board_action(sel_st,hub,&session,value) else { return refused("action.board-unported") };
            crate::session::patch::merge(&mut session,&o.patch);
            cmds=o.before;
        },
        // the Boards menu opened: the project's boards read again (present/viewer.rs `boards_look`)
        "view-boards" => {
            let Some(c)=super::viewer::boards_look(sel_st,hub,&session) else { return refused("action.board-unported") };
            session["rid"]=json!(session["rid"].as_u64().unwrap_or(0)+1); cmds=c;
        },
        // a piece tapped (or none): the board's probe, kept by the hub (present/viewer.rs `probe_action`)
        "view-probe" => {
            let Some(c)=super::viewer::probe_action(sel_st,hub,&session,value) else { return refused("action.probe-unported") };
            if !c.is_empty() { session["rid"]=json!(session["rid"].as_u64().unwrap_or(0)+1); } cmds=c;
        },
        // the Layers menu: hide/show a layer, the active layer, All/Dim/Only, Ctrl+H, Reset layers (present/viewer.rs `layer_action`)
        "view-layer" | "view-layer-pick" | "view-layer-mode" | "view-layer-cycle" | "view-layers-reset" => {
            let Some(patch)=super::viewer::layer_action(sel_st,&session,action,value) else { return refused("action.layer-unported") };
            crate::session::patch::merge(&mut session,&patch);
        },
        // a sheet of the schematic (present/viewer.rs `sheet_action`)
        "view-sheet" => {
            let Some(o)=super::viewer::sheet_action(sel_st,hub,&session,value) else { return refused("action.sheet-unported") };
            crate::session::patch::merge(&mut session,&o.patch);
            cmds=o.before;
        },
        _ => return refused("action.unported"),
    } }
    // Dv.sync: an open Diff follows its thread
    // (a thread of another machine: its turn is in that machine's mirror, so its Diff is asked again when that turn moves)
    if !thread.is_empty() { let hold: &State=far.iter().map(|(_,f)|*f).find(|f|thread.contains('~') && f.threads.contains_key(thread)).unwrap_or(st); crate::session::pages::dv_sync(&crate::session::dispatch::StateFacts(hold),hub,&mut session,&mut cmds); }
    // Fab.sync for every request on a thread, view.open included: the window opens a thread with view.open (no select action), and the card
    // (Buy it in the header) comes only from fab.look. fab_tail asks only when where the thread stands changed since the last ask (@fab.at).
    if !thread.is_empty() { crate::session::dispatch::fab_tail(&crate::session::dispatch::StateFacts(st),hub,&mut session,&mut cmds); }
    // A thread is drawn only when the async session seam supplied a native window and indexed page.
    // Composer fields are drawn from the same adopted draft that the action dispatcher patches.
    // a thread of a linked machine is drawn from that machine's display state (its page and window come from the mirror)
    let tst: &State=far.iter().map(|(_,f)|*f).find(|f|thread.contains('~') && f.threads.contains_key(thread)).unwrap_or(st);
    if thread.contains('~') && std::ptr::eq(tst,st) { return refused("thread.far-unknown") }
    let shown_thread=if thread.is_empty() { Value::Null }else{
        if !p["window"].is_object() { return refused("window.missing") }
        let mut params=p.clone(); params["session"]=session.clone();
        match super::thread::thread_object_checked(tst,hub,thread,&params,text(&session["drafts"],thread)) {
            Ok(value)=>value, Err(reason)=>return refused(reason),
        }
    };
    // a thread of another machine: its marker with the full name, for the header (this machine's threads have none)
    let mut shown_thread=shown_thread;
    if shown_thread.is_object() { if let Some(m)=header_mark(far,tst,un) { shown_thread["mark"]=m; } }
    let local_mode=text(&session["drafts"],"@local.view");
    let mode: &'static str=if local_mode=="projects" { "projects" } else if local_mode=="active" || setting(st,"sidebar.view")=="active" { "active" } else { "projects" };
    let query=text(&session["drafts"],"@f.pq").trim().to_string();
    // the machine switcher's choice (`@machine`): one no longer linked is every machine again
    let mpick=text(&session["drafts"],"@machine"); let mpick=if mpick=="@here" || far.iter().any(|(n,_)|n==mpick) { mpick.to_string() } else { String::new() };
    let here_on=mpick.is_empty() || mpick=="@here";
    let no_join: HashMap<String,String>=HashMap::new(); let un_far: &HashMap<String,String>=if mpick.is_empty() { un.far } else { &no_join };
    let Some(mut ns)=NativeSidebar::prepare(p,mode,&query,&session["open"],st.projects.keys().cloned().chain(far.iter().flat_map(|(_,f)|f.projects.keys().cloned())).collect()) else { return refused("sidebar.key-or-capability") };
    if action=="native-sidebar-page" && !ns.accept(value) { return Some(json!({"refused":true,"local":p.get("local").cloned().unwrap_or(json!({}))})) }
    if action=="native-sidebar-page" { session["drafts"]["@menu"]=json!(""); session["drafts"]["@del"]=json!(""); }
    let open: HashSet<String>=session["open"].as_object().into_iter().flatten().filter(|(_,v)|v.as_bool()==Some(true)).map(|(k,_)|k.clone()).collect();
    // a search unfolds the older projects too (Side.unfolded)
    let opts=sidebar::Opts { hub,now:p["now"].as_u64().unwrap_or(0)/1000,selected:thread,open:&open,older_open:flag(&session,"old-projects") || !query.is_empty(),query:&query };
    let plan=sidebar::plan(st,&opts);
    // a linked project shown as one with a project here opens and shuts with it
    let mut fopen=open.clone();
    for (f,l) in un_far.iter() { if open.contains(l) { fopen.insert(f.clone()); } if open.contains(&format!("arch:{l}")) { fopen.insert(format!("arch:{f}")); } }
    let fopts=sidebar::Opts { open:&fopen,..opts };
    let fplans: Vec<(&str,&State,sidebar::Plan)>=far.iter().filter(|(m,_)|mpick.is_empty() || *m==mpick).map(|(m,f)|(m.as_str(),*f,sidebar::plan(f,&fopts))).collect();
    // this hub's project -> the linked ones shown in it: (machine, state, plan, far id)
    let mut partners: HashMap<&str,Vec<(&str,&State,&sidebar::Plan,&str)>>=HashMap::new();
    for (m,f,fp) in &fplans { for id in fp.ids() { if let Some(l)=un_far.get(id) { if st.projects.contains_key(l.as_str()) { partners.entry(l.as_str()).or_default().push((*m,*f,fp,id)); } } } }
    let selected_project=st.threads.get(thread).map(|t|t.project.as_str()).unwrap_or("");
    // Find.rows: the projects whose name fuzzy-matches, and those holding a thread the search finds; the one holding the open thread stays (Side.cur.held)
    let far_hit=|id: &str| partners.get(id).map_or(false,|v|v.iter().any(|(_,_,fp,f)|fp.has_hit(f)));
    let mut ids: Vec<&str>=plan.ids().into_iter().filter(|id| here_on || *id==selected_project).filter(|id| query.is_empty() || *id==selected_project || st.projects.get(*id).map_or(false,|p|fuzzy_hit(&p.title,&query)) || plan.has_hit(id) || far_hit(id)).collect();
    let joined: HashSet<&str>=ids.iter().filter_map(|id|partners.get(id)).flatten().map(|(_,_,_,f)|*f).collect();
    // one list over machines: each machine's projects keep their order, and stand by recency among the others' (live first, then the newest);
    // a project shown as one with a linked machine's stands where the livelier of them would
    let lowest=|pl: &sidebar::Plan,id: &str| pl.place(id).unwrap_or((2,std::cmp::Reverse(0)));
    let mut lists=vec![ids.drain(..).map(|id|(partners.get(id).into_iter().flatten().map(|(_,_,fp,f)|lowest(fp,f)).fold(lowest(&plan,id),std::cmp::min),id)).collect::<Vec<_>>()];
    for (_,f,fp) in &fplans { lists.push(fp.ids().into_iter().filter(|id| !joined.contains(id)).filter(|id| query.is_empty() || f.projects.get(*id).map_or(false,|p|fuzzy_hit(&p.title,&query)) || fp.has_hit(id)).map(|id|(lowest(fp,id),id)).collect()); }
    let ids: Vec<&str>=sidebar::merge_ordered(lists,&|x|x.0).into_iter().map(|x|x.1).collect();
    let project_limit=ns.project_limit();
    // For now an explicitly opened thread in a folded Older project is unported. The recent list
    // already offers only visible projects; forcing an excluded project needs the filter exception port.
    // (a removed project's thread still draws, with no row for it)
    if mode!="active" && !selected_project.is_empty() && selected_project!=sidebar::SCRATCH && !ids.contains(&selected_project) && st.projects.get(selected_project).map_or(false,|p|!p.removed) { return refused("sidebar.selected-project-hidden") }
    let ids: Vec<&str>=if mode=="active" { vec![] } else { ids };
    let (mut indices,projects_page)=ns.slice_ids(&ids,"projects","",project_limit,selected_project);
    indices.sort_unstable(); // Mob.native.keep_projects keeps source order, including the selected exception.
    let selected: Vec<&str>=indices.into_iter().map(|i|ids[i]).collect();
    let joined_sel: Vec<&str>=selected.iter().filter_map(|id|partners.get(id)).flatten().map(|(_,_,_,f)|*f).collect();
    let row_limit=(ns.row_budget()/(plan.list_count(&selected)+fplans.iter().map(|(_,_,fp)|fp.list_count(&selected)+fp.list_count(&joined_sel)).sum::<usize>()).max(1)).max(1);
    // a thread row's place among another machine's rows of the same project: its shelf's order key (sidebar::merge_rows)
    let row_state=|qid: &str| -> Option<(&State,String)> { let id=qid.strip_prefix(&format!("{hub}|")).unwrap_or(qid); if id.contains('~') { far.iter().map(|(_,f)|*f).find(|f|f.threads.contains_key(id)).map(|f|(f,id.to_string())) } else { st.threads.get(id).map(|_|(st,id.to_string())) } };
    let row_key=|kind: &str,qid: &str| -> i128 { row_state(qid).map_or(0,|(s,id)| sidebar::row_key(s,kind,&id)) };
    let projects: Vec<Value>=selected.into_iter().filter_map(|owner| {
        // the plan (and state) that holds this project: this hub's, else the linked machine's it came from
        let (pst,pplan,machine)=match fplans.iter().find(|(_,f,_)|f.projects.contains_key(owner)) { Some((m,f,fp)) if !st.projects.contains_key(owner) => (*f,fp,*m), _ => (st,&plan,"") };
        let mut project=pplan.project(owner,&mut |kind,ids| {
            let (mut indices,pager)=ns.slice_ids(ids,kind,owner,row_limit,thread);
            indices.sort_unstable(); // Mob.native.tree_rows filters in tree order.
            (indices,pager)
        })?;
        let icon=setting(pst,&format!("project.icon.{owner}"));
        project["icon"]=json!(super::badge::icon_of(icon));
        let color=super::badge::color_of(setting(pst,&format!("project.color.{owner}")),owner);
        project["color"]=json!(color); project["css"]=json!(super::badge::css_of(color));
        project["machine"]=json!(machine);
        // folded by its chevron (0.14's project-closed:<id>): the window draws its head alone
        project["closed"]=json!(flag(&session,&format!("project-closed:{owner}")));
        if machine.is_empty() && text(&session["drafts"],"@look")==owner { project["look"]=super::badge::editor(owner,setting(st,&format!("project.color.{owner}")),icon); }
        // another machine's project: its marker after the name ("on desktop"); this machine's has none
        if !machine.is_empty() { let m=un.mark(machine); sidebar::label_rows(&mut project,&m); project["mark"]=m; }
        // the same repository on a linked machine: its threads in this project, each row naming its machine
        if let Some(ps)=partners.get(owner).filter(|_|machine.is_empty()) {
            let (mut machines,mut marks): (Vec<Value>,Vec<Value>)=(vec![],vec![]);
            for (m,_,fp,fid) in ps {
                let Some(mut fproj)=fp.project(fid,&mut |kind,ids| { let (mut indices,pager)=ns.slice_ids(ids,kind,fid,row_limit,thread); indices.sort_unstable(); (indices,pager) }) else { continue };
                sidebar::label_rows(&mut fproj,&un.mark(m));
                sidebar::join_project(&mut project,fproj,&row_key);
                if !machines.contains(&json!(m)) { machines.push(json!(m)); marks.push(un.mark(m)); }
            }
            project["machines"]=json!(machines);
            // a new thread of this repository on the machine chosen: this one's project, or a linked one's (the request goes there)
            project["newOn"]=new_on(owner,ps.iter().map(|(m,_,_,fid)|(*m,*fid)),un);
            // the machines it is also on, after its name ("also on desktop"); each of their rows keeps its own marker
            project["also"]=json!(sidebar::also_label(&marks)); project["marks"]=json!(marks);
        }
        Some(project)
    }).collect();
    let ren=text(&session["drafts"],"@ren");
    let ren_kind=if text(&session["drafts"],"@ren.kind")=="project" {"project"} else {"thread"};
    let renaming=if ren.is_empty() || text(&session["drafts"],"@ren.inline")=="1" { Value::Null }else{json!({"id":format!("{hub}|{ren}"),"kind":ren_kind,"title":if ren_kind=="project" {"Rename project"} else {"Rename thread"},"text":text(&session["drafts"],"@f.title"),"yes":"Rename","no":"Cancel"})};
    let del=text(&session["drafts"],"@del");
    let deleting=if del.is_empty() { Value::Null }else{json!({"id":if del.contains('|'){del.to_string()}else{format!("{hub}|{del}")},"title":"Delete thread","body":st.threads.get(del.strip_prefix(&format!("{hub}|" )).unwrap_or(del)).map(|t|t.title.as_str()).unwrap_or(""),"yes":"Delete","no":"Cancel"})};
    let pages=json!([{"version":1,"hub":hub,"projects":projects_page,"active":null,"legacy":null}]);
    let shown=json!({"projects":projects,"sidebarPages":pages,"nativeSidebar":1,"active":[],"legacy":[],"thread":shown_thread,"rooms":rooms(st,hub),"bots":bots(st,hub,opts.now),"theme":setting(st,"ui.theme"),
        "renaming":renaming,"deleting":deleting,"bot":null,"settings":if super::settings::is_open(&session) { super::settings::view(st,&session,p.get("hubInfo").unwrap_or(&Value::Null)) }else{Value::Null},"newRoom":super::botforms::new_room(st,hub,&session).unwrap_or(Value::Null),"newBot":super::botforms::new_bot(&session).unwrap_or(Value::Null),"projectHome":null});
    let mut shown=shown;
    if !far.is_empty() { shown["machines"]=machine_switch(far,un,&mpick); }
    // the Active view: what needs the person first, then the recently settled folded under "Settled N" (a search shows them)
    if mode=="active" {
        let keep=|title:&str,proj:&str| query.is_empty() || fuzzy_hit(title,&query) || fuzzy_hit(proj,&query);
        let (live,faded)=if here_on { plan.active_keyed(&keep) } else { (vec![],vec![]) };
        let (mut lives,mut fadeds)=(vec![live],vec![faded]);
        for (m,_,fp) in &fplans { let (mut l,mut f)=fp.active_keyed(&keep);
            // a linked machine's thread names its machine, and the project it is shown in here
            for r in l.iter_mut().chain(f.iter_mut()).map(|x|&mut x.1) { r["machine"]=json!(m); r["mark"]=un.mark(m); let fid=text(r,"projectId").strip_prefix(&format!("{hub}|")).unwrap_or("").to_string(); if let Some(lp)=un.far.get(&fid).and_then(|l|st.projects.get(l).map(|p|(l,p))) { r["projectId"]=json!(format!("{hub}|{}",lp.0)); r["project"]=json!(lp.1.title); } }
            lives.push(l); fadeds.push(f); }
        // every machine's rows in one order: what needs the person first, then the newest
        let live: Vec<Value>=sidebar::merge_ordered(lives,&|x|x.0).into_iter().map(|x|x.1).collect();
        let faded: Vec<Value>=sidebar::merge_ordered(fadeds,&|x|x.0).into_iter().map(|x|x.1).collect();
        let open=flag(&session,"active-settled") || !query.is_empty();
        shown["active"]=json!(live);
        shown["activeSettled"]=json!({"count":faded.len(),"open":open,"rows":if open { faded } else { vec![] }});
    }
    // Conversation history: the Scratch chats. 0.14 stopped new ones and kept these; a search opens the group by itself, and the palette jumps to any of them
    { let open=flag(&session,"conversation-history") || !query.is_empty(); let (rows,history)=plan.legacy(open);
      shown["legacy"]=json!(rows);
      // the group's row stays while any chat is in it, the open one too (it has its row under the group, never under a project)
      let live=rows.iter().filter(|r| !history.iter().any(|h| h["id"]==r["id"])).count();
      if !history.is_empty() || live>0 { shown["conversationHistory"]=json!({"count":history.len()+live,"open":open,"toggle":query.is_empty(),"jump":history}); } }
    // the Older group: a row that opens or shuts it (a search opens it by itself)
    let older=plan.older_count()+fplans.iter().map(|(_,_,fp)|fp.older_count()).sum::<usize>();
    if mode!="active" && query.is_empty() && older>0 { shown["olderProjects"]=json!({"count":older,"open":flag(&session,"old-projects")}); }
    // the query in force, said back once the box has been used (the client takes it when it is not what it sent: Enter clears it)
    if session["drafts"].as_object().map_or(false,|d|d.contains_key("@f.pq")) { shown["search"]=json!({"query":query,"hint":"Search projects and threads"}); }
    { let f=picker::folders(&session["drafts"],&std::env::var("HOME").unwrap_or_default(),&st.projects.values().filter(|p|!p.removed).map(|p|p.root.as_str()).collect::<Vec<_>>()); if !f.is_null() { shown["folders"]=f; } } // (present only while the picker is open)
    if !thread.is_empty() { shown["stats"]=super::keys::stats(tst,thread); shown["tokChip"]=json!(super::keys::tok_chip(tst,thread)); } // the Context panel's rows (beside the thread: its object is the helper's, key for key)
    if bot_page.is_object() {
        let id=text(&p["target"],"id"); let key=format!("{hub}|{id}");
        let mut b=bot_page.clone();
        if text(&b,"tab")=="routines" { b["routine"]=super::botforms::routine_form(&session); }
        if let Some(rows)=shown["bots"].as_array_mut() { if let Some(row)=rows.iter_mut().find(|r|r["id"]==json!(key)) {
            row["sel"]=json!(true); b["mood"]=row["mood"].clone(); b["note"]=row["note"].clone(); b["cat"]=row["cat"].clone();
        } }
        shown["bot"]=b;
    }
    if room_page.is_object() { shown["bot"]=room_page.clone(); }
    if let Some(t)=super::onboard::view(st,&session,p.get("hubInfo").unwrap_or(&Value::Null),&super::onboard::Client::of(p),&super::onboard::home_dir()) { shown["tour"]=t; }
    { let r=super::onboard::resume(st,&session); if !r.is_null() { shown["setup"]=r; } }
    // a 0.14 home coming across: over everything, setup included (import014)
    let mi=p.get("importV014").filter(|v| v.is_object()).map(|v| json!({"import.v014": v.to_string()})).unwrap_or(Value::Null);
    if let Some(m)=super::onboard::import_overlay(st,&session,&mi) { shown["migrate"]=m; }
    let mut overlay=shown.clone();
    for project in overlay["projects"].as_array_mut().unwrap() { project.as_object_mut().unwrap().remove("machine"); }
    let window=json!({"contextMessages":[],"messageIds":[],"historyUnits":[],"partial":[],"ids":[],"anchor":{"id":"","offset":0},"older":"","olderNext":"","hasMore":false,"remaining":0,"newer":0,"total":0});
    let window=if thread.is_empty() { window }else{p["window"].clone()};
    Some(json!({"remoteReady":true,"shown":shown,"session":session,"cmds":cmds,"overlay":{"j":overlay,"key":hub,"name":hub.split(':').next().unwrap_or("")},"window":window,"local":ns.local()}))
}

#[cfg(test)]
mod tests {
    use super::*;
    // These are explicit output differences, not additional parity normalisations. The JS replay gate
    // still compares the whole answer and must report them until the corresponding slices are ported.
    const OMITTED: &[&str]=&["bubHomes","online","version","error","note","sel","making","lang","empty","older","settled","view","rowMenu","removing","search","folders","tour","starNudge","find","island","islands","uber","hub","hubs","usageMachines","found","tabs"];
    const OVERLAY_OMITTED: &[&str]=&["machineUsage","tabs","online","version","seq","error","bubHome","lang","note","sel","making","empty","older","view","settled","rowMenu","search","removing","tour","starNudge","find","folders","island","islandAt","uber","plane"];
    fn retain_root(v: &mut Value, omitted: &[&str]) {
        let m=v.as_object_mut().unwrap();
        for k in omitted { assert!(m.remove(*k).is_some(),"named approved difference absent in oracle: {k}"); }
    }
    fn oracle(fixture: &str) {
        let recording:Value=serde_json::from_str(fixture).unwrap(); let frames=recording["frames"].as_array().unwrap();
        let mut st=State::new(); st.lean=true; let mut now=0;
        for frame in &frames[..7] {
            let j=&frame["json"];
            if matches!(text(j,"t"),"log"|"changes") {
                for event in j["items"].as_array().unwrap() { st.apply(event); now=now.max(event["at"].as_u64().unwrap_or(0)); }
            }
        }
        let expected=&frames[7]["json"]["result"];
        let mut p=frames[3]["json"]["p"].clone();
        p["viewId"]=expected["viewId"].clone(); p["revision"]=json!(st.seq); p["now"]=json!(now*1000); p["nativeKey"]=json!("00112233445566778899aabbccddeeff");
        let mut actual=recent(&st,&p).expect("recorded recent request is ported");
        // approved differences: a project carries its badge colour (`color`, `css`; present/badge.rs) and whether its chevron folded it (`closed`), which 0.14's recording does not have
        for path in ["/shown/projects","/overlay/j/projects"] { for pr in actual.pointer_mut(path).and_then(|v| v.as_array_mut()).into_iter().flatten() { if let Some(o)=pr.as_object_mut() { o.remove("color"); o.remove("css"); o.remove("closed");
            // and a red row's want word (sidebar::want), drawn where its age goes
            for k in ["threads","settled","snoozed","archived"] { for r in o.get_mut(k).and_then(|v| v.as_array_mut()).into_iter().flatten() { if let Some(r)=r.as_object_mut() { r.remove("want"); } } } } } }
        let mut shown=expected["shown"].clone(); retain_root(&mut shown,OMITTED);
        assert_eq!(actual["shown"],shown,"retained GPUI shown fields differ (no value normalisations)");
        let mut overlay=expected["overlay"].clone(); retain_root(&mut overlay["j"],OVERLAY_OMITTED);
        assert_eq!(actual["overlay"],overlay,"retained overlay fields differ");
        for k in ["remoteReady","session","cmds","window"] { assert_eq!(actual[k],expected[k],"{k}"); }
    }
    #[test] fn recorded_js_recent_gpui() { oracle(include_str!("fixtures/session-recent.json")); }
    #[test] fn recorded_js_recent_gpuid() { oracle(include_str!("fixtures/session-recent-d.json")); }
    /// A fresh home (no projects, no threads): the sidebar descriptor has null pages, and the phone's bridge then requires `projects`, `active` and `legacy` to be
    /// arrays (empty) in BOTH `shown` and `overlay.j` (src/mobile/thin-native-host-307265.js sidebarRange); a missing key is its 'invalid view presentation'.
    #[test] fn a_fresh_home_sends_projects_active_and_legacy_as_empty_lists_in_shown_and_overlay() {
        let recording:Value=serde_json::from_str(include_str!("fixtures/session-recent.json")).unwrap(); let frames=recording["frames"].as_array().unwrap();
        let mut st=State::new(); st.lean=true;
        let mut p=frames[3]["json"]["p"].clone(); p["viewId"]=frames[7]["json"]["result"]["viewId"].clone(); p["revision"]=json!(1); p["now"]=json!(1000); p["nativeKey"]=json!("00112233445566778899aabbccddeeff");
        let out=recent(&st,&p).expect("a fresh home is claimed");
        for (name,v) in [("shown",&out["shown"]),("overlay.j",&out["overlay"]["j"])] {
            for k in ["projects","active","legacy"] { assert_eq!(v[k],json!([]),"{name}.{k} is an empty list"); }
            assert_eq!(v["nativeSidebar"],1,"{name}.nativeSidebar");
        }
        assert_eq!(out["shown"]["sidebarPages"],out["overlay"]["j"]["sidebarPages"]);
    }
    #[test] fn other_targets_and_unported_actions_are_unclaimed() {
        let s=State::new();
        for kind in ["thread","settings","bot","room","project"] {
            assert!(recent(&s,&json!({"target":{"kind":kind,"id":"t"},"nativeSidebar":1})).is_none());
        }
        assert!(recent(&s,&json!({"target":{"kind":"recent","id":""}})).is_none());
        assert!(recent(&s,&json!({"target":{"kind":"recent","id":""},"nativeSidebar":1,"action":"send"})).is_none());
    }
}

#[cfg(test)]
mod state_tests {
    use super::*;
    fn params() -> Value { json!({"target":{"kind":"recent","id":""},"nativeSidebar":1,"nativeKey":"00112233445566778899aabbccddeeff",
        "hub":"h","generation":"g","epoch":"e","viewId":"v","revision":1,"now":1_000_000,"session":{"rid":1,"open":{},"drafts":{}}}) }
    fn event(st: &mut State,e: Value) { assert!(st.apply(&e)); }
    /// One repository on two machines is one sidebar project: the linked machine's threads sit in this hub's project, each row names its
    /// machine; a linked project of another repository keeps its own row; the Active view names this hub's project for both.
    #[test] fn one_repository_on_two_machines_is_one_project() {
        let d=std::env::temp_dir().join(format!("bp-unify-{}",std::process::id())); let _=std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join(".git")).unwrap(); std::fs::write(d.join(".git/config"),"[remote \"origin\"]\n\turl = git@github.com:o/r.git\n").unwrap();
        let mut st=State::new(); st.lean=true;
        event(&mut st,json!({"$":"ProjectCreated","id":"p1","title":"backplane-gpui","root":d.to_string_lossy(),"at":1}));
        event(&mut st,json!({"$":"ThreadCreated","id":"t1","project":"p1","title":"Mine","at":5}));
        let mut f=State::new(); f.lean=true;
        for e in [json!({"$":"ProjectCreated","id":"L~p9","title":"Backplane","root":"/home/h/Documents/Backplane","at":1}), json!({"$":"ThreadCreated","id":"L~t9","project":"L~p9","title":"Theirs","at":7}),
                  json!({"$":"ThreadCreated","id":"L~t8","project":"L~p9","title":"Older","at":3}),
                  json!({"$":"ProjectCreated","id":"L~q1","title":"Other","root":"/o","at":2}), json!({"$":"ThreadCreated","id":"L~u1","project":"L~q1","title":"Else","at":4})] { event(&mut f,e); }
        let far=vec![("framework".to_string(),&f)];
        let origin=|id: &str| match id { "L~p9"=>"github.com/o/r".to_string(), "L~q1"=>"github.com/o/elsewhere".to_string(), _=>String::new() };
        let map=unified_of(&st,&far,&origin); assert_eq!(map.len(),1); assert_eq!(map["L~p9"],"p1");
        let marks: HashMap<String,Value>=[("framework".to_string(),crate::mirror::mark_of("framework",true))].into_iter().collect();
        let un=Unified{ far:&map, marks:&marks, here:"desk" };
        let v=recent_unified(&st,&far,&un,&params()).unwrap(); let ps=v["shown"]["projects"].as_array().unwrap();
        let titles: Vec<(&str,&str)>=ps.iter().map(|p|(p["title"].as_str().unwrap(),p["machine"].as_str().unwrap())).collect();
        assert_eq!(titles,[("backplane-gpui",""),("Other","framework")],"{ps:?}");
        let rows: Vec<(&str,Option<&str>)>=ps[0]["threads"].as_array().unwrap().iter().map(|r|(r["id"].as_str().unwrap(),r["mark"]["short"].as_str())).collect();
        assert_eq!(rows,[("h|L~t9",Some("framework")),("h|t1",None),("h|L~t8",Some("framework"))],"newest first across both machines; this machine's row has no marker");
        assert!(ps[0]["threads"][1].get("mark").is_none() && ps[0]["threads"][1].get("machine").is_none(),"this machine's row is as it was");
        assert_eq!(ps[0]["threads"][0]["mark"],crate::mirror::mark_of("framework",true),"away when that machine did not answer");
        assert_eq!(ps[0]["machines"],json!(["framework"])); assert_eq!(ps[1]["threads"][0]["mark"]["name"],"framework");
        // the project header: the merged one says where else it is, the other machine's says where it is, this machine's own says nothing
        assert_eq!((ps[0]["also"].as_str(),ps[0]["marks"][0]["short"].as_str(),ps[0].get("mark")),(Some("also on framework"),Some("framework"),None));
        assert_eq!((ps[1]["mark"]["label"].as_str(),ps[1].get("also")),(Some("on framework"),None));
        // without the map (a machine that did not say its origins) the two stay apart, as before
        let v=recent_far(&st,&far,&params()).unwrap(); assert_eq!(v["shown"]["projects"].as_array().unwrap().len(),3);
        // the Active view: the linked thread names its machine and this hub's project
        let mut a=params(); a["session"]["drafts"]["@local.view"]=json!("active");
        let v=recent_unified(&st,&far,&un,&a).unwrap(); let act=v["shown"]["active"].as_array().unwrap();
        let t9=act.iter().find(|r|r["id"]=="h|L~t9").unwrap(); assert_eq!((t9["projectId"].as_str(),t9["project"].as_str(),t9["machine"].as_str()),(Some("h|p1"),Some("backplane-gpui"),Some("framework")));
        // the thread header of another machine's thread has its marker with the full name; this machine's has none
        assert_eq!(header_mark(&far,&f,&un),Some(crate::mirror::mark_of("framework",true))); assert_eq!(header_mark(&far,&st,&un),None);
        assert_eq!(t9["mark"]["short"],"framework"); assert!(act.iter().find(|r|r["id"]=="h|t1").unwrap().get("mark").is_none());
        assert_eq!(act.iter().find(|r|r["id"]=="h|L~u1").unwrap()["projectId"],"h|L~q1");
        // one order over machines: the Active rows of both stand by their last move, not this machine's first
        assert_eq!(act.iter().map(|r|r["id"].as_str().unwrap()).collect::<Vec<_>>(),["h|L~t9","h|t1","h|L~u1","h|L~t8"]);
        // the switcher: every machine, this one by its name, the linked one (away here)
        let v=recent_unified(&st,&far,&un,&params()).unwrap();
        assert_eq!(v["shown"]["machines"].as_array().unwrap().iter().map(|c|(c["value"].as_str().unwrap(),c["label"].as_str().unwrap(),c["on"]==true,c["away"]==true)).collect::<Vec<_>>(),
            [("","All",true,false),("@here","desk",false,false),("framework","framework",false,true)]);
        assert!(recent_far(&st,&[],&params()).unwrap()["shown"].get("machines").is_none(),"no machine linked: no switcher");
        // New thread on the machine chosen, this one first; the away one says so
        assert_eq!(v["shown"]["projects"][0]["newOn"],json!([{"value":"p1","label":"New thread on desk","away":false},{"value":"L~p9","label":"New thread on framework (away)","away":true}]));
        // this machine only: its project, its own rows
        let pick=|m: &str| { let mut q=params(); q["session"]["drafts"]["@machine"]=json!(m); recent_unified(&st,&far,&un,&q).unwrap() };
        let here=pick("@here"); let hp=here["shown"]["projects"].as_array().unwrap();
        assert_eq!((hp.len(),hp[0]["threads"].as_array().unwrap().iter().map(|r|r["id"].as_str().unwrap()).collect::<Vec<_>>()),(1,vec!["h|t1"]));
        assert_eq!(here["shown"]["machines"][1]["on"],true);
        // the linked machine only: its projects as it has them (not joined to this one's), labelled
        let fw=pick("framework"); let fp=fw["shown"]["projects"].as_array().unwrap();
        assert_eq!(fp.iter().map(|p|(p["title"].as_str().unwrap(),p["machine"].as_str().unwrap())).collect::<Vec<_>>(),[("Other","framework"),("Backplane","framework")],"its own order: newest made first");
        assert_eq!(pick("gone")["shown"]["machines"][0]["on"],true,"a machine no longer linked: every machine again");
        let mut a=params(); a["session"]["drafts"]["@local.view"]=json!("active"); a["session"]["drafts"]["@machine"]=json!("@here");
        assert_eq!(recent_unified(&st,&far,&un,&a).unwrap()["shown"]["active"].as_array().unwrap().iter().map(|r|r["id"].as_str().unwrap()).collect::<Vec<_>>(),["h|t1"]);
        // the switcher's press keeps the choice
        let mut q=params(); q["action"]=json!("side-machine"); q["value"]=json!("framework");
        assert_eq!(recent_unified(&st,&far,&un,&q).unwrap()["session"]["drafts"]["@machine"],"framework");
        // a thread's link opens it, by the machine's name too
        let open=|u: &str| { let mut q=params(); q["action"]=json!("link-open"); q["value"]=json!(u); recent_unified(&st,&far,&un,&q).unwrap() };
        assert_eq!(open("backplane://thread/framework~t9")["nextTarget"],json!({"kind":"thread","id":"L~t9"}));
        assert_eq!(open("bpgpui://thread/L~t8")["nextTarget"]["id"],"L~t8"); assert_eq!(open("backplane://thread/desk~t1")["nextTarget"]["id"],"t1");
        let none=open("backplane://thread/lab~t1"); assert!(none.get("nextTarget").is_none()); assert!(none["cmds"].as_array().unwrap().iter().any(|c|c["tone"]=="note"&&c["text"]=="That thread is on a machine this hub does not mirror (yet)."),"{none}");
        let _=std::fs::remove_dir_all(&d);
    }
    /// The window opens a thread with view.open (no select action): the card's look (Buy it in the header) is asked for then, once, and again only when the turn moves.
    #[test] fn opening_a_thread_asks_for_its_card_once() {
        let mut st=State::new(); st.lean=true;
        event(&mut st,json!({"$":"ProjectCreated","id":"p1","title":"lamp","root":"/w/lamp","at":1}));
        event(&mut st,json!({"$":"ThreadCreated","id":"t1","project":"p1","title":"Lamp board","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}));
        let looks=|c: &[Value]| c.iter().filter(|c| c["text"].as_str().map_or(false,|t| t.contains("\"m\":\"fab.look\"") && t.contains("\"t1\""))).count();
        let mut session=json!({"sel":"t1","rid":0,"info":{},"drafts":{}}); let mut cmds=vec![];
        crate::session::dispatch::fab_tail(&crate::session::dispatch::StateFacts(&st),"h",&mut session,&mut cmds);
        assert_eq!(looks(&cmds),1,"the open asks for the card: {cmds:?} {session}");
        let mut again=vec![]; crate::session::dispatch::fab_tail(&crate::session::dispatch::StateFacts(&st),"h",&mut session,&mut again);
        assert_eq!(looks(&again),0,"the same place asks nothing more");
        // and the presenter runs it on a thread's view (view.open has no action): whatever it answers, its session has asked
        let mut p=params(); p["target"]=json!({"kind":"thread","id":"t1"}); p["window"]=json!({"contextMessages":[],"messageIds":[],"historyUnits":[],"partial":[],"ids":[],"anchor":{"id":"","offset":0},"older":"","olderNext":"","hasMore":false,"remaining":0,"newer":0,"total":0});
        if let Some(v)=recent(&st,&p) { assert_eq!(looks(v["cmds"].as_array().unwrap()),1,"{}",v["cmds"]); } else { eprintln!("REFUSED (see the not ported line)"); }
    }
    #[test] fn recent_reads_the_existing_policy_copy_only_after_boot() {
        use crate::state::fam::policy::{Core,Policy};
        let policy=Policy::new(); assert!(policy.with_state(|st|st.seq).is_none());
        let mut core=Core::new(); core.st.apply(&json!({"$":"ProjectCreated","id":"p","title":"P","root":"/p","at":1}));
        policy.finish_boot(core,1,1); assert_eq!(policy.with_state(|st|st.projects.len()),Some(1));
        policy.before_commit(2,vec![json!({"$":"ProjectCreated","id":"p2","title":"P2","root":"/p2","at":2})]);
        assert_eq!(policy.with_state(|st|st.projects.len()),Some(2));
        assert!(policy.commit_failed(2)); assert!(policy.with_state(|st|st.projects.len()).is_none());
    }
    #[test] fn recent_lean_navigation_fields_follow_edits_and_deletions() {
        let mut st=State::new(); st.lean=true;
        for e in [json!({"$":"ProjectCreated","id":"p","title":"P","root":"/p","at":1}),
            json!({"$":"ThreadCreated","id":"t","project":"p","title":"T","at":1}),
            json!({"$":"BotSet","id":"b","name":"Cat","thread":"t","look":42,"at":2}),
            json!({"$":"RoomSet","id":"r","name":"Room","members":"b\nother\n","at":3}),
            json!({"$":"RoomSet","id":"dm:a:b","name":"Direct","members":"b\nother","at":3}),
            json!({"$":"SettingSet","key":"ui.theme","value":"light","at":4}),
            json!({"$":"SettingSet","key":"project.icon.p","value":"cpu","at":4}),
            json!({"$":"SettingSet","key":"project.color.p","value":"pink","at":4}),
            json!({"$":"SettingSet","key":"ride.t","value":"not kept","at":4})] { event(&mut st,e); }
        let v=recent(&st,&params()).unwrap(); let s=&v["shown"];
        assert_eq!(s["theme"],"light"); assert_eq!(s["projects"][0]["icon"],"cpu");
        assert_eq!((s["projects"][0]["color"].as_str(),s["projects"][0]["css"].as_str()),(Some("pink"),Some("#e06fb4")));
        assert!(s["projects"][0].get("look").is_none(),"the editor is shut");
        let mut open=params(); open["session"]["drafts"]["@look"]=json!("p");
        let e=&recent(&st,&open).unwrap()["shown"]["projects"][0]["look"];
        assert_eq!(e["colors"].as_array().unwrap().iter().find(|c|c["on"]==true).unwrap()["label"],"pink"); assert_eq!(e["icons"].as_array().unwrap().iter().find(|c|c["on"]==true).unwrap()["value"],"project.icon.p=cpu");
        assert_eq!(s["bots"][0]["id"],"h|b"); assert_eq!(s["bots"][0]["name"],"Cat"); assert_eq!(s["bots"][0]["look"],42);
        assert_eq!(s["rooms"].as_array().unwrap().len(),1); assert_eq!(s["rooms"][0]["members"],2);
        assert!(!st.settings.contains_key("ride.t") && st.entries.is_empty());
        event(&mut st,json!({"$":"BotSet","id":"b","name":"Edited","thread":"","look":8,"at":5}));
        assert_eq!(st.bots_nav["b"].thread,"t"); assert_eq!(st.bots_nav["b"].created,2);
        event(&mut st,json!({"$":"BotDeleted","id":"b","at":6})); event(&mut st,json!({"$":"RoomDeleted","id":"r","at":6}));
        let v=recent(&st,&params()).unwrap(); assert_eq!(v["shown"]["bots"],json!([])); assert_eq!(v["shown"]["rooms"],json!([]));
    }
    /// A lean state from log lines (JSON text).
    fn state(lines: &[&str]) -> State { let mut st=State::new(); st.lean=true; for l in lines { event(&mut st,serde_json::from_str(l).unwrap()); } st }
    fn two_projects() -> State {
        let mut st=State::new(); st.lean=true;
        for e in [json!({"$":"ProjectCreated","id":"pa","title":"Alpha board","root":"/a","at":1}), json!({"$":"ProjectCreated","id":"pb","title":"Beta","root":"/b","at":2}),
            json!({"$":"ThreadCreated","id":"ta","project":"pa","title":"A1","at":3}), json!({"$":"ThreadCreated","id":"tb","project":"pb","title":"B1","at":4})] { event(&mut st,e); }
        st
    }
    fn names(v: &Value) -> Vec<String> { v["shown"]["projects"].as_array().unwrap().iter().map(|p|p["title"].as_str().unwrap().to_string()).collect() }
    #[test] fn the_search_keeps_the_projects_whose_name_fuzzy_matches_and_the_one_holding_the_open_thread() {
        let st=two_projects();
        let mut p=params(); p["session"]["drafts"]=json!({"@f.pq":"abd"});
        assert_eq!(names(&recent(&st,&p).unwrap()),["Alpha board"],"a, b, d in order in 'alpha board'");
        p["session"]["drafts"]=json!({"@f.pq":"zzz"}); assert!(names(&recent(&st,&p).unwrap()).is_empty());
        p["session"]["drafts"]=json!({"@f.pq":""}); assert_eq!(names(&recent(&st,&p).unwrap()).len(),2);
        assert!(fuzzy_hit("Alpha board","ALB") && !fuzzy_hit("Beta","ab"));
    }
    #[test] fn the_search_finds_threads_by_title_and_narrows_a_project_to_them() {
        let st=state(&[r#"{"$":"ProjectCreated","id":"pa","title":"Alpha","root":"/a","at":1}"#,r#"{"$":"ProjectCreated","id":"pb","title":"Beta","root":"/b","at":1}"#,
            r#"{"$":"ThreadCreated","id":"ta","project":"pa","title":"Route the rail","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}"#,
            r#"{"$":"ThreadCreated","id":"tb","project":"pa","title":"Other work","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":3}"#,
            r#"{"$":"ThreadCreated","id":"tc","project":"pb","title":"Quiet one","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":4}"#]);
        let titles=|v:&Value| v["shown"]["projects"].as_array().unwrap().iter().flat_map(|p|p["threads"].as_array().unwrap().iter().map(|t|t["title"].as_str().unwrap().to_string()).collect::<Vec<_>>()).collect::<Vec<_>>();
        let mut p=params(); p["session"]["drafts"]=json!({"@f.pq":"route"});
        let v=recent(&st,&p).unwrap(); assert_eq!(names(&v),["Alpha"],"a thread's title finds its project with no thread open"); assert_eq!(titles(&v),["Route the rail"],"and only that thread shows");
        p["session"]["drafts"]=json!({"@f.pq":"alpha"}); assert_eq!(titles(&recent(&st,&p).unwrap()).len(),2,"a project found by its name shows as it is");
        p["session"]["drafts"]=json!({"@f.pq":"nothing like it"}); assert!(names(&recent(&st,&p).unwrap()).is_empty());
        let mut o=params(); o["session"]["drafts"]=json!({"@f.pq":"route"}); o["action"]=json!("proj-go"); assert_eq!(recent(&st,&o).unwrap()["nextTarget"]["id"],"ta","Enter opens the thread found");
    }
    #[test] fn scratch_chats_are_conversation_history_found_by_search_and_never_a_place_to_start() {
        let chat=|id:&str,title:&str,at:u32| format!(r#"{{"$":"ThreadCreated","id":"{id}","project":"scratch","title":"{title}","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":{at}}}"#);
        let settled=r#"{"$":"ThreadSettled","id":"s1","manual":true,"at":9}"#;
        let (a,b)=(chat("s1","zebra scratch chat",2),chat("s2","live chat",3));
        let st=state(&[r#"{"$":"ProjectCreated","id":"scratch","title":"Scratch","root":"/s","at":1}"#,&a,&b,settled]);
        let v=recent(&st,&params()).unwrap();
        assert!(names(&v).is_empty(),"no Scratch project among the projects: nothing to start a thread in");
        let rows=|v:&Value| v["shown"]["legacy"].as_array().unwrap().iter().map(|t|t["title"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert_eq!(rows(&v),["live chat"],"a chat still going keeps its row"); assert_eq!(v["shown"]["conversationHistory"]["count"],2,"both chats: the live one too"); assert_eq!(v["shown"]["conversationHistory"]["jump"][0]["title"],"zebra scratch chat");
        let mut p=params(); p["session"]["drafts"]=json!({"@f.pq":"zebra"}); assert_eq!(rows(&recent(&st,&p).unwrap()),["zebra scratch chat"],"a search opens the group by itself and filters it");
        let mut p=params(); p["session"]["open"]=json!({"conversation-history":true}); assert_eq!(rows(&recent(&st,&p).unwrap()).len(),2,"the toggle shows the rest");
        let mut p=params(); p["action"]=json!("flag"); p["value"]=json!("conversation-history"); assert_eq!(recent(&st,&p).unwrap()["session"]["open"]["conversation-history"],true);
        let mut o=params(); o["session"]["drafts"]=json!({"@f.pq":"zebra"}); o["action"]=json!("proj-go"); assert_eq!(recent(&st,&o).unwrap()["nextTarget"]["id"],"s1","Enter opens the chat found");
        let only=state(&[r#"{"$":"ProjectCreated","id":"scratch","title":"Scratch","root":"/s","at":1}"#,&a,settled]);
        let mut p=params(); p["target"]=json!({"kind":"thread","id":"s1"}); p["window"]=json!({"contextMessages":[],"messageIds":[],"historyUnits":[],"partial":[],"ids":[],"anchor":{"id":"","offset":0},"older":"","olderN":0});
        if let Some(v)=recent(&only,&p) { assert_eq!(v["shown"]["conversationHistory"]["count"],1,"the open chat keeps the group's row: {}",v["shown"]); assert_eq!(rows(&v),["zebra scratch chat"]); }
    }
    #[test] fn a_projects_chevron_folds_it_and_a_rename_says_its_kind() {
        let st=two_projects();
        let mut p=params(); p["action"]=json!("flag"); p["value"]=json!("h|project-closed:pa");
        let v=recent(&st,&p).unwrap(); assert_eq!(v["session"]["open"]["project-closed:pa"],true);
        let closed=|v:&Value,t:&str| v["shown"]["projects"].as_array().unwrap().iter().find(|x|x["title"]==t).unwrap()["closed"].clone();
        assert_eq!(closed(&v,"Alpha board"),true); assert_eq!(closed(&v,"Beta"),false);
        let mut again=params(); again["session"]=v["session"].clone(); again["action"]=json!("flag"); again["value"]=json!("project-closed:pa");
        assert_eq!(recent(&st,&again).unwrap()["session"]["open"]["project-closed:pa"],false,"the same press unfolds it");
        let mut bad=params(); bad["action"]=json!("flag"); bad["value"]=json!("project-closed:nope"); assert!(recent(&st,&bad).is_none(),"no such project");
        let mut r=params(); r["session"]["drafts"]=json!({"@ren":"pb","@ren.kind":"project","@f.title":"Beta"});
        let v=recent(&st,&r).unwrap(); assert_eq!((v["shown"]["renaming"]["kind"].as_str(),v["shown"]["renaming"]["title"].as_str()),(Some("project"),Some("Rename project")));
        r["session"]["drafts"]=json!({"@ren":"ta","@ren.kind":"thread","@f.title":"A1"}); assert_eq!(recent(&st,&r).unwrap()["shown"]["renaming"]["kind"],"thread");
    }
    #[test] fn typing_in_the_search_sets_the_query_and_enter_opens_the_first_projects_top_thread() {
        let st=two_projects();
        let mut p=params(); p["action"]=json!("proj-find-q"); p["value"]=json!("bet");
        let v=recent(&st,&p).unwrap(); assert_eq!(v["session"]["drafts"]["@f.pq"],"bet"); assert_eq!(names(&v),["Beta"]); assert_eq!(v["shown"]["search"]["query"],"bet");
        let mut p=params(); p["session"]["drafts"]=json!({"@f.pq":"bet"}); p["action"]=json!("proj-go");
        let v=recent(&st,&p).unwrap(); assert_eq!(v["nextTarget"]["id"],"tb"); assert_eq!(v["session"]["drafts"]["@f.pq"],"");
        assert!(recent(&st,&params()).unwrap()["shown"].get("search").is_none(),"a session that never used the box says nothing of it");
    }
    #[test] fn old_quiet_projects_fold_under_an_older_row_that_opens_them() {
        let mut st=State::new(); st.lean=true;
        for e in [json!({"$":"ProjectCreated","id":"po","title":"Old one","root":"/o","at":1}), json!({"$":"ProjectCreated","id":"pn","title":"New one","root":"/n","at":8_600_000})] { event(&mut st,e); }
        let mut p=params(); p["now"]=json!(8_700_000_000u64);
        let v=recent(&st,&p).unwrap(); assert_eq!(names(&v),["New one"]); assert_eq!(v["shown"]["olderProjects"],json!({"count":1,"open":false}));
        let mut p2=p.clone(); p2["action"]=json!("side-older"); let v2=recent(&st,&p2).unwrap();
        assert_eq!(v2["session"]["open"]["old-projects"],true);
        let mut p3=p.clone(); p3["session"]["open"]=json!({"old-projects":true}); let v3=recent(&st,&p3).unwrap();
        assert_eq!(names(&v3).len(),2); assert_eq!(v3["shown"]["olderProjects"],json!({"count":1,"open":true}));
    }
    fn active_titles(v: &Value) -> Vec<String> { v["shown"]["active"].as_array().unwrap().iter().map(|r|r["title"].as_str().unwrap().to_string()).collect() }
    #[test] fn the_active_view_lists_live_threads_by_urgency_with_the_settled_folded_and_no_project_rows() {
        let mut st=two_projects();
        for e in [json!({"$":"ThreadCreated","id":"tc","project":"pa","title":"Failing one","at":5}), json!({"$":"TurnChanged","thread":"tc","state":"failed","at":7}),
            json!({"$":"ThreadCreated","id":"td","project":"pb","title":"Pinned old","at":2}), json!({"$":"ThreadCreated","id":"te","project":"pb","title":"Settled one","at":2})] { st.apply(&e); }
        event(&mut st,json!({"$":"SettingSet","key":"sidebar.view","value":"active","at":9}));
        let v=recent(&st,&params()).unwrap();
        assert!(v["shown"]["projects"].as_array().unwrap().is_empty(), "the Active view has no project rows");
        let titles=active_titles(&v); assert!(titles.contains(&"A1".to_string()) && titles.contains(&"B1".to_string()), "{titles:?}");
        assert_eq!(v["shown"]["active"][0]["project"].as_str().is_some(), true, "each row names its project");
        // a thread whose last turn failed (needs the person) leads the idle ones
        assert_eq!(titles[0],"Failing one","{titles:?}");
        // a search keeps the threads whose title or project matches
        let mut p=params(); p["session"]["drafts"]=json!({"@f.pq":"beta"}); assert!(active_titles(&recent(&st,&p).unwrap()).iter().all(|t|t.starts_with('B') || t=="Pinned old" || t=="Settled one"));
        // asked for in this session alone works too (the phones' local view)
        let mut p=params(); p["session"]["drafts"]=json!({"@local.view":"active"}); assert!(recent(&st,&p).is_some());
    }
    #[test] fn a_removed_projects_open_thread_keeps_its_row_until_another_is_chosen() {
        let mut st=two_projects(); event(&mut st,json!({"$":"ProjectRemoved","id":"pa","at":9}));
        assert_eq!(names(&recent(&st,&params()).unwrap()),["Beta"],"nothing open: the removed project is gone");
        let mut p=params(); p["target"]=json!({"kind":"thread","id":"ta"}); p["window"]=json!({"contextMessages":[],"messageIds":[],"historyUnits":[],"partial":[],"ids":[],"anchor":{"id":"","offset":0},"older":"","olderNext":"","hasMore":false,"remaining":0,"newer":0,"total":0});
        if let Some(v)=recent(&st,&p) { assert!(names(&v).contains(&"Alpha board".to_string()),"its thread is open: the row stays"); }
    }
    #[test] fn the_projects_list_is_the_default_and_the_settled_fold_toggles() {
        let st=two_projects(); let v=recent(&st,&params()).unwrap(); assert_eq!(names(&v).len(),2); assert!(v["shown"]["activeSettled"].is_null());
        let mut p=params(); p["session"]["drafts"]=json!({"@local.view":"active"}); p["action"]=json!("side-settled");
        let v=recent(&st,&p).unwrap(); assert_eq!(v["session"]["open"]["active-settled"],true);
    }
    #[test] fn recent_projects_rows_and_page_actions_are_bounded_and_read_only() {
        let mut st=State::new(); st.lean=true;
        for i in 0..12 {
            event(&mut st,json!({"$":"ProjectCreated","id":format!("p{i}"),"title":"P","root":"/p","at":i+1}));
            for j in 0..80 { event(&mut st,json!({"$":"ThreadCreated","id":format!("t{i}-{j}"),"project":format!("p{i}"),"title":"T","at":j+1})); }
        }
        let seq=st.seq; let p=params(); let out=recent(&st,&p).unwrap(); let projects=out["shown"]["projects"].as_array().unwrap();
        assert_eq!(projects.len(),8); assert_eq!(projects.iter().map(|p|p["threads"].as_array().unwrap().len()).sum::<usize>(),48);
        for p in projects { assert_eq!(p["threadsPage"]["total"],80); assert_eq!(p["threadsPage"]["limit"],6); }
        let mut p=p.clone(); p["local"]=out["local"].clone(); p["action"]=json!("native-sidebar-page");
        p["value"]=out["shown"]["sidebarPages"][0]["projects"]["next"]["value"].clone();
        let out=recent(&st,&p).unwrap(); assert_eq!(out["shown"]["sidebarPages"][0]["projects"]["offset"],8);
        assert_eq!(out["shown"]["projects"].as_array().unwrap().len(),4); assert_eq!(out["cmds"],json!([])); assert_eq!(seq,st.seq);
        p["value"]=json!("forged"); assert_eq!(recent(&st,&p).unwrap()["refused"],true);
    }
}

#[cfg(test)]
mod dispatcher_tests {
    use super::*;
    #[test] fn recent_dispatches_selection_without_drawing_or_starting_the_target() {
        let mut st=State::new(); st.lean=true;
        st.apply(&json!({"$":"ThreadCreated","id":"t","project":"p","title":"T","at":1}));
        let p=json!({"target":{"kind":"recent","id":""},"nativeSidebar":1,"hub":"h","action":"select","value":"t",
            "session":{"rid":1,"sel":"","drafts":{},"open":{},"panel":false}});
        let o=recent(&st,&p).unwrap();
        assert_eq!(o["remoteReady"],false); assert_eq!(o["nextTarget"],json!({"kind":"thread","id":"t"}));
        assert_eq!(o["session"]["sel"],"t"); assert!(o.get("shown").is_none()); assert!(o["local"].is_object());
        assert!(o["cmds"].as_array().unwrap().iter().all(|c|!text(c,"text").contains("turn.start")));
    }
}

#[cfg(test)]
mod thread_binding_tests {
    #[test] fn a_bot_chat_gets_the_models_info_like_a_thread() { for k in ["thread","bot"] { assert!(shows_thread(&json!({"target":{"kind":k,"id":"x"}})), "{k}"); } assert!(!shows_thread(&json!({"target":{"kind":"recent"}}))); }
    use super::*;
    fn fixture(lean: bool) -> (State,Value,Value) {
        let pair:Value=serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/slim/fixtures/thread-pair.json"))).unwrap();
        let mut st=State::new(); st.lean=lean;
        for line in include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/slim/fixtures/thread-log.jsonl")).lines() { st.apply_line(line); }
        let mut p=pair["params"].clone(); p["nativeSidebar"]=json!(1); p["nativeKey"]=json!("00112233445566778899aabbccddeeff");
        p["generation"]=json!("g"); p["epoch"]=json!("e"); p["viewId"]=json!("v"); p["revision"]=json!(st.seq);
        // The fixture predates the host window seam. Its raw helper reply has earlier=0.
        // The seam itself is checked against the indexed JS-window fixtures by present::window.
        p["window"]=json!({"remaining":0});
        (st,p,pair["out"]["shown"]["thread"].clone())
    }
    #[test] fn thread_root_binds_the_complete_recorded_thread_and_window() {
        for lean in [false,true] {
            let (st,p,expected)=fixture(lean);
            let out=recent(&st,&p).expect("plain thread root is supported");
            // the Allowance chip's text (keys.rs `allowance`), the picker's other agents (`pickerAgents`) and the masked box and the composer's commands (`secretBox`, `commands`; `btw` is the helper's own key) are Backplane's own: the recorded helper reply has no such keys
            let bare=|mut t:Value|{ for k in ["allowance","combo","pickerAgents","secretBox","commands","project","subView"] { t.as_object_mut().unwrap().remove(k); } t["picker"]=crate::present::keys::as_helper(&t["picker"]); t };
            assert_eq!(bare(out["shown"]["thread"].clone()),expected); assert_eq!(bare(out["overlay"]["j"]["thread"].clone()),expected);
            assert_eq!(out["session"]["sel"],p["target"]["id"]); assert_eq!(out["window"],p["window"]);
            assert_eq!(out["shown"]["nativeSidebar"],1);
            // approved difference from the recording: opening a thread asks for its card (fab.look), as the select action does, so Buy it can show in the window
            let th=p["target"]["id"].as_str().unwrap();
            assert_eq!(out["cmds"],json!([{"type":"send","hub":"h","text":format!("{{\"id\":1,\"m\":\"fab.look\",\"p\":{{\"thread\":\"{th}\"}}}}")}]));
        }
    }
    #[test] fn thread_root_keeps_drafts_and_refuses_missing_indexed_inputs_and_unported_config() {
        let (mut st,p,_)=fixture(true); let id=text(&p["target"],"id").to_string();
        for key in ["page","window"] { let mut q=p.clone(); q.as_object_mut().unwrap().remove(key); assert!(recent(&st,&q).is_none(),"{key}"); }
        let mut q=p.clone(); q["session"]["drafts"][&id]=json!("typing"); assert_eq!(recent(&st,&q).unwrap()["shown"]["thread"]["draft"],"typing");
        let mut q=p.clone(); q["action"]=json!("draft"); q["value"]=json!("typing"); assert_eq!(recent(&st,&q).unwrap()["shown"]["thread"]["draft"],"typing");
        st.apply(&json!({"$":"ThreadRenamed","id":id,"title":"  ","at":1}));
        assert!(recent(&st,&p).is_none(),"a thread with no title refuses (thread.title-empty)");
    }
}

#[cfg(test)]
mod selected_thread_tests {
    use super::*;
    #[test] fn a_selected_project_and_thread_outside_the_first_page_remain_in_source_order() {
        let mut st=State::new(); st.lean=true;
        for i in 0..12 {
            st.apply(&json!({"$":"ProjectCreated","id":format!("p{i}"),"title":"P","root":"/tmp/synthetic","at":i+1}));
            for j in 0..100 { st.apply(&json!({"$":"ThreadCreated","id":format!("t{i}-{j}"),"project":format!("p{i}"),"title":"T","provider":"claude","model":"","at":j+1})); }
        }
        let p=json!({"target":{"kind":"thread","id":"t0-0"},"nativeSidebar":1,"nativeKey":"00112233445566778899aabbccddeeff",
            "hub":"h","generation":"g","epoch":"e","viewId":"v","revision":st.seq,"now":1000000,"session":{"drafts":{},"open":{}},
            "window":{"remaining":0},"page":{"thread":"t0-0","rows":[]}});
        let out=recent(&st,&p).unwrap(); let projects=out["shown"]["projects"].as_array().unwrap();
        assert_eq!(out["shown"]["sidebarPages"][0]["projects"]["selectedOutside"],true);
        assert_eq!(projects.len(),9); assert_eq!(projects.last().unwrap()["id"],"h|p0");
        let chosen=projects.last().unwrap(); assert_eq!(chosen["threadsPage"]["selectedOutside"],true);
        assert_eq!(chosen["threads"].as_array().unwrap().last().unwrap()["id"],"h|t0-0");
        assert_eq!(out["shown"]["thread"]["id"],"h|t0-0");
    }
}
