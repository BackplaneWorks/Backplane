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
use std::{collections::HashSet, sync::{Arc, OnceLock, Weak}};

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
        let withinfo; let params=if super::settings::is_open(&params["session"]) || params["target"]["kind"]=="thread" || setup { let mut c=params.clone(); c["hubInfo"]=_hub.info_now(); c["hubInfo"]["client"]=params["client"].clone(); if super::settings::page(&params["session"])=="devices" { if let Some(r)=crate::net::start::routes() { c["hubInfo"]["devices"]=r.reg.list(); } } withinfo=c; &withinfo } else { params };
        match policies.with_state(|st|recent(st,params)) { Some(out)=>out,None=>refused("state.booting") }
    }
}
fn mine(p: &Value) -> bool {
    let id=text(&p["target"],"id");
    p["nativeSidebar"]==json!(1) && match text(&p["target"],"kind") { "recent"=>id.is_empty(), "thread"=>!id.is_empty() && !id.contains('~'), "bot"=>p["botPage"].is_object(), "room"=>p["roomPage"].is_object(), _=>false }
}
fn setting<'a>(st: &'a State,k: &str) -> &'a str { st.settings.get(k).map(String::as_str).unwrap_or("") }
/// Fuzzy.score > 0: every character of the query is in the title, in order, whatever the case (an empty query matches all)
pub fn fuzzy_hit(title: &str,q: &str) -> bool {
    let mut t=title.chars().flat_map(char::to_lowercase);
    q.chars().flat_map(char::to_lowercase).all(|c| t.any(|x| x==c))
}
fn flag(s: &Value,k: &str) -> bool { s["open"][k].as_bool().unwrap_or(false) }
fn refused<T>(reason: &'static str) -> Option<T> { super::refusal::report(reason); None }

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
    let opts=sidebar::Opts { hub:text(p,"hub"),now:p["now"].as_u64().unwrap_or(0)/1000,selected:"",open:&open,older_open:true };
    let plan=sidebar::plan(st,&opts);
    let first=plan.ids().into_iter().find(|id| q.is_empty() || st.projects.get(*id).map_or(false,|pr|fuzzy_hit(&pr.title,&q)))?.to_string();
    let top=st.threads.iter().filter(|(_,t)|t.project==first && !t.archived && !t.deleted).max_by_key(|(id,t)|(t.pinned,t.activity,id.as_str().to_string()))?.0.clone();
    let mut c=p.clone(); c["action"]=json!("select"); c["value"]=json!(format!("{}|{top}",text(p,"hub")));
    c["session"]["drafts"]["@f.pq"]=json!("");
    Some(c)
}
pub fn recent(st: &State,p: &Value) -> Option<Value> {
    if text(p,"action")=="proj-go" { match find_go(st,p) { Some(c)=>return recent(st,&c), None=>{ let mut c=p.clone(); c["action"]=json!("proj-find-q"); c["value"]=json!(""); return recent(st,&c) } } }
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
        crate::session::patch::merge(&mut session,&o.patch); cmds=[read_cmds.clone(),o.cmds].concat();
    } else if let Some(o)=super::botforms::act(st,hub,&session,action,value) {
        crate::session::patch::merge(&mut session,&o.patch); cmds=[read_cmds.clone(),o.cmds].concat();
    } else if let Some(o)=super::settings::act(hub,&session,action,value) {
        crate::session::patch::merge(&mut session,&o.patch); cmds=[read_cmds.clone(),o.cmds].concat();
    } else { match action {
        "" | "native-sidebar-page" => {},
        // the sidebar's search box: the query is kept in the session (`@f.pq`); "proj-find" (the phones' toggle) changes nothing here
        "proj-find-q" => { session["drafts"]["@f.pq"]=json!(value); },
        "proj-find" => {},
        "flag" | "side-older" | "side-settled" => {
            let raw=value.strip_prefix(&format!("{hub}|" )).unwrap_or(value);
            let key=match action { "side-older"=>"old-projects", "side-settled"=>"active-settled", _=>raw };
            let known=key=="old-projects" || key=="active-settled" || st.projects.contains_key(key.strip_prefix("arch:").unwrap_or(key));
            if !known { return refused("action.sidebar-flag") }
            if !session["open"].is_object() { session["open"]=json!({}); }
            let was=flag(&session,key); session["open"][key]=json!(!was);
        },
        // the viewer panel opened on a drawing (or closed): the watch, the project card's look, the boards (present/viewer.rs); other pages are not ported
        "view" => {
            let Some(o)=super::viewer::view_action(st,hub,&session,value) else { return refused("action.view-unported") };
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
            let Some(o)=super::viewer::files_action(st,hub,&session,action,value) else { return refused("action.files-unported") };
            crate::session::patch::merge(&mut session,&o.patch);
            cmds=o.before;
        },
        // another board of a project with several (present/viewer.rs `board_action`)
        "view-board" => {
            let Some(o)=super::viewer::board_action(st,hub,&session,value) else { return refused("action.board-unported") };
            crate::session::patch::merge(&mut session,&o.patch);
            cmds=o.before;
        },
        // the Layers menu: hide/show a layer, the active layer, All/Dim/Only, Ctrl+H, Reset layers (present/viewer.rs `layer_action`)
        "view-layer" | "view-layer-pick" | "view-layer-mode" | "view-layer-cycle" | "view-layers-reset" => {
            let Some(patch)=super::viewer::layer_action(st,&session,action,value) else { return refused("action.layer-unported") };
            crate::session::patch::merge(&mut session,&patch);
        },
        // a sheet of the schematic (present/viewer.rs `sheet_action`)
        "view-sheet" => {
            let Some(o)=super::viewer::sheet_action(st,hub,&session,value) else { return refused("action.sheet-unported") };
            crate::session::patch::merge(&mut session,&o.patch);
            cmds=o.before;
        },
        _ => return refused("action.unported"),
    } }
    // Dv.sync: an open Diff follows its thread
    if !thread.is_empty() { crate::session::pages::dv_sync(&crate::session::dispatch::StateFacts(st),hub,&mut session,&mut cmds); }
    // Fab.sync for every request on a thread, view.open included: the window opens a thread with view.open (no select action), and the card
    // (Buy it in the header) comes only from fab.look. fab_tail asks only when where the thread stands changed since the last ask (@fab.at).
    if !thread.is_empty() { crate::session::dispatch::fab_tail(&crate::session::dispatch::StateFacts(st),hub,&mut session,&mut cmds); }
    // A thread is drawn only when the async session seam supplied a native window and indexed page.
    // Composer fields are drawn from the same adopted draft that the action dispatcher patches.
    let shown_thread=if thread.is_empty() { Value::Null }else{
        if !p["window"].is_object() { return refused("window.missing") }
        let mut params=p.clone(); params["session"]=session.clone();
        match super::thread::thread_object_checked(st,hub,thread,&params,text(&session["drafts"],thread)) {
            Ok(value)=>value, Err(reason)=>return refused(reason),
        }
    };
    let local_mode=text(&session["drafts"],"@local.view");
    let mode: &'static str=if local_mode=="projects" { "projects" } else if local_mode=="active" || setting(st,"sidebar.view")=="active" { "active" } else { "projects" };
    let query=text(&session["drafts"],"@f.pq").trim().to_string();
    let Some(mut ns)=NativeSidebar::prepare(p,mode,&query,&session["open"],st.projects.keys().cloned().collect()) else { return refused("sidebar.key-or-capability") };
    if action=="native-sidebar-page" && !ns.accept(value) { return Some(json!({"refused":true,"local":p.get("local").cloned().unwrap_or(json!({}))})) }
    if action=="native-sidebar-page" { session["drafts"]["@menu"]=json!(""); session["drafts"]["@del"]=json!(""); }
    let open: HashSet<String>=session["open"].as_object().into_iter().flatten().filter(|(_,v)|v.as_bool()==Some(true)).map(|(k,_)|k.clone()).collect();
    // a search unfolds the older projects too (Side.unfolded)
    let opts=sidebar::Opts { hub,now:p["now"].as_u64().unwrap_or(0)/1000,selected:thread,open:&open,older_open:flag(&session,"old-projects") || !query.is_empty() };
    let plan=sidebar::plan(st,&opts);
    let selected_project=st.threads.get(thread).map(|t|t.project.as_str()).unwrap_or("");
    // Find.rows: only the projects whose name fuzzy-matches; the one holding the open thread stays (Side.cur.held)
    let ids: Vec<&str>=plan.ids().into_iter().filter(|id| query.is_empty() || *id==selected_project || st.projects.get(*id).map_or(false,|p|fuzzy_hit(&p.title,&query))).collect();
    let project_limit=ns.project_limit();
    // For now an explicitly opened thread in a folded Older project is unported. The recent list
    // already offers only visible projects; forcing an excluded project needs the filter exception port.
    // (a removed project's thread still draws, with no row for it)
    if mode!="active" && !selected_project.is_empty() && !ids.contains(&selected_project) && st.projects.get(selected_project).map_or(false,|p|!p.removed) { return refused("sidebar.selected-project-hidden") }
    let ids: Vec<&str>=if mode=="active" { vec![] } else { ids };
    let (mut indices,projects_page)=ns.slice_ids(&ids,"projects","",project_limit,selected_project);
    indices.sort_unstable(); // Mob.native.keep_projects keeps source order, including the selected exception.
    let selected: Vec<&str>=indices.into_iter().map(|i|ids[i]).collect();
    let row_limit=(ns.row_budget()/plan.list_count(&selected).max(1)).max(1);
    let projects: Vec<Value>=selected.into_iter().filter_map(|owner| {
        let mut project=plan.project(owner,&mut |kind,ids| {
            let (mut indices,pager)=ns.slice_ids(ids,kind,owner,row_limit,thread);
            indices.sort_unstable(); // Mob.native.tree_rows filters in tree order.
            (indices,pager)
        })?;
        let icon=setting(st,&format!("project.icon.{owner}"));
        project["icon"]=json!(super::badge::icon_of(icon));
        let color=super::badge::color_of(setting(st,&format!("project.color.{owner}")),owner);
        project["color"]=json!(color); project["css"]=json!(super::badge::css_of(color));
        if text(&session["drafts"],"@look")==owner { project["look"]=super::badge::editor(owner,setting(st,&format!("project.color.{owner}")),icon); }
        Some(project)
    }).collect();
    let ren=text(&session["drafts"],"@ren");
    let renaming=if ren.is_empty() || text(&session["drafts"],"@ren.inline")=="1" { Value::Null }else{json!({"id":format!("{hub}|{ren}"),"title":if text(&session["drafts"],"@ren.kind")=="project" {"Rename project"} else {"Rename thread"},"text":text(&session["drafts"],"@f.title"),"yes":"Rename","no":"Cancel"})};
    let del=text(&session["drafts"],"@del");
    let deleting=if del.is_empty() { Value::Null }else{json!({"id":if del.contains('|'){del.to_string()}else{format!("{hub}|{del}")},"title":"Delete thread","body":st.threads.get(del.strip_prefix(&format!("{hub}|" )).unwrap_or(del)).map(|t|t.title.as_str()).unwrap_or(""),"yes":"Delete","no":"Cancel"})};
    let pages=json!([{"version":1,"hub":hub,"projects":projects_page,"active":null,"legacy":null}]);
    let shown=json!({"projects":projects,"sidebarPages":pages,"nativeSidebar":1,"active":[],"legacy":[],"thread":shown_thread,"rooms":rooms(st,hub),"bots":bots(st,hub,opts.now),"theme":setting(st,"ui.theme"),
        "renaming":renaming,"deleting":deleting,"bot":null,"settings":if super::settings::is_open(&session) { super::settings::view(st,&session,p.get("hubInfo").unwrap_or(&Value::Null)) }else{Value::Null},"newRoom":super::botforms::new_room(st,hub,&session).unwrap_or(Value::Null),"newBot":super::botforms::new_bot(&session).unwrap_or(Value::Null),"projectHome":null});
    let mut shown=shown;
    // the Active view: what needs the person first, then the recently settled folded under "Settled N" (a search shows them)
    if mode=="active" {
        let (live,faded)=plan.active(&|title,proj| query.is_empty() || fuzzy_hit(title,&query) || fuzzy_hit(proj,&query));
        let open=flag(&session,"active-settled") || !query.is_empty();
        shown["active"]=json!(live);
        shown["activeSettled"]=json!({"count":faded.len(),"open":open,"rows":if open { faded } else { vec![] }});
    }
    // the Older group: a row that opens or shuts it (a search opens it by itself)
    if mode!="active" && query.is_empty() && plan.older_count()>0 { shown["olderProjects"]=json!({"count":plan.older_count(),"open":flag(&session,"old-projects")}); }
    // the query in force, said back once the box has been used (the client takes it when it is not what it sent: Enter clears it)
    if session["drafts"].as_object().map_or(false,|d|d.contains_key("@f.pq")) { shown["search"]=json!({"query":query,"hint":"Search projects"}); }
    { let f=picker::folders(&session["drafts"],&std::env::var("HOME").unwrap_or_default(),&st.projects.values().filter(|p|!p.removed).map(|p|p.root.as_str()).collect::<Vec<_>>()); if !f.is_null() { shown["folders"]=f; } } // (present only while the picker is open)
    if !thread.is_empty() { shown["stats"]=super::keys::stats(st,thread); } // the Context panel's rows (beside the thread: its object is the helper's, key for key)
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
        // approved difference: a project carries its badge colour (`color`, `css`; present/badge.rs), which 0.14's recording does not have
        for path in ["/shown/projects","/overlay/j/projects"] { for pr in actual.pointer_mut(path).and_then(|v| v.as_array_mut()).into_iter().flatten() { if let Some(o)=pr.as_object_mut() { o.remove("color"); o.remove("css"); } } }
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
            // the Allowance chip's text (keys.rs `allowance`) and the picker's other agents (`pickerAgents`) are Backplane's own: the recorded helper reply has no such keys
            let bare=|mut t:Value|{ for k in ["allowance","pickerAgents"] { t.as_object_mut().unwrap().remove(k); } t };
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
