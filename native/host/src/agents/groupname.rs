//! A sidebar group's name picked by a light model, the way project icons are (agents/iconpick.rs): Haiku reads the group's raw label (its git
//! owner or folder) and its projects, and answers a short name ("American Embedded", "Nordic"). Kept in `sidebar.group.pick.<gid>` once per
//! group; the person's name (`sidebar.group.name.<gid>`) always wins. Off to the side: one group at a time, never on a client's request path,
//! nothing in safe or copy mode, never in unit tests.
use super::{textgen, Agents};
use crate::hub::Hub;
use serde_json::json;
use std::{collections::HashMap, sync::{Arc, Mutex, OnceLock}, time::Duration};

const RETRY: u64 = 6 * 3600;
const ROUND: Duration = Duration::from_secs(60);

pub fn system() -> String {
    "You name a group of projects in an engineering app's sidebar. The group's raw label (a git owner or a folder name) and its projects follow. \
Do not answer them, do not do the work and do not use tools. Reply with only the group's name: one to four words, the way a person would say who \
the work is for (a company, a client, a team, or \"personal\"), with normal capitals and spaces (AmericanEmbedded becomes American Embedded). \
No quotes, no punctuation at the end.".to_string()
}
/// The answer as a name: its first line, quotes and a trailing full stop dropped, at most four words and 32 characters; "" for nothing usable.
pub fn parse(answer: &str) -> String {
    let l = answer.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let l = l.trim_matches(|c: char| c == '"' || c == '\'' || c == '`' || c == '*' || c == '.' || c.is_whitespace());
    let l = l.strip_prefix("Name:").map(str::trim).unwrap_or(l);
    let w: Vec<&str> = l.split_whitespace().take(4).collect();
    w.join(" ").chars().take(32).collect()
}
pub fn input(raw: &str, members: &[String]) -> String { format!("Raw label: {raw}\nProjects:\n{}\n", members.iter().map(|m| format!("- {m}")).collect::<Vec<_>>().join("\n")) }

fn failed() -> &'static Mutex<HashMap<String, u64>> { static F: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new(); F.get_or_init(Default::default) }

/// `before_recover`: the time this start began, before its recovery cut any turn (agents/mod.rs).
pub fn start(ag: &Arc<Agents>, hub: &Arc<Hub>, before_recover: u64) {
    if cfg!(test) { return }
    // the look marks' baseline (state/visible.rs `looked`): once, at the first start with the sections, a copy's first start too. It is the time
    // before this start's recovery, so a turn the restart into this build cut shows (strictly older stops count as seen).
    { let h = hub.clone(); tokio::spawn(async move { if h.setting("sidebar.looked.since").await.unwrap_or_default().is_empty() {
        let _ = h.commit_many(&[json!({"$":"SettingSet","key":"sidebar.looked.since","value":before_recover.to_string(),"at":super::now_s()})]).await; } }); }
    if crate::guard::safe() || crate::copy::on() { return }
    let me = Arc::downgrade(ag);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(40)).await;
        loop {
            let Some(ag) = me.upgrade() else { return };
            ag.group_round().await; drop(ag);
            tokio::time::sleep(ROUND).await;
        }
    });
}

impl Agents {
    async fn group_round(&self) {
        let now = super::now_s();
        let Some(pol) = crate::statehost::policies() else { return };
        let m = self.hub.mirrors();
        let due = pol.with_state(|st| crate::present::merged::merged_of(st, m, |rows| crate::present::sections::groups_due(st, &rows))).unwrap_or_default();
        let due: Vec<_> = { let f = failed().lock().unwrap(); due.into_iter().filter(|g| f.get(&g.0).map_or(true, |at| now >= at + RETRY)).collect() };
        for (gid, raw, members) in due {
            match self.shot_healthy(textgen::TITLE_MODEL, &system(), &input(&raw, &members), "swarm names").await.map(|a| parse(&a)).filter(|n| !n.is_empty()) {
                Some(name) => { self.commit_events(vec![json!({"$":"SettingSet","key":format!("sidebar.group.pick.{gid}"),"value":name,"at":super::now_s()})]).await; eprintln!("groups: {gid}: {name}"); }
                None => { eprintln!("groups: {gid}: no answer; asked again later"); failed().lock().unwrap().insert(gid, super::now_s()); }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn an_answer_becomes_a_short_name() {
        assert_eq!(parse("American Embedded"), "American Embedded");
        assert_eq!(parse("\"Nordic\".\n\nBecause..."), "Nordic");
        assert_eq!(parse("Name: i2cjak personal projects and more words"), "i2cjak personal projects and");
        assert_eq!(parse("   \n"), "");
        let i = input("AmericanEmbedded", &["FazePro (github.com/AmericanEmbedded/fazepro)".into()]);
        assert!(i.contains("Raw label: AmericanEmbedded") && i.contains("- FazePro"), "{i}");
    }
}
