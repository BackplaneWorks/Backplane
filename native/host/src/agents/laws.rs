//! Rust statements of LAWS's turn, queue, ledger, background-work and usage laws that this module ported. Each test is named after its law
//! (scripts/law-map.tsv maps law -> `rust <test>`); the claim is the law's, stated over the Rust code.
use super::{bg, cmds::{Cmd, Cmds}, lines::{speed_value, wake_text, Cx, Flow}, tools::wait_clamp, world::World};
use serde_json::{json, Value};

fn lc(u: &str, st: &str) -> Value { json!({"type":"command_lifecycle","command_uuid":u,"state":st}) }
fn result() -> Value { json!({"type":"result","is_error":false,"result":"ok"}) }
fn failed() -> Value { json!({"type":"result","is_error":true,"subtype":"error_during_execution"}) }
fn note() -> Value { json!({"type":"system","subtype":"task_notification","task_id":"b1","summary":"done"}) }
fn init(v: &str) -> Value { json!({"type":"system","subtype":"init","claude_code_version":v}) }
fn one(u: &str, st: u8, wake: bool) -> Cmds { Cmds { cs: vec![Cmd { uuid: u.into(), st, err: false, text: String::new() }], mode: 1, wake } }
fn new() -> Cmds { Cmds::new() }
fn running_cx() -> Cx { let mut c = Cx::new("t1"); c.running = true; c }
/// Bg.demo: a thread with one background shell at work
fn demo() -> Cx { let mut c = Cx::new("t1"); c.bg = bg::lines("a\tx\tc1\tsleep 8\tshell"); c.bg_text = bg::show(&c.bg); c }
fn sys(c: &mut Cx, v: Value) { c.line(&v, 2); }

