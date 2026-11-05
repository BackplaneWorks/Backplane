use crate::{delta,wire};
use anyhow::{ensure, Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{collections::HashMap, path::PathBuf, time::{Duration, SystemTime, UNIX_EPOCH}};
use tokio::sync::mpsc::{self, Sender};
use tokio_tungstenite::tungstenite::{Message, protocol::WebSocketConfig};

#[derive(Debug)] pub enum Command { Sleep(f64), Select(String), Send(String), Draft(String), Action(String,String), Page(String) }
#[derive(Debug)] pub enum Event { NetWake, Picture(Value,Timing), Status(String), Accepted, Error(String) }
/// Where a reply's time went before the UI saw it: send to frame received, decode, delta/merge, and the body's size.
#[derive(Debug,Clone,Default)] pub struct Timing { pub rtt_ms:f64, pub decode_ms:f64, pub process_ms:f64, pub bytes:usize, pub delta:bool }
#[derive(Clone)] pub struct Config { pub hub:String, pub token_file:PathBuf, pub exit_after:Option<f64>, pub script:Option<PathBuf>, pub swatches:bool, pub gallery:bool, pub fonts:bool, pub solid_demo:Option<String>, pub theme:Option<String> }
impl Config {
    pub fn parse() -> Result<Self> {
        let home=std::env::var("HOME")?;
        let mut config=Self{hub:"ws://100.64.84.41:3799".into(),token_file:PathBuf::from(&home).join(".backplane-gpui/token"),exit_after:None,swatches:false,gallery:false,fonts:false,solid_demo:None,theme:None,script:std::env::var_os("BACKPLANE_GPUI_SCRIPT").map(PathBuf::from)};
        let mut args=std::env::args().skip(1);
        while let Some(arg)=args.next() { match arg.as_str() {
            "--hub"|"--url"=>config.hub=args.next().context("--url needs a URL")?,
            "--exit-after"=>config.exit_after=Some(args.next().context("--exit-after needs seconds")?.parse()?),
            "--swatches"=>config.swatches=true,
            "--gallery"=>config.gallery=true,
            "--fonts"=>config.fonts=true,
            "--solid-demo"=>config.solid_demo=Some(args.next().context("--solid-demo needs TRIS[,STEPS]")?),
            "--theme"=>config.theme=Some(args.next().context("--theme needs light|dark")?),
            "--script"=>config.script=Some(PathBuf::from(args.next().context("--script needs a file")?)),
            "--token-file"=>config.token_file=PathBuf::from(args.next().context("--token-file needs a path")?),
            _=>anyhow::bail!("usage: backplane-gpui [--url http://HOST:PORT] [--token-file FILE] [--exit-after SECONDS] [--script FILE] [--swatches] [--gallery] [--fonts] [--solid-demo TRIS[,STEPS]] [--theme light|dark]")
        }}
        config.hub=config.hub.trim_end_matches('/').to_string();
        if let Some(rest)=config.hub.strip_prefix("http://") {config.hub=format!("ws://{rest}");}
        ensure!(config.hub.starts_with("ws://") && !config.hub.contains(['?','#','@']) && !config.hub.trim_end_matches('/').ends_with(":3787"),"experiment needs an explicit ws:// hub on a separate port");
        let token_path=std::fs::canonicalize(&config.token_file).context("experiment token file missing; start its hub first")?;
        ensure!(!token_path.starts_with(PathBuf::from(home).join(".backplane-bend")),"live home refused");
        Ok(config)
    }
}
fn unqualify(id:&str)->String {id.rsplit_once('|').map_or(id,|(_,id)|id).into()}
fn millis()->u64 {SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64}
struct View {
    hub:String, cid:String, greeting:Value, target:Value, session:Value,
    view_id:String, revision:u64, next:u64, pending:HashMap<u64,(u64,Value,String)>,
    outgoing:HashMap<u64,bool>, dirty:bool, controls:HashMap<u64,Control>, sent:HashMap<u64,std::time::Instant>, req_draft:HashMap<u64,String>, offered:HashMap<u64,String>, bases:delta::Bases,
}
#[derive(Default)] struct Control { next:u64, bytes:usize, parts:Vec<String>, done:bool }
const MAX_CONTROLS:usize=4;const MAX_CONTROL_BYTES:usize=16*1024*1024;const MAX_CONTROL_PARTS:u64=4096;
impl View {
    fn new(config:&Config)->Self {Self {hub:config.hub.replacen("ws://","http://",1),cid:format!("gpui-{}-{}",std::process::id(),millis()),
        greeting:Value::Null,target:json!({"kind":"recent","id":""}),session:json!({"sel":"","rid":millis(),"drafts":{},"open":{},"adding":false,"panel":false,"info":{}}),
        view_id:String::new(),revision:0,next:(1_u64<<47)+millis(),pending:HashMap::new(),outgoing:HashMap::new(),dirty:false,controls:HashMap::new(),sent:HashMap::new(),req_draft:HashMap::new(),offered:HashMap::new(),bases:delta::Bases::default()}}
    fn request(&mut self, method:&str, extra:Value)->Value {
        self.next+=1;self.revision+=1;
        let mut p=json!({"version":1,"hub":self.hub,"generation":self.greeting["generation"],"epoch":self.greeting["epoch"],"requestRevision":self.revision,
            "target":self.target,"viewId":self.view_id,"session":self.session,"cid":self.cid});
        for(k,v)in extra.as_object().into_iter().flatten(){p[k]=v.clone();}
        // Opt in to the bounded native sidebar (at most 48 rows a page, paged by controls).
        if ["view.open","view.page","view.update","view.action"].contains(&method) {
            p["nativeSidebar"]=json!(1);
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
}
pub fn connect(config:Config)->(Sender<Command>,async_channel::Receiver<Event>) {
    let (tx,mut commands)=mpsc::channel(256);let (events,rx)=async_channel::bounded(64);
    std::thread::Builder::new().name("hub".into()).spawn(move || {
        let runtime=tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async move {
            let mut view=View::new(&config);let mut delay=1;
            loop {
                let _=events.send(Event::Status("Connecting…".into())).await;
                match session(&config,&mut commands,&events,&mut view).await {
                    Ok(())=>break,
                    Err(error)=>{ // Never log the handshake URL or its credential.
                        if let Some(secs)=format!("{error:#}").strip_prefix("net_sleep:").and_then(|s|s.parse::<f64>().ok()) {
                            // the e2e harness's net_sleep: the socket is gone for `secs`, then the normal reconnect runs
                            let _=events.send(Event::Status("Disconnected (net_sleep)".into())).await;
                            view.pending.clear();view.outgoing.clear();view.controls.clear();view.offered.clear();view.bases.clear();view.view_id.clear();view.greeting=Value::Null;
                            tokio::time::sleep(Duration::from_secs_f64(secs)).await;let _=events.send(Event::NetWake).await;delay=1;continue;
                        }
                        let token=std::fs::read_to_string(&config.token_file).unwrap_or_default();let mut text=format!("{error:#}");if !token.trim().is_empty(){text=text.replace(token.trim(),"<token>");}eprintln!("GPUI_LOG {{\"disconnect\":{}}}",serde_json::Value::String(text));
                        let _=events.send(Event::Error("Hub disconnected. Draft kept; reconnecting. Check the experimental hub log.".into())).await;
                        view.pending.clear();view.outgoing.clear();view.controls.clear();view.offered.clear();view.bases.clear();view.view_id.clear();view.greeting=Value::Null;
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
    let limits=WebSocketConfig::default().max_message_size(Some(524288)).max_frame_size(Some(524288));
    let (socket,_)=tokio::time::timeout(Duration::from_secs(5),tokio_tungstenite::connect_async_with_config(url,Some(limits),true)).await??;
    let (mut sink,mut source)=socket.split();let mut last=tokio::time::Instant::now();
    let mut heartbeat=tokio::time::interval(Duration::from_secs(10));
    loop {tokio::select! {
        command=commands.recv(), if !v.greeting.is_null()=>{
            let Some(command)=command else{return Ok(())};
            if let Command::Sleep(s)=&command {anyhow::bail!("net_sleep:{s}");}
            let request=match command {
                Command::Sleep(_)=>unreachable!(),
                Command::Select(id)=>{let id=unqualify(&id);v.target=json!({"kind":"thread","id":id});v.session["sel"]=json!(id);v.view_id.clear();v.request("view.open",json!({}))},
                Command::Send(text)=>{ensure!(!v.view_id.is_empty(),"view not ready");let id=v.target["id"].as_str().unwrap_or("").to_string();v.session["drafts"][id]=json!(text);v.request("view.action",json!({"action":"send","value":""}))},
                Command::Draft(text)=>{let id=v.target["id"].as_str().unwrap_or("").to_string();v.session["drafts"][id]=json!(text);v.request("view.action",json!({"action":"draft","value":text}))},
                Command::Page(value)=>{ensure!(!v.view_id.is_empty(),"view not ready");v.request("view.action",json!({"action":"native-sidebar-page","value":value}))},
                Command::Action(action,value)=>v.request("view.action",json!({"action":action,"value":value})),
            };
            sink.send(Message::Binary(wire::encode(&request).into())).await?;
        },
        frame=source.next()=>{
            let frame=frame.context("hub closed")??;last=tokio::time::Instant::now();let recv=std::time::Instant::now();let mut size=0usize;
            let mut x=match frame {Message::Binary(bytes)=>{size=bytes.len();wire::decode(&bytes)?},Message::Text(text)=>serde_json::from_str(&text)?,Message::Ping(bytes)=>{sink.send(Message::Pong(bytes)).await?;continue},Message::Close(_)=>anyhow::bail!("closed"),_=>continue};
            let decoded=std::time::Instant::now();
            match x["t"].as_str().unwrap_or("") {
                "view"=>{ensure!(x["version"]==1&&x["renderer"]=="mobile"&&x["generation"].is_string()&&x["epoch"].is_string(),"unsupported hub greeting");v.greeting=x;let request=v.request("view.open",json!({}));sink.send(Message::Binary(wire::encode(&request).into())).await?;},
                "reply"=>{
                    let id=x["id"].as_u64().unwrap_or(0);
                    if let Some(is_post)=v.outgoing.remove(&id) {if x["ok"]==true {if is_post {let _=events.send(Event::Accepted).await;}}else{let _=events.send(Event::Error(format!("Hub refused request: {}",x["error"]["code"]))).await;}continue;}
                    let Some((revision,target,method))=v.pending.remove(&id) else{continue};
                    if x["ok"]!=true {let _=events.send(Event::Error(format!("View request refused: {}",x["error"]["code"]))).await;continue;}
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
                    let timing=Timing{rtt_ms,decode_ms,process_ms,bytes:size,delta:is_delta};
                    let rv=r;let r=&rv;
                    if target!=v.target||revision!=v.revision {v.dirty=true;continue;}
                    ensure!(r["version"]==1&&r["hub"]==v.hub&&r["generation"]==v.greeting["generation"]&&r["epoch"]==v.greeting["epoch"]&&r["requestRevision"]==revision&&r["target"]==target,"view identity mismatch");
                    v.view_id=r["viewId"].as_str().context("missing view id")?.to_string();
                    ensure!(r["session"].is_object(),"missing session");v.session=r["session"].clone();
                    for command in r["cmds"].as_array().into_iter().flatten(){
                        if command["type"]=="send" {
                            ensure!(command["hub"].is_null()||command["hub"]==v.hub,"cross-hub command refused");
                            let request:Value=serde_json::from_str(command["text"].as_str().context("missing command text")?)?;
                            let rpc_id=request["id"].as_u64().context("missing RPC id")?;
                            let m=request["m"].as_str().unwrap_or("");ensure!(!m.starts_with("view.")&&rpc_id<(1<<47),"invalid product RPC");
                            v.outgoing.insert(rpc_id,method=="view.action"&&["message.send","thread.send","send"].contains(&m));
                            // The send action's canonical command is named by the hub; ACK all sends from this action.
                            if method=="view.action" && m=="chat.send" {v.outgoing.insert(rpc_id,true);}
                            sink.send(Message::Binary(wire::encode(&request).into())).await?;
                        }
                    }
                    if r["nextTarget"].is_object(){v.target=r["nextTarget"].clone();v.view_id.clear();let request=v.request("view.open",json!({}));sink.send(Message::Binary(wire::encode(&request).into())).await?;}
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
                "view.changed"=>{if x["generation"]==v.greeting["generation"]&&x["epoch"]==v.greeting["epoch"]&&x["viewId"]==v.view_id&&x["target"]==v.target {v.dirty=true;}},
                "hb"=>{sink.send(Message::Binary(wire::encode(&json!({"id":0,"m":"ping","p":{}})).into())).await?;},
                _=>{}
            }
            if v.dirty&&v.pending.is_empty()&&!v.view_id.is_empty(){v.dirty=false;let request=v.request("view.update",json!({}));sink.send(Message::Binary(wire::encode(&request).into())).await?;}
        },
        _=heartbeat.tick()=>{ensure!(last.elapsed()<Duration::from_secs(35),"hub heartbeat timeout");sink.send(Message::Ping(Vec::new().into())).await?;}
    }}
}
