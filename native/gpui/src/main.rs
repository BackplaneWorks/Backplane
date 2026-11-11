#![recursion_limit = "256"]
// Windows: a window program, not a console one, so starting it (Explorer, the Start menu, the guard) opens no console window beside it.
// What it prints still reaches a file or pipe it is given (the tests, `--exit-after` runs).
#![cfg_attr(windows, windows_subsystem = "windows")]
mod art;
mod delta;
mod hub;
mod i18n;
#[cfg(test)]
mod i18n_scan;
mod input;
mod keys;
mod plat;
mod launch;
mod install;
mod local;
mod pairing;
mod hubs;
mod perf;
mod part;
mod screens;
mod script;
mod swatch;
mod ui;
mod viewers;
mod theme;
mod wire;
/// The hub's terminal emulator, shared by path: the floating terminal draws the shell with it (screens/term.rs).
#[allow(dead_code)]
#[path = "../../host/src/term/vt.rs"]
mod vt;
use gpui::{prelude::*, *, actions};
use serde_json::Value;
use std::{collections::{HashMap,VecDeque},time::{Duration,Instant}};
use script::{Pending,Step};
use hub::{Command,Event};
actions!(backplane,[Send,ClosePopup,CopySelection,ToggleSidebarKey,FocusSearchKey,OpenPaletteKey,FindInThreadKey]);
pub struct Backplane { pub pair:Option<screens::pair::Ui>, pub paired:bool, /** the hubs this device knows (the first is in use) */ pub hubs:Vec<hubs::Hub>, /** the sidebar picker's menu is open */ pub hub_menu:bool, /** this window has a hub to go back to from the pairing form */ pub has_hub:bool,
    pub input:Entity<screens::composer::Composer>, data:Value, status:String, error:String, note:String,
    tx:tokio::sync::mpsc::Sender<Command>, selected:String,
    drafts:HashMap<String,String>, started:Instant, usable:bool,
    first_frame:bool, script_done:bool, pub timeline:ListState, pub cats:art::Cats, pub links_sent:bool, pub links_last:String, pub sidebar_open:bool, pub models_pending:bool, pub local:local::Local, pub project_page:Option<String>, pub arm_new:Option<(Instant,std::collections::HashSet<String>)>, pub echo_seq:u64, pub thread_cache:Vec<(String,Value)>, pub thread_source:&'static str, pub confirm:Option<(String,Instant)>, pub ack_pending:Option<(String,Instant)>, pub recording:Option<(String,Vec<f64>)>, pub ui_epoch:u64, pub controls:Vec<script::CtlEntry>, pub ctl_last:String, pub probes_last:String, pub last_layout:String, pub wake_at:Option<Instant>, pub reconnected:bool, pub click_info:Option<(String,String)>, pub timing:Option<hub::Timing>, pub applied_at:Option<Instant>, pub apply_ms:f64, pub render_us:u64, swatches:bool, gallery:bool, fonts:bool, theme_override:Option<bool>, steps:VecDeque<Step>, pending:Option<Pending>, seq:u64, want_focus:bool, script_total:usize, screens:screens::Screens,
}
fn pager_row(cx:&mut Context<Backplane>,id:String,pager:&Value,light:bool)->Option<AnyElement> {let c=|name:&str|theme::color(light,name);
    let mut row=div().flex().gap_2().px_3().py_1().text_size(px(11.));let mut any=false;
    for (name,key) in [("Newer","previous"),("Older","next")] {
        let control=&pager[key];
        if let Some(value)=control["value"].as_str() {
            any=true;let value=value.to_string();let label=format!("{} ({})",name,pager["total"]);
            row=row.child(div().id(SharedString::from(format!("{id}-{key}"))).cursor_pointer().text_color(c("accent")).hover(|s|s.bg(c("raise"))).px_1().child(label)
                .on_click(cx.listener(move|this,_,_,cx|{let _=this.tx.try_send(Command::Page(value.clone()));cx.notify();})));
        }
    }
    if any {Some(row.into_any_element())} else {None}
}
pub fn string(v:&Value,key:&str)->String {v[key].as_str().unwrap_or("").to_string()}
impl Backplane {
    /// An agent's viewer_show (or the script's `viewshow` step): open the page its kind or file names on the thread on screen, and a model's part on it.
    pub(crate) fn view_show(&mut self,thread:&str,kind:&str,path:&str,cx:&mut Context<Self>){
        let Some((k,part))=screens::viewer::shown(kind,path) else {return};
        if !(self.selected==thread||self.selected.ends_with(&format!("|{thread}"))||thread.is_empty()) {return}
        let _=self.tx.try_send(screens::viewer::view_kind(&match part {Some(p)=>format!("{k}\t{p}"),None=>k})); cx.notify();
    }
    pub(crate) fn new(window:&mut Window,cx:&mut Context<Self>,config:hub::Config,started:Instant)->Self {
        let (swatches,gallery,fonts,theme)=(config.swatches,config.gallery,config.fonts,config.theme.clone());let steps:VecDeque<Step>=config.script.as_ref().map(|p|script::load(p)).unwrap_or_default();let script_total=steps.len();
        // a client that never gets projects says so (a hub with a canned presenter, a refused token ...) instead of leaving a script waiting silently
        cx.spawn(async move |weak,cx|{cx.background_executor().timer(Duration::from_secs(10)).await;let _=weak.update(cx,|this,_|{if !this.usable {this.m("not_usable",serde_json::json!({"after_s":10,"status":this.status,"error":this.error,"projects":this.data["projects"].as_array().map_or(0,|p|p.len())}));}});}).detach();
        let input=cx.new(screens::composer::Composer::new);input.focus_handle(cx).focus(window);
        let local=local::Local::load();
        let target=screens::attach::Target::from(&config.hub,&config.token_file);let drafts_path=screens::place::beside_credential(&config.token_file,"gpui-drafts.json");let saved=screens::drafts::load(&drafts_path);
        eprintln!("GPUI_LOG {}",serde_json::json!({"hub_in_use":pairing::host_port(&config.hub).0}));let open_link=config.pair_link.clone().filter(|l|pairing::is_open_link(l));let pair=screens::pair::start(config.unpaired,config.pair_link.as_deref().filter(|l|!pairing::is_open_link(l)),cx);let paired=config.paired;let has_hub=!config.unpaired;
        cx.spawn(async move |weak,cx|{while let Ok(link)=OPENED.1.recv().await {if weak.update(cx,|this,cx|screens::pair::incoming(this,&link,cx)).is_err(){break;}}}).detach();
        let(hub_at,token_file)=(screens::attach::Target::from(&config.hub,&config.token_file),config.token_file.clone());
        let elsewhere=hubs::elsewhere(&config.hub);let(tx,events)=hub::connect(config);
        // started with a thread's link (the system's handler for backplane://): the hub opens it once the window is up
        if let Some(l)=open_link {let _=tx.try_send(Command::Action("link-open".into(),l));}
        screens::place::watch(cx);screens::devbar::watch(cx);screens::hubbar::watch(cx,hub_at,&token_file);
        cx.spawn(async move |weak,cx|{while let Ok(event)=events.recv().await {if weak.update(cx,|this,cx|this.receive(event,cx)).is_err(){break;}}}).detach();
        // the keys held show their hints only in the window in use (keys::hint_mode): its focus coming or going draws them again
        cx.observe_window_activation(window,|_,_,cx|cx.notify()).detach();
        cx.observe(&input,|this,_,cx|{
            let text=this.input.read(cx).content.to_string();
            let changed=this.draft_changed(&text);
            // no notify: the composer is a child of this view, so its own repaints (a key typed, the caret's blink) re-render this view's shell
            // anyway, and the cached parts (part.rs: sidebar, messages, viewer) show nothing of the draft, so typing does not redraw them
            if changed {this.send_draft(text,cx);}
        }).detach();
        cx.subscribe(&input,|this,_,event:&screens::composer::ComposerEvent,cx|this.attach_event(event,cx)).detach();
        cx.on_app_quit(|this,_|{this.screens.drafts.flush();async{}}).detach();
        // Closing the window ends the program at once (the window manager's close, Alt+F4): what must not be lost is put away first (the drafts
        // on disk, a screen recording stopped), then the process exits without gpui's teardown, which drew nothing and took a second or two
        // (the renderer's destroy before the window went, every entity's drop). The hub runs on; it sees the socket close.
        {let weak=cx.entity().downgrade();window.on_window_should_close(cx,move|_,cx|{let t=Instant::now();let _=weak.update(cx,|this,_|this.before_exit());
            script::metric(started,"close",serde_json::json!({"put_away_ms":(t.elapsed().as_secs_f64()*1e6).round()/1e3}));std::process::exit(0)});}
        Self{pair,paired,hubs:hubs::load(),hub_menu:false,has_hub,input,data:Value::Null,status:"Connecting…".into(),note:String::new(),error:art::assets_missing().unwrap_or_default(),tx,selected:String::new(),drafts:saved.clone(),started,usable:false,first_frame:false,script_done:false,timeline:ListState::new(0,ListAlignment::Bottom,px(screens::timeline::RUNWAY)),cats:art::Cats::new(),links_sent:false,links_last:String::new(),sidebar_open:local.sidebar_open,models_pending:false,local,project_page:None,arm_new:None,echo_seq:0,thread_cache:Vec::new(),thread_source:"hub",confirm:None,ack_pending:None,recording:None,ui_epoch:0,controls:Vec::new(),ctl_last:String::new(),probes_last:String::new(),last_layout:String::new(),wake_at:None,reconnected:false,click_info:None,timing:None,applied_at:None,apply_ms:0.0,render_us:0,swatches,gallery,fonts,theme_override:theme.as_deref().map(|t|t=="light"),steps,pending:None,seq:0,want_focus:false,script_total,screens:{let mut s=screens::Screens::new(cx,target,drafts_path,saved);s.elsewhere=elsewhere;s}}
    }
    /// What the window's close keeps before the process exits: the drafts written (bounded, normally a few ms) and a screen recording ended.
    fn before_exit(&mut self) {self.screens.drafts.flush();drop(self.screens.imp.take_recording());}
    fn receive(&mut self,event:Event,cx:&mut Context<Self>) {
        let timed=perf::on().then(||(event.name(),Instant::now()));
        if let Some(s)=hub::status_for(&event){self.status=s.into();}
        match event {
            Event::NetWake=>{self.wake_at=Some(Instant::now());self.m("net",serde_json::json!({"event":"hub_reachable_again"}));},
            Event::Picture(r,timing)=>{
                let t_apply=Instant::now();self.ui_epoch+=1;
                if let Some(w)=self.wake_at.take(){self.reconnected=true;self.m("net",serde_json::json!({"event":"reconnected","ms":(w.elapsed().as_secs_f64()*1e6).round()/1e3}));}
                let mut r=r;let prev=self.data["thread"].take();self.data=r["shown"].take();self.picture_applied(cx);screens::histbar::picture(self);
                self.timeline_picture(prev,&r["window"]);
                if self.data["deleting"].is_object(){eprintln!("GPUI_LOG {{\"deleting\":{}}}",self.data["deleting"]["id"]);}let selected=string(&self.data["thread"],"id");
                screens::accounts::thread_changed(self,&selected);if !selected.is_empty() {let selected=screens::attach::raw_id(&selected).to_string();self.picture_arrived(&selected,cx);self.selected=selected;}
                self.thread_source="hub";
                if let Some((label,t0))=self.confirm.take(){self.m("input",serde_json::json!({"step":label,"ok":true,"latency_ms":(t0.elapsed().as_secs_f64()*1e6).round()/1e3,"source":"hub"}));}
                self.apply_ms=t_apply.elapsed().as_secs_f64()*1000.0;self.timing=Some(timing);self.applied_at=Some(Instant::now());self.adopt_new(cx);
            },
            Event::Plot(b)=>self.plot_received(b,cx),
            Event::Answer(id,reply) if id==screens::unsent::TAG=>{let why=if reply["ok"]==true {String::new()} else {hub::rpc_error(&reply)};self.screens.unsent.answered_out(reply["ok"]==true,&why);self.send_unsent();cx.notify();},
            Event::HubOffer(text)=>screens::hubpick::offered(self,&text,cx),
            Event::Using(home)=>{eprintln!("GPUI_LOG {}",serde_json::json!({"hub":"using","text":home.clone().unwrap_or_default()}));self.screens.offers.on_home=home;cx.notify();},
            Event::Answer(id,reply) if screens::hubpick::answered(self,id,&reply,cx)=>{},
            Event::Answer(id,reply)=>if !screens::projects::answered(self,id,&reply,cx) {screens::improvements::answered(self,id,reply,cx)},
            Event::Data(tag,v)=>if tag=="google-changed" {screens::google::changed(self)} else if tag=="google" {screens::google::got(self,v,cx)} else if tag=="acct-moveall" {screens::accounts::moved(self,&v)} else {screens::accounts::got(self,v,cx)},
            Event::Voice(f)=>self.voice_frame(f,cx),
            Event::Rpc("voice",ok,why)=>{if !ok{self.error=why;}},
            Event::Term(key,pid,b,reset,state)=>self.term_frame(&key,pid,&b,reset,state,cx),
            Event::Hello=>{self.term_rejoin();self.screens.unsent.live=true;self.send_unsent();},
            Event::Star(v)=>self.star_offer(&v,cx),
            Event::Rpc(tag,ok,why)=>if tag=="term"{if !ok{self.error=format!("Terminal: {why}");}}else if tag=="google"{screens::google::done(self,ok,&why)}else if tag.starts_with("acct"){screens::accounts::done(self,tag,ok,&why,cx)}else if tag=="fileopen" {if !ok {self.error=why;}} else if tag.starts_with("files") {screens::filesui::rpc_done(self,tag,ok,&why,cx)} else {screens::settings::rpc_done(self,tag,ok,&why,cx)},
            Event::PlotTooBig(n)=>self.screens.plot.too_big(n),
            Event::Refs(r)=>{ if r.get("resolved").is_some() { screens::histbar::resolved(self,&r,cx); } else { self.screens.plotview.hist.refs=screens::plot::refs_of(&r); } cx.notify(); }
            Event::Steps(r)=>{ self.screens.plotview.hist.steps=screens::histbar::steps_of(&r); screens::histbar::arrived(self); cx.notify(); }
            Event::Sheets(rows,refs)=>{ self.screens.plot.sheets=screens::plot::sheet_rows(&rows); self.screens.plot.sheet_refs=screens::plot::sheet_refs(&refs); cx.notify(); }
            Event::BrowserDownload(owner,id,name,error)=>{if let Some((o,page))=&self.screens.browser {if *o==owner {let page=page.clone();page.update(cx,|p,cx|p.take_download(&id,&name,&error,cx));}}},
            Event::BrowserTabs(owner,text)=>{if let Some((o,page))=&self.screens.browser {if *o==owner {let page=page.clone();page.update(cx,|p,cx|p.set_tabs(&text,cx));}}},
            Event::ViewShow(thread,kind,path)=>self.view_show(&thread,&kind,&path,cx),
            Event::Pick(info)=>{ let slot=self.screens.pick_slot.unwrap_or(0); let card=screens::plot::card(&info); let pv=self.screens.pv(slot); if pv.picked.is_some() { pv.card=card.clone(); } if let Some(c)=card { screens::plotview::cross_probe(self,slot,&c); } screens::plotview::report_state(self); cx.notify(); }
            Event::Probe(slot,m) if slot>=3=>{ screens::plotview::frame_on(self,slot-3,&m); cx.notify(); }
            Event::Probe(slot,m)=>{ let pv=self.screens.pv(slot); if pv.lit.len()<20000 { pv.lit.extend(m); } cx.notify(); }
            Event::Status(s)=>{if s!=self.status {eprintln!("GPUI_LOG {}",serde_json::json!({"status":s}));}self.status=s;if self.status=="Connected"{/* (every shown view says Connected: while setup is up its refusal stays until the person presses something there, or it would be gone before it was read) */ if !self.data["tour"].is_object(){self.error.clear();}screens::hubbar::connected(&mut self.screens.hubbar);self.send_unsent();}},
            Event::Accepted=>{self.input.update(cx,|input,cx|input.set(String::new(),cx));let _=self.tx.try_send(Command::Draft(String::new())); /* a /give keeps its text in the hub until it took it: the box is empty now, so is that copy */ self.drafts.remove(&self.selected);self.screens.drafts.set(&self.selected,"");self.voice_sent(cx);self.timeline_to_end();}, // (the file too: the box's own change comes after the map entry is gone, and would not clear it)
            Event::Older(cursor,page)=>self.older_landed(cursor,page,cx),
            Event::Error(e)=>{self.error=e;self.screens.unsent.lost();},
            Event::Unpaired=>screens::pair::revoked(self,cx),
            Event::Refused(e)=>{self.error=e;},
            Event::Note(n)=>{self.note=n;},
            // the hub moved the composer on: a command left it, or the conversation's draft came back after an intent was saved. Another thread's is kept for when it opens
            Event::Compose(th,text)=>{if th==self.selected {self.input.update(cx,|input,cx|input.set(text.clone(),cx));}if text.is_empty(){self.drafts.remove(&th);}else{self.drafts.insert(th.clone(),text.clone());}self.screens.drafts.set(&th,&text);},
        }
        if let Some((name,t0))=timed {if name=="picture" {if let Some(t)=&self.timing {perf::picture(t,self.apply_ms);}}perf::event(name,t0);}
        cx.notify();
    }
    /// The header's search: show the sidebar and give its search box the keys.
    /// The sidebar folded or unfolded (a click, a key or a script step): this device remembers it.
    pub fn sidebar_toggle(&mut self,cx:&mut Context<Self>){self.sidebar_open=!self.sidebar_open;self.local.sidebar_open=self.sidebar_open;self.local.save();self.ui_epoch+=1;cx.notify();}
    /// Whether the picture is light: the command line's choice, else this device's, else the hub's.
    pub fn light(&self)->bool{self.theme_override.unwrap_or_else(||self.local.light(self.data["theme"]=="light"))}
    /// A press of a `local:<name>` button in the settings dialog: this device's own setting, applied here and kept in the local file; the hub hears nothing.
    pub fn local_press(&mut self,name:&str,value:&str,cx:&mut Context<Self>){
        if !self.local.set(name,value){return;}
        if name=="sidebar-open"{self.sidebar_open=self.local.sidebar_open;}
        if name=="lang"{crate::i18n::set_choice(&self.local.lang);}
        if name.starts_with("shortcut"){let l=self.local.clone();cx.defer(move|cx|bind_all(cx,&l));}
        self.local.save();self.ui_epoch+=1;cx.notify();
    }
    /// The gear: this device's page opens at once (no round trip), and the hub is asked for its pages beside it.
    pub fn open_settings(&mut self,cx:&mut Context<Self>){
        let hub_open=self.data["settings"].is_object();
        let ui=&mut self.screens.settings_ui;
        if ui.open_local||hub_open{ui.open_local=false;ui.device=false;ui.accounts=false;} else {ui.open_local=true;ui.device=true;self.screens.focus_settings=true;}
        let _=self.tx.try_send(Command::Action("flag".into(),"settings".into()));cx.notify();
    }
    /// Search threads (the top bar's button; Ctrl+Shift+F is Find across threads, palette.rs): the sidebar's box takes the keys; a sidebar folded to its strip, by hand or for want of room, unfolds first
    /// (the box is not drawn there: `sidebar_open` alone missed the fold for room, and the press did nothing)
    pub fn focus_search(&mut self,cx:&mut Context<Self>){if !self.sidebar_open{self.screens.side_force=false;self.sidebar_toggle(cx);}else if !self.screens.side_shown{self.screens.side_force=true;}self.ui_epoch+=1;self.screens.projects.focus=Some("search");cx.notify();}
    fn select(&mut self,id:String,window:&mut Window,cx:&mut Context<Self>) {self.select_id(id,cx);self.input.focus_handle(cx).focus(window);}
    /// A project's name was clicked: its page (its threads, a New thread button) opens in the main area, at once, from what the sidebar holds; the folder's goal and notes,
    /// parts and boards follow when the hub answers (screens::projects::ask_home).
    pub fn open_project(&mut self,pid:String,cx:&mut Context<Self>){screens::projects::ask_home(self,&pid);self.project_page=Some(pid);self.ui_epoch+=1;cx.notify();}
    fn select_id(&mut self,id:String,cx:&mut Context<Self>) {self.project_page=None;self.ui_epoch+=1;self.screens.panels.sub_view=None;
        let raw=id.rsplit_once('|').map_or(id.as_str(),|(_,id)|id).to_string();
        
        let current=self.input.read(cx).content.to_string();
        let text=screens::drafts::switch(&mut self.drafts,&self.selected,&raw,&current);
        self.thread_switch(&raw);
        self.selected=raw.clone();
        self.input.update(cx,|input,cx|input.set(text,cx));
        // Stale-while-revalidate: the thread on screen goes into a small cache; a thread seen before is drawn from it at once and the hub's picture
        // replaces it; one never seen shows its title (from the sidebar row) and "Loading…". Bounded at 32 threads.
        let cur=string(&self.data["thread"],"id");let cur=cur.rsplit_once('|').map_or(cur.as_str(),|(_,i)|i).to_string();
        if !cur.is_empty()&&self.data["thread"]["loading"]!=true {let t=self.data["thread"].take();self.thread_cache.retain(|(k,_)|*k!=cur);self.thread_cache.insert(0,(cur,t));self.thread_cache.truncate(32);}
        if let Some(i)=self.thread_cache.iter().position(|(k,_)|*k==raw) {let (_,t)=self.thread_cache.remove(i);self.data["thread"]=t;self.thread_source="cache";}
        else {let title=self.thread_title(&raw);self.data["thread"]=serde_json::json!({"id":raw,"title":title,"entries":[],"loading":true});self.thread_source="placeholder";}
        self.timeline_show(&raw);
        let _=self.tx.try_send(Command::Select(id));self.want_focus=true;cx.notify();
    }
    fn thread_title(&self,raw:&str)->String {
        for project in self.data["projects"].as_array().into_iter().flatten() {
            for t in project["threads"].as_array().into_iter().flatten().chain(project["settled"].as_array().into_iter().flatten()) {
                let id=string(t,"id");if id.rsplit_once('|').map_or(id.as_str(),|(_,i)|i)==raw {return string(t,"title");}
            }
        }
        String::new()
    }
    fn send(&mut self,_:&Send,_:&mut Window,cx:&mut Context<Self>) {self.do_send(cx);}
    fn do_send(&mut self,cx:&mut Context<Self>) {if self.popup_enter(cx){return;}self.ui_epoch+=1;
        let text=self.input.read(cx).content.to_string();
        // a project's page on show: Send starts a thread in that project with this first message (0.14's home composer), never the thread
        // that was open before the page (it is not on screen)
        if let Some(pid)=self.project_page.clone().filter(|p|screens::projects::find(&self.data,p).is_some()) {
            if text.trim().is_empty()||self.status!="Connected"||self.screens.projects.first_send.is_some() {return;}
            self.screens.projects.first_send=Some((self.selected.rsplit('|').next().unwrap_or("").to_string(),text,Instant::now()));
            self.input.update(cx,|i,cx|i.set(String::new(),cx));
            self.arm_new();let _=self.tx.try_send(Command::Action("new-thread".into(),pid));cx.notify();return;
        }
        // a picture or file alone is a message too: the hub sends the attachments with whatever text there is (none: just them)
        let has_att=self.data["thread"]["attaching"].as_array().map_or(false,|a|!a.is_empty());
        // no hub now: the message waits in the window's own outbox (screens/unsent.rs), in the strip over the box, and goes once the hub is back
        let th=self.data["thread"]["id"].as_str().unwrap_or("").to_string();
        if self.status!="Connected"&&!text.trim().is_empty()&&!has_att&&!th.is_empty()&&!screens::sidebox::is_command(&self.data["thread"]["commands"],&text) {
            self.screens.unsent.push(&th,&text,std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0,|d|d.as_millis() as u64));
            self.input.update(cx,|i,cx|i.set(String::new(),cx));self.drafts.remove(&self.selected);self.screens.drafts.set(&self.selected,"");cx.notify();return;
        }
        if (!text.trim().is_empty()||has_att)&&self.status=="Connected" {if !text.trim().is_empty()&&!screens::sidebox::is_command(&self.data["thread"]["commands"],&text){self.echo_sent(&text);}let _=self.tx.try_send(Command::Send(text));self.error.clear();self.note.clear();}cx.notify();
    }
    /// The message is drawn at the end of the open thread the moment it is sent; the hub's picture, which has the real entry, replaces it (history::merge
    /// keeps the rows up to the first one the hub also has, so an echo past it is dropped, not doubled).
    /// Edit on an unsent message: its text after what is in the box, and the keyboard there.
    pub fn unsent_to_draft(&mut self,text:&str,cx:&mut Context<Self>){let d=self.input.read(cx).content.to_string();let merged=if d.trim().is_empty(){text.to_string()}else{format!("{d}\n\n{text}")};self.input.update(cx,|i,cx|i.set(merged.clone(),cx));self.screens.drafts.set(&self.selected,&merged);self.drafts.insert(self.selected.clone(),merged);self.want_focus=true;}
    /// The window's unsent messages go, one at a time, while it is connected (each answer sends the next).
    fn send_unsent(&mut self){if !self.screens.unsent.live{return;}if let Some(i)=self.screens.unsent.next(){if self.tx.try_send(hub::Command::Ask("turn.start".into(),screens::unsent::Unsent::request(&i),screens::unsent::TAG)).is_err(){self.screens.unsent.lost();}}}
    fn echo_sent(&mut self,text:&str){
        if self.data["thread"]["id"].as_str().map_or(true,str::is_empty)||self.data["thread"]["loading"]==true{return;}
        let Some(rows)=self.data["thread"]["entries"].as_array_mut() else {return};
        let n=rows.len();self.echo_seq+=1;
        rows.push(serde_json::json!({"id":format!("local-echo-{}",self.echo_seq),"kind":"user","text":text,"stamp":"","blocks":[],"attachments":[]}));
        self.timeline.splice(n..n,1);self.screens.born.insert(format!("local-echo-{}",self.echo_seq),Instant::now());
    }
    /// A thread (or a project with its first thread) is being made on the person's say: the first new thread row to appear is opened.
    pub fn arm_new(&mut self){self.arm_new=Some((Instant::now(),self.thread_ids().into_iter().collect()));}
    fn adopt_new(&mut self,cx:&mut Context<Self>){
        let Some((at,known))=self.arm_new.as_ref() else {return};
        // (a thread made on a linked machine shows once that machine has pushed it: a few seconds, more while it sets up a worktree)
        if at.elapsed()>Duration::from_secs(30){self.arm_new=None;return;}
        let fresh=self.thread_ids().into_iter().find(|t|!known.contains(t));
        if let Some(id)=fresh{self.arm_new=None;self.select_id(id,cx);self.want_focus=true;}
    }
    fn thread_ids(&self)->Vec<String> {
        let mut ids=Vec::new();
        for project in self.data["projects"].as_array().into_iter().flatten() {
            for thread in project["threads"].as_array().into_iter().flatten().chain(project["settled"].as_array().into_iter().flatten()) {ids.push(string(thread,"id"));}
        }
        ids
    }
    fn user_entries(&self)->usize {self.data["thread"]["entries"].as_array().map_or(0,|e|e.iter().filter(|e|e["kind"]=="user").count())}
    /// Ctrl/Cmd-C with no text field focused (a click in the timeline leaves none): copy what is selected there.
    fn copy_selection(&mut self,_:&CopySelection,_:&mut Window,cx:&mut Context<Self>){self.timeline_copy(cx);}
    fn close_popup(&mut self,_:&ClosePopup,window:&mut Window,cx:&mut Context<Self>){if screens::migrate::is_up(&self.data){return;}if viewers::boards::escape(self,cx){return;}if self.pad_escape(window,cx){return;}if self.ctx_close(cx){return;}if self.screens.acct.menu{screens::accounts::toggle(self,window,cx);return;}if self.picker_keys(){self.picker_close(cx);return;}if screens::filesui::escape(self,window,cx){return;}if self.palette_close(cx){return;}if self.find_close(cx){return;}if screens::histbar::escape(self){cx.notify();return;}if self.screens.lightbox.take().is_some(){cx.notify();return;}if self.screens.picker_open{self.picker_close(cx);return;}if screens::settings::is_open(self){screens::settings::close(self,cx);return;}if self.data["tour"].is_object(){screens::settings::flush_fields(self);let _=self.tx.try_send(Command::Action("ob-skip".into(),String::new()));cx.notify();return;}if screens::improvements::escape(self,cx){return;}if self.term_hide(cx){return;}if self.screens.panels.sub_view.is_some(){self.sub_back(cx);return;}if let Some(a)=screens::projects::close_on_escape(&self.data){let _=self.tx.try_send(a);cx.notify();return;}if let Some(a)=screens::tools::close_question(&self.data) {let _=self.tx.try_send(a);}else if self.data["settings"].is_object() {let _=self.tx.try_send(Command::Action("flag".into(),"settings".into()));}else if let Some(a)=screens::bots::close_form(&self.data) {let _=self.tx.try_send(a);}else if self.input.read(cx).popup_open {let draft=self.input.read(cx).content.to_string();self.screens.popup_off=Some(draft);}else{self.escape_rest(window,cx);}cx.notify();}
}
/// Every key binding, from the defaults and this device's shortcut choices (called again when one changes).
pub fn bind_all(cx:&mut App,l:&local::Local){keys::bind_all(cx,l);}
static OPENED:std::sync::LazyLock<(async_channel::Sender<String>,async_channel::Receiver<String>)>=std::sync::LazyLock::new(||async_channel::bounded(8));
/// The window could not be made (no display, a surface the compositor refuses): say so and leave with an error code, instead of a panic that aborts into a core dump.
fn window_failed<T>(e: anyhow::Error) -> T { eprintln!("Backplane: cannot open its window: {e:#}"); std::process::exit(1) }
/// Quit from the next executor turn: the platform may still be inside its own handler for the close (X11 holds its client borrowed there, and `quit` borrows it again: "already borrowed").
fn quit_later(cx: &mut App) { cx.spawn(async move |cx| { let _ = cx.update(|cx| cx.quit()); }).detach(); }

