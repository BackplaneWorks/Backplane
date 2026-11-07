use crate::{delta,wire};
use anyhow::{ensure, Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{collections::HashMap, path::PathBuf, time::{Duration, SystemTime, UNIX_EPOCH}};
use tokio::sync::mpsc::{self, Sender};
use tokio_tungstenite::tungstenite::{Message, protocol::WebSocketConfig};

#[derive(Debug)] pub enum Command { Rpc(String,crate::screens::settings::Secret,&'static str), Sleep(f64), Select(String), Send(String), Draft(String), Action(String,String), Page(String), Ask(String,Value,u64), /** read this history page of the thread, then stand on the newest again */ History(String) }
#[derive(Debug)] pub enum Event { /** the hub put this text in a thread's composer */ Compose(String,String), /** a line that is no error (Intent saved) */ Note(String), /** the hub refused this device's credential: it was revoked */ Unpaired, /** a terminal's output: key, pid (None in a snapshot), bytes, and a snapshot's cols x rows */ Term(String,Option<i32>,Vec<u8>,Option<(usize,usize)>), /** a new connection's greeting: what a terminal printed meanwhile is lost */ Hello, /** the browser helper's tabs of an owner's page (info `browser.tabs.<owner>`, JSON text) */ BrowserTabs(String,String), PlotTooBig(usize), /** a direct request's result (the accounts snapshot) */ Data(&'static str,Value), /** a `{"t":"voice"}` frame (screens/voice.rs) */ Voice(Value), /** history.steps' answer */ Steps(Value), /** history.refs' answer: git dir, top, refs lines, board, schematic */ Refs(Value), /** a cross-probe's pieces on canvas slot: (chunk, piece) */ Probe(usize,Vec<(usize,usize)>), /** a schematic's sheets: rows `depth<TAB>leaf<TAB>name<TAB>file` per line */ Sheets(String), /** an agent's viewer_show: thread, kind, path */ ViewShow(String,String,String), /** what a tapped piece is (kicad.info's answer) */ Pick(String), Rpc(&'static str,bool,String), Answer(u64,Value), Plot(Vec<u8>), NetWake, Picture(Value,Timing), Status(String), Accepted, /** a history page read for this cursor (None: its answer went stale) */ Older(String,Option<Value>), /** the star suggestion the hub shows now (null: none) */ Star(Value), Error(String), /** the hub answered, and said no: shown, but the connection is fine */ Refused(String) }
impl Event {
    /// What the performance overlay calls it.
    pub fn name(&self)->&'static str {match self {Event::Term(..)=>"term",Event::Hello=>"hello",Event::Star(_)=>"star",Event::BrowserTabs(..)=>"browser tabs",Event::PlotTooBig(_)=>"plot too big",Event::Data(..)=>"data",Event::Voice(_)=>"voice",Event::Steps(_)=>"steps",Event::Refs(_)=>"refs",Event::Probe(..)=>"probe",Event::Sheets(_)=>"sheets",Event::ViewShow(..)=>"view show",Event::Pick(_)=>"pick",Event::Rpc(..)=>"rpc",Event::Answer(..)=>"answer",Event::Plot(_)=>"plot",Event::NetWake=>"net wake",Event::Picture(..)=>"picture",Event::Status(_)=>"status",Event::Accepted=>"accepted",Event::Older(..)=>"older",Event::Error(_)=>"error",Event::Refused(_)=>"refused",Event::Unpaired=>"unpaired",Event::Compose(..)=>"compose",Event::Note(_)=>"note"}}
}
/// The connection label an event sets (None: it leaves the label alone). A refusal is a live connection, so it is not "Disconnected".
pub fn status_for(ev:&Event)->Option<&'static str>{match ev{Event::Error(_)=>Some("Disconnected"),_=>None}}
#[cfg(test)] mod status_tests{use super::*;use core::prelude::v1::test;
    #[test] fn a_refused_view_or_command_leaves_the_connection_label_alone(){assert_eq!(status_for(&Event::Refused("View request refused: \"PRESENTER_UNAVAILABLE\"".into())),None);assert_eq!(status_for(&Event::Error("Hub disconnected".into())),Some("Disconnected"));assert_eq!(status_for(&Event::Accepted),None);}}
/// Where a reply's time went before the UI saw it: send to frame received, decode, delta/merge, the body's size, and when its frame arrived.
#[derive(Debug,Clone,Default)] pub struct Timing { pub rtt_ms:f64, pub decode_ms:f64, pub process_ms:f64, pub bytes:usize, pub delta:bool, pub at:Option<std::time::Instant> }
#[derive(Clone)] pub struct Config { pub hub:String, pub token_file:PathBuf, pub exit_after:Option<f64>, pub script:Option<PathBuf>, pub swatches:bool, pub gallery:bool, pub fonts:bool, pub solid_demo:Option<String>, pub theme:Option<String>, /** a pairing credential is what connects: a 401 means this device was revoked */ pub paired:bool, /** no hub to connect to: the client starts at pairing */ pub unpaired:bool, /** a link from outside, to ask about */ pub pair_link:Option<String>, /** no --url, no pairing: the launcher (launch.rs) finds or starts this machine's hub, or the chooser asks */ pub pending:bool, /** this client started or found its own hub */ pub launched:bool }
impl Config {
    pub fn parse() -> Result<Self> {
        let home=crate::plat::user_home().context("no home folder ($HOME)")?;
        let mut config=Self{hub:"ws://100.64.84.41:3799".into(),token_file:PathBuf::from(&home).join(".backplane-gpui/token"),exit_after:None,swatches:false,gallery:false,fonts:false,solid_demo:None,theme:None,paired:false,unpaired:false,pair_link:None,pending:false,launched:false,script:std::env::var_os("BACKPLANE_GPUI_SCRIPT").map(PathBuf::from)};
        let (mut args,mut named,mut keyed)=(std::env::args().skip(1),false,false);
        while let Some(arg)=args.next() { match arg.as_str() {
            "--hub"|"--url"=>{config.hub=args.next().context("--url needs a URL")?;named=true;},
            "--pair"=>config.pair_link=Some(args.next().context("--pair needs a link")?),
            link if crate::pairing::is_link(link)=>config.pair_link=Some(link.to_string()),
            "--exit-after"=>config.exit_after=Some(args.next().context("--exit-after needs seconds")?.parse()?),
            "--swatches"=>config.swatches=true,
            "--gallery"=>config.gallery=true,
            "--fonts"=>config.fonts=true,
            "--solid-demo"=>config.solid_demo=Some(args.next().context("--solid-demo needs TRIS[,STEPS]")?),
            "--theme"=>config.theme=Some(args.next().context("--theme needs light|dark")?),
            "--script"=>config.script=Some(PathBuf::from(args.next().context("--script needs a file")?)),
            "--token-file"=>{config.token_file=PathBuf::from(args.next().context("--token-file needs a path")?);keyed=true;},
            _=>anyhow::bail!("usage: backplane-gpui [LINK] [--pair LINK] [--url http://HOST:PORT] [--token-file FILE] [--exit-after SECONDS] [--script FILE] [--swatches] [--gallery] [--fonts] [--solid-demo TRIS[,STEPS]] [--theme light|dark]")
        }}
        // no hub named: the one this client paired with, or (no hub's token here either) pairing
        let (saved,saved_token)=match crate::hubs::target_saved() {crate::hubs::Target::Remote{key,ws}=>(Some(ws),crate::hubs::credential_saved(&key)),crate::hubs::Target::Local=>(None,None)};
        match unnamed(named,keyed,saved,config.token_file.exists()) {
            Start::Paired(hub)=>{config.hub=hub;config.token_file=saved_token.context("no config folder")?;config.paired=true;return Ok(config);}
            Start::Unpaired if config.pair_link.is_none()&&config.script.is_none()&&!(config.swatches||config.gallery||config.fonts||config.solid_demo.is_some())=>{config.pending=true;return Ok(config);} // a first launch: setup's chooser (screens/first.rs); a link or a script goes to pairing, where the script runner is
            Start::Unpaired=>{config.hub.clear();config.token_file=crate::hubs::credential_saved("unpaired").context("no config folder")?;config.unpaired=true;return Ok(config);}
            Start::Named=>{ if named&&keyed {crate::hubs::remember_saved(&config.hub,&config.token_file.to_string_lossy());} }
        }
        config.hub=config.hub.trim_end_matches('/').to_string();
        config.hub=crate::pairing::ws_of(&config.hub);
        ensure!((config.hub.starts_with("ws://")||config.hub.starts_with("wss://")) && !config.hub.contains(['?','#','@']) && !config.hub.trim_end_matches('/').ends_with(":3787"),"experiment needs an explicit ws:// or wss:// hub on a separate port");
        let token_path=std::fs::canonicalize(&config.token_file).context("experiment token file missing; start its hub first")?;
        ensure!(!token_path.starts_with(PathBuf::from(home).join(".backplane-bend")),"live home refused");
        Ok(config)
    }
}
#[derive(Debug,PartialEq)] enum Start {Named,Paired(String),Unpaired}
/// Which hub a start with no --url means. A named hub or token file is that; then the saved pairing; then a hub's own token on this machine; else there is none.
fn unnamed(named:bool,keyed:bool,saved:Option<String>,token_here:bool)->Start {
    if named||keyed {return Start::Named;}
    match saved {Some(hub)=>Start::Paired(hub),None if token_here=>Start::Named,None=>Start::Unpaired}
}
#[cfg(test)] mod start_tests { use super::*;
    #[test] fn a_start_names_its_hub_else_the_pairing_else_pairing_begins(){
        let s=||Some("wss://b.ts.net".to_string());
        assert_eq!(unnamed(true,true,s(),true),Start::Named,"--url wins over a saved pairing");
        assert_eq!(unnamed(false,true,None,false),Start::Named,"so does --token-file");
        assert_eq!(unnamed(false,false,s(),true),Start::Paired("wss://b.ts.net".into()),"a pairing wins over a local hub's token");
        assert_eq!(unnamed(false,false,None,true),Start::Named,"a machine with a hub's token keeps working");
        assert_eq!(unnamed(false,false,None,false),Start::Unpaired);
    }
    #[test] fn only_a_paired_clients_401_means_revoked(){
        let e=|c:u16|anyhow::Error::from(tokio_tungstenite::tungstenite::Error::Http(tokio_tungstenite::tungstenite::http::Response::builder().status(c).body(None).unwrap().into()));
        assert!(refused(true,&e(401)));assert!(!refused(false,&e(401)),"a hub token that fails is a wrong file, not a revoked device");
        assert!(!refused(true,&e(500)));assert!(!refused(true,&anyhow::anyhow!("connection refused")));
    }
}
/// The hub said no to this device's credential at the door, as it does once it is revoked (a restart or a lost route is only a reconnect).
fn refused(paired:bool,e:&anyhow::Error)->bool {
    paired&&matches!(e.downcast_ref::<tokio_tungstenite::tungstenite::Error>(),Some(tokio_tungstenite::tungstenite::Error::Http(r)) if r.status()==401)
}
fn unqualify(id:&str)->String {id.rsplit_once('|').map_or(id,|(_,id)|id).into()}
fn millis()->u64 {SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64}
struct View {
    launcher:bool,
    rpcs:HashMap<u64,&'static str>, asks:HashMap<u64,u64>,
    hub:String, cid:String, greeting:Value, target:Value, session:Value,
    view_id:String, revision:u64, next:u64, pending:HashMap<u64,(u64,Value,String)>,
    restored:Restore, saved:Option<Value>, drawn:bool, outgoing:HashMap<u64,bool>, rpc_names:HashMap<u64,String>, dirty:bool, controls:HashMap<u64,Control>, sent:HashMap<u64,std::time::Instant>, req_draft:HashMap<u64,String>, offered:HashMap<u64,String>, bases:delta::Bases,
    /// The history page on its way (request id, cursor), and one waiting for the view's requests to settle: its picture must carry their session.
    paging:Option<(u64,String)>, want_page:Option<String>,
}
#[derive(Default)] struct Control { next:u64, bytes:usize, parts:Vec<String>, done:bool }
const MAX_CONTROLS:usize=4;const MAX_CONTROL_BYTES:usize=16*1024*1024;const MAX_CONTROL_PARTS:u64=4096;
impl View {
    fn new(config:&Config)->Self {let mut v=Self::fresh(config);let reopen=crate::local::Local::load().default_view!="list"; let saved_view=crate::screens::place::take_view(); if let (true,Some((target,session)))=(reopen,saved_view){v.target=target;v.session=crate::screens::place::opened_session(&session,millis());v.restored=Restore::First;}v}
    fn fresh(config:&Config)->Self {Self {launcher:config.launched,restored:Restore::None,saved:None,drawn:false,rpcs:HashMap::new(),asks:HashMap::new(),hub:crate::pairing::http_of(&config.hub),cid:format!("gpui-{}-{}",std::process::id(),millis()),
        greeting:Value::Null,target:json!({"kind":"recent","id":""}),session:json!({"sel":"","rid":millis(),"drafts":{},"open":{},"adding":false,"panel":false,"info":{}}),
        view_id:String::new(),revision:0,next:(1_u64<<47)+millis(),pending:HashMap::new(),outgoing:HashMap::new(),rpc_names:HashMap::new(),dirty:false,controls:HashMap::new(),sent:HashMap::new(),paging:None,want_page:None,req_draft:HashMap::new(),offered:HashMap::new(),bases:delta::Bases::default()}}
    /// the session key the composer's text goes under: the thread on show (a bot's target shows its chat thread), else the target
    fn draft_key(&self)->String {self.session["sel"].as_str().filter(|s|!s.is_empty()).or(self.target["id"].as_str()).unwrap_or("").to_string()}
    fn request(&mut self, method:&str, extra:Value)->Value {
        self.next+=1;self.revision+=1;
        let mut p=json!({"version":1,"hub":self.hub,"generation":self.greeting["generation"],"epoch":self.greeting["epoch"],"requestRevision":self.revision,
            "target":self.target,"viewId":self.view_id,"session":self.session,"cid":self.cid});
        for(k,v)in extra.as_object().into_iter().flatten(){p[k]=v.clone();}
        // Opt in to the bounded native sidebar (at most 48 rows a page, paged by controls).
        if ["view.open","view.page","view.update","view.action"].contains(&method) {
            p["nativeSidebar"]=json!(1);
            p["accountProfiles"]=json!(1); // the account page: the hub offers its `accounts` grant only to a view that declares this version
            p["client"]=json!({"kind":"desktop","os":match std::env::consts::OS{"macos"=>"darwin",o=>o},"launcher":self.launcher}); // setup's words for keys, and whether another home can be chosen
            // Offer keyed deltas against the newest body held; the hub answers whole or with ops from it.
            p["viewDelta"]=json!({"version":1,"token":self.bases.newest});
            self.offered.insert(self.next,self.bases.newest.clone());
            if self.offered.len()>64 {self.offered.clear();}
        }
        self.pending.insert(self.next,(self.revision,self.target.clone(),method.into()));
        if self.sent.len()>64 {self.sent.clear();}
        self.sent.insert(self.next,std::time::Instant::now());
        // the draft this request carried: a delta's "t" op puts back exactly that text (a later reply may have replaced the session since)
        if self.req_draft.len()>64 {self.req_draft.clear();}
        if let Some(d)=self.session["sel"].as_str().and_then(|s|self.session["drafts"][s].as_str()).filter(|d|!d.is_empty()) {self.req_draft.insert(self.next,d.to_string());}
        json!({"id":self.next,"m":method,"p":p})
    }
    /// view.page: a view stays on the page it asked for, so a history page is always followed by this with cursor "", the newest again.
    fn page(&mut self, cursor:&str)->Value {
        let request=self.request("view.page",json!({"cursor":cursor,"anchor":{"id":"","offset":0}}));
        if !cursor.is_empty() {self.paging=Some((self.next,cursor.to_string()));}
        request
    }
}
/// The id of a direct request (not a view's): from 2^30, apart from the views' own.
fn rpc_id()->u64 {static RPC:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(1<<30);RPC.fetch_add(1,std::sync::atomic::Ordering::Relaxed)}
pub fn connect(config:Config)->(Sender<Command>,async_channel::Receiver<Event>) {
    crate::screens::place::open(&config);
    let (tx,mut commands)=mpsc::channel(256);let (events,rx)=async_channel::bounded(64);
    std::thread::Builder::new().name("hub".into()).spawn(move || {
        let runtime=tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async move {
            if config.unpaired {while commands.recv().await.is_some() {} return;} // nothing to connect to: pairing starts this client again
            let mut view=View::new(&config);let mut delay=1;
            loop {
                let _=events.send(Event::Status("Connecting…".into())).await;
                match session(&config,&mut commands,&events,&mut view).await {
                    Ok(())=>break,
                    Err(error) if refused(config.paired,&error)=>{let _=events.send(Event::Unpaired).await;break;}
                    Err(error)=>{ // Never log the handshake URL or its credential.
                        if let Some(secs)=format!("{error:#}").strip_prefix("net_sleep:").and_then(|s|s.parse::<f64>().ok()) {
                            // the e2e harness's net_sleep: the socket is gone for `secs`, then the normal reconnect runs
                            let _=events.send(Event::Status("Disconnected (net_sleep)".into())).await;
                            view.pending.clear();view.outgoing.clear();view.rpc_names.clear();view.controls.clear();view.offered.clear();view.bases.clear();view.view_id.clear();view.greeting=Value::Null;view.paging=None;view.want_page=None;
                            tokio::time::sleep(Duration::from_secs_f64(secs)).await;let _=events.send(Event::NetWake).await;delay=1;continue;
                        }
                        let token=std::fs::read_to_string(&config.token_file).unwrap_or_default();let mut text=format!("{error:#}");if !token.trim().is_empty(){text=text.replace(token.trim(),"<token>");}eprintln!("GPUI_LOG {{\"disconnect\":{}}}",serde_json::Value::String(text));
                        let _=events.send(Event::Error("Hub disconnected. Draft kept; reconnecting. Check the experimental hub log.".into())).await;
                        view.pending.clear();view.outgoing.clear();view.rpc_names.clear();view.controls.clear();view.offered.clear();view.bases.clear();view.view_id.clear();view.greeting=Value::Null;view.paging=None;view.want_page=None;
                        tokio::time::sleep(Duration::from_secs(delay)).await;delay=(delay*2).min(10);
                    }
                }
            }
        });
    }).expect("hub worker");(tx,rx)
}
async fn session(config:&Config,commands:&mut mpsc::Receiver<Command>,events:&async_channel::Sender<Event>,v:&mut View)->Result<()> {
    let token=std::fs::read_to_string(&config.token_file)?;
    ensure!(token.trim().chars().all(|c|c.is_ascii_alphanumeric()||c=='-'||c=='_'),"invalid token file");
    let url=format!("{}/ws?view=1&mobile=1&token={}",config.hub.trim_end_matches('/'),token.trim());
    let limits=WebSocketConfig::default().max_message_size(Some(64<<20)).max_frame_size(Some(64<<20));
    let (socket,_)=tokio::time::timeout(Duration::from_secs(5),tokio_tungstenite::connect_async_with_config(url,Some(limits),true)).await??;
    let (mut sink,mut source)=socket.split();let mut last=tokio::time::Instant::now();
    let mut heartbeat=tokio::time::interval(Duration::from_secs(10));
    loop {tokio::select! {
        command=commands.recv(), if !v.greeting.is_null()=>{
            let Some(command)=command else{return Ok(())};
            if let Command::Sleep(s)=&command {anyhow::bail!("net_sleep:{s}");}
            let request=match command {
                Command::Sleep(_)=>unreachable!(),
                Command::Rpc(m,p,tag)=>{let id=rpc_id();v.rpcs.insert(id,tag);if v.rpcs.len()>64 {v.rpcs.clear();}json!({"id":id,"m":m,"p":p.0})},
                Command::Ask(m,p,tag)=>{static ASK:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(1<<31);let id=ASK.fetch_add(1,std::sync::atomic::Ordering::Relaxed);v.asks.insert(id,tag);if v.asks.len()>64 {v.asks.clear();}json!({"id":id,"m":m,"p":p})},
                Command::Select(id)=>{let id=unqualify(&id);v.want_page=None;v.target=json!({"kind":"thread","id":id});v.session["sel"]=json!(id);v.view_id.clear();v.request("view.open",json!({}))},
                Command::Send(text)=>{ensure!(!v.view_id.is_empty(),"view not ready");let id=v.draft_key();v.session["drafts"][id]=json!(text);v.request("view.action",json!({"action":"send","value":""}))},
                Command::Draft(text)=>{let id=v.draft_key();v.session["drafts"][id]=json!(text);v.request("view.action",json!({"action":"draft","value":text}))},
                Command::History(cursor)=>{if v.view_id.is_empty()||!v.pending.is_empty()||v.paging.is_some(){v.want_page=Some(cursor);continue;}v.page(&cursor)},
                Command::Page(value)=>{ensure!(!v.view_id.is_empty(),"view not ready");v.request("view.action",json!({"action":"native-sidebar-page","value":value}))},
                Command::Action(action,value)=>{if action.starts_with("row-")||action.starts_with("delete")||action.starts_with("rename"){eprintln!("GPUI_LOG {{\"action\":{},\"value\":{}}}",Value::String(action.clone()),Value::String(value.chars().take(80).collect()));}v.request("view.action",json!({"action":action,"value":value}))},
            };
            sink.send(Message::Binary(wire::encode(&request).into())).await?;
        },
        frame=source.next()=>{
            let frame=frame.context("hub closed")??;last=tokio::time::Instant::now();crate::perf::wire(frame.len());if let Message::Binary(b)=&frame {if crate::screens::plot::is_plot(b) {if b.len()<=(32<<20) {let _=events.send(Event::Plot(b.to_vec())).await;}else{let _=events.send(Event::PlotTooBig(b.len())).await;}continue;}}let recv=std::time::Instant::now();let mut size=0usize;
            let mut x=match frame {Message::Binary(bytes)=>{size=bytes.len();wire::decode(&bytes)?},Message::Text(text)=>serde_json::from_str(&text)?,Message::Ping(bytes)=>{sink.send(Message::Pong(bytes)).await?;continue},Message::Close(_)=>anyhow::bail!("closed"),_=>continue};
            let decoded=std::time::Instant::now();
            match x["t"].as_str().unwrap_or("") {
                "voice"=>{let _=events.send(Event::Voice(x)).await;continue;}
                "view"=>{if x.get("version").is_none()&&x["thread"].is_string()&&x["kind"].is_string() {let g=|k:&str|x[k].as_str().unwrap_or("").to_string();let _=events.send(Event::ViewShow(g("thread"),g("kind"),g("path"))).await;continue;}ensure!(x["version"]==1&&x["renderer"]=="mobile"&&x["generation"].is_string()&&x["epoch"].is_string(),"unsupported hub greeting");v.greeting=x;let _=events.send(Event::Hello).await;v.drawn=false;if v.target["kind"]!="recent"&&v.restored==Restore::None&&v.saved.is_none(){v.restored=Restore::First;}let request=v.request("view.open",json!({}));sink.send(Message::Binary(wire::encode(&request).into())).await?;
                    // the star suggestion, if it is still out (a reconnect shows it again until it is decided)
                    let id=rpc_id();v.rpcs.insert(id,"star");sink.send(Message::Binary(wire::encode(&json!({"id":id,"m":"star.offer","p":{}})).into())).await?;},
                "reply"=>{
                    let id=x["id"].as_u64().unwrap_or(0);
                    if let Some(tag)=v.asks.remove(&id) {let _=events.send(Event::Answer(tag,x)).await;continue;}
                    if let Some(tag)=v.rpcs.remove(&id) {if tag=="star" {let _=events.send(Event::Star(if x["ok"]==true {x["result"]["offer"].clone()} else {Value::Null})).await;continue;}if tag=="acct" || (tag=="acct-moveall" && x["ok"]==true) {let _=events.send(Event::Data(tag.clone(),x["result"].clone())).await;}if tag=="steps" {let _=events.send(Event::Steps(if x["ok"]==true {x["result"].clone()} else {Value::Null})).await;continue;}if tag.starts_with("probe") {let slot:usize=tag[5..].parse().unwrap_or(0);let m=x["result"]["matches"].as_array().map(|a|a.iter().filter_map(|p|Some((p[0].as_u64()? as usize,p[1].as_u64()? as usize))).collect()).unwrap_or_default();let _=events.send(Event::Probe(slot,m)).await;continue;}if tag=="refs" {let _=events.send(Event::Refs(if x["ok"]==true {x["result"].clone()} else {Value::Null})).await;continue;}if tag=="pick" {let _=events.send(Event::Pick(if x["ok"]==true {x["result"]["info"].as_str().unwrap_or("").to_string()} else {String::new()})).await;continue;}let ok=x["ok"]==true;let why=if ok&&tag.starts_with("hook"){crate::screens::bots::hook_answer(&x).to_string()}else if ok&&tag.starts_with("files"){x["result"].to_string()}else{crate::screens::settings::reply_error(&x)};let _=events.send(Event::Rpc(tag,ok,why)).await;continue;}
                    if let Some(is_post)=v.outgoing.remove(&id) {let name=v.rpc_names.remove(&id).unwrap_or_default();if !is_post {v.dirty=true;} /* a job's answer (mech.list, boards.list, fab.look...) is folded by the hub into this view: ask for it */
                        // every product RPC's answer is logged, so a refused send is never silent (the reply's error may be a plain string, a code or a message)
                        let why=if x["ok"]==true {String::new()} else {rpc_error(&x)};
                        eprintln!("GPUI_LOG {}",json!({"rpc_reply":name,"ok":x["ok"]==true,"error":why.chars().take(160).collect::<String>()}));
                        if x["ok"]==true {if is_post {let _=events.send(Event::Accepted).await;}}else{let _=events.send(Event::Refused(format!("The hub refused it: {why}"))).await;}if v.dirty&&v.pending.is_empty()&&!v.view_id.is_empty(){v.dirty=false;let request=v.request("view.update",json!({}));sink.send(Message::Binary(wire::encode(&request).into())).await?;}continue;}
                    let Some((revision,target,method))=v.pending.remove(&id) else{continue};
                    let paged=v.paging.take_if(|(page,_)|*page==id).map(|(_,cursor)|cursor);
                    // A hub still opening its log says "not yet", and says the same of a thread it cannot show, for as long as it likes. So: the list first, asked for
                    // until the first view of this connection, which proves the hub is up; then the saved place once. Refused then, it is gone.
                    match view_refusal(&x,&method,v.drawn,v.restored) {
                        Refusal::Show=>{}
                        Refusal::Again=>{let _=events.send(Event::Status("Hub is opening its log…".into())).await;tokio::time::sleep(Duration::from_secs(1)).await;let request=v.request("view.open",json!({}));sink.send(Message::Binary(wire::encode(&request).into())).await?;continue;}
                        Refusal::ListFirst=>{v.saved=Some(v.target.clone());v.restored=Restore::None;v.target=json!({"kind":"recent","id":""});v.session["sel"]=json!("");v.view_id.clear();let request=v.request("view.open",json!({}));sink.send(Message::Binary(wire::encode(&request).into())).await?;continue;}
                        Refusal::Gone=>{v.restored=Restore::None;v.target=json!({"kind":"recent","id":""});v.session["sel"]=json!("");v.view_id.clear();let request=v.request("view.open",json!({}));sink.send(Message::Binary(wire::encode(&request).into())).await?;continue;}
                    }
                    if x["ok"]==true {if let Some(target)=v.saved.take(){v.drawn=true;v.session["sel"]=target["id"].clone();v.target=target;v.restored=Restore::AfterList;v.view_id.clear();let request=v.request("view.open",json!({}));sink.send(Message::Binary(wire::encode(&request).into())).await?;continue;}}
                    if x["ok"]!=true {if let Some(cursor)=paged {let _=events.send(Event::Older(cursor,None)).await;}let _=events.send(Event::Refused(format!("View request refused: {}",x["error"]["code"]))).await;continue;}
                    let mut r:Value=x["result"].take();
                    if let Some(control)=r["controlId"].as_u64() {
                        let c=v.controls.remove(&control).filter(|c|c.done);
                        let Some(c)=c else{anyhow::bail!("segmented reply incomplete")};
                        let values:Value=serde_json::from_str(&c.parts.concat())?;
                        ensure!(values.is_object()&&values.get("shown").is_none(),"invalid control values");
                        r.as_object_mut().unwrap().remove("controlId");
                        for(k,val)in values.as_object().unwrap(){if r.get(k).is_none(){r[k]=val.clone();}}
                    }
                    let offered=v.offered.remove(&id).unwrap_or_default();
                    let rtt_ms=v.sent.remove(&id).map_or(0.0,|t|recv.duration_since(t).as_secs_f64()*1000.0);
                    let is_delta=r.get("viewDelta").map_or(false,|d|d.get("ops").is_some());
                    if r.get("viewDelta").is_some() {
                        let draft=v.req_draft.remove(&id);
                        match delta::picture(r,&offered,revision,&v.bases,draft.as_deref()) {
                            Ok((full,token,body))=>{v.bases.keep(token,body);r=full;},
                            Err(error)=>{ // fail this reply and ask for the whole view next time
                                eprintln!("GPUI_LOG {{\"delta\":{}}}",Value::String(format!("{error:#}")));
                                v.bases.clear();v.dirty=true;continue;
                            }
                        }
                    }
                    let process_ms=decoded.elapsed().as_secs_f64()*1000.0;
                    let decode_ms=decoded.duration_since(recv).as_secs_f64()*1000.0;
                    let timing=Timing{rtt_ms,decode_ms,process_ms,bytes:size,delta:is_delta,at:Some(recv)};
                    let rv=r;let r=&rv;
                    if target!=v.target||revision!=v.revision {
                        // The reply is stale for drawing (the hub moved on while it was in flight) but its commands are what the person's press asked for: they still go out, once.
                        if method=="view.action" && r["version"]==1 && r["hub"]==v.hub && r["generation"]==v.greeting["generation"] && r["epoch"]==v.greeting["epoch"] {
                            for command in r["cmds"].as_array().into_iter().flatten() {
                                if let Some(e)=client_cmd(command) {let _=events.send(e).await;continue;}
                                if command["type"]!="send" || !(command["hub"].is_null()||command["hub"]==v.hub) {continue;}
                                let Some(request)=command["text"].as_str().and_then(|t|serde_json::from_str::<Value>(t).ok()) else {continue};
                                let (Some(rpc_id),m)=(request["id"].as_u64(),request["m"].as_str().unwrap_or("")) else {continue};
                                if m.starts_with("view.")||rpc_id>=(1<<47) {continue;}
                                eprintln!("GPUI_LOG {{\"rpc_sent\":\"{m}\",\"thread\":{},\"stale\":true}}",request["p"]["thread"]);
                                v.outgoing.insert(rpc_id,is_post(m));v.rpc_names.insert(rpc_id,m.to_string());
                                sink.send(Message::Binary(wire::encode(&request).into())).await?;
                            }
                        }
                        v.dirty=true;
                        if let Some(cursor)=paged {
                            let _=events.send(Event::Older(cursor,None)).await;
                            if target==v.target {let request=v.page("");sink.send(Message::Binary(wire::encode(&request).into())).await?;}
                        }
                        continue;
                    }
                    ensure!(r["version"]==1&&r["hub"]==v.hub&&r["generation"]==v.greeting["generation"]&&r["epoch"]==v.greeting["epoch"]&&r["requestRevision"]==revision&&r["target"]==target,"view identity mismatch");
                    v.view_id=r["viewId"].as_str().context("missing view id")?.to_string();
                    ensure!(r["session"].is_object(),"missing session");v.session=r["session"].clone();v.restored=Restore::None;v.drawn=true;crate::screens::place::note_view(&v.target,&v.session);
                    for command in r["cmds"].as_array().into_iter().flatten(){
                        if let Some(e)=client_cmd(command) {let _=events.send(e).await;continue;}
                        if command["type"]=="send" {
                            ensure!(command["hub"].is_null()||command["hub"]==v.hub,"cross-hub command refused");
                            let request:Value=serde_json::from_str(command["text"].as_str().context("missing command text")?)?;
                            let rpc_id=request["id"].as_u64().context("missing RPC id")?;
                            let m=request["m"].as_str().unwrap_or("");ensure!(!m.starts_with("view.")&&rpc_id<(1<<47),"invalid product RPC");eprintln!("GPUI_LOG {{\"rpc_sent\":\"{m}\",\"thread\":{}}}",request["p"]["thread"]);
                            v.outgoing.insert(rpc_id,method=="view.action"&&is_post(m));v.rpc_names.insert(rpc_id,m.to_string());
                            // The send action's canonical command is named by the hub; ACK all sends from this action.
                            if method=="view.action" && m=="chat.send" {v.outgoing.insert(rpc_id,true);}
                            sink.send(Message::Binary(wire::encode(&request).into())).await?;
                        }
                    }
                    if r["nextTarget"].is_object(){v.target=r["nextTarget"].clone();v.view_id.clear();let request=v.request("view.open",json!({}));sink.send(Message::Binary(wire::encode(&request).into())).await?;}
                    else if let Some(cursor)=paged {
                        let _=events.send(Event::Older(cursor,Some(rv))).await;
                        let request=v.page("");sink.send(Message::Binary(wire::encode(&request).into())).await?;
                    }
                    else if r["shown"].is_object(){let _=events.send(Event::Picture(rv,timing)).await;let _=events.send(Event::Status("Connected".into())).await;}
                },
                "view.control"=>{
                    let id=x["id"].as_u64().unwrap_or(0);let part=x["part"].as_u64().unwrap_or(u64::MAX);let text=x["text"].as_str().unwrap_or("");
                    if !v.pending.contains_key(&id)||text.len()>32768 {continue;}
                    if !v.controls.contains_key(&id) {if part!=0||v.controls.len()>=MAX_CONTROLS{continue;}v.controls.insert(id,Control::default());}
                    let c=v.controls.get_mut(&id).unwrap();
                    if c.done||part!=c.next||part>=MAX_CONTROL_PARTS||c.bytes+text.len()>MAX_CONTROL_BYTES {v.controls.remove(&id);continue;}
                    c.bytes+=text.len();c.parts.push(text.to_string());c.next+=1;c.done=x["last"]==true;
                },
                "view.changed"=>{eprintln!("GPUI_LOG {{\"view_changed\":true}}");if x["generation"]==v.greeting["generation"]&&x["epoch"]==v.greeting["epoch"]&&x["viewId"]==v.view_id&&x["target"]==v.target {v.dirty=true;}},
                "term"=>{let key=x["thread"].as_str().unwrap_or("").to_string();let pid=x["pid"].as_i64().map(|p|p as i32);
                    let b:Vec<u8>=x["b"].as_array().map(|a|a.iter().map(|n|n.as_u64().unwrap_or(0) as u8).collect()).unwrap_or_default();
                    let reset=x["reset"].as_array().and_then(|r|Some((r.first()?.as_u64()? as usize,r.get(1)?.as_u64()? as usize)));
                    if !key.is_empty() {let _=events.send(Event::Term(key,pid,b,reset)).await;}},
                "star"=>{let _=events.send(Event::Star(x["offer"].clone())).await;},
                "info"=>{if let Some(m)=x["info"].as_object(){for(k,val) in m{if let Some(owner)=k.strip_prefix("browser.tabs."){let _=events.send(Event::BrowserTabs(owner.to_string(),val.as_str().unwrap_or("").to_string())).await;}}}},
                "notify"=>{crate::screens::desk::show(&x);},"notify.clear"=>{crate::screens::desk::clear(&x);},
                "sheets"=>{let _=events.send(Event::Sheets(x["rows"].as_str().unwrap_or("").to_string())).await;},
                "hb"=>{sink.send(Message::Binary(wire::encode(&json!({"id":0,"m":"ping","p":{}})).into())).await?;},
                _=>{}
            }
            if v.dirty&&v.pending.is_empty()&&!v.view_id.is_empty(){v.dirty=false;let request=v.request("view.update",json!({}));sink.send(Message::Binary(wire::encode(&request).into())).await?;}
            else if v.want_page.is_some()&&v.pending.is_empty()&&v.paging.is_none()&&!v.view_id.is_empty(){let cursor=v.want_page.take().unwrap_or_default();let request=v.page(&cursor);sink.send(Message::Binary(wire::encode(&request).into())).await?;}
        },
        _=heartbeat.tick()=>{ensure!(last.elapsed()<Duration::from_secs(35),"hub heartbeat timeout");sink.send(Message::Ping(Vec::new().into())).await?;}
    }}
}

