//! The GPUI recent and plain-thread root. Borrows the state policies' lean fold; never replays or clones a log.
//! Approved differences for this slice: unread shown keys `bubHomes`, `online`, `version`, `error`,
//! `note`, `sel`, `making`, `lang`, `empty`, `older`, `settled`, `view`, `active`, `legacy`, `rowMenu`,
//! `removing`, `search`, `folders`, `tour`, `starNudge`, `find`, `island`, `islands`, `uber`, `hub`,
//! `hubs`, `usageMachines`, `found`, `tabs` are omitted. They belong to other clients/screens. Added beyond the JS answer (docs/ALWAYS-ON.md section 8):
//! `sections`, `jump`, `groupsTo`, `projectPage` (present/sections.rs).
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
        let withinfo; let params=if super::settings::is_open(&params["session"]) || shows_thread(params) || shows_swarm_models(params) || setup { let mut c=params.clone(); c["hubInfo"]=_hub.info_now(); c["hubInfo"]["client"]=params["client"].clone(); if devpage { if let Some(r)=crate::net::start::routes() { c["hubInfo"]["devices"]=r.devices(); } } if setup || super::settings::page(&params["session"])=="machines" { if let Some(p)=crate::net::start::peering() { c["hubInfo"]["machines"]=p.page_data(); } } withinfo=c; &withinfo } else { params };
        // a 0.14 import of this hub: its card goes over any page (present/onboard.rs import_overlay)
        let withmig; let params=match crate::import014::last_published(&_hub.cfg.home) { Some(v) => { let mut c=params.clone(); c["importV014"]=v; withmig=c; &withmig } None => params };
        let m=_hub.mirrors();
        match policies.with_state(|st|m.shown(|far| { let j=join_full(st,far,m); FAR_ORIGINS.with(|o|*o.borrow_mut()=far.iter().flat_map(|(_,f)|f.projects.keys()).map(|id|(id.clone(),m.origin(id))).collect()); SECTION_HUB.with(|h|*h.borrow_mut()=Arc::downgrade(&_hub)); let marks: HashMap<String,Value>=far.iter().map(|(n,_)|(n.clone(),m.mark(n))).collect(); let here=m.here(); recent_unified(st,far,&Unified{ far:&j.far, marks:&marks, here:&here, rows:&j.rows },params) })) { Some(out)=>out,None=>refused("state.booting") }
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
/// A swarm's page (its add picker) and the New swarm form list the composer's models: they get the hub's models info too (present/swarmform.rs)
pub fn shows_swarm_models(p: &Value) -> bool { (text(&p["target"],"kind")=="room" && p["roomPage"]["swarm"].is_object()) || !text(&p["session"]["drafts"],"@snew").is_empty() || text(p,"action")=="swarm-new" }
/// The uberbot a state names, when it is one of its bots ("" for none)
fn uber_in(st: &State) -> String { let u=setting(st,"uber.bot"); if !u.is_empty() && st.bots_nav.contains_key(u) { u.to_string() } else { String::new() } }
/// The sidebar's bots: this hub's, then each shown linked machine's (`<link>~<id>`, with its machine's `mark`). The uberbot leads: this hub's own, or with
/// none a linked machine's (`bots::api::uber_shown`, the rule `bots.list` follows), drawn in Bub's place.
fn bots(st: &State, far: &[(String,&State)], un: &Unified, hub: &str, now: u64) -> Vec<Value> {
    let uber=crate::bots::api::uber_shown(&uber_in(st),&far.iter().map(|(_,f)|uber_in(f)).collect::<Vec<_>>());
    let mut rows=bot_rows(st,hub,now,&uber,&Value::Null);
    for (m,f) in far { rows.extend(bot_rows(f,hub,now,&uber,&un.mark(m))); }
    rows.sort_by_key(|r| !r["uber"].as_bool().unwrap_or(false)); // (stable: each machine's own order kept)
    // a home move (net/homemove.rs, the setting `home.moving`): the uberbot's own row says where it is going while it goes, and once that it did not
    if let Some((mood,note))=crate::net::homemove::moving_note(setting(st,"home.moving"),now) { if let Some(r)=rows.iter_mut().find(|r|r["uber"]==true && r["mark"].is_null()) { r["mood"]=json!(mood); r["note"]=json!(note); r["cat"]=json!(format!("{}:{mood}",r["look"])); } }
    rows
}
/// One state's bots as sidebar rows (its own only: a mirror's bots are `<link>~<id>`, never re-listed by this hub's state)
fn bot_rows(st: &State, hub: &str, now: u64, uber: &str, mark: &Value) -> Vec<Value> {
    let far=!mark.is_null();
    let mut bots: Vec<_>=st.bots_nav.iter().filter(|(id,_)|id.contains('~')==far).collect();
    bots.sort_by(|(ia,a),(ib,b)| (**ib==uber).cmp(&(**ia==uber)).then(a.created.cmp(&b.created)).then(ia.cmp(ib))); // the uberbot leads
    bots.iter().map(|(id,b)| {
        let name=if bots.iter().filter(|(_,other)|other.name.eq_ignore_ascii_case(&b.name)).count()>1 { format!("{} ({})",b.name,crate::net::far::local(id)) }else{b.name.clone()};
        let t=st.threads.get(&b.thread);
        let (mood,note)=if st.asks.values().any(|(th,open)| th==&b.thread && *open) || t.map_or(false,|t|t.pending>0) { ("wait","needs you") }
            else if t.map_or(false,|t|t.turn==crate::state::Turn::Running) || st.queue.get(&b.thread).map_or(false,|q|!q.is_empty()) {
                if now<=b.talked.saturating_add(60) { ("talk","talking") }else{("work","working")}
            } else if t.map_or(false,|t|t.turn==crate::state::Turn::Failed) { ("error","failed") }
            else if now>t.map_or(0,|t|t.activity).saturating_add(900) { ("sleep","asleep") }else{("idle","ready")};
        json!({"id":format!("{hub}|{id}"),"name":name,"title":name,"act":if b.thread.is_empty(){"remote"}else{"bot"},
            "look":b.look,"mood":mood,"note":note,"uber":!uber.is_empty()&&**id==uber,"sel":false,"remote":false,"peer":"","cat":format!("{}:{mood}",b.look),"key":id,"link":"",
            "nav":if b.thread.is_empty(){format!("@remote:{id}")}else{format!("{hub}|{}",b.thread)},"mark":mark})
    }).collect()
}
/// The sidebar's rooms (permanent ones only: a bots' exchange, `RoomNav.ephemeral`, is never listed): this hub's, then each shown linked machine's (with its `mark`); a room kept on both machines (a room shared across them) is listed once, as this hub's.
fn rooms(st: &State, far: &[(String,&State)], un: &Unified, hub: &str) -> Vec<Value> {
    let mut rows=room_rows(st,hub,&Value::Null,&|_|false);
    for (m,f) in far { rows.extend(room_rows(f,hub,&un.mark(m),&|id| st.rooms_nav.contains_key(&crate::net::far::local(id)))); }
    rows
}
fn room_rows(st: &State, hub: &str, mark: &Value, here: &dyn Fn(&str)->bool) -> Vec<Value> {
    let far=!mark.is_null();
    let mut rooms: Vec<_>=st.rooms_nav.iter().filter(|(id,r)|!r.ephemeral && !r.swarm() && !crate::net::far::local(id).starts_with("dm:") && id.contains('~')==far && !here(id)).collect(); rooms.sort_by_key(|(id,_)|*id);
    rooms.iter().map(|(id,r)| {
        let name=if rooms.iter().filter(|(_,other)|other.name.eq_ignore_ascii_case(&r.name)).count()>1 { format!("{} · {}",r.name,crate::net::far::local(id)) }else{r.name.clone()};
        json!({"id":format!("{hub}|{id}"),"name":name,"members":r.members,"sel":false,"nav":format!("@room:{id}"),"mark":mark})
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
pub fn recent_far(st: &State,far: &[(String,&State)],p: &Value) -> Option<Value> { let marks=far.iter().map(|(n,_)|(n.clone(),crate::mirror::mark_of(n,false))).collect(); recent_unified(st,far,&Unified{ far:&HashMap::new(), marks:&marks, here:"", rows:&HashMap::new() },p) }
/// One repository on two machines is one project: `far` maps a linked machine's project (`<link>~<id>`) to this hub's project of the
/// same origin (`crate::mirror::unify`); `marks` is each shown machine's marker by name (`crate::mirror::mark_of`), on every row of its threads.
/// `here` is this machine's name, for the machine switcher ("" when it is not known yet).
/// `rows` is per row owner (`crate::present::merged::Join::rows`): its key, name, machine chips and the copies only the registry knows.
pub struct Unified<'a> { pub far: &'a HashMap<String,String>, pub marks: &'a HashMap<String,Value>, pub here: &'a str, pub rows: &'a HashMap<String,Value> }
impl Unified<'_> { fn mark(&self,machine: &str) -> Value { self.marks.get(machine).cloned().unwrap_or_else(||crate::mirror::mark_of(machine,false)) } }
/// The unified map for the sidebar: every live copy of a project (one key: its origin, a hand link, else machine/id; `present::merged`) that
/// does not draw the row -> the copy that does (this hub's, else the first shown machine's). Two linked machines' copies join each other too,
/// on a hub that has none (the home, boat-hub).
pub fn unified_of(st: &State,far: &[(String,&State)],origin: &dyn Fn(&str)->String) -> HashMap<String,String> {
    let list=super::merged::merged_projects(st,far,&|_|false,origin,"",&[]);
    super::merged::join_of(&list,&far.iter().map(|(n,_)|n.clone()).collect::<Vec<_>>()).far
}
/// The join over this hub's mirrors and the registry: what the sidebar and data/1 draw from.
pub fn join_full(st: &State,far: &[(String,&State)],m: &crate::mirror::Mirrors) -> super::merged::Join {
    let reg=crate::net::registry::cached(); let here=m.here();
    let mut list=super::merged::merged_projects(st,far,&|n|m.mark(n)["away"]==true,&|id|m.origin(id),&here,&reg);
    for p in list.iter_mut() { crate::mirror::live_chips(m,&mut p.machines) } // (a linked machine whose history has not come yet is on, not off)
    let mut j=super::merged::join_of(&list,&far.iter().map(|(n,_)|n.clone()).collect::<Vec<_>>());
    // a project every machine of which is off: a row of its own, drawn from the registry (`@off:<key>`)
    for p in super::merged::all_off(&list) { let mut r=super::merged::row_json(p); r["off"]=json!({"copies":p.ids.iter().map(|(m,i)|json!({"machine":m,"id":i})).collect::<Vec<_>>(),"threads":p.threads.iter().map(|t|json!({"machine":t.machine,"id":t.id,"title":t.title,"status":t.status,"activity":t.activity,"pinned":t.pinned})).collect::<Vec<_>>()}); r["machineChips"]=r["machines"].take(); j.rows.insert(format!("@off:{}",p.key),r); }
    j
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
/// A row drawn from a linked machine's copy (none here) that other machines share: a new thread on each of them, the drawing one first.
pub fn new_on_far<'a>(copies: impl Iterator<Item=(&'a str,&'a str)>,un: &Unified) -> Value {
    let mut seen=HashSet::new(); let mut v=vec![];
    for (m,fid) in copies { if !seen.insert(m) { continue } let mk=un.mark(m); let away=mk["away"].as_bool().unwrap_or(false);
        v.push(json!({"value":fid,"label":format!("New thread on {}{}",text(&mk,"short"),if away {" (away)"} else {""}),"away":away})); }
    json!(v)
}
/// A machine only the registry knows (it is off): its chip as `merged` makes them.
fn off_mark(row: Option<&Value>,machine: &str) -> Value {
    row.and_then(|r|r["machineChips"].as_array()).and_then(|a|a.iter().find(|c|text(c,"name")==machine).cloned()).unwrap_or_else(||{ let mut m=crate::mirror::mark_of(machine,true); m["state"]=json!("off"); m["dim"]=json!(true); m })
}
/// A registry thread (its machine is off) as a sidebar row: dimmed, not opened (`off`), its machine's chip as its mark.
fn off_thread(t: &Value,row: Option<&Value>,hub: &str,now: u64) -> Value {
    let m=text(t,"machine"); let mk=off_mark(row,m);
    json!({"depth":0,"expanded":false,"children":0,"id":format!("{hub}|{}",text(t,"id")),"title":text(t,"title"),"state":"idle","ago":crate::sidebar::ago(now,t["activity"].as_u64().unwrap_or(0)),
        "agents":"","pinned":t["pinned"],"status":text(t,"status"),"want":"","canChild":false,"lead":[],"trail":[],"off":true,"machine":m,"mark":mk})
}
/// The machine picker of a merged row (D6), on its `newOn` (the + and the composer offer these; the first is where it runs by default):
/// this machine's copy first, else the most recently active machine's; an away machine's entry says "Wait for <machine>" (a held launch: the far
/// outbox delivers it when that machine answers); "Clone it here" when this machine has no copy and the project has a remote; "A Boat (fresh
/// clone)" when it has a remote, disabled until a Boat account is connected (`net::registry::boat_ready`). `runsOn` says where: "runs on desktop".
pub fn picker(project: &mut Value,row: &Value,here: &str) { picker_on(project,row,here,"") }
/// `picker` with the person's choice in this window (`@runon.<project>`, the `run-on` action): that entry first and `on`, and `runsOn` names it.
pub fn picker_on(project: &mut Value,row: &Value,here: &str,chosen: &str) {
    let default=if chosen.is_empty() { text(&row["default"],"id").to_string() } else { chosen.to_string() };
    let mut on: Vec<Value>=project["newOn"].as_array().cloned().unwrap_or_default();
    if on.is_empty() && !default.is_empty() { on.push(json!({"value":default,"label":"New thread","away":false})); }
    // the machine picked is no longer listed (it went off and its copy left the live rows): it stays the choice, as one to wait for; never
    // another machine in its place (the line says it is away, the person picks another or waits)
    if !chosen.is_empty() && !on.iter().any(|o| text(o,"value")==chosen) {
        let dim: Vec<&str>=row["machineChips"].as_array().into_iter().flatten().filter(|c|c["dim"]==true).filter_map(|c|c["short"].as_str()).collect();
        let name=if dim.len()==1 { dim[0].to_string() } else { "that machine".to_string() };
        on.push(json!({"value":chosen,"label":format!("New thread on {name} (away)"),"away":true}));
    }
    on.sort_by_key(|o| text(o,"value")!=default);
    for o in on.iter_mut() { if o["away"]==true { let l=text(o,"label").trim_start_matches("New thread on ").trim_end_matches(" (away)").to_string(); o["label"]=json!(format!("Wait for {l}")); o["kind"]=json!("wait"); } else { o["kind"]=json!("machine"); } }
    let here_has=row["machineChips"].as_array().map_or(false,|c|c.iter().any(|c|c["here"]==true));
    let origin=text(row,"origin");
    if !origin.is_empty() && !here_has { on.push(json!({"value":origin,"label":"Clone it here","away":false,"kind":"clone","action":"proj-clone"})); }
    if !origin.is_empty() { on.push(json!({"value":origin,"label":"A Boat (fresh clone)","away":false,"kind":"boat","action":"proj-boat","disabled":!crate::net::registry::boat_ready()})); }
    let first=on.first().cloned().unwrap_or(Value::Null);
    if let Some(f)=on.first_mut() { if matches!(f["kind"].as_str(),Some("machine")|Some("wait")) { f["on"]=json!(true); } }
    // the machine the first entry is on: a linked copy's id is `<link>~<id>`; its chip says which machine that is
    let first_id=text(&first,"value");
    let waiting=text(&first,"label").strip_prefix("Wait for ").map(str::to_string);
    let m=if let Some(w)=waiting.filter(|_|first_id.contains('~')) { w } else if !first_id.contains('~') { if here.is_empty() { "this machine".to_string() } else { crate::mirror::short_name(here) } } else {
        let ids=row["ids"].as_array().cloned().unwrap_or_default();
        let mach=ids.iter().find(|i|text(i,"id")==first_id).map(|i|text(i,"machine").to_string()).unwrap_or_else(||text(&row["default"],"machine").to_string());
        row["machineChips"].as_array().and_then(|c|c.iter().find(|c|text(c,"name")==mach)).map(|c|text(c,"short").to_string()).unwrap_or_else(||crate::mirror::short_name(&mach)) };
    let _=here_has;
    // the machine picked is away: the line says so (Wait for it, or pick another); it never falls back to another machine by itself
    let away=first["away"]==true;
    project["runsOn"]=json!(if m.is_empty() { String::new() } else if away { format!("{m} is away") } else { format!("runs on {m}") });
    project["newOn"]=json!(on);
}
/// Link with… on the menu of this machine's project that has no remote (`key` is `<machine>/<id>`, or a hand link's): the other machines'
/// projects with no remote, by name and machine (`proj-link` `<id>|<their key>`); a linked one offers Unlink (`<id>|`).
pub fn link_with(owner: &str,rows: &HashMap<String,Value>) -> Value {
    let Some(me)=rows.get(owner) else { return json!([]) };
    if !text(me,"origin").is_empty() { return json!([]) }
    let mine=text(me,"key");
    let linked=me["machineChips"].as_array().map_or(0,|c|c.len())>1;
    let mut v: Vec<Value>=rows.iter().filter(|(o,r)|o.as_str()!=owner && text(r,"origin").is_empty() && text(r,"key")!=mine && !o.starts_with("@off:") && !r["machineChips"].as_array().map_or(false,|c|c.iter().any(|c|c["here"]==true)))
        .map(|(_,r)|{ let m=r["machineChips"][0]["short"].as_str().unwrap_or(""); json!({"label":format!("Link with {} on {m}",text(r,"name")),"value":format!("{owner}|{}",text(r,"key"))}) }).collect();
    v.sort_by(|a,b|text(a,"label").cmp(text(b,"label")));
    if linked { v.insert(0,json!({"label":"Unlink","value":format!("{owner}|")})); }
    json!(v)
}
/// The merged row's own fields on a sidebar project: `key`, `machineChips` (every machine with a copy: here/on/away/off, dimmed when away or
/// off, a `note` "desktop is off"), and the threads of its machines that are off, from the registry, after the live ones.
fn merged_fields(project: &mut Value,row: Option<&Value>,hub: &str,now: u64,here: &str,drafts: &Value) {
    let Some(r)=row else { return };
    let own=text(project,"id").rsplit('|').next().unwrap_or("").to_string();
    project["key"]=r["key"].clone(); project["machineChips"]=r["machineChips"].clone(); picker_on(project,r,here,text(drafts,&format!("@runon.{own}")));
    let off: Vec<Value>=r["off"]["threads"].as_array().into_iter().flatten().map(|t|off_thread(t,row,hub,now)).collect();
    if !off.is_empty() { if let Some(a)=project["threads"].as_array_mut() { a.extend(off); } }
}
/// A project all of whose machines are off: its row from the registry (`@off:<key>`), threads dimmed, nothing to start there.
fn off_project(id: &str,r: &Value,hub: &str,now: u64) -> Value {
    let threads: Vec<Value>=r["off"]["threads"].as_array().into_iter().flatten().map(|t|off_thread(t,Some(r),hub,now)).collect();
    let marks: Vec<Value>=r["machineChips"].as_array().cloned().unwrap_or_default();
    json!({"machine":"","quiet":false,"id":format!("{hub}|{id}"),"title":text(r,"name"),"root":text(r,"folder"),"icon":r["icon"],"color":r["color"],"css":r["css"],"open":false,
        "snoozedShelf":"Snoozed","shelf":"Settled 0","settledCount":0,"value":"","archOpen":false,"archivedCount":0,"threads":threads,"snoozed":[],"settled":[],"archived":[],
        "key":r["key"],"machineChips":marks,"marks":marks,"also":crate::sidebar::also_label(&marks).replacen("also on","on",1),"off":true,"newOn":[]})
}
/// The switcher's pick over merged rows: "@here" keeps the rows this machine has a copy of and their rows of this machine (no mark); a machine's
/// name keeps the rows that machine has and its rows. A row with no merged fields (no join yet) is judged by its own machine.
pub fn pick_filter(projects: Vec<Value>,pick: &str) -> Vec<Value> {
    let here=pick=="@here";
    let mine=|r: &Value| if here { r.get("mark").map_or(true,|m|m.is_null()) } else { r["mark"]["name"]==json!(pick) };
    projects.into_iter().filter_map(|mut p| {
        let has=match p["machineChips"].as_array() { Some(c) => c.iter().any(|c| if here { c["here"]==json!(true) } else { c["name"]==json!(pick) }), None => if here { text(&p,"machine").is_empty() } else { text(&p,"machine")==pick || p["machines"].as_array().map_or(false,|a|a.iter().any(|m|m==&json!(pick))) } };
        if !has { return None }
        for k in ["threads","snoozed","settled","archived"] { if let Some(a)=p[k].as_array_mut() { a.retain(|r|mine(r)); } }
        let n=p["settled"].as_array().map_or(0,|a|a.iter().filter(|r|r["depth"].as_u64().unwrap_or(0)==0).count());
        if p["open"]==json!(true) { p["settledCount"]=json!(n); p["shelf"]=json!(format!("Settled {n}")); }
        Some(p)
    }).collect()
}
/// The merged project rows the sections are drawn from (present/merged.rs `merged_projects`, with the registry the home sent last). A linked
/// machine's project takes its origin from `FAR_ORIGINS` (set by the bound screen from the mirrors for the request), else from the project here
/// it is unified with (`un.far`).
/// The chips follow `mirror::live_chips` (a machine known only from the registry that is linked and answering is on), with the hub's mirrors
/// the bound screen set for the request (`SECTION_HUB`); none in a test or the oracle.
pub fn merged_here<'a>(st: &'a State,far: &[(String,&'a State)],un: &Unified) -> Vec<super::merged::MergedProject<'a>> {
    match SECTION_HUB.with(|h|h.borrow().upgrade()) { Some(hub) => merged_with(st,far,un,Some(hub.mirrors()),&crate::net::registry::cached()), None => merged_with(st,far,un,None,&crate::net::registry::cached()) }
}
/// `merged_here` with the mirrors the chips are judged by (None: as `merged_projects` made them).
pub fn merged_with<'a>(st: &'a State,far: &[(String,&'a State)],un: &Unified,m: Option<&crate::mirror::Mirrors>,reg: &[super::merged::RegProject]) -> Vec<super::merged::MergedProject<'a>> {
    let origin=|id: &str| FAR_ORIGINS.with(|o|o.borrow().get(id).cloned()).filter(|o|!o.is_empty())
        .or_else(||un.far.get(id).and_then(|l|st.projects.get(l)).map(|p|crate::mirror::origin_of_root(&p.root))).unwrap_or_default();
    let mut list=super::merged::merged_projects(st,far,&|n|un.mark(n)["away"]==json!(true),&origin,un.here,reg);
    if let Some(m)=m { for p in list.iter_mut() { crate::mirror::live_chips(m,&mut p.machines) } }
    list
}
thread_local! {
    /// the hub the bound screen presents for, on this thread (its mirrors judge the chips: `merged_here`)
    static SECTION_HUB: std::cell::RefCell<Weak<Hub>> = std::cell::RefCell::new(Weak::new());
    /// a linked machine's project id -> its origin (`Mirrors::origin`), for the request being presented on this thread
    static FAR_ORIGINS: std::cell::RefCell<HashMap<String,String>> = Default::default();
}
/// The marker in the header of a thread drawn from `tst`: that machine's (full name), or none when it is this hub's state.
fn header_mark(far: &[(String,&State)],tst: &State,un: &Unified) -> Option<Value> { far.iter().find(|(_,f)|std::ptr::eq(*f,tst)).map(|(m,_)|un.mark(m)) }
/// `link-file`: the plan (present/filelink.rs) carried out. Another thread: the target moves to it with its viewer open on the file (as `link-far`);
/// the thread on show: presented as it is with the viewer's patch and its watch; a file of no design in no project: the system opens it.
/// What `link-file` does to the session, before it is presented: the thread it opens (and the thread on show), the session with that
/// thread's viewer on the file, and the commands (its watch); or a note, or a path for the system (a file of no design in no project).
enum LinkFile { Open { thread: String, shown: String, session: Value, cmds: Vec<Value> }, Note(&'static str), Os(String) }
fn link_file_of(st: &State,p: &Value,href: &str) -> LinkFile {
    use super::filelink::{plan, Plan};
    let hub=text(p,"hub").to_string();
    let mut session=p.get("session").filter(|s|s.is_object()).cloned().unwrap_or(json!({}));
    for k in ["open","drafts"] { if !session[k].is_object() { session[k]=json!({}); } }
    let shown=if p["target"]["kind"]=="thread" { text(&p["target"],"id").to_string() } else { text(&session,"sel").to_string() };
    let shown=shown.rsplit_once('|').map_or(shown.as_str(),|(_,r)|r).to_string();
    let home=crate::plat::dirs::home_or_empty();
    // a design file named alone (`main.kicad_sch`) that is not at the thread's root: the one of that name below it
    let base=super::filelink::root_of(st,&shown);
    let bare=crate::data::link::split_line(href.trim()).0.to_string();
    let found=if !bare.contains('/') && !std::path::Path::new(&format!("{base}/{bare}")).is_file() { super::filelink::find_named(&base,&bare) } else { None };
    let href=found.as_deref().unwrap_or(href);
    let (thread,out)=match plan(st,&shown,href,&home,&|x| std::path::Path::new(x).is_file()) {
        Plan::Note(n) => return LinkFile::Note(n),
        Plan::Os(path) => return LinkFile::Os(path),
        Plan::Open { thread, open, action, value } => {
            for (k,v) in open { session["open"][k]=json!(v); }
            session["sel"]=json!(thread);
            let o=if action=="view" { super::viewer::view_action(st,&hub,&session,&value) } else { super::viewer::files_action(st,&hub,&session,action,&value) };
            (thread,o)
        }
        Plan::Scoped { thread, kind, path } => { session["sel"]=json!(thread); let o=super::filelink::scoped_view(st,&hub,&session,kind,&path); (thread,o) }
    };
    let Some(o)=out else { return LinkFile::Note("That file does not open here.") };
    crate::session::patch::merge(&mut session,&o.patch);
    session["drafts"]["@ppage"]=json!(""); // (a project page on show shuts: the thread and its viewer are what the link opens)
    LinkFile::Open { thread, shown, session, cmds: o.before }
}
/// `link-file` carried out. Another thread: the target moves to it with its viewer open on the file (as `link-far`); the thread on show:
/// presented as it is with the viewer's patch and its watch first; a file of no design in no project: the system opens it, as before.
fn link_file(st: &State,far: &[(String,&State)],un: &Unified,p: &Value,href: &str) -> Option<Value> {
    if !mine(p) { return refused("target.or-native-sidebar") }
    let hub=text(p,"hub").to_string();
    let mut c=p.clone(); c["action"]=json!(""); c["value"]=json!("");
    match link_file_of(st,p,href) {
        LinkFile::Note(n) => { c["action"]=json!("link-note"); c["value"]=json!(n); recent_unified(st,far,un,&c) }
        LinkFile::Os(path) => {
            let mut v=recent_unified(st,far,un,&c)?; let rid=v["session"]["rid"].as_u64().unwrap_or(0);
            if let Some(a)=v["cmds"].as_array_mut() { a.push(json!({"type":"send","hub":hub,"text":json!({"id":rid,"m":"file.open","p":{"path":path,"how":"open"}}).to_string()})); }
            v["session"]["rid"]=json!(rid+1); Some(v)
        }
        LinkFile::Open { thread, shown, session, cmds } if thread!=shown =>
            Some(json!({"remoteReady":false,"nextTarget":{"kind":"thread","id":thread},"session":session,"cmds":cmds,"local":p.get("local").cloned().unwrap_or(json!({"local.native.sidebar.pages":"","local.native.sidebar.context":"","local.native.sidebar.key":"","local.native.sidebar.budget":""}))})),
        LinkFile::Open { session, cmds: first, .. } => {
            c["session"]=session;
            let mut v=recent_unified(st,far,un,&c)?;
            let mut cmds=first; cmds.extend(v["cmds"].as_array().cloned().unwrap_or_default()); v["cmds"]=json!(cmds);
            Some(v)
        }
    }
}
/// `recent_far` with projects shared across machines shown once (`Unified`).
pub fn recent_unified(st: &State,far: &[(String,&State)],un: &Unified,p: &Value) -> Option<Value> {
    if text(p,"action")=="proj-go" { match find_go(st,p) { Some(c)=>return recent_unified(st,far,un,&c), None=>{ let mut c=p.clone(); c["action"]=json!("proj-find-q"); c["value"]=json!(""); return recent_unified(st,far,un,&c) } } }
    // a file clicked in a message or a note (present/filelink.rs): its project's thread and the viewer tab that draws it, never the system's app;
    // a `backplane://file/<scope>/<path>` link is the same
    if text(p,"action")=="link-file" { let href=text(p,"value").to_string(); return link_file(st,far,un,p,&href) }
    if text(p,"action")=="link-open" { if let Some(r)=crate::md::refs::parse(text(p,"value").trim()).filter(|r|r.kind=="file") {
        let Some(path)=super::filelink::file_ref(st,&r.rest) else { let mut c=p.clone(); c["action"]=json!("link-note"); c["value"]=json!("That file is not on this hub."); return recent_unified(st,far,un,&c) };
        return link_file(st,far,un,p,&path) } }
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
    // a swarm's page: the thread its viewer is drawn for (its first member), when the seam gave its window; the room page covers its conversation
    let room_view=if room_page.is_object() { super::swarmform::viewer_thread(room_page) }else{String::new()};
    let room_view=if !room_view.is_empty() && p["window"].is_object() && (st.threads.contains_key(&room_view) || far.iter().any(|(_,f)|f.threads.contains_key(&room_view))) { room_view }else{String::new()};
    let thread=if p["target"]["kind"]=="thread" { text(&p["target"],"id") }else if bot_page.is_object() { text(bot_page,"chat") }else{room_view.as_str()};
    // the uberbot's (or a bot's) chat after its home moved: this hub keeps the old copy of the thread, the live one is the new home's, the same id there; it is drawn instead
    let moved=crate::net::homemove::moved_thread(setting(st,"home.at"),thread,st.threads.get(thread).map(|t|t.project.as_str()),&|id|far.iter().any(|(_,f)|f.threads.contains_key(id)));
    let thread=moved.as_deref().unwrap_or(thread);
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
        if action=="select" && session["drafts"].is_object() { session["drafts"]["@ppage"]=json!(""); } // a thread opened: the project page it came from shuts
        if let Some(target)=out.next_target {
            return Some(json!({"remoteReady":false,"nextTarget":target,"session":session,"cmds":cmds,
                "local":p.get("local").cloned().unwrap_or(json!({"local.native.sidebar.pages":"","local.native.sidebar.context":"","local.native.sidebar.key":"","local.native.sidebar.budget":""}))}));
        }
    } else if let Some(o)=super::onboard::act(st,hub,&session,p.get("hubInfo").unwrap_or(&Value::Null),&super::onboard::Client::of(p),&super::onboard::home_dir(),action,value) {
        let ended=super::onboard::ended(action,&o.patch);
        crate::session::patch::merge(&mut session,&o.patch); cmds=[read_cmds.clone(),o.cmds].concat();
        // setup ended with no thread open: the person lands in their newest thread (the step said they could write in it after setup),
        // not on a page whose message box sends nowhere
        // Open the uberbot on Done: its chat, once this home has one
        if ended && action=="ob-uber-open" { if let Some(id)=super::onboard::land_uber(st) {
            if let Some(o)=session.as_object_mut() { o.remove("route"); }
            return Some(json!({"remoteReady":false,"nextTarget":{"kind":"bot","id":id},"session":session,"cmds":cmds,
                "local":p.get("local").cloned().unwrap_or(json!({"local.native.sidebar.pages":"","local.native.sidebar.context":"","local.native.sidebar.key":"","local.native.sidebar.budget":""}))}));
        } }
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
    } else if let Some(o)=super::swarmform::act(st,hub,&session,action,value) {
        crate::session::patch::merge(&mut session,&o.patch); cmds=[read_cmds.clone(),o.cmds].concat();
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
        // the sidebar's sections (present/sections.rs): a fold, a project's page, a project pin, a group's name, a project moved to a group
        "side-section" | "project-page" | "project-pin" | "project-fold" | "thread-hide" | "group-rename" | "group-rename-start" | "group-move" | "group-new-start" | "project-archive" => {
            let rows=merged_here(st,far,un);
            let Some(a)=super::sections::act(st,&rows,action,value,&session["drafts"],p["now"].as_u64().unwrap_or(0)/1000) else { return refused("action.sidebar-section") };
            if let Some(k)=a.open_toggle { if !session["open"].is_object() { session["open"]=json!({}); } let was=flag(&session,&k); session["open"][k]=json!(!was); }
            for k in a.clear { session["drafts"][k]=json!(""); }
            for (k,v) in a.drafts { session["drafts"][k]=json!(v); }
            for (k,v) in a.sets { let rid=session["rid"].as_u64().unwrap_or(0); cmds.push(json!({"type":"send","hub":hub,"text":json!({"id":rid,"m":"setting.set","p":{"key":k,"value":v}}).to_string()})); session["rid"]=json!(rid+1); }
            // a project's page (its threads: running, needs you, recent, done); "" shuts it. A thread opened from it shuts it too (`select`).
            if let Some(key)=a.page { session["drafts"]["@ppage"]=json!(key); }
        },
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
            if !st.bots_nav.contains_key(id) && !(id.contains('~') && far.iter().any(|(_,f)|f.bots_nav.contains_key(id))) { return refused("action.bot-unknown") }
            if let Some(o)=session.as_object_mut() { o.remove("route"); }
            return Some(json!({"remoteReady":false,"nextTarget":{"kind":"bot","id":id},"session":session,"cmds":cmds,"local":p.get("local").cloned().unwrap_or(json!({"local.native.sidebar.pages":"","local.native.sidebar.context":"","local.native.sidebar.key":"","local.native.sidebar.budget":""}))}));
        },
        // a room's page: its row opens it, the post box (`bfield` rpost) is sent as a post by the person (`bots.post`)
        "room" => {
            let id=value.strip_prefix(&format!("{hub}|" )).unwrap_or(value);
            if !(st.rooms_nav.contains_key(id) || (id.contains('~') && far.iter().any(|(_,f)|f.rooms_nav.contains_key(id)))) || id.starts_with("dm:") { return refused("action.room-unknown") }
            if let Some(o)=session.as_object_mut() { o.remove("route"); }
            return Some(json!({"remoteReady":false,"nextTarget":{"kind":"room","id":id},"session":session,"cmds":cmds,"local":p.get("local").cloned().unwrap_or(json!({"local.native.sidebar.pages":"","local.native.sidebar.context":"","local.native.sidebar.key":"","local.native.sidebar.budget":""}))}));
        },
        // Life.keep: Keep this room on a bots' exchange's page (`roomPage.keep`) makes it permanent (`bots.room_keep`)
        "room-keep" if room_page["keep"]==json!(true) => {
            let rid=session["rid"].as_u64().unwrap_or(0);
            cmds.push(json!({"type":"send","hub":hub,"text":json!({"id":rid,"m":"bots.room_keep","p":{"room":text(&p["target"],"id")}}).to_string()}));
            session["rid"]=json!(rid+1);
        },
        // a swarm's controls on its room page (`roomPage.swarm.actions`, its add picker, a member's remove): pause, resume, stop,
        // `add|<provider>|<model>|<role>`, `remove|<thread>` (`swarm.act`, present/swarmform.rs); the hub refuses what does not apply
        "swarm-act" if room_page["swarm"].is_object() => {
            let Some(q)=super::swarmform::act_params(&room_page["swarm"],text(&p["target"],"id"),value) else { return refused("action.swarm-act") };
            let rid=session["rid"].as_u64().unwrap_or(0);
            cmds.push(json!({"type":"send","hub":hub,"text":json!({"id":rid,"m":"swarm.act","p":q}).to_string()}));
            session["rid"]=json!(rid+1);
            if value.starts_with("add|") { session["drafts"]["@sadd"]=json!(""); }
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
            Ok(value)=>value,
            // a swarm's page stands without its viewer's thread: the page is what the person opened
            Err(reason) if !room_view.is_empty() => { super::refusal::report(reason); Value::Null },
            Err(reason)=>return refused(reason),
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
    // (a machine picked filters the merged rows: the join stays, so a project is the same row whichever machine is picked)
    let un_far: &HashMap<String,String>=un.far;
    let Some(mut ns)=NativeSidebar::prepare(p,mode,&query,&session["open"],st.projects.keys().cloned().chain(far.iter().flat_map(|(_,f)|f.projects.keys().cloned())).collect()) else { return refused("sidebar.key-or-capability") };
    if action=="native-sidebar-page" && !ns.accept(value) { return Some(json!({"refused":true,"local":p.get("local").cloned().unwrap_or(json!({}))})) }
    if action=="native-sidebar-page" { session["drafts"]["@menu"]=json!(""); session["drafts"]["@del"]=json!(""); }
    let open: HashSet<String>=session["open"].as_object().into_iter().flatten().filter(|(_,v)|v.as_bool()==Some(true)).map(|(k,_)|k.clone()).collect();
    // a search unfolds the older projects too (Side.unfolded)
    // the sidebar marks the thread on show: none on a room's page (its viewer's thread is not what the person opened)
    let side_sel=if room_page.is_object() { "" }else{thread};
    let opts=sidebar::Opts { hub,now:p["now"].as_u64().unwrap_or(0)/1000,selected:side_sel,open:&open,older_open:flag(&session,"old-projects") || !query.is_empty(),query:&query };
    let plan=sidebar::plan(st,&opts);
    // a linked project shown as one with a project here opens and shuts with it
    let mut fopen=open.clone();
    for (f,l) in un_far.iter() { if open.contains(l) { fopen.insert(f.clone()); } if open.contains(&format!("arch:{l}")) { fopen.insert(format!("arch:{f}")); } }
    let fopts=sidebar::Opts { open:&fopen,..opts };
    let fplans: Vec<(&str,&State,sidebar::Plan)>=far.iter().map(|(m,f)|(m.as_str(),*f,sidebar::plan(f,&fopts))).collect();
    // the project that draws a merged row (this hub's, else a linked machine's) -> the other copies shown in it: (machine, state, plan, far id)
    let owner_live=|l: &str| st.projects.contains_key(l) || far.iter().any(|(_,f)|f.projects.contains_key(l));
    let mut partners: HashMap<&str,Vec<(&str,&State,&sidebar::Plan,&str)>>=HashMap::new();
    for (m,f,fp) in &fplans { for id in fp.ids() { if let Some(l)=un_far.get(id) { if l!=id && owner_live(l) { partners.entry(l.as_str()).or_default().push((*m,*f,fp,id)); } } } }
    let selected_project=st.threads.get(side_sel).map(|t|t.project.as_str()).unwrap_or("");
    // Find.rows: the projects whose name fuzzy-matches, and those holding a thread the search finds; the one holding the open thread stays (Side.cur.held)
    let far_hit=|id: &str| partners.get(id).map_or(false,|v|v.iter().any(|(_,_,fp,f)|fp.has_hit(f)));
    let mut ids: Vec<&str>=plan.ids().into_iter().filter(|id| query.is_empty() || *id==selected_project || st.projects.get(*id).map_or(false,|p|fuzzy_hit(&p.title,&query)) || plan.has_hit(id) || far_hit(id)).collect();
    let joined: HashSet<&str>=partners.values().flatten().map(|(_,_,_,f)|*f).collect();
    // one list over machines: each machine's projects keep their order, and stand by recency among the others' (live first, then the newest);
    // a project shown as one with a linked machine's stands where the livelier of them would
    let lowest=|pl: &sidebar::Plan,id: &str| pl.place(id).unwrap_or((2,std::cmp::Reverse(0)));
    let mut lists=vec![ids.drain(..).map(|id|(partners.get(id).into_iter().flatten().map(|(_,_,fp,f)|lowest(fp,f)).fold(lowest(&plan,id),std::cmp::min),id)).collect::<Vec<_>>()];
    // (a linked copy in no merged row is waiting for its machine's origin (present/merged.rs): not drawn until it is known; no rows: no join made)
    let keyed=|id: &str| un.rows.is_empty() || un.rows.contains_key(id) || un.far.contains_key(id);
    for (_,f,fp) in &fplans { lists.push(fp.ids().into_iter().filter(|id| !joined.contains(id) && keyed(id)).filter(|id| query.is_empty() || f.projects.get(*id).map_or(false,|p|fuzzy_hit(&p.title,&query)) || fp.has_hit(id) || far_hit(id)).map(|id|(partners.get(id).into_iter().flatten().map(|(_,_,pp,x)|lowest(pp,x)).fold(lowest(fp,id),std::cmp::min),id)).collect()); }
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
    let mut projects: Vec<Value>=selected.into_iter().filter_map(|owner| {
        // the plan (and state) that holds this project: this hub's, else the linked machine's it came from
        let (pst,pplan,machine)=match fplans.iter().find(|(_,f,_)|f.projects.contains_key(owner)) { Some((m,f,fp)) if !st.projects.contains_key(owner) => (*f,fp,*m), _ => (st,&plan,"") };
        let mut project=pplan.project(owner,&mut |kind,ids| {
            let (mut indices,pager)=ns.slice_ids(ids,kind,owner,row_limit,side_sel);
            indices.sort_unstable(); // Mob.native.tree_rows filters in tree order.
            (indices,pager)
        })?;
        let pick=setting(pst,&format!("project.pick.{owner}"));
        let icon=super::badge::icon_eff(setting(pst,&format!("project.icon.{owner}")),pick,pst.projects.get(owner).map_or("",|p|p.title.as_str()));
        project["icon"]=json!(icon);
        let color=super::badge::color_eff(setting(pst,&format!("project.color.{owner}")),pick,owner);
        project["color"]=json!(color); project["css"]=json!(super::badge::css_of(color));
        project["machine"]=json!(machine);
        // folded by its chevron (0.14's project-closed:<id>): the window draws its head alone
        project["closed"]=json!(flag(&session,&format!("project-closed:{owner}")));
        if machine.is_empty() && text(&session["drafts"],"@look")==owner { project["look"]=super::badge::editor(owner,color,icon); }
        // another machine's project: its marker after the name ("on desktop"); this machine's has none
        if !machine.is_empty() { let m=un.mark(machine); sidebar::label_rows(&mut project,&m); project["mark"]=m; }
        // the same repository on a linked machine: its threads in this project, each row naming its machine
        if let Some(ps)=partners.get(owner) {
            let (mut machines,mut marks): (Vec<Value>,Vec<Value>)=(vec![],vec![]);
            // a row drawn from a linked machine's copy (no copy here) names that machine among its machines, and no single "on <machine>"
            if !machine.is_empty() { machines.push(json!(machine)); marks.push(un.mark(machine)); project.as_object_mut().unwrap().remove("mark"); }
            for (m,_,fp,fid) in ps {
                let Some(mut fproj)=fp.project(fid,&mut |kind,ids| { let (mut indices,pager)=ns.slice_ids(ids,kind,fid,row_limit,side_sel); indices.sort_unstable(); (indices,pager) }) else { continue };
                sidebar::label_rows(&mut fproj,&un.mark(m));
                sidebar::join_project(&mut project,fproj,&row_key);
                if !machines.contains(&json!(m)) { machines.push(json!(m)); marks.push(un.mark(m)); }
            }
            project["machines"]=json!(machines);
            // a new thread of this repository on the machine chosen: this one's project, or a linked one's (the request goes there)
            project["newOn"]=if machine.is_empty() { new_on(owner,ps.iter().map(|(m,_,_,fid)|(*m,*fid)),un) } else { new_on_far(std::iter::once((machine,owner)).chain(ps.iter().map(|(m,_,_,fid)|(*m,*fid))),un) };
            // the machines it is also on, after its name ("also on desktop"); each of their rows keeps its own marker
            project["also"]=json!(if machine.is_empty() { sidebar::also_label(&marks) } else { sidebar::also_label(&marks).replacen("also on","on",1) }); project["marks"]=json!(marks);
        }
        merged_fields(&mut project,un.rows.get(owner),hub,opts.now,un.here,&session["drafts"]);
        if machine.is_empty() { let l=link_with(owner,un.rows); if l.as_array().map_or(false,|a|!a.is_empty()) { project["linkWith"]=l; } }
        Some(project)
    }).collect();
    // a project every machine of which is off (known from the registry on the home): its row, dimmed, after the live ones
    if mode!="active" { for (k,r) in un.rows.iter().filter(|(k,_)|k.starts_with("@off:")) { if query.is_empty() || fuzzy_hit(text(r,"name"),&query) { projects.push(off_project(k,r,hub,opts.now)); } } }
    // the machine switcher filters the merged rows: the rows that machine has a copy of, with that machine's threads only
    if !mpick.is_empty() { projects=pick_filter(projects,&mpick); }
    let ren=text(&session["drafts"],"@ren");
    let ren_kind=match text(&session["drafts"],"@ren.kind") { "project"=>"project", "group"=>"group", "group-new"=>"group-new", _=>"thread" };
    // New group… (a project's menu) asks the new group's name in the same bar
    let renaming=if ren.is_empty() || text(&session["drafts"],"@ren.inline")=="1" { Value::Null }else{json!({"id":format!("{hub}|{ren}"),"kind":ren_kind,"title":match ren_kind { "project"=>"Rename project", "group"=>"Rename group", "group-new"=>"New group", _=>"Rename thread" },"text":text(&session["drafts"],"@f.title"),"yes":if ren_kind=="group-new" { "Create" } else { "Rename" },"no":"Cancel"})};
    let del=text(&session["drafts"],"@del");
    let deleting=if del.is_empty() { Value::Null }else{json!({"id":if del.contains('|'){del.to_string()}else{format!("{hub}|{del}")},"title":"Delete thread","body":st.threads.get(del.strip_prefix(&format!("{hub}|" )).unwrap_or(del)).map(|t|t.title.as_str()).unwrap_or(""),"yes":"Delete","no":"Cancel"})};
    let pages=json!([{"version":1,"hub":hub,"projects":projects_page,"active":null,"legacy":null}]);
    let shown=json!({"projects":projects,"sidebarPages":pages,"nativeSidebar":1,"active":[],"legacy":[],"thread":shown_thread,"rooms":rooms(st,far,un,hub),"bots":bots(st,far,un,hub,opts.now),"theme":setting(st,"ui.theme"),
        "renaming":renaming,"deleting":deleting,"bot":null,"settings":if super::settings::is_open(&session) { super::settings::view(st,&session,p.get("hubInfo").unwrap_or(&Value::Null)) }else{Value::Null},"newRoom":super::botforms::new_room(st,hub,&session).unwrap_or(Value::Null),"newBot":super::botforms::new_bot(&session).unwrap_or(Value::Null),"projectHome":null});
    let mut shown=shown;
    if !far.is_empty() { shown["machines"]=machine_switch(far,un,&mpick); }
    // the sections (docs/ALWAYS-ON.md section 8): Bub, Needs you, Pinned, Active, groups, Quiet; the project page; Ctrl+K's list
    { let rows=merged_here(st,far,un); let open=super::sections::opened(&session);
      let c=super::sections::Ctx{ hub,now:opts.now,query:&query,open:&open,selected:side_sel };
      let bub=shown["bots"].as_array().and_then(|b|b.iter().find(|r|r["uber"]==json!(true))).cloned().unwrap_or(Value::Null);
      shown["sections"]=super::sections::build(st,&rows,&bub,&c);
      shown["jump"]=super::sections::jump(&rows,&c);
      shown["groupsTo"]=super::sections::move_targets(st,&rows);
      let key=text(&session["drafts"],"@ppage");
      if !key.is_empty() { shown["projectPage"]=super::sections::page(st,&rows,key,&c,open.contains("page-done")); } }
    // the Active view: what needs the person first, then the recently settled folded under "Settled N" (a search shows them)
    if mode=="active" {
        let keep=|title:&str,proj:&str| query.is_empty() || fuzzy_hit(title,&query) || fuzzy_hit(proj,&query);
        let (live,faded)=if here_on { plan.active_keyed(&keep) } else { (vec![],vec![]) };
        let (mut lives,mut fadeds)=(vec![live],vec![faded]);
        for (m,_,fp) in fplans.iter().filter(|(m,_,_)|mpick.is_empty() || *m==mpick) { let (mut l,mut f)=fp.active_keyed(&keep);
            // a linked machine's thread names its machine, and the project it is shown in here
            for r in l.iter_mut().chain(f.iter_mut()).map(|x|&mut x.1) { r["machine"]=json!(m); r["mark"]=un.mark(m); let fid=text(r,"projectId").strip_prefix(&format!("{hub}|")).unwrap_or("").to_string(); if let Some(lp)=un.far.get(&fid).and_then(|l|st.projects.get(l).or_else(||far.iter().find_map(|(_,f)|f.projects.get(l))).map(|p|(l,p))) { r["projectId"]=json!(format!("{hub}|{}",lp.0)); r["project"]=json!(lp.1.title); } }
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
    { let f=picker::folders(&session["drafts"],&crate::plat::dirs::home_or_empty(),&st.projects.iter().filter(|(_,p)|!p.removed).map(|(id,p)|(p.root.as_str(),id.as_str())).collect::<Vec<_>>()); if !f.is_null() { shown["folders"]=f; } } // (present only while the picker is open)
    if !thread.is_empty() { shown["usage"]=super::keys::usage(tst,thread,opts.now); shown["tokChip"]=json!(super::keys::tok_chip(tst,thread)); } // the Context panel (present/tokstats.rs `usage`) and its chip
    if bot_page.is_object() {
        let id=text(&p["target"],"id"); let key=format!("{hub}|{id}");
        let mut b=bot_page.clone();
        if text(&b,"tab")=="routines" { b["routine"]=super::botforms::routine_form(&session); }
        if let Some(rows)=shown["bots"].as_array_mut() { if let Some(row)=rows.iter_mut().find(|r|r["id"]==json!(key)) {
            row["sel"]=json!(true); b["mood"]=row["mood"].clone(); b["note"]=row["note"].clone(); b["cat"]=row["cat"].clone();
        } }
        shown["bot"]=b;
    }
    if room_page.is_object() { shown["bot"]=room_page.clone();
        // a swarm's page: its add-member picker (present/swarmform.rs), from the composer's model lists
        if room_page["swarm"].is_object() { let agents=super::keys::picker_agents_of(p["hubInfo"]["models"].as_str()).1;
            if let Some(a)=super::swarmform::add_block(&session,&room_page["swarm"],&agents) { shown["bot"]["swarm"]["add"]=a; } } }
    // New swarm, opened from a project's menu
    if let Some(f)=super::swarmform::new_swarm(st,&session,&super::keys::picker_agents_of(p["hubInfo"]["models"].as_str()).1) { shown["newSwarm"]=f; }
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
        for path in ["/shown/projects","/overlay/j/projects"] { for pr in actual.pointer_mut(path).and_then(|v| v.as_array_mut()).into_iter().flatten() { if let Some(o)=pr.as_object_mut() { o.remove("color"); o.remove("css"); o.remove("closed"); o.remove("swarms"); /* a project's swarms (docs/SWARM.md), which 0.14 had none of */
            // and a red row's want word (sidebar::want), drawn where its age goes
            for k in ["threads","settled","snoozed","archived"] { for r in o.get_mut(k).and_then(|v| v.as_array_mut()).into_iter().flatten() { if let Some(r)=r.as_object_mut() { r.remove("want"); } } } } } }
        // approved additions: the sidebar's sections, Ctrl+K's list, the groups a project moves to, a project's page (present/sections.rs), which 0.14 has none of
        for path in ["/shown","/overlay/j"] { if let Some(o)=actual.pointer_mut(path).and_then(|v|v.as_object_mut()) { for k in ["sections","jump","groupsTo","projectPage"] { o.remove(k); } } }
        let mut shown=expected["shown"].clone(); retain_root(&mut shown,OMITTED);
        assert_eq!(actual["shown"],shown,"retained GPUI shown fields differ (no value normalisations)");
        let mut overlay=expected["overlay"].clone(); retain_root(&mut overlay["j"],OVERLAY_OMITTED);
        assert_eq!(actual["overlay"],overlay,"retained overlay fields differ");
        for k in ["remoteReady","session","cmds","window"] { assert_eq!(actual[k],expected[k],"{k}"); }
    }
    #[test] fn a_machine_known_from_the_registry_that_answers_is_not_dimmed_in_the_sections() {
        use crate::present::merged::{RegProject,RegMachine,RegThread};
        let st={ let mut s=State::new(); s.lean=true; s }; let marks=HashMap::new(); let un=Unified{ far:&HashMap::new(), marks:&marks, here:"laptop", rows:&HashMap::new() };
        let reg=vec![RegProject{ key:"github.com/o/r".into(), name:"Far".into(), origin:"github.com/o/r".into(), machines:vec![RegMachine{ machine:"desktop".into(), id:"p9".into(), folder:"/d/r".into(), seen:1 }],
            threads:vec![RegThread{ machine:"desktop".into(), id:"t9".into(), title:"runs".into(), status:"running".into(), activity:1, pinned:false }] }];
        let open=HashSet::new(); let c=super::super::sections::Ctx{ hub:"h",now:2,query:"",open:&open,selected:"" };
        let row=|rows: &[super::super::merged::MergedProject]| super::super::sections::build(&st,rows,&Value::Null,&c).as_array().unwrap().iter().find(|s|s["id"]=="active").unwrap()["rows"][0].clone();
        // no mirrors to ask (a test, the oracle): known from the registry alone, the machine is off and the row dimmed
        let p=row(&merged_with(&st,&[],&un,None,&reg)); assert_eq!((p["dots"][0]["dim"].clone(),p["off"].clone()),(json!(true),json!(true)));
        // linked and answering: its chip is on and the row is not dimmed (mirror::live_chips, the one rule)
        let m=crate::mirror::Mirrors::default(); m.set_names(Arc::new(||vec![("L1".to_string(),"desktop".to_string())]));
        let rows=merged_with(&st,&[],&un,Some(&m),&reg); assert_eq!(rows[0].machines[0]["state"],"on");
        let p=row(&rows); assert_eq!((p["dots"][0]["dim"].clone(),p["off"].clone()),(json!(false),json!(false)));
        // linked but away: still dimmed
        m.set_away("L1",true); let p=row(&merged_with(&st,&[],&un,Some(&m),&reg)); assert_eq!(p["dots"][0]["dim"],json!(true));
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
    /// The home (boat-hub) has no copy; desktop and framework each have one: ONE row, drawn from the first machine's copy, holding both machines'
    /// threads, its chips naming both, no "on <machine>" of one; a machine picked keeps that row with that machine's threads.
    #[test] fn two_linked_machines_copies_are_one_row_on_a_hub_with_none() {
        let mut st=State::new(); st.lean=true;
        let mk=|evs: Vec<Value>| { let mut f=State::new(); f.lean=true; for e in evs { event(&mut f,e); } f };
        let d=mk(vec![json!({"$":"ProjectCreated","id":"D~p1","title":"repo-desk","root":"/d","at":1}), json!({"$":"ThreadCreated","id":"D~t1","project":"D~p1","title":"on desk","at":5})]);
        let f=mk(vec![json!({"$":"ProjectCreated","id":"F~p1","title":"repo-fw","root":"/f","at":1}), json!({"$":"ThreadCreated","id":"F~t1","project":"F~p1","title":"on fw","at":7})]);
        let far=vec![("desktop".to_string(),&d),("framework".to_string(),&f)];
        let origin=|id: &str| if id.ends_with("~p1") { "github.com/o/r".to_string() } else { String::new() };
        let list=super::super::merged::merged_projects(&st,&far,&|_|false,&origin,"boat-hub",&[]);
        let j=super::super::merged::join_of(&list,&["desktop".to_string(),"framework".to_string()]);
        assert_eq!(j.far.get("F~p1").map(String::as_str),Some("D~p1"));
        let marks: HashMap<String,Value>=far.iter().map(|(n,_)|(n.clone(),crate::mirror::mark_of(n,false))).collect();
        let un=Unified{ far:&j.far, marks:&marks, here:"boat-hub", rows:&j.rows };
        let v=recent_unified(&st,&far,&un,&params()).unwrap(); let ps=v["shown"]["projects"].as_array().unwrap();
        assert_eq!(ps.len(),1,"{ps:?}");
        let p=&ps[0]; assert_eq!(p["key"],"github.com/o/r"); assert!(p.get("mark").is_none(),"no single machine's label");
        assert_eq!(p["also"],"on desktop, framework");
        assert_eq!(p["machineChips"].as_array().unwrap().iter().map(|c|c["short"].as_str().unwrap()).collect::<Vec<_>>(),["framework","desktop"],"by activity");
        assert_eq!(p["threads"].as_array().unwrap().iter().map(|r|(r["id"].as_str().unwrap(),r["mark"]["short"].as_str().unwrap())).collect::<Vec<_>>(),[("h|F~t1","framework"),("h|D~t1","desktop")]);
        // the picker: where it runs by default first (framework: the livelier, no copy here), then Clone it here and a Boat (off: no Boat account)
        assert_eq!(p["newOn"].as_array().unwrap().iter().map(|o|(o["value"].as_str().unwrap(),o["kind"].as_str().unwrap(),o["disabled"]==true)).collect::<Vec<_>>(),
            [("F~p1","machine",false),("D~p1","machine",false),("github.com/o/r","clone",false),("github.com/o/r","boat",true)]);
        assert_eq!(p["runsOn"],"runs on framework");
        let mut q=params(); q["session"]["drafts"]["@machine"]=json!("framework");
        let v=recent_unified(&st,&far,&un,&q).unwrap(); let ps=v["shown"]["projects"].as_array().unwrap();
        assert_eq!((ps.len(),ps[0]["threads"].as_array().unwrap().iter().map(|r|r["id"].as_str().unwrap()).collect::<Vec<_>>()),(1,vec!["h|F~t1"]));
        // a machine that is off (registry only): its chip dimmed, its threads listed, not opened
        let reg=vec![super::super::merged::RegProject{ key:"github.com/o/r".into(), name:"repo".into(), origin:"github.com/o/r".into(),
            machines:vec![super::super::merged::RegMachine{ machine:"laptop".into(), id:"p4".into(), folder:"/l".into(), seen:3 }],
            threads:vec![super::super::merged::RegThread{ machine:"laptop".into(), id:"laptop~t4".into(), title:"off one".into(), status:"needs".into(), activity:9, pinned:false }] }];
        let list=super::super::merged::merged_projects(&st,&far,&|_|false,&origin,"boat-hub",&reg);
        let j=super::super::merged::join_of(&list,&["desktop".to_string(),"framework".to_string()]);
        let un=Unified{ far:&j.far, marks:&marks, here:"boat-hub", rows:&j.rows };
        let v=recent_unified(&st,&far,&un,&params()).unwrap(); let p=&v["shown"]["projects"][0];
        let off=p["threads"].as_array().unwrap().iter().find(|r|r["off"]==true).unwrap();
        assert_eq!((off["id"].as_str(),off["mark"]["note"].as_str(),off["status"].as_str()),(Some("h|laptop~t4"),Some("laptop is off"),Some("needs")));
        assert!(p["machineChips"].as_array().unwrap().iter().any(|c|c["short"]=="laptop" && c["dim"]==true));
    }
    /// A picked machine that has gone off and left the live rows stays the choice, as one to wait for, and the line says it is away.
    #[test] fn a_picked_machine_that_left_the_rows_stays_picked_and_away() {
        let row=json!({"default":{"machine":"","id":"pA1"},"machineChips":[{"name":"desktop","short":"desktop","here":true,"dim":false},{"name":"boat-hub","short":"boat-hub","here":false,"dim":true,"state":"off"}],"origin":""});
        let mut p=json!({"id":"h|pA1","newOn":[{"value":"pA1","label":"New thread on desktop","away":false}]});
        picker_on(&mut p,&row,"desktop","L~pB1");
        assert_eq!((p["newOn"][0]["value"].as_str(),p["newOn"][0]["kind"].as_str(),p["newOn"][0]["on"]==true),(Some("L~pB1"),Some("wait"),true));
        assert_eq!(p["newOn"][0]["label"],"Wait for boat-hub"); assert_eq!(p["newOn"][1]["on"],Value::Null);
        assert_eq!(p["runsOn"],"boat-hub is away");
    }
    /// Link with… lists the other machines' projects with no remote; a linked one offers Unlink; a project with a remote offers none.
    #[test] fn link_with_offers_other_machines_projects_with_no_remote() {
        let c=|here: bool,short: &str| json!({"here":here,"short":short});
        let rows: HashMap<String,Value>=[
            ("p1".to_string(),json!({"key":"desk/p1","origin":"","name":"Notes","machineChips":[c(true,"desk")]})),
            ("L~p2".to_string(),json!({"key":"fw/p2","origin":"","name":"Jots","machineChips":[c(false,"fw")]})),
            ("L~p3".to_string(),json!({"key":"github.com/o/r","origin":"github.com/o/r","name":"R","machineChips":[c(false,"fw")]})),
            ("p4".to_string(),json!({"key":"github.com/o/x","origin":"github.com/o/x","name":"X","machineChips":[c(true,"desk")]})),
        ].into_iter().collect();
        assert_eq!(link_with("p1",&rows),json!([{"label":"Link with Jots on fw","value":"p1|fw/p2"}]));
        assert_eq!(link_with("p4",&rows),json!([]));
        let mut r2=rows.clone(); r2.insert("p1".into(),json!({"key":"fw/p2","origin":"","name":"Notes","machineChips":[c(true,"desk"),c(false,"fw")]})); r2.remove("L~p2");
        assert_eq!(link_with("p1",&r2)[0],json!({"label":"Unlink","value":"p1|"}));
    }
    /// One repository on two machines is one sidebar project: the linked machine's threads sit in this hub's project, each row names its
    /// machine; a linked project of another repository keeps its own row; the Active view names this hub's project for both.
    /// A KiCad file clicked in a message opens inside Backplane: the target moves to its project's thread with the Schematic tab on that file,
    /// and its watch is sent; in the thread on show it stays there; nothing goes to the system's app (`file.open`).
    #[test] fn a_kicad_file_link_opens_its_projects_thread_on_the_viewer_tab() {
        let d=crate::plat::tmp::dir("filelink");
        std::fs::create_dir_all(d.join("amp/boards/main")).unwrap(); std::fs::create_dir_all(d.join("other")).unwrap();
        std::fs::write(d.join("amp/boards/main/main.kicad_sch"),"(kicad_sch)").unwrap(); std::fs::write(d.join("amp/boards/main/main.kicad_pcb"),"(kicad_pcb)").unwrap();
        let (amp,other)=(d.join("amp").to_string_lossy().to_string(),d.join("other").to_string_lossy().to_string());
        let mut st=State::new(); st.lean=true;
        for e in [json!({"$":"ProjectCreated","id":"pa","title":"Amp","root":amp,"at":1}),json!({"$":"ProjectCreated","id":"po","title":"Other","root":other,"at":1}),
                  json!({"$":"ThreadCreated","id":"ta","project":"pa","title":"A","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":5}),
                  json!({"$":"ThreadCreated","id":"to","project":"po","title":"O","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":6})] { event(&mut st,e); }
        let map=HashMap::new(); let marks=HashMap::new(); let un=Unified{ far:&map, marks:&marks, here:"desk", rows:&HashMap::new() };
        let click=|on: &str,href: &str| { let mut q=params(); q["target"]=json!({"kind":"thread","id":on}); q["window"]=json!({"contextMessages":[],"messageIds":[],"historyUnits":[],"partial":[],"ids":[],"anchor":{"id":"","offset":0},"older":"","olderNext":"","hasMore":false,"remaining":0,"newer":0,"total":0}); q["action"]=json!("link-file"); q["value"]=json!(href); recent_unified(&st,&[],&un,&q).unwrap() };
        let sends=|v: &Value| v["cmds"].as_array().into_iter().flatten().filter_map(|c| serde_json::from_str::<Value>(c["text"].as_str()?).ok()).collect::<Vec<_>>();
        // from another project's thread: the target moves to Amp's thread, the Schematic tab on that file
        let sch=format!("file://{amp}/boards/main/main.kicad_sch");
        let v=click("to",&sch);
        assert_eq!(v["nextTarget"],json!({"kind":"thread","id":"ta"}),"{v}");
        let dr=&v["session"]["drafts"];
        assert_eq!((dr["@view:ta"].as_str(),dr["@view.file:ta"].as_str()),(Some("schematic"),Some(format!("{amp}/boards/main/main.kicad_sch").as_str())));
        assert_eq!(v["session"]["open"]["project-closed:pa"],false,"its project unfolded");
        let w=sends(&v); assert!(w.iter().any(|c|c["m"]=="kicad.watch" && c["p"]["kind"]=="schematic"),"{w:?}"); assert!(!w.iter().any(|c|c["m"]=="file.open"),"never the system's app");
        // in the thread on show (a path from its folder): no move, the Board tab on the board
        let mut q=params(); q["target"]=json!({"kind":"thread","id":"ta"});
        let LinkFile::Open { thread, shown, session, cmds }=link_file_of(&st,&q,"boards/main/main.kicad_pcb") else { panic!("opens") };
        assert_eq!((thread.as_str(),shown.as_str(),session["drafts"]["@view:ta"].as_str()),("ta","ta",Some("board")));
        assert!(sends(&json!({"cmds":cmds})).iter().any(|c|c["m"]=="kicad.watch" && c["p"]["kind"]=="board"));
        // a file the viewer does not draw, in no project: the system's, as a click on a path always did
        assert!(matches!(link_file_of(&st,&q,"/etc/hostname"),LinkFile::Os(p) if p=="/etc/hostname"));
        // a backplane://file link is the same as its path
        let mut q=params(); q["target"]=json!({"kind":"thread","id":"to"}); q["action"]=json!("link-open"); q["value"]=json!("backplane://file/pa/boards/main/main.kicad_pcb");
        let v=recent_unified(&st,&[],&un,&q).unwrap(); assert_eq!(v["nextTarget"]["id"],"ta"); assert_eq!(v["session"]["drafts"]["@view:ta"],"board");
        let _=std::fs::remove_dir_all(&d);
    }
    #[test] fn one_repository_on_two_machines_is_one_project() {
        let d=crate::plat::tmp::dir("unify");
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
        let un=Unified{ far:&map, marks:&marks, here:"desk", rows:&HashMap::new() };
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
        // the linked machine only: the merged rows it has a copy of (the same row as under All, not split off), with its threads only
        let fw=pick("framework"); let fp=fw["shown"]["projects"].as_array().unwrap();
        let mut got=fp.iter().map(|p|(p["title"].as_str().unwrap(),p["machine"].as_str().unwrap())).collect::<Vec<_>>(); got.sort();
        assert_eq!(got,[("Other","framework"),("backplane-gpui","")],"the merged row stays one row");
        let bp=fp.iter().find(|p|p["title"]=="backplane-gpui").unwrap();
        assert_eq!(bp["threads"].as_array().unwrap().iter().map(|r|r["id"].as_str().unwrap()).collect::<Vec<_>>(),["h|L~t9","h|L~t8"],"framework's threads only");
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
    /// A bots' exchange (bots/life.rs) is in no sidebar list, here or a linked machine's; it still opens by its id, and Keep is sent only from a page that offers it
    #[test] fn an_ephemeral_room_is_not_listed_but_opens_by_id() {
        let mut st=State::new(); st.lean=true;
        for e in [json!({"$":"BotSet","id":"b","name":"Cat","thread":"t","look":1,"at":2}),
            json!({"$":"RoomSet","id":"r1","name":"staff","members":"b\nother","at":3}),
            json!({"$":"RoomSet","id":"r2","name":"Google sign-in check","members":"b\nother","ephemeral":true,"at":3}),
            json!({"$":"RoomSet","id":"r3","name":"old check","members":"b\nother","ephemeral":true,"closed":true,"at":3})] { event(&mut st,e); }
        let v=recent(&st,&params()).unwrap();
        let ids: Vec<&str>=v["shown"]["rooms"].as_array().unwrap().iter().map(|r|r["id"].as_str().unwrap()).collect();
        assert_eq!(ids,["h|r1"]);
        let mut p=params(); p["action"]=json!("room"); p["value"]=json!("h|r2");
        assert_eq!(recent(&st,&p).unwrap()["nextTarget"],json!({"kind":"room","id":"r2"}),"reachable by its id");
        // the person keeps it: the room is listed from then on
        event(&mut st,json!({"$":"RoomSet","id":"r2","name":"Google sign-in check","members":"b\nother","ephemeral":false,"closed":false,"at":4}));
        let v=recent(&st,&params()).unwrap(); assert_eq!(v["shown"]["rooms"].as_array().unwrap().len(),2);
        // a later change with no life fields (a join) keeps the closed room hidden
        event(&mut st,json!({"$":"RoomSet","id":"r3","name":"old check","members":"b","at":5}));
        assert!(st.rooms_nav["r3"].ephemeral && st.rooms_nav["r3"].closed);
    }
    /// A swarm (docs/SWARM.md): its project's row lists it and leaves its members out; New swarm opens from the project and starts `swarm.create`;
    /// on its page the controls, the add picker and a member's remove are `swarm.act`.
    #[test] fn a_swarm_is_a_project_row_a_form_and_a_page_whose_controls_are_swarm_act() {
        let mut st=State::new(); st.lean=true;
        for e in [json!({"$":"ProjectCreated","id":"p1","title":"Motor","root":"/m","at":1}),
            json!({"$":"ThreadCreated","id":"t1","project":"p1","title":"opus","at":2}), json!({"$":"ThreadCreated","id":"t2","project":"p1","title":"codex","at":2}), json!({"$":"ThreadCreated","id":"t3","project":"p1","title":"Mine","at":2}),
            json!({"$":"RoomSet","id":"s1","name":"debounce","members":"thread:t1\nthread:t2","kind":"swarm","project":"p1","state":"","at":3})] { event(&mut st,e); }
        let v=recent(&st,&params()).unwrap(); let pr=&v["shown"]["projects"][0];
        assert_eq!(pr["swarms"][0]["nav"],"@room:s1"); assert_eq!(pr["swarms"][0]["members"],2);
        let ids: Vec<&str>=pr["threads"].as_array().unwrap().iter().map(|r|r["id"].as_str().unwrap()).collect(); assert_eq!(ids,["h|t3"],"the members are the swarm's");
        // New swarm from the project's menu
        let mut p=params(); p["action"]=json!("swarm-new"); p["value"]=json!("p1");
        let v=recent(&st,&p).unwrap(); assert_eq!(v["shown"]["newSwarm"]["members"].as_array().unwrap().len(),2); assert_eq!(v["shown"]["newSwarm"]["title"],"Motor");
        let mut p=params(); p["session"]=v["session"].clone(); p["session"]["drafts"]["@f.stopic"]=json!("pair"); p["action"]=json!("swarm-start");
        let v=recent(&st,&p).unwrap(); let sent: Value=serde_json::from_str(v["cmds"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!((sent["m"].as_str(),sent["p"]["project"].as_str(),sent["p"]["topic"].as_str()),(Some("swarm.create"),Some("p1"),Some("pair"))); assert!(v["shown"]["newSwarm"].is_null());
        // its page
        let page=json!({"kind":"room","id":"s1","name":"debounce","posts":[],"swarm":{"room":{"id":"s1","state":"active"},"ended":false,"members":[{"member":"thread:t1","thread":"t1","state":"idle"},{"member":"thread:t2","thread":"t2","state":"working"}],"posts":[],"claims":[],"actions":[]}});
        let on_page=|action:&str,value:&str,drafts:Value| { let mut p=params(); p["target"]=json!({"kind":"room","id":"s1"}); p["roomPage"]=page.clone(); p["session"]["drafts"]=drafts; p["action"]=json!(action); p["value"]=json!(value); recent(&st,&p) };
        let v=on_page("","",json!({})).unwrap(); assert_eq!(v["shown"]["bot"]["swarm"]["add"]["open"],false,"the add picker, shut");
        let v=on_page("swarm-add-open","",json!({})).unwrap(); assert_eq!(v["shown"]["bot"]["swarm"]["add"]["open"],true);
        let v=on_page("swarm-act","add|codex|gpt-6|critic",json!({"@sadd":"1"})).unwrap(); let sent: Value=serde_json::from_str(v["cmds"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(sent["m"],"swarm.act"); assert_eq!(sent["p"],json!({"room":"s1","action":"add","provider":"codex","model":"gpt-6","role":"critic"})); assert_eq!(v["session"]["drafts"]["@sadd"],"","Add shuts the picker");
        let v=on_page("swarm-act","remove|t2",json!({})).unwrap(); let sent: Value=serde_json::from_str(v["cmds"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(sent["p"],json!({"room":"s1","action":"remove","thread":"t2"}));
        let v=on_page("swarm-act","stop",json!({})).unwrap(); let sent: Value=serde_json::from_str(v["cmds"][0]["text"].as_str().unwrap()).unwrap(); assert_eq!(sent["p"]["action"],"stop");
        assert!(on_page("swarm-act","remove|t3",json!({})).is_none(),"not a member: refused");
        // its viewer: drawn for its first member (the seam gives that thread's window), so the rail opens Board on the project's design
        let win=json!({"contextMessages":[],"messageIds":[],"historyUnits":[],"partial":[],"ids":[],"anchor":{"id":"","offset":0},"older":"","olderNext":"","hasMore":false,"remaining":0,"newer":0,"total":0});
        let with_window=|action:&str,value:&str| { let mut p=params(); p["target"]=json!({"kind":"room","id":"s1"}); p["roomPage"]=page.clone(); p["window"]=win.clone(); p["action"]=json!(action); p["value"]=json!(value); recent(&st,&p).unwrap() };
        let v=with_window("view","board");
        assert_eq!(v["session"]["sel"],"t1"); assert_eq!(v["session"]["drafts"]["@view:t1"],"board");
        let watch=v["cmds"].as_array().unwrap().iter().filter_map(|c| serde_json::from_str::<Value>(c["text"].as_str()?).ok()).find(|c| c["m"]=="kicad.watch").expect("the board is watched");
        assert_eq!((watch["p"]["kind"].as_str(),watch["p"]["root"].as_str()),(Some("board"),Some("/m")));
        assert_eq!(v["shown"]["bot"]["kind"],"room","the page stays the swarm's");
        // with no window for it (a member the seam could not page) the page is still drawn, with no viewer
        let v=on_page("","",json!({})).unwrap(); assert_eq!(v["session"]["sel"],""); assert!(v["shown"]["thread"].is_null()); assert_eq!(v["shown"]["bot"]["kind"],"room");
    }
    /// A linked machine's bots and rooms are listed after this hub's, each with its machine's marker; with no uberbot here, the linked machine's
    /// leads in Bub's place, and opening it, a far room or a far bot is a target like a bot here (its chat is its thread there).
    #[test] fn a_linked_machines_bots_rooms_and_uberbot_show_here() {
        let mut here=State::new(); here.lean=true;
        for e in [json!({"$":"ThreadCreated","id":"t1","project":"","title":"T","at":1}), json!({"$":"BotSet","id":"b1","name":"Scout","thread":"t1","look":1,"at":2}),
            json!({"$":"RoomSet","id":"r1","name":"Here","members":"b1","at":2}), json!({"$":"RoomSet","id":"r2","name":"Shared","members":"b1\nbub@boat","at":2})] { event(&mut here,e); }
        let mut boat=State::new(); boat.lean=true;
        for e in [json!({"$":"ThreadCreated","id":"L~tb","project":"","title":"Bub","at":1}), json!({"$":"BotSet","id":"L~bub","name":"Bub","thread":"L~tb","look":7,"at":1}),
            json!({"$":"BotSet","id":"L~cal","name":"Calendar","thread":"","look":3,"at":2}), json!({"$":"UberSet","bot":"L~bub","uid":"u","at":3}),
            json!({"$":"RoomSet","id":"L~r9","name":"Ring","members":"L~bub","at":3}), json!({"$":"RoomSet","id":"L~r2","name":"Shared","members":"L~bub\nscout@desk","at":3})] { event(&mut boat,e); }
        let far=vec![("boat-hub".to_string(),&boat)];
        let marks: HashMap<String,Value>=[("boat-hub".to_string(),crate::mirror::mark_of("boat-hub",false))].into();
        let un=Unified{ far:&HashMap::new(), marks:&marks, here:"", rows:&HashMap::new() };
        let v=recent_unified(&here,&far,&un,&params()).unwrap(); let s=&v["shown"];
        let ids: Vec<&str>=s["bots"].as_array().unwrap().iter().map(|b|b["id"].as_str().unwrap()).collect();
        assert_eq!(ids,["h|L~bub","h|b1","h|L~cal"],"the far uberbot leads, then this hub's bots, then the far ones");
        assert_eq!((s["bots"][0]["uber"].as_bool(),s["bots"][0]["nav"].as_str(),s["bots"][0]["mark"]["short"].as_str()),(Some(true),Some("h|L~tb"),Some("boat-hub")));
        assert!(s["bots"][1]["mark"].is_null() && s["bots"][1]["uber"]==false);
        let rooms: Vec<&str>=s["rooms"].as_array().unwrap().iter().map(|r|r["id"].as_str().unwrap()).collect();
        assert_eq!(rooms,["h|r1","h|r2","h|L~r9"],"a room kept on both machines is listed once, as this hub's");
        assert_eq!(s["rooms"][2]["mark"]["name"],"boat-hub");
        // an uberbot of this hub's own stays the one
        event(&mut here,json!({"$":"UberSet","bot":"b1","uid":"u1","at":4}));
        let v=recent_unified(&here,&far,&un,&params()).unwrap();
        assert_eq!(v["shown"]["bots"].as_array().unwrap().iter().filter(|b|b["uber"]==true).map(|b|b["id"].as_str().unwrap()).collect::<Vec<_>>(),["h|b1"]);
        // the far bot and the far room open as targets here
        let mut p=params(); p["action"]=json!("bot"); p["value"]=json!("h|L~bub");
        assert_eq!(recent_unified(&here,&far,&un,&p).unwrap()["nextTarget"],json!({"kind":"bot","id":"L~bub"}));
        let mut p=params(); p["action"]=json!("room"); p["value"]=json!("h|L~r9");
        assert_eq!(recent_unified(&here,&far,&un,&p).unwrap()["nextTarget"],json!({"kind":"room","id":"L~r9"}));
        let mut p=params(); p["action"]=json!("bot"); p["value"]=json!("h|L~nobody");
        assert!(recent_unified(&here,&far,&un,&p).is_none(),"an unknown far bot is refused");
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
    #[test] fn a_swarm_page_and_the_new_swarm_form_get_the_models_info() {
        assert!(shows_swarm_models(&json!({"target":{"kind":"room","id":"s1"},"roomPage":{"swarm":{}}}))); assert!(!shows_swarm_models(&json!({"target":{"kind":"room","id":"r1"},"roomPage":{}})), "a plain room lists no models");
        assert!(shows_swarm_models(&json!({"target":{"kind":"recent"},"action":"swarm-new"}))); assert!(shows_swarm_models(&json!({"target":{"kind":"recent"},"session":{"drafts":{"@snew":"p1"}}})));
        assert!(!shows_swarm_models(&json!({"target":{"kind":"recent"},"session":{"drafts":{"@snew":""}}})));
    }
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
