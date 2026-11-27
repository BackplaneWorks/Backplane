//! The session against a stand-in MCP server (a shell script that answers by tool name and keeps its "design" in a folder), so the
//! harness's paths are exercised without SolidCraft: checkpoint and save around an edit, a stalled call, a server that dies, an
//! interrupted caller, the same failure repeated, the pane opening mid-session.
#![cfg(unix)]
use super::{client::{args, Mode, Spawn}, session::{Seam, Session}, *};
use std::sync::atomic::{AtomicBool, Ordering};

/// The stand-in: one JSON line in, one out. State in `$S`: `path` (the design's file), `dirty`, `log` (one line per start and call).
const SERVER: &str = r#"#!/bin/sh
S="__STATE__"
echo "start $*" >> "$S/log"
ok() { printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[{"type":"text","text":"%s"}]}}\n' "$1" "$2"; }
bad() { printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[{"type":"text","text":"%s"}],"isError":true}}\n' "$1" "$2"; }
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
  [ -z "$id" ] && continue
  case "$line" in
    *'"method":"initialize"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instructions":"stand-in","capabilities":{"tools":{}}}}\n' "$id"; continue;;
    *'"method":"tools/list"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"tools":[{"name":"measure","description":"Measure","inputSchema":{"type":"object"}},{"name":"quit_app","description":"Quit","inputSchema":{"type":"object"}}]}}\n' "$id"; continue;;
  esac
  name=$(printf '%s' "$line" | sed -n 's/.*"name":"\([a-z_]*\)".*/\1/p')
  path=$(printf '%s' "$line" | sed -n 's/.*"path":"\([^"]*\)".*/\1/p')
  cmd=$(printf '%s' "$line" | sed -n 's/.*"command":"\([^"]*\)".*/\1/p')
  echo "call $name $cmd $path" >> "$S/log"
  case "$name" in
    inspect_design)
      p=null; [ -s "$S/path" ] && p="\\\"$(cat "$S/path")\\\""
      d=false; [ -f "$S/dirty" ] && d=true
      ok "$id" "{\\\"path\\\":$p,\\\"dirty\\\":$d,\\\"timeline\\\":[{\\\"id\\\":1,\\\"name\\\":\\\"Base\\\"}],\\\"params\\\":[]}";;
    measure) if [ -f "$S/die_once" ]; then rm "$S/die_once"; echo "kernel panic" >&2; exit 3; fi
      ok "$id" "{\\\"bodies\\\":[{\\\"name\\\":\\\"Body1\\\",\\\"closed\\\":true,\\\"bbox\\\":{\\\"min\\\":[0,0,0],\\\"max\\\":[4,2,1]},\\\"volume_mm3\\\":8}],\\\"body_count\\\":1,\\\"total\\\":{\\\"volume_mm3\\\":8}}";;
    open) printf '%s' "$path" > "$S/path"; case "$path" in *.step) : > "$S/path";; esac; rm -f "$S/dirty"; ok "$id" "{}";;
    save) [ -n "$path" ] && printf '%s' "$path" > "$S/path"; rm -f "$S/dirty"; n=$(grep -c "call save" "$S/log"); printf 'saved %s' "$n" > "$(cat "$S/path")"; ok "$id" "{}";;
    execute) case "$cmd" in
        hang) exec sleep 600;;
        slow) sleep 1; : > "$S/dirty"; ok "$id" "{}";;
        bad) bad "$id" "no such edge";;
        *) : > "$S/dirty"; ok "$id" "{}";; esac;;
    screenshot) printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[{"type":"image","data":"AAAA","mimeType":"image/png"},{"type":"text","text":"{}"}]}}\n' "$id";;
    cancel) ok "$id" "cancelled";;
    *) printf '{"jsonrpc":"2.0","id":%s,"error":{"code":-32602,"message":"unknown tool"}}\n' "$id";;
  esac
done
"#;

