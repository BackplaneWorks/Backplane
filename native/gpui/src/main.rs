mod art;
mod delta;
mod hub;
mod input;
mod local;
mod screens;
mod script;
mod swatch;
mod ui;
mod viewers;
mod theme;
mod wire;
use gpui::{prelude::*, *, actions};
use serde_json::Value;
use std::{collections::{HashMap,VecDeque},time::{Duration,Instant}};
use script::{Pending,Step};
use hub::{Command,Event};
actions!(backplane,[Send,ClosePopup,CopySelection,ToggleSidebarKey,FocusSearchKey]);
pub struct Backplane {
    pub input:Entity<screens::composer::Composer>, data:Value, status:String, error:String,
    tx:tokio::sync::mpsc::Sender<Command>, selected:String,
    drafts:HashMap<String,String>, started:Instant, usable:bool,
    first_frame:bool, script_done:bool, pub timeline:ListState, pub cats:art::Cats, pub links_sent:bool, pub links_last:String, pub sidebar_open:bool, pub local:local::Local, pub echo_seq:u64, pub thread_cache:Vec<(String,Value)>, pub thread_source:&'static str, pub confirm:Option<(String,Instant)>, pub ack_pending:Option<(String,Instant)>, pub recording:Option<(String,Vec<f64>)>, pub ui_epoch:u64, pub controls:Vec<script::CtlEntry>, pub ctl_last:String, pub last_layout:String, pub wake_at:Option<Instant>, pub reconnected:bool, pub click_info:Option<(String,String)>, pub timing:Option<hub::Timing>, pub applied_at:Option<Instant>, pub apply_ms:f64, pub render_us:u64, swatches:bool, gallery:bool, fonts:bool, theme_override:Option<bool>, steps:VecDeque<Step>, pending:Option<Pending>, seq:u64, want_focus:bool, script_total:usize, screens:screens::Screens,
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
    fn new(window:&mut Window,cx:&mut Context<Self>,config:hub::Config,started:Instant)->Self {
        let (swatches,gallery,fonts,theme)=(config.swatches,config.gallery,config.fonts,config.theme.clone());let steps:VecDeque<Step>=config.script.as_ref().map(|p|script::load(p)).unwrap_or_default();let script_total=steps.len();
        // a client that never gets projects says so (a hub with a canned presenter, a refused token ...) instead of leaving a script waiting silently
        cx.spawn(async move |weak,cx|{cx.background_executor().timer(Duration::from_secs(10)).await;let _=weak.update(cx,|this,_|{if !this.usable {this.m("not_usable",serde_json::json!({"after_s":10,"status":this.status,"error":this.error,"projects":this.data["projects"].as_array().map_or(0,|p|p.len())}));}});}).detach();
        let input=cx.new(screens::composer::Composer::new);input.focus_handle(cx).focus(window);
        let local=local::Local::load();
        let target=screens::attach::Target::from(&config.hub,&config.token_file);let drafts_path=config.token_file.with_file_name("gpui-drafts.json");let saved=screens::drafts::load(&drafts_path);
        let(tx,events)=hub::connect(config);
        screens::place::watch(cx);
        cx.spawn(async move |weak,cx|{while let Ok(event)=events.recv().await {if weak.update(cx,|this,cx|this.receive(event,cx)).is_err(){break;}}}).detach();
        cx.observe(&input,|this,_,cx|{
            let text=this.input.read(cx).content.to_string();
            let changed=this.draft_changed(&text);
            if changed {this.send_draft(text,cx);}
            cx.notify();
        }).detach();
        cx.subscribe(&input,|this,_,event:&screens::composer::ComposerEvent,cx|this.attach_event(event,cx)).detach();
        cx.on_app_quit(|this,_|{this.screens.drafts.flush();async{}}).detach();
        Self{input,data:Value::Null,status:"Connecting…".into(),error:art::assets_missing().unwrap_or_default(),tx,selected:String::new(),drafts:saved.clone(),started,usable:false,first_frame:false,script_done:false,timeline:ListState::new(0,ListAlignment::Bottom,px(screens::timeline::RUNWAY)),cats:art::Cats::new(),links_sent:false,links_last:String::new(),sidebar_open:local.sidebar_open,local,echo_seq:0,thread_cache:Vec::new(),thread_source:"hub",confirm:None,ack_pending:None,recording:None,ui_epoch:0,controls:Vec::new(),ctl_last:String::new(),last_layout:String::new(),wake_at:None,reconnected:false,click_info:None,timing:None,applied_at:None,apply_ms:0.0,render_us:0,swatches,gallery,fonts,theme_override:theme.as_deref().map(|t|t=="light"),steps,pending:None,seq:0,want_focus:false,script_total,screens:screens::Screens::new(cx,target,drafts_path,saved)}
    }
    fn receive(&mut self,event:Event,cx:&mut Context<Self>) {
        if let Some(s)=hub::status_for(&event){self.status=s.into();}
        match event {
            Event::NetWake=>{self.wake_at=Some(Instant::now());self.m("net",serde_json::json!({"event":"hub_reachable_again"}));},
            Event::Picture(r,timing)=>{
                let t_apply=Instant::now();self.ui_epoch+=1;
                if let Some(w)=self.wake_at.take(){self.reconnected=true;self.m("net",serde_json::json!({"event":"reconnected","ms":(w.elapsed().as_secs_f64()*1e6).round()/1e3}));}
                let mut r=r;let prev=self.data["thread"].take();self.data=r["shown"].take();self.picture_applied(cx);
                self.timeline_picture(prev,&r["window"]);
                if self.data["deleting"].is_object(){eprintln!("GPUI_LOG {{\"deleting\":{}}}",self.data["deleting"]["id"]);}let selected=string(&self.data["thread"],"id");
                if !selected.is_empty() {let selected=screens::attach::raw_id(&selected).to_string();self.screens.menu_open=screens::tools::menu_keeps(self.screens.menu_open,&self.selected,&selected);self.picture_arrived(&selected,cx);self.selected=selected;}
                self.thread_source="hub";
                if let Some((label,t0))=self.confirm.take(){self.m("input",serde_json::json!({"step":label,"ok":true,"latency_ms":(t0.elapsed().as_secs_f64()*1e6).round()/1e3,"source":"hub"}));}
                self.apply_ms=t_apply.elapsed().as_secs_f64()*1000.0;self.timing=Some(timing);self.applied_at=Some(Instant::now());
            },
            Event::Plot(b)=>self.plot_received(b,cx),
            Event::Answer(id,reply)=>screens::improvements::answered(self,id,reply,cx),
            Event::Rpc(tag,ok,why)=>screens::settings::rpc_done(self,tag,ok,&why,cx),
            Event::PlotTooBig(n)=>self.screens.plot.too_big(n),
            Event::Status(s)=>{self.status=s;if self.status=="Connected"{self.error.clear();}},
            Event::Accepted=>{self.input.update(cx,|input,cx|input.set(String::new(),cx));self.drafts.remove(&self.selected);self.timeline_to_end();},
            Event::Older(cursor,page)=>self.older_landed(cursor,page,cx),
            Event::Error(e)=>{self.error=e;},
            Event::Refused(e)=>{self.error=e;},
        }
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
        if name.starts_with("shortcut"){let l=self.local.clone();cx.defer(move|cx|bind_all(cx,&l));}
        self.local.save();self.ui_epoch+=1;cx.notify();
    }
    /// The gear: this device's page opens at once (no round trip), and the hub is asked for its pages beside it.
    pub fn open_settings(&mut self,cx:&mut Context<Self>){
        let hub_open=self.data["settings"].is_object();
        let ui=&mut self.screens.settings_ui;
        if ui.open_local||hub_open{ui.open_local=false;ui.device=false;} else {ui.open_local=true;ui.device=true;}
        let _=self.tx.try_send(Command::Action("flag".into(),"settings".into()));cx.notify();
    }
    pub fn focus_search(&mut self,cx:&mut Context<Self>){if !self.sidebar_open{self.sidebar_toggle(cx);}self.ui_epoch+=1;self.screens.projects.focus=Some("search");cx.notify();}
    fn select(&mut self,id:String,window:&mut Window,cx:&mut Context<Self>) {self.select_id(id,cx);self.input.focus_handle(cx).focus(window);}
    fn select_id(&mut self,id:String,cx:&mut Context<Self>) {self.ui_epoch+=1;
        let raw=id.rsplit_once('|').map_or(id.as_str(),|(_,id)|id).to_string();
        self.screens.menu_open=screens::tools::menu_keeps(self.screens.menu_open,&self.selected,&raw);
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
    fn do_send(&mut self,cx:&mut Context<Self>) {self.ui_epoch+=1;
        let text=self.input.read(cx).content.to_string();
        if !text.trim().is_empty()&&self.status=="Connected" {self.echo_sent(&text);let _=self.tx.try_send(Command::Send(text));self.error.clear();}cx.notify();
    }
    /// The message is drawn at the end of the open thread the moment it is sent; the hub's picture, which has the real entry, replaces it (history::merge
    /// keeps the rows up to the first one the hub also has, so an echo past it is dropped, not doubled).
    fn echo_sent(&mut self,text:&str){
        if self.data["thread"]["id"].as_str().map_or(true,str::is_empty)||self.data["thread"]["loading"]==true{return;}
        let Some(rows)=self.data["thread"]["entries"].as_array_mut() else {return};
        let n=rows.len();self.echo_seq+=1;
        rows.push(serde_json::json!({"id":format!("local-echo-{}",self.echo_seq),"kind":"user","text":text,"stamp":"","blocks":[],"attachments":[]}));
        self.timeline.splice(n..n,1);
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
    fn close_popup(&mut self,_:&ClosePopup,_:&mut Window,cx:&mut Context<Self>){if screens::improvements::escape(self,cx){return;}if let Some(a)=screens::projects::close_on_escape(&self.data){let _=self.tx.try_send(a);cx.notify();return;}if self.screens.menu_open {self.screens.menu_open=false;}else if let Some(a)=screens::tools::close_question(&self.data) {let _=self.tx.try_send(a);}else if self.data["settings"].is_object() {let _=self.tx.try_send(Command::Action("flag".into(),"settings".into()));}else if let Some(a)=screens::bots::close_form(&self.data) {let _=self.tx.try_send(a);}else{let _=self.tx.try_send(Command::Action("skill-close".into(),String::new()));}cx.notify();}
}
/// Every key binding, from the defaults and this device's shortcut choices (called again when one changes).
pub fn bind_all(cx:&mut App,l:&local::Local){
    cx.clear_key_bindings();
    input::bind(cx);screens::composer::bind(cx);screens::improvements::bind(cx);
    let send=l.key("send");
    cx.bind_keys([KeyBinding::new(&send,Send,Some("Composer")),KeyBinding::new("ctrl-enter",Send,Some("Composer")),KeyBinding::new(&l.key("close"),ClosePopup,None),KeyBinding::new("secondary-c",CopySelection,None),
        KeyBinding::new(&l.key("sidebar"),ToggleSidebarKey,None),KeyBinding::new(&l.key("search"),FocusSearchKey,None)]);
}
fn main()->anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let started=Instant::now();let config=hub::Config::parse()?;let exit_after=config.exit_after;
    if config.swatches||config.gallery||config.fonts||std::env::var_os("BACKPLANE_GPUI_RECTS").is_some(){art::enable_rects();}
    script::metric(started,"process_start",serde_json::json!({}));
    Application::new().run(move|cx:&mut App|{
        art::load_fonts(cx);bind_all(cx,&local::Local::load());
        cx.on_window_closed(|cx|if cx.windows().is_empty(){cx.quit();}).detach();
        let bounds=Bounds::centered(None,size(px(1400.),px(900.)),cx);
        let opts=WindowOptions{window_bounds:Some(WindowBounds::Windowed(bounds)),titlebar:Some(TitlebarOptions{title:Some("Backplane GPUI · experimental".into()),..Default::default()}),..Default::default()};
        if let Some(spec)=config.solid_demo.clone() { // --solid-demo TRIS[,STEPS]: the 3D page alone on a synthetic model (measurement, check mode)
            let mut it=spec.split(',');let tris=it.next().and_then(|t|t.parse().ok()).unwrap_or(100_000);let steps=it.next().and_then(|t|t.parse().ok()).unwrap_or(200);
            cx.open_window(opts,move|_,cx|cx.new(|cx|viewers::demo::Demo::new(cx,tris,steps,started))).expect("open GPUI window");
        } else {
            let measure=config.script.is_some();
            cx.open_window(opts,move|window,cx|cx.new(|cx|{let mut b=Backplane::new(window,cx,config,started);b.screens.plotview.measure=measure;b})).expect("open GPUI window");
        }
        cx.activate(true);
        if let Some(secs)=exit_after {cx.spawn(async move|cx|{cx.background_executor().timer(Duration::from_secs_f64(secs)).await;let _=cx.update(|cx|cx.quit());}).detach();}
    });Ok(())
}
