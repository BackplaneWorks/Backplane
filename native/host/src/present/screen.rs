//! The GPUI recent and plain-thread root. Borrows the state policies' lean fold; never replays or clones a log.
//! Approved differences for this slice: unread shown keys `bubHomes`, `online`, `version`, `error`,
//! `note`, `sel`, `making`, `lang`, `empty`, `older`, `settled`, `view`, `active`, `legacy`, `rowMenu`,
//! `removing`, `search`, `folders`, `tour`, `starNudge`, `find`, `island`, `islands`, `uber`, `hub`,
//! `hubs`, `usageMachines`, `found`, `tabs` are omitted. They belong to other clients/screens.
//! Overlay is the same reduced root (without the aggregate's `machine` field on projects),
//! with its hub key/name. Unsupported targets/actions return None, so Routed supplies a retryable error.
use super::native_sidebar::{text, NativeSidebar};
use crate::{hub::Hub, sidebar, state::State};
use serde_json::{json, Value};
use std::{collections::HashSet, sync::{Arc, OnceLock, Weak}};

#[derive(Clone, Default)]
pub struct Recent { hub: Arc<OnceLock<Weak<Hub>>> }
impl Recent {
    pub fn new() -> Self { Self::default() }
    pub fn bind(&self, hub: &Arc<Hub>) { let _ = self.hub.set(Arc::downgrade(hub)); }
}
impl super::Screen for Recent {
    fn name(&self) -> &'static str { "recent/thread (native sidebar)" }
    fn view(&self, params: &Value) -> Option<Value> {
        if !mine(params) { return None }
        let _hub = self.hub.get()?.upgrade()?;
        crate::statehost::policies()?.with_state(|st| recent(st, params)).flatten()
    }
}
fn mine(p: &Value) -> bool {
    let id=text(&p["target"],"id");
    p["nativeSidebar"]==json!(1) && match text(&p["target"],"kind") { "recent"=>id.is_empty(), "thread"=>!id.is_empty() && !id.contains('~'), _=>false }
}
fn setting<'a>(st: &'a State,k: &str) -> &'a str { st.settings.get(k).map(String::as_str).unwrap_or("") }
fn flag(s: &Value,k: &str) -> bool { s["open"][k].as_bool().unwrap_or(false) }