struct Rig { _tmp: crate::plat::tmp::TempDir, root: PathBuf, state: PathBuf, seam: Seam, pane: Arc<AtomicBool>, s: Session }
fn rig() -> Rig {
    use std::os::unix::fs::PermissionsExt;
    let tmp = crate::plat::tmp::dir("solidcad");
    let (root, state, dir) = (tmp.path().join("proj"), tmp.path().join("state"), tmp.path().join("home"));
    for d in [&root, &state] { std::fs::create_dir_all(d).unwrap() }
    let prog = tmp.path().join("solidcraft-cli");
    std::fs::write(&prog, SERVER.replace("__STATE__", &state.to_string_lossy())).unwrap();
    std::fs::set_permissions(&prog, std::fs::Permissions::from_mode(0o755)).unwrap();
    let pane = Arc::new(AtomicBool::new(false)); let p2 = pane.clone();
    let seam = Seam { cli: Box::new(move || Some(prog.clone())), control: Box::new(move |_| p2.load(Ordering::SeqCst).then(|| (27878, "tok".to_string()))) };
    let s = Session::new(root.clone(), dir);
    Rig { _tmp: tmp, root, state, seam, pane, s }
}
impl Rig {
    fn log(&self) -> String { std::fs::read_to_string(self.state.join("log")).unwrap_or_default() }
    async fn call(&self, tool: &str, a: Value) -> (bool, Value) { self.s.call(&self.seam, tool, &a, &|_| Box::pin(async { Ok(()) })).await }
    /// A design saved in the project and open in the session.
    async fn opened(&self) -> String {
        let f = self.root.join("a.sc").to_string_lossy().into_owned(); std::fs::write(&f, "v0").unwrap();
        let (ok, _) = self.call("open", json!({"path":"a.sc"})).await; assert!(ok); f
    }
}
/// The harness's own block of an answer.
fn bp(v: &Value) -> Value { v["$content"].as_array().unwrap().iter().rev().find_map(|b| b["text"].as_str().and_then(|t| serde_json::from_str::<Value>(t).ok()).and_then(|j| j.get("backplane").cloned())).unwrap_or(Value::Null) }

#[tokio::test]
async fn an_edit_is_checkpointed_saved_and_digested() {
    let r = rig(); let f = r.opened().await;
    let (ok, v) = r.call("execute", json!({"command":"solid.box","params":{"length":4}})).await;
    assert!(ok, "{v}"); let b = bp(&v);
    assert_eq!(b["saved"], f, "{b}"); assert_eq!(b["design"]["bodies"], 1); assert_eq!(b["design"]["unsaved"], false); assert_eq!(b["design"]["volume_mm3"], 8.0);
    assert_eq!(b["checkpoint"]["of"], f); assert!(b["next"].as_str().unwrap().contains("solidcraft_look"), "{b}");
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "saved 1", "the edit was kept in the design's file");
    let cks = r.s.checkpoints(); assert_eq!(cks.len(), 1); assert_eq!(cks[0].note, "before execute");
    // looking at it settles the reminder
    let (ok, v) = r.s.look(&r.seam, &json!({"views":["iso","top"]})).await; assert!(ok);
    assert_eq!(v["$content"].as_array().unwrap().iter().filter(|b| b["type"] == "image").count(), 2);
    let (_, v) = r.call("execute", json!({"command":"view.fit"})).await; assert!(bp(&v).get("next").is_none(), "a read is no edit: {v}");
    // the restore puts the file back and reopens it, keeping what was there
    let (ok, v) = r.s.checkpoint(&r.seam, &json!({"action":"restore"})).await; assert!(ok, "{v}");
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "v0"); assert_eq!(v["kept"]["note"], "before restoring checkpoint 1");
    assert!(r.log().lines().last().unwrap().starts_with("call measure") && r.log().matches(&format!("call open  {f}")).count() == 2, "{}", r.log());
}

#[tokio::test]
async fn a_stalled_call_is_answered_and_the_design_comes_back() {
    let mut r = rig(); let f = r.opened().await;
    r.s.fixed_budget = Some(Duration::from_millis(400));
    let t0 = Instant::now();
    let (ok, v) = r.call("execute", json!({"command":"hang"})).await;
    assert!(!ok && t0.elapsed() < Duration::from_secs(20), "the call is answered, never left hanging");
    assert!(v["error"].as_str().unwrap().contains("stalled"), "{v}"); assert_eq!(v["recovery"]["session"], "restarted");
    assert!(v["recovery"]["design"].as_str().unwrap().contains(&f) && v["next"].as_str().unwrap().contains("solidcraft_summary"), "{v}");
    assert_eq!(r.log().matches("start mcp --headless").count(), 2, "a new server: {}", r.log());
    assert!(r.log().trim_end().ends_with(&format!("call open  {f}")), "reopened from the last save: {}", r.log());
    let (ok, _) = r.call("measure", json!({})).await; assert!(ok, "the session works again");
    assert_eq!(r.s.status(&r.seam).await["restarts"], 1);
}