/// A saved place on its way back: not asked yet / refused before the hub was known to be up / asked again now it is.
#[derive(Clone,Copy,Debug,PartialEq)] pub enum Restore { None, First, AfterList }
#[derive(Debug,PartialEq)] pub enum Refusal { /** nothing to do here: the reply goes on as it did */ Show, Again, ListFirst, Gone }
/// What a view reply means for the open in progress. Only a view.open is ever repeated; an action is not repeated behind the person's back. Once this
/// connection has drawn a view the hub is up, so a refusal is the hub's real answer, and the person sees it.
pub fn view_refusal(x:&Value,method:&str,drawn:bool,restored:Restore)->Refusal {
    if x["ok"]==true||method!="view.open" {return Refusal::Show;}
    match restored {
        Restore::AfterList=>Refusal::Gone,
        Restore::First if x["error"]["retryable"]==true=>Refusal::ListFirst,
        Restore::First=>Refusal::Gone,
        Restore::None if x["error"]["retryable"]==true&&!drawn=>Refusal::Again,
        Restore::None=>Refusal::Show,
    }
}

/// The words for a failed product RPC's answer: the hub's error may be a plain string, an object with a message or a code, or something else.
pub fn rpc_error(x:&Value)->String {
    let e=&x["error"];
    e.as_str().map(String::from).or_else(||e["message"].as_str().map(String::from)).or_else(||e["code"].as_str().map(String::from)).unwrap_or_else(||if e.is_null(){"no reason given".into()}else{e.to_string()})
}
/// The product RPCs that post a message: their acceptance is the client's "sent" (chat.send comes through `view.action` by name too).
/// The product RPCs whose ok answer is a message the hub took (the window then empties its box). A bot's chat sends `bots.say`, not `turn.start`.
pub fn is_post(m:&str)->bool { ["message.send","thread.send","send","turn.start","bots.say","bots.give"].contains(&m) }
/// A view command that is for this client, not the hub: `compose` sets a thread's composer, `say` is one line for the person (a refusal, unless its `tone` is "note").
pub fn client_cmd(c:&Value)->Option<Event>{let text=c["text"].as_str().unwrap_or("").to_string();match c["type"].as_str()?{"compose"=>Some(Event::Compose(c["thread"].as_str()?.to_string(),text)),"say" if c["tone"]=="note"=>Some(Event::Note(text)),"say"=>Some(Event::Refused(text)),_=>None}}

