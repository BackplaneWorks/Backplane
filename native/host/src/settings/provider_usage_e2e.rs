//! Remaining usage of the providers without accounts end to end (#1337): a real hub on a temp home, stand-in `agy`, `grok` and `pi` CLIs (no model runs, nothing of the
//! person's is read), the reader taking each one's reading the way the real CLI gives it (`agy -p /quota`, Grok's ACP `_x.ai/billing`), and what Settings › Accounts
//! (the phones' `accounts.page` over data/1, the desktop's `accounts` list: the same `people`) and `account_list` show of them.
use crate::{agents::{proc::Cli, Agents}, hub::{Cfg, Hub}, presenter::Canned, settings::{accounts_hub, provider_usage as pu}};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};

fn now() -> u64 { crate::hub::now_ms() / 1000 }
fn script(dir: &std::path::Path, name: &str, body: &str) {
    use crate::plat::fs::PermissionsExt;
    let p = dir.join(name); std::fs::write(&p, body).unwrap(); std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn antigravity_and_grok_usage_is_read_from_their_clis_and_shown_with_what_pi_does_not_report() {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;
    let (home, bin) = (crate::plat::tmp::dir("pu-home"), crate::plat::tmp::dir("pu-bin"));
    let t = now(); let (r5, r7) = (crate::google::cal::iso(t + 3 * 3600), crate::google::cal::iso(t + 5 * 86400));
    // agy 1.5's answer to `agy -p /quota`: Gemini has room, the Claude and GPT models' 5-hour window is spent
    script(&bin, "agy", &format!("#!/bin/sh\n[ \"$1\" = \"-p\" ] && [ \"$2\" = \"/quota\" ] || exit 9\nprintf 'Gemini Models\\tWeekly Limit Remaining\\t94%%\\t{r7}\\nGemini Models\\tFive Hour Limit Remaining\\t90%%\\t{r5}\\nClaude and GPT models\\tWeekly Limit Remaining\\t60%%\\t{r7}\\nClaude and GPT models\\tFive Hour Limit Remaining\\t0%%\\t{r5}\\n'\n"));
    // Grok Build's ACP server: it answers initialize, then the billing request (and only once initialize has been answered, as the reader sends it)
    script(&bin, "grok", &format!(r#"#!/usr/bin/env python3
import sys, json
if sys.argv[1:] != ["agent", "stdio"]: sys.exit(9)
init = False
for line in sys.stdin:
    q = json.loads(line)
    if q["method"] == "initialize": init = True; print(json.dumps({{"jsonrpc":"2.0","id":q["id"],"result":{{"protocolVersion":1}}}}), flush=True)
    elif q["method"] == "_x.ai/billing" and init: print(json.dumps({{"jsonrpc":"2.0","id":q["id"],"result":{{"config":{{"creditUsagePercent":19.0,"currentPeriod":{{"type":"USAGE_PERIOD_TYPE_WEEKLY","end":"{r7}"}}}},"subscription_tier":"SuperGrok"}}}}), flush=True)
    else: print(json.dumps({{"jsonrpc":"2.0","id":q["id"],"error":{{"code":-32601,"message":"Method not found"}}}}), flush=True)
"#));
    script(&bin, "pi", "#!/bin/sh\nexit 0\n");
    std::fs::write(home.join("events.jsonl"), format!("{{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"f\",\"root\":\"/tmp\",\"at\":1700000000}}\n{{\"$\":\"ThreadCreated\",\"id\":\"t1\",\"project\":\"p1\",\"title\":\"On Pi\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"pi\",\"model\":\"\",\"at\":1700000000}}\n")).unwrap();
    std::fs::write(home.join("token"), "tok").unwrap();
    let hub = Hub::start(Cfg { home: home.to_path_buf(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let ag = Agents::start_with(hub.clone(), Cli::standin(&bin)).await.unwrap();
    crate::commands::start_with(hub.clone(), Some(Arc::new(ag.clone())));
    let _mgr = accounts_hub::start_cli(&hub, Some(ag.clone()), Cli::standin(&bin));
    let _views = crate::statehost::start_with(hub.clone(), Some(true)); // the views' fold, which keeps the day's token totals (passive: none of its policies acts)

    // one look: Antigravity, Grok and Pi are here; Muse, Oh My Pi and OpenCode are not (a stand-in folder blocks every other CLI)
    assert_eq!(pu::refresh(&hub, &Cli::standin(&bin)).await, 3);
    let al = |p: &str| { let h = hub.clone(); let k = pu::key(p); async move { serde_json::from_str::<Value>(&h.setting(&k).await.unwrap_or_default()).unwrap_or(Value::Null) } };
    let agy = al("antigravity").await;
    assert_eq!((agy["state"].as_str(), agy["source"].as_str(), agy["windows"].as_array().map(Vec::len)), (Some("ok"), Some("agy-quota"), Some(4)), "{agy}");
    let grok = al("grok").await;
    assert_eq!((grok["windows"][0]["used_percent"].as_f64(), grok["windows"][0]["resets_at"].as_u64(), grok["plan"].as_str()), (Some(19.0), Some(t + 5 * 86400), Some("SuperGrok")), "{grok}");
    assert_eq!(al("pi").await["state"], "unreported");
    assert!(hub.setting(&pu::key("muse")).await.unwrap_or_default().is_empty());
    assert_eq!(pu::refresh(&hub, &Cli::standin(&bin)).await, 0, "the same readings again a moment later are not written again");
    // Pi met a rate limit (as the failover driver records it) and ran two turns today
    hub.commit_many(&[json!({"$":"SettingSet","key":pu::seen_key("pi"),"value":json!({"scope":"pi","reason":"429 Too Many Requests","reset":now() + 1800,"at":now() - 30}).to_string(),"at":now()}),
        json!({"$":"TurnUsage","thread":"t1","provider":"pi","model":"","key":"k","input":1200,"cread":0,"cwrite":0,"output":800,"reasoning":0,"cost":0,"known":"o","basis":"b","at":now()}),
        json!({"$":"TurnUsage","thread":"t1","provider":"pi","model":"","key":"k","input":100,"cread":0,"cwrite":0,"output":50,"reasoning":0,"cost":0,"known":"o","basis":"b","at":now()})]).await.unwrap();

    let end = std::time::Instant::now() + Duration::from_secs(60);
    while crate::statehost::with_state_of(&hub, |st| st.settings.keys().any(|k| k.starts_with("tok.") && k.ends_with("|pi") && st.settings[k].starts_with("2/"))) != Some(true) {
        assert!(std::time::Instant::now() < end, "the views' fold has the day's totals"); tokio::time::sleep(Duration::from_millis(50)).await }

    // the phone: data/1 `accounts.page`, whose `people` the desktop's Settings › Accounts draws too (the `accounts` list answer)
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(); let port = l.local_addr().unwrap().port();
    { let hub = hub.clone(); tokio::spawn(async move { loop { let Ok((st, _)) = l.accept().await else { return }; let h = hub.clone(); tokio::spawn(async move { let _ = crate::wire::serve_with(h, st, false).await; }); } }); }
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws?data=1&enc=json&token=tok")).await.unwrap();
    let _hello = loop { if let Message::Text(t) = ws.next().await.unwrap().unwrap() { break serde_json::from_str::<Value>(t.as_str()).unwrap() } };
    ws.send(Message::Text(json!({"id":1,"m":"accounts.page","p":{}}).to_string().into())).await.unwrap();
    let page = loop { let m = tokio::time::timeout(Duration::from_secs(60), ws.next()).await.expect("an answer").unwrap().unwrap(); if let Message::Text(t) = m { let v: Value = serde_json::from_str(t.as_str()).unwrap(); if v["t"] == "reply" && v["id"] == 1 { break v } } };
    assert_eq!(page["ok"], true, "{page}");
    let people = page["result"]["people"].as_array().unwrap().clone();
    let entry = |k: &str| people.iter().find(|e| e["key"] == k).unwrap_or_else(|| panic!("no {k}: {people:#?}"))["logins"][0].clone();
    let a = entry("provider:antigravity");
    assert_eq!((a["title"].as_str(), a["line"].as_str(), a["out"].as_bool()), (Some("Antigravity"), Some("connected"), Some(false)));
    let bars: Vec<(String, String)> = a["bars"].as_array().unwrap().iter().map(|b| (b["name"].as_str().unwrap().to_string(), b["tone"].as_str().unwrap().to_string())).collect();
    assert_eq!(bars.iter().map(|b| b.0.as_str()).collect::<Vec<_>>(), ["Gemini weekly", "Gemini 5-hour", "Claude and GPT weekly", "Claude and GPT 5-hour"]);
    assert_eq!(bars[3].1, "bad", "the spent window reads so"); assert!(a["bars"][1]["text"].as_str().unwrap().starts_with("10% used · resets "), "{a}");
    let g = entry("provider:grok"); assert_eq!((g["plan"].as_str(), g["bars"][0]["name"].as_str(), g["bars"][0]["used"].as_f64()), (Some("SuperGrok"), Some("Weekly"), Some(19.0)));
    let p = entry("provider:pi");
    assert_eq!((p["line"].as_str(), p["out"].as_bool(), p["note"].as_str()), (Some("usage not reported"), Some(true), Some("Usage not reported by Pi. Seen here today: 2 turns, 2k tokens")), "{p}");
    assert!(p["hold"].as_str().unwrap().starts_with("rate-limited until "), "{p}");

    // account_list, as an agent of the Pi thread asks it
    let (ok, r) = ag.tool_accounts("t1", &json!({})).await; assert!(ok, "{r}");
    let row = |pv: &str| r["accounts"].as_array().unwrap().iter().find(|a| a["provider"] == pv).cloned().unwrap_or_else(|| panic!("no {pv}: {r}"));
    assert_eq!((row("antigravity")["remainingPercent"].as_f64(), row("antigravity")["usageReported"].as_bool()), (Some(0.0), Some(true)), "no model named: its fullest window");
    assert_eq!((row("grok")["remainingPercent"].as_f64(), row("grok")["plan"].as_str()), (Some(81.0), Some("SuperGrok")));
    let pi = row("pi");
    assert_eq!((pi["usageReported"].as_bool(), pi["note"].as_str(), pi["held"].as_bool(), pi["current"].as_bool(), pi["seenToday"]["turns"].as_u64()), (Some(false), Some("usage not reported by Pi"), Some(true), Some(true), Some(2)));
    let (_, only) = ag.tool_accounts("t1", &json!({"provider":"grok"})).await;
    assert_eq!(only["accounts"].as_array().unwrap().iter().map(|a| a["provider"].as_str().unwrap()).collect::<Vec<_>>(), ["grok"]);
}
