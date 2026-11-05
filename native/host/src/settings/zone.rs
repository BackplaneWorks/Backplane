//! The person's time zone (core/sched.bend `Zone.*`, hub.bend `Zone.report` / `Zone.set`, server.bend `ZoneJob.*`): `zone.report {zone}` (a phone says
//! where it is: kept as `person.tz.device`) and `zone.set {zone}` (Settings; "" or "auto" follows the phone). The hub keeps each zone's POSIX
//! rule as the setting `tz.rule.<name>`, read from the footer line of the system's TZif file, so routines follow daylight saving.
//! Not ported: parsing and evaluating a POSIX rule (`core/tz.bend`'s `Tz.*`: offset at an instant, wall clock, gaps and overlaps) belongs with
//! the routines, which are the bots lane's; a rule here is only checked to be a plausible footer.
use crate::{api::{Caller, Fut, RpcHandler}, hub::{now_ms, Hub}};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc};

/// Zone.valid: segments of letters, digits, `_` `+` `-` joined by `/`, no leading `/`, 1 to 64 characters.
pub fn valid(name: &str) -> bool {
    (1..=64).contains(&name.chars().count()) && name.split('/').all(|s| !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '+' | '-')))
}
/// Zone.utc
pub fn utc(name: &str) -> bool { matches!(name, "UTC" | "Etc/UTC" | "Etc/UCT" | "UCT") }
/// Zone.fixed.rule: "+0530" (date +%z) as a POSIX rule `<+0530>-5:30`.
pub fn fixed_rule(z: &str) -> String {
    let (sign, hh, mm) = (z.chars().take(1).collect::<String>(), z.chars().skip(1).take(2).collect::<String>(), z.chars().skip(3).take(2).collect::<String>());
    let (h, m): (u32, u32) = (hh.parse().unwrap_or(0), mm.parse().unwrap_or(0));
    format!("<{z}>{}{h}{}", if sign == "-" { "" } else { "-" }, if m == 0 { String::new() } else { format!(":{mm}") })
}
/// A footer is a plausible POSIX rule: a name, then an offset.
fn plausible(rule: &str) -> bool { !rule.is_empty() && rule.len() <= 128 && rule.chars().all(|c| !c.is_control()) && rule.chars().any(|c| c.is_ascii_digit()) && !rule.starts_with(['/', '#']) }