#[cfg(test)]
mod rpc_tests {
    use super::{is_post, rpc_error};
    use serde_json::json;
    #[test] fn a_failed_answer_always_says_something() {
        assert_eq!(rpc_error(&json!({"ok":false,"error":"unknown thread"})), "unknown thread", "a plain string, not null");
        assert_eq!(rpc_error(&json!({"ok":false,"error":{"code":"nope","message":"no such thing"}})), "no such thing");
        assert_eq!(rpc_error(&json!({"ok":false,"error":{"code":"nope"}})), "nope");
        assert_eq!(rpc_error(&json!({"ok":false})), "no reason given");
        assert!(rpc_error(&json!({"ok":false,"error":[1]})).contains('1'));
    }
    #[test] fn a_hub_saying_not_yet_is_waited_for_and_a_thread_it_cannot_show_is_not() {
        use super::{view_refusal as f, Refusal::*, Restore};
        let no = json!({"ok":false,"error":{"code":"PRESENTER_UNAVAILABLE","retryable":true}}); let hard = json!({"ok":false,"error":{"code":"nope"}});
        assert_eq!(f(&no, "view.open", false, Restore::None), Again, "booting: ask again");
        assert_eq!(f(&no, "view.open", true, Restore::None), Show, "this connection has drawn: the hub is up, so a click on a thread it cannot show says so at once");
        assert_eq!(f(&no, "view.action", false, Restore::None), Show, "an action is not repeated");
        assert_eq!(f(&no, "view.open", false, Restore::First), ListFirst, "a saved place: booting or gone? the list says which");
        assert_eq!(f(&no, "view.open", true, Restore::AfterList), Gone, "refused with the hub up: it is gone, and never asked again");
        assert_eq!(f(&hard, "view.open", false, Restore::First), Gone);
        assert_eq!(f(&hard, "view.open", false, Restore::None), Show);
        assert_eq!(f(&json!({"ok":true}), "view.open", false, Restore::First), Show);
    }
    #[test] fn the_clients_own_commands_become_events() {
        use super::{client_cmd,Event};use serde_json::json;
        assert!(matches!(client_cmd(&json!({"type":"compose","thread":"t1","text":"hi"})),Some(Event::Compose(t,x)) if t=="t1"&&x=="hi"));
        assert!(matches!(client_cmd(&json!({"type":"say","text":"no"})),Some(Event::Refused(x)) if x=="no"));
        assert!(matches!(client_cmd(&json!({"type":"say","tone":"note","text":"saved"})),Some(Event::Note(x)) if x=="saved"));
        assert!(client_cmd(&json!({"type":"send","text":"{}"})).is_none()&&client_cmd(&json!({"type":"keep"})).is_none());
    }
    #[test] fn turn_start_is_a_post() { for m in ["turn.start","message.send","thread.send","send","bots.say"] { assert!(is_post(m), "{m}"); } assert!(!is_post("fab.look") && !is_post("mech.list")); }
}
