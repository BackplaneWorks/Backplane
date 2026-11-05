mod hub;
mod input;
mod script;
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
    tx:tokio::sync::mpsc::UnboundedSender<Command>, selected:String,
    drafts:HashMap<String,String>, popup:bool, started:Instant, usable:bool,
    first_frame:bool, script_done:bool, steps:VecDeque<Step>, pending:Option<Pending>, seq:u64, want_focus:bool, script_total:usize,
}
fn pager_row(cx:&mut Context<Backplane>,id:String,pager:&Value,c:&dyn Fn(&str)->Rgba)->Option<AnyElement> {
    let mut row=div().flex().gap_2().px_3().py_1().text_size(px(11.));let mut any=false;
    for (name,key) in [("Newer","previous"),("Older","next")] {
        let control=&pager[key];
        if let Some(value)=control["value"].as_str() {
            any=true;let value=value.to_string();let label=format!("{} ({})",name,pager["total"]);
            row=row.child(div().id(SharedString::from(format!("{id}-{key}"))).cursor_pointer().text_color(c("accent")).hover(|s|s.bg(c("raise"))).px_1().child(label)
                .on_click(cx.listener(move|this,_,_,cx|{let _=this.tx.send(Command::Page(value.clone()));cx.notify();})));
        }
    }
    if any {Some(row.into_any_element())} else {None}
}
fn string(v:&Value,key:&str)->String {v[key].as_str().unwrap_or("").to_string()}
impl Backplane {
    fn new(window:&mut Window,cx:&mut Context<Self>,config:hub::Config,started:Instant)->Self {
        let steps:VecDeque<Step>=config.script.as_ref().map(|p|script::load(p)).unwrap_or_default();let script_total=steps.len();
        let input=cx.new(input::TextInput::new);input.focus_handle(cx).focus(window);
        let(tx,events)=hub::connect(config);
        cx.spawn(async move |weak,cx|{while let Ok(event)=events.recv().await {if weak.update(cx,|this,cx|this.receive(event,cx)).is_err(){break;}}}).detach();
        cx.observe(&input,|this,_,cx|{
            let text=this.input.read(cx).content.to_string();
            this.drafts.insert(this.selected.clone(),text.clone());
            this.popup=text.starts_with('/')||text.starts_with('$');
            if this.popup {let _=this.tx.send(Command::Draft(text));}
            cx.notify();
        }).detach();
        Self{input,data:Value::Null,status:"Connecting…".into(),error:String::new(),tx,selected:String::new(),drafts:HashMap::new(),popup:false,started,usable:false,first_frame:false,script_done:false,steps,pending:None,seq:0,want_focus:false,script_total}
    }
    fn receive(&mut self,event:Event,cx:&mut Context<Self>) {
        match event {
            Event::Picture(r)=>{
                self.data=r["shown"].clone();
                let selected=string(&self.data["thread"],"id");
                if !selected.is_empty() {self.selected=selected;}
            },
            Event::Status(s)=>{self.status=s;if self.status=="Connected"{self.error.clear();}},
            Event::Accepted=>{self.input.update(cx,|input,cx|input.set(String::new(),cx));self.drafts.remove(&self.selected);self.popup=false;},
            Event::Error(e)=>{self.error=e;self.status="Disconnected".into();},
        }
        cx.notify();
    }
    fn select(&mut self,id:String,window:&mut Window,cx:&mut Context<Self>) {self.select_id(id,cx);self.input.focus_handle(cx).focus(window);}
    fn select_id(&mut self,id:String,cx:&mut Context<Self>) {
        let raw=id.rsplit_once('|').map_or(id.as_str(),|(_,id)|id).to_string();
        self.drafts.insert(self.selected.clone(),self.input.read(cx).content.to_string());
        self.selected=raw.clone();let text=self.drafts.get(&raw).cloned().unwrap_or_default();
        self.input.update(cx,|input,cx|input.set(text,cx));self.popup=false;
        let _=self.tx.send(Command::Select(id));self.want_focus=true;cx.notify();
    }
    fn send(&mut self,_:&Send,_:&mut Window,cx:&mut Context<Self>) {self.do_send(cx);}
    fn do_send(&mut self,cx:&mut Context<Self>) {
        let text=self.input.read(cx).content.to_string();
        if !text.trim().is_empty()&&self.status=="Connected" {let _=self.tx.send(Command::Send(text));self.error.clear();}cx.notify();
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
impl Render for Backplane {
    fn render(&mut self,window:&mut Window,cx:&mut Context<Self>)->impl IntoElement {
        if self.want_focus {self.want_focus=false;self.input.focus_handle(cx).focus(window);}
        self.frame_marks(window,cx);
        let light=self.data["theme"]=="light";let c=|name|theme::color(light,name);
        let mut rows=Vec::new();
        let projects=self.data["projects"].as_array().cloned().unwrap_or_default();
        for project in projects {
            rows.push(div().px_3().pt_3().pb_1().text_size(px(11.)).text_color(c("dim")).child(string(&project,"title").to_uppercase()).into_any_element());
            for thread in project["threads"].as_array().into_iter().flatten().chain(project["settled"].as_array().into_iter().flatten()) {
                let id=string(thread,"id");let raw=id.rsplit_once('|').map_or(id.as_str(),|(_,id)|id);
                let selected=raw==self.selected;
                let color=match thread["state"].as_str().unwrap_or(""){"running"=>c("accent"),"failed"=>c("bad"),_=>c("faint")};
                rows.push(div().id(SharedString::from(format!("thread-{id}"))).flex().items_center().gap_2().px_3().py_1().border_l_2().border_color(if selected{c("accent")}else{c("panel")})
                    .bg(if selected{c("raise")}else{c("panel")}).hover(|s|s.bg(c("raise"))).cursor_pointer()
                    .on_click(cx.listener(move|this,_,window,cx|this.select(id.clone(),window,cx)))
                    .child(div().size(px(6.)).bg(color)).child(div().flex_1().overflow_hidden().child(string(thread,"title")))
                    .child(div().text_size(px(11.)).text_color(c("faint")).child(string(thread,"ago"))).into_any_element());
            }
        }
        for page in self.data["sidebarPages"].as_array().cloned().unwrap_or_default() {
            for key in ["projects","active","legacy"] {if let Some(row)=pager_row(cx,format!("pg-{key}"),&page[key],&c){rows.push(row);}}
        }
        let thread=&self.data["thread"];let mut messages=Vec::new();
        for entry in thread["entries"].as_array().into_iter().flatten() {
            let own=entry["kind"]=="user";let text=string(entry,"text");
            if text.is_empty(){continue;}
            messages.push(div().id(SharedString::from(format!("entry-{}",string(entry,"id")))).w_full().py_3()
                .child(div().text_size(px(11.)).text_color(if own{c("accent")}else{c("dim")}).mb_1().child(if own{"You"}else if entry["kind"]=="assistant"{"Assistant"}else{"Activity"}))
                .child(div().px(if own{px(12.)}else{px(0.)}).py(if own{px(8.)}else{px(0.)}).bg(if own{c("raise")}else{c("bg")}).child(text))
                .child(div().text_size(px(11.)).text_color(c("faint")).mt_1().child(string(entry,"stamp"))).into_any_element());
        }
        for live in thread["live"].as_array().into_iter().flatten(){messages.push(div().py_2().child(string(live,"text")).into_any_element());}
        if messages.is_empty(){messages.push(div().py_6().text_color(c("dim")).child(if thread.is_object(){"Start a conversation."}else{"Select a thread."}).into_any_element());}
        let skills=thread["skills"].as_array().cloned().unwrap_or_default();
        let mut popup=div().flex().flex_col().border_1().border_color(c("line")).bg(c("panel"));
        if self.popup {for (index,skill) in skills.iter().take(8).enumerate(){let word=skill.get("word").or_else(||skill.get("name")).and_then(Value::as_str).unwrap_or("").to_string();let label=skill.get("label").and_then(Value::as_str).unwrap_or(&word).to_string();popup=popup.child(div().id(("skill",index)).p_2().cursor_pointer().hover(|s|s.bg(c("raise"))).child(label).on_click(cx.listener(move|this,_,window,cx|{let _=this.tx.send(Command::Action("skill".into(),word.clone()));this.input.update(cx,|input,cx|input.set(word.clone()+" ",cx));this.popup=false;this.input.focus_handle(cx).focus(window);cx.notify();})));}}
        div().size_full().flex().bg(c("bg")).text_color(c("text")).text_size(px(13.)).font_family("Noto Sans Mono CJK JP").line_height(px(20.))
            .on_action(cx.listener(Self::send)).on_action(cx.listener(Self::close_popup))
            .child(div().w(px(264.)).h_full().flex_shrink_0().bg(c("panel")).border_r_1().border_color(c("line")).flex().flex_col()
                .child(div().px_3().py_3().border_b_1().border_color(c("line")).flex().items_center().gap_2().child(div().size(px(7.)).bg(if self.status=="Connected"{c("ok")}else{c("bad")})).child("Backplane").child(div().text_size(px(11.)).text_color(c("faint")).child("GPUI")))
                .child(div().px_3().py_2().border_b_1().border_color(c("line")).text_color(c("dim")).child("Projects"))
                .child(div().id("sidebar").overflow_y_scroll().flex_1().children(rows))
                .child(div().px_3().py_2().border_t_1().border_color(c("line")).text_size(px(11.)).text_color(c("dim")).child(self.status.clone())))
            .child(div().flex_1().min_w_0().h_full().flex().flex_col()
                .child(div().px_5().py_3().border_b_1().border_color(c("line")).flex().justify_between().child(if thread.is_object(){string(thread,"title")}else{"Backplane GPUI".into()}).child(div().text_color(c("dim")).child(string(thread,"phase"))))
                .child(div().id("timeline").overflow_y_scroll().flex_1().px_6().children(messages))
                .when(!self.error.is_empty(),|d|d.child(div().px_5().py_2().text_color(c("bad")).child(self.error.clone())))
                .child(div().px_5().py_3().border_t_1().border_color(c("line")).flex().flex_col().gap_2()
                    .when(self.popup&&!skills.is_empty(),|d|d.child(popup))
                    .child(div().border_1().border_color(c("line")).p_2().min_h(px(64.)).child(self.input.clone()))
                    .child(div().flex().justify_between().items_center().child(div().text_size(px(11.)).text_color(c("dim")).child("Enter to send · / commands · $ skills"))
                        .child(div().id("send").px_3().py_1().bg(c("accent")).text_color(c("on-accent")).cursor_pointer().child("Send").on_click(cx.listener(|this,_,window,cx|this.send(&Send,window,cx)))))))
    }
}
fn main()->anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let started=Instant::now();let config=hub::Config::parse()?;let exit_after=config.exit_after;
    script::metric(started,"process_start",serde_json::json!({}));
    Application::new().run(move|cx:&mut App|{
        input::bind(cx);cx.bind_keys([KeyBinding::new("enter",Send,Some("TextInput")),KeyBinding::new("ctrl-enter",Send,Some("TextInput")),KeyBinding::new("escape",ClosePopup,None)]);
        cx.on_window_closed(|cx|if cx.windows().is_empty(){cx.quit();}).detach();
        let bounds=Bounds::centered(None,size(px(1400.),px(900.)),cx);
        cx.open_window(WindowOptions{window_bounds:Some(WindowBounds::Windowed(bounds)),titlebar:Some(TitlebarOptions{title:Some("Backplane GPUI · experimental".into()),..Default::default()}),..Default::default()},move|window,cx|cx.new(|cx|Backplane::new(window,cx,config,started))).expect("open GPUI window");
        cx.activate(true);
        if let Some(secs)=exit_after {cx.spawn(async move|cx|{cx.background_executor().timer(Duration::from_secs_f64(secs)).await;let _=cx.update(|cx|cx.quit());}).detach();}
    });Ok(())
}