fn zoneinfo() -> PathBuf { std::env::var_os("BACKPLANE_ZONEINFO").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/usr/share/zoneinfo")) }

/// ZoneJob.rule: the zone's rule, "" when the name is not valid or the system does not know it. The footer is the last line of the TZif file
/// (`tail -n 1`); a zone it cannot read that way but the system knows is taken as the offset it has now (`TZ=<name> date +%z`).
pub fn rule(name: &str) -> String {
    if !valid(name) { return String::new() }
    let path = zoneinfo().join(name);
    let Ok(bytes) = std::fs::read(&path) else { return String::new() };
    let text = String::from_utf8_lossy(&bytes);
    let footer = text.trim_end_matches('\n').rsplit('\n').next().unwrap_or("").trim().to_string();
    if plausible(&footer) { return footer }
    let out = crate::proc::std_cmd(crate::proc::Kind::Local, "date").arg("+%z").env("TZ", name).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    if out.len() == 5 && (out.starts_with('+') || out.starts_with('-')) { fixed_rule(&out) } else { String::new() }
}

fn err(id: &Value, m: &str) -> Value { json!({"t":"reply","id":id,"ok":false,"error":m}) }
fn ok(id: &Value) -> Value { json!({"t":"reply","id":id,"ok":true}) }
fn ev(k: &str, v: &str, at: u64) -> Value { json!({"$":"SettingSet","key":k,"value":v,"at":at}) }

pub struct ZoneRpc;
impl RpcHandler for ZoneRpc {
    fn methods(&self) -> &'static [&'static str] { &["zone.report", "zone.set"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let zone = p.get("zone").and_then(Value::as_str).unwrap_or("").trim().to_string();
            let at = now_ms() / 1000;
            let write = |evs: Vec<Value>| { let hub = hub.clone(); async move { hub.commit_many(&evs).await.map_err(|e| format!("{e:#}")) } };
            if method == "zone.report" {
                if !valid(&zone) { return vec![err(id, "not a time zone name")] }
                if hub.setting("person.tz.device").await.unwrap_or_default() == zone { return vec![ok(id)] }
                return match write(vec![ev("person.tz.device", &zone, at)]).await { Ok(_) => vec![ok(id)], Err(e) => vec![err(id, &e)] };
            }
            // zone.set: "" or "auto" follows the phone; a zone is set once its rule is known (looked up first when it is not)
            if zone.is_empty() || zone == "auto" { return match write(vec![ev("person.tz", "", at)]).await { Ok(_) => vec![ok(id)], Err(e) => vec![err(id, &e)] } }
            if !valid(&zone) { return vec![err(id, "that is not a time zone name (like Europe/Helsinki)")] }
            let held = hub.setting(&format!("tz.rule.{zone}")).await.unwrap_or_default();
            if utc(&zone) || plausible(&held) { return match write(vec![ev("person.tz", &zone, at)]).await { Ok(_) => vec![ok(id)], Err(e) => vec![err(id, &e)] } }
            let z2 = zone.clone();
            let r = tokio::task::spawn_blocking(move || rule(&z2)).await.unwrap_or_default();
            if r.is_empty() { return vec![err(id, &format!("I don't know a time zone called {zone}"))] }
            let mut evs = vec![]; if r != held { evs.push(ev(&format!("tz.rule.{zone}"), &r, at)) }
            evs.push(ev("person.tz", &zone, at));
            match write(evs).await { Ok(_) => vec![json!({"t":"reply","id":id,"ok":true,"zone":zone})], Err(e) => vec![err(id, &e)] }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hub::Cfg, presenter::Canned};
    #[test] fn zone_names_are_checked_before_any_lookup() {
        for ok in ["Europe/Helsinki", "UTC", "America/Argentina/Buenos_Aires", "Etc/GMT+5", "a"] { assert!(valid(ok), "{ok}") }
        for bad in ["", "/etc/passwd", "../x", "a//b", "Europe/", "has space", "semi;colon", &"a".repeat(65)] { assert!(!valid(bad), "{bad}") }
        assert!(utc("Etc/UTC") && !utc("Europe/Helsinki"));
    }
    /// One zoneinfo folder for every test (the environment is process-wide).
    fn zi() {
        let d = std::env::temp_dir().join(format!("bp-zi-{}", std::process::id())); std::fs::create_dir_all(d.join("Europe")).unwrap();
        std::fs::write(d.join("Europe/Helsinki"), b"TZif2\0\0binary\n\nEET-2EEST,M3.5.0/3,M10.5.0/4\n").unwrap(); std::env::set_var("BACKPLANE_ZONEINFO", &d);
    }
    #[test] fn a_fixed_offset_is_a_posix_rule() { assert_eq!(fixed_rule("+0530"), "<+0530>-5:30"); assert_eq!(fixed_rule("-0300"), "<-0300>3"); assert_eq!(fixed_rule("+0000"), "<+0000>-0"); }
    #[test] fn the_rule_is_the_footer_of_the_tzif_file() {
        zi();
        assert_eq!(rule("Europe/Helsinki"), "EET-2EEST,M3.5.0/3,M10.5.0/4"); assert_eq!(rule("Europe/Nowhere"), ""); assert_eq!(rule("../../etc/passwd"), "");
    }
    #[tokio::test(flavor = "multi_thread")] async fn report_and_set_write_the_settings_the_bend_hub_writes() {
        zi();
        let home = std::env::temp_dir().join(format!("bp-zh-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap();
        let hub = Hub::start(Cfg { home: home.clone(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let call = |p: Value, m: &'static str| { let hub = hub.clone(); async move { ZoneRpc.call(&hub, &Caller { gen: "1".into(), client: 1 }, &json!(3), m, &p).await.remove(0) } };
        assert_eq!(call(json!({"zone":"not a zone"}), "zone.report").await["error"], "not a time zone name");
        assert_eq!(call(json!({"zone":"Europe/Helsinki"}), "zone.report").await["ok"], true); assert_eq!(hub.setting("person.tz.device").await.as_deref(), Some("Europe/Helsinki"));
        let n = hub.store.len(); call(json!({"zone":"Europe/Helsinki"}), "zone.report").await; assert_eq!(hub.store.len(), n, "the same zone reported again writes nothing");
        let r = call(json!({"zone":"Europe/Helsinki"}), "zone.set").await; assert_eq!((r["ok"].clone(), r["zone"].clone()), (json!(true), json!("Europe/Helsinki")));
        assert_eq!(hub.setting("person.tz").await.as_deref(), Some("Europe/Helsinki")); assert_eq!(hub.setting("tz.rule.Europe/Helsinki").await.as_deref(), Some("EET-2EEST,M3.5.0/3,M10.5.0/4"));
        assert_eq!(call(json!({"zone":"Mars/Olympus"}), "zone.set").await["error"], "I don't know a time zone called Mars/Olympus");
        assert_eq!(call(json!({"zone":"a b"}), "zone.set").await["error"], "that is not a time zone name (like Europe/Helsinki)");
        assert_eq!(call(json!({"zone":"auto"}), "zone.set").await["ok"], true); assert_eq!(hub.setting("person.tz").await, None, "follows the phone again");
        let _ = std::fs::remove_dir_all(&home);
    }
}