// ---- the command ledger ----
#[test] fn cmds_foreign_result_quiet() { assert_eq!(one("u1", 1, false).verdict(true, &result()), 1) }
#[test] fn cmds_foreign_result_keeps_running() { let mut c = running_cx(); let fx = c.line_flow(&result(), 9, Flow::Quiet); assert!(c.running && fx.events.iter().all(|e| e["$"] != "TurnChanged")) }
#[test] fn cmds_result_waits_lc() { assert_eq!(one("u1", 2, false).verdict(true, &result()), 1) }
#[test] fn cmds_lc_completed_ends() { assert_eq!(one("u1", 2, false).see(&result()).verdict(true, &lc("u1", "completed")), 3) }
#[test] fn cmds_lc_end_completes() { let mut c = running_cx(); let ev = c.end(3, "ok", 9); assert_eq!(ev[0]["state"], "completed"); assert!(!c.running) }
#[test] fn cmds_pair_stays_running() { let c = Cmds { cs: vec![Cmd { uuid: "u3".into(), st: 2, err: false, text: String::new() }, Cmd { uuid: "u4".into(), st: 1, err: false, text: String::new() }], mode: 1, wake: false }; assert_eq!(c.see(&result()).verdict(true, &lc("u3", "completed")), 1) }
#[test] fn cmds_tomb_error_quiet() { assert_eq!(one("u1", 2, false).stop(&[]).verdict(false, &failed()), 1) }
#[test] fn cmds_tomb_cancel_quiet() { assert_eq!(one("u1", 2, false).stop(&[]).see(&failed()).verdict(false, &lc("u1", "cancelled")), 1) }
#[test] fn cmds_stop_then_new_error_quiet() { assert_eq!(one("u1", 2, false).stop(&[]).accept("u2").sent("u2").verdict(true, &failed()), 1) }
#[test] fn cmds_stop_then_new() { let c = one("u1", 2, false).stop(&[]).accept("u2").sent("u2").see(&failed()).see(&lc("u1", "cancelled")).see(&lc("u2", "started")).see(&result()); assert_eq!(c.verdict(true, &lc("u2", "completed")), 3) }
#[test] fn cmds_wake_start() { assert_eq!(Cmds { cs: vec![], mode: 1, wake: false }.verdict(false, &note()), 6) }
#[test] fn cmds_wake_runs() { let mut c = Cx::new("t1"); let ev = c.wake_start("Woken: x", "m1", 7, 9); assert!(c.running); assert_eq!(ev[0]["state"], "running"); assert_eq!(ev[4]["kind"], "wake") }
#[test] fn cmds_wake_result_ends() { assert_eq!(Cmds { cs: vec![], mode: 1, wake: true }.verdict(true, &result()), 3) }
#[test] fn cmds_wake_with_queued_user() { assert_eq!(one("u1", 1, true).verdict(true, &result()), 1) }
#[test] fn cmds_tomb_started_resumes() { assert_eq!(one("u1", 4, false).verdict(false, &lc("u1", "started")), 6) }
#[test] fn cmds_tomb_started_says() { assert_eq!(wake_text(true, &[], &lc("u1", "started")), "Resumed after Stop") }
#[test] fn cmds_prep_counts() { assert_eq!(one("u1", 0, true).verdict(true, &result()), 1) }
#[test] fn cmds_mode_version() { assert_eq!(new().see(&init("2.1.286")).mode, 1) }
#[test] fn cmds_mode_old() { assert_eq!(new().see(&init("2.0.0")).mode, 2) }
#[test] fn cmds_mode_old_legacy() { assert_eq!(new().see(&init("2.0.0")).verdict(true, &result()), 0) }
#[test] fn cmds_mode_lc() { assert_eq!(new().see(&lc("u1", "queued")).mode, 1) }
#[test] fn cmds_legacy_retires_sent() { assert_eq!(new().see(&init("2.0.0")).accept("u1").sent("u1").cs.len(), 0) }
#[test] fn cmds_legacy_learned_retires() { assert_eq!(new().accept("u1").sent("u1").see(&init("2.0.0")).cs.len(), 0) }
#[test] fn cmds_legacy_prep_stays() { assert_eq!(new().accept("u1").see(&init("2.0.0")).cs.len(), 1) }
#[test] fn cmds_lifecycle_keeps_sent() { assert_eq!(new().see(&init("2.1.286")).accept("u1").sent("u1").cs.len(), 1) }
// cmds_legacy_same: the legacy verdict is the old rule exactly
#[test] fn cmds_legacy_same() {
    let lines = [result(), failed(), json!({"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"x"}}}), init("2.0.0"), json!({"type":"assistant","message":{"id":"m","content":[{"type":"text","text":"t"}]}})];
    for l in lines { let (mut a, mut b) = (running_cx(), running_cx()); assert_eq!(a.line(&l, 5), b.line_flow(&l, 5, Flow::Legacy)); assert_eq!(a.running, b.running) }
}
#[test] fn wake_act_once() { assert_eq!(Cmds { cs: vec![], mode: 1, wake: true }.verdict(true, &note()), 2) }

// ---- stuck turns ----
#[test] fn stuck_ends_ok() { assert_eq!(one("u", 3, false).stuck(true, 1, 4, 0, 3), 3) }
#[test] fn stuck_ends_failed_or_stopped() {
    let mut f = one("u", 3, false); f.cs[0].err = true; f.cs[0].text = "x".into(); assert_eq!(f.stuck(true, 1, 4, 0, 3), 4);
    let mut s = Cmds { cs: vec![Cmd { uuid: "u".into(), st: 4, err: false, text: String::new() }, Cmd { uuid: "v".into(), st: 3, err: true, text: "x".into() }], mode: 1, wake: false }; assert_eq!(s.stuck(true, 1, 4, 0, 3), 5); s.mode = 1;
}
#[test] fn stuck_waits() {
    let n = one("u", 3, false).stuck(true, 1, 3, 0, 3) + one("u", 2, false).stuck(true, 1, 4, 0, 3) + Cmds { cs: vec![], mode: 1, wake: true }.stuck(true, 1, 4, 0, 3) + Cmds { cs: vec![], mode: 1, wake: false }.stuck(true, 1, 4, 1, 3)
        + Cmds { cs: vec![], mode: 1, wake: false }.stuck(false, 1, 4, 0, 3) + Cmds { cs: vec![], mode: 2, wake: false }.stuck(true, 1, 4, 0, 3);
    assert_eq!(n, 0)
}
#[test] fn stuck_step_ends_turn() { let mut c = running_cx(); let ev = c.end(3, "", 9); assert_eq!(ev[0]["state"], "completed"); assert!(!c.running) }

