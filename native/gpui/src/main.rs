mod art;
mod delta;
mod hub;
mod input;
mod script;
mod swatch;
mod ui;
mod theme;
mod wire;
use gpui::{prelude::*, *, actions};
use serde_json::Value;
use std::{collections::{HashMap,VecDeque},time::{Duration,Instant}};
use script::{Pending,Step};
use hub::{Command,Event};
actions!(backplane,[Send,ClosePopup]);
pub struct Backplane {
    pub input:Entity<input::TextInput>, data:Value, status:String, error:String,
    tx:tokio::sync::mpsc::Sender<Command>, selected:String,
    drafts:HashMap<String,String>, popup:bool, started:Instant, usable:bool,
    first_frame:bool, script_done:bool, scroll:ScrollHandle, last_entries:usize, pub cats:art::Cats, pub ack_pending:Option<(String,Instant)>, pub recording:Option<(String,Vec<f64>)>, pub ui_epoch:u64, pub controls:Vec<script::CtlEntry>, pub ctl_last:String, pub last_layout:String, pub wake_at:Option<Instant>, pub reconnected:bool, pub click_info:Option<(String,String)>, pub timing:Option<hub::Timing>, pub applied_at:Option<Instant>, pub apply_ms:f64, pub render_us:u64, swatches:bool, gallery:bool, fonts:bool, theme_override:Option<bool>, steps:VecDeque<Step>, pending:Option<Pending>, seq:u64, want_focus:bool, script_total:usize,
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
        let input=cx.new(input::TextInput::new);input.focus_handle(cx).focus(window);
        let(tx,events)=hub::connect(config);
        cx.spawn(async move |weak,cx|{while let Ok(event)=events.recv().await {if weak.update(cx,|this,cx|this.receive(event,cx)).is_err(){break;}}}).detach();
        cx.observe(&input,|this,_,cx|{
            let text=this.input.read(cx).content.to_string();
            this.drafts.insert(this.selected.clone(),text.clone());
            this.popup=text.starts_with('/')||text.starts_with('$');
            if this.popup {let _=this.tx.try_send(Command::Draft(text));}
            cx.notify();
        }).detach();
        Self{input,data:Value::Null,status:"Connecting…".into(),error:art::assets_missing().unwrap_or_default(),tx,selected:String::new(),drafts:HashMap::new(),popup:false,started,usable:false,first_frame:false,script_done:false,scroll:ScrollHandle::new(),last_entries:0,cats:art::Cats::new(),ack_pending:None,recording:None,ui_epoch:0,controls:Vec::new(),ctl_last:String::new(),last_layout:String::new(),wake_at:None,reconnected:false,click_info:None,timing:None,applied_at:None,apply_ms:0.0,render_us:0,swatches,gallery,fonts,theme_override:theme.as_deref().map(|t|t=="light"),steps,pending:None,seq:0,want_focus:false,script_total}
    }
    fn receive(&mut self,event:Event,cx:&mut Context<Self>) {
        match event {
            Event::NetWake=>{self.wake_at=Some(Instant::now());self.m("net",serde_json::json!({"event":"hub_reachable_again"}));},
            Event::Picture(r,timing)=>{
                let t_apply=Instant::now();self.ui_epoch+=1;
                if let Some(w)=self.wake_at.take(){self.reconnected=true;self.m("net",serde_json::json!({"event":"reconnected","ms":(w.elapsed().as_secs_f64()*1e6).round()/1e3}));}
                let mut r=r;self.data=r["shown"].take();
                let entries=self.data["thread"]["entries"].as_array().map_or(0,|e|e.len());
                if entries!=self.last_entries {self.last_entries=entries;self.scroll.set_offset(point(px(0.),px(-1_000_000.)));}
                let selected=string(&self.data["thread"],"id");
                if !selected.is_empty() {self.selected=selected;}
                self.apply_ms=t_apply.elapsed().as_secs_f64()*1000.0;self.timing=Some(timing);self.applied_at=Some(Instant::now());
            },
            Event::Status(s)=>{self.status=s;if self.status=="Connected"{self.error.clear();}},
            Event::Accepted=>{self.input.update(cx,|input,cx|input.set(String::new(),cx));self.drafts.remove(&self.selected);self.popup=false;},
            Event::Error(e)=>{self.error=e;self.status="Disconnected".into();},
        }
        cx.notify();
    }
    fn select(&mut self,id:String,window:&mut Window,cx:&mut Context<Self>) {self.select_id(id,cx);self.input.focus_handle(cx).focus(window);}
    fn select_id(&mut self,id:String,cx:&mut Context<Self>) {self.ui_epoch+=1;
        let raw=id.rsplit_once('|').map_or(id.as_str(),|(_,id)|id).to_string();
        self.drafts.insert(self.selected.clone(),self.input.read(cx).content.to_string());
        self.selected=raw.clone();let text=self.drafts.get(&raw).cloned().unwrap_or_default();
        self.input.update(cx,|input,cx|input.set(text,cx));self.popup=false;
        let _=self.tx.try_send(Command::Select(id));self.want_focus=true;cx.notify();
    }
    fn send(&mut self,_:&Send,_:&mut Window,cx:&mut Context<Self>) {self.do_send(cx);}
    fn do_send(&mut self,cx:&mut Context<Self>) {self.ui_epoch+=1;
        let text=self.input.read(cx).content.to_string();
        if !text.trim().is_empty()&&self.status=="Connected" {let _=self.tx.try_send(Command::Send(text));self.error.clear();}cx.notify();
    }
    fn thread_ids(&self)->Vec<String> {
        let mut ids=Vec::new();
        for project in self.data["projects"].as_array().into_iter().flatten() {
            for thread in project["threads"].as_array().into_iter().flatten().chain(project["settled"].as_array().into_iter().flatten()) {ids.push(string(thread,"id"));}
        }
        ids
    }
    fn user_entries(&self)->usize {self.data["thread"]["entries"].as_array().map_or(0,|e|e.iter().filter(|e|e["kind"]=="user").count())}
    fn close_popup(&mut self,_:&ClosePopup,_:&mut Window,cx:&mut Context<Self>){self.popup=false;cx.notify();}
}
fn main()->anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let started=Instant::now();let config=hub::Config::parse()?;let exit_after=config.exit_after;
    if config.swatches||config.gallery||config.fonts||std::env::var_os("BACKPLANE_GPUI_RECTS").is_some(){art::enable_rects();}
    script::metric(started,"process_start",serde_json::json!({}));
    Application::new().run(move|cx:&mut App|{
        art::load_fonts(cx);input::bind(cx);cx.bind_keys([KeyBinding::new("enter",Send,Some("TextInput")),KeyBinding::new("ctrl-enter",Send,Some("TextInput")),KeyBinding::new("escape",ClosePopup,None)]);
        cx.on_window_closed(|cx|if cx.windows().is_empty(){cx.quit();}).detach();
        let bounds=Bounds::centered(None,size(px(1400.),px(900.)),cx);
        cx.open_window(WindowOptions{window_bounds:Some(WindowBounds::Windowed(bounds)),titlebar:Some(TitlebarOptions{title:Some("Backplane GPUI · experimental".into()),..Default::default()}),..Default::default()},move|window,cx|cx.new(|cx|Backplane::new(window,cx,config,started))).expect("open GPUI window");
        cx.activate(true);
        if let Some(secs)=exit_after {cx.spawn(async move|cx|{cx.background_executor().timer(Duration::from_secs_f64(secs)).await;let _=cx.update(|cx|cx.quit());}).detach();}
    });Ok(())
}