fn bots(st: &State, hub: &str, now: u64) -> Vec<Value> {
    let mut bots: Vec<_>=st.bots_nav.iter().filter(|(id,_)|!id.contains('~')).collect();
    bots.sort_by(|(ia,a),(ib,b)| a.created.cmp(&b.created).then(ia.cmp(ib)));
    bots.iter().map(|(id,b)| {
        let name=if bots.iter().filter(|(_,other)|other.name.eq_ignore_ascii_case(&b.name)).count()>1 { format!("{} ({id})",b.name) }else{b.name.clone()};
        let t=st.threads.get(&b.thread);
        let (mood,note)=if st.asks.values().any(|(th,open)| th==&b.thread && *open) || t.map_or(false,|t|t.pending>0) { ("wait","needs you") }
            else if t.map_or(false,|t|t.turn==crate::state::Turn::Running) || st.queue.get(&b.thread).map_or(false,|q|!q.is_empty()) {
                if now<=b.talked.saturating_add(60) { ("talk","talking") }else{("work","working")}
            } else if t.map_or(false,|t|t.turn==crate::state::Turn::Failed) { ("error","failed") }
            else if now>t.map_or(0,|t|t.activity).saturating_add(900) { ("sleep","asleep") }else{("idle","ready")};
        json!({"id":format!("{hub}|{id}"),"name":name,"title":name,"act":if b.thread.is_empty(){"remote"}else{"bot"},
            "look":b.look,"mood":mood,"note":note,"sel":false,"remote":false,"peer":"","cat":format!("{}:{mood}",b.look),"key":id,"link":"",
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
pub fn recent(st: &State,p: &Value) -> Option<Value> {
    if !mine(p) { return None }
    let mut session=p.get("session").filter(|s|s.is_object()).cloned().unwrap_or(json!({}));
    // Ui.blank + Target.bind(recent). The actor reshapes this adopted session after presentation.
    for (k,v) in [("sel",json!("")),("adding",json!(false)),("panel",json!(false)),("rid",json!(0)),
        ("wantNew",json!(false)),("error",json!("")),("note",json!("")),("info",json!({})),("open",json!({})),("drafts",json!({})),("outbox",json!({}))] {
        session.as_object_mut()?.entry(k.to_string()).or_insert(v);
    }
    let thread=if p["target"]["kind"]=="thread" { text(&p["target"],"id") }else{""};
    session["sel"]=json!(thread);
    if !session["drafts"].is_object() { session["drafts"]=json!({}); }
    for k in ["@boot","@bot","@bot.want","@btab","@del","@desk","@draft.bound","@mech.at","@presentation","@remote","@room","@solid.at","@telemetry.dismiss","@thread.want"] {
        session["drafts"].as_object_mut()?.entry(k.to_string()).or_insert(json!(""));
    }
    session["drafts"].as_object_mut()?.entry("@sync".to_string()).or_insert(json!("1"));
    let action=text(p,"action"); let value=text(p,"value"); let hub=text(p,"hub");
    // The shared pure dispatcher owns composer/commands/select; this root owns private sidebar paging.
    let mut adopted=p.clone(); adopted["session"]=session.clone();
    let mut cmds=vec![];
    let dispatched=crate::session::dispatch::dispatch(&crate::session::dispatch::StateFacts(st),&adopted);
    if let Some(out)=dispatched {
        session=out.session; cmds=out.cmds;
        if let Some(target)=out.next_target {
            return Some(json!({"remoteReady":false,"nextTarget":target,"session":session,"cmds":cmds,
                "local":p.get("local").cloned().unwrap_or(json!({"local.native.sidebar.pages":"","local.native.sidebar.context":"","local.native.sidebar.key":"","local.native.sidebar.budget":""}))}));
        }
    } else { match action {
        "" | "native-sidebar-page" => {},
        "flag" | "side-older" => {
            let raw=value.strip_prefix(&format!("{hub}|" )).unwrap_or(value);
            let key=if action=="side-older" { "old-projects" } else { raw };
            let known=key=="old-projects" || st.projects.contains_key(key.strip_prefix("arch:").unwrap_or(key));
            if !known { return None }
            if !session["open"].is_object() { session["open"]=json!({}); }
            let was=flag(&session,key); session["open"][key]=json!(!was);
        },
        _ => return None,
    } }
    // A thread is drawn only when the async session seam supplied a native window and indexed page.
    // The first thread cut supports an empty draft. Refusing after dispatch also refuses a draft action
    // that would otherwise acknowledge text while drawing a blank composer.
    let shown_thread=if thread.is_empty() { Value::Null }else{
        if !p["window"].is_object() || !text(&session["drafts"],thread).is_empty() { return None }
        let mut params=p.clone(); params["session"]=session.clone();
        super::thread::thread_object(st,hub,thread,&params,"")?
    };
    let local_mode=text(&session["drafts"],"@local.view");
    let mode=if ["projects","active"].contains(&local_mode) { local_mode } else if setting(st,"sidebar.view")=="active" { "active" } else { "projects" };
    // Active list and search are separate port slices; claiming them would hide projects silently.
    if mode!="projects" || !text(&session["drafts"],"@f.pq").is_empty() { return None }
    let mut ns=NativeSidebar::prepare(p,mode,"",&session["open"],st.projects.keys().cloned().collect())?;
    if action=="native-sidebar-page" && !ns.accept(value) { return Some(json!({"refused":true,"local":p.get("local").cloned().unwrap_or(json!({}))})) }
    if action=="native-sidebar-page" { session["drafts"]["@menu"]=json!(""); session["drafts"]["@del"]=json!(""); }
    let open: HashSet<String>=session["open"].as_object().into_iter().flatten().filter(|(_,v)|v.as_bool()==Some(true)).map(|(k,_)|k.clone()).collect();
    let opts=sidebar::Opts { hub,now:p["now"].as_u64().unwrap_or(0)/1000,selected:thread,open:&open,older_open:flag(&session,"old-projects") };
    let plan=sidebar::plan(st,&opts);
    let ids=plan.ids(); let project_limit=ns.project_limit();
    let selected_project=st.threads.get(thread).map(|t|t.project.as_str()).unwrap_or("");
    // For now an explicitly opened thread in a folded Older project is unported. The recent list
    // already offers only visible projects; forcing an excluded project needs the filter exception port.
    if !selected_project.is_empty() && !ids.contains(&selected_project) { return None }
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
        if ["folder","circuit-board","cpu","zap","radio","box","layers","code","terminal","globe","wrench","flask-conical","book","star"].contains(&icon) { project["icon"]=json!(icon); }
        Some(project)
    }).collect();
    let ren=text(&session["drafts"],"@ren");
    let renaming=if ren.is_empty() || text(&session["drafts"],"@ren.inline")=="1" { Value::Null }else{json!({"id":format!("{hub}|{ren}"),"title":"Rename thread","text":text(&session["drafts"],"@f.title"),"yes":"Rename","no":"Cancel"})};
    let del=text(&session["drafts"],"@del");
    let deleting=if del.is_empty() { Value::Null }else{json!({"id":if del.contains('|'){del.to_string()}else{format!("{hub}|{del}")},"title":"Delete thread","body":st.threads.get(del.strip_prefix(&format!("{hub}|" )).unwrap_or(del)).map(|t|t.title.as_str()).unwrap_or(""),"yes":"Delete","no":"Cancel"})};
    let pages=json!([{"version":1,"hub":hub,"projects":projects_page,"active":null,"legacy":null}]);
    let shown=json!({"projects":projects,"sidebarPages":pages,"nativeSidebar":1,"thread":shown_thread,"rooms":rooms(st,hub),"bots":bots(st,hub,opts.now),"theme":setting(st,"ui.theme"),
        "renaming":renaming,"deleting":deleting,"bot":null,"settings":null,"newRoom":null,"newBot":null,"projectHome":null});
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
    const OMITTED: &[&str]=&["bubHomes","online","version","error","note","sel","making","lang","empty","older","settled","view","active","legacy","rowMenu","removing","search","folders","tour","starNudge","find","island","islands","uber","hub","hubs","usageMachines","found","tabs"];
    const OVERLAY_OMITTED: &[&str]=&["machineUsage","tabs","online","version","seq","error","bubHome","lang","note","sel","making","empty","legacy","older","view","active","settled","rowMenu","search","removing","tour","starNudge","find","folders","island","islandAt","uber","plane"];
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
        let actual=recent(&st,&p).expect("recorded recent request is ported");
        let mut shown=expected["shown"].clone(); retain_root(&mut shown,OMITTED);
        assert_eq!(actual["shown"],shown,"retained GPUI shown fields differ (no value normalisations)");
        let mut overlay=expected["overlay"].clone(); retain_root(&mut overlay["j"],OVERLAY_OMITTED);
        assert_eq!(actual["overlay"],overlay,"retained overlay fields differ");
        for k in ["remoteReady","session","cmds","window"] { assert_eq!(actual[k],expected[k],"{k}"); }
    }
    #[test] fn recorded_js_recent_gpui() { oracle(include_str!("fixtures/session-gpui-recent.json")); }
    #[test] fn recorded_js_recent_gpuid() { oracle(include_str!("fixtures/session-gpuid-recent.json")); }
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
            json!({"$":"SettingSet","key":"usage.t","value":"not kept","at":4})] { event(&mut st,e); }
        let v=recent(&st,&params()).unwrap(); let s=&v["shown"];
        assert_eq!(s["theme"],"light"); assert_eq!(s["projects"][0]["icon"],"cpu");
        assert_eq!(s["bots"][0]["id"],"h|b"); assert_eq!(s["bots"][0]["name"],"Cat"); assert_eq!(s["bots"][0]["look"],42);
        assert_eq!(s["rooms"].as_array().unwrap().len(),1); assert_eq!(s["rooms"][0]["members"],2);
        assert!(!st.settings.contains_key("usage.t") && st.entries.is_empty());
        event(&mut st,json!({"$":"BotSet","id":"b","name":"Edited","thread":"","look":8,"at":5}));
        assert_eq!(st.bots_nav["b"].thread,"t"); assert_eq!(st.bots_nav["b"].created,2);
        event(&mut st,json!({"$":"BotDeleted","id":"b","at":6})); event(&mut st,json!({"$":"RoomDeleted","id":"r","at":6}));
        let v=recent(&st,&params()).unwrap(); assert_eq!(v["shown"]["bots"],json!([])); assert_eq!(v["shown"]["rooms"],json!([]));
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
        let pair:Value=serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/gpui/slim/fixtures/thread-pair.json"))).unwrap();
        let mut st=State::new(); st.lean=lean;
        for line in include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/gpui/slim/fixtures/thread-log.jsonl")).lines() { st.apply_line(line); }
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
            assert_eq!(out["shown"]["thread"],expected); assert_eq!(out["overlay"]["j"]["thread"],expected);
            assert_eq!(out["session"]["sel"],p["target"]["id"]); assert_eq!(out["window"],p["window"]);
            assert_eq!(out["shown"]["nativeSidebar"],1); assert_eq!(out["cmds"],json!([]));
        }
    }
    #[test] fn thread_root_refuses_missing_indexed_inputs_nonempty_drafts_and_unported_config() {
        let (mut st,p,_)=fixture(true); let id=text(&p["target"],"id").to_string();
        for key in ["page","window"] { let mut q=p.clone(); q.as_object_mut().unwrap().remove(key); assert!(recent(&st,&q).is_none(),"{key}"); }
        let mut q=p.clone(); q["session"]["drafts"][&id]=json!("typing"); assert!(recent(&st,&q).is_none());
        let mut q=p.clone(); q["action"]=json!("draft"); q["value"]=json!("typing"); assert!(recent(&st,&q).is_none(),"a draft cannot be acknowledged with an empty picture");
        st.apply(&json!({"$":"SettingSet","key":"think","value":"1","at":1}));
        assert!(recent(&st,&p).is_none(),"an unported setting survives the lean fold and refuses the static thread");
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