#[tokio::test]
async fn a_server_that_dies_is_replaced_and_a_read_is_asked_again() {
    let r = rig(); r.opened().await;
    std::fs::write(r.state.join("die_once"), "").unwrap();
    let (ok, v) = r.call("measure", json!({})).await;
    assert!(ok, "{v}"); assert!(bp(&v)["notes"][0].as_str().unwrap().contains("restarted"), "{v}");
    // a change is never sent twice on its own
    std::fs::write(r.state.join("die_once"), "").unwrap();
    let (ok, v) = r.s.summary(&r.seam).await; assert!(!ok && v["error"].as_str().unwrap().contains("ended"), "{v}");
}

#[tokio::test]
async fn the_same_failure_three_times_is_called_out() {
    let r = rig(); r.opened().await; let a = json!({"command":"bad"});
    for _ in 0..2 { let (ok, v) = r.call("execute", a.clone()).await; assert!(!ok); assert!(bp(&v).get("stuck").is_none()) }
    let (_, v) = r.call("execute", a.clone()).await; assert!(bp(&v)["stuck"].as_str().unwrap().contains("3 times"), "{v}");
    let (_, v) = r.call("nonesuch", json!({})).await; assert_eq!(v["error"], "unknown tool");
}

#[tokio::test]
async fn an_interrupted_call_is_told_to_the_next_caller() {
    let r = rig(); r.opened().await;
    let gone = tokio::time::timeout(Duration::from_millis(150), r.call("execute", json!({"command":"slow"}))).await;
    assert!(gone.is_err(), "the caller went away mid-call");
    let (ok, v) = r.call("measure", json!({})).await; assert!(ok, "{v}");
    assert!(bp(&v)["notes"][0].as_str().unwrap().contains("solidcraft_execute call was interrupted"), "{v}");
    let (_, v) = r.call("measure", json!({})).await; assert!(bp(&v).is_null(), "said once: {v}");
}

#[tokio::test]
async fn the_session_follows_the_pane() {
    let r = rig(); r.opened().await;
    r.pane.store(true, Ordering::SeqCst);
    let (ok, v) = r.call("measure", json!({})).await; assert!(ok);
    assert!(bp(&v)["notes"][0].as_str().unwrap().contains("now open"), "{v}");
    assert!(r.log().contains("start mcp --connect 127.0.0.1:27878 --compact"), "{}", r.log());
    assert!(!r.log().contains("tok"), "the token is never an argument");
    assert_eq!(r.s.status(&r.seam).await["session"], "pane");
}

#[tokio::test]
async fn a_step_file_has_no_design_file_until_it_is_saved() {
    let r = rig(); std::fs::write(r.root.join("case.step"), "ISO-10303-21;").unwrap();
    let (ok, _) = r.call("open", json!({"path":"case.step"})).await; assert!(ok);
    let (ok, v) = r.call("execute", json!({"command":"solid.hole"})).await; assert!(ok);
    let b = bp(&v); assert!(b.get("saved").is_none() && b["next"].as_str().unwrap().contains("no file yet"), "{b}");
    assert!(r.s.checkpoints().is_empty());
    let (ok, _) = r.call("save", json!({"path":"case.sc"})).await; assert!(ok);
    let (_, v) = r.call("execute", json!({"command":"solid.hole"})).await;
    assert_eq!(bp(&v)["saved"], r.root.join("case.sc").to_string_lossy().as_ref());
    assert_eq!(r.s.checkpoints().len(), 1);
}

#[test]
fn a_bridged_server_gets_the_port_and_the_folders() {
    let a = args(&Spawn { program: "x".into(), mode: Mode::Bridged { port: 7, token: "secret".into() }, allow: vec!["/p".into(), "/h/ck".into()] });
    assert_eq!(a, ["mcp", "--connect", "127.0.0.1:7", "--compact", "--allow-path", "/p", "--allow-path", "/h/ck"]);
    assert_eq!(args(&Spawn { program: "x".into(), mode: Mode::Headless, allow: vec![] }), ["mcp", "--headless", "--compact"]);
}

