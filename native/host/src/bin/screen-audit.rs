//! Counts-only screen audit. No hub, helpers, agents or writes; bodies are read one indexed page at a time.
//! Only synthetic logs are accepted; stdout contains counts and fixed refusal labels, never content or ids.
#[path="../state/mod.rs"] mod state;
#[path="../store.rs"] mod store;
#[path="../plat/mod.rs"] #[allow(dead_code)] mod plat;
#[path="../ckpt.rs"] mod ckpt;
#[path="../startup.rs"] mod startup;
#[path="../model.rs"] mod model;
#[path="../stamp.rs"] mod stamp;
#[path="../md/mod.rs"] mod md;
#[path="../shelf.rs"] mod shelf;
#[path="../signal.rs"] mod signal;
#[path="../sidebar.rs"] mod sidebar;
#[path="../session/slash.rs"] mod slash;
#[path="../session/composer.rs"] pub mod composer;
#[path="../session/patch.rs"] pub mod patch;
#[path="../session/select.rs"] pub mod select;
#[path="../session/pages.rs"] pub mod pages;
mod session { pub use crate::{composer, patch, select, pages}; }
#[path="../present/tokstats.rs"] mod tokstats;
#[path="../present/tz.rs"] mod tz;
#[path="../present/keys.rs"] mod keys;
#[path="../settings/order_view.rs"] mod order_view;
// the speed dial's table (agents/speed.rs) asks for the detected models; the audit counts screens and detects none
mod settings { pub mod order { pub use crate::order_view::*; } pub mod models { pub fn info() -> String { String::new() } } }
#[path="../present/thread.rs"] mod thread;
#[path="../present/window.rs"] mod window;
#[path="../present/audit_files.rs"] mod files;
#[path="../present/design_mode.rs"] mod design_mode;
#[path="../present/rail.rs"] mod rail;
#[path="../present/viewer.rs"] mod viewer;
#[path="../present/panels.rs"] mod panels;
#[path="../present/ready.rs"] pub mod ready;
mod identity_context {
    use serde_json::Value;
    pub fn facts(_: &crate::state::State, _: &Value, _: &str, local_docs: &Value) -> Value { local_docs.clone() }
    pub fn browser(_: &Value, _: &str) -> (String, String) { Default::default() }
    pub fn far_exists(_: &str, _: &str) -> Option<bool> { None }
}
mod present { pub use crate::ready; }
// the issue links' repository (present/thread.rs): the audit reads no project's git config, so it links nothing
mod mirror { pub fn origin_of_root(_: &str) -> String { String::new() } }
/// kicad/follow.rs keeps the hub to redraw the views after a follow; the audit has none.
#[allow(dead_code)] mod hub { pub struct Hub; impl Hub { pub fn bump_views(&self) {} } }
#[path="../agents/bg.rs"] pub mod bg_;
#[path="../agents/inputs.rs"] pub mod inputs_;
#[path="../agents/skills.rs"] pub mod skills_;
#[path="../agents/lockrec.rs"] pub mod lockrec_;
#[path="../agents/halt_view.rs"] pub mod halt_;
#[path="../agents/review_mode.rs"] pub mod review_mode_;
#[path="../agents/unanswered.rs"] pub mod unanswered_;
#[path="../agents/speed.rs"] pub mod speed_;
// (the audit draws this machine's threads only: a project on another machine has no listing here)
mod net { pub mod farfiles { pub fn listing(_: &str, _: &str) -> Option<Result<String, String>> { None } pub fn copy(_: &str, _: &str) -> Option<Result<String, String>> { None } } }
mod agents { pub use crate::bg_ as bg; pub use crate::inputs_ as inputs; pub use crate::skills_ as skills; pub use crate::lockrec_ as lockrec; pub use crate::halt_ as halt; pub use crate::unanswered_ as unanswered; pub use crate::review_mode_ as review_mode; pub use crate::speed_ as speed; }
use serde_json::json;
use std::{collections::{BTreeMap,HashSet},path::Path};
fn main() -> anyhow::Result<()> {
    let path=std::env::args().nth(1).ok_or_else(||anyhow::anyhow!("usage: screen-audit SYNTHETIC_LOG"))?;
    let path=Path::new(&path);
    anyhow::ensure!(path.file_name().and_then(|s|s.to_str()).map_or(false,|s|s.starts_with("synthetic-")),"synthetic log required");
    let store=store::Store::open(path)?;
    let model=model::Model::build(&store)?;
    let mut st=state::State::new(); st.lean=true;
    for at in (0..store.len()).step_by(2000) { for line in store.range(at,at+2000)? { st.apply_line(&line); } }
    let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let open=HashSet::new();
    let opts=sidebar::Opts{hub:"audit",now,selected:"",open:&open,older_open:false,query:""};
    let plan=sidebar::plan(&st,&opts); let visible:HashSet<&str>=plan.ids().into_iter().collect();
    let mut reasons:BTreeMap<&str,usize>=BTreeMap::new(); let mut selected_matches=0; let mut selected_reason="thread.not-found"; let mut requested_775_utc_gate_count=0;
    for (id,t) in &st.threads {
        let reason=if let Some(reason)=thread::metadata_refusal(&st,id) { reason } else {
            match model.page(&store,id,0,32) {
                Err(_)=>"page.read",
                Ok(page)=>{
                    let mut p=json!({"target":{"kind":"thread","id":id},"now":now*1000,"session":{"drafts":{},"info":{}},"page":{"thread":id,"rows":page.rows.iter().map(model::Row::json).collect::<Vec<_>>()}});
                    match window::window(&page,&[],"audit",0,&p) {
                        None=>"window.unsupported",
                        Some(w)=>{p["window"]=w; match thread::thread_object_checked(&st,"audit",id,&p,"") {
                            Err(reason)=>reason,
                            Ok(_)=>if st.settings.get("sidebar.view").map_or(false,|v|v=="active") {"sidebar.active-mode"} else if !t.project.is_empty()&&!visible.contains(t.project.as_str()) {"sidebar.selected-project-hidden"}else{"supported"},
                        }}
                    }
                }
            }
        };
        *reasons.entry(reason).or_default()+=1;
        if t.title=="Thread 12415" { selected_matches+=1; selected_reason=reason;
            // Historical 77542a24de accepted only these UTC strings for all three zone settings.
            // Fixed counters expose that old gate without printing zone names or any setting values.
            requested_775_utc_gate_count+=["person.tz","person.tz.device","tz.machine"].iter().filter(|k|st.settings.get(**k).map_or(false,|v|!matches!(v.trim(),""|"UTC"|"Etc/UTC"))).count(); }
    }
    println!("{}",json!({"events":store.len(),"threads":st.threads.len(),"first_reason_histogram":reasons,"requested_thread_matches":selected_matches,"requested_thread_reason":selected_reason,"requested_775_non_utc_zone_gate_count":requested_775_utc_gate_count}));
    Ok(())
}