fn main()->anyhow::Result<()> {
    #[cfg(feature="cef")]
    if let Some(code)=viewers::cef::subprocess(){std::process::exit(code);} // a CEF helper process (renderer, gpu...) is this executable run again
    // `--install [DIR]`: the headless install of an unpacked Windows release (install.rs); nothing else runs
    #[cfg(windows)] if let Some(dir)=install::install_arg(&std::env::args().collect::<Vec<_>>()) {std::process::exit(install::headless(dir));}
    if let Some(why)=plat::display_problem(std::env::var("DISPLAY").ok().as_deref(),std::env::var("WAYLAND_DISPLAY").ok().as_deref()) {eprintln!("backplane-gpui: {why}");eprintln!("GPUI_LOG {}",serde_json::json!({"no_display":why}));std::process::exit(3);}
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let started=Instant::now();let config=hub::Config::parse()?;let exit_after=config.exit_after;
    if config.swatches||config.gallery||config.fonts||std::env::var_os("BACKPLANE_GPUI_RECTS").is_some(){art::enable_rects();}
    script::metric(started,"process_start",serde_json::json!({}));
    // Windows: an unpacked release asks to install itself where the guard can update it (install.rs); on Install the installed one runs
    #[cfg(windows)] if install::maybe_install(config.pending) { return Ok(()) }
    let app=Application::new().with_assets(art::Assets);
    // macOS hands a backplane:// link to the app itself, at launch and while it runs
    app.on_open_urls(|urls|for u in urls {if pairing::is_link(&u) {let _=OPENED.0.try_send(u);}});
    app.run(move|cx:&mut App|{
        art::load_fonts(cx);bind_all(cx,&local::Local::load());
        cx.on_window_closed(|cx|if cx.windows().is_empty(){quit_later(cx);}).detach();
        let bounds=Bounds::centered(None,size(px(1400.),px(900.)),cx);
        let opts=WindowOptions{window_bounds:Some(WindowBounds::Windowed(bounds)),titlebar:Some(TitlebarOptions{title:Some("Backplane GPUI · experimental".into()),..Default::default()}),..Default::default()};
        if let Some(spec)=config.solid_demo.clone() { // --solid-demo TRIS|FILE[,STEPS]: the 3D page alone on a synthetic model or a frame file (measurement, check mode)
            let mut it=spec.split(',');let source=it.next().unwrap_or("100000").to_string();let steps=it.next().and_then(|t|t.parse().ok()).unwrap_or(200);
            cx.open_window(opts,move|_,cx|cx.new(|cx|viewers::demo::Demo::new(cx,&source,steps,started))).unwrap_or_else(window_failed);
        } else {
            let measure=config.script.is_some();
            if config.pending { // no --url: this device's own hub, or the chooser
                let mut c=config.clone();c.launched=true;let saved=launch::load();
                let env=std::env::var_os("BACKPLANE_HOME").filter(|h|!h.is_empty()).map(std::path::PathBuf::from);
                // this computer's hub is the one this client was started on, when it kept one; only a client never given one falls through to launch.json
                if let (Some((url,tf)),true)=(hubs::local_target(&hubs::load()),env.is_none()) {c.hub=url.replacen("http://","ws://",1);c.token_file=tf;c.pending=false;cx.open_window(opts,move|window,cx|cx.new(|cx|Backplane::new(window,cx,c,started))).unwrap_or_else(window_failed);}
                else if !saved.url.is_empty()&&env.is_none() {c.hub=saved.url.replacen("http://","ws://",1);c.token_file=saved.token_file.into();c.pending=false;c.paired=true;/* a hub the chooser paired with: its 401 means this device was revoked */cx.open_window(opts,move|window,cx|cx.new(|cx|Backplane::new(window,cx,c,started))).unwrap_or_else(window_failed);}
                else {let auto=env.or_else(||(!saved.home.is_empty()).then(||saved.home.clone().into()));cx.open_window(opts,move|window,cx|cx.new(|cx|screens::first::First::new(window,cx,c,started,auto))).unwrap_or_else(window_failed);}
            } else {
            cx.open_window(opts,move|window,cx|cx.new(|cx|{let mut b=Backplane::new(window,cx,config,started);b.screens.plotview.measure=measure;b})).unwrap_or_else(window_failed);
            }
        }
        cx.activate(true);
        if let Some(secs)=exit_after {cx.spawn(async move|cx|{cx.background_executor().timer(Duration::from_secs_f64(secs)).await;let _=cx.update(|cx|cx.quit());}).detach();}
    });Ok(())
}