#[test]
fn a_folder_is_scanned_for_what_it_holds() {
    let t = crate::plat::tmp::dir("solidcad-scan"); let p = t.path();
    assert_eq!(scan(p), (false, false, false));
    std::fs::create_dir_all(p.join("boards/main")).unwrap(); std::fs::write(p.join("boards/main/main.kicad_pcb"), "").unwrap();
    assert_eq!(scan(p), (false, false, true));
    std::fs::create_dir_all(p.join(".git")).unwrap(); std::fs::write(p.join(".git/x.step"), "").unwrap();
    assert_eq!(scan(p).0, false, "hidden folders are not looked into");
    std::fs::create_dir_all(p.join("mech")).unwrap(); std::fs::write(p.join("mech/Case.STEP"), "").unwrap();
    assert_eq!(scan(p), (true, true, true));
}

#[test]
fn the_own_tools_are_named_as_the_harness_knows_them() {
    let names: Vec<String> = own_defs().iter().map(|d| d["name"].as_str().unwrap().to_string()).collect();
    for n in h::OWN { assert!(names.iter().any(|x| x == n), "{n}") }
    assert_eq!(names.len(), h::OWN.len());
}

/// The real program, when `BACKPLANE_SOLIDCRAFT_CLI` names one (`cargo test -- --ignored real_solidcraft`): model a plate with a hole,
/// see it, summarise it, change a parameter, restore the checkpoint, export STEP and STL.
#[tokio::test]
#[ignore]
async fn real_solidcraft_models_a_part() {
    let Some(cli) = cli_on_path() else { eprintln!("no solidcraft-cli: skipped"); return };
    let tmp = crate::plat::tmp::dir("solidcad-real"); let root = tmp.path().join("proj"); std::fs::create_dir_all(root.join("mech")).unwrap();
    let seam = Seam { cli: Box::new(move || Some(cli.clone())), control: Box::new(|_| None) };
    let s = Session::new(root.clone(), tmp.path().join("home"));
    let call = |tool: &'static str, a: Value| { let (s, seam) = (&s, &seam); async move { s.call(seam, tool, &a, &|_| Box::pin(async { Ok(()) })).await } };
    let tools = s.tools(&seam).await.unwrap(); assert!(tools.iter().any(|t| t["name"] == "batch") && tools.len() > 20);
    let (ok, v) = call("set_parameter", json!({"name":"w","value":40})).await; assert!(ok, "{v}");
    let (ok, v) = call("batch", json!({"verbose":false,"commands":[
        {"command":"solid.box","params":{"length":"w","width":20,"height":10,"name":"Base"}},
        {"command":"solid.cylinder","params":{"radius":3,"height":10,"operation":"cut","name":"Hole"}}]})).await;
    assert!(ok, "{v}"); let b = bp(&v); assert_eq!(b["design"]["bodies"], 1, "{b}"); assert!(b["next"].as_str().unwrap().contains("no file yet"));
    let (ok, v) = call("save", json!({"path":"mech/plate.sc"})).await; assert!(ok, "{v}");
    let (ok, v) = s.look(&seam, &json!({"views":["iso"],"width":320,"height":240})).await; assert!(ok, "{v}");
    assert!(v["$content"].as_array().unwrap().iter().any(|b| b["type"] == "image"));
    let (ok, v) = s.summary(&seam).await; assert!(ok, "{v}");
    assert_eq!(v["parameters"][0]["name"], "w"); assert!((v["total"]["volume_mm3"].as_f64().unwrap() - 7929.314).abs() < 0.01, "{v}"); assert!(v["bodies"][0]["mass_g"].as_f64().unwrap() > 0.0);
    let (ok, v) = call("set_parameter", json!({"name":"w","value":80})).await; assert!(ok, "{v}");
    let b = bp(&v); assert_eq!(b["design"]["bbox"]["size"][0], 80.0, "{b}"); assert!(b["saved"].as_str().unwrap().ends_with("plate.sc")); assert!(b["checkpoint"]["id"].as_u64().is_some());
    let (ok, v) = s.checkpoint(&seam, &json!({"action":"restore"})).await; assert!(ok, "{v}"); assert_eq!(v["design"]["bbox"]["size"][0], 40.0, "{v}");
    for out in ["mech/plate.step", "mech/plate.stl"] { let (ok, v) = call("export", json!({"path":out})).await; assert!(ok, "{v}"); assert!(root.join(out).metadata().unwrap().len() > 100) }
    let (ok, v) = call("execute", json!({"command":"solid.fillet","params":{"radius":500}})).await; assert!(!ok, "{v}");
    s.stop().await;
}