// ---- background work ----
#[test] fn bg_start_progress_end() { assert_eq!(bg::show(&bg::drop(&bg::set(&bg::add(&bg::lines("a\tx"), "b", "y"), "b", "z"), "a")), "b\tz") }
#[test] fn bg_added_once() { assert_eq!(bg::show(&bg::add(&bg::lines("a\tx"), "a", "y")), "a\tx") }
#[test] fn bg_progress_unlisted() { assert_eq!(bg::show(&bg::set(&bg::lines("a\tx"), "b", "z")), "a\tx") }
#[test] fn bg_doing() { assert_eq!(bg::doing("a\tRunning tests"), "Running tests") }
#[test] fn bg_start_keeps_call() { assert_eq!(bg::show(&bg::set(&bg::start(&[], "a", "x", "c1", "List files", "subagent"), "a", "Running ls")), "a\tRunning ls\tc1\tList files\tsubagent") }
#[test] fn bg_call_title() { let l = "a\tx\tc1\tList files"; assert_eq!(format!("{}|{}", bg::field(l, 2), bg::field(l, 3)), "c1|List files") }
#[test] fn bg_sync_ends_absent() { assert_eq!(bg::show(&bg::sync(&bg::lines("a\tx\nb\ty"), &["b".to_string()])), format!("{}\nb\ty", bg::line("a", &format!("{}done", bg::ENDED)))) }
#[test] fn bg_sync_never_adds() {
    let mut seed = 99u64; let mut r = move || { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (seed >> 33) as usize };
    for _ in 0..200 { let ls: Vec<String> = (0..r() % 6).map(|i| format!("id{i}\tdoing{}", r() % 3)).collect(); let open: Vec<String> = (0..r() % 6).map(|_| format!("id{}", r() % 9)).collect(); assert_eq!(bg::sync(&ls, &open).len(), ls.len()) }
}
#[test] fn bg_updated_ends_not_drops() {
    let mut c = demo(); sys(&mut c, json!({"type":"system","subtype":"task_updated","task_id":"a","patch":{"status":"completed"}}));
    assert_eq!(c.bg.len(), 1); assert!(bg::ended(&c.bg[0])); assert_eq!(bg::doing(&c.bg[0]), format!("{}completed", bg::ENDED)); assert_eq!(bg::field(&c.bg[0], 4), "shell")
}
#[test] fn bg_notification_drops() { let mut c = demo(); sys(&mut c, json!({"type":"system","subtype":"task_notification","task_id":"a","status":"completed"})); assert_eq!(c.bg.len(), 0) }
#[test] fn bg_empty_snapshot_keeps_pending() { let mut c = demo(); sys(&mut c, json!({"type":"system","subtype":"background_tasks_changed","tasks":[]})); assert_eq!((c.bg.len(), bg::running(&c.bg)), (1, 0)) }
#[test] fn bg_turn_end_settles_ended() { let mut c = Cx::new("t1"); c.bg = bg::lines(&format!("{}\nb\ty", bg::line("a", &format!("{}done", bg::ENDED)))); c.bg_text = bg::show(&c.bg); c.bg_settle(2); assert_eq!(c.bg.len(), 1) }
fn l1() -> Value { json!({"type":"system","subtype":"task_progress","task_id":"a","description":"one"}) }
fn l2() -> Value { json!({"type":"system","subtype":"task_progress","task_id":"a","description":"two"}) }
#[test] fn bg_progress_coalesced() {
    let p = bg::Prog { at: 100, pend: vec![] }; let (a, b) = (p.line(102, l1()), p.line(105, l1()));
    assert_eq!(format!("{}/{}/{}", a.flush.len(), a.pend.len(), b.flush.len()), "0/1/1")
}
#[test] fn bg_progress_one_held_per_task() { let p = bg::Prog { at: 100, pend: vec![] }; let s = p.line(101, l1()); let p2 = bg::Prog { at: s.at, pend: s.pend }; assert_eq!(p2.line(102, l2()).pend.len(), 1) }
#[test] fn bg_progress_beat_waits() { assert_eq!(bg::Prog { at: 100, pend: vec![l1()] }.beat(103).flush.len(), 0) }
#[test] fn bg_progress_latest_lands() {
    let s = bg::Prog { at: 100, pend: vec![] }.line(101, l1()); let s = bg::Prog { at: s.at, pend: s.pend }.line(102, l2()); let b = bg::Prog { at: 100, pend: s.pend }.beat(105);
    let mut a = demo(); a.progress(&b.flush, 3); let mut c = demo(); sys(&mut c, l2()); assert_eq!(bg::show(&a.bg), bg::show(&c.bg))
}
#[test] fn bg_progress_is_own_line() { let v = l1(); assert!(v["type"] == "system" && v["subtype"] == "task_progress" && v["parent_tool_use_id"].as_str().unwrap_or("").is_empty()) }
#[test] fn wake_says_killed() { assert_eq!(wake_text(false, &[], &json!({"task_id":"a","status":"killed"})), "Woken: background work (killed)") }
#[test] fn wake_completed_plain() { assert_eq!(wake_text(false, &[], &json!({"task_id":"a","status":"completed"})), "Woken: background work") }
// wait_busy_not_done / wait_idle_done: a thread is done when no turn runs and nothing is out of hand (its background rows, ended ones included)
fn done(w: &World, th: &str) -> bool { !w.running(th) && w.work(th).0 == 0 }
fn world(rows: &str) -> World { let mut w = World::default(); w.apply(r#"{"$":"ThreadCreated","id":"t1","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}"#); if !rows.is_empty() { w.apply(&json!({"$":"WorkSet","thread":"t1","kind":"bg","rows":rows,"at":3}).to_string()) } w }
#[test] fn wait_busy_not_done() { assert!(!done(&world("a\tx\tc1\tsleep 8\tshell"), "t1")) }
#[test] fn wait_idle_done() { assert!(done(&world(""), "t1")) }
#[test] fn wait_at_least_a_second() { assert_eq!(wait_clamp(0), 1) }
#[test] fn wait_at_most_ten_minutes() { assert_eq!(wait_clamp(9_000_000), 600) }

// ---- Claude's lines ----
#[test] fn claude_result_running_completes() { let mut c = running_cx(); assert_eq!(c.line(&json!({"type":"result","is_error":false}), 9).events[0]["state"], "completed") }
#[test] fn claude_result_after_stop_interrupted() { let mut c = Cx::new("t1"); assert!(c.line(&json!({"type":"result","is_error":false}), 9).events.iter().all(|e| e["$"] != "TurnChanged")) }
#[test] fn claude_delta_running_streams() { let mut c = running_cx(); assert_eq!(c.line(&json!({"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"live"}}}), 9).deltas, ["live"]) }
#[test] fn claude_text_ids_distinct() {
    let mut ids = vec![]; for u in ["aaaaaaaa-1111", "bbbbbbbb-2222", ""] { let mut c = Cx::new("t"); let fx = c.line(&json!({"type":"assistant","uuid":u,"message":{"id":"m","content":[{"type":"text","text":"x"}]}}), 1); ids.push(fx.events[0]["msg"].as_str().unwrap().to_string()) }
    assert_eq!(ids, ["m:0:aaaaaaaa", "m:0:bbbbbbbb", "m:0"])
}
// ---- usage and speed ----
fn merge(old: &str, used: u64, window: u64) -> String { let mut w = World::default(); if !old.is_empty() { let (u, win) = old.split_once('/').unwrap(); w.apply(&json!({"$":"UsageSet","thread":"t1","used":u.parse::<u64>().unwrap(),"window":win.parse::<u64>().unwrap(),"at":1}).to_string()) } w.apply(&json!({"$":"UsageSet","thread":"t1","used":used,"window":window,"at":2}).to_string()); w.setting("usage.t1").to_string() }
#[test] fn usage_keeps_window() { assert_eq!(merge("10/200", 5, 0), "5/200") }
#[test] fn usage_keeps_used() { assert_eq!(merge("10/200", 0, 400), "10/400") }
#[test] fn usage_change_logged() { let mut c = Cx::new("t1"); c.used = 7; let fx = c.line(&json!({"type":"assistant","message":{"id":"m","content":[],"usage":{"output_tokens":8}}}), 2); assert_eq!(fx.events, vec![json!({"$":"UsageSet","thread":"t1","used":8,"window":0,"at":2})]) }
#[test] fn speed_unseen_untold() { assert_eq!(speed_value("", 5, 3, 1, 1), "5/3/0/0/1/0/0/1") }
#[test] fn speed_told_rise() { assert_eq!(speed_value("2/1/0/0/0/0/0/0", 5, 4, 1, 1), "5/4/3/3/1/3/3/1") }

// ---- small rules of hub / prompt ----
#[test] fn claude_kistack_plugin() { let a = super::rules::claude_args(&super::rules::ArgsIn { model: "", session: "", runtime: "full-access", interaction: "default", prompt: "P", dirs: &[], effort: "", think: false, plugins: &["/k/rev".to_string()], mcp_url: "u" }); let i = a.iter().position(|x| x == "--plugin-dir").unwrap(); assert_eq!(&a[i..i + 2], ["--plugin-dir", "/k/rev"]) }
#[test] fn tool_summary_question() { assert_eq!(super::lines::tool_summary("mcp__backplane__order_ask", &json!({"question":"Ship where?"})), "Ship where?") }
#[test] fn tool_summary_says_what_a_search_a_fetch_and_a_task_update_did() {
    assert_eq!(super::lines::tool_summary("WebSearch", &json!({"query":"TPS54331 datasheet"})), "TPS54331 datasheet");
    assert_eq!(super::lines::tool_summary("WebFetch", &json!({"url":"https://ti.com/x","prompt":"pinout"})), "https://ti.com/x");
    assert_eq!(super::lines::tool_summary("TaskUpdate", &json!({"taskId":"3","status":"completed"})), "#3 completed");
    assert_eq!(super::lines::tool_summary("TaskUpdate", &json!({})), "TaskUpdate");
}
#[test] fn tool_summary_todo() { assert_eq!(super::lines::todo_summary(&json!({"todos":[{"content":"Read","status":"completed"},{"content":"Edit","status":"in_progress"},{"content":"Test","status":"pending"}]})), "1 of 3 done · now: Edit") }
#[test] fn todo_write_sets_list() { let mut c = Cx::new("t1"); let fx = c.line(&json!({"type":"assistant","message":{"id":"m","content":[{"type":"tool_use","id":"u","name":"TodoWrite","input":{"todos":[]}}]}}), 5); assert_eq!(fx.events.iter().filter(|e| e["$"] == "TodosChanged").count(), 1) }
#[test] fn interrupt_claude_input() { let v: Value = serde_json::from_str(&super::rules::interrupt_line("r1")).unwrap(); assert_eq!(v, json!({"type":"control_request","request_id":"r1","request":{"subtype":"interrupt"}})) }
#[test] fn resume_carry_tells_cut() { assert!(super::rules::CARRY_CUT.contains("its result was lost")); assert!(format!("{} {}", super::world::CARRY_TEXT, super::rules::CARRY_CUT).starts_with(super::world::CARRY_TEXT)) }
#[test] fn lost_text_not_carry_text() { for ls in [vec![], vec!["a\tx".to_string()]] { assert_ne!(bg::lost_text(&ls, "restart"), super::world::CARRY_TEXT) } }
